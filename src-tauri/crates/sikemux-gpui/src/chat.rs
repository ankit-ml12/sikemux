//! An agent's chat: the rows the core folds, kept in a transcript, and how
//! each row draws. Rows change only by the core's changes.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::base::Scrollbar;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, relative, AnyElement, App, AppContext, Entity, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, Radians, SharedString, StatefulInteractiveElement, Styled, Transformation, WeakEntity, Window,
};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::Subscription;
use tokio::sync::mpsc::UnboundedSender;
use gpui_transcript::{Change, Markdown, Pen, Row, Style, Transcript};
use serde_json::{json, Map, Value};
use sikemux_wire::chat::{ChatChange, ChatMessage, ChatPage, ChatPart, ChatRole, ChatView, PartTree, SubagentState, WorkFold};
use sikemux_wire::protocol::{ChatAttachment, ChatMark};

use crate::core::{Ask, ChatAsk};
use crate::ui::{Button, Variant};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui;
use crate::workbench::Workbench;

/// Turns asked for each time the reader nears the top of what is held.
const PAGE_TURNS: u32 = 20;

/// One message as the core folded it, and the tree of each text it holds.
pub struct ChatRow {
    message: ChatMessage,
    docs: Vec<Doc>,
}

/// A text part's markdown, and its text's length in UTF-16 units, which the core's patches count.
struct Doc {
    part: String,
    markdown: Markdown,
    units: usize,
}

impl Doc {
    fn new(part: &str, text: &str, md: Option<&PartTree>) -> Doc {
        let units = text.encode_utf16().count();
        let markdown = match md {
            Some(tree) => Markdown::new(&tree.blocks, units_of(tree.n)),
            None => Markdown::new(&paragraphs(text), units),
        };
        Doc { part: part.to_owned(), markdown, units }
    }
}

fn units_of(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// Text the core sends without a tree, as the paragraphs it was written in.
fn paragraphs(text: &str) -> Value {
    let lines = |paragraph: &str| -> Vec<Value> {
        let mut nodes = Vec::new();
        for (ix, line) in paragraph.lines().enumerate() {
            if ix > 0 {
                nodes.push(json!({ "t": "br" }));
            }
            nodes.push(json!(line));
        }
        nodes
    };
    Value::Array(
        text.split("\n\n")
            .filter(|paragraph| !paragraph.trim().is_empty())
            .map(|paragraph| json!({ "t": "p", "c": lines(paragraph) }))
            .collect(),
    )
}

fn text_of(part: &ChatPart) -> Option<(&str, Option<&PartTree>)> {
    match part {
        ChatPart::Text { text, md, .. } | ChatPart::Thought { text, md, .. } => Some((text, md.as_ref())),
        _ => None,
    }
}

impl ChatRow {
    fn new(message: ChatMessage) -> ChatRow {
        let docs = message
            .parts
            .iter()
            .filter_map(|part| text_of(part).map(|(text, md)| Doc::new(part.id(), text, md)))
            .collect();
        ChatRow { message, docs }
    }

    fn doc(&self, part: &str) -> Option<&Doc> {
        self.docs.iter().find(|doc| doc.part == part)
    }

    fn put_part(&mut self, part: ChatPart) {
        if let Some((text, md)) = text_of(&part) {
            let doc = Doc::new(part.id(), text, md);
            match self.docs.iter_mut().find(|held| held.part == doc.part) {
                Some(held) => *held = doc,
                None => self.docs.push(doc),
            }
        }
        match self.message.parts.iter_mut().find(|held| held.id() == part.id()) {
            Some(held) => *held = part,
            None => self.message.parts.push(part),
        }
    }

    fn write(&mut self, part: &str, more: &str, md: Option<&Value>) {
        let Some(ChatPart::Text { text, .. } | ChatPart::Thought { text, .. }) =
            self.message.parts.iter_mut().find(|held| held.id() == part)
        else {
            return;
        };
        text.push_str(more);
        if let Some(doc) = self.docs.iter_mut().find(|doc| doc.part == part) {
            doc.units += more.encode_utf16().count();
            if let Some(md) = md {
                doc.markdown.write(md, doc.units);
            }
        }
    }

    fn set(&mut self, fields: &Value) {
        let Ok(mut whole) = serde_json::to_value(&self.message) else {
            return;
        };
        merge(&mut whole, fields);
        if let Ok(message) = serde_json::from_value(whole) {
            self.message = message;
        }
    }
}

impl Row for ChatRow {
    fn id(&self) -> &str {
        &self.message.id
    }

    fn docs(&self) -> Vec<&Markdown> {
        self.message
            .parts
            .iter()
            .filter_map(|part| self.doc(part.id()))
            .map(|doc| &doc.markdown)
            .collect()
    }
}

/// `fields` laid over `on`; a field sent as null is gone.
fn merge(on: &mut Value, fields: &Value) {
    let (Some(on), Some(fields)) = (on.as_object_mut(), fields.as_object()) else {
        return;
    };
    for (key, value) in fields {
        if value.is_null() {
            on.remove(key);
        } else {
            on.insert(key.clone(), value.clone());
        }
    }
}

/// A chat the window shows: its rows in a transcript, how it stands, its
/// subagents' rows, and where in the core's changes it has got to.
pub struct Chat {
    pub transcript: Entity<Transcript<ChatRow>>,
    head: Map<String, Value>,
    subagents: HashMap<String, Vec<ChatRow>>,
    pub mark: Option<ChatMark>,
    older_before: Option<u64>,
    asked_earlier: bool,
    /// What the person is writing to the agent, made once the window can make it.
    pub composer: Option<(Entity<TextareaState>, Subscription)>,
}

impl Chat {
    pub fn new(agent: String, provider: &str, bench: WeakEntity<Workbench>, cx: &mut App) -> Chat {
        let looks = Looks::new(Tokens::get(cx), provider);
        let transcript = cx.new(|cx| {
            Transcript::new(Vec::new(), looks.text.clone(), move |rows, ix, pen, window, cx| row(rows, ix, pen, &looks, window, cx), cx)
                .on_reach_top(move |_, cx| {
                    bench.update(cx, |bench, _| bench.ask_earlier(&agent)).ok();
                })
        });
        Chat { transcript, head: Map::new(), subagents: HashMap::new(), mark: None, older_before: None, asked_earlier: false, composer: None }
    }

    /// What the core answered an attach with.
    pub fn attached(&mut self, attachment: ChatAttachment, cx: &mut App) {
        match attachment {
            ChatAttachment::Live { view, mark } => {
                match serde_json::from_value::<ChatView>(view) {
                    Ok(view) => self.show(view, cx),
                    Err(error) => eprintln!("sikemux: the core's chat view does not read: {error}"),
                }
                self.mark = Some(mark);
            }
            ChatAttachment::Resumed { changes, mark } => {
                self.apply(changes, cx);
                self.mark = Some(mark);
            }
            ChatAttachment::Missing => {}
        }
    }

    /// Takes the core's changes numbered `seq`, and says whether some before them went missing.
    pub fn changed(&mut self, seq: u64, changes: Value, cx: &mut App) -> bool {
        let Some(mark) = self.mark.as_mut() else {
            return false;
        };
        if seq <= mark.seq {
            return false;
        }
        if seq != mark.seq + 1 {
            return true;
        }
        mark.seq = seq;
        self.apply(changes, cx);
        false
    }

    fn show(&mut self, view: ChatView, cx: &mut App) {
        self.head = serde_json::to_value(&view.head).ok().and_then(|head| head.as_object().cloned()).unwrap_or_default();
        self.subagents = rows_by_session(view.subagents);
        self.older_before = view.older_before;
        self.asked_earlier = false;
        let rows = view.rows.into_iter().map(ChatRow::new).collect();
        self.transcript.update(cx, |transcript, cx| transcript.push(Change::Reset(rows), cx));
    }

    /// A turn is running, so what the person sends steers it.
    pub fn running(&self) -> bool {
        self.head.get("running").and_then(Value::as_bool).unwrap_or(false)
    }

    /// The earlier turns to ask for, when there are more and none are on their way.
    pub fn earlier(&mut self, agent: &str) -> Option<ChatAsk> {
        let (before, mark) = (self.older_before?, self.mark.as_ref()?);
        if self.asked_earlier {
            return None;
        }
        self.asked_earlier = true;
        Some(ChatAsk::History { agent: agent.to_owned(), feed: mark.feed.clone(), before, turns: PAGE_TURNS })
    }

    pub fn page(&mut self, page: Value, cx: &mut App) {
        self.asked_earlier = false;
        let Ok(page) = serde_json::from_value::<ChatPage>(page) else {
            return;
        };
        self.older_before = page.older_before;
        for (session, rows) in rows_by_session(page.subagents) {
            self.subagents.entry(session).or_insert(rows);
        }
        let rows = page.rows.into_iter().map(ChatRow::new).collect();
        self.transcript.update(cx, |transcript, cx| transcript.push(Change::Prepend(rows), cx));
    }

    fn apply(&mut self, changes: Value, cx: &mut App) {
        let changes = match serde_json::from_value::<Vec<ChatChange>>(changes) {
            Ok(changes) => changes,
            Err(error) => return eprintln!("sikemux: the core's chat changes do not read: {error}"),
        };
        let mut rows = Vec::new();
        for change in changes {
            match change {
                ChatChange::Head { set } => {
                    let mut head = Value::Object(std::mem::take(&mut self.head));
                    merge(&mut head, &set);
                    self.head = head.as_object().cloned().unwrap_or_default();
                }
                ChatChange::View { view } => {
                    self.transcript.update(cx, |transcript, cx| transcript.extend(rows.drain(..), cx));
                    self.show(*view, cx);
                }
                ChatChange::Row { row, within: Some(session) } => {
                    let held = self.subagents.entry(session).or_default();
                    match held.iter_mut().find(|held| held.message.id == row.id) {
                        Some(held) => *held = ChatRow::new(row),
                        None => held.push(ChatRow::new(row)),
                    }
                }
                ChatChange::Row { row, within: None } => rows.push(Change::Put(ChatRow::new(row))),
                ChatChange::Part { row, part, within } => self.update(&mut rows, row, within, move |held| held.put_part(part)),
                ChatChange::Text { row, part, text, md, within } => {
                    self.update(&mut rows, row, within, move |held| held.write(&part, &text, md.as_ref()))
                }
                ChatChange::Set { row, set, within } => self.update(&mut rows, row, within, move |held| held.set(&set)),
                ChatChange::Keep { rows: kept } => rows.push(Change::Keep(kept.into_iter().map(SharedString::from).collect())),
                ChatChange::Drop { from } => rows.push(Change::DropFrom(from.into())),
            }
        }
        self.transcript.update(cx, |transcript, cx| transcript.extend(rows, cx));
    }

    /// Changes a row of the chat, or of one of its subagents.
    fn update(
        &mut self,
        rows: &mut Vec<Change<ChatRow>>,
        row: String,
        within: Option<String>,
        change: impl FnOnce(&mut ChatRow) + 'static,
    ) {
        match within {
            Some(session) => {
                let held = self.subagents.get_mut(&session).and_then(|held| held.iter_mut().find(|held| held.message.id == row));
                if let Some(held) = held {
                    change(held);
                }
            }
            None => rows.push(Change::Update(row.into(), Box::new(change))),
        }
    }
}

fn rows_by_session(subagents: impl IntoIterator<Item = (String, Vec<ChatMessage>)>) -> HashMap<String, Vec<ChatRow>> {
    subagents
        .into_iter()
        .map(|(session, rows)| (session, rows.into_iter().map(ChatRow::new).collect()))
        .collect()
}

/// The chat in the stage: its rows, with a scrollbar over their right edge,
/// and under them what the person writes.
pub fn pane(agent: &str, chat: &Chat, asks: &UnboundedSender<Ask>, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx);
    let list = chat.transcript.read(cx).list().clone();
    let rows = div()
        .flex_1()
        .min_h_0()
        .relative()
        .child(chat.transcript.clone())
        .child(div().absolute().top_0().right_0().bottom_0().child(Scrollbar::vertical(&list)));
    let composer = chat.composer.as_ref().map(|(draft, _)| {
        let (asks, agent) = (asks.clone(), agent.to_owned());
        let stop = Button::new("chat-stop", "Stop", Variant::Outline).icon(Icon::Stop).on_click(move |_, _, _| {
            let _ = asks.send(Ask::Chat(ChatAsk::Cancel { agent: agent.clone() }));
        });
        div().px(px(16.)).pb(px(16.)).flex().justify_center().child(
            div()
                .w_full()
                .max_w(px(820.))
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .border_1()
                .border_color(t.border)
                .rounded(theme::RADIUS_3)
                .bg(t.surface_raised)
                .child(Textarea::new(draft).appearance(false).text_size(px(14.5)))
                .when(chat.running(), |composer| composer.child(div().flex().justify_end().child(stop))),
        )
    });
    div().size_full().flex().flex_col().child(rows).children(composer)
}

/// How the chat's markdown looks: an answer, a thought, and what the person wrote.
struct Looks {
    text: Rc<Style>,
    thought: Rc<Style>,
    user: Rc<Style>,
    line: Hsla,
    bubble: Hsla,
    quiet: Hsla,
    faint: Hsla,
    ink: Hsla,
    hover: Hsla,
    rule: Hsla,
    provider: String,
    /// The finished turns the reader opened, by their first row.
    opened: Rc<RefCell<HashSet<String>>>,
}

impl Looks {
    fn new(t: &Tokens, provider: &str) -> Looks {
        let mut mono = gpui_kit::font(theme::MONO);
        mono.fallbacks = Some(gpui_kit::FontFallbacks::from_fonts(vec![theme::MONO_ICONS.into()]));
        let text = Style {
            font: theme::UI.into(),
            mono,
            ink: t.text_primary,
            ink_dim: t.text_tertiary,
            ink_faint: t.text_disabled,
            accent: t.accent,
            line: Hsla { a: 0.8, ..t.line },
            line_soft: t.line_soft,
            selection: Hsla { a: 0.3, ..t.accent },
            mark_ground: Hsla { a: 0.1, ..t.accent },
            table_ground: Hsla { a: 0.72, ..t.line_soft },
            code_ground: Hsla { a: 0.72, ..t.surface_sunken },
            ..Style::default()
        };
        let thought = Style { text_size: px(12.5), ink: t.text_disabled, ..text.clone() };
        let user = Style { text_size: px(13.5), ..text.clone() };
        Looks {
            text: Rc::new(text),
            thought: Rc::new(thought),
            user: Rc::new(user),
            line: Hsla { a: 0.8, ..t.line },
            bubble: Hsla { a: 0.72, ..t.line_soft },
            quiet: t.text_tertiary,
            faint: t.text_disabled,
            ink: t.text_primary,
            hover: t.surface_hover,
            rule: Hsla { a: 0.14, ..t.text_primary },
            provider: provider.to_owned(),
            opened: Rc::default(),
        }
    }
}

/// A message's row: the person's in a bubble at the right, the agent's across
/// the column. A finished turn folds its work behind a line saying how long it
/// worked, which the reader opens; its reply stays out.
fn row(rows: &[ChatRow], ix: usize, pen: &Pen, looks: &Looks, window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(row) = rows.get(ix) else {
        return div().into_any_element();
    };
    let message = &row.message;
    let user = message.role == ChatRole::User;
    let fold = message.fold.as_ref();
    let folded = fold.is_some_and(|fold| !looks.opened.borrow().contains(&fold.id));
    let opens = fold.filter(|fold| ix == 0 || rows[ix - 1].message.fold.as_ref().is_none_or(|before| before.id != fold.id));
    let from = match fold {
        Some(fold) if folded && fold.last != message.id => usize::MAX,
        Some(fold) if folded => usize::try_from(fold.from).unwrap_or(0),
        _ => 0,
    };
    let shown = message.parts.iter().enumerate().filter(|(index, part)| {
        let reply = matches!(part, ChatPart::Text { .. } | ChatPart::Content { .. });
        from == 0 || outlives(part) || *index >= from && reply
    });
    let parts: Vec<AnyElement> = shown.map(|(_, part)| self::part(row, part, user, pen, looks, window, cx)).collect();
    let letters: Vec<_> = message
        .letters
        .iter()
        .map(|letter| {
            let from = ui::text(format!("From @{}", letter.name), 11., FontWeight::MEDIUM).text_color(looks.quiet);
            div().flex().flex_col().gap(px(4.)).child(from).child(div().text_size(px(13.5)).child(letter.text.clone()))
        })
        .collect();
    let summary = opens.map(|fold| work(fold, !folded, looks));
    if parts.is_empty() && letters.is_empty() && summary.is_none() {
        return div().into_any_element();
    }
    let content = if user {
        div()
            .max_w(relative(0.8))
            .px(px(12.))
            .py(px(8.))
            .border_1()
            .border_color(looks.line)
            .rounded(theme::RADIUS_3)
            .bg(looks.bubble)
            .children(parts)
    } else {
        div().flex().flex_col().gap(px(10.)).min_w_0().children(summary).children(parts).children(letters)
    };
    div()
        .w_full()
        .flex()
        .justify_center()
        .child(
            div()
                .w_full()
                .max_w(px(800.))
                .px(px(22.))
                .map(|row| if user { row.pt(px(17.)).pb(px(10.)).flex().justify_end() } else { row.py(px(5.)) })
                .when(ix == 0, |row| row.pt(px(56.)))
                .when(ix + 1 == rows.len(), |row| row.pb(px(32.)))
                .child(content),
        )
        .into_any_element()
}

/// A page the agent showed and a subagent still at work stay in view when the turn around them folds.
fn outlives(part: &ChatPart) -> bool {
    match part {
        ChatPart::Tool { page, .. } => page.is_some(),
        ChatPart::Subagent { subagent, .. } => subagent.state == SubagentState::Running,
        _ => false,
    }
}

/// The line a finished turn folds behind: who worked, for how long, with how many tool calls.
fn work(fold: &WorkFold, open: bool, looks: &Looks) -> AnyElement {
    let verb = if fold.thought { "Thought" } else { "Worked" };
    let worked = fold.took.map_or_else(|| verb.to_owned(), |took| format!("{verb} for {}", duration(took)));
    let calls = (fold.calls > 0).then(|| format!("· {} tool {}", fold.calls, if fold.calls == 1 { "call" } else { "calls" }));
    let (opened, id) = (looks.opened.clone(), fold.id.clone());
    let (hover, ink) = (looks.hover, looks.ink);
    let toggle = div()
        .id(SharedString::from(format!("work:{}", fold.id)))
        .h(px(22.))
        .ml(px(-6.))
        .px(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(theme::RADIUS_2)
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(looks.quiet)
        .cursor_pointer()
        .hover(move |toggle| toggle.bg(hover).text_color(ink))
        .child(ui::mark(&looks.provider, 17.))
        .child(worked)
        .children(calls)
        .child(ui::icon(Icon::Chevron, 10.).text_color(looks.quiet).when(open, |chevron| chevron.with_transformation(Transformation::rotate(Radians(std::f32::consts::FRAC_PI_2)))))
        .on_click(move |_, window, _| {
            let mut opened = opened.borrow_mut();
            if !opened.remove(&id) {
                opened.insert(id.clone());
            }
            window.refresh();
        });
    div().mt(px(3.)).flex().flex_col().gap(px(4.)).child(div().flex().child(toggle)).child(div().h(px(1.)).bg(looks.rule)).into_any_element()
}

/// How long something took, as the app says it: milliseconds under a second, then seconds, minutes and hours.
fn duration(ms: u64) -> String {
    let seconds = (ms + 500) / 1000;
    match ms {
        0..1000 => format!("{ms}ms"),
        _ if seconds < 60 => format!("{seconds}s"),
        _ if seconds < 3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, seconds / 60 % 60),
    }
}

fn part(row: &ChatRow, part: &ChatPart, user: bool, pen: &Pen, looks: &Looks, window: &mut Window, cx: &mut App) -> AnyElement {
    let id = SharedString::from(format!("{}:{}", row.message.id, part.id()));
    let line = |icon: Icon, text: String, tint: Hsla| {
        div()
            .flex()
            .items_center()
            .gap(px(7.))
            .min_w_0()
            .text_size(px(12.5))
            .text_color(tint)
            .child(ui::icon(icon, 12.).text_color(tint))
            .child(div().min_w_0().truncate().child(text))
            .into_any_element()
    };
    match part {
        ChatPart::Text { .. } | ChatPart::Thought { .. } => {
            let Some(doc) = row.doc(part.id()) else {
                return div().into_any_element();
            };
            let look = match (part, user) {
                (ChatPart::Thought { .. }, _) => &looks.thought,
                (_, true) => &looks.user,
                _ => &looks.text,
            };
            pen.markdown_with(look, id, &doc.markdown, window, cx)
        }
        ChatPart::Tool { tool, failure, .. } => {
            let tint = if failure.is_some() { Tokens::get(cx).danger } else { looks.quiet };
            line(Icon::Run, tool.title.clone(), tint)
        }
        ChatPart::Subagent { subagent, .. } => line(Icon::Agents, format!("{} · {}", subagent.name, subagent.task), looks.quiet),
        ChatPart::Notice { notice, .. } => line(Icon::Run, notice.name.clone(), looks.faint),
        ChatPart::AgentNotice { notice, .. } => line(Icon::Inbox, notice.title.clone(), looks.faint),
        ChatPart::Account { moved, .. } => line(Icon::LogIn, format!("Moved to {}", moved.label), looks.faint),
        ChatPart::Content { .. } => line(Icon::File, "Attachment".into(), looks.quiet),
    }
}
