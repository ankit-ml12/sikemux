mod agents;
mod core;
mod home;
mod icons;
mod rail;
mod tabs;
mod theme;
mod ui;
mod workbench;

use gpui_kit::{point, px, size, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

fn main() {
    let link = core::start(core::socket());
    gpui_kit::application()
        .with_assets(icons::Icons)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::load_fonts(cx);
            let bounds = Bounds::centered(None, size(px(1340.), px(880.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(860.), px(540.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Sikemux".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(19.))),
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| workbench::Workbench::new(link, window, cx))
            })
            .expect("window opens");
            cx.activate(true);
        });
}
