use gpui_kit::{div, px, AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, WindowControlArea};
use sikemux_wire::protocol::{Pane, PaneKind, Tab as LayoutTab};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, IconButton, Tab};
use crate::workbench::Workbench;

/// The bar over the stage: the project's tabs, then the clock.
pub fn top_bar(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let tabs: Vec<_> = bench
        .project()
        .map(|project| {
            let front = bench.tab().map(|tab| tab.id.clone());
            bench
                .tabs(&project.id)
                .iter()
                .map(|tab| strip_tab(bench, &project.id, tab, front.as_deref() == Some(tab.id.as_str()), cx))
                .collect()
        })
        .unwrap_or_default();
    div()
        .id("top-bar")
        .flex()
        .flex_none()
        .items_center()
        .h(px(34.))
        .window_control_area(WindowControlArea::Drag)
        .child(div().w(px(80.)).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .pl(px(20.))
                .pr(px(12.))
                .flex()
                .items_center()
                .overflow_hidden()
                .child(div().h_full().min_w_0().overflow_hidden().px(px(4.)).flex().items_center().gap(px(4.)).children(tabs))
                .child(IconButton::new("new-tab", Icon::Plus, "New tab").glyph(13.)),
        )
        .child(div().flex().items_center().gap(px(8.)).pr(px(16.)).child(clock(cx)).child(IconButton::new("focus", Icon::Focus, "Focus mode").width(30.)))
}

fn strip_tab(bench: &Workbench, project: &str, tab: &LayoutTab, active: bool, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let (project_id, tab_id) = (project.to_owned(), tab.id.clone());
    let base = Tab::new(SharedString::from(format!("tab-{}", tab.id)), label(bench, tab))
        .active(active)
        .on_click(cx.listener(move |bench, _, _, _| bench.activate_tab(&project_id, &tab_id)));
    let view = first_view(&tab.pane);
    match view {
        Some((PaneKind::Agent, target)) => match bench.agents_of(project).into_iter().find(|agent| &agent.id == target) {
            Some(agent) => base
                .mark(ui::mark(&agent.provider, 19.))
                .note(agent.name.as_ref().map(|name| SharedString::from(format!("@{name}"))))
                .tail(ui::status_mark(agent.status, cx)),
            None => base,
        },
        Some((kind, _)) => base.mark(ui::icon(kind_icon(kind), 13.).text_color(if kind == PaneKind::Terminal { t.live } else { t.text_tertiary })),
        None => base,
    }
    .into_any_element()
}

/// A tab's name: the person's, else what its first view shows.
fn label(bench: &Workbench, tab: &LayoutTab) -> SharedString {
    if let Some(title) = &tab.title {
        return title.clone().into();
    }
    match first_view(&tab.pane) {
        Some((PaneKind::Agent, target)) => bench
            .view
            .as_ref()
            .and_then(|view| view.workspace.agents.iter().find(|agent| &agent.id == target))
            .map_or_else(|| "Agent".into(), |agent| agent.title.clone().into()),
        Some((PaneKind::Terminal, target)) => bench
            .session(target)
            .and_then(|session| session.window_title.clone().or_else(|| session.title.clone()))
            .map_or_else(|| "Terminal".into(), SharedString::from),
        Some((kind, _)) => kind_label(kind).into(),
        None => "".into(),
    }
}

pub fn first_view(pane: &Pane) -> Option<(PaneKind, &String)> {
    match pane {
        Pane::View { kind, target, .. } => Some((*kind, target)),
        Pane::Split { panes, .. } => panes.iter().find_map(first_view),
    }
}

pub fn kind_icon(kind: PaneKind) -> Icon {
    match kind {
        PaneKind::Agent => Icon::Agent,
        PaneKind::Terminal => Icon::Terminal,
        PaneKind::Files => Icon::Editor,
        PaneKind::Git => Icon::Git,
        PaneKind::Search => Icon::Search,
        PaneKind::Board => Icon::Board,
        PaneKind::Browser => Icon::Globe,
        PaneKind::Plugin => Icon::Window,
    }
}

fn kind_label(kind: PaneKind) -> &'static str {
    match kind {
        PaneKind::Agent => "Agent",
        PaneKind::Terminal => "Terminal",
        PaneKind::Files => "Files",
        PaneKind::Git => "Git",
        PaneKind::Search => "Search",
        PaneKind::Board => "Agent board",
        PaneKind::Browser => "Browser",
        PaneKind::Plugin => "Plugin",
    }
}

fn clock(cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx);
    let now = chrono::Local::now();
    div()
        .flex()
        .items_baseline()
        .font_features(crate::theme::tabular())
        .child(ui::text(now.format("%-I:%M").to_string(), 12., FontWeight::SEMIBOLD).text_color(t.text_secondary))
        .child(ui::text(now.format("%p").to_string(), 10., FontWeight::SEMIBOLD).ml(px(4.)).text_color(t.text_tertiary))
}
