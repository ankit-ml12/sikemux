//! A message's markdown as a tree of blocks, kept up to date by patches.
//! The README describes the tree's JSON and the patches.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;

use gpui::{Pixels, SharedString, Size};
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Debug, Default)]
pub enum Kind {
    #[default]
    Text,
    Paragraph,
    Heading(u8),
    Quote,
    Bullets,
    Numbers(u64),
    Item(Option<bool>),
    Code(Option<SharedString>),
    Rule,
    Table(Box<Table>),
    Row,
    Cell,
    Footnote(u64),
    Details {
        summary: SharedString,
        open: bool,
    },
    Emphasis,
    Strong,
    Strike,
    InlineCode,
    Link(SharedString),
    Image {
        src: SharedString,
        alt: SharedString,
    },
    Break,
    FootnoteRef(u64),
    Unknown,
}

impl Kind {
    fn is_inline(&self) -> bool {
        matches!(
            self,
            Kind::Text
                | Kind::Emphasis
                | Kind::Strong
                | Kind::Strike
                | Kind::InlineCode
                | Kind::Link(_)
                | Kind::Break
                | Kind::FootnoteRef(_)
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub kind: Kind,
    /// A text node's text, or a code element's.
    pub text: String,
    pub children: Vec<Node>,
    flow: OnceCell<Vec<Piece>>,
    /// A top-level block's size when last laid out, at the width it had then.
    pub(crate) size: Cell<Option<Size<Pixels>>>,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub align: Vec<Align>,
    pub head: Option<Vec<Node>>,
}

/// What a run of text is marked as, combined from the elements around it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks(u8);

impl Marks {
    pub const STRONG: Marks = Marks(1);
    pub const EMPHASIS: Marks = Marks(2);
    pub const STRIKE: Marks = Marks(4);
    pub const CODE: Marks = Marks(8);
    pub const LINK: Marks = Marks(16);

    pub fn has(self, other: Marks) -> bool {
        self.0 & other.0 != 0
    }

    fn with(self, other: Marks) -> Marks {
        Marks(self.0 | other.0)
    }
}

/// A run of inline content flattened into one string, ready to lay out.
#[derive(Clone, Debug, Default)]
pub struct Rich {
    pub text: SharedString,
    pub marks: Vec<(Range<usize>, Marks)>,
    pub links: Vec<(Range<usize>, SharedString)>,
}

/// An element's content in reading order: runs of inline text, and the
/// children that stand as blocks of their own.
#[derive(Clone, Debug)]
pub enum Piece {
    Text(Rich),
    Block(usize),
}

impl Node {
    fn text(text: String) -> Node {
        Node {
            text,
            ..Node::default()
        }
    }

    /// The node's content as it is drawn, worked out once until a patch reaches inside it.
    pub fn flow(&self) -> &[Piece] {
        self.flow.get_or_init(|| {
            if let Kind::Code(_) = self.kind {
                return vec![Piece::Text(Rich {
                    text: self.text.trim_end_matches('\n').to_owned().into(),
                    ..Rich::default()
                })];
            }
            let mut pieces = Vec::new();
            let mut rich = None;
            for (ix, child) in self.children.iter().enumerate() {
                if child.kind.is_inline() {
                    flatten(
                        child,
                        Marks::default(),
                        None,
                        rich.get_or_insert_with(Builder::default),
                    );
                } else {
                    if let Some(rich) = rich.take() {
                        pieces.push(Piece::Text(rich.finish()));
                    }
                    pieces.push(Piece::Block(ix));
                }
            }
            if let Some(rich) = rich {
                pieces.push(Piece::Text(rich.finish()));
            }
            pieces
        })
    }

    fn touched(&mut self) {
        self.flow = OnceCell::new();
        self.size.set(None);
    }
}

#[derive(Default)]
struct Builder {
    text: String,
    marks: Vec<(Range<usize>, Marks)>,
    links: Vec<(Range<usize>, SharedString)>,
}

impl Builder {
    fn push(&mut self, text: &str, marks: Marks) {
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        if marks == Marks::default() || start == end {
            return;
        }
        match self.marks.last_mut() {
            Some((range, last)) if range.end == start && *last == marks => range.end = end,
            _ => self.marks.push((start..end, marks)),
        }
    }

    fn finish(self) -> Rich {
        Rich {
            text: self.text.into(),
            marks: self.marks,
            links: self.links,
        }
    }
}

fn flatten(node: &Node, marks: Marks, link: Option<&SharedString>, into: &mut Builder) {
    let (marks, link) = match &node.kind {
        Kind::Text => return into.push(&node.text, marks),
        Kind::InlineCode => return into.push(&node.text, marks.with(Marks::CODE)),
        Kind::Break => return into.push("\n", marks),
        Kind::FootnoteRef(n) => return into.push(&superscript(*n), marks),
        Kind::Image { alt, .. } => return into.push(alt, marks),
        Kind::Strong => (marks.with(Marks::STRONG), link),
        Kind::Emphasis => (marks.with(Marks::EMPHASIS), link),
        Kind::Strike => (marks.with(Marks::STRIKE), link),
        Kind::Link(href) if link.is_none() => (marks.with(Marks::LINK), Some(href)),
        _ => (marks, link),
    };
    let start = into.text.len();
    for child in &node.children {
        flatten(child, marks, link, into);
    }
    if let (Some(href), Kind::Link(_)) = (link, &node.kind) {
        if into.text.len() > start {
            into.links.push((start..into.text.len(), href.clone()));
        }
    }
}

pub fn superscript(n: u64) -> String {
    const DIGITS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    n.to_string()
        .bytes()
        .map(|digit| DIGITS[usize::from(digit - b'0')])
        .collect()
}

/// What the list at a place in the tree holds, which says how to read its JSON.
#[derive(Clone, Copy)]
enum Holds {
    Nodes,
    Rows,
    Cells,
}

impl Holds {
    fn inside(kind: &Kind) -> Holds {
        match kind {
            Kind::Table(_) => Holds::Rows,
            Kind::Row => Holds::Cells,
            _ => Holds::Nodes,
        }
    }
}

fn read_list(values: &[Value], holds: Holds) -> Vec<Node> {
    values.iter().map(|value| read(value, holds)).collect()
}

fn read(value: &Value, holds: Holds) -> Node {
    match holds {
        Holds::Rows => Node {
            kind: Kind::Row,
            children: read_list(as_list(value), Holds::Cells),
            ..Node::default()
        },
        Holds::Cells => Node {
            kind: Kind::Cell,
            children: read_list(as_list(value), Holds::Nodes),
            ..Node::default()
        },
        Holds::Nodes => match value {
            Value::String(text) => Node::text(text.clone()),
            Value::Object(_) => read_element(value),
            _ => Node::text(String::new()),
        },
    }
}

fn as_list(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn string(value: &Value, key: &str) -> SharedString {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
        .into()
}

fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn read_element(value: &Value) -> Node {
    let tag = value.get("t").and_then(Value::as_str).unwrap_or_default();
    let kind = match tag {
        "p" => Kind::Paragraph,
        "h" => Kind::Heading(number(value, "l").unwrap_or(1).clamp(1, 6) as u8),
        "quote" => Kind::Quote,
        "ul" => Kind::Bullets,
        "ol" => Kind::Numbers(number(value, "start").unwrap_or(1)),
        "li" => Kind::Item(value.get("checked").and_then(Value::as_bool)),
        "pre" => Kind::Code(
            value
                .get("lang")
                .and_then(Value::as_str)
                .map(|lang| lang.to_owned().into()),
        ),
        "hr" => Kind::Rule,
        "table" => Kind::Table(Box::new(Table {
            align: as_list(value.get("align").unwrap_or(&Value::Null))
                .iter()
                .map(|align| match align.as_str() {
                    Some("center") => Align::Center,
                    Some("right") => Align::End,
                    _ => Align::Start,
                })
                .collect(),
            head: value
                .get("head")
                .map(|head| read_list(as_list(head), Holds::Cells)),
        })),
        "fndef" => Kind::Footnote(number(value, "n").unwrap_or(0)),
        "details" => Kind::Details {
            summary: string(value, "summary"),
            open: value.get("open").and_then(Value::as_bool).unwrap_or(false),
        },
        "em" => Kind::Emphasis,
        "strong" => Kind::Strong,
        "del" => Kind::Strike,
        "code" => Kind::InlineCode,
        "a" => Kind::Link(string(value, "href")),
        "img" => Kind::Image {
            src: string(value, "src"),
            alt: string(value, "alt"),
        },
        "br" => Kind::Break,
        "fnref" => Kind::FootnoteRef(number(value, "n").unwrap_or(0)),
        _ => Kind::Unknown,
    };
    let holds = Holds::inside(&kind);
    let children = value
        .get("c")
        .or_else(|| value.get("rows"))
        .map(|list| read_list(as_list(list), holds))
        .unwrap_or_default();
    Node {
        kind,
        text: value
            .get("v")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        children,
        ..Node::default()
    }
}

/// The byte in `text` where its first `units` UTF-16 units end.
fn byte_at_units(text: &str, units: usize) -> usize {
    let mut counted = 0;
    for (at, ch) in text.char_indices() {
        if counted >= units {
            return at;
        }
        counted += ch.len_utf16();
    }
    text.len()
}

/// One message's blocks, as the last patch left them, and what drawing them
/// last left: the width they had and the folds the reader opened or closed.
#[derive(Default)]
pub struct Markdown {
    blocks: Vec<Rc<Node>>,
    /// The text's length the tree matches, or `None` once a patch did not fit
    /// and the tree waits for a whole one.
    units: Option<usize>,
    pub(crate) width: Rc<Cell<Option<Pixels>>>,
    /// Each fold the reader turned from how it was written, by block and its place in the block.
    pub(crate) turned: Rc<RefCell<HashSet<(usize, u64)>>>,
}

impl Markdown {
    /// A tree read from its JSON blocks, matching a text `units` UTF-16 units long.
    pub fn new(blocks: &Value, units: usize) -> Self {
        Markdown {
            blocks: read_list(as_list(blocks), Holds::Nodes)
                .into_iter()
                .map(Rc::new)
                .collect(),
            units: Some(units),
            ..Markdown::default()
        }
    }

    /// A message with no tree yet; one arrives as a patch that starts from nothing.
    pub fn empty() -> Self {
        Markdown::default()
    }

    pub fn blocks(&self) -> &[Rc<Node>] {
        &self.blocks
    }

    /// Whether the tree matches the text; when it does not, draw the text as it is.
    pub fn is_current(&self) -> bool {
        self.units.is_some()
    }

    /// Applies a patch `{ n, o }`, leaving every block it does not reach as it was.
    pub fn write(&mut self, patch: &Value, units: usize) {
        let ops = as_list(patch.get("o").unwrap_or(&Value::Null));
        let whole = ops.first().is_some_and(|op| {
            as_list(op.get("p").unwrap_or(&Value::Null)).is_empty()
                && number(op, "at") == Some(0)
                && op.get("c").is_some()
        });
        if !whole && self.units.is_none() {
            return;
        }
        for op in ops {
            self.apply(op);
        }
        let n = number(patch, "n").and_then(|n| usize::try_from(n).ok());
        self.units = (n == Some(units)).then_some(units);
    }

    fn apply(&mut self, op: &Value) {
        let path: Vec<usize> = as_list(op.get("p").unwrap_or(&Value::Null))
            .iter()
            .filter_map(|ix| ix.as_u64().and_then(|ix| usize::try_from(ix).ok()))
            .collect();
        let at = number(op, "at")
            .and_then(|at| usize::try_from(at).ok())
            .unwrap_or(0);
        let Some((first, rest)) = path.split_first() else {
            if let Some(added) = op.get("c") {
                self.blocks.truncate(at);
                let added = read_list(as_list(added), Holds::Nodes);
                self.blocks.extend(added.into_iter().map(Rc::new));
            }
            return;
        };
        let Some(block) = self.blocks.get_mut(*first) else {
            return;
        };
        let mut node = Rc::make_mut(block);
        node.touched();
        for &ix in rest {
            let Some(child) = node.children.get_mut(ix) else {
                return;
            };
            node = child;
            node.touched();
        }
        if let Some(added) = op.get("c") {
            let holds = Holds::inside(&node.kind);
            node.children.truncate(at);
            node.children.extend(read_list(as_list(added), holds));
        } else if let Some(added) = op.get("s").and_then(Value::as_str) {
            let keep = byte_at_units(&node.text, at);
            node.text.truncate(keep);
            node.text.push_str(added);
        }
    }
}
