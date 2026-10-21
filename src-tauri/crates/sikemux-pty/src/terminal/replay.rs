use std::fmt::Write;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, CursorShape};

use super::Terminal;

fn color(out: &mut String, color: Color, base: u8) {
    match color {
        Color::Named(named) => {
            let index = named as usize;
            match index {
                0..8 => {
                    let _ = write!(out, ";{}", base as usize + index);
                }
                8..16 => {
                    let _ = write!(out, ";{}", base as usize + 60 + index - 8);
                }
                259..267 => {
                    let _ = write!(out, ";{}", base as usize + index - 259);
                }
                _ => {}
            }
        }
        Color::Indexed(index) => {
            let _ = write!(out, ";{};5;{index}", base + 8);
        }
        Color::Spec(rgb) => {
            let _ = write!(out, ";{};2;{};{};{}", base + 8, rgb.r, rgb.g, rgb.b);
        }
    }
}

fn sgr(out: &mut String, cell: &Cell) {
    out.push_str("\x1b[0");
    for (flag, code) in [
        (Flags::BOLD, "1"),
        (Flags::DIM, "2"),
        (Flags::ITALIC, "3"),
        (Flags::UNDERLINE, "4"),
        (Flags::DOUBLE_UNDERLINE, "4:2"),
        (Flags::UNDERCURL, "4:3"),
        (Flags::DOTTED_UNDERLINE, "4:4"),
        (Flags::DASHED_UNDERLINE, "4:5"),
        (Flags::INVERSE, "7"),
        (Flags::HIDDEN, "8"),
        (Flags::STRIKEOUT, "9"),
    ] {
        if cell.flags.contains(flag) {
            out.push(';');
            out.push_str(code);
        }
    }
    color(out, cell.fg, 30);
    color(out, cell.bg, 40);
    match cell.underline_color() {
        Some(Color::Indexed(index)) => {
            let _ = write!(out, ";58;5;{index}");
        }
        Some(Color::Spec(rgb)) => {
            let _ = write!(out, ";58;2;{};{};{}", rgb.r, rgb.g, rgb.b);
        }
        _ => {}
    }
    out.push('m');
}

fn same_look(a: &Cell, b: &Cell) -> bool {
    const LOOK: Flags = Flags::BOLD
        .union(Flags::DIM)
        .union(Flags::ITALIC)
        .union(Flags::ALL_UNDERLINES)
        .union(Flags::INVERSE)
        .union(Flags::HIDDEN)
        .union(Flags::STRIKEOUT);
    a.fg == b.fg
        && a.bg == b.bg
        && (a.flags & LOOK) == (b.flags & LOOK)
        && a.underline_color() == b.underline_color()
}

impl Terminal {
    /// Bytes that rebuild this screen, its history and modes in a terminal
    /// of the same size. On the alternate screen only that screen is kept.
    pub fn replay(&self) -> Vec<u8> {
        let mut out = String::new();
        let mode = *self.term.mode();
        if mode.contains(TermMode::ALT_SCREEN) {
            out.push_str("\x1b[?1049h\x1b[H\x1b[2J");
        }
        let grid = self.term.grid();
        let cols = self.term.columns();
        let top = self.term.topmost_line().0;
        let bottom = self.term.bottommost_line().0;
        let plain = Cell::default();
        let mut look = plain.clone();
        let mut link: Option<String> = None;
        for line in top..=bottom {
            let row = &grid[Line(line)];
            let wrapped = row[Column(cols - 1)].flags.contains(Flags::WRAPLINE);
            let end = if wrapped {
                cols
            } else {
                (0..cols)
                    .rev()
                    .find(|&column| {
                        let cell = &row[Column(column)];
                        cell.c != ' ' || !same_look(cell, &plain) || cell.hyperlink().is_some()
                    })
                    .map_or(0, |column| column + 1)
            };
            for column in 0..end {
                let cell = &row[Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                if !same_look(cell, &look) {
                    sgr(&mut out, cell);
                    look = cell.clone();
                }
                let uri = cell.hyperlink().map(|link| link.uri().to_owned());
                if uri != link {
                    let _ = write!(out, "\x1b]8;;{}\x1b\\", uri.as_deref().unwrap_or(""));
                    link = uri;
                }
                out.push(if cell.c == '\t' { ' ' } else { cell.c });
                out.extend(cell.zerowidth().into_iter().flatten());
            }
            if !wrapped && line != bottom {
                if link.take().is_some() {
                    out.push_str("\x1b]8;;\x1b\\");
                }
                if !same_look(&look, &plain) {
                    out.push_str("\x1b[0m");
                    look = plain.clone();
                }
                out.push_str("\r\n");
            }
        }
        if link.is_some() {
            out.push_str("\x1b]8;;\x1b\\");
        }
        out.push_str("\x1b[0m");
        let cursor = grid.cursor.point;
        let _ = write!(out, "\x1b[{};{}H", cursor.line.0 + 1, cursor.column.0 + 1);
        for (flag, set) in [
            (TermMode::APP_CURSOR, "\x1b[?1h"),
            (TermMode::APP_KEYPAD, "\x1b="),
            (TermMode::BRACKETED_PASTE, "\x1b[?2004h"),
            (TermMode::MOUSE_REPORT_CLICK, "\x1b[?1000h"),
            (TermMode::MOUSE_DRAG, "\x1b[?1002h"),
            (TermMode::MOUSE_MOTION, "\x1b[?1003h"),
            (TermMode::SGR_MOUSE, "\x1b[?1006h"),
            (TermMode::UTF8_MOUSE, "\x1b[?1005h"),
            (TermMode::FOCUS_IN_OUT, "\x1b[?1004h"),
        ] {
            if mode.contains(flag) {
                out.push_str(set);
            }
        }
        for (flag, unset) in [
            (TermMode::SHOW_CURSOR, "\x1b[?25l"),
            (TermMode::LINE_WRAP, "\x1b[?7l"),
            (TermMode::ALTERNATE_SCROLL, "\x1b[?1007l"),
        ] {
            if !mode.contains(flag) {
                out.push_str(unset);
            }
        }
        let style = self.term.cursor_style();
        let shape = match style.shape {
            CursorShape::Block | CursorShape::HollowBlock | CursorShape::Hidden => 1,
            CursorShape::Underline => 3,
            CursorShape::Beam => 5,
        } + u8::from(!style.blinking);
        let _ = write!(out, "\x1b[{shape} q");
        out.into_bytes()
    }
}
