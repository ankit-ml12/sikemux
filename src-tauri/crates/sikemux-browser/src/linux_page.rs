//! The WebKitGTK side of a tab's page: its scripts, its pictures, its history and what it
//! tells the app. Everything here runs on the GTK thread that owns the page.

use std::sync::{Arc, OnceLock};

use gtk::prelude::*;
use gtk::{cairo, gdk, gdk_pixbuf, glib};
use javascriptcore::ValueExt;
use webkit2gtk::{SettingsExt, SnapshotOptions, SnapshotRegion, WebView, WebViewExt};

use crate::{BrowserShortcut, TabEvents, TabStall};

/// The world the agents' helpers run in: it shares the page's DOM but none of its
/// JavaScript, so the page cannot tamper with what runs there.
const HELPER_WORLD: &str = "sikemux";
const JPEG_QUALITY: &str = "82";
const STILL_QUALITY: &str = "80";

type Shortcut = Box<dyn Fn(BrowserShortcut) + Send + Sync>;

static SHORTCUT: OnceLock<Shortcut> = OnceLock::new();

#[derive(Clone, Copy)]
pub enum World {
    Page,
    Helper,
}

/// Wires a page WebKitGTK just made to the tab that holds it.
pub(crate) fn attach(webview: &WebView, agent_id: &str, tab_id: &str, events: Arc<dyn TabEvents>) {
    let crashed = events.clone();
    webview.connect_web_process_terminated(move |_, _| crashed.health(Some(TabStall::Crashed)));
    let hung = events;
    webview.connect_is_web_process_responsive_notify(move |view| {
        hung.health((!view.is_web_process_responsive()).then_some(TabStall::Unresponsive));
    });
    let (agent_id, tab_id) = (agent_id.to_owned(), tab_id.to_owned());
    webview.connect_key_press_event(move |_, event| {
        let Some(shortcut) = chord_of(event) else {
            return glib::Propagation::Proceed;
        };
        if let Some(send) = SHORTCUT.get() {
            send(BrowserShortcut {
                agent_id: agent_id.clone(),
                tab_id: tab_id.clone(),
                ..shortcut
            });
        }
        glib::Propagation::Stop
    });
}

/// Where command chords pressed in a page go: the app's keymap, not the page.
pub(crate) fn install_shortcuts(send: impl Fn(BrowserShortcut) + Send + Sync + 'static) {
    let _ = SHORTCUT.set(Box::new(send));
}

/// A Control chord is the app's, apart from the editing set every text field keeps.
fn chord_of(event: &gdk::EventKey) -> Option<BrowserShortcut> {
    let state = event.state();
    if !state.contains(gdk::ModifierType::CONTROL_MASK)
        || state.intersects(gdk::ModifierType::MOD1_MASK | gdk::ModifierType::SUPER_MASK)
    {
        return None;
    }
    let key = event
        .keyval()
        .to_lower()
        .to_unicode()
        .filter(char::is_ascii_graphic)?;
    if matches!(key, 'a' | 'c' | 'v' | 'x' | 'z' | 'y') {
        return None;
    }
    Some(BrowserShortcut {
        agent_id: String::new(),
        tab_id: String::new(),
        key: key.to_string(),
        code: dom_code(key),
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        alt: false,
    })
}

fn dom_code(key: char) -> String {
    match key {
        'a'..='z' => format!("Key{}", key.to_ascii_uppercase()),
        '0'..='9' => format!("Digit{key}"),
        '[' => "BracketLeft".into(),
        ']' => "BracketRight".into(),
        ',' => "Comma".into(),
        '.' => "Period".into(),
        '/' => "Slash".into(),
        ';' => "Semicolon".into(),
        '\'' => "Quote".into(),
        '\\' => "Backslash".into(),
        '-' => "Minus".into(),
        '=' => "Equal".into(),
        '`' => "Backquote".into(),
        _ => String::new(),
    }
}

/// Runs `body` as the body of an async function in the page, awaiting any promise it
/// returns. The body must return a string.
pub fn call_async(
    webview: &WebView,
    body: &str,
    world: World,
    done: Box<dyn FnOnce(Result<String, String>) + Send>,
) {
    let world = match world {
        World::Page => None,
        World::Helper => Some(HELPER_WORLD),
    };
    webview.call_async_javascript_function(
        body,
        None,
        world,
        None,
        None::<&gtk::gio::Cancellable>,
        move |answer| {
            done(match answer {
                Ok(value) if value.is_string() => Ok(value.to_str().into()),
                Ok(_) => Err("the script returned nothing readable".into()),
                Err(error) => Err(error.message().to_owned()),
            })
        },
    );
}

/// The visible part of the page, or only `area` of it in CSS pixels.
pub fn snapshot_jpeg(
    webview: &WebView,
    area: Option<(f64, f64, f64, f64)>,
    done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
) {
    let zoom = webview.zoom_level().max(0.01);
    snapshot(webview, SnapshotRegion::Visible, move |picture| {
        let crop = area.map(|(left, top, width, height)| {
            (left * zoom, top * zoom, width * zoom, height * zoom)
        });
        encode(
            picture.and_then(|(pixbuf, scale)| cropped(&pixbuf, scale, crop)),
            JPEG_QUALITY,
            done,
        );
    });
}

/// The whole page down to `height` CSS pixels, at most `most` of them.
pub fn full_page_jpeg(
    webview: &WebView,
    height: f64,
    most: f64,
    done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
) {
    let zoom = webview.zoom_level().max(0.01);
    snapshot(webview, SnapshotRegion::FullDocument, move |picture| {
        let crop = (height > most).then_some((0.0, 0.0, f64::MAX, most * zoom));
        encode(
            picture.and_then(|(pixbuf, scale)| cropped(&pixbuf, scale, crop)),
            JPEG_QUALITY,
            done,
        );
    });
}

/// The page as it stands, for the app to slide in its place while the stage moves.
pub fn still_jpeg(webview: &WebView, done: Box<dyn FnOnce(Option<Vec<u8>>) + Send>) {
    snapshot(webview, SnapshotRegion::Visible, move |picture| {
        encode(
            picture.map(|(pixbuf, _)| pixbuf),
            STILL_QUALITY,
            Box::new(move |jpeg: Result<Vec<u8>, String>| done(jpeg.ok())),
        );
    });
}

/// The page's picture and how many image pixels make one of its own.
fn snapshot(
    webview: &WebView,
    region: SnapshotRegion,
    then: impl FnOnce(Result<(gdk_pixbuf::Pixbuf, f64), String>) + 'static,
) {
    webview.snapshot(
        region,
        SnapshotOptions::NONE,
        None::<&gtk::gio::Cancellable>,
        move |surface| {
            then(
                surface
                    .map_err(|error| error.message().to_owned())
                    .and_then(|surface| {
                        let picture = cairo::ImageSurface::try_from(surface)
                            .map_err(|_| "the page could not be captured".to_string())?;
                        let (scale, _) = picture.device_scale();
                        gdk::pixbuf_get_from_surface(
                            &picture,
                            0,
                            0,
                            picture.width(),
                            picture.height(),
                        )
                        .map(|pixbuf| (pixbuf, scale.max(1.0)))
                        .ok_or_else(|| "the page could not be captured".to_string())
                    }),
            )
        },
    );
}

/// `crop` is in the page's own pixels, before the screen's scale.
fn cropped(
    pixbuf: &gdk_pixbuf::Pixbuf,
    scale: f64,
    crop: Option<(f64, f64, f64, f64)>,
) -> Result<gdk_pixbuf::Pixbuf, String> {
    let Some((left, top, width, height)) = crop else {
        return Ok(pixbuf.clone());
    };
    let (most_x, most_y) = (pixbuf.width(), pixbuf.height());
    let at = |value: f64, most: i32| ((value * scale).round() as i32).clamp(0, most);
    let (x, y) = (at(left, most_x), at(top, most_y));
    let (width, height) = (
        at(width, most_x - x).max(1).min(most_x - x),
        at(height, most_y - y).max(1).min(most_y - y),
    );
    if width <= 0 || height <= 0 {
        return Err("that area is outside the page".into());
    }
    Ok(pixbuf.new_subpixbuf(x, y, width, height))
}

/// Compressing the picture takes milliseconds, so it happens off the GTK thread.
fn encode(
    picture: Result<gdk_pixbuf::Pixbuf, String>,
    quality: &'static str,
    done: Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>,
) {
    // A cut-out shares its parent's pixels, so it is copied out whole before its bytes go.
    let pixbuf = match picture.map(|pixbuf| pixbuf.copy()) {
        Ok(Some(pixbuf)) => pixbuf,
        Ok(None) => return done(Err("could not encode the page image".into())),
        Err(error) => return done(Err(error)),
    };
    let (bytes, alpha, width, height, stride) = (
        pixbuf.read_pixel_bytes(),
        pixbuf.has_alpha(),
        pixbuf.width(),
        pixbuf.height(),
        pixbuf.rowstride(),
    );
    std::thread::spawn(move || {
        let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
            &bytes,
            gdk_pixbuf::Colorspace::Rgb,
            alpha,
            8,
            width,
            height,
            stride,
        );
        done(
            pixbuf
                .save_to_bufferv("jpeg", &[("quality", quality)])
                .map_err(|_| "could not encode the page image".to_string()),
        )
    });
}

pub fn history(webview: &WebView, delta: i32) {
    if delta < 0 {
        webview.go_back();
    } else {
        webview.go_forward();
    }
}

pub fn introduce_as(webview: &WebView, agent: &str) {
    if let Some(settings) = WebViewExt::settings(webview) {
        settings.set_user_agent(Some(agent));
    }
}
