//! A project's code host as the Git tab shows it in place of the local workbench: its
//! pull requests, CI runs, issues and releases, as the core lists them.

mod pull;
mod pulls;
mod thread;

use std::future::Future;
use std::rc::Rc;

use gpui_kit::component::notification::Notification;
use gpui_kit::{
    div, px, AnyElement, App, Context, Div, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sikemux_wire::codehost::{CheckTally, Label, Outcome, PullState};
use sikemux_wire::protocol::ProjectCodeHost;
use tokio::sync::mpsc::UnboundedSender;

use crate::core::{request, Ask, Watch};
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui;
use crate::workbench::Workbench;

/// What the Git tab shows: the local workbench, or one of the code host's areas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Area {
    #[default]
    Local,
    Pulls,
    Actions,
    Issues,
    Releases,
}

impl Area {
    /// The area a code host's section names, as the core lists them.
    pub fn of(section: &str) -> Option<Self> {
        Some(match section {
            "pulls" => Self::Pulls,
            "actions" => Self::Actions,
            "issues" => Self::Issues,
            "releases" => Self::Releases,
            _ => return None,
        })
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Local => Icon::Git,
            Self::Pulls => Icon::PullRequest,
            Self::Actions => Icon::Actions,
            Self::Issues => Icon::Issues,
            Self::Releases => Icon::Releases,
        }
    }

    pub fn label(self, host: &ProjectCodeHost) -> SharedString {
        match self {
            Self::Local => "Git".into(),
            Self::Pulls => "Pull requests".into(),
            Self::Actions => host.ci_name.clone().into(),
            Self::Issues => "Issues".into(),
            Self::Releases => "Releases".into(),
        }
    }
}

/// Brings the project's Git tab to the front showing `area`.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(name = "git.openArea", no_json)]
pub struct OpenArea {
    pub area: Area,
}

/// What a code-host list answered: its value, or why the host could not give it.
type Read<T> = Option<Result<T, SharedString>>;

/// What the person agreeing does.
type Then = Rc<dyn Fn(&mut HostArea, &mut Context<HostArea>)>;

/// What the person is asked before a change the host cannot take back.
pub struct Asking {
    title: SharedString,
    body: SharedString,
    yes: SharedString,
    destructive: bool,
    then: Then,
}

impl Asking {
    pub fn new(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        yes: impl Into<SharedString>,
        destructive: bool,
        then: impl Fn(&mut HostArea, &mut Context<HostArea>) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            yes: yes.into(),
            destructive,
            then: Rc::new(then),
        }
    }
}

/// How something the person did went.
pub enum Said {
    Done(SharedString),
    Failed(SharedString),
}

/// The code host's areas for one project.
pub struct HostArea {
    asks: UnboundedSender<Ask>,
    bench: WeakEntity<Workbench>,
    /// The project's folder, which the core's lists are asked by.
    pub project: String,
    host: ProjectCodeHost,
    /// The account the project picked on its host, else the host's default.
    account: Option<String>,
    area: Area,
    pulls: pulls::Pulls,
    pull: Option<pull::Opened>,
    asking: Option<Asking>,
}

impl HostArea {
    pub fn new(
        asks: UnboundedSender<Ask>,
        bench: WeakEntity<Workbench>,
        project: String,
        host: ProjectCodeHost,
        account: Option<String>,
        branch: Option<(String, bool)>,
    ) -> Self {
        Self {
            asks,
            bench,
            project,
            host,
            account,
            area: Area::Local,
            pulls: pulls::Pulls::new(branch),
            pull: None,
            asking: None,
        }
    }

    /// Shows `area`, asking the core for its lists the first time.
    pub fn show(&mut self, area: Area, cx: &mut Context<Self>) {
        if self.area != area {
            self.area = area;
            self.watch();
            cx.notify();
        }
    }

    /// Takes what the project's repository on its host now is, and where its branch stands.
    pub fn project_moved(
        &mut self,
        host: &ProjectCodeHost,
        branch: Option<(String, bool)>,
        cx: &mut Context<Self>,
    ) {
        if &self.host != host || self.pulls.branch != branch {
            self.host = host.clone();
            self.pulls.branch = branch;
            cx.notify();
        }
    }

    /// Takes a watched list's answer, under the key it was watched by.
    pub fn read(&mut self, key: &str, value: &Value, cx: &mut Context<Self>) {
        match key {
            pulls::KEY => self.pulls.list = Some(answer(value)),
            pull::VIEW => {
                if let Some(opened) = &mut self.pull {
                    opened.take_view(Some(answer(value)));
                }
            }
            pull::FILES => {
                if let Some(opened) = &mut self.pull {
                    opened.take_files(answer(value));
                }
            }
            _ => return,
        }
        cx.notify();
    }

    fn watch(&self) {
        if self.area != Area::Pulls {
            return;
        }
        match &self.pull {
            Some(opened) => {
                let params = json!({ "project": self.project, "number": opened.number });
                self.ask_watch(pull::VIEW, "codeHostPull", params.clone());
                self.ask_watch(pull::FILES, "codeHostPullFiles", params);
            }
            None => self.ask_watch(
                pulls::KEY,
                "codeHostPulls",
                json!({ "project": self.project, "state": self.pulls.state.word() }),
            ),
        }
    }

    /// Calls `method` of the host's plugin about the project's repository, with `more`.
    fn plugin(
        &self,
        method: &'static str,
        more: Value,
    ) -> impl Future<Output = Result<Value, SharedString>> + 'static {
        let (owner, name) = self.host.repo.split_once('/').unwrap_or_default();
        let mut params = json!({
            "provider": self.host.host,
            "owner": owner,
            "name": name,
            "account": self.account,
        });
        if let (Some(params), Value::Object(more)) = (params.as_object_mut(), more) {
            params.extend(more);
        }
        let call = json!({ "plugin": format!("sikemux.{}", self.host.host), "method": method, "params": params });
        let answered = request(&self.asks, "plugin", call);
        async move {
            match answered.await {
                Ok(value) => answer(&value),
                Err(error) => Err(error.into()),
            }
        }
    }

    /// Tells the person how something went, in a toast.
    fn say(&self, said: Said, cx: &mut Context<Self>) {
        let note = match said {
            Said::Done(text) => Notification::success(text),
            Said::Failed(text) => Notification::error(text).autohide(false),
        };
        self.bench
            .update(cx, |bench, cx| bench.notify(note, cx))
            .ok();
    }

    fn ask(&mut self, asking: Asking, cx: &mut Context<Self>) {
        self.asking = Some(asking);
        cx.notify();
    }

    fn ask_watch(&self, key: &'static str, method: &'static str, params: Value) {
        let _ = self
            .asks
            .send(Ask::Watch(key, Watch::new(method, params, &["codeHost"])));
    }
}

impl Render for HostArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.area {
            Area::Pulls if self.pull.is_some() => pull::view(self, window, cx),
            Area::Pulls => div()
                .id("host-list")
                .size_full()
                .overflow_y_scroll()
                .child(pulls::list(self, cx))
                .into_any_element(),
            area => notice(
                format!("{} is not here yet", area.label(&self.host)),
                None,
                cx,
            ),
        };
        let asking = self.asking.as_ref().map(|asking| {
            let (yes, no) = (cx.entity().downgrade(), cx.entity().downgrade());
            let then = asking.then.clone();
            ui::Confirm::new(
                asking.title.clone(),
                asking.body.clone(),
                asking.yes.clone(),
            )
            .destructive(asking.destructive)
            .on_yes(move |_, cx| {
                let then = then.clone();
                yes.update(cx, |area, cx| {
                    area.asking = None;
                    then(area, cx);
                    cx.notify();
                })
                .ok();
            })
            .on_no(move |_, cx| {
                no.update(cx, |area, cx| {
                    area.asking = None;
                    cx.notify();
                })
                .ok();
            })
        });
        div().size_full().relative().child(body).children(asking)
    }
}

/// The value inside a code-host answer, or what its failure said.
fn answer<T: DeserializeOwned>(value: &Value) -> Result<T, SharedString> {
    if let Some(failed) = value.get("failed") {
        return Err(failed["message"]
            .as_str()
            .unwrap_or("the code host did not answer")
            .to_owned()
            .into());
    }
    serde_json::from_value(value["value"].clone()).map_err(|error| error.to_string().into())
}

/// What a list says while it has nothing to show: still loading, empty, or failed.
fn notice(title: impl Into<SharedString>, detail: Option<SharedString>, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .size_full()
        .min_h(px(240.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .px(px(24.))
        .child(ui::text(title, 12.5, FontWeight::MEDIUM).text_color(t.text_secondary))
        .children(detail.map(|detail| ui::meta(detail, cx).text_center()))
        .into_any_element()
}

/// A list's head: its area's name with its icon, then what it holds.
fn list_head(area: Area, host: &ProjectCodeHost, cx: &App) -> Div {
    let t = Tokens::get(cx);
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(12.))
        .pt(px(12.))
        .pb(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .mr(px(8.))
                .child(ui::icon(area.icon(), 14.).text_color(t.text_tertiary))
                .child(
                    ui::text(area.label(host), 12., FontWeight::MEDIUM).text_color(t.text_primary),
                ),
        )
}

/// A pull request's state as a mark in its colour: green while open, red once closed,
/// purple once merged, grey as a draft.
fn pull_mark(state: PullState, size: f32, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    let (glyph, ink, label) = match state {
        PullState::Open => (Icon::PullRequest, t.live, "Open"),
        PullState::Draft => (Icon::PullRequest, t.text_tertiary, "Draft"),
        PullState::Closed => (Icon::PullRequest, t.danger, "Closed"),
        PullState::Merged => (Icon::Merge, t.accent, "Merged"),
    };
    div()
        .id(SharedString::from(format!("pull-mark-{label}")))
        .flex_none()
        .tooltip(ui::tooltip(label))
        .child(ui::icon(glyph, size).text_color(ink))
        .into_any_element()
}

fn outcome_word(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Running => "running",
        Outcome::Queued => "queued",
        Outcome::Success => "success",
        Outcome::Failure => "failure",
        Outcome::Cancelled => "cancelled",
        Outcome::Skipped => "skipped",
        Outcome::Blocked => "blocked",
        Outcome::Unknown => "unknown",
    }
}

/// How many of a commit's checks stand each way, as one line.
fn tally_summary(tally: &CheckTally) -> String {
    if tally.passed == tally.total {
        return "all passed".into();
    }
    [
        (tally.failing, "failing"),
        (tally.running + tally.queued, "running"),
        (tally.waiting, "waiting"),
        (tally.cancelled, "cancelled"),
        (tally.unknown, "unknown"),
        (tally.passed, "passed"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, word)| format!("{count} {word}"))
    .collect::<Vec<_>>()
    .join(", ")
}

fn host_label(host: &str) -> &str {
    match host {
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "bitbucket" => "Bitbucket",
        other => other,
    }
}

fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Running => "Running",
        Outcome::Queued => "Queued",
        Outcome::Success => "Passed",
        Outcome::Failure => "Failed",
        Outcome::Cancelled => "Cancelled",
        Outcome::Skipped => "Skipped",
        Outcome::Blocked => "Waiting for approval",
        Outcome::Unknown => "Unknown",
    }
}

/// What started a run, in words.
fn event_label(event: &str) -> String {
    match event {
        "push" => "Push".into(),
        "pull_request" | "pull_request_target" => "Pull request".into(),
        "workflow_dispatch" => "Manual".into(),
        "schedule" => "Scheduled".into(),
        "release" => "Release".into(),
        "workflow_run" => "Workflow run".into(),
        "merge_group" => "Merge queue".into(),
        other => {
            let words = other.replace('_', " ");
            let mut letters = words.chars();
            letters
                .next()
                .map(|first| first.to_uppercase().chain(letters).collect())
                .unwrap_or_default()
        }
    }
}

/// How long something took from `started`, to `finished` or until now, in words.
fn elapsed(started: &str, finished: Option<&str>) -> String {
    let parse = |at: &str| chrono::DateTime::parse_from_rfc3339(at).ok();
    let Some(from) = parse(started) else {
        return "—".into();
    };
    let to = finished
        .and_then(parse)
        .map_or_else(chrono::Utc::now, |to| to.with_timezone(&chrono::Utc));
    let seconds = (to.timestamp() - from.timestamp()).max(0);
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 if seconds % 60 == 0 => format!("{}m", seconds / 60),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ if seconds / 60 % 60 == 0 => format!("{}h", seconds / 3600),
        _ => format!("{}h {}m", seconds / 3600, seconds / 60 % 60),
    }
}

/// A label as the host colours it.
fn label_chip(label: &Label, cx: &App) -> Div {
    let t = Tokens::get(cx);
    let ink = u32::from_str_radix(&label.color, 16)
        .map(|rgb| Hsla::from(gpui_kit::rgb(rgb)))
        .unwrap_or(t.text_tertiary);
    div()
        .px(px(7.))
        .rounded_full()
        .border_1()
        .border_color(ink.opacity(0.5))
        .bg(ink.opacity(0.14))
        .text_color(t.text_primary)
        .child(label.name.clone())
}

/// When something happened, as how long ago, from the host's RFC 3339 time.
fn ago(at: &str) -> String {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(at) else {
        return String::new();
    };
    match crate::agents::age(at.timestamp_millis().max(0) as u64) {
        now if now == "now" => now,
        age => format!("{age} ago"),
    }
}

/// A branch's name in a quiet chip.
fn branch_chip(name: &str, ink: Hsla, cx: &App) -> Div {
    let t = Tokens::get(cx);
    div()
        .flex_shrink(1.)
        .min_w_0()
        .max_w(px(220.))
        .px(px(6.))
        .py(px(1.))
        .rounded(crate::theme::RADIUS_1)
        .bg(t.surface_active)
        .font_family(crate::theme::MONO)
        .text_color(ink)
        .truncate()
        .child(name.to_owned())
}

/// A comment count with its icon, or nothing when there are none.
fn comments(count: u64, cx: &App) -> Option<Div> {
    let t = Tokens::get(cx);
    (count > 0).then(|| {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .text_size(px(11.))
            .text_color(t.text_tertiary)
            .font_features(crate::theme::tabular())
            .child(ui::icon(Icon::Message, 12.).text_color(t.text_tertiary))
            .child(count.to_string())
    })
}

/// Whether `key` is one this module watches, so the workbench hands its answers here.
pub fn watches(key: &str) -> bool {
    key.starts_with("host-")
}

/// The project's branch, and whether the core says it needs a pull request.
pub fn branch_of(project: &sikemux_wire::protocol::ProjectInfo) -> Option<(String, bool)> {
    project
        .git
        .as_ref()
        .map(|git| (git.branch.clone(), git.needs_pull))
}
