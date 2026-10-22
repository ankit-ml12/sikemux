use gpui_kit::{point, px, size, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};
use sikemux_gpui::{core, icons, theme, workbench, WINDOW_SIZE};

fn main() {
    // Until this app replaces the Mac app, it keeps a core and files of its own, so it never
    // starts or upgrades the core the Mac app uses.
    if std::env::var_os(sikemux_process::vars::CONFIG_DIR).is_none() {
        if let Some(home) = sikemux_process::paths::home() {
            std::env::set_var(
                sikemux_process::vars::CONFIG_DIR,
                home.join(".config").join("sikemux-gpui"),
            );
        }
    }
    let link = core::start(core::Home::default());
    gpui_kit::application()
        .with_assets(icons::Icons)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::install(cx);
            cx.set_reduce_motion(system_reduces_motion());
            let bounds = Bounds::centered(None, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(860.), px(540.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sikemux".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(20.))),
                }),
                ..Default::default()
            };
            let (_window, _) = gpui_kit::open_window(options, cx, |window, cx| {
                if let Err(error) = window.enable_scene_overlay() {
                    eprintln!("sikemux: pages will cover menus, with no overlay plane: {error}");
                }
                cx.new(|cx| workbench::Workbench::new(link, window, cx))
            })
            .expect("window opens");
            #[cfg(feature = "drive")]
            sikemux_gpui::drive::listen(_window, cx);
            cx.activate(true);
        });
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

#[cfg(not(target_os = "macos"))]
fn system_reduces_motion() -> bool {
    false
}
