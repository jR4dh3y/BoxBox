use super::*;
use gpui::{TestAppContext, VisualTestContext, Modifiers, MouseButton};

struct ProbeServer(std::process::Child);

/// Points the settings files at a temporary folder. Tests that render a workspace
/// share the environment, so they take turns.
pub(super) struct TestConfig {
    previous: Option<std::ffi::OsString>,
    path: std::path::PathBuf,
    _turn: std::sync::MutexGuard<'static, ()>,
}

impl TestConfig {
    pub(super) fn new(path: std::path::PathBuf) -> Self {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let turn = TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", &path);
        Self { previous, path, _turn: turn }
    }
}

impl Drop for TestConfig {
    fn drop(&mut self) {
        match &self.previous {
            Some(previous) => std::env::set_var("XDG_CONFIG_HOME", previous),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Drop for ProbeServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// The same rendered workflow can run against a freshly built Go server instead of local Rust.
fn fixture_api(fixture: &std::path::Path) -> (Api, Option<ProbeServer>) {
    let Some(binary) = std::env::var_os("BOXBOX_PARITY_SERVER") else {
        return (Api::local_with_root(fixture.to_path_buf()).unwrap(), None);
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_dir = fixture.with_extension("server");
    fs::create_dir_all(&server_dir).unwrap();
    let config = server_dir.join("config.yaml");
    fs::write(&config, format!("host: 127.0.0.1\nport: {port}\ndata_dir: {}/data\nrate_limit_rps: 1000\nmount_points:\n  - name: home\n    path: {}\n", server_dir.display(), fixture.display())).unwrap();
    drop(listener);
    let log = fs::File::create(server_dir.join("server.log")).unwrap();
    let mut server = ProbeServer(std::process::Command::new(binary).arg("-dev").arg("-config").arg(config)
        .stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap());
    let api = Api::new(&format!("http://127.0.0.1:{port}"), None);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while api.list("home").is_err() {
        assert!(server.0.try_wait().unwrap().is_none(), "probe server exited; see {}", server_dir.display());
        assert!(std::time::Instant::now() < deadline, "probe server did not start");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    (api, Some(server))
}

pub(super) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} must be rendered"));
    cx.simulate_click(bounds.center(), Modifiers::default());
}

// Exercise the real workspace, key routing and filesystem rather than isolated helpers.
#[gpui::test]
fn desktop_workflow(cx: &mut TestAppContext) {
    let fixture = std::env::temp_dir().join(format!("boxbox-parity-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _config = TestConfig::new(fixture.with_extension("config"));
    fs::create_dir_all(&fixture).unwrap();
    let (api, server) = fixture_api(&fixture);
    let local = api.is_local();
    cx.update(bind_keys);
    cx.update(crate::components::text_field_bind_keys);
    let (view, cx) = cx.add_window_view(|window, cx| Workspace::new(api, Fonts {
        sans: "sans-serif".into(), mono: "monospace".into(),
    }, "home".into(), window, cx));
    cx.update(|_, app| view.update(app, |workspace, _| {
        workspace.preferences = Preferences::default();
        workspace.desktop_settings = DesktopSettings::default();
    }));
    cx.simulate_keystrokes("ctrl-shift-n p o r t enter");
    cx.run_until_parked();
    assert!(fixture.join("port").is_dir(), "create-folder input must reach the filesystem");
    let folder_row = cx.debug_bounds("row-0-0").unwrap();
    cx.simulate_mouse_down(folder_row.center(), MouseButton::Right, Modifiers::default());
    click(cx, "context-favorite");
    view.read_with(cx, |workspace, _| assert!(workspace.favorites.entries.iter().any(|favorite| favorite.path == "home/port")));
    cx.simulate_mouse_down(folder_row.center(), MouseButton::Right, Modifiers::default());
    click(cx, "context-favorite");
    view.read_with(cx, |workspace, _| assert!(!workspace.favorites.entries.iter().any(|favorite| favorite.path == "home/port")));
    cx.simulate_keystrokes("ctrl-l h o m e / p o r t enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/port"));
    cx.simulate_keystrokes("ctrl-shift-e n o t e . t x t enter");
    cx.run_until_parked();
    assert!(fixture.join("port/note.txt").is_file());
    if local {
        fs::write(fixture.join("port/outside.txt"), "created outside BoxBox").unwrap();
        cx.update(|_, app| view.update(app, |workspace, cx| workspace.poll_directory_changes(cx)));
        cx.run_until_parked();
        view.read_with(cx, |workspace, _| assert!(workspace.pane().items.iter().any(|item| item.name == "outside.txt"), "external creation must refresh the listing"));
        fs::remove_file(fixture.join("port/outside.txt")).unwrap();
        cx.update(|_, app| view.update(app, |workspace, cx| workspace.poll_directory_changes(cx)));
        cx.run_until_parked();
        view.read_with(cx, |workspace, _| assert!(!workspace.pane().items.iter().any(|item| item.name == "outside.txt"), "external removal must refresh the listing"));
    }
    // Previews are read-only; editing belongs to the file's own app.
    fs::write(fixture.join("port/note.txt"), "hello").unwrap();
    let row = cx.debug_bounds("row-0-0").expect("created file must render a row");
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    assert!(cx.debug_bounds("file-context-menu").is_some(), "right click must render the menu");
    click(cx, "context-preview");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let preview = workspace.preview.as_ref().expect("the menu must open a preview");
        assert!(matches!(&preview.content, crate::workspace::preview::PreviewContent::Text(text) if text.as_ref() == "hello"));
    });
    cx.simulate_keystrokes("escape f3");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.split_panes.len(), 1));
    assert!(cx.debug_bounds("browser-pane-0").is_some());
    assert!(cx.debug_bounds("browser-pane-1").is_some(), "split pane must be drawn");
    click(cx, "toggle-split");
    view.read_with(cx, |workspace, _| assert!(workspace.split_panes.is_empty()));
    cx.simulate_keystrokes("f2 ctrl-a r e n a m e d . t x t enter");
    cx.simulate_keystrokes("ctrl-c ctrl-l h o m e enter");
    cx.run_until_parked();
    assert!(fixture.join("port/renamed.txt").is_file());
    cx.simulate_keystrokes("ctrl-shift-n d e s t enter");
    cx.simulate_keystrokes("ctrl-l h o m e / d e s t enter");
    cx.simulate_keystrokes("ctrl-v");
    cx.run_until_parked();
    assert_eq!(fs::read_to_string(fixture.join("dest/renamed.txt")).unwrap(), "hello");
    if !local {
        let row = cx.debug_bounds("row-0-0").unwrap();
        cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
        click(cx, "context-share");
        view.read_with(cx, |workspace, app| {
            assert!(matches!(&workspace.settings_panel.dialog, Some(crate::settings_state::SettingsDialog::CreateShare { .. })));
            assert_eq!(workspace.settings_field(crate::ui::SettingsField::First).read(app).text(), "home/dest/renamed.txt");
        });
        click(cx, "settings-share-create-confirm");
        cx.run_until_parked();
        view.read_with(cx, |workspace, _| {
            assert!(workspace.settings_panel.error.is_none(), "share creation failed: {:?}", workspace.settings_panel.error);
            assert_eq!(workspace.active_api().list_shares().unwrap().len(), 1);
        });
        let share_id = view.read_with(cx, |workspace, _| workspace.active_api().list_shares().unwrap()[0].id.clone());
        // GPUI's test selector API requires a static string, including server-generated IDs.
        let selector = Box::leak(format!("share-revoke-{share_id}").into_boxed_str());
        let revoke = cx.debug_bounds(selector).expect("share revoke action must render");
        cx.simulate_click(revoke.center(), Modifiers::default());
        click(cx, "settings-share-revoke-confirm");
        cx.run_until_parked();
        view.read_with(cx, |workspace, _| assert!(workspace.active_api().list_shares().unwrap().is_empty()));
        cx.simulate_keystrokes("escape");
    }
    cx.simulate_keystrokes("down delete");
    click(cx, "confirm-accept");
    cx.run_until_parked();
    assert!(!fixture.join("dest/renamed.txt").exists());
    if local {
        click(cx, "sidebar-trash");
        click(cx, "restore-renamed.txt");
        cx.run_until_parked();
        assert_eq!(fs::read_to_string(fixture.join("dest/renamed.txt")).unwrap(), "hello");
        cx.simulate_keystrokes("alt-left");
    }
    // Exercise the async decoder, persistence, fit and removal. The GPUI test platform has no file picker.

    let wallpaper = fixture.join("wallpaper.png");
    image::RgbImage::from_fn(96, 64, |x, _| if x < 48 { image::Rgb([200, 20, 30]) } else { image::Rgb([20, 30, 200]) }).save(&wallpaper).unwrap();
    cx.simulate_keystrokes("ctrl-,");
    click(cx, "settings-appearance");
    cx.update(|_, app| view.update(app, |workspace, cx| workspace.install_wallpaper(wallpaper, cx)));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.has_wallpaper()));
    assert!(DesktopSettings::load().wallpaper.is_some(), "wallpaper choice must survive restart");
    // Accent: switch the source to Custom, then type a colour into the field.
    click(cx, "accent-source");
    click(cx, "accent-source-1");
    click(cx, "accent-color-input");
    cx.simulate_keystrokes("1 2 a b c d");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.accent_hex(), 0x12abcd));
    click(cx, "setting-wallpaper-fit");
    click(cx, "setting-wallpaper-fit-1");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.desktop_settings.wallpaper_fit, crate::settings_state::WallpaperFit::Fit));
    click(cx, "setting-frost");
    click(cx, "setting-tint");
    view.read_with(cx, |workspace, _| {
        assert!(workspace.preferences.frosted_glass);
        assert!(workspace.preferences.wallpaper_tint);
    });
    click(cx, "wallpaper-remove");
    view.read_with(cx, |workspace, _| assert!(!workspace.has_wallpaper()));
    assert!(DesktopSettings::load().wallpaper.is_none());
    cx.simulate_keystrokes("escape ctrl-t ctrl-tab");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.active_tab, 0));
    cx.simulate_keystrokes("ctrl-shift-tab");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.active_tab, 1));
    cx.simulate_keystrokes("ctrl-w ctrl-shift-o");
    view.read_with(cx, |workspace, _| assert!(workspace.settings_section == SettingsSection::Connections));
    click(cx, "settings-appearance");
    cx.update(|_, app| view.update(app, |workspace, _| {
        workspace.desktop_settings.connections.push(crate::settings_state::SavedConnection {
            id: "kept".into(), name: "Kept server".into(), url: "http://127.0.0.1:1234".into(),
        });
    }));
    click(cx, "settings-reset-button");
    click(cx, "confirm-cancel");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.accent_hex(), 0x12abcd));
    click(cx, "settings-reset-button");
    click(cx, "confirm-accept");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.desktop_settings.connections.len(), 1, "reset must preserve saved servers");
        assert!(workspace.desktop_settings.custom_accent.is_none());
        assert!(workspace.favorites.entries.is_empty());
    });
    drop(server);
    fs::remove_dir_all(&fixture).unwrap();
    if !local { fs::remove_dir_all(fixture.with_extension("server")).unwrap(); }
}
