//! The Git pane: the branch and what syncs it, the changes to commit with the commit
//! box, and every changed file's diff beside them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::FRAC_PI_2;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::base::{Button as BaseButton, Popover, PopoverState};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, list, px, radians, relative, Action, Anchor, AnyElement, App, AppContext,
    ClipboardItem, Context, Div, ElementId, Entity, FocusHandle, Focusable, FontWeight, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ListAlignment, ListState, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Task, Transformation, Window,
};
use serde_json::{json, Value};
use sikemux_core::git::log::{GitCommit, GitOverview};
use sikemux_core::git::stash::GitStash;
use sikemux_core::git::status::GitFile;
use tokio::sync::mpsc::UnboundedSender;

use branches::{branch_card, branches_page, Branches};
use history::{commit_card, commit_row, graph, Graph, Inks};

mod branches;
mod history;
use super::review::{review, Review};
use crate::core::{request, Ask};
use crate::editor::OpenAt;
use crate::icons::Icon;
use crate::theme::{self, Tokens, RADIUS_2};
use crate::ui::{self, Choice, Choices, Confirm, Menu, MenuItem, Prompt};

/// Brings the project's Git tab to the front, opening one when it has none.
#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "window.git")]
pub struct FocusGit;

const ROW_HEIGHT: f32 = 26.;
/// How long typing rests before the commit message is kept as the draft.
const DRAFT_PAUSE: Duration = Duration::from_millis(300);
/// How long a failed operation's message stays in the head.
const FAILED_FOR: Duration = Duration::from_millis(3500);

/// The side column's pages.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Changes,
    History,
    Branches,
}

/// A page's filter: its field once shown, and what it finds.
#[derive(Default)]
struct Filter {
    field: Option<Entity<InputState>>,
    query: String,
    /// The Changes filter shows once asked for; the other pages' always do.
    open: bool,
    typing: Option<Subscription>,
}

/// What the review column shows.
enum Shown {
    Changes,
    Commit(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Staged,
    Unstaged,
}

pub fn is_staged(file: &GitFile) -> bool {
    file.index != " " && file.index != "?"
}

pub fn has_unstaged(file: &GitFile) -> bool {
    file.worktree != " "
}

/// How a file reads once it has moved to `side`, before git has said so.
fn moved_to(file: &GitFile, side: Side) -> GitFile {
    let mut moved = file.clone();
    match side {
        Side::Staged if has_unstaged(file) => {
            if !is_staged(file) {
                moved.index = if file.worktree == "?" {
                    "A".into()
                } else {
                    file.worktree.clone()
                };
            }
            moved.worktree = " ".into();
        }
        Side::Unstaged if is_staged(file) => {
            let worktree = if has_unstaged(file) {
                file.worktree.clone()
            } else if file.index == "A" {
                "?".into()
            } else {
                file.index.clone()
            };
            moved.index = if worktree == "?" { "?" } else { " " }.into();
            moved.worktree = worktree;
        }
        _ => {}
    }
    moved
}

/// What git's status letter for one side of a file means: the letter shown, and its colour.
fn decoration(code: &str, t: &Tokens) -> Option<(&'static str, Hsla, &'static str)> {
    Some(match code.trim() {
        "" => return None,
        "?" => ("U", t.git_untracked, "untracked"),
        "U" => ("U", t.danger, "unmerged"),
        "A" => ("A", t.git_added, "added"),
        "D" => ("D", t.git_deleted, "deleted"),
        "R" => ("R", t.git_renamed, "renamed"),
        "C" => ("C", t.git_renamed, "copied"),
        "T" => ("T", t.git_modified, "type changed"),
        _ => ("M", t.git_modified, "modified"),
    })
}

/// A file's status letter as a review card shows it.
pub fn status_mark(code: &str, cx: &App) -> Option<Div> {
    let (letter, colour, _) = decoration(code, Tokens::get(cx))?;
    Some(
        div()
            .font_family(theme::MONO)
            .text_size(px(10.))
            .font_weight(FontWeight::BOLD)
            .text_color(colour)
            .child(letter),
    )
}

/// Something done to the pane, held by a button or a question until it is pressed.
type Act = Rc<dyn Fn(&mut GitPane, &mut Context<GitPane>)>;

#[derive(Clone, PartialEq)]
enum Entry {
    Group { side: Side, count: usize },
    File { side: Side, file: GitFile },
}

/// Files moved between the groups by one click, which run one after another.
struct Move {
    paths: Vec<String>,
    side: Side,
    method: &'static str,
    params: Value,
}

/// Something done with a line of text the person typed, such as a name.
type Named = Rc<dyn Fn(&mut GitPane, String, &mut Context<GitPane>)>;

/// What the pane asks the person before going on.
enum Asking {
    /// Whether to go ahead with something, `destructive` when it cannot be undone.
    Confirm {
        title: SharedString,
        body: SharedString,
        yes: SharedString,
        destructive: bool,
        then: Act,
    },
    /// A line of text, such as a name; its field is made when the pane next draws.
    Prompt {
        title: SharedString,
        placeholder: SharedString,
        initial: String,
        field: Option<Entity<InputState>>,
        then: Named,
    },
    /// One of several things to do, each with its key.
    Menu {
        title: SharedString,
        choices: Vec<Pick>,
    },
}

/// One choice of a menu the pane asks with.
struct Pick {
    key: Option<char>,
    label: SharedString,
    hint: Option<SharedString>,
    destructive: bool,
    disabled: bool,
    then: Act,
}

impl Pick {
    fn new(
        key: impl Into<Option<char>>,
        label: impl Into<SharedString>,
        then: impl Fn(&mut GitPane, &mut Context<GitPane>) + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            destructive: false,
            disabled: false,
            then: Rc::new(then),
        }
    }

    fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
}

pub struct GitPane {
    pub(super) asks: UnboundedSender<Ask>,
    pub repo: String,
    overview: Option<Result<GitOverview, String>>,
    /// Files moved to a group before git says so, by path.
    moving: HashMap<String, Side>,
    /// Moves git has made, whose files return to what git says at its next overview.
    moved: Vec<String>,
    queue: VecDeque<Move>,
    running: bool,
    busy: Option<SharedString>,
    failed: Option<SharedString>,
    folded: HashSet<Side>,
    selected: usize,
    entries: Vec<Entry>,
    rows: ListState,
    pub(super) review: Review,
    review_folded: bool,
    page: Page,
    /// Where a range of rows started, which Space stages and `d` discards at once.
    range: Option<usize>,
    /// The commit chosen in the history, by its place in the log.
    commit: usize,
    graph: Rc<Graph>,
    hashes: Vec<String>,
    commit_rows: ListState,
    /// The commit whose files were last asked for, so it is asked for once.
    asked_commit: Option<String>,
    branches: Branches,
    branch_rows: ListState,
    stashes: Vec<GitStash>,
    /// The agents that write commit messages: their kind, name and models, the
    /// default first, as the core's catalog lists them.
    writers: Vec<(String, String, Vec<String>)>,
    /// The agent and model the person picked to write messages, else the core's default.
    writer: Option<(String, String)>,
    /// The commits the history's search finds, which the graph lays out.
    commits: Vec<GitCommit>,
    /// Each page's filter, by `Page`.
    filters: [Filter; 3],
    draft: Entity<TextareaState>,
    /// A message written into the draft from outside the window's turn, set when it next draws.
    written: Option<String>,
    /// The commit message as the core last kept it.
    synced: String,
    _keeping: Task<()>,
    generating: bool,
    asking: Option<Asking>,
    /// The row or card header the pointer is on, whose actions show.
    pub(super) hovered: Option<SharedString>,
    focus: FocusHandle,
    _clearing: Task<()>,
    _drafting: Subscription,
}

impl GitPane {
    pub fn new(
        asks: UnboundedSender<Ask>,
        repo: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 9)
                .placeholder("Message (⌘⏎ to commit)")
        });
        let drafting = cx.subscribe(&draft, |pane, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                pane.typed(cx);
                cx.notify();
            }
        });
        Self {
            asks,
            repo,
            overview: None,
            moving: HashMap::new(),
            moved: Vec::new(),
            queue: VecDeque::new(),
            running: false,
            busy: None,
            failed: None,
            folded: HashSet::new(),
            selected: 0,
            entries: Vec::new(),
            rows: ListState::new(0, ListAlignment::Top, px(400.)),
            review: Review::default(),
            review_folded: false,
            page: Page::Changes,
            range: None,
            commit: 0,
            graph: Rc::new(graph(&[])),
            hashes: Vec::new(),
            commit_rows: ListState::new(0, ListAlignment::Top, px(400.)),
            asked_commit: None,
            branches: Branches::default(),
            stashes: Vec::new(),
            writers: Vec::new(),
            writer: None,
            commits: Vec::new(),
            filters: Default::default(),
            branch_rows: ListState::new(0, ListAlignment::Top, px(400.)),
            draft,
            written: None,
            synced: String::new(),
            _keeping: Task::ready(()),
            generating: false,
            asking: None,
            hovered: None,
            focus: cx.focus_handle(),
            _clearing: Task::ready(()),
            _drafting: drafting,
        }
    }

    /// Takes what `git.overview` last answered for the repository.
    pub fn overview(&mut self, value: &Value, cx: &mut Context<Self>) {
        self.overview =
            Some(serde_json::from_value(value.clone()).map_err(|error| error.to_string()));
        for path in self.moved.drain(..) {
            self.moving.remove(&path);
        }
        if self.review.commit().is_none() {
            self.review.reread();
        }
        self.list_commits();
        self.list_branches();
        if self.page == Page::Branches {
            self.read_remotes(cx);
        }
        self.read_stashes(cx);
        self.list(cx);
    }

    /// Takes the agents the core's catalog says write commit messages.
    pub fn catalog(&mut self, value: &Value, cx: &mut Context<Self>) {
        self.writers = value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|agent| {
                let models: Vec<String> = agent["commitModels"]
                    .as_array()?
                    .iter()
                    .filter_map(|model| model.as_str().map(str::to_owned))
                    .collect();
                (!models.is_empty()).then(|| {
                    (
                        agent["type"].as_str().unwrap_or_default().to_owned(),
                        agent["label"].as_str().unwrap_or_default().to_owned(),
                        models,
                    )
                })
            })
            .collect();
        cx.notify();
    }

    /// The agent and model that write the next message: the one picked, else the core's
    /// default, which is Hermes's first model.
    fn writer(&self) -> Option<(String, String, String)> {
        let (kind, model) = match &self.writer {
            Some((kind, model)) => (kind.clone(), Some(model.clone())),
            None => ("hermes".to_owned(), None),
        };
        let (_, label, models) = self.writers.iter().find(|(writer, _, _)| *writer == kind)?;
        let model = model.or_else(|| models.first().cloned())?;
        Some((kind, label.clone(), model))
    }

    /// Lists the commits the history's search finds, laying the graph out again when
    /// they changed.
    fn list_commits(&mut self) {
        let query = self.filters[Page::History as usize].query.clone();
        let shown: Vec<GitCommit> = self
            .log()
            .iter()
            .filter(|commit| {
                query.is_empty()
                    || [&commit.subject, &commit.hash, &commit.author]
                        .iter()
                        .any(|field| field.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();
        let hashes: Vec<String> = shown
            .iter()
            .map(|commit| commit.full_hash.clone())
            .collect();
        if hashes != self.hashes {
            self.graph = Rc::new(graph(&shown));
            super::relist(&self.commit_rows, &self.hashes, &hashes);
            self.hashes = hashes;
            self.commit = self.commit.min(self.hashes.len().saturating_sub(1));
        }
        self.commits = shown;
    }

    /// Makes the filter field of the page in front when it shows one, and keeps its
    /// query as the person types.
    fn filter_field(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<InputState>> {
        let page = self.page;
        let filter = &self.filters[page as usize];
        if page == Page::Changes && !filter.open && filter.query.is_empty() {
            return None;
        }
        if let Some(field) = &filter.field {
            return Some(field.clone());
        }
        let placeholder = match page {
            Page::Changes => "Filter files",
            Page::History => "Search commits",
            Page::Branches => "Filter branches",
        };
        let field = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let typing = cx.subscribe_in(
            &field,
            window,
            move |pane, field, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    pane.filters[page as usize].query = field.read(cx).value().to_lowercase();
                    pane.refilter(page, cx);
                }
                InputEvent::PressEnter { .. } => {
                    pane.filters[page as usize].open = false;
                    window.focus(&pane.focus, cx);
                }
                _ => {}
            },
        );
        let filter = &mut self.filters[page as usize];
        filter.field = Some(field.clone());
        filter.typing = Some(typing);
        Some(field)
    }

    fn refilter(&mut self, page: Page, cx: &mut Context<Self>) {
        match page {
            Page::Changes => {
                self.selected = 0;
                self.list(cx);
            }
            Page::History => {
                self.commit = 0;
                self.list_commits();
                self.follow(cx);
            }
            Page::Branches => {
                self.branches.chosen = 0;
                self.list_branches();
                self.follow(cx);
            }
        }
        cx.notify();
    }

    /// Puts the keyboard in the page's filter, opening the Changes one.
    fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filters[self.page as usize].open = true;
        if let Some(field) = self.filter_field(window, cx) {
            field.update(cx, |field, cx| field.focus(window, cx));
        }
        cx.notify();
    }

    /// Clears the page's filter, closing the Changes one; false when there was none.
    fn unfind(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let filter = &mut self.filters[self.page as usize];
        if filter.query.is_empty() && !filter.open {
            return false;
        }
        filter.open = false;
        filter.query.clear();
        if let Some(field) = filter.field.clone() {
            field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        let page = self.page;
        self.refilter(page, cx);
        window.focus(&self.focus, cx);
        true
    }

    fn log(&self) -> &[GitCommit] {
        match &self.overview {
            Some(Ok(overview)) => &overview.log,
            _ => &[],
        }
    }

    pub(super) fn choose_commit(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit = index;
        window.focus(&self.focus, cx);
        self.follow(cx);
        cx.notify();
    }

    fn step_commit(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.hashes.is_empty() {
            return;
        }
        self.commit =
            (self.commit as isize + step).clamp(0, self.hashes.len() as isize - 1) as usize;
        self.commit_rows.scroll_to_reveal_item(self.commit);
        self.follow(cx);
        cx.notify();
    }

    fn turn(&mut self, page: Page, cx: &mut Context<Self>) {
        if page == Page::Branches && self.page != page {
            self.read_remotes(cx);
        }
        self.page = page;
        self.follow(cx);
        cx.notify();
    }

    /// What the review column shows: the changes while there are some and Changes is in
    /// front, else the commit chosen in the history, else the latest.
    fn shown(&self) -> Option<Shown> {
        let changes = self.page == Page::Changes && !self.entries.is_empty();
        if changes {
            return Some(Shown::Changes);
        }
        if self.page == Page::Branches {
            let entry = self.chosen_branch()?;
            return entry.rev().map(|rev| Shown::Commit(rev.to_owned()));
        }
        let commit = if self.page == Page::History {
            self.commits.get(self.commit)
        } else {
            self.log().first()
        };
        commit.map(|commit| Shown::Commit(commit.full_hash.clone()))
    }

    /// Points the review at what it shows, asking for a commit's files the first time.
    fn follow(&mut self, cx: &mut Context<Self>) {
        match self.shown() {
            Some(Shown::Changes) => {
                let files = self.listed_files();
                self.review.show_changes(&self.repo, &files);
                let path = self.chosen().map(|(_, file)| file.path.clone());
                self.review.focus(path.as_deref());
            }
            Some(Shown::Commit(hash)) => {
                if self.review.commit() == Some(hash.as_str())
                    || self.asked_commit.as_deref() == Some(hash.as_str())
                {
                    return;
                }
                self.asked_commit = Some(hash.clone());
                let params = self.repo_call(json!({ "rev": hash }));
                let answer = request(&self.asks, "git.commitFiles", params);
                cx.spawn(async move |pane, cx| {
                    let paths: Vec<String> = answer
                        .await
                        .ok()
                        .and_then(|paths| serde_json::from_value(paths).ok())
                        .unwrap_or_default();
                    pane.update(cx, |pane, cx| {
                        if pane.asked_commit.as_deref() == Some(hash.as_str()) {
                            let repo = pane.repo.clone();
                            pane.review.show_commit(&repo, &hash, &paths);
                            cx.notify();
                        }
                    })
                    .ok();
                })
                .detach();
            }
            None => self.review.show_changes(&self.repo, &[]),
        }
    }

    /// The repository's changed files, moved as the person moved them.
    fn files(&self) -> Vec<GitFile> {
        let Some(Ok(overview)) = &self.overview else {
            return Vec::new();
        };
        overview
            .status
            .files
            .iter()
            .map(|file| match self.moving.get(&file.path) {
                Some(side) => moved_to(file, *side),
                None => file.clone(),
            })
            .collect()
    }

    /// The changed files the Changes filter finds.
    fn listed_files(&self) -> Vec<GitFile> {
        let query = &self.filters[Page::Changes as usize].query;
        let mut files = self.files();
        if !query.is_empty() {
            files.retain(|file| file.path.to_lowercase().contains(query.as_str()));
        }
        files
    }

    fn list(&mut self, cx: &mut Context<Self>) {
        let files = self.listed_files();
        let mut entries = Vec::new();
        for (side, wanted) in [
            (Side::Staged, is_staged as fn(&GitFile) -> bool),
            (Side::Unstaged, has_unstaged),
        ] {
            let listed: Vec<&GitFile> = files.iter().filter(|file| wanted(file)).collect();
            if listed.is_empty() {
                continue;
            }
            entries.push(Entry::Group {
                side,
                count: listed.len(),
            });
            if !self.folded.contains(&side) {
                entries.extend(listed.into_iter().map(|file| Entry::File {
                    side,
                    file: file.clone(),
                }));
            }
        }
        super::relist(&self.rows, &self.entries, &entries);
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.follow(cx);
        cx.notify();
    }

    /// The file the list's selection is on, else the first file listed.
    fn chosen(&self) -> Option<(Side, &GitFile)> {
        fn file(entry: &Entry) -> Option<(Side, &GitFile)> {
            match entry {
                Entry::File { side, file } => Some((*side, file)),
                Entry::Group { .. } => None,
            }
        }
        self.entries
            .get(self.selected)
            .and_then(file)
            .or_else(|| self.entries.iter().find_map(file))
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        self.follow(cx);
        cx.notify();
    }

    /// Moves the selection by `step` files, over the groups' rows.
    fn step(&mut self, step: isize, cx: &mut Context<Self>) {
        let files: Vec<usize> = (0..self.entries.len())
            .filter(|index| matches!(self.entries[*index], Entry::File { .. }))
            .collect();
        let Some(at) = files
            .iter()
            .position(|index| *index >= self.selected)
            .or(files.len().checked_sub(1))
        else {
            return;
        };
        let next = (at as isize + step).clamp(0, files.len() as isize - 1) as usize;
        self.select(files[next], cx);
        self.rows.scroll_to_reveal_item(files[next]);
    }

    /// Runs one git operation at a time, saying in the head what it is while it runs and
    /// what went wrong if it fails.
    fn run(
        &mut self,
        label: impl Into<SharedString>,
        method: &'static str,
        params: Value,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<Value, String>, &mut Context<Self>) + 'static,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(label.into());
        self.failed = None;
        let answer = request(&self.asks, method, params);
        cx.spawn(async move |pane, cx| {
            let answer = answer.await;
            pane.update(cx, |pane, cx| {
                pane.busy = None;
                if let Err(error) = &answer {
                    pane.fail(error, cx);
                }
                then(pane, answer, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn fail(&mut self, error: &str, cx: &mut Context<Self>) {
        let first = error.trim().lines().next().unwrap_or_default();
        let shown: String = first.chars().take(80).collect();
        let cut = if shown.len() < first.len() { "…" } else { "" };
        self.failed = Some(format!("✗ {shown}{cut}").into());
        self._clearing = cx.spawn(async move |pane, cx| {
            cx.background_executor().timer(FAILED_FOR).await;
            pane.update(cx, |pane, cx| {
                pane.failed = None;
                cx.notify();
            })
            .ok();
        });
    }

    fn repo_call(&self, extra: Value) -> Value {
        let mut params = json!({ "repo": self.repo });
        if let (Some(params), Value::Object(extra)) = (params.as_object_mut(), extra) {
            params.extend(extra);
        }
        params
    }

    /// Moves files between the groups at once; git's next overview confirms the move
    /// or puts them back. Clicks in quick succession wait their turn.
    fn shift(
        &mut self,
        paths: Vec<String>,
        side: Side,
        method: &'static str,
        params: Value,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        for path in &paths {
            self.moving.insert(path.clone(), side);
        }
        self.queue.push_back(Move {
            paths,
            side,
            method,
            params,
        });
        self.list(cx);
        self.next_move(cx);
    }

    fn next_move(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        let Some(next) = self.queue.pop_front() else {
            return;
        };
        self.running = true;
        let answer = request(&self.asks, next.method, next.params);
        cx.spawn(async move |pane, cx| {
            let answer = answer.await;
            pane.update(cx, |pane, cx| {
                pane.running = false;
                if let Err(error) = &answer {
                    pane.fail(error, cx);
                }
                let later: HashSet<&String> =
                    pane.queue.iter().flat_map(|queued| &queued.paths).collect();
                let settled: Vec<String> = next
                    .paths
                    .into_iter()
                    .filter(|path| {
                        !later.contains(path) && pane.moving.get(path) == Some(&next.side)
                    })
                    .collect();
                pane.moved.extend(settled);
                pane.next_move(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn stage(&mut self, file: &GitFile, cx: &mut Context<Self>) {
        let params = self.repo_call(json!({ "path": file.path }));
        self.shift(
            vec![file.path.clone()],
            Side::Staged,
            "git.stage",
            params,
            cx,
        );
    }

    fn unstage(&mut self, file: &GitFile, cx: &mut Context<Self>) {
        let params = self.repo_call(json!({ "path": file.path }));
        self.shift(
            vec![file.path.clone()],
            Side::Unstaged,
            "git.unstage",
            params,
            cx,
        );
    }

    fn stage_all(&mut self, cx: &mut Context<Self>) {
        let paths = self.paths(has_unstaged);
        let params = self.repo_call(json!({}));
        self.shift(paths, Side::Staged, "git.stageAll", params, cx);
    }

    fn unstage_all(&mut self, cx: &mut Context<Self>) {
        let paths = self.paths(is_staged);
        let params = self.repo_call(json!({}));
        self.shift(paths, Side::Unstaged, "git.unstageAll", params, cx);
    }

    fn paths(&self, wanted: fn(&GitFile) -> bool) -> Vec<String> {
        self.files()
            .into_iter()
            .filter(wanted)
            .map(|file| file.path)
            .collect()
    }

    fn toggle_stage(&mut self, cx: &mut Context<Self>) {
        if self.range.is_some() {
            let mut paths: Vec<String> = Vec::new();
            for entry in self.in_range() {
                if let Entry::File {
                    side: Side::Unstaged,
                    file,
                } = entry
                {
                    if !paths.contains(&file.path) {
                        paths.push(file.path.clone());
                    }
                }
            }
            self.range = None;
            let params = self.repo_call(json!({ "paths": paths }));
            self.shift(paths, Side::Staged, "git.stagePaths", params, cx);
            return;
        }
        match self.chosen().map(|(side, file)| (side, file.clone())) {
            Some((Side::Staged, file)) => self.unstage(&file, cx),
            Some((Side::Unstaged, file)) => self.stage(&file, cx),
            None => {}
        }
    }

    /// Asks whether to go ahead with something that cannot be undone.
    fn ask(
        &mut self,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        yes: impl Into<SharedString>,
        then: impl Fn(&mut GitPane, &mut Context<GitPane>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.confirm(title, body, yes, true, then, cx);
    }

    fn confirm(
        &mut self,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        yes: impl Into<SharedString>,
        destructive: bool,
        then: impl Fn(&mut GitPane, &mut Context<GitPane>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.asking = Some(Asking::Confirm {
            title: title.into(),
            body: body.into(),
            yes: yes.into(),
            destructive,
            then: Rc::new(then),
        });
        cx.notify();
    }

    /// Asks for a line of text, such as a name, and goes on with what was typed.
    fn prompt(
        &mut self,
        title: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
        initial: impl Into<String>,
        then: impl Fn(&mut GitPane, String, &mut Context<GitPane>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.asking = Some(Asking::Prompt {
            title: title.into(),
            placeholder: placeholder.into(),
            initial: initial.into(),
            field: None,
            then: Rc::new(then),
        });
        cx.notify();
    }

    fn menu(&mut self, title: impl Into<SharedString>, choices: Vec<Pick>, cx: &mut Context<Self>) {
        self.asking = Some(Asking::Menu {
            title: title.into(),
            choices,
        });
        cx.notify();
    }

    /// The question being asked, as a dialog, making a prompt's field the first time.
    fn asked(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pane = cx.entity();
        let done = move |pane: &Entity<GitPane>| {
            let pane = pane.clone();
            move |window: &mut Window, cx: &mut App| {
                pane.update(cx, |pane, cx| {
                    pane.asking = None;
                    window.focus(&pane.focus, cx);
                    cx.notify();
                })
            }
        };
        Some(match self.asking.as_mut()? {
            Asking::Confirm {
                title,
                body,
                yes,
                destructive,
                then,
            } => {
                let (then, going) = (then.clone(), pane.clone());
                Confirm::new(title.clone(), body.clone(), yes.clone())
                    .destructive(*destructive)
                    .on_yes(move |_, cx| {
                        let then = then.clone();
                        going.update(cx, |pane, cx| {
                            pane.asking = None;
                            then(pane, cx);
                            cx.notify();
                        })
                    })
                    .on_no(done(&pane))
                    .into_any_element()
            }
            Asking::Prompt {
                title,
                placeholder,
                initial,
                field,
                then,
            } => {
                let field = field
                    .get_or_insert_with(|| {
                        let (placeholder, initial) = (placeholder.clone(), initial.clone());
                        cx.new(|cx| {
                            let mut state = InputState::new(window, cx).placeholder(placeholder);
                            state.set_value(initial, window, cx);
                            state.focus(window, cx);
                            state
                        })
                    })
                    .clone();
                let (then, going) = (then.clone(), pane.clone());
                Prompt::new(title.clone(), &field)
                    .on_yes(move |typed, _, cx| {
                        let then = then.clone();
                        going.update(cx, |pane, cx| {
                            pane.asking = None;
                            then(pane, typed.trim().to_owned(), cx);
                            cx.notify();
                        })
                    })
                    .on_no(done(&pane))
                    .into_any_element()
            }
            Asking::Menu { title, choices } => {
                let choices = choices
                    .iter()
                    .map(|pick| {
                        let (then, going) = (pick.then.clone(), pane.clone());
                        let mut choice = Choice::new(pick.key, pick.label.clone(), move |_, cx| {
                            let then = then.clone();
                            going.update(cx, |pane, cx| {
                                pane.asking = None;
                                then(pane, cx);
                                cx.notify();
                            })
                        })
                        .disabled(pick.disabled);
                        if let Some(hint) = &pick.hint {
                            choice = choice.hint(hint.clone());
                        }
                        if pick.destructive {
                            choice = choice.destructive();
                        }
                        choice
                    })
                    .collect();
                Choices::new(title.clone(), choices)
                    .on_no(done(&pane))
                    .into_any_element()
            }
        })
    }

    fn discard(&mut self, file: &GitFile, cx: &mut Context<Self>) {
        let name = file
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&file.path)
            .to_owned();
        let body = if file.worktree == "?" {
            "The file is new and will be deleted. This can't be undone."
        } else {
            "Unstaged changes to this file will be lost. This can't be undone."
        };
        let path = file.path.clone();
        self.ask(
            format!("Discard changes to {name}?"),
            body,
            "discard",
            move |pane, cx| {
                let params = pane.repo_call(json!({ "paths": [path], "mode": "unstaged" }));
                pane.run(
                    format!("discarding {path}"),
                    "git.discardFiles",
                    params,
                    cx,
                    |_, _, _| {},
                );
            },
            cx,
        );
    }

    fn discard_all(&mut self, cx: &mut Context<Self>) {
        let unstaged: Vec<GitFile> = self.files().into_iter().filter(has_unstaged).collect();
        if unstaged.is_empty() {
            return;
        }
        let added = unstaged.iter().filter(|file| file.worktree == "?").count();
        let count = format!(
            "{} file{}",
            unstaged.len(),
            if unstaged.len() == 1 { "" } else { "s" }
        );
        let fate = match added {
            0 => "Every file goes back to what is staged or committed. ".to_owned(),
            1 => "1 new file is deleted and the rest go back to what is staged or committed. ".to_owned(),
            added => format!("{added} new files are deleted and the rest go back to what is staged or committed. "),
        };
        let paths: Vec<String> = unstaged.into_iter().map(|file| file.path).collect();
        self.ask(
            format!("Discard unstaged changes in {count}?"),
            format!("{fate}Staged changes are kept. This can't be undone."),
            "discard all",
            move |pane, cx| {
                let params = pane.repo_call(json!({ "paths": paths, "mode": "unstaged" }));
                pane.run(
                    format!("discarding {count}"),
                    "git.discardFiles",
                    params,
                    cx,
                    |_, _, _| {},
                );
            },
            cx,
        );
    }

    fn stash_menu(&mut self, cx: &mut Context<Self>) {
        let stash = |mode: &'static str, title: &'static str| {
            move |pane: &mut GitPane, cx: &mut Context<GitPane>| {
                pane.prompt(
                    title,
                    "(optional) stash message",
                    "",
                    move |pane, message, cx| {
                        let message = (!message.is_empty()).then_some(message);
                        let params = pane.repo_call(json!({ "mode": mode, "message": message }));
                        pane.run(
                            format!("stashing ({mode})"),
                            "git.stashPush",
                            params,
                            cx,
                            |_, _, _| {},
                        );
                    },
                    cx,
                )
            }
        };
        self.menu(
            "Stash",
            vec![
                Pick::new(
                    's',
                    "stash everything (working tree + index + untracked)",
                    stash("all", "Stash everything"),
                ),
                Pick::new(
                    'i',
                    "stash staged only",
                    stash("staged", "Stash staged changes"),
                ),
                Pick::new(
                    'u',
                    "stash unstaged only (keep index)",
                    stash("unstaged", "Stash unstaged changes"),
                ),
            ],
            cx,
        );
    }

    /// The ways to throw away the chosen file's changes, each asked about once more.
    /// The ways to throw away the changes of the chosen file, or of every file in the
    /// range, each asked about once more.
    fn discard_menu(&mut self, cx: &mut Context<Self>) {
        let mut paths: Vec<String> = Vec::new();
        for entry in self.in_range() {
            if let Entry::File { file, .. } = entry {
                if !paths.contains(&file.path) {
                    paths.push(file.path.clone());
                }
            }
        }
        let Some(first) = paths.first().cloned() else {
            return;
        };
        let count = paths.len();
        let files = format!("{count} file{}", if count > 1 { "s" } else { "" });
        let label = if count > 1 {
            format!("Discard {count} files")
        } else {
            format!(
                "Discard changes — {}",
                first.rsplit('/').next().unwrap_or(&first)
            )
        };
        let apply = |mode: &'static str, body: String, yes: &'static str| {
            let (paths, label, files) = (paths.clone(), label.clone(), files.clone());
            move |pane: &mut GitPane, cx: &mut Context<GitPane>| {
                let (paths, files) = (paths.clone(), files.clone());
                pane.range = None;
                pane.ask(
                    label.clone(),
                    body.clone(),
                    yes,
                    move |pane, cx| {
                        let params = pane.repo_call(json!({ "paths": paths, "mode": mode }));
                        pane.run(
                            format!("discarding {files} ({mode})"),
                            "git.discardFiles",
                            params,
                            cx,
                            |_, _, _| {},
                        );
                    },
                    cx,
                )
            }
        };
        self.menu(
            label.clone(),
            vec![
                Pick::new('d', "discard all changes", apply("all", format!("This will discard BOTH staged and unstaged changes in {files}. Cannot be undone."), "discard"))
                    .hint("staged + worktree")
                    .destructive(),
                Pick::new('u', "discard unstaged changes", apply("unstaged", format!("Discard unstaged changes in {files}? (staged changes preserved)"), "discard"))
                    .destructive(),
                Pick::new('s', "unstage changes", apply("staged", format!("Unstage {files} from the index? (worktree preserved)"), "unstage")),
            ],
            cx,
        );
    }

    /// The rows from the range's anchor to the selection, else the chosen file's.
    fn in_range(&self) -> Vec<&Entry> {
        match self.range {
            Some(anchor) => {
                let (from, to) = (anchor.min(self.selected), anchor.max(self.selected));
                self.entries.iter().skip(from).take(to + 1 - from).collect()
            }
            None => self
                .entries
                .get(self.selected)
                .filter(|entry| matches!(entry, Entry::File { .. }))
                .into_iter()
                .collect(),
        }
    }

    fn read_stashes(&mut self, cx: &mut Context<Self>) {
        let answer = request(&self.asks, "git.stashList", self.repo_call(json!({})));
        cx.spawn(async move |pane, cx| {
            let stashes: Vec<GitStash> = answer
                .await
                .ok()
                .and_then(|stashes| serde_json::from_value(stashes).ok())
                .unwrap_or_default();
            pane.update(cx, |pane, cx| {
                pane.stashes = stashes;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Reads everything the pane shows from the repository again.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let answer = request(&self.asks, "git.overview", self.repo_call(json!({})));
        cx.spawn(async move |pane, cx| {
            if let Ok(overview) = answer.await {
                pane.update(cx, |pane, cx| pane.overview(&overview, cx))
                    .ok();
            }
        })
        .detach();
        self.read_remotes(cx);
    }

    fn add_remote(&mut self, cx: &mut Context<Self>) {
        self.prompt(
            "Add remote",
            "name (e.g. upstream) — then you'll enter the URL",
            "",
            |pane, name, cx| {
                if name.is_empty() {
                    return;
                }
                pane.prompt(
                    format!("Add remote · {name}"),
                    "URL (https://… or git@…)",
                    "",
                    move |pane, url, cx| {
                        if url.is_empty() {
                            return;
                        }
                        let params = pane.repo_call(json!({ "name": name, "url": url }));
                        pane.run(
                            format!("adding remote {name}"),
                            "git.remoteAdd",
                            params,
                            cx,
                            |pane, _, cx| pane.read_remotes(cx),
                        );
                    },
                    cx,
                );
            },
            cx,
        );
    }

    /// The stashes, each opening what can be done with it.
    fn stashes_menu(&mut self, cx: &mut Context<Self>) {
        let picks = self
            .stashes
            .iter()
            .enumerate()
            .map(|(index, stash)| {
                let key = (index < 9)
                    .then(|| char::from_digit(index as u32 + 1, 10))
                    .flatten();
                let chosen = stash.clone();
                Pick::new(key, stash.message.clone(), move |pane, cx| {
                    pane.stash_menu_of(&chosen, cx)
                })
                .hint(stash.refname.clone())
            })
            .collect();
        self.menu("Stashes", picks, cx);
    }

    fn stash_menu_of(&mut self, stash: &GitStash, cx: &mut Context<Self>) {
        let call = |method: &'static str, label: String, extra: Value| {
            let (refname, sha) = (stash.refname.clone(), stash.sha.clone());
            move |pane: &mut GitPane, cx: &mut Context<GitPane>| {
                let mut params = json!({ "refname": refname, "sha": sha });
                if let (Some(params), Value::Object(extra)) =
                    (params.as_object_mut(), extra.clone())
                {
                    params.extend(extra);
                }
                let params = pane.repo_call(params);
                pane.run(label.clone(), method, params, cx, |pane, _, cx| {
                    pane.read_stashes(cx)
                });
            }
        };
        let apply = call(
            "git.stashApply",
            format!("applying {}", stash.refname),
            json!({}),
        );
        let pop = call(
            "git.stashPop",
            format!("popping {}", stash.refname),
            json!({}),
        );
        let drop = call(
            "git.stashDrop",
            format!("dropping {}", stash.refname),
            json!({}),
        );
        let (branched, renamed) = (stash.clone(), stash.clone());
        let refname = stash.refname.clone();
        self.menu(
            format!("{} · {}", stash.refname, stash.message),
            vec![
                Pick::new('a', "apply stash", apply).hint("keep stash"),
                Pick::new('p', "pop stash", pop).hint("apply + drop"),
                Pick::new('b', "create branch from stash", move |pane, cx| {
                    let stash = branched.clone();
                    pane.prompt(format!("Branch from {}", stash.refname), "branch name", "", move |pane, name, cx| {
                        if name.is_empty() {
                            return;
                        }
                        let params = pane.repo_call(json!({ "refname": stash.refname, "sha": stash.sha, "name": name }));
                        pane.run(format!("branching from {}", stash.refname), "git.stashBranch", params, cx, |pane, _, cx| pane.read_stashes(cx));
                    }, cx);
                }),
                Pick::new('r', "rename stash", move |pane, cx| {
                    let stash = renamed.clone();
                    pane.prompt(format!("Rename {}", stash.refname), "", stash.message.clone(), move |pane, message, cx| {
                        if message.is_empty() || message == stash.message {
                            return;
                        }
                        let params = pane.repo_call(json!({ "refname": stash.refname, "sha": stash.sha, "newMessage": message }));
                        pane.run(format!("renaming {}", stash.refname), "git.stashRename", params, cx, |pane, _, cx| pane.read_stashes(cx));
                    }, cx);
                }),
                Pick::new('d', "drop stash", move |pane, cx| {
                    let drop = drop.clone();
                    pane.ask(format!("Drop {refname}?"), "This permanently deletes the stash entry.", "drop", move |pane, cx| drop(pane, cx), cx);
                })
                .destructive(),
            ],
            cx,
        );
    }

    fn chosen_commit(&self) -> Option<GitCommit> {
        self.commits.get(self.commit).cloned()
    }

    /// What can be done with the commit chosen in the history.
    fn commit_menu(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.chosen_commit() else {
            return;
        };
        let full = if commit.full_hash.is_empty() {
            commit.hash.clone()
        } else {
            commit.full_hash.clone()
        };
        self.menu(
            format!("{} · {}", commit.hash, commit.subject),
            vec![
                Pick::new(
                    'b',
                    "create branch from commit",
                    GitPane::branch_from_commit,
                ),
                Pick::new('r', "reset to this commit", GitPane::reset_menu)
                    .hint("choose soft / mixed / hard"),
                Pick::new('v', "revert this commit", GitPane::revert_commit)
                    .hint("new inverse commit"),
                Pick::new('y', "copy hash", move |_, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(full.clone()))
                }),
            ],
            cx,
        );
    }

    fn branch_from_commit(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.chosen_commit() else {
            return;
        };
        let hash = commit.hash.clone();
        self.prompt(
            format!("Branch from {hash}"),
            "branch name",
            "",
            move |pane, name, cx| {
                if name.is_empty() {
                    return;
                }
                let params = pane.repo_call(json!({ "name": name, "startPoint": hash }));
                pane.run(
                    format!("creating {name} from {hash}"),
                    "git.branchCreate",
                    params,
                    cx,
                    |_, _, _| {},
                );
            },
            cx,
        );
    }

    fn reset_menu(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.chosen_commit() else {
            return;
        };
        let reset = |mode: &'static str, body: &'static str| {
            let hash = commit.hash.clone();
            move |pane: &mut GitPane, cx: &mut Context<GitPane>| {
                let hash = hash.clone();
                pane.confirm(
                    format!("{mode} reset to {hash}?"),
                    body,
                    format!("{mode} reset"),
                    mode == "hard",
                    move |pane, cx| {
                        let params = pane.repo_call(json!({ "rev": hash, "mode": mode }));
                        pane.run(
                            format!("reset --{mode} {hash}"),
                            "git.reset",
                            params,
                            cx,
                            |_, _, _| {},
                        );
                    },
                    cx,
                )
            }
        };
        self.menu(
            format!("Reset to {}", commit.hash),
            vec![
                Pick::new('s', "soft reset", reset("soft", "Moves HEAD to this commit and keeps all later changes staged.")).hint("keep changes staged"),
                Pick::new('m', "mixed reset", reset("mixed", "Moves HEAD to this commit and keeps all later changes in the working tree.")).hint("keep changes unstaged"),
                Pick::new('h', "hard reset", reset("hard", "Moves HEAD to this commit and discards all later changes from the index and working tree."))
                    .hint("discard later changes")
                    .destructive(),
            ],
            cx,
        );
    }

    fn revert_commit(&mut self, cx: &mut Context<Self>) {
        let Some(commit) = self.chosen_commit() else {
            return;
        };
        let hash = commit.hash.clone();
        self.confirm(
            format!("Revert {hash}?"),
            "Creates a new commit that reverses this commit. Existing history is preserved.",
            "revert",
            false,
            move |pane, cx| {
                let params = pane.repo_call(json!({ "rev": hash }));
                pane.run(
                    format!("reverting {hash}"),
                    "git.revert",
                    params,
                    cx,
                    |_, _, _| {},
                );
            },
            cx,
        );
    }

    fn staged_count(&self) -> usize {
        self.files().iter().filter(|file| is_staged(file)).count()
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        let message = self.draft.read(cx).value().trim().to_owned();
        if message.is_empty() || self.busy.is_some() {
            return;
        }
        if self.staged_count() == 0 {
            self.fail("Stage the changes you want to commit first.", cx);
            return;
        }
        let params = json!({ "dir": self.repo, "message": message });
        self.run(
            "Commit staged changes",
            "commitStaged",
            params,
            cx,
            move |pane, answer, cx| {
                if answer.is_ok() && pane.draft.read(cx).value().trim() == message {
                    pane.clear_draft(cx);
                }
            },
        );
    }

    fn clear_draft(&mut self, cx: &mut Context<Self>) {
        self.write(String::new(), cx);
    }

    /// The key the core keeps the repository's commit message under.
    fn draft_key(&self) -> String {
        format!("git:{}", self.repo)
    }

    /// Takes the drafts the core keeps. The commit message follows the core's only while
    /// the person has typed nothing since they last agreed, so their own words are never
    /// overwritten and their cursor never jumps.
    pub fn drafts(&mut self, value: &Value, cx: &mut Context<Self>) {
        let kept = value[self.draft_key().as_str()]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if kept == self.synced {
            return;
        }
        if self.draft.read(cx).value().as_ref() == self.synced {
            self.written = Some(kept.clone());
            cx.notify();
        }
        self.synced = kept;
    }

    /// Keeps what was typed once the person pauses.
    fn typed(&mut self, cx: &mut Context<Self>) {
        self._keeping = cx.spawn(async move |pane, cx| {
            cx.background_executor().timer(DRAFT_PAUSE).await;
            pane.update(cx, |pane, cx| {
                let text = pane.draft.read(cx).value().to_string();
                pane.keep_draft(text);
            })
            .ok();
        });
    }

    /// Puts `text` in the commit box and keeps it as the draft.
    fn write(&mut self, text: String, cx: &mut Context<Self>) {
        self.keep_draft(text.clone());
        self.written = Some(text);
        cx.notify();
    }

    fn keep_draft(&mut self, text: String) {
        if text == self.synced {
            return;
        }
        self.synced = text.clone();
        let _ = self.asks.send(Ask::Call {
            method: "setDraft",
            params: json!({ "key": self.draft_key(), "text": text }),
        });
    }

    fn generate(&mut self, cx: &mut Context<Self>) {
        let before = self.draft.read(cx).value().to_string();
        self.generating = true;
        let (provider, label, model) = match self.writer() {
            Some((kind, label, model)) => (Some(kind), label, Some(model)),
            None => (None, "Hermes".to_owned(), None),
        };
        let params = self.repo_call(json!({ "provider": provider, "model": model }));
        self.run(format!("Generate message with {label}"), "git.aiMessage", params, cx, move |pane, answer, cx| {
            pane.generating = false;
            let Ok(Value::String(message)) = answer else {
                return;
            };
            if message.trim().is_empty() {
                pane.fail("The provider returned an empty message. Your draft was kept.", cx);
            } else if pane.draft.read(cx).value() != before {
                pane.fail("Your draft changed during generation. It was kept; generate again when ready.", cx);
            } else {
                pane.write(message, cx);
            }
        });
    }

    fn fetch(&mut self, cx: &mut Context<Self>) {
        let params = self.repo_call(json!({ "remote": null }));
        self.run(
            "fetching all remotes…",
            "git.fetch",
            params,
            cx,
            |_, _, _| {},
        );
    }

    fn pull(&mut self, cx: &mut Context<Self>) {
        let params = self.repo_call(json!({}));
        self.run("pulling…", "git.pull", params, cx, |_, _, _| {});
    }

    fn push(&mut self, cx: &mut Context<Self>) {
        let params = self.repo_call(json!({}));
        self.run("pushing…", "git.push", params, cx, |_, _, _| {});
    }

    fn pull_request(&mut self, cx: &mut Context<Self>) {
        let params = self.repo_call(json!({}));
        self.run(
            "opening PR…",
            "git.pullRequestUrl",
            params,
            cx,
            |_, answer, cx| {
                if let Ok(Value::String(url)) = answer {
                    cx.open_url(&url);
                }
            },
        );
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keys = &event.keystroke;
        if self.draft.focus_handle(cx).is_focused(window) || self.asking.is_some() {
            return;
        }
        let modifiers = keys.modifiers;
        if modifiers.alt || modifiers.platform {
            return;
        }
        if modifiers.control {
            if keys.key == "p" {
                self.pull_request(cx);
                cx.stop_propagation();
            }
            return;
        }
        if self.busy.is_some() {
            return;
        }
        let typed = match keys.key.as_str() {
            named @ ("enter" | "space" | "up" | "down" | "escape") => named,
            key => keys.key_char.as_deref().unwrap_or(key),
        };
        let history = self.page == Page::History;
        match typed {
            "/" => self.find(window, cx),
            "escape" if self.range.is_some() => {
                self.range = None;
                cx.notify();
            }
            "escape" => {
                if !self.unfind(window, cx) {
                    return;
                }
            }
            "1" => self.turn(Page::Changes, cx),
            "2" => self.turn(Page::History, cx),
            "3" => self.turn(Page::Branches, cx),
            "F" => self.fetch(cx),
            "P" => self.push(cx),
            "p" => self.pull(cx),
            "r" if !history => self.refresh(cx),
            _ if self.page == Page::Branches && self.branch_key(typed, cx) => {}
            _ if self.page == Page::Branches => return,
            "h" if history => self.turn(Page::Changes, cx),
            "h" => self.turn(Page::History, cx),
            "j" | "down" if history => self.step_commit(1, cx),
            "k" | "up" if history => self.step_commit(-1, cx),
            " " | "space" | "enter" if history => self.commit_menu(cx),
            "b" if history => self.branch_from_commit(cx),
            "r" if history => self.reset_menu(cx),
            "v" if history => self.revert_commit(cx),
            _ if history => return,
            "j" => self.step(1, cx),
            "k" => self.step(-1, cx),
            "down" => self.step(1, cx),
            "up" => self.step(-1, cx),
            " " | "space" => self.toggle_stage(cx),
            "a" if self.files().iter().any(has_unstaged) => self.stage_all(cx),
            "a" => self.unstage_all(cx),
            "c" => {
                let draft = self.draft.clone();
                draft.update(cx, |draft, cx| draft.focus(window, cx));
            }
            "C" => self.commit(cx),
            "g" => self.generate(cx),
            "d" => self.discard_menu(cx),
            "v" => {
                self.range = match self.range {
                    Some(_) => None,
                    None => Some(self.selected),
                };
                cx.notify();
            }
            "s" => self.stash_menu(cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for GitPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(written) = self.written.take() {
            self.draft
                .update(cx, |draft, cx| draft.set_value(written, window, cx));
        }
        let filter = self.filter_field(window, cx);
        let reviewed = review(self, window, cx);
        let pushed =
            matches!(&self.overview, Some(Ok(overview)) if overview.status.upstream.is_some());
        let card = if self.page == Page::Branches {
            self.chosen_branch()
                .and_then(|entry| branch_card(self, &entry, cx))
        } else {
            self.review.commit().and_then(|hash| {
                let commit = self.log().iter().find(|commit| commit.full_hash == hash)?;
                Some(commit_card(commit, pushed, cx))
            })
        }
        .map(|card| card.mx(px(-8.)).mt(px(-8.)).mb(px(8.)));
        let asking = self.asked(window, cx);
        div()
            .id("git-pane")
            .key_context("GitPane")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .gap(px(8.))
            .font_family(theme::UI)
            .on_key_down(cx.listener(Self::key))
            .child(side(self, filter, cx))
            .child(
                ui::pane(cx)
                    .flex_1()
                    .min_w_0()
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .children(card)
                    .child(div().flex_1().min_h_0().child(reviewed)),
            )
            .children(asking)
    }
}

/// The side column: the branch with its sync actions, then the changes and the commit box.
fn side(
    pane: &mut GitPane,
    filter: Option<Entity<InputState>>,
    cx: &mut Context<GitPane>,
) -> impl IntoElement {
    let count = (pane.page == Page::History).then(|| pane.log().len());
    let filter = filter.map(|field| filter_row(&field, count, cx));
    ui::panel(cx)
        .w(relative(0.3))
        .min_w(px(320.))
        .max_w(px(400.))
        .h_full()
        .flex_none()
        .child(head(pane, cx))
        .map(|side| match pane.page {
            Page::Changes => side.child(changes(pane, filter, cx)),
            Page::History => side.children(filter).child(history_list(pane, cx)),
            Page::Branches => side.children(filter).child(branches_page(pane, cx)),
        })
        .child(pages(pane, cx))
}

/// The icons at the side column's foot that turn its pages.
fn pages(pane: &GitPane, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let page = |id: &'static str, icon: Icon, label: &'static str, which: Page| {
        ui::IconButton::new(id, icon, label)
            .width(32.)
            .glyph(14.)
            .pressed(pane.page == which)
            .on_click(cx.listener(move |pane, _, _, cx| pane.turn(which, cx)))
    };
    div()
        .flex_none()
        .flex()
        .justify_center()
        .gap(px(4.))
        .px(px(8.))
        .py(px(4.))
        .border_t_1()
        .border_color(t.border)
        .child(page(
            "git-page-changes",
            Icon::File,
            "Changes — 1",
            Page::Changes,
        ))
        .child(page(
            "git-page-history",
            Icon::Commit,
            "History — 2",
            Page::History,
        ))
        .child(page(
            "git-page-branches",
            Icon::Git,
            "Branches — 3",
            Page::Branches,
        ))
}

/// The History page: the branch's commits on the graph.
fn history_list(pane: &mut GitPane, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let view = cx.entity();
    let inks = Inks::new(&t);
    let empty = pane.hashes.is_empty();
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .when(empty, |page| {
            page.child(
                div()
                    .p(px(20.))
                    .text_size(px(12.))
                    .text_color(t.text_tertiary)
                    .child("No commits on this branch yet."),
            )
        })
        .child(
            list(pane.commit_rows.clone(), move |index, _, cx| {
                let cx = &*cx;
                let pane = view.read(cx);
                commit_row(&view, pane, index, &inks, cx)
            })
            .flex_1()
            .px(px(6.))
            .py(px(4.)),
        )
}

fn head(pane: &mut GitPane, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let status = match &pane.overview {
        Some(Ok(overview)) => Some(&overview.status),
        _ => None,
    };
    let branch = match (&pane.overview, status) {
        (None, _) => "…".to_owned(),
        (_, Some(status)) if !status.branch.is_empty() => status.branch.clone(),
        _ => "detached".to_owned(),
    };
    let (ahead, behind) = status.map_or((0, 0), |status| (status.ahead, status.behind));
    let upstream = status.is_some_and(|status| status.upstream.is_some());
    let failed = matches!(pane.overview, Some(Err(_)));
    let busy = pane.failed.clone().or_else(|| pane.busy.clone());
    let count = |value: i32, colour: Hsla, arrow: &str| {
        (value > 0).then(|| {
            div()
                .flex_none()
                .font_family(theme::MONO)
                .text_size(px(11.))
                .text_color(colour)
                .child(format!("{arrow}{value}"))
        })
    };
    let sync = |id: &'static str,
                icon: Icon,
                label: &'static str,
                tip: String,
                number: i32,
                primary: bool,
                act: fn(&mut GitPane, &mut Context<GitPane>),
                cx: &mut Context<GitPane>| {
        BaseButton::new(id)
            .accessibility_label(label)
            .tooltip(ui::tooltip(tip))
            .h(px(30.))
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .map(|button| {
                if primary {
                    button.bg(t.accent).text_color(t.on_accent)
                } else {
                    button
                        .text_color(t.text_secondary)
                        .hover(move |style| style.bg(t.surface_hover).text_color(t.text_primary))
                }
            })
            .on_click(cx.listener(move |pane, _, _, cx| act(pane, cx)))
            .child(ui::icon(icon, 12.).text_color(if primary {
                t.on_accent
            } else {
                t.text_tertiary
            }))
            .child(label)
            .when(number > 0, |button| {
                button.child(
                    div()
                        .text_color(if primary { t.on_accent } else { t.accent })
                        .child(number.to_string()),
                )
            })
    };
    let plural = |count: i32| if count == 1 { "" } else { "s" };
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .flex_none()
        .pt(px(8.))
        .border_b_1()
        .border_color(t.border)
        .child(
            div()
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(4.))
                .min_w_0()
                .child(branch_switch(pane, branch, cx))
                .children(count(ahead, t.live, "↑"))
                .children(count(behind, t.warn, "↓"))
                .child(div().flex_1())
                .when(failed, |title| {
                    title.child(
                        div()
                            .h(px(18.))
                            .px(px(6.))
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(t.danger.opacity(0.32))
                            .rounded(px(4.))
                            .font_family(theme::MONO)
                            .text_size(px(11.))
                            .text_color(t.danger)
                            .child("git error"),
                    )
                })
                .children(busy.map(|busy| {
                    let error = busy.starts_with('✗');
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(if error { t.danger } else { t.warn })
                        .child(busy)
                }))
                .child(more_menu(pane, cx)),
        )
        .child(
            div()
                .flex()
                .border_t_1()
                .border_color(t.border)
                .child(sync(
                    "git-fetch",
                    Icon::Fetch,
                    "Fetch",
                    "Fetch all remotes (F)".into(),
                    0,
                    false,
                    GitPane::fetch,
                    cx,
                ))
                .child(div().w(px(1.)).bg(t.border))
                .child(sync(
                    "git-pull",
                    Icon::Pull,
                    "Pull",
                    if behind > 0 {
                        format!("Pull {behind} commit{} (p)", plural(behind))
                    } else {
                        "Pull (p)".into()
                    },
                    behind,
                    false,
                    GitPane::pull,
                    cx,
                ))
                .child(div().w(px(1.)).bg(t.border))
                .child(sync(
                    "git-push",
                    Icon::Push,
                    if upstream { "Push" } else { "Publish" },
                    if !upstream {
                        "Publish branch and set upstream (P)".into()
                    } else if ahead > 0 {
                        format!("Push {ahead} commit{} (P)", plural(ahead))
                    } else {
                        "Push (P)".into()
                    },
                    ahead,
                    ahead > 0 || !upstream,
                    GitPane::push,
                    cx,
                ))
                .child(div().w(px(1.)).bg(t.border))
                .child(sync(
                    "git-pr",
                    Icon::PullRequest,
                    "PR",
                    "Open pull request (⌃P)".into(),
                    0,
                    false,
                    GitPane::pull_request,
                    cx,
                )),
        )
}

/// A line of one of the head's menus, doing `act` to the pane and closing the menu.
fn menu_line(
    view: &Entity<GitPane>,
    popover: &Entity<PopoverState>,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    icon: Icon,
    act: impl Fn(&mut GitPane, &mut Context<GitPane>) + 'static,
) -> MenuItem {
    let (view, popover) = (view.clone(), popover.clone());
    MenuItem::new(ElementId::Name(id.into()), label)
        .icon(icon)
        .on_click(move |_, window, cx| {
            view.update(cx, |pane, cx| act(pane, cx));
            popover.update(cx, |popover, cx| popover.dismiss(window, cx));
        })
}

/// The branch in the head, which opens a menu to make, find or switch to a branch.
fn branch_switch(pane: &GitPane, branch: String, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let view = cx.entity();
    let branches: Vec<(String, bool, Option<String>)> = match &pane.overview {
        Some(Ok(overview)) => overview
            .branches
            .iter()
            .map(|branch| (branch.name.clone(), branch.current, branch.upstream.clone()))
            .collect(),
        _ => Vec::new(),
    };
    Popover::new("git-branch-switch")
        .anchor(Anchor::TopLeft)
        .offset(px(4.))
        .trigger(
            BaseButton::new("git-switch")
                .accessibility_label("Switch branch")
                .tooltip(ui::tooltip("Switch branch"))
                .h(px(26.))
                .px(px(6.))
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(RADIUS_2)
                .text_color(t.text_primary)
                .hover(move |style| style.bg(t.surface_hover))
                .child(ui::icon(Icon::Git, 13.).text_color(t.text_tertiary))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(branch),
                )
                .child(
                    ui::icon(Icon::Chevron, 9.)
                        .text_color(t.text_tertiary)
                        .with_transformation(Transformation::rotate(radians(FRAC_PI_2))),
                ),
        )
        .content(move |_, _, cx| {
            let popover = cx.entity();
            let mut menu = Menu::new()
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "switch-new",
                        "New branch…",
                        Icon::Plus,
                        |pane, cx| pane.new_branch(None, cx),
                    )
                    .hint("N"),
                )
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "switch-by-name",
                        "Check out by name…",
                        Icon::Checkout,
                        |pane, cx| pane.checkout_by_name(cx),
                    )
                    .hint("c"),
                )
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "switch-all",
                        "Show all branches",
                        Icon::Git,
                        |pane, cx| pane.turn(Page::Branches, cx),
                    )
                    .hint("3"),
                )
                .separator(cx);
            for (name, current, upstream) in &branches {
                let checked = name.clone();
                let mut line = menu_line(
                    &view,
                    &popover,
                    format!("switch:{name}"),
                    name.clone(),
                    Icon::Git,
                    move |pane, cx| pane.checkout(checked.clone(), cx),
                )
                .disabled(*current);
                if *current {
                    line = line.hint("current");
                } else if let Some(upstream) = upstream {
                    line = line.hint(upstream.clone());
                }
                menu = menu.child(line);
            }
            menu
        })
}

/// The head's menu of everything else: remotes, stashes, and the pane's own tools.
fn more_menu(pane: &GitPane, cx: &mut Context<GitPane>) -> impl IntoElement {
    let view = cx.entity();
    let changed = !pane.files().is_empty();
    let unstaged = pane.files().iter().any(has_unstaged);
    let stashes = pane.stashes.len();
    Popover::new("git-more")
        .anchor(Anchor::TopRight)
        .offset(px(4.))
        .trigger(
            ui::IconButton::new("git-more-button", Icon::More, "Remotes, stashes and more")
                .glyph(13.),
        )
        .content(move |_, _, cx| {
            let popover = cx.entity();
            Menu::new()
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "more-fetch",
                        "Fetch all remotes",
                        Icon::Fetch,
                        GitPane::fetch,
                    )
                    .hint("F"),
                )
                .child(menu_line(
                    &view,
                    &popover,
                    "more-remote",
                    "Add remote…",
                    Icon::Plus,
                    GitPane::add_remote,
                ))
                .separator(cx)
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "more-stash",
                        "Stash changes…",
                        Icon::Stack,
                        GitPane::stash_menu,
                    )
                    .hint("s")
                    .disabled(!changed),
                )
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "more-discard",
                        "Discard unstaged changes…",
                        Icon::Discard,
                        GitPane::discard_all,
                    )
                    .disabled(!unstaged),
                )
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "more-stashes",
                        if stashes > 0 {
                            format!("Stashes ({stashes})…")
                        } else {
                            "No stashes".to_owned()
                        },
                        Icon::Stack,
                        GitPane::stashes_menu,
                    )
                    .disabled(stashes == 0),
                )
                .separator(cx)
                .child(
                    menu_line(
                        &view,
                        &popover,
                        "more-refresh",
                        "Refresh",
                        Icon::Refresh,
                        GitPane::refresh,
                    )
                    .hint("r"),
                )
        })
}

/// A page's filter: what to find, and how many there are when the page counts them.
fn filter_row(field: &Entity<InputState>, count: Option<usize>, cx: &App) -> Div {
    let t = Tokens::get(cx);
    div()
        .flex_none()
        .h(px(34.))
        .mb(px(4.))
        .px(px(12.))
        .flex()
        .items_center()
        .gap(px(8.))
        .border_b_1()
        .border_color(t.border)
        .text_size(px(12.))
        .child(ui::icon(Icon::Search, 12.).text_color(t.text_tertiary))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(field).appearance(false)),
        )
        .children(count.filter(|count| *count > 0).map(|count| {
            div()
                .flex_none()
                .font_family(theme::MONO)
                .text_size(px(11.))
                .text_color(t.text_tertiary)
                .child(count.to_string())
        }))
}

fn changes(pane: &mut GitPane, filter: Option<Div>, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let empty = pane.entries.is_empty();
    let query = pane.filters[Page::Changes as usize].query.clone();
    let note = match &pane.overview {
        _ if !empty => None,
        None => Some((
            "Loading changed files".to_owned(),
            String::new(),
            t.text_tertiary,
        )),
        Some(Err(error)) => Some(("Git error".to_owned(), error.clone(), t.danger)),
        Some(Ok(_)) if !query.is_empty() => Some((
            "No matches".to_owned(),
            format!("Nothing matches \"{query}\"."),
            t.text_tertiary,
        )),
        Some(Ok(_)) => Some((
            "Nothing to commit".to_owned(),
            "The working tree is clean.".to_owned(),
            t.text_tertiary,
        )),
    };
    let view = cx.entity();
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .pb(px(6.))
        .child(composer(pane, cx))
        .children(filter)
        .children(note.map(|(title, message, colour)| {
            div()
                .p(px(20.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .text_size(px(12.))
                .child(div().text_color(colour).child(title))
                .child(div().text_color(t.text_tertiary).child(message))
        }))
        .child(
            list(pane.rows.clone(), move |index, window, cx| {
                let cx = &*cx;
                let pane = view.read(cx);
                match pane.entries.get(index).cloned() {
                    Some(entry) => entry_row(&view, pane, entry, index, window, cx),
                    None => div().into_any_element(),
                }
            })
            .flex_1()
            .px(px(6.)),
        )
}

/// The 28px button beside the message's writer that picks the agent and model.
fn writer_picker(pane: &GitPane, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let view = cx.entity();
    let writers = pane.writers.clone();
    let current = pane.writer().map(|(kind, _, model)| (kind, model));
    Popover::new("git-writer")
        .anchor(Anchor::TopLeft)
        .offset(px(4.))
        .trigger(
            BaseButton::new("git-writer-pick")
                .accessibility_label("Pick the agent and model")
                .tooltip(ui::tooltip("Pick the agent and model"))
                .size(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(t.border)
                .hover(move |style| style.bg(t.surface_hover).border_color(t.border_strong))
                .child(
                    ui::icon(Icon::Chevron, 9.)
                        .text_color(t.text_tertiary)
                        .with_transformation(Transformation::rotate(radians(FRAC_PI_2))),
                ),
        )
        .content(move |_, _, cx| {
            let popover = cx.entity();
            let mut menu = Menu::new();
            for (index, (kind, label, models)) in writers.iter().enumerate() {
                if index > 0 {
                    menu = menu.separator(cx);
                }
                for model in models {
                    let picked = Some((kind.clone(), model.clone()));
                    let checked = picked == current;
                    let (view, popover, picked) = (view.clone(), popover.clone(), picked.clone());
                    menu = menu.child(
                        MenuItem::new(
                            ElementId::Name(format!("writer:{kind}:{model}").into()),
                            format!("{label} · {model}"),
                        )
                        .mark(ui::mark(kind, 13.))
                        .checked(checked)
                        .on_click(move |_, window, cx| {
                            let picked = picked.clone();
                            view.update(cx, |pane, cx| {
                                pane.writer = picked;
                                cx.notify();
                            });
                            popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                        }),
                    );
                }
            }
            menu
        })
}

fn composer(pane: &mut GitPane, cx: &mut Context<GitPane>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let writer_label = match pane.writer() {
        Some((_, label, model)) => {
            let model = model
                .split_once('/')
                .map_or(model.as_str(), |(_, model)| model);
            format!("{label} · {model}")
        }
        None => "Write message".to_owned(),
    };
    let staged = pane.staged_count();
    let message = !pane.draft.read(cx).value().trim().is_empty();
    let can_commit = pane.busy.is_none() && staged > 0 && message;
    let label = if staged > 0 {
        format!("Commit {staged} file{}", if staged == 1 { "" } else { "s" })
    } else {
        "Commit".to_owned()
    };
    let tip = if staged > 0 {
        format!("{label} (⌘⏎)")
    } else {
        "Stage files to commit them".to_owned()
    };
    div()
        .flex_none()
        .mb(px(6.))
        .border_b_1()
        .border_color(t.border)
        .capture_key_down(cx.listener(|pane, event: &KeyDownEvent, window, cx| {
            let keys = &event.keystroke;
            if keys.key == "enter" && (keys.modifiers.platform || keys.modifiers.control) {
                pane.commit(cx);
                cx.stop_propagation();
            } else if keys.key == "escape" {
                window.focus(&pane.focus, cx);
                cx.stop_propagation();
            }
        }))
        .child(
            div()
                .px(px(10.))
                .py(px(9.))
                .text_size(px(13.))
                .line_height(px(19.5))
                .child(
                    Textarea::new(&pane.draft)
                        .appearance(false)
                        .text_size(px(13.)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .px(px(10.))
                .pb(px(8.))
                .child(
                    BaseButton::new("git-generate")
                        .accessibility_label("Write the message from the staged changes")
                        .tooltip(ui::tooltip("Write the message from the staged changes (g)"))
                        .h(px(28.))
                        .px(px(8.))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .border_1()
                        .border_color(t.border)
                        .text_size(px(12.))
                        .text_color(t.text_secondary)
                        .hover(move |style| {
                            style
                                .bg(t.surface_hover)
                                .border_color(t.border_strong)
                                .text_color(t.text_primary)
                        })
                        .on_click(cx.listener(|pane, _, _, cx| pane.generate(cx)))
                        .child(ui::icon(Icon::Sparkle, 11.).text_color(t.cmd))
                        .child(div().min_w_0().truncate().child(if pane.generating {
                            "writing…".to_owned()
                        } else {
                            writer_label
                        })),
                )
                .child(writer_picker(pane, cx))
                .child(
                    BaseButton::new("git-commit")
                        .accessibility_label(SharedString::from(label.clone()))
                        .tooltip(ui::tooltip(tip))
                        .disabled(!can_commit)
                        .ml_auto()
                        .h(px(28.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .border_1()
                        .border_color(t.accent)
                        .bg(t.accent)
                        .text_color(t.on_accent)
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .when(!can_commit, |button| button.opacity(0.45))
                        .on_click(cx.listener(|pane, _, _, cx| pane.commit(cx)))
                        .child(label),
                ),
        )
}

/// A glyph a row shows while the pointer is on it.
fn row_button(
    id: impl Into<SharedString>,
    icon: Icon,
    glyph: f32,
    label: impl Into<SharedString>,
    cx: &App,
) -> BaseButton {
    let t = Tokens::get(cx);
    let (rest, lit, wash) = (t.text_tertiary, t.text_primary, t.surface_active);
    let label = label.into();
    BaseButton::new(id.into())
        .group(ROW_ACTION)
        .accessibility_label(label.clone())
        .tooltip(ui::tooltip(label))
        .size(px(22.))
        .flex()
        .items_center()
        .justify_center()
        .hover(move |style| style.bg(wash))
        .child(
            ui::icon(icon, glyph)
                .text_color(rest)
                .group_hover(ROW_ACTION, move |style| style.text_color(lit)),
        )
}

/// The hover group a row's glyph button puts its glyph in, so the glyph lights with it.
const ROW_ACTION: &str = "git-row-action";

/// The hover actions of a changed file, as its list row and its review card show them:
/// open it, discard its unstaged change, and move it to the other group.
pub fn file_actions(
    view: &Entity<GitPane>,
    file: &GitFile,
    side: Option<Side>,
    cx: &App,
) -> Vec<AnyElement> {
    let action = |id: String, icon: Icon, label: String, act: Act| {
        let view = view.clone();
        row_button(id, icon, 12., label, cx)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                let act = act.clone();
                view.update(cx, |pane, cx| act(pane, cx));
            })
            .into_any_element()
    };
    let unstaged = side.map_or(has_unstaged(file), |side| side == Side::Unstaged);
    let staged = side.map_or(is_staged(file), |side| side == Side::Staged);
    let key = format!("{}:{:?}", file.path, side);
    let mut actions = Vec::new();
    if side.is_some() {
        let target = OpenAt {
            path: Path::new(&view.read(cx).repo).join(&file.path),
            line: 0,
            column: None,
        };
        actions.push(
            row_button(
                format!("open:{key}"),
                Icon::File,
                12.,
                format!("Open {}", file.path),
                cx,
            )
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                window.dispatch_action(target.boxed_clone(), cx);
            })
            .into_any_element(),
        );
    }
    if unstaged {
        let discarded = file.clone();
        actions.push(action(
            format!("discard:{key}"),
            Icon::Discard,
            "Discard changes".into(),
            Rc::new(move |pane, cx| pane.discard(&discarded, cx)),
        ));
    }
    if staged {
        let moved = file.clone();
        actions.push(action(
            format!("unstage:{key}"),
            Icon::Minus,
            format!("Unstage {}", file.path),
            Rc::new(move |pane, cx| pane.unstage(&moved, cx)),
        ));
    }
    if unstaged {
        let moved = file.clone();
        actions.push(action(
            format!("stage:{key}"),
            Icon::Plus,
            format!("Stage {}", file.path),
            Rc::new(move |pane, cx| pane.stage(&moved, cx)),
        ));
    }
    actions
}

fn entry_row(
    view: &Entity<GitPane>,
    pane: &GitPane,
    entry: Entry,
    index: usize,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    match entry {
        Entry::Group { side, count } => group_row(view, pane, side, count, index, cx),
        Entry::File { side, file } => {
            let first = !matches!(
                pane.entries.get(index.wrapping_sub(1)),
                Some(Entry::File { .. })
            );
            let last = !matches!(pane.entries.get(index + 1), Some(Entry::File { .. }));
            let chosen = index == pane.selected && pane.focus.contains_focused(window, cx);
            let ranged = pane.range.is_some_and(|anchor| {
                (anchor.min(pane.selected)..=anchor.max(pane.selected)).contains(&index)
            });
            let (dir, base) = match file.path.trim_end_matches('/').rsplit_once('/') {
                Some((dir, base)) => (Some(dir.to_owned()), base.to_owned()),
                None => (None, file.path.clone()),
            };
            let code = match side {
                Side::Staged => file.index.as_str(),
                Side::Unstaged => file.worktree.as_str(),
            };
            let letter = decoration(code, &t).map(|(letter, colour, label)| {
                let colour = if side == Side::Unstaged && letter == "M" {
                    t.git_renamed
                } else {
                    colour
                };
                div()
                    .id(SharedString::from(format!("status:{}:{side:?}", file.path)))
                    .flex_none()
                    .w(px(14.))
                    .text_center()
                    .font_family(theme::MONO)
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colour)
                    .tooltip(ui::tooltip(label))
                    .child(letter)
            });
            let glyph = match crate::file_icons::icon(&base, cx) {
                Some(icon) => img(icon).size(px(14.)).flex_none().into_any_element(),
                None => ui::icon(Icon::File, 14.)
                    .text_color(t.text_tertiary)
                    .into_any_element(),
            };
            let key = SharedString::from(format!("row:{}:{side:?}", file.path));
            let selecting = view.clone();
            div()
                .id(SharedString::from(format!(
                    "git-file:{}:{side:?}",
                    file.path
                )))
                .on_hover(hover(view, key.clone()))
                .relative()
                .w_full()
                .h(px(ROW_HEIGHT))
                .pl(px(18.))
                .pr(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .border_1()
                .border_color(if chosen {
                    t.border_selected
                } else {
                    gpui_kit::transparent_black()
                })
                .rounded(RADIUS_2)
                .text_size(px(13.))
                .text_color(t.text_secondary)
                .when(ranged && !chosen, |row| row.bg(t.warn.opacity(0.1)))
                .hover(move |row| row.bg(t.surface_hover).text_color(t.text_primary))
                .on_click(move |_, window, cx| {
                    selecting.update(cx, |pane, cx| {
                        window.focus(&pane.focus, cx);
                        pane.select(index, cx);
                    })
                })
                .children(tree_lines(first, last, &t))
                .child(glyph)
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(div().flex_none().child(base))
                        .children(dir.map(|dir| {
                            div()
                                .ml(px(6.))
                                .min_w_0()
                                .truncate()
                                .text_size(px(12.))
                                .text_color(t.text_tertiary)
                                .child(dir)
                        })),
                )
                .when(pane.hovered.as_ref() == Some(&key), |row| {
                    row.child(div().flex().gap(px(2.)).children(file_actions(
                        view,
                        &file,
                        Some(side),
                        cx,
                    )))
                })
                .children(letter)
                .into_any_element()
        }
    }
}

/// The lines a group's row hangs from, as the project tree's children do: a spine from
/// under the group's chevron, a tick to the row, and an elbow at the last.
fn tree_lines(first: bool, last: bool, t: &Tokens) -> Vec<Div> {
    let top = px(if first { -13. } else { -1. });
    if last {
        return vec![div()
            .absolute()
            .left(px(11.))
            .top(top)
            .w(px(6.))
            .h(px(if first { 27. } else { 14.5 }))
            .border_l_1()
            .border_b_1()
            .border_color(t.tree_spine)
            .rounded_bl(px(4.))];
    }
    vec![
        div()
            .absolute()
            .left(px(11.))
            .top(top)
            .bottom(px(-1.))
            .w(px(1.))
            .bg(t.tree_spine),
        div()
            .absolute()
            .left(px(12.))
            .top(px(12.))
            .w(px(5.))
            .h(px(1.))
            .bg(t.tree_tick),
    ]
}

fn group_row(
    view: &Entity<GitPane>,
    pane: &GitPane,
    side: Side,
    count: usize,
    index: usize,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let folded = pane.folded.contains(&side);
    let staged = side == Side::Staged;
    let key = SharedString::from(format!("group:{side:?}"));
    let button =
        |id: &str, icon: Icon, label: &str, act: fn(&mut GitPane, &mut Context<GitPane>)| {
            let view = view.clone();
            row_button(format!("{id}-{side:?}"), icon, 13., label.to_owned(), cx).on_click(
                move |_, _, cx| {
                    cx.stop_propagation();
                    view.update(cx, act);
                },
            )
        };
    let folding = view.clone();
    div()
        .when(index > 0, |row| row.pt(px(6.)))
        .child(
            div()
                .id(SharedString::from(format!("git-group:{side:?}")))
                .on_hover(hover(view, key.clone()))
                .h(px(ROW_HEIGHT))
                .pl(px(2.))
                .pr(px(4.))
                .flex()
                .items_center()
                .gap(px(6.))
                .rounded(RADIUS_2)
                .hover(move |row| row.bg(t.surface_hover))
                .child(
                    div()
                        .id(SharedString::from(format!("git-group-toggle:{side:?}")))
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .px(px(4.))
                        .items_center()
                        .gap(px(4.))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| {
                            folding.update(cx, |pane, cx| {
                                if !pane.folded.remove(&side) {
                                    pane.folded.insert(side);
                                }
                                pane.list(cx);
                            })
                        })
                        .child(ui::icon(Icon::Chevron, 13.).text_color(t.tree_spine).when(
                            !folded,
                            |chevron| {
                                chevron
                                    .with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                            },
                        ))
                        .child(
                            ui::icon(if staged { Icon::FilePlus } else { Icon::File }, 13.)
                                .text_color(t.text_tertiary),
                        )
                        .child(
                            ui::text(
                                if staged { "Staged Changes" } else { "Changes" },
                                12.,
                                FontWeight::MEDIUM,
                            )
                            .text_color(t.text_primary),
                        ),
                )
                .when(pane.hovered.as_ref() == Some(&key), |row| {
                    row.child(
                        div()
                            .flex()
                            .gap(px(2.))
                            .when(index == 0, |actions| {
                                actions.child(button(
                                    "git-fold-review",
                                    if pane.review_folded {
                                        Icon::ExpandAll
                                    } else {
                                        Icon::CollapseAll
                                    },
                                    if pane.review_folded {
                                        "Expand all diffs"
                                    } else {
                                        "Collapse all diffs"
                                    },
                                    |pane, cx| {
                                        pane.review_folded = !pane.review_folded;
                                        pane.review.fold_all(pane.review_folded);
                                        cx.notify();
                                    },
                                ))
                            })
                            .when(index == 0, |actions| {
                                actions.child(button(
                                    "git-stash",
                                    Icon::Stash,
                                    "Stash",
                                    GitPane::stash_menu,
                                ))
                            })
                            .when(!staged, |actions| {
                                actions.child(button(
                                    "git-discard-all",
                                    Icon::Discard,
                                    "Discard all",
                                    GitPane::discard_all,
                                ))
                            })
                            .map(|actions| {
                                if staged {
                                    actions.child(button(
                                        "git-unstage-all",
                                        Icon::Minus,
                                        "Unstage all",
                                        GitPane::unstage_all,
                                    ))
                                } else {
                                    actions.child(button(
                                        "git-stage-all",
                                        Icon::Plus,
                                        "Stage all",
                                        GitPane::stage_all,
                                    ))
                                }
                            }),
                    )
                })
                .child(
                    div()
                        .flex_none()
                        .min_w(px(18.))
                        .h(px(18.))
                        .px(px(5.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(t.surface_chosen)
                        .text_size(px(11.))
                        .text_color(t.text_secondary)
                        .child(count.to_string()),
                ),
        )
        .into_any_element()
}

/// Keeps the pane's `hovered` on `key` while the pointer is inside it.
pub fn hover(
    view: &Entity<GitPane>,
    key: SharedString,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let view = view.clone();
    move |inside, _, cx| {
        view.update(cx, |pane, cx| {
            if *inside {
                pane.hovered = Some(key.clone());
            } else if pane.hovered.as_ref() == Some(&key) {
                pane.hovered = None;
            }
            cx.notify();
        })
    }
}
