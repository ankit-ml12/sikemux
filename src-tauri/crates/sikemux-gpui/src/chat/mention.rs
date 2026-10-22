//! The menu the draft opens over itself while a word starts with `@`, `+` or `/`: the
//! agents to name, the project's files and folders to hand over, or the session's
//! commands. A named agent, earlier chat or file goes into the draft as a token, which
//! is what the message sends: the agent or chat for the core to describe (an earlier
//! chat the core opens again first), the file as a path.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::input::InlineToken;
use gpui_kit::component::input::TextareaState;
use gpui_kit::{px, App, AppContext, Context, IntoElement, SharedString, Styled, Task, Window};
use serde::Deserialize;
use serde_json::{json, Value};
use sikemux_wire::protocol::ChatContext;

use super::Composing;
use crate::icons::Icon;
use crate::ui::{self, DockMenu, DockRow};
use crate::workbench::Workbench;

const SHOWN: usize = 8;
const AGENT: &str = "agent:";
const FILE: &str = "file:";
const FOLDER: &str = "folder:";
const EARLIER: &str = "earlier:";
/// Earlier chats the core is asked for, and how long it may read transcripts for them.
const EARLIER_SHOWN: usize = 4;
const EARLIER_BUDGET_MS: u64 = 600;
/// How long the person's typing must settle before the core searches earlier chats.
const SETTLE: Duration = Duration::from_millis(150);

/// A word the caret is at the end of that starts with one of the menu's triggers.
#[derive(Clone, PartialEq)]
struct Typed {
    trigger: char,
    /// Where the word starts, in bytes.
    start: usize,
    needle: String,
}

fn typed(text: &str, caret: usize) -> Option<Typed> {
    let before = text.get(..caret)?;
    let start = before.rfind(char::is_whitespace).map_or(0, |at| at + 1);
    let mut word = before[start..].chars();
    let trigger = word
        .next()
        .filter(|first| matches!(first, '@' | '+' | '/'))?;
    Some(Typed {
        trigger,
        start,
        needle: word.as_str().to_owned(),
    })
}

/// What the menu offers.
#[derive(Clone)]
enum Choice {
    Mate {
        id: String,
        name: String,
        title: String,
        project: Option<String>,
        provider: String,
    },
    Entry {
        path: String,
        folder: bool,
    },
    Command {
        name: String,
        description: String,
        hint: Option<String>,
    },
    /// A saved chat no agent holds, which the core opens again for the message: one
    /// the person named and closed, or one found by what was said in it.
    Earlier {
        cli: String,
        session: String,
        name: Option<String>,
        title: String,
    },
}

/// A closed agent the person named, as the core's `closedAgents` lists it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Closed {
    name: String,
    provider: String,
    session_id: String,
    title: Option<String>,
}

/// A saved chat the core's `agentSessions` search found.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Found {
    cli: String,
    session_id: String,
    title: String,
}

/// The menu on show: the word it answers, what it offers for it, and the one picked.
pub struct Menu {
    typed: Typed,
    choices: Rc<[Choice]>,
    selected: usize,
    _ranking: Option<Task<()>>,
}

impl Menu {
    /// Whether the caret is still at the end of the word the menu was made for.
    pub fn answers(&self, draft: &TextareaState) -> bool {
        typed(&draft.value(), draft.cursor()).as_ref() == Some(&self.typed)
    }
}

impl Composing {
    /// What the message sends besides its text: its attachments and the files it names,
    /// and the agents it names, for the core to describe.
    pub fn outgoing(&self, cx: &App) -> (Vec<String>, Vec<ChatContext>) {
        let mut paths: Vec<String> = self
            .attachments
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let mut context = Vec::new();
        for span in self.draft.read(cx).tokens() {
            let token = span.token();
            if let Some(path) = token
                .id()
                .strip_prefix(FILE)
                .or_else(|| token.id().strip_prefix(FOLDER))
            {
                if !paths.iter().any(|held| held == path) {
                    paths.push(path.to_owned());
                }
            } else if let Some(uri) = token
                .id()
                .strip_prefix(AGENT)
                .map(|agent| format!("sikemux://agent/{agent}"))
                .or_else(|| {
                    let chat = token.id().strip_prefix(EARLIER)?;
                    Some(format!("sikemux://earlier/{chat}"))
                })
            {
                if !context.iter().any(|item: &ChatContext| item.uri == uri) {
                    context.push(ChatContext {
                        uri,
                        title: token.text().to_string(),
                        text: String::new(),
                    });
                }
            }
        }
        for item in &self.context {
            if !context
                .iter()
                .any(|held: &ChatContext| held.uri == item.uri)
            {
                context.push(item.clone());
            }
        }
        paths.truncate(sikemux_core::acp::MAX_ATTACHMENTS);
        (paths, context)
    }
}

impl Workbench {
    /// Opens, changes or closes the menu of `agent`'s draft after the person changed it.
    pub fn offer(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(composing) = self
            .chats
            .get(agent)
            .and_then(|chat| chat.composer.as_ref())
        else {
            return;
        };
        let draft = composing.draft.read(cx);
        let typed = typed(&draft.value(), draft.cursor());
        let kept = composing.menu.as_ref().map(|menu| &menu.typed);
        if typed.as_ref() == kept {
            return;
        }
        let menu = typed.map(|typed| self.menu(agent, typed, cx));
        if let Some(composing) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.composer.as_mut())
        {
            composing.menu = menu;
        }
        cx.notify();
    }

    fn menu(&mut self, agent: &str, typed: Typed, cx: &mut Context<Self>) -> Menu {
        let needle = typed.needle.to_lowercase();
        let choices: Rc<[Choice]> = match typed.trigger {
            '@' => self.mates(agent, &needle).into(),
            '/' => self
                .chats
                .get(agent)
                .and_then(|chat| chat.state.as_ref())
                .map_or(Vec::new(), |head| {
                    head.commands
                        .iter()
                        .filter(|command| command.name.to_lowercase().contains(&needle))
                        .take(SHOWN)
                        .map(|command| Choice::Command {
                            name: command.name.clone(),
                            description: command.description.clone(),
                            hint: command.hint.clone(),
                        })
                        .collect()
                })
                .into(),
            _ => Rc::from(Vec::new()),
        };
        let ranking = match typed.trigger {
            '+' => Some(self.rank_entries(agent, typed.needle.clone(), cx)),
            '@' => self.look_back(agent, typed.needle.clone(), cx),
            _ => None,
        };
        Menu {
            typed,
            choices,
            selected: 0,
            _ranking: ranking,
        }
    }

    /// The other named agents whose name starts with `needle`: the chat's own project's
    /// first, then every other project's.
    fn mates(&self, agent: &str, needle: &str) -> Vec<Choice> {
        let own = self
            .view
            .as_ref()
            .and_then(|view| view.workspace.agents.iter().find(|saved| saved.id == agent))
            .map(|saved| saved.project.clone());
        let mut projects: Vec<(&str, Option<String>)> = Vec::new();
        if let Some(own) = &own {
            projects.push((own, None));
        }
        projects.extend(
            self.projects()
                .iter()
                .filter(|project| Some(&project.id) != own.as_ref())
                .map(|project| (project.id.as_str(), Some(project.name.clone()))),
        );
        projects
            .into_iter()
            .flat_map(|(project, name)| {
                self.agents_of(project).into_iter().filter_map(move |row| {
                    let mate = row.name.clone().filter(|mate| {
                        row.id != agent && mate.to_lowercase().starts_with(needle)
                    })?;
                    Some(Choice::Mate {
                        id: row.id,
                        name: mate,
                        title: row.title,
                        project: name.clone(),
                        provider: row.provider,
                    })
                })
            })
            .take(SHOWN)
            .collect()
    }

    /// Asks the core for the closed agents the person named and the earlier chats that
    /// match `needle`, in the chat's own project, and offers them after the agents open now.
    fn look_back(
        &mut self,
        agent: &str,
        needle: String,
        cx: &mut Context<Self>,
    ) -> Option<Task<()>> {
        let view = self.view.as_ref()?;
        let own = view
            .workspace
            .agents
            .iter()
            .find(|saved| saved.id == agent)?;
        let cwd = self
            .projects()
            .iter()
            .find(|project| project.id == own.project)?
            .path
            .clone();
        let open: Vec<(String, String)> = view
            .workspace
            .agents
            .iter()
            .filter_map(|saved| Some((saved.provider.clone(), saved.resume_id.clone()?)))
            .collect();
        let offered: Vec<String> = self
            .read("catalog")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|kind| kind["available"] != false)
            .filter_map(|kind| Some(kind["type"].as_str()?.to_owned()))
            .collect();
        let asks = self.asks.clone();
        let ask = move |method, params| {
            let answer = crate::core::request(&asks, method, params);
            async move { answer.await.unwrap_or_default() }
        };
        let closed = ask("closedAgents", json!({ "cwd": cwd }));
        let wanted = needle.trim().to_lowercase();
        let search = (!wanted.is_empty()).then(|| {
            let params = json!({ "query": wanted, "projects": [cwd], "limit": EARLIER_SHOWN, "budgetMs": EARLIER_BUDGET_MS });
            (ask.clone(), params)
        });
        let agent = agent.to_owned();
        Some(cx.spawn(async move |this, cx| {
            let offer = |choices: Vec<Choice>, cx: &mut gpui_kit::AsyncApp| {
                this.update(cx, |bench: &mut Workbench, cx| {
                    let menu = bench
                        .chats
                        .get_mut(&agent)
                        .and_then(|chat| chat.composer.as_mut())
                        .and_then(|composing| composing.menu.as_mut());
                    if let Some(menu) = menu.filter(|_| !choices.is_empty()) {
                        menu.choices = menu.choices.iter().cloned().chain(choices).collect();
                        cx.notify();
                    }
                })
                .ok();
            };
            let closed: Vec<Closed> = serde_json::from_value(closed.await).unwrap_or_default();
            let named: Vec<Choice> = closed
                .into_iter()
                .filter(|closed| closed.name.to_lowercase().starts_with(&wanted))
                .take(SHOWN)
                .map(|closed| Choice::Earlier {
                    cli: closed.provider,
                    session: closed.session_id,
                    title: closed.title.unwrap_or_default(),
                    name: Some(closed.name),
                })
                .collect();
            let sessions: Vec<String> = named
                .iter()
                .filter_map(|choice| match choice {
                    Choice::Earlier { session, .. } => Some(session.clone()),
                    _ => None,
                })
                .collect();
            offer(named, cx);
            let Some((ask, params)) = search else {
                return;
            };
            cx.background_executor().timer(SETTLE).await;
            let found: Vec<Found> =
                serde_json::from_value(ask("agentSessions", params).await).unwrap_or_default();
            let found = found
                .into_iter()
                .filter(|found| offered.contains(&found.cli))
                .filter(|found| !open.contains(&(found.cli.clone(), found.session_id.clone())))
                .filter(|found| !sessions.contains(&found.session_id))
                .map(|found| Choice::Earlier {
                    cli: found.cli,
                    session: found.session_id,
                    name: None,
                    title: found.title,
                })
                .collect();
            offer(found, cx);
        }))
    }

    /// Ranks the project's files and their folders for `needle` off the main thread, and
    /// shows the ranking in `agent`'s menu once it is in.
    fn rank_entries(&mut self, agent: &str, needle: String, cx: &mut Context<Self>) -> Task<()> {
        let project = self
            .project()
            .map(|project| project.path.to_string_lossy().into_owned());
        if self.project_files.project != project || self.project_files.files.is_empty() {
            self.project_files
                .read(project.as_deref(), &crate::core::Target::here(&self.asks));
        }
        let files = self.project_files.files.clone();
        let ranking = cx.background_spawn(async move { ranked_entries(&needle, &files) });
        let agent = agent.to_owned();
        cx.spawn(async move |this, cx| {
            let entries = ranking.await;
            this.update(cx, |bench, cx| {
                if let Some(menu) = bench
                    .chats
                    .get_mut(&agent)
                    .and_then(|chat| chat.composer.as_mut())
                    .and_then(|composing| composing.menu.as_mut())
                {
                    menu.choices = entries.into();
                    menu.selected = 0;
                    cx.notify();
                }
            })
            .ok();
        })
    }

    /// Ranks the menu of `agent`'s draft again, once the core has listed the project's files.
    pub fn files_listed(&mut self, cx: &mut Context<Self>) {
        let menus: Vec<(String, String)> = self
            .chats
            .iter()
            .filter_map(|(agent, chat)| {
                let typed = &chat.composer.as_ref()?.menu.as_ref()?.typed;
                (typed.trigger == '+').then(|| (agent.clone(), typed.needle.clone()))
            })
            .collect();
        for (agent, needle) in menus {
            let ranking = self.rank_entries(&agent, needle, cx);
            if let Some(menu) = self
                .chats
                .get_mut(&agent)
                .and_then(|chat| chat.composer.as_mut())
                .and_then(|composing| composing.menu.as_mut())
            {
                menu._ranking = Some(ranking);
            }
        }
    }

    /// Moves the menu's pick by `by`, round its ends.
    pub fn pick_choice(&mut self, agent: &str, by: isize, cx: &mut Context<Self>) {
        if let Some(menu) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.composer.as_mut())
            .and_then(|composing| composing.menu.as_mut())
        {
            let count = menu.choices.len() as isize;
            if count > 0 {
                menu.selected = (menu.selected as isize + by).rem_euclid(count) as usize;
                cx.notify();
            }
        }
    }

    pub fn dismiss_menu(&mut self, agent: &str, cx: &mut Context<Self>) {
        if let Some(composing) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.composer.as_mut())
        {
            composing.menu = None;
            cx.notify();
        }
    }

    /// Writes choice `index` of the menu over the word it answers.
    pub fn take_choice(
        &mut self,
        agent: &str,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = self.project().map(|project| project.path.clone());
        let Some(composing) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.composer.as_mut())
        else {
            return;
        };
        let Some(menu) = composing.menu.take() else {
            return;
        };
        let Some(choice) = menu.choices.get(index.unwrap_or(menu.selected)).cloned() else {
            return;
        };
        if let Choice::Earlier {
            cli,
            session,
            name: None,
            title,
        } = &choice
        {
            let uri = format!("sikemux://earlier/{cli}/{session}");
            if !composing.context.iter().any(|held| held.uri == uri) {
                composing.context.push(ChatContext {
                    uri,
                    title: title.clone(),
                    text: String::new(),
                });
            }
        }
        composing.draft.update(cx, |draft, cx| {
            let end = draft.cursor();
            draft.set_selected_range(menu.typed.start..end, cx);
            let token = match &choice {
                Choice::Mate { id, name, .. } => {
                    Some(InlineToken::new(format!("{AGENT}{id}"), format!("@{name}")))
                }
                Choice::Entry { path, folder } => {
                    let whole = project.as_deref().map_or_else(
                        || Path::new(path).to_path_buf(),
                        |project| project.join(path),
                    );
                    let named = draft
                        .tokens()
                        .iter()
                        .any(|span| span.token().text().as_ref() == format!("+{}", leaf(path)));
                    let tag = if named {
                        format!("+{path}")
                    } else {
                        format!("+{}", leaf(path))
                    };
                    let kind = if *folder { FOLDER } else { FILE };
                    Some(InlineToken::new(
                        format!("{kind}{}", whole.to_string_lossy()),
                        tag,
                    ))
                }
                Choice::Command { name, .. } => {
                    draft.replace(format!("/{name}"), window, cx);
                    None
                }
                Choice::Earlier {
                    cli,
                    session,
                    name: Some(name),
                    ..
                } => Some(InlineToken::new(
                    format!("{EARLIER}{cli}/{session}"),
                    format!("@{name}"),
                )),
                Choice::Earlier { .. } => {
                    draft.replace("", window, cx);
                    None
                }
            };
            if let Some(token) = token {
                if let Err(error) = draft.replace_with_token(token, window, cx) {
                    eprintln!("sikemux: a mention did not go into the draft: {error}");
                }
            }
            draft.replace(" ", window, cx);
        });
        cx.notify();
    }
}

fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The best files and folders for `needle`, a file before a folder of the same name.
fn ranked_entries(needle: &str, files: &Arc<Vec<String>>) -> Vec<Choice> {
    let mut folders: Vec<&str> = files
        .iter()
        .flat_map(|file| file.match_indices('/').map(|(at, _)| &file[..at]))
        .collect();
    folders.sort_unstable();
    folders.dedup();
    let entries: Vec<String> = files
        .iter()
        .cloned()
        .chain(folders.iter().map(|folder| (*folder).to_owned()))
        .collect();
    crate::fuzzy::rank_paths(needle, &entries, SHOWN)
        .into_iter()
        .map(|place| Choice::Entry {
            path: entries[place].clone(),
            folder: place >= files.len(),
        })
        .collect()
}

/// The menu over the draft of `agent`'s chat, while it offers anything.
pub fn dock(agent: &str, menu: &Menu, cx: &mut Context<Workbench>) -> Option<DockMenu> {
    if menu.choices.is_empty() {
        return None;
    }
    let this = cx.weak_entity();
    let tint = crate::theme::Tokens::get(cx).text_tertiary;
    let mut shown_project: Option<&str> = None;
    let mut shown_kind = None;
    let rows = menu.choices.iter().enumerate().map(|(index, choice)| {
        let (this, agent) = (this.clone(), agent.to_owned());
        let choose = Rc::new(move |window: &mut Window, cx: &mut App| {
            this.update(cx, |bench, cx| {
                bench.take_choice(&agent, Some(index), window, cx)
            })
            .ok();
        });
        match choice {
            Choice::Mate {
                name,
                title,
                project,
                provider,
                ..
            } => {
                let group = (project.as_deref() != shown_project)
                    .then(|| project.clone().map(SharedString::from))
                    .flatten();
                shown_project = project.as_deref();
                DockRow::new(format!("@{name}"), choose)
                    .mark(ui::mark(provider, 13.))
                    .detail(title.clone())
                    .group(group)
            }
            Choice::Entry { path, folder } => {
                let parent = path.rsplit_once('/').map_or(".", |(parent, _)| parent);
                let mark = if *folder {
                    ui::icon(Icon::Folder, 13.)
                        .text_color(tint)
                        .into_any_element()
                } else {
                    crate::file_icons::icon(leaf(path), cx).map_or_else(
                        || {
                            ui::icon(Icon::File, 13.)
                                .text_color(tint)
                                .into_any_element()
                        },
                        |icon| gpui_kit::img(icon).size(px(13.)).into_any_element(),
                    )
                };
                DockRow::new(leaf(path).to_owned(), choose)
                    .mark(mark)
                    .detail(parent.to_owned())
            }
            Choice::Command {
                name,
                description,
                hint,
            } => DockRow::new(format!("/{name}"), choose)
                .detail(description.clone())
                .aside(hint.clone().map(SharedString::from)),
            Choice::Earlier {
                cli, name, title, ..
            } => {
                let kind = if name.is_some() {
                    "Closed"
                } else {
                    "Earlier chats"
                };
                let group = (shown_kind != Some(kind)).then_some(SharedString::from(kind));
                shown_kind = Some(kind);
                let row = match name {
                    Some(name) => DockRow::new(format!("@{name}"), choose).detail(title.clone()),
                    None => DockRow::new(title.clone(), choose),
                };
                row.mark(ui::mark(cli, 13.)).group(group)
            }
        }
    });
    Some(DockMenu::new(rows.collect(), menu.selected))
}
