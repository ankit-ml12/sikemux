use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::{RegexIter, RegexSearch};
use sikemux_wire::grid::{FindOptions, FindResult, GridPoint};

use super::Terminal;

const MAX_MATCHES: usize = 1_000;

impl Terminal {
    fn top(&self) -> u64 {
        self.pushed - self.history_len() as u64
    }

    fn term_point(&self, point: GridPoint) -> Point {
        let bottom = self.pushed + self.term.screen_lines() as u64 - 1;
        let line = point.line.clamp(self.top(), bottom);
        Point::new(
            Line((line as i64 - self.pushed as i64) as i32),
            Column(usize::from(point.col).min(self.term.columns() - 1)),
        )
    }

    fn grid_point(&self, point: Point) -> GridPoint {
        GridPoint {
            line: (self.pushed as i64 + i64::from(point.line.0)) as u64,
            col: point.column.0 as u16,
        }
    }

    /// The cursor's row and column on the screen.
    pub fn cursor(&self) -> (u16, u16) {
        let point = self.term.grid().cursor.point;
        (point.line.0.max(0) as u16, point.column.0 as u16)
    }

    /// The lowest screen row with anything written in it.
    pub fn last_written_row(&self) -> Option<u16> {
        let grid = self.term.grid();
        let cols = self.term.columns();
        (0..self.term.screen_lines()).rev().find_map(|line| {
            let row = &grid[Line(line as i32)];
            (0..cols)
                .any(|column| !matches!(row[Column(column)].c, ' ' | '\t'))
                .then_some(line as u16)
        })
    }

    /// The text between two cells, both included, lines that wrapped joined.
    pub fn text(&self, from: GridPoint, to: GridPoint) -> String {
        let (from, to) = if to < from { (to, from) } else { (from, to) };
        self.term
            .bounds_to_string(self.term_point(from), self.term_point(to))
    }

    /// Everything the terminal still holds, history and screen.
    pub fn all_text(&self) -> String {
        let top = self.term.topmost_line();
        let bottom = Point::new(self.term.bottommost_line(), self.last_column());
        let text = self
            .term
            .bounds_to_string(Point::new(top, Column(0)), bottom);
        text.trim_end().to_owned()
    }

    /// The screen's text the way agent detection reads it: rows that wrapped
    /// joined, blanks after a row's last character and empty rows at the
    /// bottom left out.
    pub fn contents(&self) -> String {
        let grid = self.term.grid();
        let cols = self.term.columns();
        let mut contents = String::new();
        for line in 0..self.term.screen_lines() {
            let row = &grid[Line(line as i32)];
            let mut pending = 0;
            for column in 0..cols {
                let cell = &row[Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                if cell.c == ' ' || cell.c == '\t' {
                    pending += 1;
                    continue;
                }
                contents.extend(std::iter::repeat_n(' ', pending));
                pending = 0;
                contents.push(cell.c);
                contents.extend(cell.zerowidth().into_iter().flatten());
            }
            if !row[Column(cols - 1)].flags.contains(Flags::WRAPLINE) {
                contents.push('\n');
            }
        }
        while contents.ends_with('\n') {
            contents.pop();
        }
        contents
    }

    pub fn find(
        &self,
        query: &str,
        options: &FindOptions,
        from: Option<GridPoint>,
        backwards: bool,
        inclusive: bool,
    ) -> Result<FindResult, String> {
        let mut result = FindResult {
            epoch: self.epoch,
            ..FindResult::default()
        };
        if query.is_empty() {
            return Ok(result);
        }
        let mut pattern = if options.regex {
            query.to_owned()
        } else {
            regex_syntax_escape(query)
        };
        if options.whole_word {
            pattern = format!(r"(?-u:\b)(?:{pattern})(?-u:\b)");
        }
        let case = if options.case_sensitive {
            "(?-i)"
        } else {
            "(?i)"
        };
        pattern = format!("{case}{pattern}");
        let mut regex = RegexSearch::new(&pattern).map_err(|error| error.to_string())?;
        let start = Point::new(self.term.topmost_line(), Column(0));
        let end = Point::new(self.term.bottommost_line(), self.last_column());
        for found in RegexIter::new(start, end, Direction::Right, &self.term, &mut regex) {
            if result.matches.len() == MAX_MATCHES {
                result.more = true;
                break;
            }
            result.matches.push((
                self.grid_point(*found.start()),
                self.grid_point(*found.end()),
            ));
        }
        if result.matches.is_empty() {
            return Ok(result);
        }
        let last = result.matches.len() - 1;
        result.current = Some(match (from, backwards) {
            (None, false) => 0,
            (None, true) => last,
            (Some(from), false) => result
                .matches
                .iter()
                .position(|(start, _)| *start > from || (inclusive && *start == from))
                .unwrap_or(0),
            (Some(from), true) => result
                .matches
                .iter()
                .rposition(|(start, _)| *start < from || (inclusive && *start == from))
                .unwrap_or(last),
        });
        Ok(result)
    }
}

fn regex_syntax_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
