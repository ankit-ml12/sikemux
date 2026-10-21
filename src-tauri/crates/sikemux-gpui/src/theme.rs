//! DESIGN.md's tokens, derived from the palette the core resolves the way
//! src/styles/tokens.css derives them.

use std::borrow::Cow;

use gpui_kit::component::theme::Theme as KitTheme;
use gpui_kit::{px, App, BoxShadow, FontFeatures, Global, Hsla, Pixels, Rgba, SharedString};
use sikemux_wire::grid::TerminalColors;
use sikemux_wire::theme::{Palette, ThemeEditor, ThemeHighlight};

pub const UI: &str = "Figtree";
pub const MONO: &str = "JetBrainsMono NFM";
/// The Nerd Font icon glyphs, which mono text falls back to.
pub const MONO_ICONS: &str = "JetBrainsMono NFM Icons";
pub const KBD: &str = ".SystemUIFont";

pub const RADIUS_1: Pixels = px(4.);
pub const RADIUS_2: Pixels = px(5.);
pub const RADIUS_3: Pixels = px(10.);
pub const RADIUS_4: Pixels = px(14.);

/// What a find in a terminal tints its matches with, the current one brighter, in every theme.
const FIND_MATCH: u32 = 0x433866;
const FIND_CURRENT: u32 = 0xa277ff;

#[derive(Clone)]
pub struct Tokens {
    pub surface_sunken: Hsla,
    pub surface: Hsla,
    pub surface_raised: Hsla,
    /// What a chat's composer and its docked strips are filled with.
    pub surface_composer: Hsla,
    pub surface_overlay: Hsla,
    pub surface_hover: Hsla,
    pub surface_chosen: Hsla,
    pub surface_active: Hsla,
    pub surface_selected: Hsla,
    pub surface_tooltip: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub border_selected: Hsla,
    pub line: Hsla,
    pub line_soft: Hsla,
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_disabled: Hsla,
    /// How git sees a file: changed, new to it, added, gone, or moved.
    pub git_modified: Hsla,
    pub git_untracked: Hsla,
    pub git_added: Hsla,
    pub git_deleted: Hsla,
    pub git_renamed: Hsla,
    /// Header actions and other glyphs that recede until hovered.
    pub icon_quiet: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub live: Hsla,
    pub warn: Hsla,
    pub cmd: Hsla,
    /// The mark of a host on the account.
    pub host: Hsla,
    /// An SSH host's mark.
    pub ssh: Hsla,
    pub tree_spine: Hsla,
    pub tree_tick: Hsla,
    /// What named and indexed colours look like in a terminal; the core answers programs' colour queries with it.
    pub terminal: TerminalColors,
    pub terminal_cursor_text: Hsla,
    pub terminal_selection: Hsla,
    pub terminal_match: Hsla,
    pub terminal_match_current: Hsla,
    /// The code editor's colours and syntax highlighting, as CSS colours [`color`] reads.
    pub editor: ThemeEditor,
    pub highlight: ThemeHighlight,
}

impl Global for Tokens {}

impl Tokens {
    pub fn new(palette: &Palette) -> Self {
        let (c, t) = (&palette.theme.chrome, &palette.theme.terminal);
        let ink = parse(&c.ink);
        let base = parse(&c.bg);
        let gray = |percent: f32| mix_oklab(ink, base, percent / 100.);
        let tint = |color: Rgba, alpha: f32| Rgba { a: alpha, ..color };
        let acc = parse(&c.acc);
        let (bg_dim, ink_dim, line, cmd) = (parse(&c.bg_dim), parse(&c.ink_dim), parse(&c.line), parse(&palette.cmd));
        let ansi = [
            &t.black, &t.red, &t.green, &t.yellow, &t.blue, &t.magenta, &t.cyan, &t.white, &t.bright_black, &t.bright_red,
            &t.bright_green, &t.bright_yellow, &t.bright_blue, &t.bright_magenta, &t.bright_cyan, &t.bright_white,
        ];
        Self {
            surface_sunken: bg_dim.into(),
            surface: base.into(),
            surface_raised: gray(7.).into(),
            surface_composer: mix_srgb(gray(7.), bg_dim, 0.55).into(),
            surface_overlay: gray(4.).into(),
            surface_hover: tint(ink, 0.06).into(),
            surface_chosen: tint(ink, 0.09).into(),
            surface_active: tint(ink, 0.10).into(),
            surface_selected: tint(acc, 0.14).into(),
            surface_tooltip: mix_srgb(hex(0xffffff), gray(7.), 0.06 / 1.03).into(),
            border: gray(11.).into(),
            border_strong: gray(22.).into(),
            border_selected: tint(acc, 0.42).into(),
            line: line.into(),
            line_soft: parse(&c.bg_raised).into(),
            text_primary: ink.into(),
            text_secondary: gray(76.).into(),
            text_tertiary: parse(&palette.ink_muted).into(),
            text_disabled: gray(32.).into(),
            git_modified: parse(&palette.git_modified).into(),
            git_untracked: parse(&palette.git_untracked).into(),
            git_added: parse(&palette.git_added).into(),
            git_deleted: parse(&palette.git_deleted).into(),
            git_renamed: parse(&palette.git_renamed).into(),
            icon_quiet: gray(42.).into(),
            accent: acc.into(),
            on_accent: bg_dim.into(),
            danger: parse(&palette.danger).into(),
            live: parse(&palette.live).into(),
            warn: parse(&palette.warn).into(),
            cmd: cmd.into(),
            host: mix_oklab(acc, ink_dim, 0.8).into(),
            ssh: mix_oklab(cmd, ink_dim, 0.7).into(),
            tree_spine: mix_srgb(acc, line, 0.52).into(),
            tree_tick: tint(acc, 0.72).into(),
            terminal: TerminalColors {
                foreground: bytes(parse(&t.foreground)),
                background: bytes(parse(&t.background)),
                cursor: Some(bytes(parse(&t.cursor))),
                ansi: ansi.map(|color| bytes(parse(color))),
            },
            terminal_cursor_text: parse(&t.cursor_accent).into(),
            terminal_selection: parse(&t.selection_background).into(),
            terminal_match: hex(FIND_MATCH).into(),
            terminal_match_current: hex(FIND_CURRENT).into(),
            editor: palette.theme.editor.clone(),
            highlight: palette.theme.highlight.clone(),
        }
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// DESIGN.md's `--shadow-4`, for anything that floats.
    pub fn shadow_float(&self) -> Vec<BoxShadow> {
        let black = |alpha: f32| Hsla { h: 0., s: 0., l: 0., a: alpha };
        vec![
            BoxShadow {
                color: black(0.44),
                offset: gpui_kit::point(px(0.), px(8.)),
                blur_radius: px(16.),
                spread_radius: px(-8.),
                inset: false,
            },
            BoxShadow {
                color: black(0.62),
                offset: gpui_kit::point(px(0.), px(28.)),
                blur_radius: px(64.),
                spread_radius: px(-16.),
                inset: false,
            },
        ]
    }
}

pub fn brand(provider: &str) -> Hsla {
    hex(match provider {
        "claude" => 0xd97757,
        "codex" => 0x7a9dff,
        "hermes" => 0xe0a050,
        "pi" => 0x7dd3fc,
        "opencode" => 0xa78bfa,
        "omp" => 0xf97316,
        "grok" => 0xfcfcfc,
        "muse" => 0x0081fb,
        _ => 0x8b8898,
    })
    .into()
}

pub fn tabular() -> FontFeatures {
    FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

/// Loads the bundled fonts and draws with the default theme until the core says which is picked.
pub fn install(cx: &mut App) {
    let fonts = [
        include_bytes!("../../../notch/Fonts/Figtree_400Regular.ttf").as_slice(),
        include_bytes!("../../../notch/Fonts/Figtree_500Medium.ttf").as_slice(),
        include_bytes!("../../../notch/Fonts/Figtree_600SemiBold.ttf").as_slice(),
        include_bytes!("../assets/fonts/Figtree_400Regular_Italic.ttf").as_slice(),
        include_bytes!("../assets/fonts/Figtree_600SemiBold_Italic.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Bold-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Italic-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-BoldItalic-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular-Icons.ttf").as_slice(),
    ];
    cx.text_system()
        .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
        .expect("bundled fonts load");

    apply(&sikemux_core::server::themes::default_palette(), cx);
}

/// Draws with `palette` from the next frame, GPUI Kit's widgets too.
pub fn apply(palette: &Palette, cx: &mut App) {
    let tokens = Tokens::new(palette);
    let kit = KitTheme::global_mut(cx);
    kit.font_family = SharedString::new_static(UI);
    kit.mono_font_family = SharedString::new_static(MONO);
    kit.font_size = px(13.);
    kit.mono_font_size = px(11.);
    kit.radius = RADIUS_2;
    kit.radius_lg = RADIUS_4;
    kit.background = tokens.surface;
    kit.foreground = tokens.text_primary;
    kit.muted_foreground = tokens.text_tertiary;
    kit.popover = tokens.surface_overlay;
    kit.popover_foreground = tokens.text_primary;
    kit.border = tokens.border;
    kit.input = tokens.border;
    kit.ring = tokens.border_selected;
    kit.caret = tokens.text_primary;
    kit.selection = Hsla { a: 0.30, ..tokens.accent };
    kit.accent = tokens.surface_hover;
    kit.accent_foreground = tokens.text_primary;
    kit.list_hover = tokens.surface_hover;
    kit.list_active = tokens.surface_chosen;
    kit.primary = tokens.accent;
    kit.primary_foreground = tokens.on_accent;
    kit.danger = tokens.danger;
    kit.scrollbar_thumb = Hsla { a: 0.42, ..tokens.text_tertiary };
    cx.set_global(tokens);
}

fn bytes(color: Rgba) -> [u8; 3] {
    [color.r, color.g, color.b].map(|channel| (channel * 255.).round() as u8)
}

/// A CSS colour as themes write them: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()` or `transparent`.
pub fn color(css: &str) -> Hsla {
    parse(css).into()
}

fn parse(css: &str) -> Rgba {
    let css = css.trim();
    if css == "transparent" {
        return Rgba { r: 0., g: 0., b: 0., a: 0. };
    }
    if let Some(digits) = css.strip_prefix('#') {
        let digits: String = if digits.len() == 3 { digits.chars().flat_map(|digit| [digit, digit]).collect() } else { digits.to_owned() };
        match (digits.len(), u32::from_str_radix(&digits, 16)) {
            (6, Ok(value)) => return gpui_kit::rgb(value),
            (8, Ok(value)) => return gpui_kit::rgba(value),
            _ => {}
        }
    }
    let channels = css.strip_prefix("rgba(").or_else(|| css.strip_prefix("rgb(")).and_then(|rest| rest.strip_suffix(')'));
    let parts: Vec<f32> = channels.into_iter().flat_map(|rest| rest.split(',')).filter_map(|part| part.trim().parse().ok()).collect();
    match parts.as_slice() {
        [r, g, b, rest @ ..] => Rgba { r: r / 255., g: g / 255., b: b / 255., a: rest.first().copied().unwrap_or(1.) },
        _ => Rgba { r: 0., g: 0., b: 0., a: 1. },
    }
}

fn hex(value: u32) -> Rgba {
    gpui_kit::rgb(value)
}

fn mix_srgb(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let lerp = |x: f32, y: f32| x * amount + y * (1. - amount);
    Rgba { r: lerp(a.r, b.r), g: lerp(a.g, b.g), b: lerp(a.b, b.b), a: 1. }
}

/// CSS `color-mix(in oklab, a amount, b)` for two opaque colours.
/// `a` mixed into `b` by `amount`, as CSS's `color-mix(in oklab, a amount, b)` does.
pub fn mix(a: Hsla, b: Hsla, amount: f32) -> Hsla {
    mix_oklab(a.into(), b.into(), amount).into()
}

fn mix_oklab(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let [l1, a1, b1] = oklab(a);
    let [l2, a2, b2] = oklab(b);
    let lerp = |x: f32, y: f32| x * amount + y * (1. - amount);
    from_oklab([lerp(l1, l2), lerp(a1, a2), lerp(b1, b2)])
}

fn oklab(color: Rgba) -> [f32; 3] {
    let linear = |c: f32| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let (r, g, b) = (linear(color.r), linear(color.g), linear(color.b));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn from_oklab([l, a, b]: [f32; 3]) -> Rgba {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let gamma = |c: f32| {
        let c = c.clamp(0., 1.);
        if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1. / 2.4) - 0.055 }
    };
    Rgba {
        r: gamma(4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_),
        g: gamma(-1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_),
        b: gamma(-0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_),
        a: 1.,
    }
}
