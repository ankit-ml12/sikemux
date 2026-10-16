//! Git commands an agent may not run in a checkout other agents share, since
//! each would take in or throw away their uncommitted work along with its own.
//! In a clone or linked worktree of its own an agent is alone, and may run them.

mod footer;
pub(crate) mod homes;
pub mod shim;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The other agents whose work a git command run in a folder would touch:
/// their uncommitted lines in the paths given there, or with none given,
/// anywhere in the checkout, and the agents working in it. None when the core
/// cannot say.
pub(crate) type Others<'a> = dyn Fn(&Path, &[String]) -> Option<Vec<String>> + 'a;

const SWEEP_HELP: &str = "Commit with the git_commit tool: it takes the lines you wrote, and with `paths`, every change in those files or folders that holds no other agent's lines, such as files your own scripts made. Or stage files by name with `git add <paths>`. In a clone or git worktree of your own these commands are allowed.";

/// Why `command`, a shell command line run in `cwd`, may not run, if it may not.
pub(crate) fn refusal(command: &str, cwd: &Path, others: &Others) -> Option<String> {
    segments(command).iter().find_map(|words| {
        let words = command_words(words)?;
        if let Some(script) = shell_script(words) {
            return refusal(script, cwd, others);
        }
        let (first, args) = words.split_first()?;
        if base_name(first) != "git" {
            return None;
        }
        let mut git = sikemux_process::user_environment::command("git");
        git.current_dir(cwd);
        refused(args, false, git, others)
    })
}

/// Whether `command` runs a git command the guard may refuse, by its words
/// alone, without asking git or the core.
pub(crate) fn may_refuse(command: &str) -> bool {
    segments(command).iter().any(|words| {
        let Some(words) = command_words(words) else {
            return false;
        };
        if let Some(script) = shell_script(words) {
            return may_refuse(script);
        }
        words.split_first().is_some_and(|(first, args)| {
            base_name(first) == "git" && touches(git_args(args), false).is_some()
        })
    })
}

/// The script of `sh -c script`, or of a shell given other flags first.
fn shell_script(words: &[String]) -> Option<&str> {
    let (shell, rest) = words.split_first()?;
    if !is_shell(shell) {
        return None;
    }
    let flags = rest.iter().take_while(|word| word.starts_with('-'));
    let at = flags
        .clone()
        .position(|flag| !flag.starts_with("--") && flag.ends_with('c'))?;
    rest.get(at + 1).map(String::as_str)
}

/// Why `git` with `args` may not run, if it may not. `git` is the real git,
/// started where the command runs, to find the repository with.
/// `private_index` says the command stages into an index of its own rather
/// than the checkout's.
fn refused(args: &[String], private_index: bool, git: Command, others: &Others) -> Option<String> {
    let subcommand = git_args(args);
    let touched = touches(subcommand, private_index)?;
    let options = args
        .get(..args.len() - subcommand.len())
        .unwrap_or_default();
    let checkout = checkout(git, options)?;
    if checkout.linked {
        return None;
    }
    let shown = format!("git {}", subcommand.join(" "));
    match touched {
        Touch::Everything(why) => {
            let who = match others(&checkout.dir, &[]) {
                Some(names) if names.is_empty() => return None,
                Some(names) => {
                    let verb = if names.len() == 1 { "works" } else { "work" };
                    format!("{} also {verb} in this checkout", listed(&names))
                }
                None => "other agents may work in this checkout too".to_owned(),
            };
            Some(format!(
                "Sikemux refused `{shown}`: it {why}, and {who}. {SWEEP_HELP}"
            ))
        }
        Touch::Paths { mut paths, named } => {
            if !named {
                paths.retain(|path| checkout.dir.join(path).symlink_metadata().is_ok());
            }
            if paths.is_empty() {
                return None;
            }
            let names = others(&checkout.dir, &paths).filter(|names| !names.is_empty())?;
            let names = listed(&names);
            Some(format!(
                "Sikemux refused `{shown}`: it throws away uncommitted lines {names} wrote there. Ask {names} with agent_send first, or undo only your own lines by editing them."
            ))
        }
    }
}

/// The checkout git runs in, given the global `options`: the folder it runs
/// in, and whether it is a linked worktree, with its own index and files,
/// rather than a repository's main checkout. None outside a checkout.
struct Checkout {
    dir: PathBuf,
    linked: bool,
}

fn checkout(mut git: Command, options: &[String]) -> Option<Checkout> {
    let output = git
        .args(options)
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
            "--show-prefix",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let (own, common, top) = (lines.next()?, lines.next()?, lines.next()?);
    Some(Checkout {
        dir: Path::new(top).join(lines.next().unwrap_or_default()),
        linked: own != common,
    })
}

enum Touch {
    /// The whole checkout, and what the command does to it.
    Everything(&'static str),
    /// Files whose uncommitted changes the command throws away. `named` says
    /// they follow a `--`, so are surely paths rather than a branch.
    Paths { paths: Vec<String>, named: bool },
}

fn touches(args: &[String], private_index: bool) -> Option<Touch> {
    if let Some(why) = sweeping(args, private_index) {
        return Some(Touch::Everything(why));
    }
    let (sub, rest) = args.split_first()?;
    let valued: &[&str] = match sub.as_str() {
        "checkout" => &["-b", "-B", "--orphan"],
        "restore" => {
            let has = |short: char, long: &str| {
                flags_before_paths(rest).any(|flag| {
                    flag == long
                        || flag
                            .strip_prefix('-')
                            .filter(|letters| !letters.starts_with('-'))
                            .is_some_and(|letters| letters.contains(short))
                })
            };
            if has('S', "--staged") && !has('W', "--worktree") {
                return None;
            }
            &["-s", "--source"]
        }
        _ => return None,
    };
    let (before, after) = match rest.iter().position(|arg| arg == "--") {
        Some(at) => (&rest[..at], Some(&rest[at + 1..])),
        None => (rest, None),
    };
    let named = after.is_some() || sub == "restore";
    let mut paths: Vec<String> = after.into_iter().flatten().cloned().collect();
    if after.is_none() || sub == "restore" {
        let mut words = before.iter();
        while let Some(word) = words.next() {
            if valued.contains(&word.as_str()) {
                words.next();
            } else if !word.starts_with('-') {
                paths.push(word.clone());
            }
        }
    }
    Some(Touch::Paths { paths, named })
}

/// `names` as one would say them: "Otter", "Otter and Briar", "Otter, Briar and Moss".
fn listed(names: &[String]) -> String {
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

fn sweeping(args: &[String], private_index: bool) -> Option<&'static str> {
    let (sub, rest) = args.split_first()?;
    let everything = |paths: &[String]| {
        paths
            .iter()
            .any(|path| matches!(path.as_str(), "." | "./" | ":/" | ":" | "*" | ":(top)"))
    };
    let short = |letter: char, valued: &str| {
        flags_before_paths(rest).any(|flag| {
            flag.strip_prefix('-')
                .filter(|letters| !letters.starts_with('-'))
                .is_some_and(|letters| {
                    letters
                        .chars()
                        .take_while(|c| !valued.contains(*c))
                        .any(|c| c == letter)
                })
        })
    };
    let long = |name: &str| flags_before_paths(rest).any(|flag| flag == name);
    match sub.as_str() {
        "add" if private_index => None,
        "add" if short('A', "") || short('u', "") || long("--all") || long("--update") => {
            Some("stages every change in the checkout")
        }
        "add" if everything(rest) => Some("stages every change in the checkout"),
        "commit" if short('a', "mFCctS") || long("--all") => {
            Some("commits every change in the checkout")
        }
        "stash" if !matches!(rest.first().map(String::as_str), Some("list" | "show")) => {
            Some("takes every uncommitted change out of the checkout")
        }
        "reset" if long("--hard") || long("--merge") || long("--keep") => {
            Some("throws away uncommitted changes")
        }
        "checkout" | "restore" if everything(rest) => Some("throws away uncommitted changes"),
        "clean" if short('f', "eE") || long("--force") => Some("deletes files nobody committed"),
        _ => None,
    }
}

/// The flags before a `--`.
fn flags_before_paths(args: &[String]) -> impl Iterator<Item = &str> {
    args.iter()
        .map(String::as_str)
        .take_while(|arg| *arg != "--")
        .filter(|arg| arg.starts_with('-'))
}

/// `git`'s arguments from its subcommand on, past options such as `-C dir`.
fn git_args(args: &[String]) -> &[String] {
    let mut at = 0;
    while let Some(arg) = args.get(at) {
        match arg.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" => at += 2,
            arg if arg.starts_with('-') => at += 1,
            _ => break,
        }
    }
    args.get(at..).unwrap_or_default()
}

/// A command's words without what runs it, such as `env X=1` or `sudo`.
fn command_words(words: &[String]) -> Option<&[String]> {
    let runner = |word: &str| {
        matches!(
            word,
            "env" | "command" | "exec" | "sudo" | "time" | "nice" | "nohup" | "builtin"
        ) || word.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    };
    let at = words.iter().position(|word| !runner(word))?;
    Some(&words[at..])
}

fn base_name(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

fn is_shell(word: &str) -> bool {
    matches!(base_name(word), "sh" | "bash" | "zsh" | "dash")
}

/// The words of each simple command in `line`, split at `;`, `&`, `|`,
/// brackets, backticks and line breaks outside quotes.
fn segments(line: &str) -> Vec<Vec<String>> {
    let mut segments = vec![Vec::new()];
    let mut word: Option<String> = None;
    let mut chars = line.chars();
    let end_word = |word: &mut Option<String>, segments: &mut Vec<Vec<String>>| {
        if let Some(done) = word.take() {
            if let Some(last) = segments.last_mut() {
                last.push(done);
            }
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let quoted: String = chars.by_ref().take_while(|c| *c != '\'').collect();
                word.get_or_insert_with(String::new).push_str(&quoted);
            }
            '"' => {
                let text = word.get_or_insert_with(String::new);
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => text.extend(chars.next()),
                        c => text.push(c),
                    }
                }
            }
            '\\' => word.get_or_insert_with(String::new).extend(chars.next()),
            ';' | '&' | '|' | '(' | ')' | '`' | '\n' | '{' | '}' => {
                end_word(&mut word, &mut segments);
                segments.push(Vec::new());
            }
            c if c.is_whitespace() => end_word(&mut word, &mut segments),
            c => word.get_or_insert_with(String::new).push(c),
        }
    }
    end_word(&mut word, &mut segments);
    segments.retain(|words| !words.is_empty());
    segments
}
