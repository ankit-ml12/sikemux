//! A tab's page on Linux: WebKitGTK through wry, in an X11 window of its own inside the app's
//! window. GTK runs on a thread of its own, which every call into a page goes to.
//!
//! Each page sits in a clip window of ours, as wide as the part of the page that shows, so a
//! page reaching past the stage's edge is cut there. The app's own drawing cuts the clip
//! window where its menus show over the page.

use std::num::NonZeroU32;
use std::os::raw::c_ulong;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use gtk::glib;
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle,
};
use sikemux_process::sync::Locked;
use webkit2gtk::WebView;
use wry::WebViewExtUnix;
use x11::xlib::{self, Display};

use crate::tab::{ParentView, TabSpec};
use crate::{PageLoad, TabEvents};

/// X11 places windows with 16-bit coordinates, so a page parked any further out comes back
/// into sight.
const PARKED: f64 = -30_000.0;

#[derive(Clone)]
pub struct Tab {
    id: Arc<str>,
    view: Arc<Held>,
}

/// The page's view and its clip window, dropped on the GTK thread whichever thread lets go
/// last.
struct Held(Mutex<Option<Page>>);

/// The page goes before the window it sits in.
struct Page {
    view: wry::WebView,
    clip: Clip,
}

/// A window of ours between the app's window and the page's.
struct Clip {
    display: *mut Display,
    window: c_ulong,
}

impl Clip {
    /// Adds a clip window to `parent`, out of sight. It draws nothing of its own.
    fn new(parent: c_ulong) -> Result<Self, String> {
        let display = crate::linux_page::xdisplay().ok_or("GTK has no X11 display")?;
        // SAFETY: GTK's live X11 display on the GTK thread, and `parent` is the app's live window.
        let window = unsafe {
            let window = xlib::XCreateSimpleWindow(
                display,
                parent,
                PARKED as i32,
                PARKED as i32,
                1,
                1,
                0,
                0,
                0,
            );
            xlib::XSetWindowBackgroundPixmap(display, window, 0);
            xlib::XFlush(display);
            window
        };
        Ok(Self { display, window })
    }

    fn place(&self, x: f64, y: f64, width: f64, height: f64) {
        // SAFETY: our live window on GTK's display, on the GTK thread.
        unsafe {
            xlib::XMoveResizeWindow(
                self.display,
                self.window,
                x as i32,
                y as i32,
                width.max(1.) as u32,
                height.max(1.) as u32,
            );
            xlib::XFlush(self.display);
        }
    }

    fn show(&self, shown: bool) {
        // SAFETY: as above.
        unsafe {
            if shown {
                xlib::XMapWindow(self.display, self.window);
            } else {
                xlib::XUnmapWindow(self.display, self.window);
            }
            xlib::XFlush(self.display);
        }
    }
}

impl Drop for Clip {
    fn drop(&mut self) {
        // SAFETY: as above; nothing uses the window after this.
        unsafe {
            xlib::XDestroyWindow(self.display, self.window);
            xlib::XFlush(self.display);
        }
    }
}

// SAFETY: the view inside is only touched on the GTK thread: through `on_gtk`, and in
// `Drop`, which sends it there.
unsafe impl Send for Held {}
unsafe impl Sync for Held {}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(page) = self.0.locked().take() {
            let held = Held(Mutex::new(Some(page)));
            on_gtk(move || drop(held));
        }
    }
}

/// Starts the thread GTK runs on, once, and says whether GTK could start.
fn gtk_thread() -> Result<(), String> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| {
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("gtk".into())
                .spawn(move || {
                    let context = glib::MainContext::default();
                    let Ok(_owned) = context.acquire() else {
                        let _ = sender.send(Err("GTK's main context is taken".to_string()));
                        return;
                    };
                    gtk::gdk::set_allowed_backends("x11");
                    let started = gtk::init().map_err(|error| error.to_string());
                    let ok = started.is_ok();
                    let _ = sender.send(started);
                    if ok {
                        glib::MainLoop::new(Some(&context), false).run();
                    }
                })
                .map_err(|error| error.to_string())?;
            receiver
                .recv()
                .map_err(|_| "GTK did not start".to_string())?
        })
        .clone()
}

pub(crate) fn on_gtk(work: impl FnOnce() + Send + 'static) {
    let context = glib::MainContext::default();
    if context.is_owner() {
        work();
    } else {
        context.invoke(work);
    }
}

/// A window by its X11 id, as wry takes a parent.
struct X11Parent(c_ulong);

impl HasWindowHandle for X11Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = RawWindowHandle::Xlib(XlibWindowHandle::new(self.0));
        // SAFETY: the window outlives the call that borrows it to make a page.
        Ok(unsafe { WindowHandle::borrow_raw(handle) })
    }
}

impl Tab {
    /// Adds the tab's page to `parent`, hidden and parked off screen, and starts loading
    /// `spec.url`. Waits for the GTK thread.
    pub fn open(
        parent: ParentView,
        spec: TabSpec<'_>,
        events: Arc<dyn TabEvents>,
    ) -> Result<Tab, String> {
        gtk_thread()?;
        let id: Arc<str> = spec.tab_id.into();
        let (agent_id, tab_id) = (spec.agent_id.to_owned(), id.to_string());
        let (url, user_agent, size) = (spec.url.to_owned(), spec.user_agent.to_owned(), spec.size);
        let window = NonZeroU32::new(parent.view as usize as u32)
            .ok_or("the window has no X11 id")?
            .get();
        let make = move || -> Result<Held, String> {
            let (load, title, popup, started, finished) = (
                events.clone(),
                events.clone(),
                events.clone(),
                events.clone(),
                events.clone(),
            );
            let attached = events.clone();
            let clip = Clip::new(c_ulong::from(window))?;
            clip.place(PARKED, PARKED, size.0, size.1);
            let view = wry::WebViewBuilder::new()
                .with_url(&url)
                .with_user_agent(&user_agent)
                .with_initialization_script(crate::RECORDER_SCRIPT)
                .with_focused(false)
                .with_visible(false)
                .with_hotkeys_zoom(true)
                .with_bounds(wry::Rect {
                    position: wry::dpi::LogicalPosition::new(0., 0.).into(),
                    size: wry::dpi::LogicalSize::new(size.0, size.1).into(),
                })
                .with_navigation_handler(move |url| events.may_load(&url))
                .with_on_page_load_handler(move |event, url| {
                    load.loaded(PageLoad {
                        started: matches!(event, wry::PageLoadEvent::Started),
                        url,
                        can_go_back: false,
                        can_go_forward: false,
                    })
                })
                .with_document_title_changed_handler(move |name| title.titled(name))
                .with_new_window_req_handler(move |url, _| {
                    popup.new_window(url);
                    wry::NewWindowResponse::Deny
                })
                .with_download_started_handler(move |url, path: &mut PathBuf| {
                    match started.download_started(url, path) {
                        Some(chosen) => {
                            *path = chosen;
                            true
                        }
                        None => false,
                    }
                })
                .with_download_completed_handler(move |url, _, saved| {
                    finished.download_finished(url, saved)
                })
                .build_as_child(&X11Parent(clip.window))
                .map_err(|error| error.to_string())?;
            crate::linux_page::attach(&view.webview(), &agent_id, &tab_id, attached);
            Ok(Held(Mutex::new(Some(Page { view, clip }))))
        };
        let (sender, receiver) = std::sync::mpsc::channel();
        on_gtk(move || {
            let _ = sender.send(make());
        });
        let held = receiver
            .recv()
            .map_err(|_| "the window cannot take a browser tab".to_string())??;
        Ok(Tab {
            id,
            view: Arc::new(held),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    fn with(&self, act: impl FnOnce(&wry::WebView) + Send + 'static) {
        self.with_page(move |page| act(&page.view));
    }

    fn with_page(&self, act: impl FnOnce(&Page) + Send + 'static) {
        let view = self.view.clone();
        on_gtk(move || {
            if let Some(page) = view.0.locked().as_ref() {
                act(page);
            }
        });
    }

    /// Runs `act` on the page on the GTK thread, at once when called there.
    pub fn run(&self, act: impl FnOnce(&WebView) + Send + 'static) {
        self.with(move |view| act(&view.webview()));
    }

    /// Runs `act` on the page on the GTK thread and hands back its answer.
    pub async fn ask<T: Send + 'static>(
        &self,
        act: impl FnOnce(&WebView) -> T + Send + 'static,
    ) -> Result<T, String> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.run(move |webview| {
            let _ = sender.send(act(webview));
        });
        receiver.await.map_err(|_| "the tab went away".to_string())
    }

    pub fn show(&self) {
        self.with_page(|page| {
            page.clip.show(true);
            let _ = page.view.set_visible(true);
        });
    }

    pub fn hide(&self) {
        self.with_page(|page| {
            let _ = page.view.set_visible(false);
            page.clip.show(false);
        });
    }

    /// Takes the page out of its window.
    pub fn close(&self) {
        let view = self.view.clone();
        on_gtk(move || drop(view.0.locked().take()));
    }

    pub fn set_zoom(&self, zoom: f64) {
        self.with(move |webview| {
            let _ = webview.zoom(zoom);
        });
    }

    pub fn reload(&self) {
        self.with(|webview| {
            let _ = webview.reload();
        });
    }

    pub fn navigate(&self, url: &str) {
        let url = url.to_owned();
        self.with(move |webview| {
            let _ = webview.load_url(&url);
        });
    }

    /// Puts the page at `frame` (x, y, width, height), top-down in the window, with `sides`
    /// (left, right) of it cut off.
    pub fn set_bounds(&self, frame: (f64, f64, f64, f64), sides: (f64, f64)) {
        let (x, y, width, height) = frame;
        let (x, y) = (x.max(PARKED), y.max(PARKED));
        let (left, right) = sides;
        self.with_page(move |page| {
            page.clip.place(x + left, y, width - left - right, height);
            let _ = page.view.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(-left, 0.).into(),
                size: wry::dpi::LogicalSize::new(width, height).into(),
            });
        });
    }

    /// Runs `script` in the page and hands back its value as JSON, or an empty string when it
    /// has none.
    pub async fn eval(&self, script: &str) -> Result<String, String> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let script = script.to_owned();
        self.with(move |webview| {
            let sender = Mutex::new(Some(sender));
            let _ = webview.evaluate_script_with_callback(&script, move |json| {
                if let Some(sender) = sender.locked().take() {
                    let _ = sender.send(json);
                }
            });
        });
        receiver.await.map_err(|_| "the tab went away".to_string())
    }
}
