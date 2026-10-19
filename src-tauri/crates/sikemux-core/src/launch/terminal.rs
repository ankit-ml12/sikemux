//! The command line an agent runs with in a terminal: its model, effort,
//! permission mode and the session it resumes, in each CLI's own flags.

/// How the person runs a terminal agent.
#[derive(Clone, Debug, Default)]
pub struct TerminalRun<'a> {
    pub resume_id: Option<&'a str>,
    pub permission_mode: &'a str,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
}

/// The closest effort the CLI takes to the one the person chose.
fn effort(provider: &str, chosen: &str) -> Option<&'static str> {
    let supported = crate::agent_catalog::agent(provider).map_or(&[][..], |agent| agent.efforts);
    let near = if chosen == "ultra" { "max" } else { chosen };
    supported
        .iter()
        .copied()
        .find(|effort| *effort == chosen)
        .or_else(|| supported.iter().copied().find(|effort| *effort == near))
}

/// The CLI's flags for Sikemux's Normal and YOLO modes. Every other mode, and
/// YOLO on an agent that has none, runs as Normal.
pub fn permission_args(provider: &str, mode: &str) -> Vec<String> {
    let args: &[&str] = match (provider, mode == "bypass") {
        ("codex", true) => &["--dangerously-bypass-approvals-and-sandbox"],
        ("codex", false) => &["--sandbox", "workspace-write"],
        ("claude", true) => &["--dangerously-skip-permissions"],
        ("claude", false) => &["--permission-mode", "acceptEdits"],
        ("hermes" | "muse", true) => &["--yolo"],
        ("omp", true) => &["--approval-mode", "yolo"],
        ("grok", true) => &["--permission-mode", "bypassPermissions"],
        _ => &[],
    };
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

/// The arguments `provider`'s CLI starts with in a terminal for `run`.
pub fn args(provider: &str, run: &TerminalRun<'_>) -> Vec<String> {
    let model = run.model.map(str::trim).filter(|model| !model.is_empty());
    let effort = run.effort.and_then(|chosen| effort(provider, chosen));
    let mut args = Vec::new();
    if let Some(model) = model {
        args.extend(["--model".to_owned(), model.to_owned()]);
    }
    if let Some(effort) = effort {
        let flag = match provider {
            "claude" => Some(("--effort", effort.to_owned())),
            "codex" => Some(("--config", format!("model_reasoning_effort=\"{effort}\""))),
            "hermes" => Some(("--reasoning", effort.to_owned())),
            "pi" | "omp" => Some(("--thinking", effort.to_owned())),
            "grok" | "muse" => Some(("--reasoning-effort", effort.to_owned())),
            _ => None,
        };
        if let Some((flag, value)) = flag {
            args.extend([flag.to_owned(), value]);
        }
    }
    args.extend(permission_args(provider, run.permission_mode));
    let Some(resume_id) = run.resume_id else {
        if provider == "hermes" && (model.is_some() || effort.is_some()) {
            args.push("--tui".into());
        }
        return args;
    };
    if provider == "codex" {
        let mut resumed = vec!["resume".to_owned()];
        resumed.append(&mut args);
        resumed.push(resume_id.to_owned());
        return resumed;
    }
    if provider == "hermes" && (model.is_some() || effort.is_some()) {
        args.push("--tui".into());
    }
    let resume = match provider {
        "claude" | "hermes" | "omp" | "grok" => "--resume",
        "pi" | "opencode" => "--session",
        _ => "resume",
    };
    args.extend([resume.to_owned(), resume_id.to_owned()]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<'a>(
        resume_id: Option<&'a str>,
        mode: &'a str,
        model: Option<&'a str>,
        effort: Option<&'a str>,
    ) -> TerminalRun<'a> {
        TerminalRun {
            resume_id,
            permission_mode: mode,
            model,
            effort,
        }
    }

    #[test]
    fn codex_and_claude_have_only_normal_and_yolo() {
        assert_eq!(
            permission_args("codex", "workspace-write"),
            ["--sandbox", "workspace-write"]
        );
        assert_eq!(
            permission_args("codex", "bypass"),
            ["--dangerously-bypass-approvals-and-sandbox"]
        );
        assert_eq!(
            permission_args("codex", "full-access"),
            ["--sandbox", "workspace-write"]
        );
        assert_eq!(
            permission_args("claude", "workspace-write"),
            ["--permission-mode", "acceptEdits"]
        );
        assert_eq!(
            permission_args("claude", "bypass"),
            ["--dangerously-skip-permissions"]
        );
        assert_eq!(
            permission_args("claude", "read-only"),
            ["--permission-mode", "acceptEdits"]
        );
        assert!(permission_args("pi", "bypass").is_empty());
    }

    #[test]
    fn claude_and_codex_take_model_effort_mode_and_resume() {
        assert_eq!(
            args(
                "claude",
                &run(
                    Some("session-1"),
                    "workspace-write",
                    Some(" sonnet "),
                    Some("high")
                )
            ),
            [
                "--model",
                "sonnet",
                "--effort",
                "high",
                "--permission-mode",
                "acceptEdits",
                "--resume",
                "session-1"
            ]
        );
        assert_eq!(
            args(
                "codex",
                &run(
                    Some("session-2"),
                    "workspace-write",
                    Some("gpt-5.6-codex"),
                    Some("xhigh")
                )
            ),
            [
                "resume",
                "--model",
                "gpt-5.6-codex",
                "--config",
                "model_reasoning_effort=\"xhigh\"",
                "--sandbox",
                "workspace-write",
                "session-2"
            ]
        );
    }

    #[test]
    fn each_cli_gets_only_the_flags_it_has() {
        assert_eq!(
            args(
                "hermes",
                &run(
                    None,
                    "workspace-write",
                    Some("anthropic/claude-sonnet-4.6"),
                    Some("ultra")
                )
            ),
            [
                "--model",
                "anthropic/claude-sonnet-4.6",
                "--reasoning",
                "ultra",
                "--tui"
            ]
        );
        assert_eq!(
            args(
                "pi",
                &run(
                    None,
                    "workspace-write",
                    Some("anthropic/claude-sonnet-4.6"),
                    Some("ultra")
                )
            ),
            [
                "--model",
                "anthropic/claude-sonnet-4.6",
                "--thinking",
                "max"
            ]
        );
        assert_eq!(
            args(
                "opencode",
                &run(None, "workspace-write", Some("openai/gpt-5"), Some("high"))
            ),
            ["--model", "openai/gpt-5"]
        );
        assert_eq!(
            args(
                "omp",
                &run(
                    Some("/sessions/omp.jsonl"),
                    "bypass",
                    Some("openai-codex/gpt-5.6-sol"),
                    Some("xhigh")
                )
            ),
            [
                "--model",
                "openai-codex/gpt-5.6-sol",
                "--thinking",
                "xhigh",
                "--approval-mode",
                "yolo",
                "--resume",
                "/sessions/omp.jsonl"
            ]
        );
        assert_eq!(
            args(
                "grok",
                &run(
                    Some("018f0000-0000-7000-8000-000000000000"),
                    "bypass",
                    Some("grok-4.5"),
                    Some("max")
                )
            ),
            [
                "--model",
                "grok-4.5",
                "--reasoning-effort",
                "max",
                "--permission-mode",
                "bypassPermissions",
                "--resume",
                "018f0000-0000-7000-8000-000000000000"
            ]
        );
        assert_eq!(effort("codex", "ultra"), Some("max"));
        assert_eq!(effort("opencode", "high"), None);
    }
}
