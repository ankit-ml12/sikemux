//! The colour themes the person picks from, and the one they picked: the
//! Sikemux themes, Ghostty's catalogue with the app's colours worked out from
//! each terminal palette, and the person's own, kept in preferences.

use std::sync::OnceLock;

use base64::Engine;
use sikemux_wire::grid::TerminalColors;
use sikemux_wire::theme::{
    Palette, Theme, ThemeChrome, ThemeEditor, ThemeHighlight, ThemeSource, ThemeSummary,
    ThemeTerminal,
};

use crate::protocol::{Preferences, DEFAULT_THEME_ID};

struct Catalogue {
    sikemux: Vec<Theme>,
    ghostty: Vec<Theme>,
}

fn catalogue() -> &'static Catalogue {
    static CATALOGUE: OnceLock<Catalogue> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let sikemux: Vec<Theme> = serde_json::from_str(include_str!("themes/curated.json"))
            .expect("the Sikemux themes parse");
        let taken: Vec<String> = sikemux
            .iter()
            .map(|theme| slugify(&theme.name).replace('-', ""))
            .collect();
        let packed: Vec<(String, String)> =
            serde_json::from_str(include_str!("themes/ghostty.json"))
                .expect("Ghostty's catalogue parses");
        let ghostty = packed
            .into_iter()
            .filter(|(name, _)| !taken.contains(&slugify(name).replace('-', "")))
            .filter_map(|(name, packed)| ghostty_theme(&name, &unpack(&packed)?))
            .collect();
        Catalogue { sikemux, ghostty }
    })
}

/// The theme the person picked, or the default when it is gone.
pub(crate) fn resolve(preferences: &Preferences) -> Theme {
    let catalogue = catalogue();
    let find = |id: &str| {
        preferences
            .custom_themes
            .iter()
            .chain(&catalogue.sikemux)
            .chain(&catalogue.ghostty)
            .find(|theme| theme.id == id)
    };
    find(&preferences.theme_id)
        .or_else(|| find(DEFAULT_THEME_ID))
        .cloned()
        .expect("the default theme is in the catalogue")
}

/// The picked theme with the colours worked out from it for drawing.
pub(crate) fn palette(preferences: &Preferences) -> Palette {
    let theme = resolve(preferences);
    let c = &theme.chrome;
    let readable_on = |color: &str, grounds: &[&str]| readable_css(color, grounds, &c.ink, 4.6);
    let all = [c.bg.as_str(), c.bg_dim.as_str(), c.bg_raised.as_str()];
    let near = [c.bg.as_str(), c.bg_raised.as_str()];
    let tone = |dark: &str, light: &str| {
        if theme.dark {
            dark.to_owned()
        } else {
            light.to_owned()
        }
    };
    Palette {
        ink_muted: readable_on(&c.ink_dim, &all),
        danger: readable_on(&c.danger, &all),
        live: readable_on(&tone("#78dca1", "#247345"), &near),
        warn: readable_on(&tone("#e9ba6c", "#875508"), &near),
        cmd: theme.highlight.function.clone(),
        git_modified: readable_on(&tone("#e2c08d", "#875508"), &all),
        git_untracked: readable_on(&tone("#73c991", "#247345"), &all),
        git_added: readable_on(&tone("#81b88b", "#247345"), &all),
        git_deleted: readable_on(&tone("#ed806b", "#aa3824"), &all),
        git_renamed: readable_on(&tone("#94b1ef", "#345bad"), &all),
        theme,
    }
}

/// [`readable`] for CSS colours; any that is not `#rgb` or `#rrggbb` gives `ink`.
fn readable_css(color: &str, grounds: &[&str], ink: &str, minimum: f64) -> String {
    let parsed = (
        hex(color),
        hex(ink),
        grounds
            .iter()
            .map(|ground| hex(ground))
            .collect::<Option<Vec<_>>>(),
    );
    match parsed {
        (Some(color), Some(ink), Some(grounds)) => css(readable(color, &grounds, ink, minimum)),
        _ => ink.to_owned(),
    }
}

/// Every theme to pick from: the person's own first, then Sikemux's, then Ghostty's.
pub(crate) fn summaries(preferences: &Preferences) -> Vec<ThemeSummary> {
    let catalogue = catalogue();
    let sources = [
        (ThemeSource::Custom, preferences.custom_themes.as_slice()),
        (ThemeSource::Sikemux, catalogue.sikemux.as_slice()),
        (ThemeSource::Ghostty, catalogue.ghostty.as_slice()),
    ];
    sources
        .into_iter()
        .flat_map(|(source, themes)| themes.iter().map(move |theme| summary(theme, source)))
        .collect()
}

fn summary(theme: &Theme, source: ThemeSource) -> ThemeSummary {
    let t = &theme.terminal;
    ThemeSummary {
        id: theme.id.clone(),
        name: theme.name.clone(),
        dark: theme.dark,
        source,
        bg: theme.chrome.bg.clone(),
        line: theme.chrome.line.clone(),
        acc: theme.chrome.acc.clone(),
        swatches: [&t.red, &t.green, &t.yellow, &t.blue, &t.magenta, &t.cyan]
            .into_iter()
            .cloned()
            .collect(),
    }
}

/// The colours the core answers programs' colour queries with.
pub(crate) fn terminal_colors(theme: &Theme) -> TerminalColors {
    let t = &theme.terminal;
    let fallback = TerminalColors::default();
    let ansi = [
        &t.black,
        &t.red,
        &t.green,
        &t.yellow,
        &t.blue,
        &t.magenta,
        &t.cyan,
        &t.white,
        &t.bright_black,
        &t.bright_red,
        &t.bright_green,
        &t.bright_yellow,
        &t.bright_blue,
        &t.bright_magenta,
        &t.bright_cyan,
        &t.bright_white,
    ];
    TerminalColors {
        foreground: hex(&t.foreground).unwrap_or(fallback.foreground),
        background: hex(&t.background).unwrap_or(fallback.background),
        cursor: hex(&t.cursor),
        ansi: std::array::from_fn(|index| hex(ansi[index]).unwrap_or(fallback.ansi[index])),
    }
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`, ignoring the alpha.
fn hex(color: &str) -> Option<[u8; 3]> {
    let digits = color.trim().strip_prefix('#')?;
    let digits: String = match digits.len() {
        3 => digits.chars().flat_map(|digit| [digit, digit]).collect(),
        6 | 8 => digits[..6].to_owned(),
        _ => return None,
    };
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for character in name.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_owned()
}

/// The catalogue keeps each theme's colours as base64 bytes, three to a
/// colour; a theme with no selection colour has 20 colours instead of 21.
fn unpack(packed: &str) -> Option<Vec<Option<[u8; 3]>>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(packed)
        .ok()?;
    let mut colours: Vec<Option<[u8; 3]>> = bytes
        .as_chunks::<3>()
        .0
        .iter()
        .map(|rgb| Some(*rgb))
        .collect();
    if colours.len() == 20 {
        colours.insert(4, None);
    }
    (colours.len() == 21).then_some(colours)
}

fn css([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn mix(from: [u8; 3], to: [u8; 3], amount: f64) -> [u8; 3] {
    std::array::from_fn(|index| {
        let (a, b) = (f64::from(from[index]), f64::from(to[index]));
        (a + (b - a) * amount).round() as u8
    })
}

fn alpha([r, g, b]: [u8; 3], amount: f64) -> String {
    format!("rgba({r},{g},{b},{amount})")
}

fn luminance(rgb: [u8; 3]) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(rgb[0]) + 0.7152 * linear(rgb[1]) + 0.0722 * linear(rgb[2])
}

fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (first, second) = (luminance(a), luminance(b));
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

/// `color` moved toward `ink` just far enough to read at `minimum` contrast on every background.
fn readable(color: [u8; 3], backgrounds: &[[u8; 3]], ink: [u8; 3], minimum: f64) -> [u8; 3] {
    let toward = |step: u8| -> [u8; 3] {
        std::array::from_fn(|index| {
            let (a, b) = (f64::from(color[index]), f64::from(ink[index]));
            (a + (b - a) * f64::from(step) / 100.).round() as u8
        })
    };
    (0..=100)
        .map(toward)
        .find(|candidate| {
            backgrounds
                .iter()
                .all(|background| contrast(*candidate, *background) >= minimum)
        })
        .unwrap_or(ink)
}

fn saturation([r, g, b]: [u8; 3]) -> f64 {
    let (r, g, b) = (
        f64::from(r) / 255.,
        f64::from(g) / 255.,
        f64::from(b) / 255.,
    );
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max == min {
        return 0.;
    }
    (max - min) / (1. - (max + min - 1.).abs())
}

/// The app's, editor's and syntax colours worked out from a terminal palette,
/// the way the Mac app worked them out for Ghostty's themes.
fn ghostty_theme(name: &str, colours: &[Option<[u8; 3]>]) -> Option<Theme> {
    let (bg, raw_fg, cursor, cursor_text) = (colours[0]?, colours[1]?, colours[2]?, colours[3]?);
    let raw_selection = colours[4];
    let ansi: Vec<[u8; 3]> = colours[5..].iter().copied().collect::<Option<_>>()?;
    let (white, black) = ([0xff; 3], [0; 3]);
    let dark = contrast(white, bg) >= contrast(black, bg);
    let bg_dim = mix(bg, black, if dark { 0.22 } else { 0.035 });
    let bg_raised = mix(bg, raw_fg, if dark { 0.06 } else { 0.045 });
    let fg = readable(
        raw_fg,
        &[bg, bg_dim, bg_raised],
        if dark { white } else { black },
        5.,
    );
    let acc = [
        ansi[4], ansi[12], ansi[5], ansi[13], ansi[6], ansi[14], ansi[3], ansi[11],
    ]
    .into_iter()
    .find(|&candidate| saturation(candidate) >= 0.35 && contrast(candidate, bg) >= 3.)
    .unwrap_or(fg);
    let line = mix(bg, fg, if dark { 0.11 } else { 0.13 });
    let selection = raw_selection
        .filter(|&selection| contrast(selection, fg) >= 2.)
        .unwrap_or_else(|| mix(bg, acc, 0.28));
    let ink_dim = mix(fg, bg, 0.38);
    let tone = |color: [u8; 3]| css(readable(color, &[bg], fg, 3.));
    let terminal = |index: usize| css(ansi[index]);
    Some(Theme {
        id: format!("ghostty-{}", slugify(name)),
        name: name.to_owned(),
        dark,
        chrome: ThemeChrome {
            bg: css(bg),
            bg_dim: css(bg_dim),
            bg_raised: css(bg_raised),
            ink: css(fg),
            ink_dim: css(ink_dim),
            ink_muted: css(mix(fg, bg, 0.5)),
            acc: css(acc),
            acc_line: alpha(acc, 0.3),
            acc_dim: alpha(acc, 0.1),
            line: css(line),
            hl: alpha(acc, 0.055),
            danger: terminal(1),
        },
        editor: ThemeEditor {
            fg: css(fg),
            bg: "transparent".to_owned(),
            caret: css(cursor),
            selection: css(selection),
            active_line: alpha(acc, 0.055),
            gutter: css(mix(fg, bg, 0.62)),
            gutter_active: css(acc),
            indent: css(line),
            indent_active: css(mix(bg, fg, 0.2)),
        },
        highlight: ThemeHighlight {
            keyword: tone(ansi[5]),
            string: tone(ansi[2]),
            comment: tone(ansi[8]),
            number: tone(ansi[3]),
            function: tone(ansi[4]),
            kind: tone(ansi[11]),
            variable: css(fg),
            property: tone(ansi[6]),
            tag: tone(ansi[1]),
            operator: css(ink_dim),
            link: tone(ansi[6]),
            invalid: tone(ansi[9]),
            meta: css(ink_dim),
            heading: css(acc),
        },
        terminal: ThemeTerminal {
            background: css(bg),
            foreground: css(raw_fg),
            cursor: css(cursor),
            cursor_accent: css(cursor_text),
            selection_background: css(selection),
            black: terminal(0),
            red: terminal(1),
            green: terminal(2),
            yellow: terminal(3),
            blue: terminal(4),
            magenta: terminal(5),
            cyan: terminal(6),
            white: terminal(7),
            bright_black: terminal(8),
            bright_red: terminal(9),
            bright_green: terminal(10),
            bright_yellow: terminal(11),
            bright_blue: terminal(12),
            bright_magenta: terminal(13),
            bright_cyan: terminal(14),
            bright_white: terminal(15),
        },
    })
}
