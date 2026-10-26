mod app;
mod assets;
mod i18n;
mod services;

use app::BandixApp;
use assets::Assets;
use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.set_app_identity("bandix-gui", "Bandix");
            app::initialize_theme(cx);

            let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
            let options = WindowOptions {
                app_id: Some("bandix-gui".into()),
                icon: Some(std::sync::Arc::new(
                    image::load_from_memory(assets::APP_ICON_BYTES)
                        .expect("embedded Bandix icon must be a valid PNG")
                        .into_rgba8(),
                )),
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
                app.update(cx, |app, cx| app.search_default_gateway(window, cx));
                app
            })
            .expect("failed to open Bandix window");
            cx.activate(true);
        });
}
