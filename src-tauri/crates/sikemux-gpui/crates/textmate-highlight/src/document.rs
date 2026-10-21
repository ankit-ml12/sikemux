use std::collections::HashMap;

use irosashi::{ScopeListId, Session, StateStack, TokenizeOptions};

use crate::{Style, Styles};

/// A styled run of a line, in bytes from the line's start. Runs in the text colour
/// are left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
    pub style: Style,
}

/// Lines still to tokenize: from `next`, and at least through `through`.
#[derive(Clone, Copy)]
struct Pending {
    next: usize,
    through: usize,
}

/// A text's lines with the tokenizer state each one starts in, so an edit retokenizes
/// from the edited line until a line ends in the state it ended in before.
pub struct Document {
    session: Session,
    options: TokenizeOptions,
    states: Vec<StateStack>,
    spans: Vec<Vec<Span>>,
    pending: Option<Pending>,
    styles: HashMap<ScopeListId, Style>,
}

impl Document {
    /// Lines longer than `max_line_length` bytes stay in the text colour and keep the
    /// state they start in.
    pub fn new(session: Session, max_line_length: Option<usize>) -> Self {
        let initial = session.initial_state();
        Self {
            session,
            options: TokenizeOptions { max_line_length },
            states: vec![initial],
            spans: Vec::new(),
            pending: None,
            styles: HashMap::new(),
        }
    }

    pub fn line_count(&self) -> usize {
        self.spans.len()
    }

    /// Replaces every line, as when the whole text changes.
    pub fn reset(&mut self, lines: usize) {
        let initial = self.session.initial_state();
        self.states = vec![initial; lines + 1];
        self.spans = vec![Vec::new(); lines];
        self.pending = (lines > 0).then_some(Pending {
            next: 0,
            through: lines - 1,
        });
    }

    /// Lines `start..=old_end` became lines `start..=new_end`.
    pub fn edit(&mut self, start: usize, old_end: usize, new_end: usize) {
        let old_end = old_end.min(self.spans.len().saturating_sub(1)).max(start);
        if start >= self.spans.len() {
            let added = new_end + 1 - self.spans.len();
            let last = self.states[self.spans.len()].clone();
            self.spans.extend(std::iter::repeat_n(Vec::new(), added));
            self.states.extend(std::iter::repeat_n(last, added));
        } else {
            let placeholder = self.states[start].clone();
            self.spans.splice(
                start..=old_end,
                std::iter::repeat_n(Vec::new(), new_end - start + 1),
            );
            self.states.splice(
                start + 1..=old_end,
                std::iter::repeat_n(placeholder, new_end - start),
            );
        }
        let shift = |line: usize| {
            if line <= start {
                line
            } else if line > old_end {
                line + new_end - old_end
            } else {
                new_end
            }
        };
        let pending = match self.pending {
            Some(pending) => Pending {
                next: shift(pending.next).min(start),
                through: shift(pending.through).max(new_end),
            },
            None => Pending {
                next: start,
                through: new_end,
            },
        };
        self.pending = Some(Pending {
            next: pending.next,
            through: pending.through.min(self.spans.len() - 1),
        });
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Tokenizes at most `budget` pending lines, reading each through `line`, and says
    /// whether any are still pending.
    pub fn tokenize<L: AsRef<str>>(
        &mut self,
        styles: &Styles,
        mut line: impl FnMut(usize) -> L,
        budget: usize,
    ) -> bool {
        let Some(mut pending) = self.pending else {
            return false;
        };
        for _ in 0..budget {
            let index = pending.next;
            let text = line(index);
            let text = text.as_ref();
            let result =
                self.session
                    .tokenize_line(text, &self.states[index], index == 0, self.options);
            let mut spans: Vec<Span> = Vec::new();
            for token in &result.tokens {
                let style = *self.styles.entry(token.scopes).or_insert_with(|| {
                    styles.style(self.session.scope_names(token.scopes).into_iter())
                });
                if style == Style::default() || token.start == token.end {
                    continue;
                }
                match spans.last_mut() {
                    Some(last) if last.end == token.start as u32 && last.style == style => {
                        last.end = token.end as u32
                    }
                    _ => spans.push(Span {
                        start: token.start as u32,
                        end: token.end as u32,
                        style,
                    }),
                }
            }
            self.spans[index] = spans;
            let settled = index >= pending.through && self.states[index + 1] == result.state;
            self.states[index + 1] = result.state;
            pending.next += 1;
            if settled || pending.next == self.spans.len() {
                self.pending = None;
                return false;
            }
        }
        self.pending = Some(pending);
        true
    }

    /// The styled runs of `line`, empty for a line not tokenized yet.
    pub fn spans(&self, line: usize) -> &[Span] {
        self.spans.get(line).map_or(&[], Vec::as_slice)
    }
}
