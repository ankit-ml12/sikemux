//! A plugin's pane. The core reads the views the plugin's manifest names, keeps them and says
//! when one changed; the pane keeps what it was last sent, reads a view again on that word, and
//! hands it to the plugin's own panel to draw. Panels hold only what the person picked; what
//! they show comes from the views, and what they change goes back through the plugin's calls.

use std::collections::HashMap;
use std::future::Future;

use gpui_kit::{
    div, px, AnyElement, App, Context, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, Task, WeakEntity, Window,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::core::Target;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui;
use crate::workbench::Workbench;

mod jira;

/// How many views a pane keeps, the least lately asked for going first.
const KEPT_AT_MOST: usize = 32;

/// What a plugin draws in its pane, from the views its host keeps.
pub trait Panel {
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
        "sikemux.jira" => Some(Box::new(jira::Jira)),
        _ => None,
    }
}

/// The surface a plugin's pane opens on, for the plugins that draw one.
pub fn surface(plugin: &str) -> Option<&'static str> {
    match plugin {
        "sikemux.jira" => Some("issues"),
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

    /// The plugin is not signed in, or its sign-in no longer works.
    pub fn signed_out(&self) -> bool {
        matches!(self.category.as_ref(), "auth" | "unconfigured")
            || (self.category == "http" && self.status == Some(401))
    }
}

/// A view as the pane has it now.
pub enum View<'a> {
    Loading,
    Ready(&'a Value),
    Failed(&'a Failure),
}

impl View<'_> {
    /// The view read as `T`; a view that does not read as one counts as failed.
    pub fn read<T: DeserializeOwned>(&self) -> Result<Option<T>, Failure> {
        match self {
            View::Loading => Ok(None),
            View::Ready(value) => serde_json::from_value((*value).clone())
                .map(Some)
                .map_err(|error| Failure::core(format!("unreadable view: {error}"))),
            View::Failed(failure) => Err((*failure).clone()),
        }
    }
}

enum Answer {
    Waiting,
    Got(Value),
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
            Answer::Got(value) => View::Ready(value),
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
                    None => Answer::Got(answer["value"].clone()),
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
            },
        }
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
        div().size_full().flex().flex_col().child(drawn)
    }
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
