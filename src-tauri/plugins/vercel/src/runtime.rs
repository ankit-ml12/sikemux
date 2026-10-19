// A deployment's runtime logs: what its functions, middleware and edge wrote while
// answering requests. Vercel sends them as they happen, one JSON object per line,
// and keeps none from before the stream opened.

use std::path::Path;
use std::time::Duration;

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sikemux_plugin_api::{reply, PluginResult, StreamSink};

use crate::client::{at, text, Session};
use crate::error::{VercelError, VercelResult};
use crate::projects::segment;

/// A message longer than this is cut, so one runaway line can't fill the pane.
const MAX_MESSAGE_CHARS: usize = 4000;
const DEFAULT_LISTEN_SECS: u64 = 20;
const MAX_LISTEN_SECS: u64 = 60;
const MAX_LINES: usize = 500;

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub at: i64,
    /// info, warning or error.
    pub level: String,
    /// serverless, edge-function, edge-middleware, static, external or redirect.
    pub source: String,
    pub message: String,
    pub method: Option<String>,
    pub path: Option<String>,
    pub status: Option<u16>,
    pub domain: Option<String>,
}

fn some_text(value: &Value, path: &str) -> Option<String> {
    Some(text(value, path)).filter(|text| !text.is_empty())
}

pub fn line_of(value: &Value) -> LogLine {
    let mut message = text(value, "/message");
    if message.chars().count() > MAX_MESSAGE_CHARS {
        message = message.chars().take(MAX_MESSAGE_CHARS).collect::<String>() + " …";
    }
    LogLine {
        at: at(value, "/timestampInMs").as_i64().unwrap_or(0),
        level: some_text(value, "/level").unwrap_or_else(|| "info".into()),
        source: text(value, "/source"),
        message,
        method: some_text(value, "/requestMethod"),
        path: some_text(value, "/requestPath"),
        status: at(value, "/responseStatusCode")
            .as_u64()
            .and_then(|status| u16::try_from(status).ok()),
        domain: some_text(value, "/domain"),
    }
}

/// Turns the stream's bytes, split anywhere, into log lines; anything that isn't one is skipped.
#[derive(Default)]
pub struct Lines {
    pending: Vec<u8>,
}

impl Lines {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<LogLine> {
        self.pending.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            if let Ok(value) = serde_json::from_slice::<Value>(&line) {
                if value.is_object() {
                    lines.push(line_of(&value));
                }
            }
        }
        lines
    }
}

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Filter {
    /// Only these levels: info, warning, error.
    pub levels: Vec<String>,
    /// Only requests answered with this status or above, such as 500.
    pub min_status: Option<u16>,
    /// Only requests whose path contains this.
    pub path: Option<String>,
    /// Only lines containing these words, ignoring case.
    pub query: Option<String>,
}

impl Filter {
    pub fn keeps(&self, line: &LogLine) -> bool {
        if !self.levels.is_empty()
            && !self
                .levels
                .iter()
                .any(|level| level.eq_ignore_ascii_case(&line.level))
        {
            return false;
        }
        if let Some(min) = self.min_status {
            if line.status.is_none_or(|status| status < min) {
                return false;
            }
        }
        if let Some(path) = self.path.as_deref().filter(|path| !path.is_empty()) {
            if !line
                .path
                .as_deref()
                .is_some_and(|line_path| line_path.contains(path))
            {
                return false;
            }
        }
        if let Some(words) = self
            .query
            .as_deref()
            .map(str::to_lowercase)
            .filter(|words| !words.trim().is_empty())
        {
            if !line.message.to_lowercase().contains(&words) {
                return false;
            }
        }
        true
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// The project's id or name.
    pub project: String,
    /// The deployment's id.
    pub id: String,
    #[serde(default)]
    pub team: Option<String>,
}

async fn open(data_dir: &Path, target: &Target) -> VercelResult<reqwest::Response> {
    let project = segment("project", &target.project)?;
    let id = segment("deployment", &target.id)?;
    let session = Session::of(data_dir, target.team.as_deref()).await?;
    session
        .open(
            &format!("/v1/projects/{project}/deployments/{id}/runtime-logs"),
            &[],
        )
        .await
}

#[derive(Serialize)]
struct Batch<'a> {
    lines: &'a [LogLine],
}

/// Sends each batch as Vercel writes it, until the pane stops listening or Vercel ends the stream.
pub async fn follow(data_dir: &Path, input: Value, sink: &StreamSink) -> PluginResult<()> {
    let target: Target =
        serde_json::from_value(input).map_err(|error| VercelError::BadArg(error.to_string()))?;
    let response = open(data_dir, &target).await?;
    let mut lines = Lines::default();
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(VercelError::from)?;
        let batch = lines.feed(&chunk);
        if !batch.is_empty() && sink.send(reply(Batch { lines: &batch })?).is_err() {
            return Ok(());
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listen {
    #[serde(flatten)]
    pub target: Target,
    #[serde(flatten)]
    pub filter: Filter,
    /// How long to listen. Default 20, at most 60.
    #[serde(default)]
    pub seconds: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Heard {
    pub lines: Vec<LogLine>,
    pub seconds: u64,
    /// Lines seen that the filter left out.
    pub filtered: usize,
    pub note: Option<String>,
}

/// What the deployment logs while listening, for an agent waiting on the person to try something.
pub async fn listen(data_dir: &Path, input: Listen) -> VercelResult<Heard> {
    let seconds = input
        .seconds
        .unwrap_or(DEFAULT_LISTEN_SECS)
        .clamp(1, MAX_LISTEN_SECS);
    let response = open(data_dir, &input.target).await?;
    let mut lines = Lines::default();
    let mut body = response.bytes_stream();
    let mut heard = Vec::new();
    let mut filtered = 0;
    let deadline = tokio::time::sleep(Duration::from_secs(seconds));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            () = &mut deadline => break,
            chunk = body.next() => {
                let Some(chunk) = chunk else { break };
                for line in lines.feed(&chunk?) {
                    if input.filter.keeps(&line) { heard.push(line) } else { filtered += 1 }
                }
                if heard.len() >= MAX_LINES { break }
            }
        }
    }
    let note = heard
        .is_empty()
        .then(|| format!("Nothing was logged in {seconds}s. Runtime logs come only as requests arrive, so ask the person to try it again while you listen."));
    Ok(Heard {
        lines: heard,
        seconds,
        filtered,
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ERROR: &str = r#"{"level":"error","message":"TypeError: cart is undefined\n    at handler (/var/task/api/checkout.js:12:9)","rowId":"r1","source":"serverless","timestampInMs":1792300000000,"domain":"shop.acme.dev","messageTruncated":false,"requestMethod":"POST","requestPath":"/api/checkout","responseStatusCode":500}"#;
    const INFO: &str = r#"{"level":"info","message":"GET /","source":"static","timestampInMs":1792300000100,"requestMethod":"GET","requestPath":"/","responseStatusCode":200}"#;

    #[test]
    fn lines_split_anywhere_come_out_whole_and_noise_is_skipped() {
        let stream = format!("{ERROR}\n\n not json\n{INFO}\n{{\"level\":\"info\"");
        let mut lines = Lines::default();
        let mut read = Vec::new();
        for chunk in stream.as_bytes().chunks(7) {
            read.extend(lines.feed(chunk));
        }
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].level, "error");
        assert_eq!(read[0].path.as_deref(), Some("/api/checkout"));
        assert_eq!(read[0].status, Some(500));
        assert!(read[0].message.contains("checkout.js:12:9"));
        assert_eq!(read[1].source, "static");
    }

    #[test]
    fn a_filter_keeps_the_level_status_path_and_words_asked_for() {
        let error = line_of(&serde_json::from_str(ERROR).unwrap());
        let info = line_of(&serde_json::from_str(INFO).unwrap());
        let errors = Filter {
            levels: vec!["ERROR".into()],
            ..Filter::default()
        };
        assert!(errors.keeps(&error) && !errors.keeps(&info));
        let failing = Filter {
            min_status: Some(500),
            ..Filter::default()
        };
        assert!(failing.keeps(&error) && !failing.keeps(&info));
        let checkout = Filter {
            path: Some("/api/checkout".into()),
            ..Filter::default()
        };
        assert!(checkout.keeps(&error) && !checkout.keeps(&info));
        let words = Filter {
            query: Some("typeerror".into()),
            ..Filter::default()
        };
        assert!(words.keeps(&error) && !words.keeps(&info));
        assert!(Filter::default().keeps(&info));
    }

    #[test]
    fn a_runaway_message_is_cut() {
        let long = json!({ "level": "info", "message": "x".repeat(MAX_MESSAGE_CHARS + 5) });
        let line = line_of(&long);
        assert!(line.message.ends_with(" …"));
        assert_eq!(line.level, "info");
        assert_eq!(line.status, None);
    }
}
