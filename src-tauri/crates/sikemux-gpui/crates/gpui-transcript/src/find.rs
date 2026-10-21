//! Finding text in a transcript: every place a query is found, in reading order.

use std::ops::Range;

/// What a reader looks for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub text: String,
    pub match_case: bool,
    pub whole_word: bool,
}

/// A place the query was found: the text it is in, by row, doc, block and text, and
/// its bytes there.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub at: (usize, usize, usize, u32),
    pub range: Range<usize>,
}

/// The places `query` is found in `text`, in order. Case is folded for ASCII letters
/// only, so every place is a range of `text` itself.
pub(crate) fn ranges_in(text: &str, query: &Query) -> Vec<Range<usize>> {
    if query.text.is_empty() {
        return Vec::new();
    }
    let (hay, needle) = if query.match_case {
        (text.to_owned(), query.text.clone())
    } else {
        (text.to_ascii_lowercase(), query.text.to_ascii_lowercase())
    };
    let word = |c: char| c.is_alphanumeric() || c == '_';
    hay.match_indices(&needle)
        .map(|(at, found)| at..at + found.len())
        .filter(|range| {
            !query.whole_word
                || (!text[..range.start].chars().next_back().is_some_and(word)
                    && !text[range.end..].chars().next().is_some_and(word))
        })
        .collect()
}

/// The transcript came to find something else, or the reader moved to another place found.
pub struct FoundChanged;
