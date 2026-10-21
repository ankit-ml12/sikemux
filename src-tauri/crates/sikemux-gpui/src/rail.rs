use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled,
};
use sikemux_wire::protocol::ProjectInfo;

use crate::icons::Icon;
use crate::theme::{Tokens, RADIUS_1, RADIUS_2};
use crate::ui::{self, Button, Group, IconButton, Row, Variant};
use crate::workbench::{AgentRow, Workbench};

pub const SIDE_RAIL_WIDTH: f32 = 258.;

pub fn side_rail(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let projects: Vec<_> = bench
        .projects()
        .iter()
        .map(|project| project_tree(bench, project, cx).into_any_element())
        .collect();
    ui::panel(cx)
        .w(px(SIDE_RAIL_WIDTH))
        .flex_none()
        .child(masthead(bench, cx))
        .child(
            div()
                .id("side-rail-scroll")
                .flex_1()
                .min_h_0()
                .pt(px(10.))
                .px(px(10.))
                .pb(px(16.))
                .overflow_y_scrollbar()
                .child(
                    Group::new("Projects", "no projects")
                        .first()
                        .actions(add_actions("add-project", "⌘O", cx))
                        .child(
                            div()
                                .mb(px(6.))
                                .child(Button::new("new-space", "New space", Variant::Dashed).icon(Icon::Plus)),
                        )
                        .children(projects),
                )
                .child(Group::new("SSH", "no SSH hosts").actions(add_actions("add-ssh", "⌘⇧S", cx)))
                .child(Group::new("Plugins", "no plugins"))
                .child(Group::new("Terminals", "no terminals")),
        )
        .child(sign_in(cx))
}

fn masthead(bench: &Workbench, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
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
        .border_color(t.border)
        .child(ui::icon(Icon::Logo, 14.).text_color(t.accent))
        .child(ui::text("Sikemux", 13., FontWeight::SEMIBOLD).text_color(t.text_primary).ml(px(1.)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .pt(px(1.))
                .child(ui::mono(format!("v{release}"), 10., cx))
                .children(channel.map(|channel| {
                    ui::text(channel, 10., FontWeight::MEDIUM)
                        .px(px(4.))
                        .py(px(1.))
                        .line_height(px(12.))
                        .border_1()
                        .border_color(t.line)
                        .rounded(RADIUS_1)
                        .text_color(t.text_tertiary)
                })),
        )
        .child(div().ml_auto().child(IconButton::new("hide-side-rail", Icon::PanelLeft, "Hide the side rail")))
}

/// `0.4.0-nightly.10` as the release `0.4.0` and the channel `nightly 10`.
fn split_version(version: &str) -> (String, Option<String>) {
    match version.split_once('-') {
        Some((release, channel)) => (release.into(), Some(channel.replacen('.', " ", 1))),
        None => (version.into(), None),
    }
}

fn add_actions(id: &'static str, keys: &'static str, cx: &App) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(ui::kbd(keys, cx))
        .child(IconButton::new(id, Icon::Plus, "Add").small())
}

fn project_tree(bench: &Workbench, project: &ProjectInfo, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let active = bench.project().is_some_and(|front| front.id == project.id);
    let agents = bench.agents_of(&project.id);
    let id = project.id.clone();
    div()
        .flex()
        .flex_col()
        .child(
            Row::new(SharedString::from(format!("project-{}", project.id)), project.name.clone())
                .mark(16., ui::icon(Icon::Folder, 13.).text_color(t.live))
                .quiet()
                .bold(active)
                .chosen(active)
                .padding_right(4.)
                .on_click(cx.listener(move |bench, _, _, _| bench.activate_project(&id)))
                .child(faces(&agents)),
        )
        .when(active, |tree| tree.pb(px(4.)).child(surfaces(cx)))
}

/// Up to five agent marks, each overlapping the one before by 6px.
fn faces(agents: &[AgentRow]) -> impl IntoElement {
    let shown = agents.len().min(5);
    div()
        .relative()
        .flex_none()
        .h(px(20.))
        .w(px(if shown == 0 { 0. } else { 20. + 14. * (shown - 1) as f32 }))
        .children(agents.iter().take(shown).enumerate().map(|(index, agent)| {
            ui::mark_box(20., ui::mark(&agent.provider, 20.))
                .absolute()
                .left(px(14. * index as f32))
        }))
}

const SURFACES: [(Icon, &str, &str); 6] = [
    (Icon::Editor, "Files", "⌘E"),
    (Icon::Terminal, "Terminal", "⌘⌥2"),
    (Icon::Agent, "Agents", "⌘⌥4"),
    (Icon::Git, "Git", "⌘⌥3"),
    (Icon::Search, "Search", "⌘⇧F"),
    (Icon::Board, "Agent board", "⌘⌥6"),
];

/// The project's surfaces, hung off a spine with a tick to each.
fn surfaces(cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let last = SURFACES.len() - 1;
    div()
        .flex()
        .flex_col()
        .ml(px(20.))
        .py(px(2.))
        .pr(px(6.))
        .children(SURFACES.iter().enumerate().map(move |(index, (glyph, label, keys))| {
            let tail = index == last;
            let spine = div().absolute().left_0().top_0();
            div()
                .relative()
                .h(px(25.))
                .pr(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(RADIUS_2)
                .text_color(t.text_tertiary)
                .child(if tail {
                    spine.w(px(9.)).h(px(13.)).border_l_1().border_b_1().border_color(t.tree_spine).rounded_bl(px(4.))
                } else {
                    spine.bottom_0().w(px(1.)).bg(t.tree_spine)
                })
                .when(!tail, |row| row.child(div().absolute().left_0().top(px(12.)).w(px(9.)).h(px(1.)).bg(t.tree_tick)))
                .child(div().w(px(9.)).flex_none())
                .child(ui::mark_box(16., ui::icon(*glyph, 13.).text_color(t.text_tertiary)))
                .child(ui::text(*label, 12.5, FontWeight::MEDIUM))
                .child(div().ml_auto().opacity(0.5).child(ui::kbd(keys, cx)))
        }))
}

fn sign_in(cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx);
    div()
        .flex_none()
        .mx(px(10.))
        .mb(px(10.))
        .p(px(8.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .rounded(RADIUS_2)
        .bg(gpui_kit::Hsla { a: 0.04, ..t.text_primary })
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(12. * 1.4))
                .text_color(t.text_secondary)
                .child("Sign in to reach this host from your phone and other devices."),
        )
        .child(Button::new("sign-in", "Sign in", Variant::Primary).icon(Icon::LogIn))
}
