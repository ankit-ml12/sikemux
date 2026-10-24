//! What an agent writes, read as markdown once here and sent with the text, so
//! no client parses it again. Each chunk of an answer carries the change to
//! the tree of the answer so far, as `_meta.sikemux.md`: `n`, the answer's
//! length in UTF-16 units once the chunk is added, and `o`, the patch.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Value};
use sikemux_markdown::{parse, Doc, Options};

use crate::protocol::{ChatEvent, ChatEventKind};

const ANSWER: Options = Options {
    gfm: true,
    html_as_text: false,
    file_links: true,
    html_images: false,
};

/// Updates that leave the answer being written open: none of them puts
/// anything in the transcript between its pieces.
const LEAVES_OPEN: [&str; 10] = [
    "usage_update",
    "current_mode_update",
    "config_option_update",
    "available_commands_update",
    "session_info_update",
    "plan",
    "task_notice",
    "async_task_spawned",
    "async_task_progress",
    "async_task_state_update",
];

/// The most answer text, counted from the newest, that one batch of replayed
/// events carries trees for. Older answers go without one, and their clients
/// read them themselves.
const MAX_TREE_TEXT: usize = 2 * 1024 * 1024;

static LETTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)<(agent-message|question-answer) [^>]*>\n(.*?)\n</(?:agent-message|question-answer)>",
    )
    .expect("letter pattern")
});

/// The tree of each message from another agent a prompt is made of, in order,
/// for a client that shows them apart from the prompt around them.
pub(crate) fn letter_trees(text: &str) -> Option<Value> {
    let start = text.trim_start();
    if !start.starts_with("<agent-message ") && !start.starts_with("<question-answer ") {
        return None;
    }
    let trees: Vec<Value> = LETTER
        .captures_iter(text)
        .map(|letter| {
            let tag = &letter[1];
            let body = letter[2].replace(&format!("&lt;/{tag}"), &format!("</{tag}"));
            json!(parse(&body, ANSWER, 0))
        })
        .collect();
    (!trees.is_empty()).then_some(Value::Array(trees))
}

fn asked_text(notification: &Value) -> Option<&str> {
    let update = notification.get("update")?;
    if update.get("sessionUpdate")?.as_str()? != "user_message_chunk" {
        return None;
    }
    update.pointer("/content/text")?.as_str()
}

/// An answer's place in a session: which kind of text, and which message.
#[derive(PartialEq, Eq)]
struct Key {
    kind: String,
    message: Option<String>,
}

/// A piece of an answer: the session it is written in, which answer, and the text.
fn piece(notification: &Value) -> Option<(&str, Key, &str)> {
    let session = notification.get("sessionId")?.as_str()?;
    let update = notification.get("update")?;
    let kind = update.get("sessionUpdate")?.as_str()?;
    if kind != "agent_message_chunk" && kind != "agent_thought_chunk" {
        return None;
    }
    let content = update.get("content")?;
    if content.get("type")?.as_str()? != "text" {
        return None;
    }
    let key = Key {
        kind: kind.to_owned(),
        message: update
            .get("messageId")
            .and_then(Value::as_str)
            .map(str::to_owned),
    };
    Some((session, key, content.get("text")?.as_str()?))
}

fn leaves_open(notification: &Value) -> bool {
    notification
        .pointer("/update/sessionUpdate")
        .and_then(Value::as_str)
        .is_some_and(|kind| LEAVES_OPEN.contains(&kind))
}

fn mark(notification: &mut Value, key: &str, tree: Value) {
    let Some(update) = notification
        .get_mut("update")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    let meta = update.entry("_meta").or_insert_with(|| json!({}));
    if let Some(meta) = meta.as_object_mut() {
        let ours = meta.entry("sikemux").or_insert_with(|| json!({}));
        if let Some(ours) = ours.as_object_mut() {
            ours.insert(key.to_owned(), tree);
        }
    }
}

struct Answer {
    key: Key,
    doc: Doc,
}

/// The answers being written, one per session, as their clients were last told.
#[derive(Default)]
pub(crate) struct Answers {
    open: HashMap<String, Answer>,
}

impl Answers {
    /// Reads each notification in order, giving the last piece of each answer
    /// in the run what changed in its tree.
    pub(crate) fn annotate(&mut self, notifications: &mut [Value]) {
        let mut last: HashMap<String, usize> = HashMap::new();
        for at in 0..notifications.len() {
            let Some(notification) = notifications.get(at) else {
                break;
            };
            let Some((session, key, text)) = piece(notification) else {
                if let Some(trees) = asked_text(notification).and_then(letter_trees) {
                    if let Some(notification) = notifications.get_mut(at) {
                        mark(notification, "letters", trees);
                    }
                }
                let Some(notification) = notifications.get(at) else {
                    break;
                };
                if !leaves_open(notification) {
                    if let Some(session) = notification.get("sessionId").and_then(Value::as_str) {
                        let session = session.to_owned();
                        self.tell(&session, &mut last, notifications);
                        self.open.remove(&session);
                    }
                }
                continue;
            };
            let (session, text) = (session.to_owned(), text.to_owned());
            if self
                .open
                .get(&session)
                .is_some_and(|answer| answer.key != key)
            {
                self.tell(&session, &mut last, notifications);
                self.open.remove(&session);
            }
            self.open
                .entry(session.clone())
                .or_insert_with(|| Answer {
                    key,
                    doc: Doc::new(ANSWER),
                })
                .doc
                .push(&text);
            last.insert(session, at);
        }
        let sessions: Vec<String> = last.keys().cloned().collect();
        for session in sessions {
            self.tell(&session, &mut last, notifications);
        }
    }

    /// Puts what changed in `session`'s answer on its last piece in the run.
    fn tell(
        &mut self,
        session: &str,
        last: &mut HashMap<String, usize>,
        notifications: &mut [Value],
    ) {
        let (Some(at), Some(answer)) = (last.remove(session), self.open.get_mut(session)) else {
            return;
        };
        let units = answer.doc.units();
        let patch = answer.doc.patch();
        if let (false, Some(notification)) = (patch.is_empty(), notifications.get_mut(at)) {
            mark(notification, "md", json!({ "n": units, "o": patch }));
        }
    }

    /// Something else was said in the chat, which ends every answer.
    pub(crate) fn close_all(&mut self) {
        self.open.clear();
    }

    /// The answers of a chat whose clients already hold `events`, for a core
    /// that takes the chat up partway through an answer.
    pub(crate) fn read_back(mut events: Vec<ChatEvent>) -> Self {
        let mut answers = Self::default();
        for event in &mut events {
            if event.kind != ChatEventKind::SessionUpdate {
                answers.close_all();
                continue;
            }
            for notification in notifications(event) {
                answers.annotate(std::slice::from_mut(notification));
            }
        }
        answers
    }
}

fn notifications(event: &mut ChatEvent) -> Vec<&mut Value> {
    if event.kind != ChatEventKind::SessionUpdate {
        return Vec::new();
    }
    if !event.payload.get("updates").is_some_and(Value::is_array) {
        return vec![&mut event.payload];
    }
    event
        .payload
        .get_mut("updates")
        .and_then(Value::as_array_mut)
        .map(|updates| updates.iter_mut().collect())
        .unwrap_or_default()
}

/// Where the newest answers in `events` begin whose text fits [`MAX_TREE_TEXT`].
fn newest_answers(events: &[ChatEvent]) -> usize {
    let mut text = 0;
    for (at, event) in events.iter().enumerate().rev() {
        if event.kind != ChatEventKind::SessionUpdate {
            continue;
        }
        let updates = event
            .payload
            .get("updates")
            .and_then(Value::as_array)
            .map_or(std::slice::from_ref(&event.payload), Vec::as_slice);
        text += updates
            .iter()
            .filter_map(piece)
            .map(|(_, _, piece)| piece.len())
            .sum::<usize>();
        if text > MAX_TREE_TEXT {
            return at + 1;
        }
    }
    0
}

/// Gives the newest answers in `events`, a slice of the chat sent at once,
/// their whole trees, within a bound on how much text that reads.
pub(crate) fn annotate_replay(events: &mut [ChatEvent]) {
    let from = newest_answers(events);
    let mut answers = Answers::default();
    for event in events.iter_mut().skip(from) {
        if event.kind != ChatEventKind::SessionUpdate {
            answers.close_all();
            continue;
        }
        for notification in notifications(event) {
            answers.annotate(std::slice::from_mut(notification));
        }
    }
}
