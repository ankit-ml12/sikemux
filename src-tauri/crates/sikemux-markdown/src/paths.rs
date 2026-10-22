//! Names of files in prose and between backticks, read the way the Mac app reads them. Each
//! is only a guess: a reader asks whether the file is there before drawing it as one.

use std::ops::Range;

use crate::{Element, Node};

/// Names a project spells out in full, which are files without an extension.
const EXTENSIONLESS: [&str; 9] = [
    "makefile",
    "dockerfile",
    "license",
    "readme",
    "changelog",
    "procfile",
    "justfile",
    "rakefile",
    "gemfile",
];

fn is_leading_noise(c: char) -> bool {
    matches!(c, '(' | '\'' | '"' | '`' | '[' | '{' | '<')
}

fn is_trailing_noise(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '`' | ')' | ']' | '}' | '>'
    )
}

fn has_extension(text: &str) -> bool {
    let Some(dot) = text.rfind('.') else {
        return false;
    };
    let extension = &text[dot + 1..];
    (1..=8).contains(&extension.len()) && extension.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_windows_drive(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

/// Starts with a scheme such as `http:`; a drive letter is one character, so a scheme needs two.
fn has_scheme(text: &str) -> bool {
    let Some(colon) = text.find(':') else {
        return false;
    };
    let scheme = text[..colon].as_bytes();
    scheme.len() >= 2
        && scheme[0].is_ascii_alphabetic()
        && scheme[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
}

fn is_file_uri(text: &str) -> bool {
    text.get(..7)
        .is_some_and(|start| start.eq_ignore_ascii_case("file://"))
}

/// The part of a word that could name a file, without the sentence's punctuation around it.
fn candidate(word: &str, at: usize) -> Option<Range<usize>> {
    let leading = word.len() - word.trim_start_matches(is_leading_noise).len();
    let inner = word[leading..].trim_end_matches(is_trailing_noise);
    if inner.is_empty() {
        return None;
    }
    let separated = inner.contains('/') || inner.contains('\\');
    if !separated
        && !has_extension(inner)
        && !EXTENSIONLESS.contains(&inner.to_lowercase().as_str())
    {
        return None;
    }
    if has_scheme(inner) && !is_file_uri(inner) && !is_windows_drive(inner) {
        return None;
    }
    Some(at + leading..at + leading + inner.len())
}

/// Every stretch of prose that could name a file, in order, by its bytes.
fn candidates(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(at),
            (true, Some(from)) => {
                found.extend(candidate(&text[from..at], from));
                start = None;
            }
            _ => {}
        }
    }
    found
}

/// Whether the text between a pair of backticks names one file as a whole. Backticks mark
/// where a name starts and ends, so a full path between them may hold spaces.
pub fn names_whole_path(text: &str) -> bool {
    if candidates(text)
        .first()
        .is_some_and(|only| *only == (0..text.len()))
    {
        return true;
    }
    let mut chars = text.chars();
    let absolute = match (chars.next(), chars.next()) {
        (Some('/'), Some(next)) => !next.is_whitespace(),
        (Some('~'), Some('/')) => chars.next().is_some_and(|next| !next.is_whitespace()),
        _ => {
            is_windows_drive(text)
                && text[3..]
                    .chars()
                    .next()
                    .is_some_and(|next| !next.is_whitespace())
        }
    };
    !text.contains('\n') && absolute
}

/// A name in prose, as a path holding its own words.
pub fn path(raw: &str, code: bool) -> Node {
    let shown = if code {
        Node::Element(Element::Code { v: raw.to_owned() })
    } else {
        Node::Text(raw.to_owned())
    };
    Node::Element(Element::Path {
        href: raw.to_owned(),
        code,
        c: vec![shown],
    })
}

/// `text` into `out`, each name in it that could be a file marked as a path.
pub fn mark_paths(text: String, out: &mut Vec<Node>) {
    let found = candidates(&text);
    if found.is_empty() {
        return out.push(Node::Text(text));
    }
    let mut cursor = 0;
    for range in found {
        if range.start > cursor {
            out.push(Node::Text(text[cursor..range.start].to_owned()));
        }
        out.push(path(&text[range.clone()], false));
        cursor = range.end;
    }
    if cursor < text.len() {
        out.push(Node::Text(text[cursor..].to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raws(text: &str) -> Vec<&str> {
        candidates(text)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn reads_names_as_the_mac_app_does() {
        assert!(raws(r#"well ... ("") ok"#).is_empty());
        assert_eq!(raws("open <file:///tmp/a.ts>"), ["file:///tmp/a.ts"]);
        assert_eq!(raws("I edited src/a.ts."), ["src/a.ts"]);
        assert_eq!(raws("see (src/a.ts)"), ["src/a.ts"]);
        assert_eq!(raws("src/a.ts:42 fails"), ["src/a.ts:42"]);
        assert_eq!(raws("moved src/a.ts to src/b.ts"), ["src/a.ts", "src/b.ts"]);
        assert_eq!(raws("read it and/or write it, either way"), ["and/or"]);
        assert!(raws("it either works or it does not").is_empty());
        assert!(raws("see https://example.com/a/b for more").is_empty());
    }

    #[test]
    fn takes_whole_names_between_backticks_as_the_mac_app_does() {
        assert!(names_whole_path("src/a.ts"));
        assert!(!names_whole_path("npm run build"));
        assert!(names_whole_path("/work/demo/Shipped Mon 5 + Wed 7.mp4"));
        assert!(names_whole_path("~/Movies/demo cut.mp4"));
        assert!(!names_whole_path("src/demo cut.mp4"));
        assert!(!names_whole_path("/ a"));
    }
}
