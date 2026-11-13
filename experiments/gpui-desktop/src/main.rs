//! Native BoxBox desktop with direct local filesystem access and remote HTTP connections.

mod api;
mod assets;
mod components;
mod gallery;
mod drag_out;
mod format;
mod local;
mod media;
mod pane;
mod persist;
mod search;
mod sort;
mod theme;
mod ui;
mod workspace;
mod settings_state;

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
    // GPUI 0.2.2 can retain an old frame on Niri's native Wayland backend.
    // Prefer the verified XWayland path when available; keep an explicit Wayland opt-in.
    #[cfg(target_os = "linux")]
    if (std::env::var_os("NIRI_SOCKET").is_some()
        || std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| desktop.split(':').any(|name| name.eq_ignore_ascii_case("niri"))))
        && std::env::var_os("DISPLAY").is_some_and(|display| !display.is_empty())
        && std::env::var_os("BOXBOX_NATIVE_WAYLAND").is_none()
    {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let token = std::env::var("BOXBOX_TOKEN")
        .ok()
        .filter(|token| !token.is_empty());
    let api = match std::env::var("BOXBOX_URL") {
        Ok(base) => Ok(Api::new(&base, token)),
        Err(std::env::VarError::NotPresent) => match std::env::var_os("BOXBOX_ROOT") {
            Some(root) => Api::local_with_root(root.into()),
            None => Api::local(),
        },
        Err(std::env::VarError::NotUnicode(_)) => {
            eprintln!("BOXBOX_URL must be valid UTF-8");
            std::process::exit(1);
        }
    }
    .unwrap_or_else(|error| {
        eprintln!("Could not open local files: {error:#}");
        std::process::exit(1);
    });
    let initial_path = std::env::args().nth(1).unwrap_or_default();
    let fonts = Fonts {
        sans: fontconfig_family("sans-serif", ".SystemUIFont"),
        mono: fontconfig_family("monospace", "monospace"),
    };

    if std::env::args().nth(1).as_deref() == Some("--gallery") {
        run_gallery(fonts.sans);
        return;
    }

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
                        Workspace::new(api, fonts, initial_path, window, cx)
                    })
                },
            )
            .expect("failed to open the BoxBox window");
            cx.activate(true);
        });
}

/// The component gallery, at the manifest's size, for comparison with the web app.
fn run_gallery(font: SharedString) {
    let manifest = gallery::manifest();
    Application::new().with_assets(assets::Assets).run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(manifest.width), px(manifest.height)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some("boxbox-gallery".into()),
                window_decorations: Some(WindowDecorations::Server),
                ..Default::default()
            },
            |_, cx| cx.new(|_| gallery::Gallery::new(font)),
        )
        .expect("failed to open the gallery window");
        cx.activate(true);
    });
}
