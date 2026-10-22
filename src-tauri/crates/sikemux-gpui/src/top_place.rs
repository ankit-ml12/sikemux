//! Where the window is, in the middle of the top bar: the chosen host, then the project in
//! front, each a picker.

use std::rc::Rc;

use gpui_kit::base::{Button as BaseButton, Popover};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    div, px, Anchor, AnyElement, App, AppContext, Context, Entity, FontWeight, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window,
};

use crate::hosts::HostChoice;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, Menu, MenuItem};
use crate::workbench::{Strip, Workbench};

/// A project picker longer than this gets a field to find one by name.
const FIND_PAST: usize = 8;

/// Picks a choice by its key and closes the picker.
type Pick = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(Clone)]
struct Choice {
    key: SharedString,
    label: SharedString,
    mark: (Icon, Hsla),
}

/// The host picker, then the project picker when a project is in front.
pub fn view(bench: &Workbench, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let host_mark = |choice: &HostChoice| match choice {
        HostChoice::This => (Icon::Laptop, t.text_secondary),
        HostChoice::Account(_) => (Icon::Server, t.host),
        HostChoice::Ssh(_) => (Icon::Server, t.ssh),
    };
    let chosen = bench.chosen_host();
    let hosts: Vec<(HostChoice, Choice)> = bench
        .host_choices()
        .into_iter()
        .map(|option| {
            let choice = Choice {
                key: option.choice.key(),
                label: option.label,
                mark: host_mark(&option.choice),
            };
            (option.choice, choice)
        })
        .collect();
    let shown = hosts
        .iter()
        .find(|(choice, _)| *choice == chosen)
        .map_or_else(
            || Choice {
                key: chosen.key(),
                label: "This host".into(),
                mark: host_mark(&chosen),
            },
            |(_, shown)| shown.clone(),
        );
    let this = cx.entity().downgrade();
    let host = picker(
        "place-host",
        shown,
        hosts.iter().map(|(_, choice)| choice.clone()).collect(),
        None,
        move |key, cx| {
            let Some((choice, _)) = hosts.iter().find(|(_, shown)| shown.key == *key) else {
                return;
            };
            let choice = choice.clone();
            this.update(cx, |bench, cx| bench.choose_host(choice, cx))
                .ok();
        },
        cx,
    );
    let projects: Vec<Choice> = bench
        .projects()
        .iter()
        .map(|project| Choice {
            key: project.id.clone().into(),
            label: project.name.clone().into(),
            mark: (Icon::Folder, t.live),
        })
        .collect();
    let front = match bench.strip() {
        Some(Strip::Project(id)) => projects
            .iter()
            .find(|project| project.key.as_ref() == id.as_str())
            .cloned(),
        _ => None,
    };
    let this = cx.entity().downgrade();
    // An SSH host without Sikemux has no projects, only the one thing that sets it up (ADR 0019).
    let set_up_alias = match &chosen {
        HostChoice::Ssh(alias) => Some(alias.clone()),
        HostChoice::Account(key) => bench
            .read("hosts")
            .and_then(|hosts| hosts.as_array())
            .and_then(|hosts| hosts.iter().find(|host| host["id"] == key.as_str()))
            .and_then(|host| host["ssh"].as_str())
            .filter(|alias| bench.set_up_of(alias).is_some())
            .map(str::to_owned),
        HostChoice::This => None,
    };
    let project = set_up_alias.and_then(|alias| set_up_offer(bench, &alias, cx));
    let project = project.or_else(|| {
        front.map(|front| {
            let find = (projects.len() > FIND_PAST).then_some("Find a project");
            picker(
                "place-project",
                front,
                projects,
                find,
                move |key, cx| {
                    this.update(cx, |bench, _| bench.activate_project(key)).ok();
                },
                cx,
            )
        })
    });
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(2.))
        .child(host)
        .children(
            project
                .map(|project| {
                    [
                        div()
                            .px(px(2.))
                            .child(ui::icon(Icon::Chevron, 11.).text_color(t.text_tertiary))
                            .into_any_element(),
                        project,
                    ]
                })
                .into_iter()
                .flatten(),
        )
        .into_any_element()
}

/// The button that sets Sikemux up on the SSH host `alias` as it needs, saying so while it runs.
fn set_up_offer(bench: &Workbench, alias: &str, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let set_up = bench.set_up_of(alias)?;
    let running = bench.setting_up(alias);
    let label = match running {
        Some(running) => running.running(alias),
        None => set_up.offer(alias),
    };
    let alias = alias.to_owned();
    Some(
        BaseButton::new("place-set-up")
            .accessibility_label(label.clone())
            .disabled(running.is_some())
            .h(px(26.))
            .max_w(px(260.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded_full()
            .text_size(crate::theme::ui_text(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if running.is_some() {
                t.text_tertiary
            } else {
                t.text_primary
            })
            .hover(move |style| style.bg(t.surface_chosen))
            .child(mark((set_up.icon(), t.text_secondary)))
            .child(div().min_w_0().truncate().child(label))
            .on_click(cx.listener(move |bench, _, _, cx| bench.set_up_host(&alias, set_up, cx)))
            .into_any_element(),
    )
}

/// A pill naming `shown`, lifted only on hover or while open, opening a menu of `choices`,
/// with a field over it that finds one by name when `find` gives it its placeholder.
fn picker(
    id: &'static str,
    shown: Choice,
    choices: Vec<Choice>,
    find: Option<&'static str>,
    on_choose: impl Fn(&SharedString, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let chosen = shown.key.clone();
    let trigger = BaseButton::new(SharedString::from(format!("{id}-pill")))
        .accessibility_label(shown.label.clone())
        .h(px(26.))
        .max_w(px(220.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(crate::theme::ui_text(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text_primary)
        .hover(move |style| style.bg(t.surface_chosen))
        .styles(move |styles| styles.selected(move |style| style.bg(t.surface_chosen)))
        .child(mark(shown.mark))
        .child(div().min_w_0().truncate().child(shown.label));
    let (choices, on_choose) = (Rc::new(choices), Rc::new(on_choose));
    Popover::new(id)
        .anchor(Anchor::TopLeft)
        .offset(px(4.))
        .trigger(trigger)
        .content(move |_, window, cx| {
            let popover = cx.entity();
            let pick: Pick = {
                let on_choose = on_choose.clone();
                Rc::new(move |key, window, cx| {
                    on_choose(key, cx);
                    popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                })
            };
            match find {
                Some(placeholder) => {
                    let (choices, chosen) = (choices.clone(), chosen.clone());
                    window
                        .use_keyed_state(
                            SharedString::from(format!("{id}-find")),
                            cx,
                            move |window, cx| {
                                Find::new(id, placeholder, choices, chosen, pick, window, cx)
                            },
                        )
                        .into_any_element()
                }
                None => Menu::new()
                    .child(rows(id, choices.iter(), &chosen, &pick))
                    .into_any_element(),
            }
        })
        .into_any_element()
}

/// The menu's rows for `choices`, `chosen` ticked, scrolling past a screenful.
fn rows<'a>(
    id: &'static str,
    choices: impl Iterator<Item = &'a Choice>,
    chosen: &SharedString,
    pick: &Pick,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("{id}-choices")))
        .max_h(px(360.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(1.))
        .children(choices.map(|choice| {
            let (pick, key) = (pick.clone(), choice.key.clone());
            MenuItem::new(
                SharedString::from(format!("{id}-{}", choice.key)),
                choice.label.clone(),
            )
            .mark(mark(choice.mark))
            .checked(choice.key == *chosen)
            .on_click(move |_, window, cx| pick(&key, window, cx))
        }))
}

/// An open picker with a field over its rows that keeps those whose name holds what is
/// typed; Return picks the first of them.
struct Find {
    id: &'static str,
    query: Entity<InputState>,
    choices: Rc<Vec<Choice>>,
    chosen: SharedString,
    pick: Pick,
    _typing: Subscription,
}

impl Find {
    fn new(
        id: &'static str,
        placeholder: &'static str,
        choices: Rc<Vec<Choice>>,
        chosen: SharedString,
        pick: Pick,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        query.update(cx, |query, cx| query.focus(window, cx));
        let typing =
            cx.subscribe_in(
                &query,
                window,
                |find: &mut Self, _, event, window, cx| match event {
                    InputEvent::Change => cx.notify(),
                    InputEvent::PressEnter { .. } => {
                        let first = find.found(cx).first().map(|choice| choice.key.clone());
                        if let Some(key) = first {
                            let pick = find.pick.clone();
                            pick(&key, window, cx);
                        }
                    }
                    _ => {}
                },
            );
        Self {
            id,
            query,
            choices,
            chosen,
            pick,
            _typing: typing,
        }
    }

    fn found(&self, cx: &App) -> Vec<&Choice> {
        let words = self.query.read(cx).value().trim().to_lowercase();
        self.choices
            .iter()
            .filter(|choice| choice.label.to_lowercase().contains(&words))
            .collect()
    }
}

impl Render for Find {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let found = self.found(cx);
        Menu::new()
            .child(
                div()
                    .pb(px(4.))
                    .child(ui::Field::new(&self.query).icon(Icon::Search)),
            )
            .child(rows(self.id, found.into_iter(), &self.chosen, &self.pick))
    }
}

fn mark((icon, color): (Icon, Hsla)) -> impl IntoElement {
    ui::icon(icon, 13.).text_color(color)
}
