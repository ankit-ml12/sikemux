//! The Mac app's motion: its curves and timings, so what moves here moves as it does there.
//! Nothing summoned animates in, leaving is front-loaded, and nothing runs past 200 ms.
//! What the Mac stills while macOS asks for less motion is stilled here too; the few it lets
//! move anyway, here as there, say so with `Ease::moves_when_reduced`.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::base::animation::cubic_bezier;
use gpui_kit::base::motion::Interpolate;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, radians, Animation, AnyElement, App, Bounds, Element, ElementId, GlobalElementId,
    Hsla, InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Rgba, SharedString,
    Styled, Svg, Transformation, Window,
};

/// How a value moves to a new target: over how long, on which curve, and whether it moves
/// while macOS asks for less motion.
#[derive(Clone, Copy)]
pub struct Ease {
    pub duration: Duration,
    pub curve: Curve,
    pub moves_when_reduced: bool,
}

#[derive(Clone, Copy)]
pub enum Curve {
    Linear,
    /// A CSS `cubic-bezier(x1, y1, x2, y2)`.
    Bezier(f32, f32, f32, f32),
}

impl Curve {
    /// Most of the movement early, then a long settle.
    pub const OUT: Curve = Curve::Bezier(0.16, 1., 0.3, 1.);
    /// CSS's plain `ease`.
    pub const CSS: Curve = Curve::Bezier(0.25, 0.1, 0.25, 1.);
    /// CSS's `ease-out`.
    pub const CSS_OUT: Curve = Curve::Bezier(0., 0., 0.58, 1.);
}

impl Ease {
    pub const fn new(millis: u64, curve: Curve) -> Self {
        Ease {
            duration: Duration::from_millis(millis),
            curve,
            moves_when_reduced: false,
        }
    }

    pub const fn moves_when_reduced(mut self) -> Self {
        self.moves_when_reduced = true;
        self
    }

    /// Where the curve is a share `t` of the way through.
    pub fn at(self, t: f32) -> f32 {
        match self.curve {
            Curve::Linear => t,
            Curve::Bezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2)(t),
        }
    }
}

/// A row's tint and text colour as it is chosen or hovered.
pub const ROW_TINT: Ease = Ease::new(90, Curve::Linear);
/// A control's wash and ink as it is hovered or pressed, as the rails' header actions.
pub const CONTROL_TINT: Ease = Ease::new(90, Curve::CSS);
/// A tab's or pill's tint as it is chosen or hovered.
pub const PILL_TINT: Ease = Ease::new(200, Curve::OUT);
/// A chevron turning as what it folds opens: in a chat or a review, where the Mac turns it
/// even while macOS asks for less motion.
pub const DETAIL_FOLD: Ease = Ease::new(120, Curve::CSS).moves_when_reduced();
/// A file tree's chevron.
pub const TREE_FOLD: Ease = Ease::new(140, Curve::CSS);
/// A list's group, a run's step or a release's assets.
pub const LIST_FOLD: Ease = Ease::new(90, Curve::CSS);
/// A team in the agents rail, or a lane on the board.
pub const LANE_FOLD: Ease = Ease::new(90, Curve::CSS_OUT);

/// A dialog's backdrop and the dialog rising over it, and a tooltip fading in.
pub const SCRIM_IN: Ease = Ease::new(140, Curve::CSS_OUT);
pub const DIALOG_IN: Ease = Ease::new(220, Curve::OUT);
pub const TOOLTIP_IN: Ease = Ease::new(110, Curve::CSS_OUT);

/// A docked rail tucking under the stage or coming out from under it, as a drawer does.
pub const RAIL_DRAWER: Ease = Ease::new(400, Curve::Bezier(0.25, 1., 0.5, 1.));
pub const RAIL_DRAWER_SHORTEST: Duration = Duration::from_millis(120);
/// A closed rail brought out over the stage at its edge, and put away again.
pub const PEEK_IN: Ease = Ease::new(180, Curve::Bezier(0.2, 0.8, 0.2, 1.));
pub const PEEK_OUT: Ease = Ease::new(140, Curve::Bezier(0.4, 0., 0.8, 0.2));

/// An agent's mark popping in as its state changes, and the two rings a mark that now needs
/// the person sends out, the second a little after the first.
pub const STATE_POP: Ease = Ease::new(200, Curve::OUT);
pub const NEEDS_YOU: Ease = Ease::new(900, Curve::Linear);
pub const RING: Ease = Ease::new(520, Curve::Bezier(0.2, 0.6, 0.3, 1.));
pub const SECOND_RING: Duration = Duration::from_millis(380);

/// An animation that plays once as its element first draws, on `ease`. It holds still while
/// macOS asks for less motion, so it is only for what the Mac stills then too.
pub fn entrance(ease: Ease) -> Animation {
    Animation::new(ease.duration).with_easing(move |t| ease.at(t))
}

/// A run of tool calls opening onto its calls, and folding them away.
pub const UNFOLD: Ease = Ease::new(180, Curve::OUT);
pub const FOLD_AWAY: Ease = Ease::new(140, Curve::Bezier(0.4, 0., 1., 1.));

/// The angle of a fold's chevron, open or shut.
pub fn fold_angle(open: bool) -> f32 {
    if open {
        std::f32::consts::FRAC_PI_2
    } else {
        0.
    }
}

pub fn ease_out() -> impl Fn(f32) -> f32 {
    cubic_bezier(0.16, 1., 0.3, 1.)
}

/// `target` as it is drawn now: when it changes, the drawn value follows it from wherever it
/// was, as a CSS transition does. The first value is drawn as it is.
pub fn follow<T: Interpolate + PartialEq + 'static>(
    id: impl Into<ElementId>,
    target: T,
    ease: Ease,
    window: &mut Window,
    cx: &mut App,
) -> T {
    follow_over(id.into(), target, ease, |_, _| ease.duration, window, cx)
}

/// A share from 0 to 1, such as how far a rail is tucked away, following `target` as `follow`
/// does; a move turned round part way takes the share of `ease`'s time its distance is, and
/// never less than `shortest`.
pub fn follow_share(
    id: impl Into<ElementId>,
    target: f32,
    ease: Ease,
    shortest: Duration,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let over = |from: &f32, to: &f32| {
        ease.duration
            .mul_f32((to - from).abs().min(1.))
            .max(shortest)
    };
    follow_over(id.into(), target, ease, over, window, cx)
}

fn follow_over<T: Interpolate + PartialEq + 'static>(
    id: ElementId,
    target: T,
    ease: Ease,
    over: impl Fn(&T, &T) -> Duration,
    window: &mut Window,
    cx: &mut App,
) -> T {
    let now = Instant::now();
    let state = window.use_keyed_state(id, cx, |_, _| Following {
        from: target.clone(),
        to: target.clone(),
        since: now,
        over: Duration::ZERO,
    });
    if state.read(cx).to != target {
        let still = cx.reduce_motion() && !ease.moves_when_reduced;
        let shown = state.read(cx).at(now, ease);
        let from = if still { target.clone() } else { shown };
        let over = over(&from, &target);
        state.update(cx, |following, _| {
            *following = Following {
                from,
                to: target.clone(),
                since: now,
                over,
            }
        });
    }
    let following = state.read(cx);
    if following.from != following.to && now - following.since < following.over {
        window.request_animation_frame();
    }
    following.at(now, ease)
}

/// The colour `channel` of the element `id` draws now, fading to `target`.
pub fn fade(
    id: &ElementId,
    channel: &'static str,
    target: Hsla,
    ease: Ease,
    window: &mut Window,
    cx: &mut App,
) -> Hsla {
    let id = ElementId::NamedChild(Arc::new(id.clone()), channel.into());
    follow(id, Paint(target.into()), ease, window, cx).0.into()
}

/// What `build` makes of `target` as it is drawn now, following it as `follow` does; for
/// drawing that has no window to hand.
pub fn animated<T: Interpolate + PartialEq + 'static>(
    id: impl Into<ElementId>,
    target: T,
    ease: Ease,
    build: impl FnOnce(T) -> AnyElement + 'static,
) -> Animated<T> {
    Animated {
        id: id.into(),
        target: Some(target),
        ease,
        build: Some(Box::new(build)),
    }
}

/// `icon` turned by `angle`, turning to a new angle as `ease` says, as a fold's chevron does.
pub fn turn(key: impl Into<SharedString>, angle: f32, ease: Ease, icon: Svg) -> Animated<f32> {
    animated(ElementId::Name(key.into()), angle, ease, move |angle| {
        icon.with_transformation(Transformation::rotate(radians(angle)))
            .into_any_element()
    })
}

pub struct Animated<T> {
    id: ElementId,
    target: Option<T>,
    ease: Ease,
    build: Option<Box<dyn FnOnce(T) -> AnyElement>>,
}

impl<T: Interpolate + PartialEq + 'static> IntoElement for Animated<T> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<T: Interpolate + PartialEq + 'static> Element for Animated<T> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let (Some(target), Some(build)) = (self.target.take(), self.build.take()) else {
            unreachable!("an animated element is laid out once");
        };
        let value = follow(self.id.clone(), target, self.ease, window, cx);
        let mut element = build(value);
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

/// What `build` makes of how far it is through playing once since `key` last changed: from
/// 0 to 1 over `ease`, and 1 when `key` was as it is when the element first drew.
pub fn replay<K: PartialEq + Clone + 'static>(
    id: impl Into<ElementId>,
    key: K,
    ease: Ease,
    build: impl FnOnce(f32) -> AnyElement + 'static,
) -> Replay<K> {
    Replay {
        id: id.into(),
        key: Some(key),
        ease,
        build: Some(Box::new(build)),
    }
}

pub struct Replay<K> {
    id: ElementId,
    key: Option<K>,
    ease: Ease,
    build: Option<Box<dyn FnOnce(f32) -> AnyElement>>,
}

impl<K: PartialEq + Clone + 'static> IntoElement for Replay<K> {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<K: PartialEq + Clone + 'static> Element for Replay<K> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, AnyElement) {
        let (Some(key), Some(build)) = (self.key.take(), self.build.take()) else {
            unreachable!("a replaying element is laid out once");
        };
        let ease = self.ease;
        let kept = key.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| (kept, None::<Instant>));
        if state.read(cx).0 != key {
            let moves = !cx.reduce_motion() || ease.moves_when_reduced;
            let since = moves.then(Instant::now);
            state.update(cx, |state, _| *state = (key, since));
        }
        let through = state.read(cx).1.map_or(1., |since| {
            (since.elapsed().as_secs_f32() / ease.duration.as_secs_f32()).min(1.)
        });
        if through < 1. {
            window.request_animation_frame();
        }
        let mut element = build(through);
        (element.request_layout(window, cx), element)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        window: &mut Window,
        cx: &mut App,
    ) {
        element.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        element: &mut AnyElement,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        element.paint(window, cx);
    }
}

/// `body` folded `shown` of the way open, from 0 shut to 1 at its own height; it fades as it
/// folds. Its height is the one it last drew at, so it must be drawn while it folds.
pub fn folded(
    id: impl Into<ElementId>,
    shown: f32,
    body: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let measured = window
        .use_keyed_state(id.into(), cx, |_, _| Rc::new(Cell::new(0.)))
        .read(cx)
        .clone();
    let height = measured.get();
    div()
        .flex()
        .flex_col()
        .overflow_hidden()
        .opacity(shown)
        .when(shown < 1., |fold| fold.h(px(height * shown)))
        .child(div().flex_none().child(body))
        .on_children_prepainted(move |bounds, _, _| {
            if let Some(body) = bounds.first() {
                measured.set(body.size.height.into());
            }
        })
        .into_any_element()
}

/// Whether the pointer was over the element `id` when it last moved, and what keeps that
/// up to date, to give the element's `on_hover`.
pub fn hovered(
    id: &ElementId,
    window: &mut Window,
    cx: &mut App,
) -> (bool, impl Fn(&bool, &mut Window, &mut App) + 'static) {
    let id = ElementId::NamedChild(Arc::new(id.clone()), "hovered".into());
    let state = window.use_keyed_state(id, cx, |_, _| false);
    let over = *state.read(cx);
    let keep = move |inside: &bool, _: &mut Window, cx: &mut App| {
        state.update(cx, |over, cx| {
            if *over != *inside {
                *over = *inside;
                cx.notify();
            }
        })
    };
    (over, keep)
}

struct Following<T> {
    from: T,
    to: T,
    since: Instant,
    over: Duration,
}

impl<T: Interpolate> Following<T> {
    fn at(&self, now: Instant, ease: Ease) -> T {
        let t = match self.over.is_zero() {
            true => 1.,
            false => ((now - self.since).as_secs_f32() / self.over.as_secs_f32()).min(1.),
        };
        self.from.interpolate(&self.to, ease.at(t))
    }
}

/// A colour blended as CSS blends one in a transition: in sRGB, weighted by its alpha, so a
/// tint fading in from clear keeps its hue.
#[derive(Clone, PartialEq)]
struct Paint(Rgba);

impl Interpolate for Paint {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let (from, to) = (self.0, target.0);
        let a = from.a + (to.a - from.a) * progress;
        let channel = |x: f32, y: f32| match a {
            0. => 0.,
            _ => (x * from.a + (y * to.a - x * from.a) * progress) / a,
        };
        Paint(Rgba {
            r: channel(from.r, to.r),
            g: channel(from.g, to.g),
            b: channel(from.b, to.b),
            a,
        })
    }
}
