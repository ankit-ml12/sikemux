//! Faces that move with what their agents are doing, as the Mac's do: one bobs while its agent
//! works and hops while it waits on the person. Each is a view of its own, drawn again in place
//! on one clock shared by every face, so moving faces cost one small frame together and never
//! draw the rows around them again. They hold still while the window is behind others.

use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use gpui::{
    div, img, px, AnyElement, App, Bounds, Context, Element, ElementId, Entity, Global,
    GlobalElementId, Image, InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels,
    Render, Styled, Task, WeakEntity, Window,
};
use sikemux_wire::protocol::{AgentFace, AgentGroup};

use crate::workbench::Workbench;

const FRAMES_PER_SECOND: u64 = 30;
/// Up and down again, each way this long, rising this many of the face's own units.
const BOB_SECONDS: f32 = 0.7;
const BOB_LIFT: f32 = 0.8;
/// Up and back down in the first part of each hop, then resting for the rest of it.
const HOP_SECONDS: f32 = 1.2;
const HOP_LIFT: f32 = 1.8;
const HOP_PEAK: f32 = 0.25;
const HOP_LANDED: f32 = 0.55;

static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Whether faces in `group` move, as the person's face style and the system's ask for less
/// motion say.
pub fn moves(group: AgentGroup, cx: &App) -> bool {
    matches!(group, AgentGroup::Working | AgentGroup::Blocked)
        && !cx.reduce_motion()
        && cx.try_global::<Clock>().is_some_and(|clock| clock.on)
}

/// The face of agent `agent`, moving.
pub fn face(
    agent: &str,
    face: &AgentFace,
    group: AgentGroup,
    image: Arc<Image>,
    size: f32,
) -> AnyElement {
    Slot {
        key: ElementId::Name(format!("face-{agent}").into()),
        face: Face {
            image,
            size,
            side: side(&face.view_box),
            group,
        },
        drawn: None,
    }
    .into_any_element()
}

/// Turns the faces' motion on or off, as the person's face style says.
pub fn set_on(on: bool, cx: &mut App) {
    let clock = cx.default_global::<Clock>();
    if clock.on != on {
        clock.on = on;
        cx.refresh_windows();
    }
}

/// Starts the faces again when the window comes back to the front or into sight.
pub fn follow(window: &mut Window, cx: &mut Context<Workbench>) {
    cx.observe_window_activation(window, |_, _, cx| wake(cx))
        .detach();
    cx.observe_window_visibility(window, |_, _, _, cx| wake(cx))
        .detach();
}

/// The faces drawn since the clock last moved them, and its next move.
#[derive(Default)]
struct Clock {
    on: bool,
    drawn: Vec<WeakEntity<Face>>,
    /// Faces that stopped while the window was behind others, to start again with it.
    resting: Vec<WeakEntity<Face>>,
    ticking: Option<Task<()>>,
}

impl Global for Clock {}

fn wake(cx: &mut App) {
    let resting = std::mem::take(&mut cx.default_global::<Clock>().resting);
    for face in resting.iter().filter_map(WeakEntity::upgrade) {
        cx.repaint(face.entity_id());
    }
}

/// Has the clock draw `face` again on its next move.
fn again(face: WeakEntity<Face>, cx: &mut Context<Face>) {
    let clock = cx.default_global::<Clock>();
    add(&mut clock.drawn, face);
    if clock.ticking.is_some() {
        return;
    }
    let tick = cx.spawn(async |_, cx| {
        cx.background_executor()
            .timer(Duration::from_millis(1000 / FRAMES_PER_SECOND))
            .await;
        cx.update(|cx| {
            let clock = cx.default_global::<Clock>();
            clock.ticking = None;
            let drawn = std::mem::take(&mut clock.drawn);
            for face in drawn.iter().filter_map(WeakEntity::upgrade) {
                cx.repaint(face.entity_id());
            }
        });
    });
    cx.default_global::<Clock>().ticking = Some(tick);
}

fn add(faces: &mut Vec<WeakEntity<Face>>, face: WeakEntity<Face>) {
    if !faces
        .iter()
        .any(|known| known.entity_id() == face.entity_id())
    {
        faces.push(face);
    }
}

/// The side of a face's square, in the units its lift is measured in.
fn side(view_box: &str) -> f32 {
    view_box
        .split_whitespace()
        .nth(3)
        .and_then(|side| side.parse().ok())
        .filter(|side: &f32| *side > 0.)
        .unwrap_or(24.)
}

/// How far above its rest a face in `group` is, in its own units, `seconds` in.
fn lift(group: AgentGroup, seconds: f32) -> f32 {
    let ease = |at: f32| at * at * (3. - 2. * at);
    match group {
        AgentGroup::Working => {
            let at = (seconds / BOB_SECONDS) % 2.;
            BOB_LIFT * ease(if at < 1. { at } else { 2. - at })
        }
        AgentGroup::Blocked => {
            let at = (seconds / HOP_SECONDS) % 1.;
            if at < HOP_PEAK {
                HOP_LIFT * ease(at / HOP_PEAK)
            } else if at < HOP_LANDED {
                HOP_LIFT * ease(1. - (at - HOP_PEAK) / (HOP_LANDED - HOP_PEAK))
            } else {
                0.
            }
        }
        AgentGroup::Idle | AgentGroup::Done => 0.,
    }
}

struct Face {
    image: Arc<Image>,
    size: f32,
    side: f32,
    group: AgentGroup,
}

impl Render for Face {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = window.is_window_active() && window.is_visible();
        let face = cx.weak_entity();
        let rise = match shown {
            true => {
                again(face, cx);
                lift(self.group, STARTED.elapsed().as_secs_f32()) * self.size / self.side
            }
            false => {
                add(&mut cx.default_global::<Clock>().resting, face);
                0.
            }
        };
        div().size(px(self.size)).flex_none().child(
            img(self.image.clone())
                .size(px(self.size))
                .relative()
                .top(px(-rise)),
        )
    }
}

/// Where a moving face sits in a row: it keeps the face's view from one frame to the next, so
/// the clock can draw it again on its own.
struct Slot {
    key: ElementId,
    face: Face,
    drawn: Option<AnyElement>,
}

impl IntoElement for Slot {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Slot {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        let Face {
            image,
            size,
            side,
            group,
        } = &self.face;
        let face: Entity<Face> = window.use_keyed_state(self.key.clone(), cx, |_, _| Face {
            image: image.clone(),
            size: *size,
            side: *side,
            group: *group,
        });
        face.update(cx, |face, _| {
            face.image = image.clone();
            face.size = *size;
            face.side = *side;
            face.group = *group;
        });
        let mut drawn = face.into_any_element();
        let layout = drawn.request_layout(window, cx);
        self.drawn = Some(drawn);
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(drawn) = self.drawn.as_mut() {
            drawn.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(drawn) = self.drawn.as_mut() {
            drawn.paint(window, cx);
        }
    }
}
