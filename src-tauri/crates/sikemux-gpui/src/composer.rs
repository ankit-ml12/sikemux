use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::{Button as BaseButton, Popover};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, radians, Anchor, App, Entity, Focusable, FontWeight, Hsla, Image, ImageFormat, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Transformation, Window,
};
use sikemux_wire::chat::ContextUsage;
use sikemux_wire::protocol::ConfigPicker;

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, IconButton, Menu, MenuItem};

type Act = Rc<dyn Fn(&mut Window, &mut App)>;
type Pick = Rc<dyn Fn(&str, &str, &mut Window, &mut App)>;

/// The project a new chat starts in, docked on the composer until its first message.
pub struct Strip {
    pub project: SharedString,
    pub path: SharedString,
    pub branch: Option<SharedString>,
}

/// A chat's composer: its draft, and the bar of what the chat lets the person change, as the
/// chat says. It keeps no state of its own.
#[derive(IntoElement)]
pub struct Composer {
    draft: Entity<TextareaState>,
    provider: String,
    agent_label: SharedString,
    strip: Option<Strip>,
    ready: bool,
    running: bool,
    steers: bool,
    yolo: Option<(bool, Act)>,
    pickers: Option<(Option<ConfigPicker>, Option<ConfigPicker>, Pick)>,
    usage: Option<ContextUsage>,
    on_send: Act,
    on_stop: Act,
}

impl Composer {
    pub fn new(draft: &Entity<TextareaState>, provider: &str, agent_label: impl Into<SharedString>, on_send: Act, on_stop: Act) -> Self {
        Self {
            draft: draft.clone(),
            provider: provider.to_owned(),
            agent_label: agent_label.into(),
            strip: None,
            ready: false,
            running: false,
            steers: true,
            yolo: None,
            pickers: None,
            usage: None,
            on_send,
            on_stop,
        }
    }

    pub fn strip(mut self, strip: Option<Strip>) -> Self {
        self.strip = strip;
        self
    }

    /// The session takes changes to its mode and settings.
    pub fn ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    /// Sending while a turn runs steers into it, else it queues behind it.
    pub fn steers(mut self, steers: bool) -> Self {
        self.steers = steers;
        self
    }

    /// Whether the agent runs without asking, for one that can switch, and how to switch it.
    pub fn yolo(mut self, on: bool, toggle: Act) -> Self {
        self.yolo = Some((on, toggle));
        self
    }

    pub fn pickers(mut self, model: Option<ConfigPicker>, effort: Option<ConfigPicker>, pick: Pick) -> Self {
        self.pickers = Some((model, effort, pick));
        self
    }

    pub fn usage(mut self, usage: Option<ContextUsage>) -> Self {
        self.usage = usage;
        self
    }
}

impl RenderOnce for Composer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let fill = t.surface_composer;
        let drafted = !self.draft.read(cx).value().trim().is_empty();
        let focused = self.draft.focus_handle(cx).contains_focused(window, cx);
        let settable = self.ready && !self.running;
        let mut bar = div()
            .min_h(px(34.))
            .pt(px(8.))
            .px(px(2.))
            .pb(px(1.))
            .flex()
            .items_center()
            .gap(px(5.))
            .child(IconButton::new("composer-attach", Icon::Plus, "Add files").glyph(17.).width(31.));
        if let Some((on, toggle)) = self.yolo {
            bar = bar.child(yolo(on, settable, &self.agent_label, toggle, cx));
        }
        if let Some((model, effort, pick)) = self.pickers {
            bar = bar.children(model.map(|model| picker(model, "Model", Some(&self.provider), settable, pick.clone(), cx)));
            bar = bar.children(effort.map(|effort| picker(effort, "Reasoning effort", None, settable, pick, cx)));
        }
        let bar = bar
            .children(self.usage.map(|usage| context_meter(usage, &self.provider, cx)))
            .child(div().flex_1())
            .child(IconButton::new("composer-dictate", Icon::Mic, "Dictate — or hold right ⌥ anywhere").glyph(16.).width(31.))
            .child(send(self.running && !drafted, drafted, self.running.then_some(self.steers), self.on_send, self.on_stop, cx));
        div()
            .w_full()
            .px(px(16.))
            .pb(px(16.))
            .flex()
            .flex_col()
            .items_center()
            .children(self.strip.map(|strip| project_strip(strip, fill, cx)))
            .child(
                div()
                    .w_full()
                    .max_w(px(820.))
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .border_1()
                    .border_color(if focused { t.border_selected } else { t.border })
                    .rounded(theme::RADIUS_3)
                    .bg(fill)
                    .child(div().px(px(12.)).py(px(10.)).child(Textarea::new(&self.draft).appearance(false).text_size(px(14.5))))
                    .child(bar),
            )
    }
}

fn project_strip(strip: Strip, fill: Hsla, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    div()
        .w_full()
        .max_w(px(780.))
        .flex()
        .items_center()
        .gap(px(10.))
        .pt(px(5.))
        .pr(px(12.))
        .pb(px(5.))
        .pl(px(6.))
        .border_t_1()
        .border_l_1()
        .border_r_1()
        .border_color(t.border)
        .rounded_t(theme::RADIUS_3)
        .bg(fill)
        .child(
            BaseButton::new("composer-project")
                .accessibility_label("Project")
                .h(px(26.))
                .pl(px(8.))
                .pr(px(9.))
                .max_w(px(240.))
                .flex()
                .items_center()
                .gap(px(7.))
                .rounded_full()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_primary)
                .hover(move |style| style.bg(t.surface_hover))
                .child(ui::icon(Icon::Folder, 12.).text_color(t.live))
                .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(strip.project))
                .child(ui::icon(Icon::Chevron, 10.).with_transformation(Transformation::rotate(radians(FRAC_PI_2))).text_color(t.text_tertiary)),
        )
        .child(
            div()
                .ml_auto()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(ui::mono(strip.path, 10.5, cx).text_color(t.text_tertiary)),
        )
        .children(strip.branch.map(|branch| {
            div()
                .flex_none()
                .max_w(px(160.))
                .flex()
                .items_center()
                .gap(px(4.))
                .child(ui::icon(Icon::Git, 11.).text_color(t.text_tertiary))
                .child(ui::mono(branch, 10.5, cx).text_color(t.text_tertiary).overflow_hidden().text_ellipsis().whitespace_nowrap())
        }))
}

/// YOLO or SAFE: whether the agent acts without asking. YOLO's letters run through the
/// rainbow the app paints it with.
fn yolo(on: bool, settable: bool, agent: &str, toggle: Act, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let about = if on {
        format!("YOLO mode on — {agent} runs without approvals. Click turns it off.")
    } else {
        format!("Safe mode — {agent} asks before it acts. Click goes YOLO.")
    };
    let letters: Vec<(char, Hsla)> = if on {
        "YOLO".chars().zip([t.accent, t.cmd, t.danger, t.warn]).collect()
    } else {
        "SAFE".chars().map(|letter| (letter, t.text_tertiary)).collect()
    };
    BaseButton::new("composer-yolo")
        .accessibility_label(if on { "YOLO" } else { "SAFE" })
        .selected(on)
        .disabled(!settable)
        .h(px(31.))
        .px(px(11.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded_full()
        .when(!settable, |toggle| toggle.opacity(0.42))
        .when(settable, |toggle| toggle.hover(move |style| style.bg(Hsla { a: 0.07, ..t.text_primary })))
        .tooltip(ui::tooltip(about))
        .on_click(move |_, window, cx| toggle(window, cx))
        .child(ui::icon(if on { Icon::ShieldBolt } else { Icon::Shield }, 12.).text_color(if on { t.accent } else { t.text_tertiary }))
        .child(div().flex().children(letters.into_iter().map(|(letter, color)| {
            ui::text(letter.to_string(), 10.5, FontWeight::SEMIBOLD).letter_spacing(0.08).text_color(color)
        })))
}

/// One of the session's settings, such as its model, with a menu of the values it takes.
fn picker(picker: ConfigPicker, name: &'static str, provider: Option<&str>, settable: bool, pick: Pick, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let label = picker
        .choices
        .iter()
        .find(|choice| choice.value == picker.current)
        .map_or_else(|| picker.current.clone(), |choice| choice.label.clone());
    let label = if label.is_empty() { name.split(' ').next_back().unwrap_or(name).to_owned() } else { label };
    let first = provider.is_some();
    let enabled = settable && !picker.choices.is_empty();
    let trigger = BaseButton::new(SharedString::from(format!("picker-{}", picker.id)))
        .accessibility_label(name)
        .disabled(!enabled)
        .h(px(31.))
        .px(px(11.))
        .max_w(px(220.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(px(11.))
        .text_color(if first { t.accent } else { t.text_tertiary })
        .when(!enabled, |trigger| trigger.opacity(0.45))
        .when(enabled, |trigger| trigger.hover(move |style| style.bg(Hsla { a: 0.07, ..t.text_primary })))
        .children(provider.map(|provider| ui::mark(provider, 17.)))
        .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(label.clone()))
        .child(div().opacity(0.6).child(ui::icon(Icon::Chevron, 10.).with_transformation(Transformation::rotate(radians(FRAC_PI_2))).text_color(t.text_tertiary)));
    if !enabled {
        return trigger.into_any_element();
    }
    let ConfigPicker { id, current, choices } = picker;
    Popover::new(SharedString::from(format!("picker-menu-{id}")))
        .anchor(if first { Anchor::BottomLeft } else { Anchor::BottomRight })
        .offset(px(12.))
        .trigger(trigger)
        .content(move |_, _, cx| {
            let popover = cx.entity();
            Menu::new().children(choices.iter().map(|choice| {
                let (pick, popover, id, value) = (pick.clone(), popover.clone(), id.clone(), choice.value.clone());
                MenuItem::new(SharedString::from(format!("{id}-{value}")), choice.label.clone()).checked(choice.value == current).on_click(
                    move |_, window, cx| {
                        pick(&id, &value, window, cx);
                        popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    },
                )
            }))
        })
        .into_any_element()
}

/// How much of the context window the session has used, as a ring.
fn context_meter(usage: ContextUsage, provider: &str, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let percent = if usage.size > 0. { (usage.used / usage.size * 100.).clamp(0., 100.) } else { 0. };
    let signal = match percent {
        p if p >= 90. => t.danger,
        p if p >= 70. => t.warn,
        _ if matches!(provider, "claude" | "codex") => theme::brand(provider),
        _ => t.accent,
    };
    let hex = |color: Hsla| {
        let rgba = color.to_rgb();
        format!("#{:02x}{:02x}{:02x}", (rgba.r * 255.) as u8, (rgba.g * 255.) as u8, (rgba.b * 255.) as u8)
    };
    let around = std::f64::consts::TAU * 7.;
    let ring = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke-width="2.2"><circle cx="8" cy="8" r="7" stroke="{track}"/><circle cx="8" cy="8" r="7" stroke="{fill}" stroke-linecap="round" stroke-dasharray="{lit:.2} {around:.2}" transform="rotate(-90 8 8)"/></svg>"#,
        track = hex(t.line),
        fill = hex(signal),
        lit = around * percent / 100.,
    );
    let mut about = vec![
        "Context window".to_owned(),
        format!("{}% used · {} of {} tokens", percent.round(), crate::home::compact(usage.used), crate::home::compact(usage.size)),
    ];
    about.extend(usage.cost.map(|cost| format!("Session cost {}", crate::home::money(cost.amount))));
    div()
        .id("context-meter")
        .size(px(31.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .hover(move |style| style.bg(Hsla { a: 0.07, ..t.text_primary }))
        .tooltip(ui::tooltip(about.join("\n")))
        .child(img(Arc::new(Image::from_bytes(ImageFormat::Svg, ring.into_bytes()))).size(px(14.)))
}

/// Send, or while a turn runs and nothing is written, Stop. While a turn runs, `steers` tells
/// whether sending steers into it.
fn send(stopping: bool, drafted: bool, steers: Option<bool>, on_send: Act, on_stop: Act, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let about = steers.filter(|_| !stopping).map(|steers| {
        if steers { "Steers into this turn — ⌘↵ queues behind it" } else { "Queues behind this turn — ⌘↵ steers into it" }
    });
    BaseButton::new("composer-send")
        .accessibility_label(if stopping { "Stop agent" } else { "Send message" })
        .disabled(!stopping && !drafted)
        .ml(px(3.))
        .size(px(30.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(t.text_primary)
        .when(!stopping && !drafted, |button| button.opacity(0.28))
        .when_some(about, |button, about| button.tooltip(ui::tooltip(about)))
        .on_click(move |_, window, cx| if stopping { on_stop(window, cx) } else { on_send(window, cx) })
        .child(if stopping {
            div().size(px(9.)).rounded(px(2.)).bg(t.surface_sunken).into_any_element()
        } else {
            ui::icon(Icon::ArrowUp, 15.).text_color(t.surface_sunken).into_any_element()
        })
}
