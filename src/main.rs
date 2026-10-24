mod app;
mod assets;
mod services;

use app::BandixApp;
use assets::Assets;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

fn main() {
    gpui_kit::application().with_assets(Assets).run(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);

        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Bandix".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            window_min_size: Some(size(px(1024.0), px(680.0))),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            let app = cx.new(|cx| BandixApp::new(window, cx));
            app.update(cx, |app, cx| app.search_default_gateway(cx));
            app
        })
        .expect("failed to open Bandix window");
        cx.activate(true);
    });
}
