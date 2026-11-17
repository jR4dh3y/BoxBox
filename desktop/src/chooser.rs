//! BoxBox as the system's Open and Save dialog. D-Bus starts `boxbox-files
//! --portal` when an app asks; that process shows one window per request and
//! ends when the last one closes. It never opens the file manager window and
//! never writes the file manager's settings.

pub mod portal;
pub mod request;
pub mod routing;
pub mod view;

use std::collections::HashMap;
use std::time::Duration;

use gpui::{px, size, App, AppContext as _, Application, Bounds, SharedString, WindowBounds, WindowDecorations, WindowHandle, WindowOptions};

use portal::Event;
use view::Chooser;

/// With no window for this long, the process ends; D-Bus starts it again.
const IDLE: Duration = Duration::from_secs(60);

pub fn run(sans: SharedString) {
    let (events, incoming) = async_channel::unbounded();
    let connection = match portal::serve(events) {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("boxbox-files --portal: {error:#}");
            std::process::exit(1);
        }
    };
    Application::new().with_assets(crate::assets::Assets).run(move |cx: &mut App| {
        cx.set_global(crate::components::UiFont(sans));
        crate::components::text_field_bind_keys(cx);
        let idle = incoming.clone();
        cx.spawn(async move |cx| {
            let mut windows: HashMap<u64, WindowHandle<Chooser>> = HashMap::new();
            while let Ok(event) = incoming.recv().await {
                windows.retain(|_, handle| handle.update(cx, |_, _, _| ()).is_ok());
                match event {
                    Event::Open { id, request, reply } => {
                        if let Ok(Some(handle)) = cx.update(|cx| open_window(id, *request, reply, cx)) {
                            windows.insert(id, handle);
                        }
                    }
                    Event::Close(id) => {
                        if let Some(handle) = windows.remove(&id) {
                            let _ = handle.update(cx, |_, window, _| window.remove_window());
                        }
                    }
                }
            }
        })
        .detach();
        // Started for a request that never came, or that was refused: end anyway.
        cx.spawn(async move |cx| loop {
            cx.background_executor().timer(IDLE).await;
            if idle.is_empty() && cx.update(|cx| cx.windows().is_empty()).unwrap_or(true) {
                let _ = cx.update(|cx| cx.quit());
                break;
            }
        })
        .detach();
    });
    // The last reply may still be on its way out when the last window closes.
    std::thread::sleep(Duration::from_millis(200));
    drop(connection);
}

fn open_window(id: u64, request: request::Request, reply: async_channel::Sender<request::Response>, cx: &mut App) -> Option<WindowHandle<Chooser>> {
    let title = Chooser::title(&request);
    let bounds = Bounds::centered(None, size(px(960.), px(620.)), cx);
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(gpui::TitlebarOptions { title: Some(title.into()), appears_transparent: false, traffic_light_position: None }),
            app_id: Some("boxbox-chooser".into()),
            window_min_size: Some(size(px(560.), px(360.))),
            window_decorations: Some(WindowDecorations::Server),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Chooser::new(request, reply, window, cx)),
    );
    match handle {
        Ok(handle) => {
            cx.activate(true);
            Some(handle)
        }
        Err(error) => {
            log::warn!("Could not open file chooser {id}: {error:#}");
            None
        }
    }
}
