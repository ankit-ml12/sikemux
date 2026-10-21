//! A tab's views side by side, as the core's layout splits them.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, App, AppContext, Bounds, Context, CursorStyle, DragMoveEvent, EmptyView,
    Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use serde_json::json;
use sikemux_wire::protocol::{Pane, PaneKind, SplitDir, Tab};

use crate::drag::DraggedTab;
use crate::workbench::{Strip, Workbench};
use crate::{desk, home, terminal, ui};

/// The room between two panes, which is also where a divider is dragged.
pub const GAP: f32 = 10.;
/// The least a pane is squeezed to by a divider.
const MIN_PANE: f32 = 160.;

/// What only this window decides about a tab's views.
#[derive(Default)]
pub struct Panes {
    /// The view the person last clicked into, by id.
    pub focused: Option<String>,
    resizing: Option<Resizing>,
    /// Where a tab dragged over a view would go beside it.
    aimed: Option<Aimed>,
}

/// A tab dragged over a view: the side of the view it would take, which the view lights.
#[derive(Clone, PartialEq)]
struct Aimed {
    tab: String,
    pane: String,
    side: Side,
    from: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn word(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Top => "top",
            Side::Bottom => "bottom",
        }
    }
}

/// A split tab holds at most three views side by side, or two stacked.
const MOST_SIDE_BY_SIDE: usize = 3;

/// Where one more view can go: any side of a lone view, then only across while a row of
/// views has room.
fn open_sides(root: &Pane) -> Vec<Side> {
    match root {
        Pane::View { .. } => vec![Side::Left, Side::Right, Side::Top, Side::Bottom],
        Pane::Split { dir, panes, .. } => {
            let flat = panes.iter().all(|pane| matches!(pane, Pane::View { .. }));
            match *dir == SplitDir::Row && flat && panes.len() < MOST_SIDE_BY_SIDE {
                true => vec![Side::Left, Side::Right],
                false => Vec::new(),
            }
        }
    }
}

/// A divider being dragged, and the sizes it gives its split until the core keeps them.
struct Resizing {
    tab: String,
    split: String,
    /// The split's sizes as the core had them when the drag began.
    from: Vec<u16>,
    sizes: Vec<u16>,
}

#[derive(Clone)]
struct Divider {
    tab: String,
    split: String,
    at: usize,
}

impl Panes {
    /// The sizes to draw `split` at: the core's, or a drag's while the core has not answered it.
    fn sizes<'a>(&'a self, split: &str, kept: &'a [u16]) -> &'a [u16] {
        match &self.resizing {
            Some(resizing) if resizing.split == split && resizing.from == kept => &resizing.sizes,
            _ => kept,
        }
    }
}

pub fn tab(
    bench: &Workbench,
    tab: &Tab,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    pane(bench, &tab.id, &tab.pane, window, cx)
}

fn pane(
    bench: &Workbench,
    tab: &str,
    pane: &Pane,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    match pane {
        Pane::View { id, kind, target } => {
            let focused = id.clone();
            let (aiming, dropping) = ((tab.to_owned(), id.clone()), id.clone());
            let aimed = bench
                .panes
                .aimed
                .as_ref()
                .filter(|aimed| aimed.pane == *id && cx.has_active_drag())
                .map(|aimed| aimed.side);
            div()
                .id(SharedString::from(id.clone()))
                .relative()
                .flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .capture_any_mouse_down(
                    cx.listener(move |bench, _, _, _| bench.panes.focused = Some(focused.clone())),
                )
                .on_drag_move(cx.listener(
                    move |bench, event: &DragMoveEvent<DraggedTab>, _, cx| {
                        let (tab, pane) = &aiming;
                        bench.aim_split(tab, pane, event, cx);
                    },
                ))
                .on_drop(cx.listener(move |bench, dropped: &DraggedTab, _, cx| {
                    bench.drop_split(&dropping, dropped, cx);
                }))
                .child(view(bench, tab, id, *kind, target, window, cx))
                .children(aimed.map(|side| preview(side, cx)))
                .into_any_element()
        }
        Pane::Split {
            id,
            dir,
            panes,
            sizes,
        } => split(bench, tab, id, *dir, panes, sizes, window, cx),
    }
}

/// The part of a view a dropped tab would take, lit while the tab is over it.
fn preview(side: Side, cx: &App) -> impl IntoElement {
    let t = crate::theme::Tokens::get(cx);
    let across = matches!(side, Side::Left | Side::Right);
    div()
        .absolute()
        .when(side != Side::Right, |half| half.left_0())
        .when(side == Side::Right, |half| half.right_0())
        .when(side != Side::Bottom, |half| half.top_0())
        .when(side == Side::Bottom, |half| half.bottom_0())
        .when(across, |half| half.w(gpui_kit::relative(0.5)).h_full())
        .when(!across, |half| half.h(gpui_kit::relative(0.5)).w_full())
        .rounded(crate::theme::RADIUS_3)
        .bg(Hsla {
            a: 0.14,
            ..t.accent
        })
        .border_2()
        .border_color(Hsla {
            a: 0.55,
            ..t.accent
        })
}

fn view(
    bench: &Workbench,
    tab: &str,
    id: &str,
    kind: PaneKind,
    target: &str,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    match kind {
        PaneKind::Terminal => ui::pane(cx)
            .child(terminal::pane(bench, &cx.entity(), target, window))
            .into_any_element(),
        PaneKind::Files => ui::pane(cx)
            .child(crate::editor::pane(bench, &cx.entity(), target, window, cx))
            .into_any_element(),
        PaneKind::Git => match &bench.git {
            Some(pane) => pane.clone().into_any_element(),
            None => ui::pane(cx).into_any_element(),
        },
        PaneKind::Search => match &bench.search {
            Some(pane) => ui::pane(cx).child(pane.clone()).into_any_element(),
            None => ui::pane(cx).into_any_element(),
        },
        PaneKind::Agent => home::agent_view(bench, tab, id, target, window, cx),
        PaneKind::Desk => desk::pane(bench, tab, id, target, window, cx),
        _ => home::agent_view(bench, tab, id, "", window, cx),
    }
}

#[allow(clippy::too_many_arguments)]
fn split(
    bench: &Workbench,
    tab: &str,
    id: &str,
    dir: SplitDir,
    panes: &[Pane],
    kept: &[u16],
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let across = dir == SplitDir::Row;
    let sizes = bench.panes.sizes(id, kept).to_vec();
    let mut row = div()
        .id(SharedString::from(id.to_owned()))
        .flex()
        .when(!across, |row| row.flex_col())
        .size_full()
        .min_w_0()
        .min_h_0();
    {
        let split = id.to_owned();
        row = row.on_drag_move(
            cx.listener(move |bench, event: &DragMoveEvent<Divider>, _, cx| {
                let divider = event.drag(cx).clone();
                if divider.split != split {
                    return;
                }
                let kept = bench
                    .tab()
                    .and_then(|tab| split_sizes(&tab.pane, &split))
                    .unwrap_or_default();
                bench
                    .panes
                    .drag(&divider, kept, event.bounds, event.event.position, across);
                bench.parts.redraw(&bench.parts.stage, cx);
            }),
        );
        row = row
            .on_drop(cx.listener(|bench, _: &Divider, _, _| bench.finish_resize()))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|bench, _, _, _| bench.finish_resize()),
            );
    }
    for (at, (child, share)) in panes.iter().zip(sizes.iter().copied()).enumerate() {
        if at > 0 {
            row = row.child(divider(tab, id, at - 1, across));
        }
        let mut cell = div().flex().min_w_0().min_h_0().flex_basis(px(0.));
        cell.style().flex_grow = Some(f32::from(share.max(1)));
        row = row.child(
            cell.when(across, |cell| cell.h_full())
                .when(!across, |cell| cell.w_full())
                .child(pane(bench, tab, child, window, cx)),
        );
    }
    row.into_any_element()
}

fn divider(tab: &str, split: &str, at: usize, across: bool) -> impl IntoElement {
    let value = Divider {
        tab: tab.to_owned(),
        split: split.to_owned(),
        at,
    };
    div()
        .id(SharedString::from(format!("{split}-divider-{at}")))
        .flex_none()
        .when(across, |divider| {
            divider
                .w(px(GAP))
                .h_full()
                .cursor(CursorStyle::ResizeLeftRight)
        })
        .when(!across, |divider| {
            divider
                .h(px(GAP))
                .w_full()
                .cursor(CursorStyle::ResizeUpDown)
        })
        .on_drag(value, |_, _, _, cx| cx.new(|_| EmptyView))
}

fn split_sizes(pane: &Pane, split: &str) -> Option<Vec<u16>> {
    match pane {
        Pane::Split { id, sizes, .. } if id == split => Some(sizes.clone()),
        Pane::Split { panes, .. } => panes.iter().find_map(|child| split_sizes(child, split)),
        Pane::View { .. } => None,
    }
}

impl Panes {
    fn drag(
        &mut self,
        divider: &Divider,
        kept: Vec<u16>,
        bounds: Bounds<Pixels>,
        at: Point<Pixels>,
        across: bool,
    ) {
        let resizing = match &mut self.resizing {
            Some(resizing) if resizing.split == divider.split && resizing.from == kept => resizing,
            _ => self.resizing.insert(Resizing {
                tab: divider.tab.clone(),
                split: divider.split.clone(),
                sizes: kept.clone(),
                from: kept,
            }),
        };
        let sizes = &mut resizing.sizes;
        let (Some(&before), Some(&after)) = (sizes.get(divider.at), sizes.get(divider.at + 1))
        else {
            return;
        };
        let (start, length, pointer) = if across {
            (bounds.left(), bounds.size.width, at.x)
        } else {
            (bounds.top(), bounds.size.height, at.y)
        };
        let room = f32::from(length) - GAP * (sizes.len() - 1) as f32;
        if room <= 0. {
            return;
        }
        let total: f32 = sizes.iter().map(|&size| f32::from(size)).sum();
        let ahead: f32 = sizes[..divider.at]
            .iter()
            .map(|&size| f32::from(size))
            .sum();
        let offset = f32::from(pointer - start) - GAP * (divider.at as f32 + 0.5);
        let pair = f32::from(before) + f32::from(after);
        let least = (MIN_PANE / room * total).min(pair / 2.);
        let wanted = (offset / room * total - ahead).clamp(least, pair - least);
        sizes[divider.at] = wanted.round() as u16;
        sizes[divider.at + 1] = (pair - wanted).round() as u16;
    }
}

impl Workbench {
    fn finish_resize(&mut self) {
        let Some(resizing) = &self.panes.resizing else {
            return;
        };
        if resizing.sizes == resizing.from {
            self.panes.resizing = None;
            return;
        }
        let Some(strip) = self.strip() else {
            return;
        };
        let params = json!({
            "tab": resizing.tab,
            "split": resizing.split,
            "sizes": resizing.sizes,
        });
        self.call("resizeSplit", strip.with(params));
    }

    /// Puts a view of `kind` showing `target` beside the view `pane` of `tab`.
    pub fn split_pane(
        &self,
        strip: &Strip,
        tab: &str,
        pane: &str,
        side: &str,
        kind: PaneKind,
        target: &str,
    ) {
        let params = json!({
            "tab": tab,
            "pane": pane,
            "side": side,
            "kind": kind,
            "target": target,
        });
        self.call("splitPane", strip.with(params));
    }

    /// Notes the side of the view `pane` of `tab` that a tab dragged over it would take: the
    /// edge the pointer is nearest that the layout still has room on.
    fn aim_split(
        &mut self,
        tab: &str,
        pane: &str,
        event: &DragMoveEvent<DraggedTab>,
        cx: &mut Context<Self>,
    ) {
        let over = event.bounds.contains(&event.event.position);
        let dragged = event.drag(cx).clone();
        let open = self
            .tab()
            .filter(|shown| shown.id == tab)
            .map(|shown| open_sides(&shown.pane))
            .unwrap_or_default();
        let aimable = over
            && dragged.tab != tab
            && self.strip().as_ref() == Some(&dragged.strip)
            && !open.is_empty();
        let next = aimable.then(|| {
            let (bounds, at) = (event.bounds, event.event.position);
            let distance = |side: Side| match side {
                Side::Left => f32::from(at.x - bounds.left()) / f32::from(bounds.size.width),
                Side::Right => f32::from(bounds.right() - at.x) / f32::from(bounds.size.width),
                Side::Top => f32::from(at.y - bounds.top()) / f32::from(bounds.size.height),
                Side::Bottom => f32::from(bounds.bottom() - at.y) / f32::from(bounds.size.height),
            };
            let side = open
                .iter()
                .copied()
                .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
                .unwrap_or(Side::Right);
            Aimed {
                tab: tab.to_owned(),
                pane: pane.to_owned(),
                side,
                from: dragged.tab.clone(),
            }
        });
        let leaving = !over
            && self
                .panes
                .aimed
                .as_ref()
                .is_some_and(|aimed| aimed.pane == pane);
        if (next.is_some() || leaving) && self.panes.aimed != next {
            self.panes.aimed = next;
            self.parts.redraw(&self.parts.stage, cx);
        }
    }

    /// Moves the dropped tab's view beside the view `pane`, on the side it was aimed at.
    fn drop_split(&mut self, pane: &str, dropped: &DraggedTab, cx: &mut Context<Self>) {
        let Some(aimed) = self
            .panes
            .aimed
            .take()
            .filter(|aimed| aimed.pane == pane && aimed.from == dropped.tab)
        else {
            return;
        };
        let params = json!({
            "tab": aimed.tab,
            "pane": aimed.pane,
            "side": aimed.side.word(),
            "fromTab": aimed.from,
        });
        self.call("splitPane", dropped.strip.with(params));
        self.parts.redraw(&self.parts.stage, cx);
    }

    pub fn close_pane(&self, strip: &Strip, tab: &str, pane: &str) {
        self.call("closePane", strip.with(json!({ "tab": tab, "pane": pane })));
    }
}
