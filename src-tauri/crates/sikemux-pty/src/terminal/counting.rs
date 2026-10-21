//! Hands the parsed output to the emulator while counting the lines that
//! scroll into its history. Clients number history lines by that count, so
//! it has to be exact, and alacritty keeps none: once its history is full,
//! a line pushed in drops one out and its length stays the same.
//!
//! The emulator is allowed a screen's worth more history than the terminal
//! keeps, more than one call can push, and is cut back after each call, so
//! the length always grows by exactly the lines a call pushed.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{
    Attr, CharsetIndex, ClearMode, CursorShape, CursorStyle, Handler, Hyperlink, KeyboardModes,
    KeyboardModesApplyBehavior, LineClearMode, Mode, ModifyOtherKeys, PrivateMode, Rgb,
    ScpCharPath, ScpUpdateMode, StandardCharset, TabulationClearMode,
};

use super::Events;

#[derive(Default)]
pub(super) struct Outcome {
    pub pushed: u64,
    /// The history was cleared or replaced, so earlier line numbers are void.
    pub cleared: bool,
    pub palette: bool,
}

pub(super) struct Counting<'a> {
    term: &'a mut Term<Events>,
    limit: usize,
    outcome: Outcome,
}

impl<'a> Counting<'a> {
    pub(super) fn new(term: &'a mut Term<Events>, limit: usize) -> Self {
        Self {
            term,
            limit,
            outcome: Outcome::default(),
        }
    }

    pub(super) fn finish(self) -> Outcome {
        self.outcome
    }

    #[inline]
    fn counted(&mut self, act: impl FnOnce(&mut Term<Events>)) {
        let alternate = self.term.mode().contains(TermMode::ALT_SCREEN);
        let before = self.term.grid().history_size();
        act(self.term);
        if alternate || self.term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let after = self.term.grid().history_size();
        if after < before {
            self.outcome.cleared = true;
            return;
        }
        if after == before {
            return;
        }
        self.outcome.pushed += (after - before) as u64;
        if after > self.limit {
            let room = self.limit + self.term.grid().screen_lines();
            let grid = self.term.grid_mut();
            grid.update_history(self.limit);
            grid.update_history(room);
        }
    }
}

macro_rules! pass {
    ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            #[inline]
            fn $name(&mut self, $($arg: $ty),*) {
                self.term.$name($($arg),*)
            }
        )*
    };
}

macro_rules! count {
    ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(
            #[inline]
            fn $name(&mut self, $($arg: $ty),*) {
                self.counted(|term| term.$name($($arg),*))
            }
        )*
    };
}

impl Handler for Counting<'_> {
    count! {
        input(c: char);
        linefeed();
        newline();
        scroll_up(lines: usize);
        delete_lines(lines: usize);
        clear_screen(mode: ClearMode);
        set_private_mode(mode: PrivateMode);
        unset_private_mode(mode: PrivateMode);
    }

    fn reset_state(&mut self) {
        self.term.reset_state();
        self.outcome.cleared = true;
        self.outcome.palette = true;
    }

    fn set_color(&mut self, index: usize, color: Rgb) {
        self.term.set_color(index, color);
        self.outcome.palette = true;
    }

    fn reset_color(&mut self, index: usize) {
        self.term.reset_color(index);
        self.outcome.palette = true;
    }

    pass! {
        set_title(title: Option<String>);
        set_cursor_style(style: Option<CursorStyle>);
        set_cursor_shape(shape: CursorShape);
        goto(line: i32, col: usize);
        goto_line(line: i32);
        goto_col(col: usize);
        insert_blank(count: usize);
        move_up(lines: usize);
        move_down(lines: usize);
        identify_terminal(intermediate: Option<char>);
        device_status(arg: usize);
        move_forward(col: usize);
        move_backward(col: usize);
        move_down_and_cr(row: usize);
        move_up_and_cr(row: usize);
        put_tab(count: u16);
        backspace();
        carriage_return();
        bell();
        substitute();
        set_horizontal_tabstop();
        scroll_down(lines: usize);
        insert_blank_lines(lines: usize);
        erase_chars(count: usize);
        delete_chars(count: usize);
        move_backward_tabs(count: u16);
        move_forward_tabs(count: u16);
        save_cursor_position();
        restore_cursor_position();
        clear_line(mode: LineClearMode);
        clear_tabs(mode: TabulationClearMode);
        set_tabs(interval: u16);
        reverse_index();
        terminal_attribute(attr: Attr);
        set_mode(mode: Mode);
        unset_mode(mode: Mode);
        report_mode(mode: Mode);
        report_private_mode(mode: PrivateMode);
        set_scrolling_region(top: usize, bottom: Option<usize>);
        set_keypad_application_mode();
        unset_keypad_application_mode();
        set_active_charset(index: CharsetIndex);
        configure_charset(index: CharsetIndex, charset: StandardCharset);
        clipboard_store(clipboard: u8, base64: &[u8]);
        clipboard_load(clipboard: u8, terminator: &str);
        decaln();
        push_title();
        pop_title();
        text_area_size_pixels();
        text_area_size_chars();
        set_hyperlink(link: Option<Hyperlink>);
        report_keyboard_mode();
        push_keyboard_mode(mode: KeyboardModes);
        pop_keyboard_modes(count: u16);
        set_keyboard_mode(mode: KeyboardModes, behavior: KeyboardModesApplyBehavior);
        set_modify_other_keys(mode: ModifyOtherKeys);
        report_modify_other_keys();
        set_scp(path: ScpCharPath, update: ScpUpdateMode);
    }

    fn dynamic_color_sequence(&mut self, prefix: String, index: usize, terminator: &str) {
        self.term.dynamic_color_sequence(prefix, index, terminator)
    }
}
