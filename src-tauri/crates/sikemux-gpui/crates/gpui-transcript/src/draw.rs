use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ops::Range;
use std::panic::Location;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, point, px, relative, rgb, rgba, size, AnyElement, App, AvailableSpace, Bounds,
    ClipboardItem, Div, Element, ElementId, Font, FontStyle, FontWeight, GlobalElementId,
    HighlightStyle, Hsla, InspectorElementId, InteractiveElement, InteractiveText, IntoElement,
    LayoutId, ParentElement, Pixels, SharedString, Stateful, StatefulInteractiveElement,
    StrikethroughStyle, Styled, StyledText, Window,
};

use crate::markdown::{superscript, Kind, Markdown, Marks, Node, Piece, Rich};
use crate::select::{Selecting, Shown};

/// A fenced code block, for a host that draws its own.
pub struct Fence {
    pub id: ElementId,
    pub lang: Option<SharedString>,
    pub code: SharedString,
}

pub type OnLink = Rc<dyn Fn(&str, &mut Window, &mut App)>;
pub type DrawFence = Rc<dyn Fn(Fence, &mut Window, &mut App) -> AnyElement>;
/// Draws a picture from its address and its alt text.
pub type DrawImage = Rc<dyn Fn(&str, &str, &mut Window, &mut App) -> AnyElement>;

/// How markdown looks and what it does when clicked. The defaults are Sikemux's dark theme.
#[derive(Clone)]
pub struct Style {
    pub font: SharedString,
    pub mono: Font,
    pub text_size: Pixels,
    pub line_height: f32,
    pub code_size: Pixels,
    pub table_size: Pixels,
    pub gap: Pixels,
    pub ink: Hsla,
    pub ink_dim: Hsla,
    pub ink_faint: Hsla,
    pub accent: Hsla,
    pub code_ink: Hsla,
    pub code_ground: Hsla,
    pub mark_ground: Hsla,
    pub selection: Hsla,
    pub line: Hsla,
    pub line_soft: Hsla,
    pub table_ground: Hsla,
    pub table_head: Hsla,
    pub radius: Pixels,
    pub on_link: Option<OnLink>,
    pub fence: Option<DrawFence>,
    pub image: Option<DrawImage>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            font: "Figtree".into(),
            mono: gpui::font("JetBrainsMono NFM"),
            text_size: px(14.),
            line_height: 1.65,
            code_size: px(12.),
            table_size: px(13.5),
            gap: px(10.),
            ink: rgb(0xe7e5ef).into(),
            ink_dim: rgb(0x8b8898).into(),
            ink_faint: rgb(0x736f80).into(),
            accent: rgb(0xa277ff).into(),
            code_ink: rgb(0xb494fb).into(),
            code_ground: rgba(0x09090bb8).into(),
            mark_ground: rgba(0xa277ff1a).into(),
            selection: rgba(0xa277ff4d).into(),
            line: rgba(0x24242bcc).into(),
            line_soft: rgb(0x19191e).into(),
            table_ground: rgba(0x19191eb8).into(),
            table_head: rgba(0xe7e5ef0a).into(),
            radius: px(5.),
            on_link: None,
            fence: None,
            image: None,
        }
    }
}

/// A message of this many blocks or more lays out only the blocks in view,
/// once each block's size is known.
const TALL: usize = 12;

/// What draws a row's markdown: the style, and in a transcript, the row's
/// selection and where its text is recorded so the pointer can select it.
pub struct Pen {
    style: Rc<Style>,
    selecting: Option<Rc<Selecting>>,
    docs: Cell<usize>,
}

impl Pen {
    pub fn new(style: Rc<Style>) -> Pen {
        Pen::selecting(style, None)
    }

    pub(crate) fn selecting(style: Rc<Style>, selecting: Option<Selecting>) -> Pen {
        Pen {
            style,
            selecting: selecting.map(Rc::new),
            docs: Cell::new(0),
        }
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Draws a message's markdown. `id` must be unique in the window. In a
    /// transcript, a row draws its markdown in the order `Row::docs` lists it.
    pub fn markdown(
        &self,
        id: impl Into<ElementId>,
        doc: &Markdown,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let id = id.into();
        let style = &self.style;
        let blocks = Blocks {
            scope: Arc::new(id.clone()),
            blocks: doc.blocks().to_vec(),
            style: style.clone(),
            selecting: self.selecting.clone(),
            doc: self.docs.replace(self.docs.get() + 1),
            width: doc.width.clone(),
            turned: doc.turned.clone(),
        };
        let root = div()
            .flex()
            .flex_col()
            .gap(style.gap)
            .min_w_0()
            .font_family(style.font.clone())
            .text_size(style.text_size)
            .line_height(relative(style.line_height))
            .text_color(style.ink);
        if blocks.blocks.len() >= TALL && blocks.width.get().is_some() {
            return root.child(Tall(blocks)).id(id).into_any_element();
        }
        let children: Vec<_> = (0..blocks.blocks.len())
            .map(|ix| blocks.draw(ix, window, cx))
            .collect();
        root.children(children)
            .on_children_prepainted(move |bounds, _, _| {
                for (block, bounds) in blocks.blocks.iter().zip(&bounds) {
                    block.size.set(Some(bounds.size));
                }
                if let Some(first) = bounds.first() {
                    blocks.width.set(Some(first.size.width));
                }
            })
            .id(id)
            .into_any_element()
    }
}

/// One message's blocks and what drawing them needs.
struct Blocks {
    scope: Arc<ElementId>,
    blocks: Vec<Rc<Node>>,
    style: Rc<Style>,
    selecting: Option<Rc<Selecting>>,
    doc: usize,
    width: Rc<Cell<Option<Pixels>>>,
    turned: Rc<RefCell<HashSet<(usize, u32)>>>,
}

impl Blocks {
    fn draw(&self, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let mut draw = Draw {
            on: self,
            block: ix,
            next: 0,
            texts: 0,
            folds: 0,
            depth: 0,
        };
        draw.block(&self.blocks[ix], ix, window, cx)
    }

    fn laid_out(&self, ix: usize, width: Pixels, window: &mut Window, cx: &mut App) -> AnyElement {
        let mut element = self.draw(ix, window, cx);
        let space = size(AvailableSpace::Definite(width), AvailableSpace::MinContent);
        let height = element.layout_as_root(space, window, cx).height;
        self.blocks[ix].size.set(Some(size(width, height)));
        element
    }
}

/// The blocks of a tall message: each laid out only when it changed or the
/// width did, and placed only when it shows.
struct Tall(Blocks);

impl IntoElement for Tall {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Tall {
    type RequestLayoutState = Vec<Option<AnyElement>>;
    type PrepaintState = Vec<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let blocks = &self.0;
        let width = blocks.width.get().unwrap_or_default();
        let laid: Vec<_> = (0..blocks.blocks.len())
            .map(|ix| {
                let fresh = blocks.blocks[ix]
                    .size
                    .get()
                    .is_some_and(|size| size.width == width);
                (!fresh).then(|| blocks.laid_out(ix, width, window, cx))
            })
            .collect();
        let heights = blocks
            .blocks
            .iter()
            .map(|block| block.size.get().map_or(px(0.), |size| size.height));
        let gaps = blocks.style.gap * blocks.blocks.len().saturating_sub(1) as f32;
        let mut style = gpui::Style::default();
        style.size.height = heights.fold(gaps, |sum, height| sum + height).into();
        (window.request_layout(style, None, cx), laid)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        laid: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let blocks = &self.0;
        let width = bounds.size.width;
        if blocks.width.replace(Some(width)) != Some(width) {
            window.refresh();
        }
        let view = window.content_mask().bounds;
        let mut top = bounds.top();
        let mut shown = Vec::new();
        for (ix, block) in blocks.blocks.iter().enumerate() {
            let height = block.size.get().map_or(px(0.), |size| size.height);
            if top + height >= view.top() && top <= view.bottom() {
                let mut element = laid[ix]
                    .take()
                    .unwrap_or_else(|| blocks.laid_out(ix, width, window, cx));
                element.prepaint_at(point(bounds.left(), top), window, cx);
                shown.push(element);
            }
            top += height + blocks.style.gap;
        }
        shown
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        shown: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for element in shown {
            element.paint(window, cx);
        }
    }
}

/// Draws one block. The texts it draws and the folds it meets are counted in
/// the order `Node::walk` lists them, so a selection's places match the copy.
struct Draw<'a> {
    on: &'a Blocks,
    block: usize,
    next: u64,
    texts: u32,
    folds: u32,
    depth: usize,
}

impl Draw<'_> {
    fn id(&mut self, name: &'static str) -> ElementId {
        self.next += 1;
        let name = format!("{}.{name}{}", self.block, self.next);
        ElementId::NamedChild(self.on.scope.clone(), name.into())
    }

    fn block(&mut self, node: &Node, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let style = &*self.on.style;
        match &node.kind {
            Kind::Heading(level) => {
                let scale = [2., 1.5, 1.17, 1., 0.83, 0.67][usize::from(level.clamp(&1, &6) - 1)];
                div()
                    .when(ix > 0, |heading| {
                        heading.pt(style.text_size * (scale * 0.4))
                    })
                    .text_size(style.text_size * scale)
                    .line_height(relative(1.3))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.flow(node, window, cx))
                    .into_any_element()
            }
            Kind::Quote => div()
                .pl(px(12.))
                .border_l_2()
                .border_color(style.line)
                .text_color(style.ink_dim)
                .child(self.flow(node, window, cx))
                .into_any_element(),
            Kind::Bullets | Kind::Numbers(_) => self.list(node, window, cx),
            Kind::Code(lang) => self.fence(node, lang.clone(), window, cx),
            Kind::Rule => div().h(px(1.)).my(px(4.)).bg(style.line).into_any_element(),
            Kind::Table(_) => self.table(node),
            Kind::Footnote(n) => div()
                .flex()
                .gap(px(4.))
                .text_color(style.ink_dim)
                .child(superscript(*n))
                .child(self.flow(node, window, cx))
                .into_any_element(),
            Kind::Details { summary, open } => self.details(node, summary, *open, window, cx),
            Kind::Image { src, alt } => match &style.image {
                Some(image) => image(src, alt, window, cx),
                None => img(src.clone()).max_w_full().into_any_element(),
            },
            _ => self.flow(node, window, cx),
        }
    }

    /// An element's content: its inline runs as text, its blocks as blocks.
    fn flow(&mut self, node: &Node, window: &mut Window, cx: &mut App) -> AnyElement {
        let pieces = node.flow();
        if let [Piece::Text(rich)] = pieces {
            return self.rich(rich);
        }
        let children: Vec<_> = pieces
            .iter()
            .map(|piece| match piece {
                Piece::Text(rich) => self.rich(rich),
                Piece::Block(ix) => self.block(&node.children[*ix], *ix, window, cx),
            })
            .collect();
        let gap = self.on.style.gap;
        div()
            .flex()
            .flex_col()
            .gap(gap)
            .min_w_0()
            .children(children)
            .into_any_element()
    }

    fn rich(&mut self, rich: &Rich) -> AnyElement {
        let on = self.on;
        let style = &*on.style;
        self.texts += 1;
        let place = (on.doc, self.block, self.texts);
        let selected = on.selecting.as_ref().and_then(|selecting| {
            let span = selecting.span?;
            span.range((selecting.ix, place.0, place.1, place.2), rich.text.len())
        });
        let mono = rich
            .marks
            .iter()
            .filter(|(_, marks)| marks.has(Marks::CODE));
        let text = StyledText::new(rich.text.clone())
            .with_highlights(highlights(rich, selected, style))
            .with_font_family_overrides(
                mono.map(|(range, _)| (range.clone(), style.mono.family.clone())),
            );
        let layout = text.layout().clone();
        let inner = if rich.links.is_empty() {
            text.into_any_element()
        } else {
            let links: Rc<[SharedString]> =
                rich.links.iter().map(|(_, href)| href.clone()).collect();
            let on_link = style.on_link.clone();
            let ranges = rich.links.iter().map(|(range, _)| range.clone()).collect();
            InteractiveText::new(self.id("text"), text)
                .on_click(ranges, move |ix, window, cx| {
                    match (&on_link, links.get(ix)) {
                        (Some(on_link), Some(href)) => on_link(href, window, cx),
                        (None, Some(href)) => cx.open_url(href),
                        _ => {}
                    }
                })
                .into_any_element()
        };
        let Some(selecting) = &on.selecting else {
            return inner;
        };
        let key = (selecting.row.clone(), place.0, place.1, place.2);
        let shown = Cell::new(Some(Shown {
            key,
            layout,
            len: rich.text.len(),
        }));
        let all = selecting.shown.clone();
        div()
            .child(inner)
            .on_children_prepainted(move |_, _, _| all.borrow_mut().extend(shown.take()))
            .into_any_element()
    }

    fn list(&mut self, node: &Node, window: &mut Window, cx: &mut App) -> AnyElement {
        let style = &*self.on.style;
        let bullet = ["•", "◦", "▪"][self.depth % 3];
        self.depth += 1;
        let items: Vec<_> = node
            .children
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let marker = match (&item.kind, &node.kind) {
                    (Kind::Item(Some(checked)), _) => task_box(*checked, style),
                    (_, Kind::Numbers(start)) => div()
                        .text_right()
                        .pr(px(6.))
                        .child(format!("{}.", start + ix as u64)),
                    _ => div().pl(px(6.)).child(bullet),
                };
                div()
                    .flex()
                    .child(div().w(px(22.)).flex_none().child(marker))
                    .child(div().flex_1().min_w_0().child(self.flow(item, window, cx)))
            })
            .collect();
        self.depth -= 1;
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .children(items)
            .into_any_element()
    }

    fn fence(
        &mut self,
        node: &Node,
        lang: Option<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let style = &*self.on.style;
        let [Piece::Text(code)] = node.flow() else {
            return div().into_any_element();
        };
        let id = self.id("code");
        if let Some(fence) = &style.fence {
            return fence(
                Fence {
                    id,
                    lang,
                    code: code.text.clone(),
                },
                window,
                cx,
            );
        }
        let title = div()
            .h(px(23.))
            .px(px(10.))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(style.line_soft)
            .text_size(px(10.))
            .text_color(style.ink_faint)
            .child(div().flex_1().min_w_0().truncate().children(lang))
            .child(self.copy(code.text.clone()));
        let body = div()
            .id(id)
            .max_h(px(420.))
            .overflow_y_scroll()
            .px(px(12.))
            .py(px(9.))
            .font(style.mono.clone())
            .text_size(style.code_size)
            .line_height(relative(1.55))
            .text_color(style.ink_dim)
            .child(self.rich(code));
        let frame = div()
            .rounded(style.radius)
            .border_1()
            .border_color(style.line_soft);
        frame
            .bg(style.code_ground)
            .child(title)
            .child(body)
            .into_any_element()
    }

    fn copy(&mut self, text: SharedString) -> Stateful<Div> {
        let ink = self.on.style.ink;
        div()
            .id(self.id("copy"))
            .cursor_pointer()
            .hover(move |copy| copy.text_color(ink))
            .child("Copy")
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()))
            })
    }

    /// A table as rows of cells. A column is as wide as its longest text,
    /// shared out when they do not all fit, so long cells wrap and every row lines up.
    fn table(&mut self, node: &Node) -> AnyElement {
        let style = &*self.on.style;
        let Kind::Table(table) = &node.kind else {
            return div().into_any_element();
        };
        let cell = |cell: &Node| match cell.flow() {
            [Piece::Text(rich)] => Some(rich.clone()),
            _ => None,
        };
        let head = table.head.iter().map(|row| (row, true));
        let rows: Vec<(Vec<Option<Rich>>, bool)> = head
            .chain(node.children.iter().map(|row| (&row.children, false)))
            .map(|(row, head)| (row.iter().map(cell).collect(), head))
            .collect();
        let columns = rows
            .iter()
            .map(|(row, _)| row.len())
            .chain([table.align.len()])
            .max();
        let text =
            |row: &[Option<Rich>], x: usize| row.get(x).cloned().flatten().map(|rich| rich.text);
        let longest: Vec<f32> = (0..columns.unwrap_or(0))
            .map(|x| {
                rows.iter()
                    .filter_map(|(row, _)| text(row, x))
                    .map(|text| text.chars().count())
                    .max()
                    .unwrap_or(0) as f32
            })
            .collect();
        let source: Vec<String> = rows
            .iter()
            .map(|(row, _)| {
                (0..longest.len())
                    .map(|x| text(row, x).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect();
        let copy = self
            .copy(source.join("\n").into())
            .absolute()
            .top(px(4.))
            .right(px(8.));
        let copy = copy
            .text_size(px(10.))
            .text_color(style.ink_faint)
            .invisible();
        let rows: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(y, (row, head))| {
                let cells: Vec<_> = longest
                    .iter()
                    .enumerate()
                    .map(|(x, chars)| {
                        let content = match row.get(x).cloned().flatten() {
                            Some(rich) if head => rich.text.to_uppercase().into_any_element(),
                            Some(rich) => self.rich(&rich),
                            None => div().into_any_element(),
                        };
                        div()
                            .flex_basis(style.table_size * (chars.min(8.) * 0.6) + px(20.))
                            .flex_grow(chars.max(1.))
                            .min_w_0()
                            .px(px(10.))
                            .py(px(5.))
                            .border_color(style.line)
                            .when(x + 1 < longest.len(), Div::border_r_1)
                            .text_align(table.align.get(x).copied().unwrap_or_default())
                            .when(head || x == 0, |cell| cell.font_weight(FontWeight::MEDIUM))
                            .child(content)
                    })
                    .collect();
                let row = div()
                    .flex()
                    .border_color(style.line)
                    .when(y > 0, Div::border_t_1);
                row.when(head, |row| {
                    let size = style.table_size - px(1.5);
                    spaced(row.bg(style.table_head))
                        .text_size(size)
                        .text_color(style.ink_dim)
                })
                .children(cells)
            })
            .collect();
        div()
            .id(self.id("table"))
            .group("table")
            .relative()
            .rounded(style.radius)
            .border_1()
            .border_color(style.line)
            .bg(style.table_ground)
            .text_size(style.table_size)
            .children(rows)
            .child(copy.group_hover("table", |copy| copy.visible()))
            .into_any_element()
    }

    fn details(
        &mut self,
        node: &Node,
        summary: &SharedString,
        written_open: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.folds += 1;
        let key = (self.block, self.folds);
        let turned = self.on.turned.clone();
        let open = written_open != turned.borrow().contains(&key);
        let block = self.on.blocks[self.block].clone();
        let summary = if summary.is_empty() {
            "Details".into()
        } else {
            summary.clone()
        };
        let toggle = div()
            .id(self.id("details"))
            .flex()
            .gap(px(6.))
            .cursor_pointer()
            .text_color(self.on.style.ink_dim)
            .child(if open { "▾" } else { "▸" })
            .child(summary)
            .on_click(move |_, window, _| {
                let mut turned = turned.borrow_mut();
                if !turned.remove(&key) {
                    turned.insert(key);
                }
                block.size.set(None);
                window.refresh();
            });
        let content = open.then(|| div().pl(px(16.)).child(self.flow(node, window, cx)));
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(toggle)
            .children(content)
            .into_any_element()
    }
}

/// Each run's look: its marks, and the selection's ground over the part selected.
fn highlights(
    rich: &Rich,
    selected: Option<Range<usize>>,
    style: &Style,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let ranges = rich
        .marks
        .iter()
        .map(|(range, _)| range.clone())
        .chain(selected.clone());
    let mut edges: Vec<usize> = ranges.flat_map(|range| [range.start, range.end]).collect();
    edges.sort_unstable();
    edges.dedup();
    let inside = |outer: &Range<usize>, inner: &Range<usize>| {
        outer.start <= inner.start && inner.end <= outer.end
    };
    edges
        .windows(2)
        .filter_map(|edge| {
            let run = edge[0]..edge[1];
            let marks = rich
                .marks
                .iter()
                .find(|(range, _)| inside(range, &run))
                .map(|(_, marks)| *marks);
            let chosen = selected
                .as_ref()
                .is_some_and(|selected| inside(selected, &run));
            let mut look = match marks {
                Some(marks) => highlight(marks, style),
                None if chosen => HighlightStyle::default(),
                None => return None,
            };
            if chosen {
                look.background_color = Some(style.selection);
            }
            Some((run, look))
        })
        .collect()
}

fn highlight(marks: Marks, style: &Style) -> HighlightStyle {
    let mut highlight = HighlightStyle::default();
    if marks.has(Marks::STRONG) {
        highlight.font_weight = Some(FontWeight::SEMIBOLD);
    }
    if marks.has(Marks::EMPHASIS) {
        highlight.font_style = Some(FontStyle::Italic);
    }
    if marks.has(Marks::STRIKE) {
        highlight.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        });
    }
    if marks.has(Marks::CODE) {
        highlight.color = Some(style.code_ink);
        highlight.background_color = Some(style.mark_ground);
        #[cfg(feature = "fork")]
        {
            highlight.font_scale = Some(gpui::FontScale(0.88));
        }
    }
    if marks.has(Marks::LINK) {
        highlight.color = Some(style.accent);
    }
    highlight
}

#[cfg(feature = "fork")]
fn spaced(row: Div) -> Div {
    row.letter_spacing(0.06)
}

#[cfg(not(feature = "fork"))]
fn spaced(row: Div) -> Div {
    row
}

fn task_box(checked: bool, style: &Style) -> Div {
    let tick = div()
        .mt(px(5.))
        .size(px(13.))
        .rounded(px(3.))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.))
        .line_height(px(10.));
    if checked {
        tick.bg(style.accent)
            .border_color(style.accent)
            .text_color(gpui::white())
            .child("✓")
    } else {
        tick.border_color(style.ink_faint)
    }
}
