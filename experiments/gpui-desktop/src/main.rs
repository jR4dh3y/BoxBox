//! Native BoxBox desktop with direct local filesystem access and remote HTTP connections.

mod api;
mod assets;
mod components;
mod devices;
mod filemanager1;
mod gallery;
mod instance;
mod drag_out;
mod format;
mod local;
mod launch;
mod media;
mod open_with;
mod os_clipboard;
mod pane;
mod progress;
#[cfg(target_os = "linux")]
mod render_gpu;
mod persist;
mod profile;
mod search;
mod sort;
mod theme;
mod thumbnail_queue;
mod ui;
mod workspace;
mod settings_state;
mod system_fonts;

use gpui::{
    point, px, size, App, AppContext, Application, Bounds, SharedString, TitlebarOptions,
    WindowBounds, WindowDecorations, WindowOptions,
};

use crate::api::Api;
use crate::workspace::Workspace;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--profile-summary") {
        let Some(trace) = std::env::args().nth(2) else {
            eprintln!("usage: boxbox-gpui --profile-summary <trace.json>");
            std::process::exit(2);
        };
        if let Err(error) = profile::summary::run(&trace) {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    profile::init();
    // Overlaps GPUI's own start-up; the window needs the result only once it opens.
    let fonts = std::thread::Builder::new().name("fonts".into()).spawn(system_fonts::resolve).expect("cannot start a thread");
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    #[cfg(target_os = "linux")]
    {
        let _span = profile::span("follow compositor GPU");
        render_gpu::follow_compositor();
    }
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
    if std::env::args().nth(1).as_deref() == Some("--gallery") {
        run_gallery(fonts.join().expect("font lookup panicked").sans);
        return;
    }
    // A second launch opens its location in the running window instead.
    let claimed = {
        let _span = profile::span("single-instance claim");
        instance::claim(&initial_path)
    };
    let launches = match claimed {
        Ok(instance::Instance::Forwarded) => return,
        Ok(instance::Instance::Primary(launches, sender)) => {
            // "Show in folder" from other apps, when no other file manager answers it.
            std::thread::Builder::new()
                .name("filemanager1".into())
                .spawn(move || match filemanager1::serve(sender) {
                    Ok(Some(_connection)) => loop {
                        std::thread::park();
                    },
                    Ok(None) => log::info!("Another file manager answers org.freedesktop.FileManager1"),
                    Err(error) => log::warn!("Could not serve org.freedesktop.FileManager1: {error:#}"),
                })
                .ok();
            Some(launches)
        }
        Err(error) => {
            log::warn!("Running without single-instance handoff: {error:#}");
            None
        }
    };

    let platform_init = profile::span("GPUI platform init");
    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            drop(platform_init);
            let _span = profile::span("open window");
            let fonts = fonts.join().expect("font lookup panicked");
            workspace::bind_keys(cx);
            cx.set_global(components::UiFont(fonts.sans.clone()));
            components::text_field_bind_keys(cx);
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
                    drag_out::init(window, cx);
                    cx.new(|cx| {
                        let _span = profile::span("Workspace::new");
                        let mut workspace = Workspace::new(api, fonts, initial_path, window, cx);
                        if let Some(launches) = launches {
                            workspace.receive_launches(launches, window, cx);
                        }
                        workspace
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
