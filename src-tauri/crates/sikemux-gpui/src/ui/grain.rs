//! What the window's renderer paints behind panes and overlays with the shaders in
//! `assets/shaders`: the moving grain, the person's own picture dithered in its place,
//! and the sky over the welcome screen and the release notes. The grain and the picture are
//! drawn as dots or, as the person picks, as characters. The grain and the picture
//! are on the window's ground, which macOS keeps in a layer of its own at one pixel per
//! point, so the grain moves without anything over it drawn again. The sky is over the
//! shell, so it moves by showing the whole last frame again with the clock moved on.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use gpui::{
    canvas, relative, App, AppContext, Context, Global, Hsla, IntoElement, RenderImage,
    ShaderProgram, Styled, Task, Window,
};
use sikemux_wire::protocol::{PaneAscii, PaneStyle, Preferences};

use crate::theme::{self, Tokens, RADIUS_3};
use crate::workbench::Workbench;

macro_rules! program {
    ($name:literal) => {
        ShaderProgram {
            name: $name,
            wgsl: include_str!(concat!(env!("OUT_DIR"), "/shaders/", $name, ".wgsl")),
            msl: include_str!(concat!(env!("OUT_DIR"), "/shaders/", $name, ".metal")),
        }
    };
}

static FIELD: ShaderProgram = program!("field");
static PICTURE: ShaderProgram = program!("picture");
static SKY: ShaderProgram = program!("sky");
static ASCII_FIELD: ShaderProgram = program!("ascii_field");
static ASCII_PICTURE: ShaderProgram = program!("ascii_picture");

const FRAMES_PER_SECOND: u64 = 30;
/// Slower on battery, where every frame is paid for out of the charge.
const BATTERY_FRAMES_PER_SECOND: u64 = 20;
const FIELD_SPEED: f32 = 0.5;
/// The ASCII jellyfish keep time in seconds.
const ASCII_SPEED: f32 = 1.;
const SKY_SPEED: f32 = 0.35;
/// Where the clock stands for a person who asked for less motion, so everything holds still.
const STILL: f32 = 2.5;
/// High, because the grain's dots are the only paint and nothing is drawn between them.
const FIELD_OPACITY: f32 = 0.85;
const PICTURE_OPACITY: f32 = 0.38;
/// How much of a pane, from its top, the picture covers.
const PICTURE_BAND: f32 = 0.6;
/// The picture is drawn in two-pixel dots, so no more of it than one texel per dot is kept.
const PICTURE_LONGEST: u32 = 2560;
/// How many rows of the ASCII atlas its glyph strip takes, the rows an ASCII picture is put
/// beneath.
const GLYPH_STRIP: u32 = 28;

/// The glyphs and the jellyfish the ASCII style draws with, from scripts/generate-ascii-atlas.swift.
static ASCII_ATLAS: LazyLock<image::RgbaImage> = LazyLock::new(|| {
    image::load_from_memory(include_bytes!("../../assets/ascii-atlas.png"))
        .expect("the ASCII atlas is a PNG")
        .into_rgba8()
});
static ASCII_IMAGE: LazyLock<Arc<RenderImage>> =
    LazyLock::new(|| render_image(ASCII_ATLAS.clone()));

/// What panes show behind them, as the person's preferences say, and whether a sky is up.
/// Until the preferences are read, panes show nothing.
#[derive(Clone)]
pub struct Grain {
    shown: bool,
    style: PaneStyle,
    /// From 0 to 2.
    strength: f32,
    picture_path: Option<String>,
    picture: Picture,
    skies: usize,
}

#[derive(Clone)]
enum Picture {
    None,
    Loading,
    Loaded(LoadedPicture),
}

/// A picture as its style draws it: alone, or under the ASCII glyph strip.
#[derive(Clone)]
struct LoadedPicture {
    image: Arc<RenderImage>,
    width: u32,
    height: u32,
}

impl Global for Grain {}

impl Default for Grain {
    fn default() -> Self {
        Self {
            shown: false,
            style: PaneStyle::Dither,
            strength: 0.,
            picture_path: None,
            picture: Picture::None,
            skies: 0,
        }
    }
}

impl Grain {
    /// Only the grain moves: a picture holds still, and one still loading or a grain at no
    /// strength paints nothing.
    fn moving(&self) -> bool {
        self.skies > 0
            || (self.shown && self.strength > 0. && matches!(self.picture, Picture::None))
    }
}

/// Takes the preferences for what panes show, loading a newly chosen picture.
pub fn set(preferences: &Preferences, cx: &mut App) {
    let drawn =
        preferences.pane_style == PaneStyle::Dither || preferences.pane_ascii == PaneAscii::Image;
    let path = preferences.pane_image.clone().filter(|_| drawn);
    let style = preferences.pane_style;
    let known = cx.try_global::<Grain>().map(|grain| {
        let kept =
            (grain.picture_path == path && grain.style == style).then(|| grain.picture.clone());
        (kept, grain.skies)
    });
    let skies = known.as_ref().map_or(0, |(_, skies)| *skies);
    let picture = match known.and_then(|(kept, _)| kept) {
        Some(kept) => kept,
        None => {
            if let Some(path) = &path {
                load(path.clone(), style, cx);
            }
            path.as_ref().map_or(Picture::None, |_| Picture::Loading)
        }
    };
    let changed = cx.try_global::<Grain>().is_none_or(|grain| {
        grain.shown != preferences.pane_shader
            || grain.style != preferences.pane_style
            || grain.strength != preferences.pane_strength as f32
            || grain.picture_path != path
    });
    if !changed {
        return;
    }
    cx.set_global(Grain {
        shown: preferences.pane_shader,
        style: preferences.pane_style,
        strength: preferences.pane_strength as f32,
        picture_path: path,
        picture,
        skies,
    });
    cx.refresh_windows();
}

fn load(path: String, style: PaneStyle, cx: &mut App) {
    cx.spawn(async move |cx| {
        let read = path.clone();
        let picture = cx
            .background_spawn(async move { read_picture(Path::new(&read), style) })
            .await;
        cx.update(|cx| {
            if let Some(grain) = cx.try_global::<Grain>() {
                if grain.picture_path.as_deref() == Some(path.as_str()) && grain.style == style {
                    let mut grain = grain.clone();
                    grain.picture = match picture {
                        Some(picture) => Picture::Loaded(picture),
                        None => Picture::None,
                    };
                    cx.set_global(grain);
                    cx.refresh_windows();
                }
            }
        })
    })
    .detach();
}

/// The picture at `path` as the atlas takes it, no larger than its dots can show, and for
/// the ASCII style beneath the glyph strip so the one image holds both.
fn read_picture(path: &Path, style: PaneStyle) -> Option<LoadedPicture> {
    let decoded = image::open(path)
        .inspect_err(|error| eprintln!("sikemux: the pane picture does not open: {error}"))
        .ok()?;
    let fitted = match decoded.width().max(decoded.height()) > PICTURE_LONGEST {
        true => decoded.thumbnail(PICTURE_LONGEST, PICTURE_LONGEST),
        false => decoded,
    };
    let picture = fitted.into_rgba8();
    let (width, height) = picture.dimensions();
    let pixels = match style {
        PaneStyle::Dither => picture,
        PaneStyle::Ascii => {
            let atlas = &*ASCII_ATLAS;
            let mut both = image::RgbaImage::new(width.max(atlas.width()), GLYPH_STRIP + height);
            let strip = image::imageops::crop_imm(atlas, 0, 0, atlas.width(), GLYPH_STRIP);
            image::imageops::replace(&mut both, &*strip, 0, 0);
            image::imageops::replace(&mut both, &picture, 0, i64::from(GLYPH_STRIP));
            both
        }
    };
    Some(LoadedPicture {
        image: render_image(pixels),
        width,
        height,
    })
}

/// Pixels as the renderer's atlas takes them, blue first.
fn render_image(mut pixels: image::RgbaImage) -> Arc<RenderImage> {
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new([image::Frame::new(pixels)]))
}

/// A colour as one number, 0xRRGGBB, which a shader parameter holds exactly.
fn packed([r, g, b]: [u8; 3]) -> f32 {
    (u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)) as f32
}

fn bytes_of(colour: Hsla) -> [u8; 3] {
    let rgb = colour.to_rgb();
    [rgb.r, rgb.g, rgb.b].map(|channel| (channel.clamp(0., 1.) * 255.).round() as u8)
}

/// What fills a pane's frame behind everything the pane shows.
pub fn field(cx: &App) -> Option<impl IntoElement> {
    let grain = cx
        .try_global::<Grain>()
        .filter(|grain| grain.shown && grain.strength > 0.)?;
    let inner_radius = f32::from(RADIUS_3) - 1.;
    let t = Tokens::get(cx);
    if let Picture::Loaded(picture) = &grain.picture {
        let LoadedPicture {
            image,
            width,
            height,
        } = picture.clone();
        let strength = (PICTURE_OPACITY * grain.strength).min(1.);
        let (program, inks) = match grain.style {
            PaneStyle::Dither => (&PICTURE, [0.; 4]),
            PaneStyle::Ascii => (
                &ASCII_PICTURE,
                [
                    packed(bytes_of(t.text_primary)),
                    packed(bytes_of(t.surface)),
                    width as f32,
                    height as f32,
                ],
            ),
        };
        return Some(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| {
                    let mut params = [0.; gpui::SHADER_PARAMS];
                    params[..3].copy_from_slice(&[window.ground_scale(), inner_radius, strength]);
                    params[4..8].copy_from_slice(&inks);
                    if let Err(error) =
                        window.paint_ground_shader(bounds, program, params, Some(&image))
                    {
                        eprintln!("sikemux: the pane picture does not draw: {error}");
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(relative(PICTURE_BAND))
            .into_any_element(),
        );
    }
    if matches!(grain.picture, Picture::Loading) {
        return None;
    }
    let opacity = (FIELD_OPACITY * grain.strength).min(1.);
    let ansi = t.terminal.ansi;
    let (program, speed, colours, atlas) = match grain.style {
        PaneStyle::Dither => {
            let dot = t.shader_dot.to_rgb();
            let colours = [dot.r, dot.g, dot.b, dot.a * opacity];
            (&FIELD, FIELD_SPEED, colours, None)
        }
        PaneStyle::Ascii => {
            let colours = [
                ansi[14],
                bytes_of(t.accent),
                bytes_of(t.text_primary),
                bytes_of(t.surface),
            ]
            .map(packed);
            (
                &ASCII_FIELD,
                ASCII_SPEED,
                colours,
                Some(ASCII_IMAGE.clone()),
            )
        }
    };
    let accents = [ansi[13], ansi[11], ansi[10], bytes_of(t.accent)].map(packed);
    let (start, speed) = clock(speed, cx);
    Some(
        canvas(
            |_, _, _| {},
            move |bounds, (), window, _| {
                let mut params = [0.; gpui::SHADER_PARAMS];
                params[..4].copy_from_slice(&[start, speed, window.ground_scale(), inner_radius]);
                params[4..8].copy_from_slice(&colours);
                params[8..12].copy_from_slice(&accents);
                params[12] = opacity;
                let _ = window.paint_ground_shader(bounds, program, params, atlas.as_ref());
            },
        )
        .absolute()
        .inset_0()
        .into_any_element(),
    )
}

/// The release sky, in the accent running through the syntax pink to the syntax green,
/// fading out from `fade_from` of the way down to nothing at the bottom.
pub fn sky(fade_from: f32, opacity: f32, cx: &App) -> impl IntoElement + Styled {
    let t = Tokens::get(cx);
    let hues = [
        t.accent,
        theme::color(&t.highlight.function),
        theme::color(&t.highlight.string),
    ]
    .map(|hue: Hsla| hue.to_rgb());
    let (start, speed) = clock(SKY_SPEED, cx);
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let mut params = [0.; gpui::SHADER_PARAMS];
            params[..4].copy_from_slice(&[start, speed, window.scale_factor(), fade_from]);
            for (slot, hue) in params[4..].as_chunks_mut::<4>().0.iter_mut().zip(hues) {
                slot.copy_from_slice(&[hue.r, hue.g, hue.b, hue.a * opacity]);
            }
            let _ = window.paint_shader(bounds, &SKY, params, None);
        },
    )
}

/// Keeps the sky moving while a screen that shows one is up, even with panes' grain off, and
/// even when the screen opens before the person's preferences are read.
pub fn sky_shown(shown: bool, cx: &mut App) {
    let mut grain = cx.try_global::<Grain>().cloned().unwrap_or_default();
    grain.skies = match shown {
        true => grain.skies + 1,
        false => grain.skies.saturating_sub(1),
    };
    cx.set_global(grain);
}

/// Where a shader's clock starts and how fast it runs, holding still for less motion.
fn clock(speed: f32, cx: &App) -> (f32, f32) {
    match cx.reduce_motion() {
        true => (STILL, 0.),
        false => (0., speed),
    }
}

/// Moves what the shaders show on while the window is in front and can be seen, at the rate
/// the power allows.
pub fn animate(window: &mut Window, cx: &mut Context<Workbench>) {
    follow_reduced_motion(cx);
    #[cfg(target_os = "linux")]
    cx.spawn(async move |bench, cx| {
        let mut changes = linux::watch_motion();
        while changes.recv().await.is_some() {
            if bench.update(cx, |_, cx| follow_reduced_motion(cx)).is_err() {
                break;
            }
        }
    })
    .detach();
    let ticking = Rc::new(RefCell::new(None));
    let restart = |ticking: Rc<RefCell<Option<Task<()>>>>| {
        move |_: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>| {
            if window.is_window_active() {
                follow_reduced_motion(cx);
            }
            *ticking.borrow_mut() = tick(window, cx);
        }
    };
    cx.observe_window_activation(window, restart(ticking.clone()))
        .detach();
    let on_visibility = restart(ticking.clone());
    cx.observe_window_visibility(window, move |bench, _, window, cx| {
        on_visibility(bench, window, cx)
    })
    .detach();
    cx.observe_global_in::<Grain>(window, restart(ticking))
        .detach();
}

fn tick(window: &Window, cx: &mut Context<Workbench>) -> Option<Task<()>> {
    let moving = window.is_window_active()
        && window.is_visible()
        && !cx.reduce_motion()
        && cx.try_global::<Grain>().is_some_and(Grain::moving);
    moving.then(|| {
        cx.spawn_in(window, async move |bench, cx| loop {
            let Ok(wait) = bench.update_in(cx, |bench, window, cx| {
                match cx
                    .try_global::<Grain>()
                    .is_some_and(|grain| grain.skies > 0)
                {
                    true => {
                        window.present_shaders();
                        #[cfg(feature = "drive")]
                        SKY_PRESENTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    false => window.present_ground(),
                }
                let on_battery = bench.battery.is_some_and(|battery| !battery.charging);
                let rate = match on_battery {
                    true => BATTERY_FRAMES_PER_SECOND,
                    false => FRAMES_PER_SECOND,
                };
                Duration::from_millis(1000 / rate)
            }) else {
                break;
            };
            cx.background_executor().timer(wait).await;
        })
    })
}

/// How many frames the tick has shown a sky moving on, for the test bundle's driver.
#[cfg(feature = "drive")]
static SKY_PRESENTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many skies are up, how many frames have moved them on, and what the tick waits on, for
/// the test bundle's driver.
#[cfg(feature = "drive")]
pub fn drive_state(window: &Window, cx: &App) -> serde_json::Value {
    serde_json::json!({
        "skies": cx.try_global::<Grain>().map(|grain| grain.skies),
        "presents": SKY_PRESENTS.load(std::sync::atomic::Ordering::Relaxed),
        "active": window.is_window_active(),
        "visible": window.is_visible(),
        "reduceMotion": cx.reduce_motion(),
    })
}

/// Takes the system's ask for less motion again. macOS changes it in System Settings, so reading
/// it each time the window comes back to the front follows it; Linux also says when it changes.
fn follow_reduced_motion(cx: &mut App) {
    let reduce = test_motion().unwrap_or_else(system_reduces_motion);
    if cx.reduce_motion() != reduce {
        cx.set_reduce_motion(reduce);
        cx.refresh_windows();
    }
}

/// The test bundle's own say on motion over macOS's, from `SIKEMUX_GPUI_DRIVE_MOTION` "on" or
/// "off", so a check can watch the sky move on a machine that asks for less, and hold still.
#[cfg(feature = "drive")]
fn test_motion() -> Option<bool> {
    std::env::var("SIKEMUX_GPUI_DRIVE_MOTION")
        .ok()
        .map(|motion| motion == "off")
}

#[cfg(not(feature = "drive"))]
fn test_motion() -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
fn system_reduces_motion() -> bool {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    // SAFETY: NSWorkspace's shared workspace always exists, and the setting is a plain BOOL.
    unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        msg_send![workspace, accessibilityDisplayShouldReduceMotion]
    }
}

#[cfg(target_os = "linux")]
fn system_reduces_motion() -> bool {
    linux::reduces_motion()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn system_reduces_motion() -> bool {
    false
}

/// The desktop's animation setting, read through the desktop portal: GNOME's "enable
/// animations" switch, or KDE's animation speed set to instant.
#[cfg(target_os = "linux")]
mod linux {
    use std::sync::atomic::{AtomicBool, Ordering};

    use tokio::sync::mpsc;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedValue, Value};

    const GNOME: (&str, &str) = ("org.gnome.desktop.interface", "enable-animations");
    const KDE: (&str, &str) = ("org.kde.kdeglobals.KDE", "AnimationDurationFactor");

    static REDUCES: AtomicBool = AtomicBool::new(false);

    pub(super) fn reduces_motion() -> bool {
        REDUCES.load(Ordering::Relaxed)
    }

    /// Says each time the setting has been read, once at the start and then on every change.
    pub(super) fn watch_motion() -> mpsc::UnboundedReceiver<()> {
        let (changed, changes) = mpsc::unbounded_channel();
        let _ = std::thread::Builder::new()
            .name("reduce-motion".into())
            .spawn(move || {
                let _ = watch(changed);
            });
        changes
    }

    /// KDE's portal hands its settings over as the text they are kept in.
    fn number(value: &Value) -> Option<f64> {
        f64::try_from(value)
            .ok()
            .or_else(|| <&str>::try_from(value).ok()?.trim().parse().ok())
    }

    fn watch(changed: mpsc::UnboundedSender<()>) -> zbus::Result<()> {
        let bus = Connection::session()?;
        let portal = Proxy::new(
            &bus,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )?;
        let mut changes = portal.receive_signal("SettingChanged")?;
        let read = |(namespace, key): (&str, &str)| {
            portal
                .call::<_, _, OwnedValue>("ReadOne", &(namespace, key))
                .ok()
        };
        let mut gnome = read(GNOME);
        let mut kde = read(KDE);
        loop {
            let still = gnome.as_deref().and_then(|on| bool::try_from(on).ok()) == Some(false)
                || kde.as_deref().and_then(number) == Some(0.);
            REDUCES.store(still, Ordering::Relaxed);
            if changed.send(()).is_err() {
                return Ok(());
            }
            let Some(message) = changes.next() else {
                return Ok(());
            };
            let Ok((namespace, key, value)) =
                message.body().deserialize::<(String, String, OwnedValue)>()
            else {
                continue;
            };
            match (namespace.as_str(), key.as_str()) {
                GNOME => gnome = Some(value),
                KDE => kde = Some(value),
                _ => {}
            }
        }
    }
}
