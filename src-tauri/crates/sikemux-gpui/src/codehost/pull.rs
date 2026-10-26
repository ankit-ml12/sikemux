//! One pull request opened from the list or a link: its head and actions over tabs for its
//! conversation, its changed files' diffs, its commits, and its checks and merge.

use std::collections::{BTreeMap, HashSet};

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, App, AppContext, Context, Div, Entity, FontWeight, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
};
use serde::Deserialize;
use serde_json::json;
use sikemux_core::git::diff::DiffRow;
use sikemux_wire::codehost::{
    ChangedFile, MergeReadiness, OpenPull, Outcome, PullCommit, PullState, ReviewVerdict,
};

use super::diffs::{Changed, Diffs};
use super::thread::{self, Block, Post, Pushers};
use super::{ago, branch_chip, outcome_word, pull_mark, tally_summary, HostArea, Read};
use crate::git::Diff;
use crate::icons::Icon;
use crate::theme::{self, Tokens, RADIUS_2};
use crate::ui::{self, Button, IconButton, Segmented, Variant};
use gpui_kit::component::input::{Textarea, TextareaState};

pub const VIEW: &str = "host-pull";
pub const FILES: &str = "host-pull-files";

/// What the view shows, picked from its one row of tabs.
#[derive(Clone, Copy, PartialEq)]
pub enum Section {
    Conversation,
    Files,
    Commits,
    Checks,
}

/// Below this width the Files tab has no room for the file list beside the diffs.
const NARROW_BELOW: f32 = 760.;
/// The widest the head, the conversation, the commits and the checks grow, centred in a
/// wider pane so their lines stay readable.
const READABLE: f32 = 860.;
/// How wide the file list is beside the diffs.
const FILE_LIST: f32 = 260.;

fn method_label(method: &str) -> &str {
    match method {
        "squash" => "Squash",
        "merge" => "Merge commit",
        "rebase" => "Rebase",
        other => other,
    }
}

/// A changed file as the core answers it: the file, and its patch as a diff's rows.
#[derive(Deserialize)]
struct Pulled {
    #[serde(flatten)]
    file: ChangedFile,
    rows: Vec<DiffRow>,
}

pub struct Opened {
    pub number: u64,
    pub view: Read<OpenPull>,
    pub files: Read<Diffs>,
    pub commits: Option<Vec<PullCommit>>,
    pub thread: Read<Vec<Block>>,
    /// The description, kept as markdown while it reads the same.
    opening: Option<(String, Post)>,
    pub section: Section,
    /// Too narrow for the file list beside the diffs, as last drawn.
    narrow: bool,
    /// The file whose diff was last asked for.
    focused: Option<String>,
    /// The file list's folders folded shut, by their path.
    shut: HashSet<String>,
    pub method: usize,
    pub busy: bool,
    /// The comment being written, made once the conversation first shows.
    draft: Option<Entity<TextareaState>>,
    /// The review or comment on its way, while one is.
    sending: Option<&'static str>,
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
            section: Section::Conversation,
            narrow: false,
            focused: None,
            shut: HashSet::new(),
            method: 0,
            busy: false,
            draft: None,
            sending: None,
        }
    }

    /// Takes the pull request as the core read it again, saying whether its head or its
    /// conversation moved since the last reading.
    pub fn take_view(&mut self, view: Read<OpenPull>) -> bool {
        let mark = |view: &Read<OpenPull>| {
            let pull = &view.as_ref()?.as_ref().ok()?.pull;
            Some((pull.head_sha.clone(), pull.updated_at.clone()))
        };
        let moved = matches!((mark(&self.view), mark(&view)), (Some(was), Some(now)) if was != now);
        if let Some(Ok(view)) = &view {
            let pull = &view.pull;
            if self.opening.as_ref().map(|(body, _)| body) != Some(&pull.body) {
                let post = Post::new(
                    pull.author.clone(),
                    pull.author_association.clone(),
                    Some(pull.created_at.clone()),
                    None,
                    &pull.body,
                )
                .pictured(pull.avatar_url.clone());
                self.opening = Some((pull.body.clone(), post));
            }
        }
        self.view = view;
        moved
    }

    pub fn take_files(&mut self, files: Result<Vec<serde_json::Value>, SharedString>) {
        self.files = Some(files.map(|files| {
            Diffs::new(
                files
                    .into_iter()
                    .filter_map(|file| serde_json::from_value::<Pulled>(file).ok())
                    .map(|pulled| Changed::new(pulled.file, Some(Diff::new(pulled.rows))))
                    .collect(),
            )
        }));
    }

    /// Shows the diffs, scrolled to `path`'s.
    fn focus(&mut self, path: &str) {
        self.section = Section::Files;
        self.focused = Some(path.to_owned());
        if let Some(Ok(diffs)) = &mut self.files {
            diffs.reveal(path);
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

    pub(super) fn read_pull_extras(&self, number: u64, cx: &mut Context<Self>) {
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
        let methods = &self.host.pulls.merge_methods;
        let Some((number, sha, method)) = self.pull.as_ref().and_then(|opened| {
            let view = opened.view.as_ref()?.as_ref().ok()?;
            let sha = view.pull.head_sha.clone().unwrap_or_default();
            Some((opened.number, sha, methods.get(opened.method)?.clone()))
        }) else {
            return;
        };
        let params = json!({ "number": number, "method": method, "sha": sha });
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

    /// Closes the pull request, saying what was written first. A host that cannot open it
    /// again asks before, and then closes it without the comment.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(opened) = &self.pull else {
            return;
        };
        if !self.host.pulls.reopen {
            let number = opened.number;
            let title = (opened.view.as_ref())
                .and_then(|view| view.as_ref().ok())
                .map(|view| view.pull.title.clone())
                .unwrap_or_default();
            let asking = super::Asking::new(
                format!("Close #{number} without merging?"),
                format!("{title}\n\nIt cannot be opened again."),
                "Close pull request",
                true,
                |area, cx| area.set_state(false, cx),
            );
            self.ask(asking, cx);
            return;
        }
        let written =
            (opened.draft.as_ref()).is_some_and(|draft| !draft.read(cx).value().trim().is_empty());
        if written {
            self.send(None, window, cx);
        }
        self.set_state(false, cx);
    }

    /// Checks the pull request out in the project, as the core does it.
    fn checkout(&mut self, cx: &mut Context<Self>) {
        let Some(opened) = &mut self.pull else {
            return;
        };
        opened.busy = true;
        let number = opened.number;
        let params = json!({ "project": self.project, "number": number });
        let answer = self.core.request("codeHostCheckout", params);
        cx.spawn(async move |area, cx| {
            let answer = answer.await;
            area.update(cx, |area, cx| {
                if let Some(opened) = &mut area.pull {
                    opened.busy = false;
                }
                let checked = answer
                    .map_err(SharedString::from)
                    .and_then(|value| super::answer::<serde_json::Value>(&value));
                let said = match checked {
                    Ok(_) => super::Said::Done(format!("Checked out #{number}").into()),
                    Err(error) => super::Said::Failed(
                        format!("Could not check out #{number}: {error}").into(),
                    ),
                };
                area.say(said, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Sends what was written as a comment, or with a review's verdict.
    fn send(&mut self, event: Option<&'static str>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(opened) = &mut self.pull else {
            return;
        };
        let Some(draft) = opened.draft.clone() else {
            return;
        };
        let body = draft.read(cx).value().trim().to_owned();
        let number = opened.number;
        opened.sending = Some(event.unwrap_or("comment"));
        let (method, more, done) = match event {
            None => (
                "addComment",
                json!({ "number": number, "body": body, "of": "pull" }),
                "Comment added".to_owned(),
            ),
            Some(event) => {
                let done = match event {
                    "APPROVE" => format!("Approved #{number}"),
                    "REQUEST_CHANGES" => format!("Asked for changes on #{number}"),
                    _ => format!("Reviewed #{number}"),
                };
                (
                    "reviewPull",
                    json!({ "number": number, "event": event, "body": body }),
                    done,
                )
            }
        };
        let answer = self.plugin(method, more);
        cx.spawn_in(window, async move |area, cx| {
            let answer = answer.await;
            area.update_in(cx, |area, window, cx| {
                if let Some(opened) = &mut area.pull {
                    opened.sending = None;
                }
                match answer {
                    Ok(_) => {
                        draft.update(cx, |draft, cx| draft.set_value("", window, cx));
                        area.read_pull_extras(number, cx);
                        area.say(super::Said::Done(done.into()), cx);
                    }
                    Err(error) => area.say(super::Said::Failed(error), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
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
    if opened.section == Section::Files {
        if let Some(Some(Ok(diffs))) = area.pull.as_mut().map(|opened| opened.files.as_mut()) {
            diffs.colour(cx, |area| area.pull.as_mut()?.files.as_mut()?.as_mut().ok());
        }
    }
    let shown = page_view(area, window, cx);
    let entity = cx.entity().downgrade();
    div()
        .size_full()
        .on_children_prepainted(move |bounds, _, cx| {
            let Some(width) = bounds.first().map(|drawn| f32::from(drawn.size.width)) else {
                return;
            };
            entity
                .update(cx, |area, cx| {
                    if let Some(opened) = &mut area.pull {
                        if opened.narrow != (width < NARROW_BELOW) {
                            opened.narrow = !opened.narrow;
                            cx.notify();
                        }
                    }
                })
                .ok();
        })
        .child(shown)
        .into_any_element()
}

/// The pull request's head with its actions, one row of tabs, then the tab's page, in a
/// column centred in a wide pane.
fn page_view(area: &mut HostArea, window: &mut Window, cx: &mut Context<HostArea>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let opened = area.pull.as_ref().expect("a pull request is open");
    let found = match &opened.view {
        None => return super::notice("Loading pull request", None, cx),
        Some(Err(error)) => return super::notice("Could not read it", Some(error.clone()), cx),
        Some(Ok(found)) => found,
    };
    let pull = &found.pull;
    let said = opened
        .thread
        .as_ref()
        .and_then(|thread| thread.as_ref().ok())
        .map(|blocks| (thread::said(blocks) + 1) as u64);
    let count = |count: Option<u64>| count.map(|count| tally(count, cx));
    let checks_mark = found.checks.as_ref().map(|tally| {
        ui::run_mark(
            format!("pull-checks-{}", pull.number),
            outcome_word(tally.outcome),
            12.,
            cx,
        )
    });
    let sections = [
        (
            Section::Conversation,
            Icon::Message,
            "Conversation",
            count(said),
        ),
        (
            Section::Files,
            Icon::File,
            "Files",
            count(pull.changed_files),
        ),
        (
            Section::Commits,
            Icon::Commit,
            "Commits",
            count(pull.commits),
        ),
        (Section::Checks, Icon::Actions, "Checks", checks_mark),
    ];
    let chosen = opened.section;
    let top = div()
        .px(px(16.))
        .pt(px(14.))
        .pb(px(12.))
        .child(header(area, found, cx));
    let folding = (chosen == Section::Files)
        .then(|| fold_all(opened, cx))
        .flatten();
    let tabs = strip(
        sections.into_iter().map(|(section, glyph, label, mark)| {
            tab(glyph, label, mark, chosen == section, cx).on_click(cx.listener(
                move |area, _, _, cx| {
                    if let Some(opened) = &mut area.pull {
                        opened.section = section;
                    }
                    cx.notify();
                },
            ))
        }),
        folding,
    );
    let page = |body: AnyElement| {
        div()
            .id("pull-page")
            .size_full()
            .overflow_y_scroll()
            .child(centred(body))
            .into_any_element()
    };
    let body = match chosen {
        Section::Conversation => conversation(area, window, cx),
        Section::Files if opened.narrow => diffs(area, window, cx),
        Section::Files => {
            let list = div()
                .id("pull-file-list")
                .w(px(FILE_LIST))
                .flex_none()
                .h_full()
                .border_r_1()
                .border_color(t.border)
                .bg(t.surface_composer)
                .overflow_y_scroll()
                .child(files_page(opened, cx));
            div()
                .size_full()
                .flex()
                .child(list)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(diffs(area, window, cx)),
                )
                .into_any_element()
        }
        Section::Commits => page(commits_page(opened, cx)),
        Section::Checks => page(merge_page(area, cx)),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(super::band(READABLE, div().child(top).child(tabs), cx))
        .child(div().flex_1().min_h_0().child(body))
        .into_any_element()
}

fn centred(inner: impl IntoElement) -> Div {
    super::centred(READABLE, inner)
}

fn back(cx: &mut Context<HostArea>) -> IconButton {
    IconButton::new("pull-back", Icon::Chevron, "Back to the pull requests")
        .turned(std::f32::consts::PI)
        .small()
        .on_click(cx.listener(|area, _, _, cx| {
            area.pull = None;
            area.elsewhere = None;
            area.watch();
            cx.notify();
        }))
}

/// Check out and merge, merge being what an open pull request asks for.
fn actions(area: &HostArea, cx: &mut Context<HostArea>) -> Div {
    let opened = area.pull.as_ref().expect("a pull request is open");
    let view = opened.view.as_ref().and_then(|view| view.as_ref().ok());
    let open = view.is_some_and(|view| view.pull.state == "open");
    let can_merge = open && view.is_some_and(mergeable);
    let (merge_label, merge_look) = match view.map(|view| view.pull.state.as_str()) {
        Some("merged") => ("Merged", Variant::Outline),
        Some("closed") => ("Closed", Variant::Outline),
        _ if can_merge => ("Merge", Variant::Primary),
        _ => ("Merge", Variant::Outline),
    };
    let checkout = view
        .and_then(|view| view.checkout.clone())
        .filter(|_| area.elsewhere.is_none());
    let current = area.pulls.branch.as_ref().map(|(branch, _)| branch.clone());
    let here = checkout.is_some() && checkout == current;
    let checkout_label = if here {
        "Checked out"
    } else if opened.busy {
        "Checking out…"
    } else {
        "Check out"
    };
    div()
        .flex()
        .gap(px(6.))
        .flex_wrap()
        .child(
            Button::new("pull-checkout", checkout_label, Variant::Outline)
                .icon(if here { Icon::Check } else { Icon::Checkout })
                .disabled(here || !open || checkout.is_none() || opened.busy)
                .on_click(cx.listener(|area, _, _, cx| area.checkout(cx))),
        )
        .child(
            Button::new("pull-merge-page", merge_label, merge_look)
                .icon(Icon::Merge)
                .disabled(open && !can_merge)
                .on_click(cx.listener(|area, _, _, cx| {
                    if let Some(opened) = &mut area.pull {
                        opened.section = Section::Checks;
                    }
                    cx.notify();
                })),
        )
}

/// Whether the host would take a merge now: not while it conflicts, its rules block it or
/// it is still working that out.
fn mergeable(view: &OpenPull) -> bool {
    matches!(view.merge, MergeReadiness::Ready | MergeReadiness::Behind)
}

/// The pull request's head: how it stands and its number, with its actions and the way to
/// it on the host, then its title, then who wants to merge what into what.
fn header(area: &HostArea, found: &OpenPull, cx: &mut Context<HostArea>) -> Div {
    let t = Tokens::get(cx).clone();
    let host = super::host_label(&area.host.host);
    let pull = &found.pull;
    let author = pull.author.clone().unwrap_or_else(|| "someone".into());
    let url = pull.url.clone();
    let commits = match pull.commits {
        Some(1) => "1 commit".to_owned(),
        Some(count) => format!("{count} commits"),
        None => "its commits".to_owned(),
    };
    let doing = match found.state {
        PullState::Merged => format!("merged {commits} into"),
        _ => format!("wants to merge {commits} into"),
    };
    let word = |words: String| div().child(words);
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .when(area.desk.is_none(), |row| row.child(back(cx)))
                .child(state_pill(found.state, cx))
                .child(ui::mono(format!("#{}", pull.number), 12., cx).text_color(t.text_tertiary))
                .child(div().flex_1())
                .child(actions(area, cx))
                .child(
                    IconButton::new("pull-open-host", Icon::Globe, format!("Open on {host}"))
                        .small()
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                ),
        )
        .child(
            ui::text(pull.title.clone(), 16., FontWeight::SEMIBOLD)
                .whitespace_normal()
                .text_color(t.text_primary),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(5.))
                .text_size(crate::theme::ui_text(12.))
                .text_color(t.text_tertiary)
                .child(area.face(Some(&author), pull.avatar_url.as_deref(), 16., cx))
                .child(div().text_color(t.text_primary).child(author.clone()))
                .child(word(doing))
                .children(
                    pull.base
                        .as_ref()
                        .map(|base| branch_chip(base, t.text_primary, cx)),
                )
                .children(pull.head.as_ref().map(|head| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .child("from")
                        .child(branch_chip(head, t.text_primary, cx))
                }))
                .child(word(format!("· {}", ago(&pull.updated_at))))
                .children(pull.labels.iter().map(|label| super::label_chip(label, cx))),
        )
}

/// How the pull request stands, as a pill in its colour.
fn state_pill(state: PullState, cx: &App) -> Div {
    let t = Tokens::get(cx);
    let (glyph, ink, word) = match state {
        PullState::Open => (Icon::PullRequest, t.live, "Open"),
        PullState::Draft => (Icon::PullRequest, t.text_tertiary, "Draft"),
        PullState::Closed => (Icon::PullRequest, t.danger, "Closed"),
        PullState::Merged => (Icon::Merge, t.accent, "Merged"),
    };
    div()
        .flex_none()
        .h(px(22.))
        .px(px(9.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded_full()
        .bg(ink.opacity(0.16))
        .text_size(crate::theme::ui_text(11.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink)
        .child(ui::icon(glyph, 12.).text_color(ink))
        .child(word)
}

/// A row of tabs over a page, the chosen one underlined in ink, with what acts on the page
/// at its end.
fn strip(tabs: impl IntoIterator<Item = gpui_kit::Stateful<Div>>, end: Option<AnyElement>) -> Div {
    div()
        .flex_none()
        .px(px(16.))
        .flex()
        .items_center()
        .gap(px(18.))
        .children(tabs)
        .children(end.map(|end| div().ml_auto().child(end)))
}

/// Folds every changed file's diff to its header, or unfolds them all, while they show.
fn fold_all(opened: &Opened, cx: &mut Context<HostArea>) -> Option<AnyElement> {
    let Some(Ok(diffs)) = &opened.files else {
        return None;
    };
    let folded = diffs.all_folded();
    let (glyph, label) = match folded {
        true => (Icon::ExpandAll, "Expand all diffs"),
        false => (Icon::CollapseAll, "Collapse all diffs"),
    };
    Some(
        IconButton::new("pull-fold-all", glyph, label)
            .small()
            .on_click(cx.listener(move |area, _, _, cx| {
                if let Some(Some(Ok(diffs))) =
                    area.pull.as_mut().map(|opened| opened.files.as_mut())
                {
                    diffs.fold_all(!folded);
                }
                cx.notify();
            }))
            .into_any_element(),
    )
}

/// One tab of a strip: its name, then how many it holds or how it stands.
fn tab(
    glyph: Icon,
    label: &'static str,
    mark: Option<AnyElement>,
    on: bool,
    cx: &App,
) -> gpui_kit::Stateful<Div> {
    let t = Tokens::get(cx).clone();
    div()
        .id(SharedString::from(format!("pull-tab-{label}")))
        .h(px(34.))
        .flex()
        .items_center()
        .gap(px(6.))
        .border_b_2()
        .border_color(if on {
            t.text_primary
        } else {
            gpui_kit::transparent_black()
        })
        .text_size(crate::theme::ui_text(13.))
        .text_color(if on { t.text_primary } else { t.text_secondary })
        .cursor_pointer()
        .when(!on, |tab| {
            tab.hover(move |tab| tab.text_color(t.text_primary))
        })
        .child(ui::icon(glyph, 13.).text_color(if on { t.text_primary } else { t.text_tertiary }))
        .child(label)
        .children(mark)
}

/// A tab's count, quieter than its name.
fn tally(count: u64, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .text_size(crate::theme::ui_text(11.5))
        .font_features(theme::tabular())
        .text_color(t.text_tertiary)
        .child(count.to_string())
        .into_any_element()
}

fn files_page(opened: &Opened, cx: &mut Context<HostArea>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let files = match &opened.files {
        None => return super::notice("Loading files", None, cx),
        Some(Err(error)) => {
            return super::notice("Could not read the files", Some(error.clone()), cx)
        }
        Some(Ok(diffs)) => &diffs.files,
    };
    let added: u64 = files.iter().map(|changed| changed.file.additions).sum();
    let removed: u64 = files.iter().map(|changed| changed.file.deletions).sum();
    let mut rows = Vec::new();
    lay_out_folder(&folders_of(files), "", 0, opened, files, &mut rows, cx);
    div()
        .flex()
        .flex_col()
        .pb(px(8.))
        .child(
            div()
                .px(px(12.))
                .py(px(8.))
                .flex()
                .gap(px(8.))
                .text_size(crate::theme::ui_text(11.5))
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
        .children(rows)
        .into_any_element()
}

/// A folder of the changed files: the folders in it by name, and its files by name with
/// where each is in the list.
#[derive(Default)]
struct Folder<'a> {
    folders: BTreeMap<&'a str, Folder<'a>>,
    files: BTreeMap<&'a str, usize>,
}

fn folders_of(files: &[Changed]) -> Folder<'_> {
    let mut root = Folder::default();
    for (at, changed) in files.iter().enumerate() {
        let mut parts: Vec<&str> = changed.file.path.split('/').collect();
        let name = parts.pop().unwrap_or_default();
        let here = parts.into_iter().fold(&mut root, |here, part| {
            here.folders.entry(part).or_default()
        });
        here.files.insert(name, at);
    }
    root
}

/// The rows of `folder` at `depth`: its folders first, a folder holding only one more
/// folder named with it on one row, then its files.
#[allow(clippy::too_many_arguments)]
fn lay_out_folder(
    folder: &Folder,
    path: &str,
    depth: usize,
    opened: &Opened,
    files: &[Changed],
    rows: &mut Vec<AnyElement>,
    cx: &mut Context<HostArea>,
) {
    for (name, mut inner) in &folder.folders {
        let mut label = (*name).to_owned();
        while inner.files.is_empty() && inner.folders.len() == 1 {
            let Some((next, deeper)) = inner.folders.iter().next() else {
                break;
            };
            label = format!("{label}/{next}");
            inner = deeper;
        }
        let full = match path {
            "" => label.clone(),
            path => format!("{path}/{label}"),
        };
        let open = !opened.shut.contains(&full);
        rows.push(folder_row(&full, &label, depth, open, cx));
        if open {
            lay_out_folder(inner, &full, depth + 1, opened, files, rows, cx);
        }
    }
    for (name, at) in &folder.files {
        let changed = &files[*at];
        let chosen = opened.focused.as_deref() == Some(changed.file.path.as_str());
        rows.push(file_row(*at, changed, name, depth, chosen, cx));
    }
}

/// How far in each level of the file list sits.
const TREE_STEP: f32 = 14.;
const TREE_INSET: f32 = 10.;
/// The column a folder's chevron takes, which a file keeps empty so names line up.
const TREE_CHEVRON: f32 = 12.;

/// A file list row at `depth`: inset by its depth, with a guide down each level above it.
fn tree_row(id: impl Into<gpui_kit::ElementId>, depth: usize, cx: &App) -> gpui_kit::Stateful<Div> {
    let t = Tokens::get(cx);
    div()
        .id(id)
        .relative()
        .h(px(26.))
        .pl(px(TREE_INSET + depth as f32 * TREE_STEP))
        .pr(px(12.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(crate::theme::ui_text(12.5))
        .cursor_pointer()
        .children((0..depth).map(|level| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(TREE_INSET
                    + level as f32 * TREE_STEP
                    + TREE_CHEVRON / 2.))
                .w(px(1.))
                .bg(t.tree_spine)
        }))
}

fn folder_row(
    path: &str,
    label: &str,
    depth: usize,
    open: bool,
    cx: &mut Context<HostArea>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let key = SharedString::from(format!("pull-folder-{path}"));
    let toggled = path.to_owned();
    tree_row(key.clone(), depth, cx)
        .hover(move |row| row.bg(t.surface_hover))
        .tooltip(ui::tooltip(path.to_owned()))
        .on_click(cx.listener(move |area, _, _, cx| {
            if let Some(opened) = &mut area.pull {
                if !opened.shut.remove(&toggled) {
                    opened.shut.insert(toggled.clone());
                }
            }
            cx.notify();
        }))
        .child(
            div()
                .w(px(TREE_CHEVRON))
                .flex_none()
                .flex()
                .justify_center()
                .child(crate::motion::turn(
                    format!("{key}:chevron"),
                    crate::motion::fold_angle(open),
                    crate::motion::DETAIL_FOLD,
                    ui::icon(Icon::Chevron, 10.).text_color(t.text_tertiary),
                )),
        )
        .child(
            ui::icon(Icon::Folder, 14.)
                .flex_none()
                .text_color(t.text_tertiary),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(t.text_secondary)
                .child(label.to_owned()),
        )
        .into_any_element()
}

/// A changed file in the list: its icon and name, with how it changed.
fn file_row(
    at: usize,
    changed: &Changed,
    name: &str,
    depth: usize,
    chosen: bool,
    cx: &mut Context<HostArea>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let path = changed.file.path.clone();
    let (letter, ink) = match changed.file.status.as_str() {
        "added" => ("A", t.git_added),
        "removed" => ("D", t.git_deleted),
        "renamed" | "copied" => ("R", t.git_renamed),
        _ => ("M", t.git_modified),
    };
    tree_row(("pull-file", at), depth, cx)
        .when(chosen, |row| row.bg(t.surface_chosen))
        .when(!chosen, |row| row.hover(move |row| row.bg(t.surface_hover)))
        .tooltip(ui::tooltip(path.clone()))
        .on_click(cx.listener(move |area, _, _, cx| {
            if let Some(opened) = &mut area.pull {
                opened.focus(&path);
            }
            cx.notify();
        }))
        .child(div().w(px(TREE_CHEVRON)).flex_none())
        .child(crate::file_icons::glyph(name, 14., cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(t.text_primary)
                .child(name.to_owned()),
        )
        .child(
            div()
                .flex_none()
                .font_family(theme::MONO)
                .text_size(crate::theme::ui_text(11.5))
                .text_color(ink)
                .child(letter),
        )
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
                .text_size(crate::theme::ui_text(12.))
                .child(ui::initials(&author, "", 18., cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(div().truncate().text_color(t.text_primary).child(subject))
                        .child(
                            div()
                                .text_size(crate::theme::ui_text(10.5))
                                .text_color(t.text_tertiary)
                                .child(format!(
                                    "{author} · {}",
                                    commit.date.as_deref().map(ago).unwrap_or_default()
                                )),
                        ),
                )
                .child(ui::mono(short, 11., cx).text_color(t.text_tertiary))
        }))
        .into_any_element()
}

/// One line of the merge box: a mark, what it says, and why.
fn part(outcome: Outcome, title: impl Into<SharedString>, detail: Option<String>, cx: &App) -> Div {
    let t = Tokens::get(cx);
    let title: SharedString = title.into();
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
                .child(ui::run_mark(
                    format!("merge-part-{title}"),
                    outcome_word(outcome),
                    14.,
                    cx,
                ))
                .child(ui::text(title, 12., FontWeight::MEDIUM).text_color(t.text_primary)),
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
        .bg(t.surface_composer)
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
                            .text_size(crate::theme::ui_text(11.))
                            .text_color(t.text_tertiary)
                            .when(!by.is_empty(), |row| {
                                let url = pull.avatars.get(&by).map(String::as_str);
                                row.child(area.face(Some(&by), url, 16., cx))
                                    .child(by.clone())
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
        Some(part(
            Outcome::Queued,
            "This is a draft",
            Some(format!(
                "Mark it ready for review on {host} before merging."
            )),
            cx,
        ))
    } else if area.host.pulls.mergeability {
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
        Some(part(outcome, title, detail, cx))
    } else {
        None
    };
    let blocked = !mergeable(view);
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
                        area.host
                            .pulls
                            .merge_methods
                            .iter()
                            .map(|method| {
                                (SharedString::from(method_label(method).to_owned()), None)
                            })
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
                        .disabled(opened.busy || blocked)
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
        });
    card.child(verdict)
        .children(checks)
        .children(readiness)
        .child(actions)
        .into_any_element()
}

/// The runs on the pull request's head commit, one row each under how they stand.
fn checks(view: &OpenPull, cx: &mut Context<HostArea>) -> Option<Div> {
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
            view.runs.iter().enumerate().map(|(index, listed)| {
                let run = &listed.run;
                let (id, number) = (run.id.clone(), view.pull.number);
                let took = super::elapsed(
                    run.started_at.as_deref().unwrap_or(&run.created_at),
                    (!listed.unfinished).then_some(run.updated_at.as_str()),
                );
                div()
                    .id(("pull-check", index))
                    .h(px(24.))
                    .pl(px(22.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(crate::theme::ui_text(11.5))
                    .rounded(RADIUS_2)
                    .cursor_pointer()
                    .hover(move |row| row.bg(t.surface_hover))
                    .on_click(cx.listener(move |area, _, _, cx| {
                        if area.elsewhere.is_none() && area.desk.is_none() {
                            area.open_run(id.clone(), Some(number), cx)
                        }
                    }))
                    .child(ui::run_mark(
                        format!("pull-check-{}", run.id),
                        outcome_word(run.outcome),
                        13.,
                        cx,
                    ))
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

/// The description and everything said and done since, with the composer under them.
fn conversation(
    area: &mut HostArea,
    window: &mut Window,
    cx: &mut Context<HostArea>,
) -> AnyElement {
    let opened = area.pull.as_ref().expect("a pull request is open");
    let Some(Ok(found)) = &opened.view else {
        return div().into_any_element();
    };
    let pull = &found.pull;
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
    let avatars = &pull.avatars;
    let shown: &HostArea = area;
    let faces = |login: Option<&str>, url: Option<&str>, size: f32, cx: &App| match url {
        Some(_) => shown.face(login, url, size, cx),
        None => {
            let known = login
                .and_then(|login| avatars.get(login))
                .map(String::as_str);
            shown.face(login, known, size, cx)
        }
    };
    let mut column = div();
    column = column.child(thread::thread(
        opening,
        blocks,
        &pushers,
        base.as_deref(),
        &faces,
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
    let review = (pull.state == "open").then(|| {
        let mine = area
            .login()
            .is_some_and(|login| pull.author.as_deref() == Some(login));
        (mine, area.host.pulls.request_changes)
    });
    div()
        .id("pull-thread")
        .size_full()
        .overflow_y_scroll()
        .child(centred(column.child(composer(area, review, window, cx))))
        .into_any_element()
}

/// Every changed file's diff.
fn diffs(area: &mut HostArea, window: &mut Window, cx: &mut Context<HostArea>) -> AnyElement {
    let view = cx.entity();
    let opened = area.pull.as_ref().expect("a pull request is open");
    let Some(Ok(found)) = &opened.view else {
        return div().into_any_element();
    };
    let pull = &found.pull;
    match &opened.files {
        None => super::notice("Loading files", None, cx),
        Some(Err(error)) => super::notice("Could not read the files", Some(error.clone()), cx),
        Some(Ok(diffs)) if diffs.files.is_empty() => super::notice("No files changed.", None, cx),
        Some(Ok(diffs)) => diffs.list(
            &view,
            |area| area.pull.as_ref()?.files.as_ref()?.as_ref().ok(),
            std::rc::Rc::new(|area| area.pull.as_mut()?.files.as_mut()?.as_mut().ok()),
            area.elsewhere.is_none().then_some(area.project.as_str()),
            Some(super::diffs::PullOf {
                number: pull.number,
                title: pull.title.clone(),
                url: pull.url.clone(),
                bench: area.bench.clone(),
            }),
            window,
        ),
    }
}

/// Where a comment or review is written, with what can be sent: `review` says whether
/// the pull request is the person's own, and whether the host takes asking for changes.
fn composer(
    area: &mut HostArea,
    review: Option<(bool, bool)>,
    window: &mut Window,
    cx: &mut Context<HostArea>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let host = super::host_label(&area.host.host).to_owned();
    let Some(opened) = area.pull.as_mut() else {
        return div().into_any_element();
    };
    let placeholder = if review.is_some() {
        "Leave a comment or a review"
    } else {
        "Leave a comment"
    };
    let draft = opened
        .draft
        .get_or_insert_with(|| {
            cx.new(|cx| {
                TextareaState::new(window, cx)
                    .auto_grow(3, 12)
                    .placeholder(placeholder)
            })
        })
        .clone();
    let written = !draft.read(cx).value().trim().is_empty();
    let sending = opened.sending;
    let busy = sending.is_some();
    let mut foot = div()
        .px(px(10.))
        .py(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .border_t_1()
        .border_color(t.border);
    if let Some((true, _)) = review {
        foot = foot.child(ui::meta(
            format!("{host} does not let you approve your own pull request."),
            cx,
        ));
    }
    foot = foot.child(div().flex_1());
    if let Some((false, request_changes)) = review {
        if request_changes {
            foot = foot.child(
                Button::new("review-changes", "Request changes", Variant::Outline)
                    .disabled(busy || !written)
                    .on_click(cx.listener(|area, _, window, cx| {
                        area.send(Some("REQUEST_CHANGES"), window, cx)
                    })),
            );
        }
        let approving = if sending == Some("APPROVE") {
            "Approving…"
        } else {
            "Approve"
        };
        foot = foot.child(
            Button::new("review-approve", approving, Variant::Outline)
                .disabled(busy)
                .on_click(
                    cx.listener(|area, _, window, cx| area.send(Some("APPROVE"), window, cx)),
                ),
        );
    }
    let state = (opened.view.as_ref())
        .and_then(|view| view.as_ref().ok())
        .map(|view| view.pull.state.clone())
        .unwrap_or_default();
    let reopens = area.host.pulls.reopen;
    match state.as_str() {
        "open" => {
            let closing = if written && reopens {
                "Close with comment"
            } else {
                "Close pull request"
            };
            foot = foot.child(
                Button::new("pull-close", closing, Variant::Outline)
                    .icon(Icon::PullRequest)
                    .disabled(busy)
                    .on_click(cx.listener(|area, _, window, cx| area.close(window, cx))),
            );
        }
        "closed" if reopens => {
            foot = foot.child(
                Button::new("pull-reopen", "Reopen pull request", Variant::Outline)
                    .icon(Icon::PullRequest)
                    .disabled(busy)
                    .on_click(cx.listener(|area, _, _, cx| area.set_state(true, cx))),
            );
        }
        _ => {}
    }
    let commenting = if sending == Some("comment") {
        "Sending…"
    } else {
        "Comment"
    };
    foot = foot.child(
        Button::new("review-comment", commenting, Variant::Primary)
            .disabled(busy || !written)
            .on_click(cx.listener(|area, _, window, cx| area.send(None, window, cx))),
    );
    div()
        .px(px(16.))
        .pb(px(16.))
        .child(
            div()
                .rounded(RADIUS_2)
                .border_1()
                .border_color(t.border)
                .bg(t.surface_composer)
                .child(
                    div()
                        .px(px(12.))
                        .py(px(8.))
                        .text_size(crate::theme::ui_text(13.))
                        .child(
                            Textarea::new(&draft)
                                .appearance(false)
                                .text_size(crate::theme::ui_text(13.)),
                        ),
                )
                .child(foot),
        )
        .into_any_element()
}
