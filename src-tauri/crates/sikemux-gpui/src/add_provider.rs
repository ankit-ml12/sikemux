//! The dialog that adds a model provider from whatever the person pastes: a key, a base URL
//! or the provider's whole quickstart. The core reads it, checks the key and says which agent
//! runs it; this host installs that agent meanwhile when it lacks it.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::WindowExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, Animation, AnimationExt, AnyElement, AppContext, Context, Entity, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Window,
};
use serde_json::{json, Value};

use crate::agent_setup::{self, Installing};
use crate::core::Target;
use crate::hosts::Place;
use crate::icons::Icon;
use crate::theme::{self, Tokens, RADIUS_2};
use crate::toast;
use crate::ui::{self, dialog};
use crate::workbench::Workbench;

/// How long typing rests before what was pasted is read.
const SETTLE: Duration = Duration::from_millis(350);

pub struct AddProvider {
    target: Target,
    field: Entity<TextareaState>,
    /// The core's reading of the text it was asked about.
    read: Option<(String, Value)>,
    reading: bool,
    adding: bool,
    failure: Option<String>,
    _typing: Subscription,
}

impl AddProvider {
    fn new(target: Target, window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let field = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 8)
                .placeholder("sk-…, https://api.example.com/v1, or the quickstart")
        });
        field.update(cx, |field, cx| field.focus(window, cx));
        let typing = cx.subscribe(&field, |bench, field, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = field.read(cx).value().trim().to_owned();
                settle(bench, text, cx);
            }
        });
        Self {
            target,
            field,
            read: None,
            reading: false,
            adding: false,
            failure: None,
            _typing: typing,
        }
    }

    /// The reading of what the field holds now, once the core has given it.
    fn current(&self, cx: &gpui_kit::App) -> Option<&Value> {
        let text = self.field.read(cx).value();
        let (read_for, read) = self.read.as_ref()?;
        (read_for == text.trim()).then_some(read)
    }
}

impl Workbench {
    /// Opens the dialog, adding to the host in front.
    pub fn add_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_provider_to(self.target(), window, cx);
    }

    pub fn add_provider_to(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        self.adding_provider = Some(AddProvider::new(target, window, cx));
        cx.notify();
    }
}

/// Reads `text` once typing rests on it.
fn settle(bench: &mut Workbench, text: String, cx: &mut Context<Workbench>) {
    let Some(adding) = bench.adding_provider.as_mut() else {
        return;
    };
    adding.failure = None;
    adding.reading = !text.is_empty();
    cx.notify();
    if text.is_empty() {
        return;
    }
    let target = adding.target.clone();
    cx.spawn(async move |bench, cx| {
        cx.background_executor().timer(SETTLE).await;
        let still = bench
            .update(cx, |bench, cx| {
                bench
                    .adding_provider
                    .as_ref()
                    .is_some_and(|adding| adding.field.read(cx).value().trim() == text)
            })
            .unwrap_or(false);
        if !still {
            return;
        }
        let read = target
            .request("modelProviderRead", json!({ "text": text }))
            .await
            .unwrap_or_else(|failure| json!({ "failure": readable(failure) }));
        let _ = bench.update(cx, |bench, cx| {
            let Some(adding) = bench.adding_provider.as_mut() else {
                return;
            };
            if adding.field.read(cx).value().trim() != text {
                return;
            }
            adding.reading = false;
            let works = read["failure"].is_null() && read["models"].is_array();
            let agent = read["runsWith"].as_str().map(str::to_owned);
            let here = matches!(adding.target.place(), Place::This);
            adding.read = Some((text, read));
            if let Some(agent) = agent.filter(|agent| works && here && !bench.has_agent(agent)) {
                agent_setup::install_behind(bench, &agent, cx);
            }
            cx.notify();
        });
    })
    .detach();
}

fn close(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    bench.adding_provider = None;
    cx.notify();
}

fn add(bench: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>) {
    let Some(adding) = bench.adding_provider.as_mut() else {
        return;
    };
    let Some(read) = adding.current(cx).cloned() else {
        return;
    };
    if adding.adding || !read["failure"].is_null() || !read["models"].is_array() {
        return;
    }
    let text = adding.field.read(cx).value().trim().to_owned();
    adding.adding = true;
    let added = adding
        .target
        .request("modelProviderAdd", json!({ "text": text }));
    cx.notify();
    cx.spawn_in(window, async move |bench, cx| {
        let added = added.await;
        let _ = bench.update_in(cx, |bench, window, cx| {
            match added {
                Ok(provider) => {
                    let label = provider["label"].as_str().unwrap_or("the provider");
                    window.push_notification(
                        toast::success(format!(
                            "Added {label}; its models are in every chat's model menu"
                        )),
                        cx,
                    );
                    bench.adding_provider = None;
                    crate::settings::providers_changed(bench, cx);
                }
                Err(failure) => {
                    if let Some(adding) = bench.adding_provider.as_mut() {
                        adding.adding = false;
                        adding.failure = Some(readable(failure));
                    }
                }
            }
            cx.notify();
        });
    })
    .detach();
}

/// A failed request in the person's words: the core words its own refusals, but a host too old
/// to know the call, or a core that went away, says only what it lacked.
fn readable(failure: String) -> String {
    if failure.starts_with("this host does not know") {
        "This host's Sikemux is too old to add providers. Update it, then try again.".into()
    } else if failure == "the core is not connected" {
        "Sikemux lost touch with this host. Try again in a moment.".into()
    } else {
        failure
    }
}

/// Why the agent Sikemux picked runs the provider, from the APIs it speaks.
fn reason(name: &str, read: &Value) -> String {
    let speaks = |api: &str| {
        read["speaks"]
            .as_array()
            .is_some_and(|all| all.iter().any(|each| each == api))
    };
    if speaks("anthropic") {
        format!("{name} speaks Anthropic's API, which Claude Code uses.")
    } else if speaks("responses") {
        format!("{name} speaks OpenAI's Responses API, which Codex uses.")
    } else {
        format!("{name} speaks only chat completions, so Claude Code and Codex can't use it.")
    }
}

/// A line under the field: what it names, then what was found.
fn fact(name: &'static str, value: AnyElement, t: &Tokens) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .text_size(crate::theme::ui_text(12.))
        .child(
            div()
                .w(px(36.))
                .flex_none()
                .text_color(t.text_tertiary)
                .child(name),
        )
        .child(div().flex_1().min_w_0().truncate().child(value))
}

/// A thin bar a short stroke runs along while the install has no end in sight; still for a
/// person who asked for less motion.
fn running_bar(t: &Tokens, still: bool) -> AnyElement {
    let (track, ink) = (t.border, t.text_tertiary);
    let bar = move |at: f32| {
        div()
            .relative()
            .h(px(2.))
            .w_full()
            .rounded(px(1.))
            .bg(track)
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .h_full()
                    .w(gpui_kit::relative(0.3))
                    .left(gpui_kit::relative(at * 1.3 - 0.3))
                    .bg(ink),
            )
    };
    if still {
        return bar(0.5).into_any_element();
    }
    div()
        .w_full()
        .with_animation(
            "provider-install",
            Animation::new(Duration::from_millis(1400))
                .repeat()
                .with_max_fps(30.),
            move |holder, at| holder.child(bar(at)),
        )
        .into_any_element()
}

/// The dialog, while one is open.
pub fn view(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let adding = bench.adding_provider.as_ref()?;
    let t = Tokens::get(cx).clone();
    let this = cx.entity();
    let read = adding.current(cx).cloned();
    let name = read
        .as_ref()
        .and_then(|read| read["label"].as_str())
        .unwrap_or("provider")
        .to_owned();
    let models = read
        .as_ref()
        .and_then(|read| read["models"].as_array())
        .map(Vec::len);
    let failure = adding
        .failure
        .clone()
        .or_else(|| read.as_ref()?["failure"].as_str().map(str::to_owned));
    let ready = read.is_some() && failure.is_none() && models.is_some() && !adding.adding;
    let field = div()
        .px(px(10.))
        .py(px(8.))
        .border_1()
        .border_color(t.border_strong)
        .rounded(RADIUS_2)
        .bg(t.surface_sunken)
        .font_family(theme::MONO)
        .text_color(t.text_secondary)
        .child(
            Textarea::new(&adding.field)
                .appearance(false)
                .text_size(crate::theme::ui_text(11.)),
        );
    let found = read
        .as_ref()
        .filter(|read| !read["baseUrl"].is_null() || !read["label"].is_null())
        .map(|read| {
            let url = read["baseUrl"]
                .as_str()
                .map(|url| {
                    url.split_once("://")
                        .map_or(url, |(_, rest)| rest)
                        .to_owned()
                })
                .unwrap_or_default();
            let key = match (&failure, models) {
                (Some(failure), _) => {
                    ui::text(failure.clone(), 12., FontWeight::NORMAL).text_color(t.danger)
                }
                (None, Some(count)) => ui::text(
                    format!("Works · {count} model{}", if count == 1 { "" } else { "s" }),
                    12.,
                    FontWeight::NORMAL,
                )
                .text_color(t.live),
                (None, None) => {
                    ui::text("Checking…", 12., FontWeight::NORMAL).text_color(t.text_tertiary)
                }
            };
            div()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(fact(
                    "Name",
                    ui::text(name.clone(), 12., FontWeight::NORMAL)
                        .text_color(t.text_primary)
                        .into_any_element(),
                    &t,
                ))
                .child(fact(
                    "URL",
                    ui::mono(url, 11., cx)
                        .text_color(t.text_primary)
                        .into_any_element(),
                    &t,
                ))
                .child(fact("Key", key.into_any_element(), &t))
        });
    let checking = (adding.reading && found.is_none())
        .then(|| ui::text("Reading…", 12., FontWeight::NORMAL).text_color(t.text_tertiary));
    let unread = found.is_none() && failure.is_some();
    let runs = read
        .as_ref()
        .filter(|_| ready || adding.adding)
        .and_then(|read| Some((read["runsWith"].as_str()?.to_owned(), read)))
        .map(|(agent, read)| {
            let label = bench
                .read("catalog")
                .and_then(Value::as_array)
                .and_then(|kinds| kinds.iter().find(|kind| kind["type"] == agent.as_str()))
                .and_then(|kind| kind["label"].as_str())
                .unwrap_or(&agent)
                .to_owned();
            let state = match (
                bench.has_agent(&agent),
                bench.agent_setup.installing(&agent),
            ) {
                (true, _) => None,
                (false, Some(Installing::Running)) => Some(("Installing", t.text_tertiary)),
                (false, Some(Installing::Failed)) => Some(("Install failed", t.danger)),
                (false, None) => Some(("Not installed", t.text_tertiary)),
            };
            let running = matches!(state, Some(("Installing", _)));
            let failed = matches!(state, Some(("Install failed", _)));
            let open = {
                let (this, agent) = (this.clone(), agent.clone());
                move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut gpui_kit::App| {
                    this.update(cx, |bench, cx| {
                        bench.adding_provider = None;
                        agent_setup::show_install(bench, &agent, window, cx)
                    })
                }
            };
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .pt(px(10.))
                .border_t_1()
                .border_color(t.border)
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(9.))
                        .text_size(crate::theme::ui_text(12.))
                        .child(div().pt(px(1.)).child(ui::mark(&agent, 14.)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_color(t.text_primary)
                                        .child(format!("Runs with {label}")),
                                )
                                .child(
                                    div()
                                        .text_size(crate::theme::ui_text(11.))
                                        .line_height(crate::theme::ui_text(15.))
                                        .text_color(t.text_tertiary)
                                        .child(reason(&name, read)),
                                ),
                        )
                        .children(state.map(|(words, ink)| {
                            div()
                                .id("provider-agent-state")
                                .flex_none()
                                .text_size(crate::theme::ui_text(11.))
                                .text_color(ink)
                                .child(words)
                                .when(failed, |state| state.cursor_pointer().on_click(open))
                        })),
                )
                .when(running, |line| {
                    line.child(div().pl(px(23.)).child(running_bar(&t, cx.reduce_motion())))
                })
        });
    let body = dialog::marked_body()
        .pt(px(4.))
        .gap(px(12.))
        .child(
            div()
                .text_size(crate::theme::ui_text(12.))
                .line_height(crate::theme::ui_text(17.))
                .text_color(t.text_tertiary)
                .child("Paste an API key, a base URL, or the quickstart from the provider's dashboard."),
        )
        .child(field)
        .children(checking)
        .children(found)
        .when(unread, |body| {
            body.child(
                ui::text(failure.clone().unwrap_or_default(), 12., FontWeight::NORMAL)
                    .text_color(t.danger),
            )
        })
        .children(runs)
        .into_any_element();
    let (adds, closing) = (this.clone(), this);
    let yes: dialog::Answer =
        Rc::new(move |window, cx| adds.update(cx, |bench, cx| add(bench, window, cx)));
    let no: dialog::Answer = Rc::new(move |_, cx| closing.update(cx, close));
    let label: SharedString = match (
        adding.adding,
        read.as_ref().and_then(|read| read["label"].as_str()),
    ) {
        (true, _) => "Adding…".into(),
        (false, Some(name)) => format!("Add {name}").into(),
        (false, None) => "Add".into(),
    };
    let foot = dialog::hinted_foot(dialog::escape("cancel", cx), cx)
        .child(dialog::button(
            "provider-cancel",
            "Cancel".into(),
            None,
            no.clone(),
            cx,
        ))
        .child(
            div()
                .when(!ready, |button| button.opacity(0.45))
                .child(dialog::button(
                    "provider-add",
                    label,
                    Some(t.accent),
                    yes.clone(),
                    cx,
                )),
        );
    Some(
        dialog::marked_sheet(Icon::Plus, "Add a provider".into(), body, foot, yes, no, cx)
            .into_any_element(),
    )
}
