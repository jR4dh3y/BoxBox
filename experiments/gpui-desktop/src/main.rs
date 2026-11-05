//! BoxBox desktop browse UI in GPUI. An experiment: it talks to a running BoxBox server
//! and reproduces the Svelte desktop/web layout pixel for pixel where GPUI allows.

mod api;
mod assets;
mod clipboard;
mod context_menu;
mod desktop;
mod dialogs;
mod drag_out;
mod format;
mod jobs;
mod notifications;
mod ops;
mod pane;
mod preferences;
mod search;
mod server;
mod sort;
mod sse;
mod theme;
mod trash;
mod ui;
mod undo;
mod watch;
mod workspace;

use std::process::Command;

use gpui::{
    point, px, size, App, AppContext, Application, Bounds, SharedString, TitlebarOptions,
    WindowBounds, WindowDecorations, WindowOptions,
};

use crate::api::{Api, Auth};
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
    // `BOXBOX_URL` attaches to a running server; otherwise start the private desktop server.
    let (api, local) = match std::env::var("BOXBOX_URL")
        .ok()
        .filter(|url| !url.is_empty())
    {
        Some(url) => {
            let auth = std::env::var("BOXBOX_TOKEN")
                .ok()
                .filter(|token| !token.is_empty())
                .map_or(Auth::None, Auth::Bearer);
            (Api::new(&url, auth), None)
        }
        None => {
            let started = server::find_binary()
                .and_then(|binary| server::LocalServer::start(&binary, &server::data_dir()));
            match started {
                Ok(local) => (
                    Api::new(&local.origin, Auth::Desktop(local.token.clone())),
                    Some(local),
                ),
                Err(error) => {
                    eprintln!("BoxBox: {error:#}");
                    std::process::exit(1);
                }
            }
        }
    };
    let initial_path = std::env::args().nth(1).unwrap_or_default();
    let fonts = Fonts {
        sans: fontconfig_family("sans-serif", ".SystemUIFont"),
        mono: fontconfig_family("monospace", "monospace"),
    };

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            workspace::bind_keys(cx);
            ui::text_input::bind_keys(cx);
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
                |window, cx| cx.new(|cx| Workspace::new(api, fonts, initial_path, window, cx)),
            )
            .expect("failed to open the BoxBox window");
            cx.activate(true);
            // Closing the last window stops the private server, like the Electrobun app.
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let local = std::sync::Mutex::new(local);
            cx.on_app_quit(move |_| {
                drop(local.lock().ok().and_then(|mut local| local.take()));
                async {}
            })
            .detach();
        });
}
