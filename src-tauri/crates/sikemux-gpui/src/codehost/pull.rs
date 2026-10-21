//! One pull request opened from the list: its files, commits, checks and merge in the
//! side column, and its conversation or every changed file's diff beside them.

use std::path::Path;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, list, px, AnyElement, App, Context, Div, Entity, FontWeight, InteractiveElement,
    IntoElement, ListAlignment, ListState, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Window,
};
use serde::Deserialize;
use serde_json::json;
use sikemux_core::git::diff::DiffRow;
use sikemux_wire::codehost::{
    ChangedFile, MergeReadiness, OpenPull, Outcome, PullCommit, ReviewVerdict,
};

use super::thread::{self, Block, Post, Pushers};
use super::{ago, branch_chip, outcome_word, pull_mark, tally_summary, HostArea, Read};
use crate::git::{row, Colours, Diff, RowLook};
use crate::icons::Icon;
use crate::theme::{self, Tokens, RADIUS_2};
use crate::ui::{self, Button, IconButton, Segmented, Variant};

pub const VIEW: &str = "host-pull";
pub const FILES: &str = "host-pull-files";

/// The side column's pages.
#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Files,
    Commits,
    Merge,
}

/// What the main column shows.
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Conversation,
    Files,
}

const METHODS: [(&str, &str); 3] = [
    ("squash", "Squash"),
    ("merge", "Merge commit"),
    ("rebase", "Rebase"),
];

/// A changed file as the core answers it: the file, and its patch as a diff's rows.
#[derive(Deserialize)]
struct Pulled {
    #[serde(flatten)]
    file: ChangedFile,
    rows: Vec<DiffRow>,
}

pub struct Changed {
    pub file: ChangedFile,
    pub diff: Diff,
    pub colours: Colours,
}

#[derive(Clone, Copy, PartialEq)]
enum Item {
    Header(usize),
    Row(usize, usize),
    End(usize),
}

pub struct Opened {
    pub number: u64,
    pub view: Read<OpenPull>,
    pub files: Read<Vec<Changed>>,
    pub commits: Option<Vec<PullCommit>>,
    pub thread: Read<Vec<Block>>,
    /// The description, kept as markdown while it reads the same.
    opening: Option<(String, Post)>,
    pub page: Page,
    pub tab: Tab,
    pub method: usize,
    pub busy: bool,
    rows: ListState,
    items: Vec<Item>,
}

impl Opened {
    pub fn new(number: u64) -> Self {
        Self {
            number,
            view: None,
            files: None,
            commits: None,
            thread: None,
            opening: None,
            page: Page::Files,
            tab: Tab::Conversation,
            method: 0,
            busy: false,
            rows: ListState::new(0, ListAlignment::Top, px(600.)),
            items: Vec::new(),
        }
    }

    pub fn take_view(&mut self, view: Read<OpenPull>) {
        if let Some(Ok(view)) = &view {
            let pull = &view.pull;
            if self.opening.as_ref().map(|(body, _)| body) != Some(&pull.body) {
                let post = Post::new(
                    pull.author.clone(),
                    pull.author_association.clone(),
                    Some(pull.created_at.clone()),
                    None,
                    &pull.body,
                );
                self.opening = Some((pull.body.clone(), post));
            }
        }
        self.view = view;
    }

    pub fn take_files(&mut self, files: Result<Vec<serde_json::Value>, SharedString>) {
        self.files = Some(files.map(|files| {
            files
                .into_iter()
                .filter_map(|file| serde_json::from_value::<Pulled>(file).ok())
                .map(|pulled| Changed {
                    file: pulled.file,
                    diff: Diff::new(pulled.rows),
                    colours: Colours::default(),
                })
                .collect()
        }));
        self.lay_out();
    }

    fn lay_out(&mut self) {
        let mut items = Vec::new();
        if let Some(Ok(files)) = &self.files {
            for (at, changed) in files.iter().enumerate() {
                items.push(Item::Header(at));
                items.extend((0..changed.diff.len()).map(|index| Item::Row(at, index)));
                items.push(Item::End(at));
            }
        }
        crate::git::relist(&self.rows, &self.items, &items);
        self.items = items;
    }

    /// Scrolls the diffs to `path`'s and shows them.
    fn focus(&mut self, path: &str) {
        self.tab = Tab::Files;
        let Some(Ok(files)) = &self.files else {
            return;
        };
        if let Some(at) = files.iter().position(|changed| changed.file.path == path) {
            if let Some(item) = self.items.iter().position(|item| *item == Item::Header(at)) {
                self.rows.scroll_to_reveal_item(item);
            }
        }
    }
}

impl HostArea {
    /// Opens pull request `number`, asking the core for it and the host for its commits
    /// and conversation.
    pub(super) fn open_pull(&mut self, number: u64, cx: &mut Context<Self>) {
        self.pull = Some(Opened::new(number));
        self.watch();
        self.read_pull_extras(number, cx);
        cx.notify();
    }

    fn read_pull_extras(&self, number: u64, cx: &mut Context<Self>) {
        let commits = self.plugin("pullCommits", json!({ "number": number }));
        let timeline = self.plugin("timeline", json!({ "number": number, "of": "pull" }));
        cx.spawn(async move |area, cx| {
            let (commits, timeline) = (commits.await, timeline.await);
            area.update(cx, |area, cx| {
                let Some(opened) = area.pull.as_mut().filter(|opened| opened.number == number)
                else {
                    return;
                };
                opened.commits = commits
                    .ok()
                    .and_then(|commits| serde_json::from_value(commits).ok());
                opened.thread = Some(timeline.and_then(|items| {
                    serde_json::from_value(items)
                        .map(thread::blocks)
                        .map_err(|error| error.to_string().into())
                }));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn merge(&mut self, cx: &mut Context<Self>) {
        let Some((number, sha, method)) = self.pull.as_ref().and_then(|opened| {
            let view = opened.view.as_ref()?.as_ref().ok()?;
            let sha = view.pull.head_sha.clone().unwrap_or_default();
            Some((opened.number, sha, METHODS[opened.method].0))
        }) else {
            return;
        };
        let params = json!({ "number": number, "method": method, "headSha": sha });
        self.write("mergePull", params, format!("Merged #{number}"), cx);
    }

    fn set_state(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(number) = self.pull.as_ref().map(|opened| opened.number) else {
            return;
        };
        let state = if open { "open" } else { "closed" };
        let said = if open {
            format!("Reopened #{number}")
        } else {
            format!("Closed #{number}")
        };
        self.write(
            "setPullState",
            json!({ "number": number, "state": state }),
            said,
            cx,
        );
    }

    /// Asks the host to change the pull request, saying `done` once it has.
    fn write(
        &mut self,
        method: &'static str,
        more: serde_json::Value,
        done: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(opened) = &mut self.pull {
            opened.busy = true;
        }
        let answer = self.plugin(method, more);
        cx.spawn(async move |area, cx| {
            let answer = answer.await;
            area.update(cx, |area, cx| {
                if let Some(opened) = &mut area.pull {
                    opened.busy = false;
                }
                match answer {
                    Ok(_) => area.say(super::Said::Done(done.into()), cx),
                    Err(error) => area.say(super::Said::Failed(error), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

pub fn view(area: &mut HostArea, window: &mut Window, cx: &mut Context<HostArea>) -> AnyElement {
    let Some(opened) = &area.pull else {
        return div().into_any_element();
    };
    let t = Tokens::get(cx).clone();
    let left = side(area, cx);
    let language = |path: &str, cx: &App| {
        crate::file_icons::language(path, cx).filter(|language| crate::syntax::knows(language))
    };
    if opened.tab == Tab::Files {
        let paths: Vec<(usize, Option<String>)> = match &opened.files {
            Some(Ok(files)) => files
                .iter()
                .enumerate()
                .map(|(at, changed)| (at, language(&changed.file.path, cx)))
                .collect(),
            _ => Vec::new(),
        };
        for (at, language) in paths {
            let Some(Some(Ok(files))) = area.pull.as_mut().map(|opened| opened.files.as_mut())
            else {
                break;
            };
            let changed = &mut files[at];
            changed.colours.ask(
                &changed.diff,
                language.as_deref(),
                cx,
                move |area: &mut HostArea| {
                    let files = area.pull.as_mut()?.files.as_mut()?.as_mut().ok()?;
                    Some(&mut files.get_mut(at)?.colours)
                },
            );
        }
    }
    let right = main(area, window, cx);
    div()
        .size_full()
        .flex()
        .child(
            div()
                .w(px(320.))
                .flex_none()
                .h_full()
                .border_r_1()
                .border_color(t.border)
                .child(left),
        )
        .child(div().flex_1().min_w_0().h_full().child(right))
        .into_any_element()
}

fn side(area: &HostArea, cx: &mut Context<HostArea>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let opened = area.pull.as_ref().expect("a pull request is open");
    let host = super::host_label(&area.host.host);
    let view = opened.view.as_ref().and_then(|view| view.as_ref().ok());
    let state = view.map(|view| view.state);
    let merge_label = match view.map(|view| view.pull.state.as_str()) {
        Some("merged") => "Merged",
        Some("closed") => "Closed",
        _ => "Merge",
    };
    let url = view.map(|view| view.pull.url.clone());
    let head = div()
        .px(px(10.))
        .pt(px(10.))
        .pb(px(8.))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            IconButton::new("pull-back", Icon::Chevron, "Back to the pull requests")
                .turned(std::f32::consts::PI)
                .small()
                .on_click(cx.listener(|area, _, _, cx| {
                    area.pull = None;
                    area.watch();
                    cx.notify();
                })),
        )
        .children(state.map(|state| pull_mark(state, 14., cx)))
        .child(ui::mono(format!("#{}", opened.number), 12., cx).text_color(t.text_primary));
    let actions = div()
        .px(px(10.))
        .pb(px(10.))
        .flex()
        .gap(px(6.))
        .child(
            Button::new("pull-on-host", format!("Open on {host}"), Variant::Outline)
                .icon(Icon::Globe)
                .disabled(url.is_none())
                .on_click(move |_, _, cx| {
                    if let Some(url) = &url {
                        cx.open_url(url);
                    }
                }),
        )
        .child(
            Button::new("pull-merge-page", merge_label, Variant::Outline)
                .icon(Icon::Merge)
                .on_click(cx.listener(|area, _, _, cx| {
                    if let Some(opened) = &mut area.pull {
                        opened.page = Page::Merge;
                    }
                    cx.notify();
                })),
        );
    let pages = [
        (Icon::File, "Files", Page::Files),
        (Icon::Commit, "Commits", Page::Commits),
        (Icon::Check, "Checks", Page::Merge),
    ];
    let chosen = pages
        .iter()
        .position(|(_, _, page)| *page == opened.page)
        .unwrap_or_default();
    let switch = div().px(px(10.)).pb(px(8.)).child(
        Segmented::new(
            "pull-pages",
            pages
                .iter()
                .map(|(glyph, label, _)| (*glyph, SharedString::from(*label)))
                .collect(),
            chosen,
        )
        .on_select(cx.listener(move |area, index: &usize, _, cx| {
            if let Some(opened) = &mut area.pull {
                opened.page = pages[*index].2;
            }
            cx.notify();
        })),
    );
    let body = match opened.page {
        Page::Files => files_page(opened, cx),
        Page::Commits => commits_page(opened, cx),
        Page::Merge => merge_page(area, cx),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(head)
        .child(actions)
        .child(switch)
        .child(
            div()
                .id("pull-side-page")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(body),
        )
        .into_any_element()
}

fn files_page(opened: &Opened, cx: &mut Context<HostArea>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let files = match &opened.files {
        None => return super::notice("Loading files", None, cx),
        Some(Err(error)) => {
            return super::notice("Could not read the files", Some(error.clone()), cx)
        }
        Some(Ok(files)) => files,
    };
    let added: u64 = files.iter().map(|changed| changed.file.additions).sum();
    let removed: u64 = files.iter().map(|changed| changed.file.deletions).sum();
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(12.))
                .py(px(4.))
                .flex()
                .gap(px(8.))
                .text_size(px(11.))
                .font_features(theme::tabular())
                .child(
                    div()
                        .text_color(t.text_secondary)
                        .child(format!("{} files", files.len())),
                )
                .child(div().flex_1())
                .child(div().text_color(t.live).child(format!("+{added}")))
                .child(div().text_color(t.danger).child(format!("−{removed}"))),
        )
        .children(files.iter().enumerate().map(|(at, changed)| {
            let path = changed.file.path.clone();
            let (dir, name) = match path.rsplit_once('/') {
                Some((dir, name)) => (Some(dir.to_owned()), name.to_owned()),
                None => (None, path.clone()),
            };
            let (letter, ink) = match changed.file.status.as_str() {
                "added" => ("A", t.git_added),
                "removed" => ("D", t.git_deleted),
                "renamed" | "copied" => ("R", t.git_renamed),
                _ => ("M", t.git_modified),
            };
            let glyph = match crate::file_icons::icon(&name, cx) {
                Some(icon) => gpui_kit::img(icon)
                    .size(px(14.))
                    .flex_none()
                    .into_any_element(),
                None => ui::icon(Icon::File, 14.)
                    .text_color(t.text_tertiary)
                    .into_any_element(),
            };
            div()
                .id(("pull-file", at))
                .h(px(26.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.))
                .cursor_pointer()
                .hover(move |row| row.bg(t.surface_hover))
                .tooltip(ui::tooltip(path.clone()))
                .on_click(cx.listener(move |area, _, _, cx| {
                    if let Some(opened) = &mut area.pull {
                        opened.focus(&path);
                    }
                    cx.notify();
                }))
                .child(glyph)
                .child(div().flex_none().text_color(t.text_primary).child(name))
                .children(dir.map(|dir| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(10.5))
                        .text_color(t.text_tertiary)
                        .child(dir)
                }))
                .child(
                    div()
                        .ml_auto()
                        .flex_none()
                        .font_family(theme::MONO)
                        .text_size(px(11.))
                        .text_color(ink)
                        .child(letter),
                )
        }))
        .into_any_element()
}

fn commits_page(opened: &Opened, cx: &App) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let Some(commits) = &opened.commits else {
        return super::notice("Loading commits", None, cx);
    };
    div()
        .flex()
        .flex_col()
        .children(commits.iter().rev().map(|commit| {
            let author = commit.author.clone().unwrap_or_else(|| "someone".into());
            let subject = commit.message.lines().next().unwrap_or_default().to_owned();
            let short: String = commit.sha.chars().take(7).collect();
            div()
                .px(px(12.))
                .py(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.))
                .child(ui::initials(&author, "", 18., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().text_color(t.text_primary).child(subject))
                        .child(div().text_size(px(10.5)).text_color(t.text_tertiary).child(
                            format!(
                                "{author} · {}",
                                commit.date.as_deref().map(ago).unwrap_or_default()
                            ),
                        )),
                )
                .child(ui::mono(short, 11., cx).text_color(t.text_tertiary))
        }))
        .into_any_element()
}

/// One line of the merge box: a mark, what it says, and why.
fn part(outcome: Outcome, title: impl Into<SharedString>, detail: Option<String>, cx: &App) -> Div {
    let t = Tokens::get(cx);
    div()
        .px(px(12.))
        .py(px(9.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .border_b_1()
        .border_color(t.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::status_of_run(outcome_word(outcome), 14., cx))
                .child(ui::text(title.into(), 12., FontWeight::MEDIUM).text_color(t.text_primary)),
        )
        .children(detail.map(|detail| ui::meta(detail, cx).pl(px(22.))))
}

fn merge_page(area: &HostArea, cx: &mut Context<HostArea>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let opened = area.pull.as_ref().expect("a pull request is open");
    let view = match &opened.view {
        None => return super::notice("Loading pull request", None, cx),
        Some(Err(error)) => return super::notice("Could not read it", Some(error.clone()), cx),
        Some(Ok(view)) => view,
    };
    let pull = &view.pull;
    let host = super::host_label(&area.host.host);
    let base = pull
        .base
        .clone()
        .unwrap_or_else(|| "the base branch".into());
    let checks = checks(view, cx);
    let card = div()
        .m(px(10.))
        .rounded(RADIUS_2)
        .border_1()
        .border_color(t.border)
        .overflow_hidden();
    if pull.state == "merged" {
        let by = pull.merged_by.clone().unwrap_or_default();
        let when = pull.merged_at.as_deref().map(ago).unwrap_or_default();
        let sha = pull.merge_commit_sha.clone();
        return card
            .child(
                div()
                    .px(px(12.))
                    .py(px(9.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(pull_mark(view.state, 14., cx))
                            .child(
                                ui::text("Merged into", 12., FontWeight::MEDIUM)
                                    .text_color(t.text_primary),
                            )
                            .child(branch_chip(&base, t.text_primary, cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(11.))
                            .text_color(t.text_tertiary)
                            .when(!by.is_empty(), |row| {
                                row.child(ui::initials(&by, "", 16., cx)).child(by.clone())
                            })
                            .child(when)
                            .children(sha.map(|sha| {
                                let short: String = sha.chars().take(7).collect();
                                div()
                                    .id("merge-sha")
                                    .font_family(theme::MONO)
                                    .cursor_pointer()
                                    .hover(move |style| style.text_color(t.text_primary))
                                    .tooltip(ui::tooltip("Copy the merge commit"))
                                    .on_click(cx.listener(move |area, _, _, cx| {
                                        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                            sha.clone(),
                                        ));
                                        let short: String = sha.chars().take(7).collect();
                                        area.say(
                                            super::Said::Done(format!("Copied {short}").into()),
                                            cx,
                                        );
                                    }))
                                    .child(short)
                            })),
                    ),
            )
            .children(checks)
            .into_any_element();
    }
    if pull.state != "open" {
        return card
            .child(
                div()
                    .px(px(12.))
                    .py(px(9.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(t.border)
                    .child(pull_mark(view.state, 14., cx))
                    .child(
                        ui::text("Closed without merging", 12., FontWeight::MEDIUM)
                            .text_color(t.text_primary),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("pull-reopen", "Reopen", Variant::Outline)
                            .disabled(opened.busy)
                            .on_click(cx.listener(|area, _, _, cx| area.set_state(true, cx))),
                    ),
            )
            .children(checks)
            .into_any_element();
    }
    let verdict = match view.verdict {
        Some(ReviewVerdict::Approved) => part(Outcome::Success, "Approved", None, cx),
        Some(ReviewVerdict::ChangesRequested) => {
            part(Outcome::Failure, "Changes requested", None, cx)
        }
        None if view.reviewed => part(Outcome::Queued, "No approving review yet", None, cx),
        None => part(Outcome::Queued, "No reviews yet", None, cx),
    };
    let readiness = if pull.draft {
        part(
            Outcome::Queued,
            "This is a draft",
            Some(format!(
                "Mark it ready for review on {host} before merging."
            )),
            cx,
        )
    } else {
        let (outcome, title, detail) = match view.merge {
            MergeReadiness::Conflicts => (
                Outcome::Failure,
                format!("Conflicts with {base}"),
                Some("Resolve the conflicts before merging.".to_owned()),
            ),
            MergeReadiness::Blocked => (
                Outcome::Blocked,
                "Merging is blocked".to_owned(),
                Some(format!(
                    "{host} is waiting on the required reviews and checks."
                )),
            ),
            MergeReadiness::Behind => (
                Outcome::Blocked,
                format!("Behind {base}"),
                Some("Bring the branch up to date before merging.".to_owned()),
            ),
            MergeReadiness::Ready => (Outcome::Success, "Ready to merge".to_owned(), None),
            MergeReadiness::Checking => (
                Outcome::Queued,
                "Checking whether it can merge".to_owned(),
                None,
            ),
        };
        part(outcome, title, detail, cx)
    };
    let dirty = pull.merge_state.as_deref() == Some("dirty");
    let number = pull.number;
    let title = pull.title.clone();
    let base_name = base.clone();
    let actions = div()
        .px(px(12.))
        .py(px(10.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .when(!pull.draft, |actions| {
            actions
                .child(
                    Segmented::words(
                        "merge-method",
                        METHODS
                            .iter()
                            .map(|(_, label)| (SharedString::from(*label), None))
                            .collect(),
                        opened.method,
                    )
                    .on_select(cx.listener(|area, index: &usize, _, cx| {
                        if let Some(opened) = &mut area.pull {
                            opened.method = *index;
                        }
                        cx.notify();
                    })),
                )
                .child(
                    Button::new("pull-merge", "Merge pull request", Variant::Primary)
                        .icon(Icon::Merge)
                        .disabled(opened.busy || dirty)
                        .on_click(cx.listener(move |area, _, _, cx| {
                            area.ask(
                                super::Asking::new(
                                    format!("Merge #{number} into {base_name}?"),
                                    title.clone(),
                                    "Merge",
                                    false,
                                    |area, cx| area.merge(cx),
                                ),
                                cx,
                            );
                        })),
                )
        })
        .child({
            let title = pull.title.clone();
            Button::new("pull-close", "Close", Variant::Outline)
                .disabled(opened.busy)
                .on_click(cx.listener(move |area, _, _, cx| {
                    area.ask(
                        super::Asking::new(
                            format!("Close #{number} without merging?"),
                            title.clone(),
                            "Close pull request",
                            true,
                            |area, cx| area.set_state(false, cx),
                        ),
                        cx,
                    );
                }))
        });
    card.child(verdict)
        .children(checks)
        .child(readiness)
        .child(actions)
        .into_any_element()
}

/// The runs on the pull request's head commit, one row each under how they stand.
fn checks(view: &OpenPull, cx: &App) -> Option<Div> {
    let t = Tokens::get(cx).clone();
    let tally = view.checks.as_ref().filter(|_| !view.runs.is_empty())?;
    let summary = tally_summary(tally);
    let title = if summary == "all passed" {
        "All checks passed".to_owned()
    } else {
        format!("Checks: {summary}")
    };
    Some(
        part(tally.outcome, title, None, cx).child(div().pt(px(4.)).flex().flex_col().children(
            view.runs.iter().map(|listed| {
                let run = &listed.run;
                let took = super::elapsed(
                    run.started_at.as_deref().unwrap_or(&run.created_at),
                    (!listed.unfinished).then_some(run.updated_at.as_str()),
                );
                div()
                    .h(px(24.))
                    .pl(px(22.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(11.5))
                    .child(ui::status_of_run(outcome_word(run.outcome), 13., cx))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(t.text_primary)
                            .child(format!("{} #{}", run.name, run.run_number)),
                    )
                    .child(
                        div()
                            .text_color(t.text_tertiary)
                            .child(super::event_label(&run.event)),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_color(t.text_tertiary)
                            .child(super::outcome_label(run.outcome)),
                    )
                    .child(
                        div()
                            .w(px(48.))
                            .text_right()
                            .font_features(theme::tabular())
                            .text_color(t.text_tertiary)
                            .child(took),
                    )
            }),
        )),
    )
}

fn main(area: &mut HostArea, window: &mut Window, cx: &mut Context<HostArea>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let host = super::host_label(&area.host.host);
    let view = cx.entity();
    let opened = area.pull.as_ref().expect("a pull request is open");
    let found = match &opened.view {
        None => return super::notice("Loading pull request", None, cx),
        Some(Err(error)) => return super::notice("Could not read it", Some(error.clone()), cx),
        Some(Ok(found)) => found,
    };
    let pull = &found.pull;
    let author = pull.author.clone().unwrap_or_else(|| "someone".into());
    let url = pull.url.clone();
    let said = opened
        .thread
        .as_ref()
        .and_then(|thread| thread.as_ref().ok())
        .map(|blocks| thread::said(blocks) + 1);
    let tabs = [
        (
            Tab::Conversation,
            "Conversation",
            said.map(|said| said as u64),
        ),
        (Tab::Files, "Files changed", pull.changed_files),
    ];
    let headline = div()
        .px(px(16.))
        .pt(px(14.))
        .pb(px(10.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .border_b_1()
        .border_color(t.border)
        .child(
            div()
                .flex()
                .gap(px(12.))
                .child(ui::initials(&author, "", 36., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(pull_mark(found.state, 16., cx))
                                .child(
                                    ui::text(pull.title.clone(), 15., FontWeight::SEMIBOLD)
                                        .text_color(t.text_primary)
                                        .min_w_0()
                                        .truncate(),
                                )
                                .child(
                                    IconButton::new(
                                        "pull-open-host",
                                        Icon::Globe,
                                        format!("Open on {host}"),
                                    )
                                    .small()
                                    .on_click(move |_, _, cx| cx.open_url(&url)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(10.5))
                                .text_color(t.text_tertiary)
                                .child(format!(
                                    "{author} · #{}, updated {}",
                                    pull.number,
                                    ago(&pull.updated_at)
                                ))
                                .children(
                                    pull.head
                                        .as_ref()
                                        .map(|head| branch_chip(head, t.text_secondary, cx)),
                                )
                                .children(pull.base.as_ref().map(|base| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child("→")
                                        .child(branch_chip(base, t.text_secondary, cx))
                                }))
                                .children(
                                    pull.labels.iter().map(|label| super::label_chip(label, cx)),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .gap(px(6.))
                .children(tabs.into_iter().map(|(tab, label, count)| {
                    let on = opened.tab == tab;
                    div()
                        .id(SharedString::from(format!("pull-tab-{label}")))
                        .h(px(24.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .rounded_full()
                        .border_1()
                        .border_color(if on { t.border_strong } else { t.border })
                        .when(on, |chip| chip.bg(t.surface_active))
                        .text_size(px(11.5))
                        .text_color(if on { t.text_primary } else { t.text_secondary })
                        .cursor_pointer()
                        .on_click(cx.listener(move |area, _, _, cx| {
                            if let Some(opened) = &mut area.pull {
                                opened.tab = tab;
                            }
                            cx.notify();
                        }))
                        .child(label)
                        .children(count.map(|count| {
                            div()
                                .font_features(theme::tabular())
                                .text_color(t.text_tertiary)
                                .child(count.to_string())
                        }))
                })),
        );
    let body = match opened.tab {
        Tab::Conversation => {
            let base = pull.base.clone();
            let pushers: Pushers = opened
                .commits
                .as_deref()
                .map(thread::pushers)
                .unwrap_or_default();
            let opening = opened.opening.as_ref().map(|(_, post)| post);
            let blocks = match &opened.thread {
                Some(Ok(blocks)) => &blocks[..],
                _ => &[],
            };
            let mut column = div().id("pull-thread").size_full().overflow_y_scroll();
            column = column.child(thread::thread(
                opening,
                blocks,
                &pushers,
                base.as_deref(),
                window,
                cx,
            ));
            if let Some(Err(error)) = &opened.thread {
                column = column.child(super::notice(
                    "Could not read the conversation",
                    Some(error.clone()),
                    cx,
                ));
            }
            column.into_any_element()
        }
        Tab::Files => diffs(&view, area, window, cx),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(headline)
        .child(div().flex_1().min_h_0().child(body))
        .into_any_element()
}

fn diffs(view: &Entity<HostArea>, area: &HostArea, window: &mut Window, cx: &App) -> AnyElement {
    let opened = area.pull.as_ref().expect("a pull request is open");
    match &opened.files {
        None => return super::notice("Loading files", None, cx),
        Some(Err(error)) => {
            return super::notice("Could not read the files", Some(error.clone()), cx)
        }
        Some(Ok(files)) if files.is_empty() => return super::notice("No files changed.", None, cx),
        Some(Ok(_)) => {}
    }
    let advance = crate::git::advance(window);
    let view = view.clone();
    let project = area.project.clone();
    list(opened.rows.clone(), move |index, _, cx| {
        let cx = &*cx;
        let area = view.read(cx);
        let Some(opened) = &area.pull else {
            return div().into_any_element();
        };
        let (Some(Ok(files)), Some(item)) = (&opened.files, opened.items.get(index).copied())
        else {
            return div().into_any_element();
        };
        diff_item(files, item, &project, advance, cx)
    })
    .size_full()
    .font_family(theme::MONO)
    .into_any_element()
}

fn diff_item(
    files: &[Changed],
    item: Item,
    project: &str,
    advance: gpui_kit::Pixels,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let frame = |inner: Div| {
        div().px(px(12.)).child(
            inner
                .border_l_1()
                .border_r_1()
                .border_color(t.border)
                .bg(t.surface),
        )
    };
    match item {
        Item::Header(at) => {
            let file = &files[at].file;
            div()
                .px(px(12.))
                .pt(px(12.))
                .child(
                    div()
                        .h(px(30.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded_t(RADIUS_2)
                        .border_1()
                        .border_color(t.border)
                        .bg(t.surface_raised)
                        .font_family(theme::UI)
                        .text_size(px(12.))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_color(t.text_primary)
                                .child(file.path.clone()),
                        )
                        .children(file.previous_path.as_ref().map(|from| {
                            div()
                                .text_color(t.text_tertiary)
                                .child(format!("from {from}"))
                        }))
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_color(t.live)
                                .child(format!("+{}", file.additions)),
                        )
                        .child(
                            div()
                                .text_color(t.danger)
                                .child(format!("−{}", file.deletions)),
                        ),
                )
                .into_any_element()
        }
        Item::Row(at, index) => {
            let changed = &files[at];
            let look = RowLook::new(&t, advance, px(0.));
            let path = Path::new(project).join(&changed.file.path);
            frame(
                div()
                    .text_size(px(12.))
                    .line_height(px(19.))
                    .when(index == 0, |rows| rows.pt(px(6.)))
                    .when(index + 1 == changed.diff.len(), |rows| rows.pb(px(6.)))
                    .child(row(
                        &changed.diff,
                        &changed.colours,
                        index,
                        &look,
                        SharedString::from(format!("pull-row-{at}-{index}")),
                        &path,
                        |_, _| {},
                    )),
            )
            .into_any_element()
        }
        Item::End(at) => {
            let empty = files[at].diff.is_empty();
            div()
                .px(px(12.))
                .child(
                    div()
                        .when(empty, |end| {
                            end.px(px(10.))
                                .py(px(8.))
                                .font_family(theme::UI)
                                .text_size(px(11.))
                                .text_color(t.text_tertiary)
                                .child(files[at].file.note.clone().unwrap_or_else(|| {
                                    "The host shows no diff for this file.".into()
                                }))
                        })
                        .when(!empty, |end| end.h(px(1.)))
                        .border_b_1()
                        .border_l_1()
                        .border_r_1()
                        .border_color(t.border)
                        .rounded_b(RADIUS_2)
                        .bg(t.surface),
                )
                .into_any_element()
        }
    }
}
