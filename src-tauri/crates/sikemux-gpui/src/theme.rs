//! DESIGN.md's tokens, derived from a theme's palette the way src/styles/tokens.css
//! derives them, so every theme the app has can be one palette here.

use std::borrow::Cow;

use gpui_kit::component::theme::Theme as KitTheme;
use gpui_kit::{px, App, BoxShadow, FontFeatures, Global, Hsla, Pixels, Rgba, SharedString};

pub const UI: &str = "Figtree";
pub const MONO: &str = "JetBrainsMono NFM";
/// The Nerd Font icon glyphs, which mono text falls back to.
pub const MONO_ICONS: &str = "JetBrainsMono NFM Icons";
pub const KBD: &str = ".SystemUIFont";

pub const RADIUS_1: Pixels = px(4.);
pub const RADIUS_2: Pixels = px(5.);
pub const RADIUS_3: Pixels = px(10.);
pub const RADIUS_4: Pixels = px(14.);

/// A theme's own colours, as src/themes/index.ts lists them.
pub struct Palette {
    pub bg: u32,
    pub bg_dim: u32,
    pub bg_raised: u32,
    pub ink: u32,
    pub ink_dim: u32,
    pub acc: u32,
    pub line: u32,
    pub danger: u32,
    pub live: u32,
    pub warn: u32,
    pub cmd: u32,
}

pub const AURA_NOIR: Palette = Palette {
    bg: 0x0f0f13,
    bg_dim: 0x09090b,
    bg_raised: 0x19191e,
    ink: 0xe7e5ef,
    ink_dim: 0x8b8898,
    acc: 0xa277ff,
    line: 0x24242b,
    danger: 0xff6767,
    live: 0x78dca1,
    warn: 0xe9ba6c,
    cmd: 0xff6ac1,
};

#[derive(Clone)]
pub struct Tokens {
    pub surface_sunken: Hsla,
    pub surface: Hsla,
    pub surface_raised: Hsla,
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
    /// Header actions and other glyphs that recede until hovered.
    pub icon_quiet: Hsla,
    pub accent: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub live: Hsla,
    pub warn: Hsla,
    pub cmd: Hsla,
    pub tree_spine: Hsla,
    pub tree_tick: Hsla,
}

impl Global for Tokens {}

impl Tokens {
    pub fn new(palette: &Palette) -> Self {
        let ink = hex(palette.ink);
        let base = hex(palette.bg);
        let gray = |percent: f32| mix_oklab(ink, base, percent / 100.);
        let tint = |color: Rgba, alpha: f32| Rgba { a: alpha, ..color };
        let acc = hex(palette.acc);
        Self {
            surface_sunken: hex(palette.bg_dim).into(),
            surface: base.into(),
            surface_raised: gray(7.).into(),
            surface_overlay: gray(4.).into(),
            surface_hover: tint(ink, 0.06).into(),
            surface_chosen: tint(ink, 0.09).into(),
            surface_active: tint(ink, 0.10).into(),
            surface_selected: tint(acc, 0.14).into(),
            surface_tooltip: mix_srgb(hex(0xffffff), gray(7.), 0.06 / 1.03).into(),
            border: gray(11.).into(),
            border_strong: gray(22.).into(),
            border_selected: tint(acc, 0.42).into(),
            line: hex(palette.line).into(),
            line_soft: hex(palette.bg_raised).into(),
            text_primary: ink.into(),
            text_secondary: gray(76.).into(),
            text_tertiary: hex(palette.ink_dim).into(),
            text_disabled: gray(32.).into(),
            icon_quiet: gray(42.).into(),
            accent: acc.into(),
            on_accent: hex(palette.bg_dim).into(),
            danger: hex(palette.danger).into(),
            live: hex(palette.live).into(),
            warn: hex(palette.warn).into(),
            cmd: hex(palette.cmd).into(),
            tree_spine: mix_srgb(acc, hex(palette.line), 0.52).into(),
            tree_tick: tint(acc, 0.72).into(),
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
        _ => AURA_NOIR.ink_dim,
    })
    .into()
}

pub fn tabular() -> FontFeatures {
    FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

/// Loads the bundled fonts, sets the tokens, and gives GPUI Kit's widgets the same.
pub fn install(cx: &mut App) {
    let fonts = [
        include_bytes!("../../../notch/Fonts/Figtree_400Regular.ttf").as_slice(),
        include_bytes!("../../../notch/Fonts/Figtree_500Medium.ttf").as_slice(),
        include_bytes!("../../../notch/Fonts/Figtree_600SemiBold.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Bold-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Italic-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-BoldItalic-Base.ttf").as_slice(),
        include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular-Icons.ttf").as_slice(),
    ];
    cx.text_system()
        .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
        .expect("bundled fonts load");

    let tokens = Tokens::new(&AURA_NOIR);
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

fn hex(value: u32) -> Rgba {
    gpui_kit::rgb(value)
}

fn mix_srgb(a: Rgba, b: Rgba, amount: f32) -> Rgba {
    let lerp = |x: f32, y: f32| x * amount + y * (1. - amount);
    Rgba { r: lerp(a.r, b.r), g: lerp(a.g, b.g), b: lerp(a.b, b.b), a: 1. }
}

/// CSS `color-mix(in oklab, a amount, b)` for two opaque colours.
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
