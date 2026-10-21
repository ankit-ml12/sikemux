use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rgba, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled,
};

use gpui_kit::component::input::Input;

use crate::rail::{header_action, panel};
use crate::theme;
use crate::ui::{agent_mark, group_header, icon, kbd, row, slot, status_mark, text, title};
use crate::workbench::{AgentRow, Workbench};

pub const TAB_RAIL_WIDTH: f32 = 288.;

pub fn tab_rail(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let project = bench.project();
    let agents = bench.listed_agents();
    let recent: Vec<AnyElement> = project
        .map(|project| bench.recent_of(&project.id))
        .unwrap_or_default()
        .into_iter()
        .map(|recent| recent_row(&recent.provider, &recent.title, recent.active_at).into_any_element())
        .collect();
    let rows: Vec<AnyElement> = agents
        .iter()
        .map(|agent| agent_row(bench, agent, cx).into_any_element())
        .collect();
    panel(TAB_RAIL_WIDTH)
        .min_w(px(240.))
        .child(header(&agents, cx))
        .children(bench.filter.as_ref().map(|filter| {
            div()
                .flex_none()
                .h(px(34.))
                .mt(px(10.))
                .mx(px(12.))
                .mb(px(2.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(7.))
                .border_1()
                .border_color(theme::line())
                .rounded(theme::RADIUS_2)
                .bg(rgba(0x19191e9e))
                .child(icon("icons/search.svg", 12., theme::tertiary()))
                .child(
                    div()
                        .flex_1()
                        .font_family(theme::MONO)
                        .text_size(px(11.))
                        .child(Input::new(filter).appearance(false).bordered(false)),
                )
        }))
        .child(
            div()
                .flex_none()
                .pt(px(4.))
                .px(px(10.))
                .child(new_row("icons/plus.svg", "New chat", "⌘N"))
                .child(new_row("icons/terminal.svg", "New terminal", "⌘T"))
                .child(new_row("icons/globe.svg", "New browser", "⌘⇧T"))
                .child(scope(project.map(|project| project.name.as_str()).unwrap_or(""), bench.all_projects, cx)),
        )
        .child(
            div()
                .id("tab-rail-scroll")
                .flex_1()
                .min_h_0()
                .pt(px(8.))
                .px(px(10.))
                .pb(px(16.))
                .overflow_y_scrollbar()
                .child(div().flex().flex_col().children(rows))
                .when(!recent.is_empty(), |scroll| {
                    scroll.child(
                        div()
                            .mt(px(16.))
                            .flex()
                            .flex_col()
                            .child(group_header("Recent", None))
                            .children(recent),
                    )
                }),
        )
        .child(pages())
}

fn header(agents: &[AgentRow], cx: &mut Context<Workbench>) -> impl IntoElement {
    let provider = agents.first().map(|agent| agent.provider.as_str()).unwrap_or("claude");
    div()
        .flex_none()
        .pt(px(6.))
        .pr(px(6.))
        .pb(px(6.))
        .pl(px(10.))
        .border_b_1()
        .border_color(theme::line_soft())
        .child(
            div()
                .min_h(px(28.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .child(agent_mark(provider, 16.))
                        .child(text(label(provider), 13., 600., theme::primary())),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .child(
                            div()
                                .id("search")
                                .on_click(cx.listener(|bench, _, window, cx| {
                                    bench.toggle_filter(window, cx);
                                    cx.notify();
                                }))
                                .child(header_action("icons/search.svg")),
                        )
                        .child(header_action("icons/plus.svg"))
                        .child(header_action("icons/panel-right.svg")),
                ),
        )
}

fn label(provider: &str) -> &'static str {
    match provider {
        "claude" => "Claude",
        "codex" => "Codex",
        _ => "Agents",
    }
}

fn new_row(path: &'static str, label: &'static str, keys: &'static str) -> impl IntoElement {
    row(28., false)
        .child(slot(22., icon(path, 15., theme::secondary())))
        .child(div().flex_1().text_color(theme::secondary()).child(label))
        .child(kbd(keys))
}

fn scope(project: &str, all: bool, cx: &mut Context<Workbench>) -> impl IntoElement {
    let half = |id: &'static str, path: &'static str, label: SharedString, chosen: bool, to: bool| {
        div()
            .id(id)
            .flex_1()
            .h_full()
            .px(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(7.))
            .rounded(px(3.))
            .when(chosen, |half| half.bg(theme::active()))
            .text_size(px(11.))
            .text_color(if chosen { theme::primary() } else { theme::tertiary() })
            .on_click(cx.listener(move |bench, _, _, cx| {
                bench.all_projects = to;
                cx.notify();
            }))
            .child(icon(path, 13., if chosen { theme::primary() } else { theme::tertiary() }))
            .child(div().overflow_hidden().text_ellipsis().whitespace_nowrap().child(label))
    };
    let this = half("scope-project", "icons/folder.svg", SharedString::from(project.to_owned()), !all, false);
    let every = half("scope-all", "icons/stack.svg", "All projects".into(), all, true);
    div()
        .mt(px(8.))
        .h(px(30.))
        .p(px(3.))
        .flex()
        .rounded(theme::RADIUS_2)
        .border_1()
        .border_color(theme::border())
        .child(this)
        .child(every)
}

fn agent_row(bench: &Workbench, agent: &AgentRow, cx: &mut Context<Workbench>) -> impl IntoElement {
    let active = bench.agent.as_deref() == Some(agent.id.as_str());
    let status = status_mark(agent.status);
    let project = bench
        .all_projects
        .then(|| bench.projects().iter().find(|project| project.id == agent.project))
        .flatten()
        .map(|project| project.name.clone());
    let id = agent.id.clone();
    row(if project.is_some() { 40. } else { 28. }, active)
        .id(SharedString::from(format!("agent-{}", agent.id)))
        .relative()
        .my(px(1.))
        .pr(px(if status.is_some() { 28. } else { 8. }))
        .when(project.is_some(), |row| row.items_start().pt(px(4.)))
        .on_click(cx.listener(move |bench, _, _, cx| {
            bench.agent = Some(id.clone());
            cx.notify();
        }))
        .child(slot(22., agent_mark(&agent.provider, 20.)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(title(agent.title.clone()).line_height(px(18.)))
                .children(project.map(|project| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(theme::tertiary())
                        .child(icon("icons/folder.svg", 11., rgba(0x82c39fff)))
                        .child(project)
                })),
        )
        .children(agent.name.clone().map(|name| {
            div()
                .pl(px(8.))
                .pr(px(4.))
                .text_size(px(11.))
                .text_color(theme::tertiary())
                .child(format!("@{name}"))
        }))
        .children(counts(agent))
        .children(status.map(|mark| div().absolute().right(px(4.)).child(slot(20., mark))))
}

fn counts(agent: &AgentRow) -> Option<Div> {
    if agent.subagents == 0 && agent.tasks == 0 {
        return None;
    }
    let count = |path: &'static str, size: f32, color, value: u32| {
        div()
            .flex()
            .items_center()
            .gap(px(3.))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .child(icon(path, size, color))
            .child(value.to_string())
    };
    Some(
        div()
            .flex()
            .gap(px(6.))
            .font_features(theme::tabular())
            .when(agent.subagents > 0, |row| {
                row.child(count("icons/agent.svg", 10., theme::live(), agent.subagents))
            })
            .when(agent.tasks > 0, |row| row.child(count("icons/command.svg", 11., theme::warn(), agent.tasks))),
    )
}

fn recent_row(provider: &str, name: &str, at: u64) -> impl IntoElement {
    row(28., false)
        .child(slot(22., agent_mark(provider, 20.)))
        .child(title(name.to_owned()))
        .child(
            div()
                .font_family(theme::MONO)
                .font_features(theme::tabular())
                .text_size(px(11.))
                .text_color(theme::tertiary())
                .child(age(at)),
        )
}

/// How long ago, in a tight column: `now`, `5m`, `3h`, `4d`, `2mo`, `1y`.
fn age(at: u64) -> String {
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let seconds = now.saturating_sub(at) / 1000;
    match seconds {
        0..60 => "now".into(),
        60..3600 => format!("{}m", seconds / 60),
        3600..86_400 => format!("{}h", seconds / 3600),
        86_400..2_592_000 => format!("{}d", seconds / 86_400),
        2_592_000..31_536_000 => format!("{}mo", seconds / 2_592_000),
        _ => format!("{}y", seconds / 31_536_000),
    }
}

fn pages() -> impl IntoElement {
    let page = |path: &'static str, chosen: bool| {
        slot(26., icon(path, 15., if chosen { theme::primary() } else { theme::tertiary() }))
            .w(px(32.))
            .rounded(theme::RADIUS_2)
            .when(chosen, |page| page.bg(theme::chosen()))
    };
    div()
        .flex_none()
        .h(px(35.))
        .px(px(8.))
        .py(px(4.))
        .flex()
        .justify_center()
        .gap(px(4.))
        .border_t_1()
        .border_color(rgba(0x222227ff))
        .child(page("icons/agents.svg", true))
        .child(page("icons/terminal.svg", false))
        .child(page("icons/file.svg", false))
}
