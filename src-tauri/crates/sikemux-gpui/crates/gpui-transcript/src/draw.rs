use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, px, relative, rgb, rgba, AnyElement, App, ClipboardItem, Div, ElementId, Font,
    FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveElement, InteractiveText, IntoElement,
    ParentElement, Pixels, SharedString, StatefulInteractiveElement, StrikethroughStyle, Styled,
    StyledText, Window,
};

use crate::markdown::{superscript, Align, Kind, Markdown, Marks, Node, Piece, Rich};

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
    pub heading_ink: Hsla,
    pub accent: Hsla,
    pub code_ink: Hsla,
    pub code_ground: Hsla,
    pub mark_ground: Hsla,
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
            heading_ink: rgb(0xe7e5ef).into(),
            accent: rgb(0xa277ff).into(),
            code_ink: rgb(0xb494fb).into(),
            code_ground: rgba(0x09090bb8).into(),
            mark_ground: rgba(0xa277ff1a).into(),
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

/// Draws a message's markdown. `id` must be unique in the window, such as the row's and part's ids.
pub fn markdown(
    id: impl Into<ElementId>,
    doc: &Markdown,
    style: &Style,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = id.into();
    let mut draw = Draw {
        style,
        scope: Arc::new(id.clone()),
        next: 0,
        depth: 0,
    };
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(style.gap)
        .min_w_0()
        .font_family(style.font.clone())
        .text_size(style.text_size)
        .line_height(relative(style.line_height))
        .text_color(style.ink)
        .children(
            doc.blocks()
                .iter()
                .enumerate()
                .map(|(ix, block)| draw.block(block, ix, window, cx)),
        )
        .into_any_element()
}

/// A message's text drawn as it was written, for when its tree is not there.
pub fn plain(text: impl Into<SharedString>, style: &Style) -> AnyElement {
    div()
        .font_family(style.font.clone())
        .text_size(style.text_size)
        .line_height(relative(style.line_height))
        .text_color(style.ink)
        .child(StyledText::new(text))
        .into_any_element()
}

struct Draw<'a> {
    style: &'a Style,
    scope: Arc<ElementId>,
    next: u64,
    depth: usize,
}

impl Draw<'_> {
    fn id(&mut self, name: &'static str) -> ElementId {
        self.next += 1;
        ElementId::NamedChild(self.scope.clone(), format!("{name}{}", self.next).into())
    }

    fn block(&mut self, node: &Node, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let style = self.style;
        match &node.kind {
            Kind::Heading(level) => {
                let scale = match level {
                    1 => 2.,
                    2 => 1.5,
                    3 => 1.17,
                    4 => 1.,
                    5 => 0.83,
                    _ => 0.67,
                };
                div()
                    .when(ix > 0, |heading| {
                        heading.pt(style.text_size * (scale * 0.4))
                    })
                    .text_size(style.text_size * scale)
                    .line_height(relative(1.3))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(style.heading_ink)
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
        div()
            .flex()
            .flex_col()
            .gap(self.style.gap)
            .min_w_0()
            .children(pieces.iter().map(|piece| match piece {
                Piece::Text(rich) => self.rich(rich),
                Piece::Block(ix) => self.block(&node.children[*ix], *ix, window, cx),
            }))
            .into_any_element()
    }

    fn rich(&mut self, rich: &Rich) -> AnyElement {
        let style = self.style;
        let text = StyledText::new(rich.text.clone())
            .with_highlights(
                rich.marks
                    .iter()
                    .map(|(range, marks)| (range.clone(), highlight(*marks, style))),
            )
            .with_font_family_overrides(
                rich.marks
                    .iter()
                    .filter(|(_, marks)| marks.has(Marks::CODE))
                    .map(|(range, _)| (range.clone(), style.mono.family.clone())),
            );
        if rich.links.is_empty() {
            return text.into_any_element();
        }
        let links: Rc<[SharedString]> = rich.links.iter().map(|(_, href)| href.clone()).collect();
        let on_link = style.on_link.clone();
        InteractiveText::new(self.id("text"), text)
            .on_click(
                rich.links.iter().map(|(range, _)| range.clone()).collect(),
                move |ix, window, cx| {
                    let Some(href) = links.get(ix) else { return };
                    match &on_link {
                        Some(on_link) => on_link(href, window, cx),
                        None => cx.open_url(href),
                    }
                },
            )
            .into_any_element()
    }

    fn list(&mut self, node: &Node, window: &mut Window, cx: &mut App) -> AnyElement {
        let style = self.style;
        let first = match node.kind {
            Kind::Numbers(start) => Some(start),
            _ => None,
        };
        let bullet = ["•", "◦", "▪"][self.depth % 3];
        self.depth += 1;
        let items = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .children(node.children.iter().enumerate().map(|(ix, item)| {
                let marker = match (&item.kind, first) {
                    (Kind::Item(Some(checked)), _) => task_box(*checked, style).into_any_element(),
                    (_, Some(start)) => div()
                        .text_right()
                        .pr(px(6.))
                        .child(format!("{}.", start + ix as u64))
                        .into_any_element(),
                    _ => div().pl(px(6.)).child(bullet).into_any_element(),
                };
                div()
                    .flex()
                    .child(div().w(px(22.)).flex_none().child(marker))
                    .child(div().flex_1().min_w_0().child(self.flow(item, window, cx)))
            }))
            .into_any_element();
        self.depth -= 1;
        items
    }

    fn fence(
        &mut self,
        node: &Node,
        lang: Option<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let style = self.style;
        let code = match node.flow() {
            [Piece::Text(rich)] => rich.text.clone(),
            _ => SharedString::default(),
        };
        let id = self.id("code");
        if let Some(fence) = &style.fence {
            return fence(Fence { id, lang, code }, window, cx);
        }
        let copied = code.clone();
        div()
            .flex()
            .flex_col()
            .rounded(style.radius)
            .border_1()
            .border_color(style.line_soft)
            .bg(style.code_ground)
            .child(
                div()
                    .h(px(23.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(style.line_soft)
                    .text_size(px(10.))
                    .text_color(style.ink_faint)
                    .child(div().flex_1().min_w_0().truncate().children(lang))
                    .child(
                        div()
                            .id(self.id("copy"))
                            .cursor_pointer()
                            .hover(|copy| copy.text_color(style.ink))
                            .child("Copy")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copied.to_string()))
                            }),
                    ),
            )
            .child(
                div()
                    .id(id)
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .px(px(12.))
                    .py(px(9.))
                    .font(style.mono.clone())
                    .text_size(style.code_size)
                    .line_height(relative(1.55))
                    .text_color(style.ink_dim)
                    .child(StyledText::new(code)),
            )
            .into_any_element()
    }

    fn table(&mut self, node: &Node) -> AnyElement {
        let style = self.style;
        let Kind::Table(table) = &node.kind else {
            return div().into_any_element();
        };
        let rows = &node.children;
        let columns = table
            .align
            .len()
            .max(rows.iter().map(|row| row.children.len()).max().unwrap_or(0));
        let cell = |column: usize, last_row: bool| {
            let align = table.align.get(column).copied().unwrap_or_default();
            div()
                .px(px(10.))
                .py(px(5.))
                .flex()
                .whitespace_nowrap()
                .when(align == Align::Center, Div::justify_center)
                .when(align == Align::End, Div::justify_end)
                .when(!last_row, |cell| cell.border_b_1().border_color(style.line))
        };
        let mut grid = div().flex().min_w_full();
        for column in 0..columns {
            let last_column = column + 1 == columns;
            let mut cells = div()
                .flex()
                .flex_col()
                .flex_grow(1.)
                .when(!last_column, |cells| {
                    cells.border_r_1().border_color(style.line)
                });
            if let Some(head) = &table.head {
                let label = head
                    .get(column)
                    .and_then(|cell| match cell.flow() {
                        [Piece::Text(rich)] => Some(rich.text.to_uppercase()),
                        _ => None,
                    })
                    .unwrap_or_default();
                cells = cells.child(
                    cell(column, rows.is_empty())
                        .bg(style.table_head)
                        .text_size(style.table_size - px(1.5))
                        .font_weight(FontWeight::MEDIUM)
                        .letter_spacing(0.06)
                        .text_color(style.ink_dim)
                        .child(label),
                );
            }
            for (ix, row) in rows.iter().enumerate() {
                let content = row
                    .children
                    .get(column)
                    .map(|content| self.cell(content))
                    .unwrap_or_else(|| div().into_any_element());
                cells = cells.child(
                    cell(column, ix + 1 == rows.len())
                        .when(column == 0, |cell| cell.font_weight(FontWeight::MEDIUM))
                        .child(content),
                );
            }
            grid = grid.child(cells);
        }
        div()
            .id(self.id("table"))
            .overflow_x_scroll()
            .rounded(style.radius)
            .border_1()
            .border_color(style.line)
            .bg(style.table_ground)
            .text_size(style.table_size)
            .child(grid)
            .into_any_element()
    }

    fn cell(&mut self, cell: &Node) -> AnyElement {
        match cell.flow() {
            [Piece::Text(rich)] => self.rich(rich),
            _ => div().into_any_element(),
        }
    }

    fn details(
        &mut self,
        node: &Node,
        summary: &SharedString,
        open: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let id = self.id("details");
        let state = window.use_keyed_state(id.clone(), cx, |_, _| open);
        let open = *state.read(cx);
        let summary = if summary.is_empty() {
            SharedString::new_static("Details")
        } else {
            summary.clone()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .id(id)
                    .flex()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_color(self.style.ink_dim)
                    .child(if open { "▾" } else { "▸" })
                    .child(summary)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |open, cx| {
                            *open = !*open;
                            cx.notify();
                        })
                    }),
            )
            .when(open, |details| {
                details.child(div().pl(px(16.)).child(self.flow(node, window, cx)))
            })
            .into_any_element()
    }
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
    }
    if marks.has(Marks::LINK) {
        highlight.color = Some(style.accent);
    }
    highlight
}

fn task_box(checked: bool, style: &Style) -> Div {
    div()
        .mt(px(5.))
        .size(px(13.))
        .rounded(px(3.))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.))
        .line_height(px(10.))
        .map(|tick| {
            if checked {
                tick.bg(style.accent)
                    .border_color(style.accent)
                    .text_color(gpui::white())
                    .child("✓")
            } else {
                tick.border_color(style.ink_faint)
            }
        })
}
