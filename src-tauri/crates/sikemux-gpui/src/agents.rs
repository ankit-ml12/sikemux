use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{div, px, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, Field, Group, IconButton, Row, Segmented};
use crate::workbench::{AgentRow, Workbench};

pub const TAB_RAIL_WIDTH: f32 = 288.;

pub fn tab_rail(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let project = bench.project();
    let agents = bench.listed_agents();
    let rows: Vec<_> = agents.iter().map(|agent| agent_row(bench, agent, cx).into_any_element()).collect();
    let recent: Vec<_> = project
        .map(|project| bench.recent_of(&project.id))
        .unwrap_or_default()
        .into_iter()
        .map(|recent| {
            Row::new(SharedString::from(format!("recent-{}", recent.id)), recent.title.clone())
                .mark(22., ui::mark(&recent.provider, 20.))
                .child(ui::mono(age(recent.active_at), 11., cx))
                .into_any_element()
        })
        .collect();
    let scope = vec![
        (Icon::Folder, SharedString::from(project.map(|project| project.name.clone()).unwrap_or_default())),
        (Icon::Stack, SharedString::from("All projects")),
    ];
    ui::panel(cx)
        .w(px(TAB_RAIL_WIDTH))
        .min_w(px(240.))
        .flex_none()
        .child(header(bench, cx))
        .children(bench.filter.as_ref().map(|filter| {
            div().flex_none().mt(px(10.)).mx(px(12.)).mb(px(2.)).child(Field::new(filter).icon(Icon::Search))
        }))
        .child(
            div()
                .flex_none()
                .pt(px(4.))
                .px(px(10.))
                .child(new_row("new-chat", Icon::Plus, "New chat", "⌘N", cx))
                .child(new_row("new-terminal", Icon::Terminal, "New terminal", "⌘T", cx))
                .child(new_row("new-browser", Icon::Globe, "New browser", "⌘⇧T", cx))
                .child(
                    div().mt(px(8.)).child(
                        Segmented::new("scope", scope, usize::from(bench.all_projects)).on_select(
                            cx.listener(|bench, all: &usize, _, cx| {
                                bench.all_projects = *all == 1;
                                cx.notify();
                            }),
                        ),
                    ),
                ),
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
                .child(div().flex().flex_col().gap(px(2.)).children(rows))
                .when(!recent.is_empty(), |scroll| scroll.child(Group::new("Recent", "").children(recent))),
        )
        .child(pages(cx))
}

fn header(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let launcher = bench.project().and_then(|project| {
        let id = project.launcher.as_deref()?;
        bench.view.as_ref()?.workspace.launchers.iter().find(|launcher| launcher.id == id)
    });
    let (provider, label) = launcher.map_or(("", "Agents"), |launcher| (launcher.provider.as_str(), launcher.label.as_str()));
    div()
        .flex_none()
        .pt(px(6.))
        .pr(px(6.))
        .pb(px(6.))
        .pl(px(10.))
        .border_b_1()
        .border_color(t.line_soft)
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
                        .child(ui::mark(provider, 16.))
                        .child(ui::text(label.to_owned(), 13., FontWeight::SEMIBOLD).text_color(t.text_primary)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .child(
                            IconButton::new("search", Icon::Search, "Filter recent chats")
                                .pressed(bench.filter.is_some())
                                .on_click(cx.listener(|bench, _, window, cx| {
                                    bench.toggle_filter(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(IconButton::new("start-agent", Icon::Plus, "New chat"))
                        .child(IconButton::new("hide-tab-rail", Icon::PanelRight, "Hide the agent rail")),
                ),
        )
}

fn new_row(id: &'static str, glyph: Icon, label: &'static str, keys: &'static str, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    Row::new(id, label)
        .mark(22., ui::icon(glyph, 15.).text_color(t.text_secondary).group_hover(ui::ROW, move |style| style.text_color(t.text_primary)))
        .quiet()
        .child(ui::kbd(keys, cx))
}

fn agent_row(bench: &Workbench, agent: &AgentRow, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let project = bench
        .all_projects
        .then(|| bench.projects().iter().find(|project| project.id == agent.project))
        .flatten()
        .map(|project| project.name.clone());
    let chosen = bench.agent().is_some_and(|front| front.id == agent.id);
    let (id, home) = (agent.id.clone(), agent.project.clone());
    Row::new(SharedString::from(format!("agent-{}", agent.id)), agent.title.clone())
        .mark(22., ui::mark(&agent.provider, 20.))
        .chosen(chosen)
        .when_some(project, |row, project| {
            row.subtitle(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .line_height(px(14.))
                    .child(ui::icon(Icon::Folder, 11.).text_color(t.live))
                    .child(ui::meta(project, cx)),
            )
        })
        .status(ui::status_mark(agent.status, cx))
        .on_click(cx.listener(move |bench, _, _, _| {
            if let Some(agent) = bench.agents_of(&home).into_iter().find(|agent| agent.id == id) {
                bench.activate_agent(&agent);
            }
        }))
        .children(agent.name.clone().map(|name| ui::meta(format!("@{name}"), cx).pl(px(8.)).pr(px(4.))))
        .children(counts(agent, cx))
}

fn counts(agent: &AgentRow, cx: &App) -> Option<impl IntoElement> {
    let t = Tokens::get(cx).clone();
    (agent.subagents > 0 || agent.tasks > 0).then(|| {
        let count = |glyph: Icon, size: f32, color, value: u32| {
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .child(ui::icon(glyph, size).text_color(color))
                .child(ui::text(value.to_string(), 11., FontWeight::MEDIUM).font_features(crate::theme::tabular()))
        };
        div()
            .flex()
            .gap(px(6.))
            .text_color(t.text_secondary)
            .when(agent.subagents > 0, |row| row.child(count(Icon::Agent, 10., t.live, agent.subagents)))
            .when(agent.tasks > 0, |row| row.child(count(Icon::Command, 11., t.warn, agent.tasks)))
    })
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

fn pages(cx: &App) -> impl IntoElement {
    let border = Tokens::get(cx).border;
    div()
        .flex_none()
        .h(px(35.))
        .px(px(8.))
        .py(px(4.))
        .flex()
        .justify_center()
        .gap(px(4.))
        .border_t_1()
        .border_color(border)
        .child(IconButton::new("page-agents", Icon::Agents, "Agents").width(32.).pressed(true))
        .child(IconButton::new("page-terminals", Icon::Terminal, "Terminals").width(32.))
        .child(IconButton::new("page-files", Icon::File, "Files").width(32.))
}
