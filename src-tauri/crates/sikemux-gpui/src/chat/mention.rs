//! The menu the draft opens over itself while a word starts with `@`, `+` or `/`: the
//! agents to name, the project's files and folders to hand over, or the session's
//! commands. A named agent or file goes into the draft as a token, which is what the
//! message sends: the agent for the core to describe, the file as a path.

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::input::InlineToken;
use gpui_kit::component::input::TextareaState;
use gpui_kit::{px, App, AppContext, Context, IntoElement, SharedString, Styled, Task, Window};
use sikemux_wire::protocol::ChatContext;

use super::Composing;
use crate::icons::Icon;
use crate::ui::{self, DockMenu, DockRow};
use crate::workbench::Workbench;

const SHOWN: usize = 8;
const AGENT: &str = "agent:";
const FILE: &str = "file:";
const FOLDER: &str = "folder:";

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
            } else if let Some(agent) = token.id().strip_prefix(AGENT) {
                let uri = format!("sikemux://agent/{agent}");
                if !context.iter().any(|item: &ChatContext| item.uri == uri) {
                    context.push(ChatContext {
                        uri,
                        title: token.text().to_string(),
                        text: String::new(),
                    });
                }
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
        let ranking =
            (typed.trigger == '+').then(|| self.rank_entries(agent, typed.needle.clone(), cx));
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

    /// Ranks the project's files and their folders for `needle` off the main thread, and
    /// shows the ranking in `agent`'s menu once it is in.
    fn rank_entries(&mut self, agent: &str, needle: String, cx: &mut Context<Self>) -> Task<()> {
        let project = self
            .project()
            .map(|project| project.path.to_string_lossy().into_owned());
        if self.project_files.project != project || self.project_files.files.is_empty() {
            self.project_files.read(project.as_deref(), &self.asks);
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
        }
    });
    Some(DockMenu::new(rows.collect(), menu.selected))
}
