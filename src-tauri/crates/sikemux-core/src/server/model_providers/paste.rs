//! What the person pastes to add a provider: a key, a base URL, or the
//! quickstart a provider's dashboard shows, in Python, TypeScript or curl.

use url::Url;

/// The paths an OpenAI-compatible API serves under its base URL.
const ENDPOINTS: [&str; 6] = [
    "/chat/completions",
    "/completions",
    "/responses",
    "/messages",
    "/models",
    "/embeddings",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Pasted {
    pub base_url: Option<String>,
    pub key: Option<String>,
    pub model: Option<String>,
}

pub(crate) fn read(text: &str) -> Pasted {
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || "\"'`(),;{}[]=<>\\".contains(c))
        .map(|word| word.trim_matches(':'))
        .filter(|word| !word.is_empty())
        .collect();
    let after = |names: &dyn Fn(&str) -> bool, fits: &dyn Fn(&str) -> bool| {
        words
            .windows(2)
            .find(|pair| names(pair[0]) && fits(pair[1]))
            .map(|pair| pair[1].to_owned())
    };
    let urls = || words.iter().copied().filter(|word| is_url(word));
    let base_url = after(&|name| plain(name).contains("base"), &is_url)
        .or_else(|| {
            urls()
                .find(|url| endpoint_of(url).is_some())
                .map(str::to_owned)
        })
        .or_else(|| urls().next().map(str::to_owned))
        .and_then(|url| base_of(&url));
    let key = after(&|name| name.eq_ignore_ascii_case("bearer"), &is_key)
        .or_else(|| after(&names_key, &is_key))
        .or_else(|| {
            let rest: Vec<_> = words.iter().filter(|word| !is_url(word)).collect();
            match rest.as_slice() {
                [word] if is_key(word) => Some((**word).to_owned()),
                _ => None,
            }
        });
    let model = after(&|name| plain(name) == "model", &is_model);
    Pasted {
        base_url,
        key,
        model,
    }
}

/// `name` in lowercase letters and digits alone, so `api_key`, `apiKey` and `--api-key` match.
fn plain(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn names_key(name: &str) -> bool {
    let name = plain(name);
    name.ends_with("apikey") || name == "key" || name.ends_with("token")
}

/// A model's name rather than a word of prose: names hold a digit, dash, slash or dot.
fn is_model(word: &str) -> bool {
    word.len() <= 200
        && !is_url(word)
        && !word.starts_with('$')
        && word
            .chars()
            .any(|c| c.is_ascii_digit() || "-/.:_".contains(c))
}

fn is_url(word: &str) -> bool {
    word.starts_with("https://") || word.starts_with("http://")
}

/// A secret rather than code naming one, such as `os.environ` or `$LITHOSAI_API_KEY`.
fn is_key(word: &str) -> bool {
    let shouting = word
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
    (16..=512).contains(&word.len())
        && word.chars().all(|c| c.is_ascii_graphic())
        && !shouting
        && !is_url(word)
        && !word.starts_with('$')
        && !word.contains('/')
        && !word.contains("env.")
        && !word.contains("environ")
}

fn endpoint_of(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next()?.trim_end_matches('/');
    ENDPOINTS
        .into_iter()
        .find(|endpoint| path.ends_with(endpoint))
}

/// The API's base URL: `url` without an endpoint, query or trailing slash.
fn base_of(url: &str) -> Option<String> {
    let mut url = Url::parse(url).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    let path = url.path().trim_end_matches('/').to_owned();
    let path = match endpoint_of(&path) {
        Some(endpoint) => path.strip_suffix(endpoint).unwrap_or(&path).to_owned(),
        None => path,
    };
    url.set_path(&path);
    Some(url.as_str().trim_end_matches('/').to_owned())
}

/// A name for the provider at `base_url`, from its domain: `api.lithosai.cloud` is LithosAI.
pub(crate) fn label(base_url: &str) -> String {
    let Some(url) = Url::parse(base_url).ok() else {
        return base_url.to_owned();
    };
    let host = url.host_str().unwrap_or_default();
    if matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
        return match url.port() {
            Some(port) => format!("Local {port}"),
            None => "Local".to_owned(),
        };
    }
    let parts: Vec<_> = host.split('.').collect();
    let name = match parts.as_slice() {
        [.., name, _] | [name] => name,
        [] => host,
    };
    let mut label: String = name
        .chars()
        .enumerate()
        .map(|(at, c)| if at == 0 { c.to_ascii_uppercase() } else { c })
        .collect();
    if label.len() > 4 && label.ends_with("ai") {
        label.replace_range(label.len() - 2.., "AI");
    }
    label
}

/// A provider id made from `label`, such as `lithosai`.
pub(crate) fn id(label: &str) -> String {
    let mut id = String::new();
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    let id = id.trim_end_matches('-');
    match id {
        "" => "provider".to_owned(),
        id => id.chars().take(40).collect(),
    }
}
