//! The Mac app's motion: its curves and timings, so what moves here moves as it does there.
//! Nothing summoned animates in, leaving is front-loaded, and nothing runs past 200 ms.
//! What the Mac stills while macOS asks for less motion is stilled here too; the few it lets
//! move anyway, here as there, say so with `Ease::moves_when_reduced`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::base::animation::cubic_bezier;
use gpui_kit::base::motion::Interpolate;
use gpui_kit::{
    radians, AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, Rgba, SharedString, Svg, Transformation,
    Window,
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
    /// Most of the movement early, then a long settle.
    Out,
    /// CSS's plain `ease`.
    Css,
    /// CSS's `ease-out`.
    CssOut,
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

    fn at(self, t: f32) -> f32 {
        match self.curve {
            Curve::Linear => t,
            Curve::Out => ease_out()(t),
            Curve::Css => cubic_bezier(0.25, 0.1, 0.25, 1.)(t),
            Curve::CssOut => cubic_bezier(0., 0., 0.58, 1.)(t),
        }
    }
}

/// A row's tint and text colour as it is chosen or hovered.
pub const ROW_TINT: Ease = Ease::new(90, Curve::Linear);
/// A tab's or pill's tint as it is chosen or hovered.
pub const PILL_TINT: Ease = Ease::new(200, Curve::Out);
/// A chevron turning as what it folds opens: in a chat or a review, where the Mac turns it
/// even while macOS asks for less motion.
pub const DETAIL_FOLD: Ease = Ease::new(120, Curve::Css).moves_when_reduced();
/// A file tree's chevron.
pub const TREE_FOLD: Ease = Ease::new(140, Curve::Css);
/// A list's group, a run's step or a release's assets.
pub const LIST_FOLD: Ease = Ease::new(90, Curve::Css);
/// A team in the agents rail, or a lane on the board.
pub const LANE_FOLD: Ease = Ease::new(90, Curve::CssOut);

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
    let state = window.use_keyed_state(id.into(), cx, |_, _| Following {
        from: target.clone(),
        to: target.clone(),
        since: Instant::now(),
    });
    let still = cx.reduce_motion() && !ease.moves_when_reduced;
    let now = Instant::now();
    let shown = state.read(cx).at(now, ease);
    if state.read(cx).to != target {
        let from = if still { target.clone() } else { shown.clone() };
        state.update(cx, |following, _| {
            *following = Following {
                from,
                to: target.clone(),
                since: now,
            }
        });
    }
    let following = state.read(cx);
    let value = following.at(now, ease);
    if following.from != following.to && now - following.since < ease.duration {
        window.request_animation_frame();
    }
    value
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
}

impl<T: Interpolate> Following<T> {
    fn at(&self, now: Instant, ease: Ease) -> T {
        let t = (now - self.since).as_secs_f32() / ease.duration.as_secs_f32().max(f32::EPSILON);
        self.from.interpolate(&self.to, ease.at(t.min(1.)))
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
