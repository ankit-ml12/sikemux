//! A DBML block in an agent's reply, drawn as a diagram of its tables, with its text a
//! click away.

use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, AnyView, App, AppContext, Div, ElementId, Entity, Hsla,
    InteractiveElement, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement, StyleRefinement, Styled, WeakEntity, Window,
};
use gpui_transcript::{Fence, Fences};

use crate::editor::diagram::Diagram;
use crate::theme::{self, Tokens};
use crate::workbench::Workbench;

/// Draws the ```dbml blocks of `agent`'s chat.
pub(super) fn fences(bench: WeakEntity<Workbench>, agent: String) -> Fences {
    Fences {
        langs: Rc::from(["dbml"]),
        draw: Rc::new(move |fence, window, cx| block(fence, &bench, &agent, window, cx)),
    }
}

/// What a block keeps between frames: its diagram, the text it was drawn from, and whether
/// the person chose to see the text.
struct Block {
    diagram: Entity<Diagram>,
    code: SharedString,
    source: bool,
}

fn block(
    fence: Fence,
    bench: &WeakEntity<Workbench>,
    agent: &str,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let key = ElementId::NamedChild(Arc::new(fence.id.clone()), "dbml".into());
    let code = fence.code.clone();
    let block = window.use_keyed_state(key, cx, |_, cx| Block {
        diagram: cx.new(|_| Diagram::embedded(&code)),
        code: code.clone(),
        source: false,
    });
    let (diagram, changed) = {
        let kept = block.read(cx);
        (kept.diagram.clone(), kept.code != fence.code)
    };
    if changed {
        block.update(cx, |kept, _| kept.code = fence.code.clone());
        diagram.update(cx, |diagram, cx| diagram.set_source(&fence.code, cx));
    }
    let empty = diagram.read(cx).is_empty();
    let source = block.read(cx).source || empty;
    let t = Tokens::get(cx).clone();
    let choice = |id: &'static str, label: &'static str, shows_source: bool| {
        let block = block.clone();
        let chosen = source == shows_source;
        chip(id)
            .text_color(if chosen {
                t.text_primary
            } else {
                t.text_disabled
            })
            .map(|chip| match chosen {
                true => chip.bg(t.surface_chosen),
                false => chip.hover(|chip| chip.bg(t.surface_hover)),
            })
            .on_click(move |_, _, cx| {
                block.update(cx, |block, cx| {
                    block.source = shows_source;
                    cx.notify();
                })
            })
            .child(label)
    };
    let open = {
        let (code, bench, agent) = (fence.code.clone(), bench.clone(), agent.to_owned());
        chip("dbml-open")
            .text_color(t.text_disabled)
            .hover(|open| open.bg(t.surface_hover).text_color(t.text_primary))
            .tooltip(crate::ui::tooltip(
                "Open on the desk, with room to spread out",
            ))
            .on_click(move |_, _, cx| open_on_desk(&code, &bench, &agent, cx))
            .child("Open on desk")
    };
    let title = div()
        .h(px(23.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(2.))
        .border_b_1()
        .border_color(t.line_soft)
        .text_size(px(10.))
        .text_color(t.text_disabled)
        .child(div().flex_1().min_w_0().truncate().child(fence.lang))
        .when(!empty, |title| {
            title
                .child(choice("dbml-diagram", "Diagram", false))
                .child(choice("dbml-source", "Source", true))
                .child(div().w(px(1.)).h(px(10.)).mx(px(4.)).bg(t.line_soft))
                .child(open)
                .child(div().w(px(6.)))
        })
        .child(fence.copy);
    div()
        .id(ElementId::NamedChild(
            Arc::new(fence.id),
            "dbml-card".into(),
        ))
        .rounded(px(5.))
        .border_1()
        .border_color(t.line_soft)
        .bg(Hsla {
            a: 0.72,
            ..t.surface_sunken
        })
        .overflow_hidden()
        .child(title)
        .map(|frame| match source {
            true => frame.child(fence.lines),
            false => {
                let height = diagram.read(cx).height();
                frame.child(
                    AnyView::from(diagram)
                        .cached(StyleRefinement::default().w_full().h(px(height))),
                )
            }
        })
        .into_any_element()
}

fn chip(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(6.))
        .h(px(17.))
        .flex()
        .items_center()
        .rounded(theme::RADIUS_1)
        .cursor_pointer()
}

/// Writes the schema to a file of its own and opens it on the agent's desk, where it has
/// room to spread out and its text can be edited.
fn open_on_desk(code: &SharedString, bench: &WeakEntity<Workbench>, agent: &str, cx: &mut App) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let folder = std::env::temp_dir().join("sikemux-schemas");
    let path = folder.join(format!("schema-{:08x}.dbml", hasher.finish() as u32));
    let written = std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(&path, &**code));
    if written.is_err() {
        return;
    }
    let _ = bench.update(cx, |bench, cx| {
        bench.open_file_on_desk(agent, path, None, None);
        bench.parts.redraw(&bench.parts.stage, cx);
    });
}
