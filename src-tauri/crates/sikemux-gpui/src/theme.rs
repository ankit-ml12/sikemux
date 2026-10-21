//! DESIGN.md's tokens for the default theme, aura-noir, at full opacity.

use std::borrow::Cow;

use gpui_kit::{px, rgb, rgba, App, FontFeatures, Pixels, Rgba};

pub const UI: &str = "Figtree";
pub const MONO: &str = "JetBrainsMono NFM";
pub const KBD: &str = ".SystemUIFont";

pub fn sunken() -> Rgba {
    rgb(0x09090b)
}
pub fn surface() -> Rgba {
    rgb(0x0f0f13)
}
pub fn hover() -> Rgba {
    rgba(0xe7e5ef0f)
}
pub fn chosen() -> Rgba {
    rgba(0xe7e5ef17)
}
pub fn active() -> Rgba {
    rgba(0xe7e5ef1a)
}
pub fn border() -> Rgba {
    rgb(0x222227)
}
pub fn line() -> Rgba {
    rgb(0x24242b)
}
pub fn line_soft() -> Rgba {
    rgb(0x19191e)
}
pub fn primary() -> Rgba {
    rgb(0xe7e5ef)
}
pub fn secondary() -> Rgba {
    rgb(0xacabb4)
}
pub fn tertiary() -> Rgba {
    rgb(0x8b8898)
}
pub fn gray_700() -> Rgba {
    rgb(0x605f65)
}
pub fn accent() -> Rgba {
    rgb(0xa277ff)
}
pub fn live() -> Rgba {
    rgb(0x78dca1)
}
pub fn warn() -> Rgba {
    rgb(0xe9ba6c)
}
pub fn tree_spine() -> Rgba {
    rgb(0x664f99)
}
pub fn tree_tick() -> Rgba {
    rgba(0xa277ffb8)
}

pub fn brand(provider: &str) -> Rgba {
    match provider {
        "claude" => rgb(0xd97757),
        "codex" => rgb(0x7a9dff),
        "hermes" => rgb(0xe0a050),
        "pi" => rgb(0x7dd3fc),
        "opencode" => rgb(0xa78bfa),
        "omp" => rgb(0xf97316),
        "grok" => rgb(0xfcfcfc),
        "muse" => rgb(0x0081fb),
        _ => tertiary(),
    }
}

pub const RADIUS_1: Pixels = px(4.);
pub const RADIUS_2: Pixels = px(5.);
pub const RADIUS_3: Pixels = px(10.);

pub fn tabular() -> FontFeatures {
    FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

pub fn load_fonts(cx: &App) {
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
}
