//! Reactions and GIFs: what an agent does in a chat without a row of its own.
//! A chat reads what each call answered and shows only that, an emoji on the
//! message it answers or a GIF under its reply.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::Core;
use crate::cli::protocol::HarnessRequest;

const KLIPY: &str = "https://api.klipy.com/api/v1";
const KLIPY_KEY: Option<&str> = option_env!("KLIPY_API_KEY");
const TIMEOUT: Duration = Duration::from_secs(10);
const RESULTS: &str = "8";
const MAX_TAGS: usize = 6;
/// Joined emoji such as a family or a flag run to several characters; anything
/// longer is words.
const MAX_EMOJI_CHARS: usize = 12;

pub(super) fn react(request: &HarnessRequest) -> Result<Value, String> {
    let emoji = request
        .params
        .get("emoji")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if !is_emoji(emoji) {
        return Err("emoji must be one emoji, such as 👍".into());
    }
    Ok(json!({ "reacted": emoji }))
}

fn is_emoji(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= MAX_EMOJI_CHARS
        && text.chars().any(|char| u32::from(char) >= 0xA9)
        && !text
            .chars()
            .any(|char| char.is_whitespace() || char.is_alphabetic())
}

/// Finds GIFs for a `query`, or sends the one an `id` names.
pub(super) async fn gif(core: &Arc<Core>, request: &HarnessRequest) -> Result<Value, String> {
    if request.params.get("id").is_some() {
        return send(text(&request.params, "id")?).await;
    }
    search(core, text(&request.params, "query")?).await
}

async fn search(core: &Arc<Core>, query: &str) -> Result<Value, String> {
    let answer = klipy(
        "gifs/search",
        &[
            ("q", query),
            ("per_page", RESULTS),
            ("customer_id", &customer(core)),
            ("content_filter", "medium"),
            ("format_filter", "gif"),
        ],
    )
    .await?;
    let gifs: Vec<Value> = items(&answer)
        .filter_map(|item| {
            let tags: Vec<&str> = item["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .take(MAX_TAGS)
                .collect();
            Some(json!({
                "id": item["slug"].as_str()?,
                "title": item["title"].as_str().unwrap_or_default(),
                "tags": tags,
            }))
        })
        .collect();
    if gifs.is_empty() {
        return Err(format!("No GIFs for “{query}”; try other words"));
    }
    Ok(json!({ "gifs": gifs, "next": "Send the one that fits with gif and its id." }))
}

async fn send(id: &str) -> Result<Value, String> {
    let answer = klipy("gifs/items", &[("slugs", id)]).await?;
    let item = items(&answer)
        .next()
        .ok_or_else(|| format!("No GIF {id}; pass an id a gif search gave"))?;
    let file = ["sm", "xs", "md"]
        .iter()
        .find_map(|size| item["file"][size]["gif"].as_object())
        .ok_or("That GIF has no picture to show; pick another")?;
    let url = file
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| url.starts_with("https://"))
        .ok_or("That GIF has no picture to show; pick another")?;
    Ok(json!({
        "gif": {
            "id": id,
            "url": url,
            "width": file.get("width").and_then(Value::as_u64).unwrap_or(0),
            "height": file.get("height").and_then(Value::as_u64).unwrap_or(0),
            "title": item["title"].as_str().unwrap_or_default(),
        },
        "shown": "The person sees the GIF under your reply. Do not mention it.",
    }))
}

fn text<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} must be nonempty text"))
}

async fn klipy(path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    let key = KLIPY_KEY
        .filter(|key| !key.is_empty())
        .ok_or("GIFs are not set up in this build of Sikemux")?;
    let client = crate::http::client().map_err(|error| error.to_string())?;
    let response = client
        .get(format!("{KLIPY}/{key}/{path}"))
        .query(query)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|_| "Couldn't reach the GIF service".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "The GIF service answered {}",
            response.status().as_u16()
        ));
    }
    response
        .json()
        .await
        .map_err(|_| "The GIF service's answer couldn't be read".to_owned())
}

fn items(answer: &Value) -> impl Iterator<Item = &Value> {
    answer["data"]["data"].as_array().into_iter().flatten()
}

/// The same person to the GIF service across searches, without saying who.
fn customer(core: &Core) -> String {
    let place = core
        .harness
        .data_dir
        .as_ref()
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or_default();
    hex::encode(Sha256::digest(format!("klipy:{place}")))[..32].to_owned()
}
