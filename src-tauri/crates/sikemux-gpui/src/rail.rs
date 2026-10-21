use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};
use sikemux_wire::protocol::ProjectInfo;

use crate::theme;
use crate::ui::{agent_mark, group_header, icon, kbd, slot, text};
use crate::workbench::Workbench;

pub const SIDE_RAIL_WIDTH: f32 = 258.;

/// Every rail is a rounded panel over the shell.
pub fn panel(width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(theme::surface())
        .border_1()
        .border_color(theme::border())
        .rounded(theme::RADIUS_3)
}

pub fn side_rail(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    panel(SIDE_RAIL_WIDTH)
        .child(masthead(bench))
        .child(
            div()
                .id("side-rail-scroll")
                .flex_1()
                .min_h_0()
                .pt(px(10.))
                .px(px(10.))
                .pb(px(16.))
                .overflow_y_scrollbar()
                .child(projects(bench, cx))
                .child(empty_group("SSH", Some("⌘⇧S"), "no SSH hosts"))
                .child(empty_group("Plugins", None, "no plugins"))
                .child(empty_group("Terminals", None, "no terminals")),
        )
        .child(sign_in())
}

fn masthead(bench: &Workbench) -> impl IntoElement {
    let (release, channel) = split_version(&bench.version);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(40.))
        .pl(px(18.))
        .pr(px(8.))
        .border_b_1()
        .border_color(theme::border())
        .child(icon("icons/logo.svg", 14., theme::accent()))
        .child(text("Sikemux", 13., 600., theme::primary()).ml(px(1.)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .pt(px(1.))
                .font_family(theme::MONO)
                .font_features(theme::tabular())
                .text_size(px(10.))
                .text_color(theme::tertiary())
                .child(format!("v{release}"))
                .children(channel.map(|channel| {
                    div()
                        .px(px(4.))
                        .py(px(1.))
                        .border_1()
                        .border_color(theme::line())
                        .rounded(theme::RADIUS_1)
                        .font_family(theme::UI)
                        .font_weight(FontWeight::MEDIUM)
                        .line_height(px(12.))
                        .child(channel)
                })),
        )
        .child(
            div()
                .ml_auto()
                .child(header_action("icons/panel-left.svg")),
        )
}

pub fn header_action(path: &'static str) -> impl IntoElement {
    slot(26., icon(path, 15., theme::gray_700())).rounded(theme::RADIUS_2)
}

/// `0.4.0-nightly.10` as the release `0.4.0` and the channel `nightly 10`.
fn split_version(version: &str) -> (String, Option<String>) {
    match version.split_once('-') {
        Some((release, channel)) => (release.into(), Some(channel.replacen('.', " ", 1))),
        None => (version.into(), None),
    }
}

fn add_actions(keys: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(kbd(keys))
        .child(slot(18., icon("icons/plus.svg", 11., theme::tertiary())).rounded(theme::RADIUS_2))
}

fn projects(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let rows: Vec<AnyElement> = bench
        .projects()
        .iter()
        .map(|project| project_tree(bench, project, cx).into_any_element())
        .collect();
    div()
        .flex()
        .flex_col()
        .child(group_header("Projects", Some(add_actions("⌘O"))))
        .child(
            div()
                .mb(px(6.))
                .h(px(22.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(5.))
                .border_1()
                .border_dashed()
                .border_color(theme::border())
                .rounded(theme::RADIUS_2)
                .text_size(px(11.))
                .text_color(theme::tertiary())
                .child(icon("icons/plus.svg", 11., theme::tertiary()))
                .child("New space"),
        )
        .children(rows)
}

fn project_tree(
    bench: &Workbench,
    project: &ProjectInfo,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let active = bench.project.as_deref() == Some(project.id.as_str());
    let agents = bench.agents_of(&project.id);
    let id = project.id.clone();
    let row = div()
        .id(SharedString::from(format!("project-{}", project.id)))
        .h(px(28.))
        .pl(px(6.))
        .pr(px(4.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(theme::RADIUS_2)
        .text_color(if active { theme::primary() } else { theme::secondary() })
        .when(active, |row| row.bg(theme::chosen()))
        .hover(|style| style.bg(theme::hover()).text_color(theme::primary()))
        .on_click(cx.listener(move |bench, _, _, cx| {
            bench.select_project(Some(id.clone()));
            cx.notify();
        }))
        .child(slot(16., icon("icons/folder.svg", 13., theme::live())))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(13.))
                .line_height(px(13. * 1.2))
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .child(project.name.clone()),
        )
        .child(faces(&agents));
    div()
        .flex()
        .flex_col()
        .child(row)
        .when(active, |tree| tree.pb(px(4.)).child(children()))
}

/// Up to five agent marks, each overlapping the one before by 6px.
fn faces(agents: &[crate::workbench::AgentRow]) -> impl IntoElement {
    let shown = agents.len().min(5);
    div()
        .relative()
        .flex_none()
        .h(px(20.))
        .w(px(if shown == 0 { 0. } else { 20. + 14. * (shown - 1) as f32 }))
        .children(agents.iter().take(shown).enumerate().map(|(index, agent)| {
            slot(20., agent_mark(&agent.provider, 20.))
                .absolute()
                .left(px(14. * index as f32))
        }))
}

const SURFACES: [(&str, &str, &str); 6] = [
    ("icons/editor.svg", "Files", "⌘E"),
    ("icons/terminal.svg", "Terminal", "⌘⌥2"),
    ("icons/agent.svg", "Agents", "⌘⌥4"),
    ("icons/git.svg", "Git", "⌘⌥3"),
    ("icons/search.svg", "Search", "⌘⇧F"),
    ("icons/board.svg", "Agent board", "⌘⌥6"),
];

/// The project's surfaces, hung off a spine with a tick to each.
fn children() -> impl IntoElement {
    let last = SURFACES.len() - 1;
    div()
        .flex()
        .flex_col()
        .ml(px(20.))
        .pt(px(2.))
        .pb(px(2.))
        .pr(px(6.))
        .children(SURFACES.iter().enumerate().map(move |(index, (path, label, keys))| {
            let tail = index == last;
            div()
                .relative()
                .h(px(25.))
                .py(px(2.))
                .pr(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_color(theme::tertiary())
                .child(if tail {
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(9.))
                        .h(px(13.))
                        .border_l_1()
                        .border_b_1()
                        .border_color(theme::tree_spine())
                        .rounded_bl(px(4.))
                } else {
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(1.))
                        .bg(theme::tree_spine())
                })
                .when(!tail, |row| {
                    row.child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(12.))
                            .w(px(9.))
                            .h(px(1.))
                            .bg(theme::tree_tick()),
                    )
                })
                .child(div().w(px(9.)).flex_none())
                .child(slot(16., icon(path, 13., theme::tertiary())).w(px(16.)))
                .child(
                    div()
                        .text_size(px(12.5))
                        .line_height(px(15.))
                        .font_weight(FontWeight::MEDIUM)
                        .whitespace_nowrap()
                        .child(*label),
                )
                .child(div().ml_auto().opacity(0.5).child(kbd(keys)))
        }))
}

fn empty_group(label: &str, keys: Option<&'static str>, empty: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .mt(px(16.))
        .child(group_header(label, keys.map(add_actions)))
        .child(
            div()
                .p(px(12.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(theme::tertiary())
                .child(empty),
        )
}

fn sign_in() -> impl IntoElement {
    div()
        .flex_none()
        .mx(px(10.))
        .mb(px(10.))
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .rounded(theme::RADIUS_2)
        .bg(gpui_kit::rgba(0xe7e5ef0a))
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(12. * 1.4))
                .text_color(theme::secondary())
                .child("Sign in to reach this host from your phone and other devices."),
        )
        .child(
            div()
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .rounded(theme::RADIUS_2)
                .bg(theme::accent())
                .text_color(theme::sunken())
                .font_weight(FontWeight::MEDIUM)
                .child(icon("icons/log-in.svg", 13., theme::sunken()))
                .child("Sign in"),
        )
}
