//! The messages Claude Code writes on its stream-json output, read once as
//! each line arrives, so a field it left out is reported there rather than
//! read as empty later.

use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Message {
    ControlResponse {
        response: ControlResponse,
    },
    ControlRequest {
        request_id: String,
        #[serde(default)]
        request: Value,
    },
    ControlCancelRequest {
        request_id: String,
    },
    StreamEvent {
        parent_tool_use_id: Option<String>,
        event: StreamEvent,
    },
    Assistant(Value),
    User(Value),
    Result(TurnResult),
    System(Value),
    CommandLifecycle {
        state: Option<String>,
        command_uuid: Option<String>,
    },
    ConversationReset,
    #[serde(other)]
    Other,
}

/// Claude Code's answer to a request this side made.
#[derive(Deserialize)]
pub(crate) struct ControlResponse {
    pub request_id: String,
    pub subtype: Option<String>,
    pub error: Option<String>,
    pub response: Option<Value>,
}

/// A piece of the model's answer as it streams in.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum StreamEvent {
    MessageStart {
        message: StartedMessage,
    },
    ContentBlockStart {
        content_block: Value,
    },
    ContentBlockDelta {
        #[serde(default)]
        index: u64,
        delta: Delta,
    },
    MessageDelta {
        usage: Option<OutputUsage>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(crate) struct StartedMessage {
    pub id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<Value>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Delta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(crate) struct OutputUsage {
    pub output_tokens: Option<u64>,
}

/// How a turn ended.
#[derive(Deserialize)]
pub(crate) struct TurnResult {
    pub subtype: Option<String>,
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    /// What the turn answered, or what went wrong.
    pub result: Option<String>,
    #[serde(default)]
    pub errors: Vec<Value>,
    /// The message the turn answers.
    pub user_message_uuid: Option<String>,
    /// Present on a turn background work started rather than a message.
    pub origin: Option<Value>,
    pub total_cost_usd: Option<f64>,
    /// Each model the turn used, by name.
    #[serde(rename = "modelUsage")]
    pub model_usage: Option<Map<String, Value>>,
}
