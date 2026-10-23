//! A plugin's pane. The core reads the views the plugin's manifest names, keeps them and says
//! when one changed; the pane keeps what it was last sent, reads a view again on that word, and
//! hands it to the plugin's own panel to draw. Panels hold only what the person picked; what
//! they show comes from the views, and what they change goes back through the plugin's calls.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;

use gpui_kit::{
    div, px, AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, Task, WeakEntity, Window,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::core::Target;
use crate::icons::Icon;
use crate::send_to::{self, Choice, Recipient};
use crate::theme::Tokens;
use crate::ui;
use crate::workbench::Workbench;

mod database;
mod jira;
mod linear;

/// How many views a pane keeps, the least lately asked for going first.
const KEPT_AT_MOST: usize = 32;

/// What a plugin draws in its pane, from the views its host keeps.
pub trait Panel: Any {
    fn render(
        &mut self,
        host: &mut Host,
        window: &mut Window,
        cx: &mut Context<PluginPane>,
    ) -> AnyElement;
}

/// The panel of `plugin`, for the plugins that draw a pane.
fn panel(plugin: &str) -> Option<Box<dyn Panel>> {
    match plugin {
        "sikemux.database" => Some(Box::<database::Database>::default()),
        "sikemux.jira" => Some(Box::<jira::Jira>::default()),
        "sikemux.linear" => Some(Box::<linear::Linear>::default()),
        _ => None,
    }
}

/// The surface a plugin's pane opens on, for the plugins that draw one.
pub fn surface(plugin: &str) -> Option<&'static str> {
    match plugin {
        "sikemux.database" => Some("browser"),
        "sikemux.jira" => Some("issues"),
        "sikemux.linear" => Some("issues"),
        _ => None,
    }
}

/// The plugin and the surface a pane's target names, as `<plugin>:<surface>`.
pub fn parts(target: &str) -> (&str, &str) {
    target.split_once(':').unwrap_or((target, ""))
}

/// Why a view or a call came to nothing, as the plugin or the core said it.
#[derive(Clone, Debug)]
pub struct Failure {
    pub category: SharedString,
    pub message: SharedString,
    pub status: Option<u64>,
}

impl Failure {
    fn of(failed: &Value) -> Self {
        Self {
            category: failed["category"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
                .into(),
            message: match failed["message"].as_str() {
                Some(message) => message.to_owned().into(),
                None => failed.to_string().into(),
            },
            status: failed["status"].as_u64(),
        }
    }

    fn core(error: String) -> Self {
        Self {
            category: "core".into(),
            message: error.into(),
            status: None,
        }
    }

    /// The same failure, said as what was being done when it happened.
    pub fn titled(&self, doing: &str) -> Self {
        Self {
            message: format!("{doing}: {}", self.message).into(),
            ..self.clone()
        }
    }

    /// The plugin is not signed in, or its sign-in no longer works.
    pub fn signed_out(&self) -> bool {
        matches!(self.category.as_ref(), "auth" | "unconfigured")
            || (self.category == "http" && self.status == Some(401))
    }
}

/// A view as the pane has it now.
pub enum View<'a> {
    Loading,
    Ready(&'a Value, &'a Read),
    Failed(&'a Failure),
}

/// A view as a panel last read it, so it is read once each time it arrives.
pub type Read = RefCell<Option<Rc<dyn Any>>>;

impl View<'_> {
    /// The view read as `T`; a view that does not read as one counts as failed.
    pub fn read<T: DeserializeOwned + 'static>(&self) -> Result<Option<Rc<T>>, Failure> {
        match self {
            View::Loading => Ok(None),
            View::Ready(value, read) => {
                if let Some(known) = read
                    .borrow()
                    .clone()
                    .and_then(|known| known.downcast().ok())
                {
                    return Ok(Some(known));
                }
                let fresh: Rc<T> = serde_json::from_value::<T>((*value).clone())
                    .map(Rc::new)
                    .map_err(|error| Failure::core(format!("unreadable view: {error}")))?;
                *read.borrow_mut() = Some(fresh.clone());
                Ok(Some(fresh))
            }
            View::Failed(failure) => Err((*failure).clone()),
        }
    }
}

enum Answer {
    Waiting,
    Got(Value, Read),
    Failed(Failure),
}

struct Kept {
    answer: Answer,
    asked: u64,
    _reading: Task<()>,
}

/// What a panel asks its plugin through: the views it shows and the calls it makes.
pub struct Host {
    pub plugin: String,
    pub surface: String,
    /// Where in the window the pane is, so two panes of one plugin each keep their views.
    slot: String,
    target: Target,
    bench: WeakEntity<Workbench>,
    kept: HashMap<(String, String), Kept>,
    asks: u64,
    status: Option<(Option<bool>, Kept)>,
    pub faces: crate::avatars::Avatars,
}

impl Host {
    /// The plugin's view `method` with `params`, asked of the core the first time and kept
    /// after, until the core says it changed.
    pub fn view(&mut self, method: &str, params: Value, cx: &mut Context<PluginPane>) -> View<'_> {
        self.asks += 1;
        let key = (method.to_owned(), params.to_string());
        if !self.kept.contains_key(&key) {
            self.forget_oldest();
            let reading = self.read(method, params, cx);
            self.kept.insert(
                key.clone(),
                Kept {
                    answer: Answer::Waiting,
                    asked: self.asks,
                    _reading: reading,
                },
            );
        }
        let kept = self.kept.get_mut(&key).expect("kept just now");
        kept.asked = self.asks;
        match &kept.answer {
            Answer::Waiting => View::Loading,
            Answer::Got(value, read) => View::Ready(value, read),
            Answer::Failed(failure) => View::Failed(failure),
        }
    }

    fn forget_oldest(&mut self) {
        if self.kept.len() < KEPT_AT_MOST {
            return;
        }
        let oldest = self
            .kept
            .iter()
            .min_by_key(|(_, kept)| kept.asked)
            .map(|(key, _)| key.clone());
        if let Some(oldest) = oldest {
            self.kept.remove(&oldest);
        }
    }

    fn read(&self, method: &str, params: Value, cx: &mut Context<PluginPane>) -> Task<()> {
        let key = (method.to_owned(), params.to_string());
        let asked = self.target.request(
            "pluginView",
            json!({
                "plugin": self.plugin,
                "method": method,
                "params": params,
                "slot": format!("{}/{method}", self.slot),
            }),
        );
        cx.spawn(async move |pane, cx| {
            let answer = match asked.await {
                Ok(answer) => match answer.get("failed") {
                    Some(failed) => Answer::Failed(Failure::of(failed)),
                    None => Answer::Got(answer["value"].clone(), Read::default()),
                },
                Err(error) => Answer::Failed(Failure::core(error)),
            };
            let _ = pane.update(cx, |pane, cx| {
                if let Some(kept) = pane.host.kept.get_mut(&key) {
                    kept.answer = answer;
                    cx.notify();
                }
            });
        })
    }

    /// Reads again the kept views the core says changed, `paths` naming them as
    /// `<plugin>/<method>`, or every one when it names none.
    fn changed(&mut self, paths: Option<&[String]>, cx: &mut Context<PluginPane>) {
        let moved = |method: &str| {
            paths.is_none_or(|paths| {
                paths
                    .iter()
                    .any(|path| *path == format!("{}/{method}", self.plugin))
            })
        };
        let stale: Vec<(String, String)> = self
            .kept
            .keys()
            .filter(|(method, _)| moved(method))
            .cloned()
            .collect();
        for key in stale {
            let params = serde_json::from_str(&key.1).unwrap_or(Value::Null);
            let reading = self.read(&key.0, params, cx);
            if let Some(kept) = self.kept.get_mut(&key) {
                kept._reading = reading;
            }
        }
    }

    /// Calls the plugin's `method`; the core reads its views again after a write.
    pub fn call(
        &self,
        method: &str,
        params: Value,
    ) -> impl Future<Output = Result<Value, Failure>> + 'static {
        let asked = self.target.request(
            "plugin",
            json!({ "plugin": self.plugin, "method": method, "params": params }),
        );
        async move {
            let answer = asked.await.map_err(Failure::core)?;
            match answer.get("failed") {
                Some(failed) => Err(Failure::of(failed)),
                None => Ok(answer["value"].clone()),
            }
        }
    }

    /// What the plugin's `status` call says of its sign-in, such as its accounts or sites,
    /// asked again whenever the core lists the plugin as signed in or out.
    pub fn status(&mut self, cx: &mut Context<PluginPane>) -> View<'_> {
        let signed_in = self.signed_in(cx);
        if self
            .status
            .as_ref()
            .is_none_or(|(asked, _)| *asked != signed_in)
        {
            let call = self.call("status", Value::Null);
            let reading = cx.spawn(async move |pane, cx| {
                let answer = match call.await {
                    Ok(value) => Answer::Got(value, Read::default()),
                    Err(failure) => Answer::Failed(failure),
                };
                let _ = pane.update(cx, |pane, cx| {
                    if let Some((_, kept)) = &mut pane.host.status {
                        kept.answer = answer;
                        cx.notify();
                    }
                });
            });
            let kept = Kept {
                answer: Answer::Waiting,
                asked: 0,
                _reading: reading,
            };
            self.status = Some((signed_in, kept));
        }
        match self.status.as_ref().map(|(_, kept)| &kept.answer) {
            Some(Answer::Got(value, read)) => View::Ready(value, read),
            Some(Answer::Failed(failure)) => View::Failed(failure),
            _ => View::Loading,
        }
    }

    /// Someone's picture, fetched through the plugin's `image` call, or their initials.
    pub fn face(&self, name: &str, url: Option<&str>, size: f32, cx: &App) -> AnyElement {
        self.faces.face(name, url, size, cx)
    }

    /// Copies `text` and says what was copied.
    pub fn copy(&self, text: String, what: &str, cx: &mut App) {
        copy(&self.bench, text, what, cx);
    }

    /// The workbench the pane is in.
    pub fn bench(&self) -> &WeakEntity<Workbench> {
        &self.bench
    }

    /// Has the core read the plugin's views again now, each of `method`'s or all of them.
    pub fn read_again(&self, method: Option<&str>) {
        self.target.call(
            "pluginViewsRead",
            json!({ "plugin": self.plugin, "method": method }),
        );
    }

    /// The host whose core the pane asks.
    pub fn place(&self) -> &crate::hosts::Place {
        self.target.place()
    }

    /// Whether the person is signed in to the plugin, as the core last listed it.
    pub fn signed_in(&self, cx: &App) -> Option<bool> {
        let bench = self.bench.upgrade()?;
        let plugins = bench.read(cx).read("plugins")?.as_array()?;
        plugins
            .iter()
            .find(|plugin| plugin["id"] == self.plugin.as_str())
            .and_then(|plugin| plugin["signedIn"].as_bool())
    }

    /// Signs in through the plugin's own browser sign-in, opening the page it names.
    pub fn sign_in_with_browser(&self, cx: &mut Context<PluginPane>) {
        let mut streaming = self.target.stream(
            "pluginStream",
            json!({ "plugin": self.plugin, "method": "signInWithBrowser", "params": {} }),
        );
        let bench = self.bench.clone();
        cx.spawn(async move |_, cx| {
            let mut failed = None;
            while let Some(part) = streaming.parts.recv().await {
                for item in part.items {
                    if let Some(url) = item["url"].as_str() {
                        let url = url.to_owned();
                        cx.update(|cx| cx.open_url(&url));
                    }
                }
                if let Some(error) = part.error {
                    failed = Some(Failure::of(&error));
                }
                if part.done {
                    break;
                }
            }
            if let Some(failed) = failed {
                let _ = bench.update(cx, |bench, cx| {
                    let note = gpui_kit::component::notification::Notification::error(format!(
                        "Sign-in did not finish: {}",
                        failed.message
                    ));
                    bench.notify(note, cx);
                });
            }
        })
        .detach();
    }

    /// The agents the pane can hand text to.
    pub fn agents(&self, cx: &App) -> Vec<Choice> {
        self.bench
            .upgrade()
            .map_or_else(Vec::new, |bench| send_to::choices(bench.read(cx)))
    }

    /// Puts `text` in the composer of the agent `to` names and brings it forward.
    pub fn send_to_agent(&self, to: Recipient, text: String, window: &mut Window, cx: &mut App) {
        if let Some(bench) = self.bench.upgrade() {
            bench.update(cx, |bench, cx| {
                send_to::send(bench, to, async move { Some(text) }, window, cx)
            });
        }
    }

    /// Says what went wrong with a call, over the window.
    pub fn said(&self, failure: &Failure, cx: &mut App) {
        let note = gpui_kit::component::notification::Notification::error(failure.message.clone());
        if let Some(bench) = self.bench.upgrade() {
            bench.update(cx, |bench, cx| bench.notify(note, cx));
        }
    }
}

/// A plugin's pane: its host and the panel that draws it.
pub struct PluginPane {
    pub host: Host,
    panel: Option<Box<dyn Panel>>,
}

impl PluginPane {
    pub fn new(target: &str, slot: String, core: Target, bench: WeakEntity<Workbench>) -> Self {
        let (plugin, surface) = parts(target);
        Self {
            panel: panel(plugin),
            host: Host {
                plugin: plugin.to_owned(),
                surface: surface.to_owned(),
                slot,
                target: core,
                bench,
                kept: HashMap::new(),
                asks: 0,
                status: None,
                faces: Default::default(),
            },
        }
    }

    /// The pane's panel as the `P` it is, with the host beside it.
    pub fn parts<P: Panel>(&mut self) -> Option<(&mut P, &mut Host)> {
        let panel: &mut dyn Any = self.panel.as_deref_mut()?;
        Some((panel.downcast_mut::<P>()?, &mut self.host))
    }

    /// The core says some of the plugin's views changed.
    pub fn changed(&mut self, paths: Option<&[String]>, cx: &mut Context<Self>) {
        self.host.changed(paths, cx);
    }
}

impl Render for PluginPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drawn = match &mut self.panel {
            Some(panel) => panel.render(&mut self.host, window, cx),
            None => empty("This plugin draws no pane here yet.", cx),
        };
        let host = &self.host;
        host.faces.fetch(
            |url| {
                let fetched = host.call("image", json!({ "url": url }));
                async move { fetched.await.ok()?.as_str().map(str::to_owned) }
            },
            |pane| &pane.host.faces,
            cx,
        );
        div().size_full().flex().flex_col().child(drawn)
    }
}

/// Copies `text` and says over the window what was copied.
pub fn copy(bench: &WeakEntity<Workbench>, text: String, what: &str, cx: &mut App) {
    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
    let note = gpui_kit::component::notification::Notification::success(format!("Copied {what}"));
    if let Some(bench) = bench.upgrade() {
        bench.update(cx, |bench, cx| bench.notify(note, cx));
    }
}

/// A handler on panel `P` and its host, for an element of the panel's.
pub fn listen<P: Panel, E: ?Sized>(
    cx: &Context<PluginPane>,
    act: impl Fn(&mut P, &mut Host, &E, &mut Window, &mut Context<PluginPane>) + 'static,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    let pane = cx.weak_entity();
    move |event, window, cx| {
        let _ = pane.update(cx, |pane, cx| {
            if let Some((panel, host)) = pane.parts::<P>() {
                act(panel, host, event, window, cx);
            }
        });
    }
}

/// Runs `work`, then hands what it gave to panel `P`.
pub fn after<P: Panel, T: 'static>(
    work: impl Future<Output = T> + 'static,
    then: impl FnOnce(&mut P, &mut Host, T, &mut Context<PluginPane>) + 'static,
    cx: &mut Context<PluginPane>,
) {
    cx.spawn(async move |pane, cx| {
        let done = work.await;
        let _ = pane.update(cx, |pane, cx| {
            if let Some((panel, host)) = pane.parts::<P>() {
                then(panel, host, done, cx);
                cx.notify();
            }
        });
    })
    .detach();
}

/// Said while a view is still on its way.
pub fn loading(cx: &App) -> AnyElement {
    notice(None, "Loading…", None, cx)
}

/// Said when a list or a page has nothing to show.
pub fn empty(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    notice(None, text, None, cx)
}

/// What went wrong with a view.
pub fn failure(failure: &Failure, cx: &App) -> AnyElement {
    notice(
        Some(Icon::Warning),
        "This did not load",
        Some(failure.message.clone()),
        cx,
    )
}

/// Asks the person to sign in to the plugin through its browser sign-in.
pub fn signed_out(
    name: &'static str,
    detail: &'static str,
    cx: &mut Context<PluginPane>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .child(
            ui::text(format!("Sign in to {name}"), 14., FontWeight::SEMIBOLD)
                .text_color(t.text_primary),
        )
        .child(ui::text(detail, 12., FontWeight::NORMAL).text_color(t.text_tertiary))
        .child(
            ui::Button::new(
                "plugin-sign-in",
                "Sign in with the browser",
                ui::Variant::Primary,
            )
            .icon(Icon::LogIn)
            .on_click(cx.listener(|pane, _, _, cx| pane.host.sign_in_with_browser(cx))),
        )
        .into_any_element()
}

fn notice(
    icon: Option<Icon>,
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .p(px(24.))
        .children(icon.map(|icon| ui::icon(icon, 16.).text_color(t.text_tertiary)))
        .child(ui::text(title, 12.5, FontWeight::MEDIUM).text_color(t.text_secondary))
        .children(detail.map(|detail| {
            div()
                .max_w(px(420.))
                .text_center()
                .text_size(crate::theme::ui_text(12.))
                .text_color(t.text_tertiary)
                .child(detail)
        }))
        .into_any_element()
}
