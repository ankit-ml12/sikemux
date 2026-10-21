use std::collections::HashSet;
use std::ops::Range;

/// A row of a transcript, known by an id that stays the same for as long as the row lives.
pub trait Row: 'static {
    fn id(&self) -> &str;
}

/// What a change did to the rows' places, for a list drawing them to follow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Nothing,
    /// The row at this index changed in place.
    Changed(usize),
    /// The rows in the range were replaced by this many new ones.
    Spliced(Range<usize>, usize),
    /// These ranges went, last first.
    Removed(Vec<Range<usize>>),
    /// Every row was replaced.
    Reset,
}

/// Rows in order, oldest first. Most changes reach the newest rows, so lookups start from the end.
pub struct Rows<R> {
    rows: Vec<R>,
}

impl<R> Default for Rows<R> {
    fn default() -> Self {
        Rows { rows: Vec::new() }
    }
}

impl<R: Row> Rows<R> {
    pub fn new(rows: Vec<R>) -> Self {
        Rows { rows }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn get(&self, ix: usize) -> Option<&R> {
        self.rows.get(ix)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, R> {
        self.rows.iter()
    }

    pub fn position(&self, id: &str) -> Option<usize> {
        self.rows.iter().rposition(|row| row.id() == id)
    }

    /// Replaces the row with the same id, or adds it at the end.
    pub fn put(&mut self, row: R) -> Edit {
        match self.position(row.id()) {
            Some(ix) => {
                self.rows[ix] = row;
                Edit::Changed(ix)
            }
            None => {
                self.rows.push(row);
                Edit::Spliced(self.rows.len() - 1..self.rows.len() - 1, 1)
            }
        }
    }

    pub fn update(&mut self, id: &str, change: impl FnOnce(&mut R)) -> Edit {
        let Some(ix) = self.position(id) else {
            return Edit::Nothing;
        };
        change(&mut self.rows[ix]);
        Edit::Changed(ix)
    }

    /// The row and every row after it go.
    pub fn drop_from(&mut self, id: &str) -> Edit {
        let Some(ix) = self.position(id) else {
            return Edit::Nothing;
        };
        let removed = ix..self.rows.len();
        self.rows.truncate(ix);
        Edit::Spliced(removed, 0)
    }

    /// Only these rows stay, from the first of them on; rows before it are kept as they are.
    pub fn keep<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) -> Edit {
        let kept: HashSet<&str> = ids.into_iter().collect();
        let Some(first) = self.rows.iter().position(|row| kept.contains(row.id())) else {
            let all = 0..self.rows.len();
            self.rows.clear();
            return Edit::Spliced(all, 0);
        };
        let mut removed: Vec<Range<usize>> = Vec::new();
        for (ix, row) in self.rows.iter().enumerate().skip(first) {
            if kept.contains(row.id()) {
                continue;
            }
            match removed.last_mut() {
                Some(range) if range.end == ix => range.end += 1,
                _ => removed.push(ix..ix + 1),
            }
        }
        let mut ix = 0;
        self.rows.retain(|row| {
            ix += 1;
            ix <= first || kept.contains(row.id())
        });
        removed.reverse();
        if removed.is_empty() {
            Edit::Nothing
        } else {
            Edit::Removed(removed)
        }
    }

    /// Rows from before the first one held go above it; any already held are skipped.
    pub fn prepend(&mut self, rows: Vec<R>) -> Edit {
        let held: HashSet<&str> = self.rows.iter().map(Row::id).collect();
        let older: Vec<R> = rows
            .into_iter()
            .filter(|row| !held.contains(row.id()))
            .collect();
        let count = older.len();
        if count == 0 {
            return Edit::Nothing;
        }
        self.rows.splice(0..0, older);
        Edit::Spliced(0..0, count)
    }

    pub fn reset(&mut self, rows: Vec<R>) -> Edit {
        self.rows = rows;
        Edit::Reset
    }
}
