//! BoxBox desktop browse UI in GPUI. An experiment: it talks to a running BoxBox server
//! and reproduces the Svelte desktop/web layout pixel for pixel where GPUI allows.

mod api;
mod assets;
mod format;
mod pane;
mod sort;
mod theme;
mod ui;
mod workspace;

use std::process::Command;

use gpui::{
    point, px, size, App, AppContext, Application, Bounds, SharedString, TitlebarOptions,
    WindowBounds, WindowDecorations, WindowOptions,
};

use crate::api::Api;
use crate::workspace::{Fonts, Workspace};

/// The browser resolves the CSS `sans-serif`/`monospace` stacks through fontconfig; do the same.
fn fontconfig_family(pattern: &str, fallback: &str) -> SharedString {
    Command::new("fc-match")
        .args(["-f", "%{family[0]}", pattern])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .filter(|family| !family.trim().is_empty())
        .map(|family| SharedString::from(family.trim().to_string()))
        .unwrap_or_else(|| SharedString::from(fallback.to_string()))
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let base = std::env::var("BOXBOX_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let token = std::env::var("BOXBOX_TOKEN")
        .ok()
        .filter(|token| !token.is_empty());
    let initial_path = std::env::args().nth(1).unwrap_or_default();
    let fonts = Fonts {
        sans: fontconfig_family("sans-serif", ".SystemUIFont"),
        mono: fontconfig_family("monospace", "monospace"),
    };

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            workspace::bind_keys(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("BoxBox".into()),
                        appears_transparent: false,
                        traffic_light_position: Some(point(px(9.), px(9.))),
                    }),
                    app_id: Some("boxbox-gpui".into()),
                    window_min_size: Some(size(px(640.), px(400.))),
                    window_decorations: Some(WindowDecorations::Server),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        Workspace::new(Api::new(&base, token), fonts, initial_path, window, cx)
                    })
                },
            )
            .expect("failed to open the BoxBox window");
            cx.activate(true);
        });
}
