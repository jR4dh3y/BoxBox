//! The file manager's behaviours, driven through the rendered workspace on a
//! temporary folder: clicks, keys and the files they change.

use std::path::{Path, PathBuf};

use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};

use super::parity_test::{click, TestConfig};
use super::*;

pub(super) struct Fixture {
    pub root: PathBuf,
    _config: TestConfig,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Read-only folders made by a test must be writable again to be removed.
        let _ = std::process::Command::new("chmod").arg("-R").arg("u+rwx").arg(&self.root).status();
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A workspace on a fresh folder holding `files` (names ending in `/` are folders),
/// opened at its root.
pub(super) fn open<'a>(cx: &'a mut TestAppContext, files: &[&str]) -> (Entity<Workspace>, &'a mut VisualTestContext, Fixture) {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("boxbox-features-{}-{stamp}", std::process::id()));
    let config = TestConfig::new(root.with_extension("config"));
    for file in files {
        let path = root.join(file);
        if file.ends_with('/') {
            fs::create_dir_all(&path).unwrap();
        } else {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, file.as_bytes()).unwrap();
        }
    }
    fs::create_dir_all(&root).unwrap();
    let api = Api::local_with_root(root.clone()).unwrap();
    cx.update(bind_keys);
    cx.update(crate::components::text_field_bind_keys);
    let (view, cx) = cx.add_window_view(|window, cx| {
        Workspace::new(startup::Startup::deferred(api, "home"), Fonts { sans: "sans-serif".into(), mono: "monospace".into() }, window, cx)
    });
    cx.update(|_, app| {
        view.update(app, |workspace, cx| {
            workspace.preferences = Preferences { default_sort: crate::sort::SortField::Name, default_sort_direction: crate::sort::SortDir::Asc, ..Preferences::default() };
            // The settings file is shared by every test in the process: what another
            // test saved must not reach the panes.
            workspace.apply_preferences(cx);
            workspace.choose_sort(PaneTarget(0), Sort { field: crate::sort::SortField::Name, dir: crate::sort::SortDir::Asc }, cx);
        })
    });
    // Focus events reach their listeners only in an active window.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx, Fixture { root, _config: config })
}

/// Draws a frame, which is when focus changes reach their listeners, then waits.
pub(super) fn settle(cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(gpui::point(gpui::px(1.), gpui::px(1.)), None, Modifiers::default());
    cx.run_until_parked();
}

pub(super) fn notices(view: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |workspace, _| workspace.notices.visible().iter().map(|notice| notice.message.clone()).collect())
}

pub(super) fn names(view: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |workspace, _| workspace.pane().items.iter().map(|item| item.name.clone()).collect())
}

fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[gpui::test]
fn right_click_on_empty_space_clears_the_selection(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt", "b.txt"]);
    click(cx, "row-0-0");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().selected.len(), 1));
    let pane = cx.debug_bounds("browser-pane-0").unwrap();
    let empty = gpui::point(pane.center().x, pane.bottom() - gpui::px(60.));
    cx.simulate_mouse_down(empty, MouseButton::Right, Modifiers::default());
    view.read_with(cx, |workspace, _| {
        assert!(workspace.pane().selected.is_empty());
        assert!(workspace.context_menu.as_ref().is_some_and(|menu| menu.items.is_empty()));
    });
}

#[gpui::test]
fn a_read_only_folder_refuses_changes_with_a_notice(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["locked/", "locked/a.txt"]);
    set_mode(&fixture.root.join("locked"), 0o555);
    cx.simulate_keystrokes("ctrl-l h o m e / l o c k e d enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-shift-n");
    view.read_with(cx, |workspace, _| assert!(workspace.operation_dialog.is_none()));
    cx.simulate_keystrokes("down delete");
    assert_eq!(notices(&view, cx).iter().filter(|message| *message == "This location is read-only or unavailable").count(), 2);
    view.read_with(cx, |workspace, _| assert!(workspace.confirm.is_none()));
}

#[gpui::test]
fn a_typed_location_opens_with_its_listing_and_a_wrong_one_stays_to_be_fixed(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["docs/b.txt", "docs/a.txt", "top.txt"]);
    cx.simulate_keystrokes("ctrl-l h o m e / m i s s i n g enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert!(workspace.location_editor.is_some());
        assert_eq!(workspace.pane().path, "home");
    });
    cx.simulate_keystrokes("ctrl-l h o m e / d o c s enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert!(workspace.location_editor.is_none());
        assert_eq!(workspace.pane().path, "home/docs");
    });
    assert_eq!(names(&view, cx), ["a.txt", "b.txt"]);
    cx.simulate_keystrokes("alt-left");
    cx.run_until_parked();
    assert_eq!(names(&view, cx), ["docs", "top.txt"]);
}

#[gpui::test]
fn pasting_with_nothing_copied_says_so(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    cx.simulate_keystrokes("ctrl-v");
    cx.run_until_parked();
    assert!(notices(&view, cx).contains(&"The clipboard has no files to paste".to_string()));
}

fn dialog_text(view: &Entity<Workspace>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |workspace, app| workspace.dialog_field.read(app).text().to_string())
}

#[gpui::test]
fn the_create_dialog_suggests_a_free_name_and_explains_a_bad_one(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["New folder/", "untitled.txt"]);
    cx.simulate_keystrokes("ctrl-shift-n");
    assert_eq!(dialog_text(&view, cx), "New folder 2");
    // The suggestion is selected, so typing replaces it.
    cx.simulate_keystrokes("a / b enter");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.dialog_error.as_deref(), Some("A name cannot contain “/”.")));
    cx.simulate_keystrokes("ctrl-a space s p a c e d space enter");
    cx.run_until_parked();
    assert!(fixture.root.join("spaced").is_dir(), "the name is trimmed");
    cx.simulate_keystrokes("ctrl-shift-e");
    assert_eq!(dialog_text(&view, cx), "untitled 2.txt");
    // Only the part before the extension is selected.
    cx.simulate_keystrokes("n o t e s enter");
    cx.run_until_parked();
    assert!(fixture.root.join("notes.txt").is_file());
    cx.simulate_keystrokes("ctrl-shift-e ctrl-a n o t e s . t x t enter");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.dialog_error.as_deref(), Some("“notes.txt” already exists here.")));
}

#[gpui::test]
fn rename_happens_in_place(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt", "b.txt", "c.txt"]);
    click(cx, "row-0-0");
    cx.simulate_keystrokes("f2");
    view.read_with(cx, |workspace, _| {
        assert!(workspace.operation_dialog.is_none(), "no dialog");
        assert!(workspace.renaming(PaneTarget(0), "home/a.txt"));
    });
    // The part before the extension is selected; Up and Down stay in the field.
    cx.simulate_keystrokes("d up down enter");
    cx.run_until_parked();
    assert!(fixture.root.join("d.txt").is_file() && !fixture.root.join("a.txt").exists());
    assert_eq!(names(&view, cx), ["b.txt", "c.txt", "d.txt"]);

    // Escape keeps the old name.
    click(cx, "row-0-0");
    cx.simulate_keystrokes("f2 x escape");
    cx.run_until_parked();
    assert!(fixture.root.join("b.txt").is_file());
    view.read_with(cx, |workspace, _| assert!(workspace.inline_rename.is_none()));

    // A taken name is refused and the field stays open; a click elsewhere keeps the text.
    cx.simulate_keystrokes("f2 c enter");
    assert!(notices(&view, cx).contains(&"“c.txt” already exists here.".to_string()));
    view.read_with(cx, |workspace, _| assert!(workspace.inline_rename.is_some()));
    cx.simulate_keystrokes("ctrl-a e . t x t");
    click(cx, "row-0-2");
    settle(cx);
    assert!(fixture.root.join("e.txt").is_file() && !fixture.root.join("b.txt").exists());
    // Undo puts the old name back.
    cx.simulate_keystrokes("ctrl-z");
    cx.run_until_parked();
    assert!(fixture.root.join("b.txt").is_file());
}

#[gpui::test]
fn properties_summarise_several_items_and_show_owner_and_permissions(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/", "docs/a.txt", "docs/deep/b.txt", "c.txt"]);
    cx.simulate_keystrokes("ctrl-a alt-enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let properties = workspace.properties.as_ref().expect("Alt+Enter opens Properties for several items");
        assert_eq!(properties.items.len(), 2);
        let Load::Loaded(totals) = &properties.totals else { panic!("totals are read") };
        // docs, docs/deep; a.txt, b.txt, c.txt and their bytes (each file holds its own name).
        assert_eq!((totals.folders, totals.files), (2, 3));
        assert_eq!(totals.bytes, ("docs/a.txt".len() + "docs/deep/b.txt".len() + "c.txt".len()) as u64);
    });
    assert!(cx.debug_bounds("properties-overlay").is_some());
    cx.simulate_keystrokes("escape");
    set_mode(&fixture.root.join("c.txt"), 0o640);
    click(cx, "row-0-1");
    cx.simulate_keystrokes("alt-enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let properties = workspace.properties.as_ref().unwrap();
        let Some(Load::Loaded(ownership)) = &properties.ownership else { panic!("owner is read") };
        assert_eq!(super::properties::permission_string(ownership.mode), "-rw-r-----");
        assert_eq!(ownership.owner, std::env::var("USER").unwrap());
    });
}

fn cursor(view: &Entity<Workspace>, cx: &mut VisualTestContext) -> (Option<usize>, Vec<String>) {
    view.read_with(cx, |workspace, _| {
        let pane = workspace.pane();
        let mut selected: Vec<String> = pane.selected.iter().map(|path| path.trim_start_matches("home/").to_string()).collect();
        selected.sort();
        (pane.active, selected)
    })
}

#[gpui::test]
fn the_keyboard_moves_selects_and_finds_by_name(cx: &mut TestAppContext) {
    let files: Vec<String> = (0..60).map(|index| format!("f{index:02}")).chain(["apple", "avocado", "banana"].map(String::from)).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (view, cx, _fixture) = open(cx, &files);
    cx.simulate_keystrokes("down down");
    assert_eq!(cursor(&view, cx), (Some(1), vec!["avocado".into()]));
    cx.simulate_keystrokes("shift-down");
    assert_eq!(cursor(&view, cx).1, ["avocado", "banana"]);
    cx.simulate_keystrokes("ctrl-down ctrl-down ctrl-space");
    assert_eq!(cursor(&view, cx), (Some(4), vec!["avocado".into(), "banana".into(), "f01".into()]));
    cx.simulate_keystrokes("end");
    assert_eq!(cursor(&view, cx), (Some(62), vec!["f59".into()]));
    cx.simulate_keystrokes("shift-home");
    assert_eq!(cursor(&view, cx).1.len(), 63);
    cx.simulate_keystrokes("home pagedown");
    let (Some(page), _) = cursor(&view, cx) else { panic!() };
    assert!(page > 5 && page < 62, "Page Down moves by the rows on screen, moved to {page}");
    // The cursor is scrolled into view.
    assert!(cx.debug_bounds(Box::leak(format!("row-0-{page}").into_boxed_str())).is_some());
    cx.simulate_keystrokes("pageup");
    assert_eq!(cursor(&view, cx).0, Some(0));
    // Left and Right do nothing in a list.
    cx.simulate_keystrokes("right");
    assert_eq!(cursor(&view, cx).0, Some(0));

    // Typing a name selects it; the same letter again goes to the next one.
    cx.simulate_keystrokes("b");
    assert_eq!(cursor(&view, cx).1, ["banana"]);
    view.update(cx, |workspace, _| workspace.type_ahead = Default::default());
    cx.simulate_keystrokes("a v");
    assert_eq!(cursor(&view, cx).1, ["avocado"]);
    view.update(cx, |workspace, _| workspace.type_ahead = Default::default());
    cx.simulate_keystrokes("a a");
    assert_eq!(cursor(&view, cx).1, ["avocado"]);

    // The Menu key opens the item's menu; with nothing selected, the folder's.
    cx.simulate_keystrokes("menu");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.context_menu.as_ref().unwrap().items.len(), 1));
    cx.simulate_keystrokes("escape escape shift-f10");
    view.read_with(cx, |workspace, _| assert!(workspace.context_menu.as_ref().unwrap().items.is_empty()));
}

#[gpui::test]
fn the_grid_moves_by_rows_and_columns(cx: &mut TestAppContext) {
    let files: Vec<String> = (0..30).map(|index| format!("f{index:02}")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (view, cx, _fixture) = open(cx, &files);
    view.update(cx, |workspace, _| workspace.pane_mut().set_view_mode(ViewMode::Grid));
    let columns = cx.update(|window, app| {
        let workspace = view.read(app);
        crate::components::GridMetrics::new(1).columns(workspace.browser_width(window))
    });
    assert!(columns > 1);
    cx.simulate_keystrokes("right right down");
    assert_eq!(cursor(&view, cx).0, Some(1 + columns));
    cx.simulate_keystrokes("shift-left");
    assert_eq!(cursor(&view, cx).1.len(), 2);
    cx.simulate_keystrokes("up");
    assert_eq!(cursor(&view, cx).0, Some(0));
}

#[gpui::test]
fn history_menus_list_and_back_skips_folders_that_are_gone(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a/", "a/b/", "a/b/c/"]);
    for path in ["home/a", "home/a/b", "home/a/b/c"] {
        view.update(cx, |workspace, cx| workspace.navigate(path.into(), cx));
    }
    cx.run_until_parked();
    let back = cx.debug_bounds("nav-back").unwrap();
    cx.simulate_mouse_down(back.center(), MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let menu = workspace.history_menu.as_ref().expect("right-click on Back lists the places");
        let paths: Vec<&str> = menu.places.iter().map(|place| place.path.as_str()).collect();
        assert_eq!(paths, ["home/a/b", "home/a", "home"]);
    });
    click(cx, "history-2");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home"));

    // Forward goes to a. With b and c removed, nothing ahead is listed, and Forward
    // drops them instead of opening a folder that is gone.
    cx.simulate_keystrokes("alt-right");
    cx.run_until_parked();
    fs::remove_dir_all(fixture.root.join("a/b")).unwrap();
    let forward = cx.debug_bounds("nav-forward").unwrap();
    cx.simulate_mouse_down(forward.center(), MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.history_menu.is_none(), "nothing ahead exists"));
    cx.simulate_keystrokes("alt-right");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.pane().path, "home/a");
        assert!(!workspace.pane().can_go_forward(), "places that are gone leave history");
    });
    cx.simulate_keystrokes("alt-left");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home"));
}

fn drag(cx: &mut VisualTestContext, from: &'static str, to: &'static str) {
    let start = cx.debug_bounds(from).unwrap().center();
    let end = cx.debug_bounds(to).unwrap().center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(start + gpui::point(gpui::px(10.), gpui::px(10.)), Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(end + gpui::point(gpui::px(1.), gpui::px(0.)), Some(MouseButton::Left), Modifiers::default());
    // The hover target settles in a deferred update, before the next frame.
    cx.update(|_, _| {});
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn servers_drives_sit_beside_this_computers(cx: &mut TestAppContext) {
    use crate::settings_state::SavedConnection;
    use servers::ServerState;
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    let server = Arc::new(Api::new("http://nas:8080", None));
    let drive = |name: &str| crate::api::DriveStats {
        name: name.into(),
        label: None,
        device: Some("/dev/sdb1".into()),
        fs_type: Some("ext4".into()),
        total_bytes: 100,
        free_bytes: 60,
        used_bytes: 40,
        used_pct: 40.,
        read_only: false,
    };
    view.update(cx, |workspace, _| {
        workspace.desktop_settings.connections.push(SavedConnection { id: "nas".into(), name: "nas".into(), url: "http://nas:8080".into() });
        workspace.desktop_settings.connections.push(SavedConnection { id: "office".into(), name: "office".into(), url: "http://office:8080".into() });
        workspace.servers.set("nas", ServerState::Ready { api: server.clone(), drives: vec![drive("media"), drive("backups")] });
        workspace.servers.set("office", ServerState::Offline("cannot reach BoxBox server".into()));
    });
    // This machine's own disks and phones may show up too; they are not this test's.
    let storage: Vec<_> = view.read_with(cx, |workspace, _| workspace.storage().expect("drives loaded")).into_iter().filter(|entry| !matches!(entry, overview::Storage::Volume(_))).collect();
    let names: Vec<String> = storage.iter().map(|entry| entry.info().name.to_string()).collect();
    assert_eq!(names, ["Home", "media", "backups", "office"], "this computer's drive, the ready server's drives, then the offline one");
    assert_eq!(storage[1].info().origin.map(|origin| origin.to_string()).as_deref(), Some("nas"));
    assert!(matches!(&storage[3], overview::Storage::Server { status: "Can’t reach the server", .. }));

    // A server drive opens in the same pane, which moves to the server; Back returns.
    view.update(cx, |workspace, cx| workspace.navigate_to(workspace.active_pane, server.clone(), "media".into(), cx));
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.tabs.len(), 1, "no new tab");
        assert!(Arc::ptr_eq(&workspace.pane().api, &server));
        assert!(storage[1].holds(workspace, &workspace.pane().api, "media/films"));
        assert!(!storage[0].holds(workspace, &workspace.pane().api, "home"));
    });
    // The sidebar keeps this computer's places while the pane is on the server.
    view.read_with(cx, |workspace, _| assert!(matches!(&workspace.home.roots, Load::Loaded(roots) if roots.iter().any(|root| root.name == "home"))));
    view.update(cx, |workspace, cx| workspace.navigate(String::new(), cx));
    view.read_with(cx, |workspace, _| {
        assert!(workspace.pane().is_at_root());
        assert!(Arc::ptr_eq(&workspace.pane().api, &workspace.home.api), "the Overview is this computer's");
    });
}

#[gpui::test]
fn a_long_server_drive_name_is_cut_before_its_server(cx: &mut TestAppContext) {
    use crate::settings_state::SavedConnection;
    use servers::ServerState;
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    let server = Arc::new(Api::new("http://pico:8080", None));
    let drive = |name: &str| crate::api::DriveStats { name: name.into(), label: None, device: None, fs_type: None, total_bytes: 100, free_bytes: 60, used_bytes: 40, used_pct: 40., read_only: false };
    view.update(cx, |workspace, cx| {
        workspace.desktop_settings.connections.push(SavedConnection { id: "pico".into(), name: "pico".into(), url: "http://pico:8080".into() });
        workspace.servers.set("pico", ServerState::Ready { api: server.clone(), drives: vec![drive("root"), drive("gdrive-radheykalra901-with-a-very-long-name")] });
        cx.notify();
    });
    settle(cx);
    let sidebar = cx.debug_bounds("sidebar").unwrap();
    let short = cx.debug_bounds("server-0-detail").unwrap();
    let long = cx.debug_bounds("server-1-detail").unwrap();
    assert_eq!(long.right(), short.right(), "both server labels end at the same place");
    assert!(long.right() <= sidebar.right(), "{long:?} inside {sidebar:?}");
}

fn server_drive(name: &str) -> crate::api::DriveStats {
    crate::api::DriveStats { name: name.into(), label: None, device: Some(format!("/srv/{name}")), fs_type: Some("ext4".into()), total_bytes: 100, free_bytes: 60, used_bytes: 40, used_pct: 40., read_only: false }
}

#[gpui::test]
fn remembered_server_drives_hold_their_places_until_the_server_answers(cx: &mut TestAppContext) {
    use crate::settings_state::SavedConnection;
    use servers::ServerState;
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    let pico = SavedConnection { id: "pico".into(), name: "pico".into(), url: "http://127.0.0.1:9".into() };
    view.update(cx, |workspace, cx| {
        workspace.desktop_settings.connections.push(pico.clone());
        // What the last run saw, as the start-up thread reads it back.
        workspace.remember("pico", &[server_drive("root"), server_drive("gdrive-radheykalra901")], cx);
    });
    // The snapshot is written off the UI thread.
    cx.run_until_parked();
    view.update(cx, |workspace, cx| {
        workspace.servers.set_remembered(Default::default());
        workspace.servers.set_remembered(servers::load_remembered());
        workspace.servers.set("pico", ServerState::Connecting { since: std::time::Instant::now() });
        workspace.navigate(String::new(), cx);
    });
    settle(cx);
    // Each entry's name, and its status while its usage is unknown.
    let entries = |view: &Entity<Workspace>, cx: &mut VisualTestContext| -> Vec<(String, String)> {
        view.read_with(cx, |workspace, _| {
            let storage = workspace.storage().unwrap().into_iter().filter(|entry| !matches!(entry, overview::Storage::Volume(_)));
            storage.map(|entry| (entry.info().name.to_string(), entry.info().state.status().map(|status| status.to_string()).unwrap_or_default())).collect()
        })
    };
    let named = |pairs: &[(&str, &str)]| pairs.iter().map(|(name, status)| (name.to_string(), status.to_string())).collect::<Vec<_>>();
    assert_eq!(entries(&view, cx), named(&[("Home", ""), ("root", "Connecting…"), ("gdrive-radheykalra901", "Connecting…")]));
    // The second server row's label, which a shift in the rows above would move.
    let card = |cx: &mut VisualTestContext| cx.debug_bounds("server-1-detail").unwrap();
    let waiting = card(cx);

    // Opened while the server connects, the drive says so and opens once it answers.
    let open = |view: &Entity<Workspace>, cx: &mut VisualTestContext, index: usize| {
        let entry = view.read_with(cx, |workspace, _| workspace.storage().unwrap().into_iter().filter(|entry| !matches!(entry, overview::Storage::Volume(_))).nth(index).unwrap());
        cx.update(|window, app| view.update(app, |workspace, cx| workspace.open_storage(entry, window, cx)));
    };
    open(&view, cx, 2);
    assert_eq!(entries(&view, cx)[2].1, "Opening…");
    view.read_with(cx, |workspace, _| assert!(workspace.pane().is_at_root(), "nothing to open yet"));
    let api = Arc::new(Api::new("http://127.0.0.1:9", None));
    let drives = vec![server_drive("root"), server_drive("gdrive-radheykalra901")];
    view.update(cx, |workspace, cx| workspace.server_answered(pico.clone(), api.clone(), drives.clone(), cx));
    settle(cx);
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.pane().path, "gdrive-radheykalra901");
        assert!(Arc::ptr_eq(&workspace.pane().api, &api));
    });
    // The server's drives took the places the remembered ones held.
    assert_eq!(entries(&view, cx), named(&[("Home", ""), ("root", ""), ("gdrive-radheykalra901", "")]));
    assert_eq!(card(cx), waiting, "nothing moved");

    // Going elsewhere before the server answers drops the waiting open.
    view.update(cx, |workspace, cx| {
        workspace.navigate(String::new(), cx);
        workspace.servers.set("pico", ServerState::Connecting { since: std::time::Instant::now() });
    });
    open(&view, cx, 1);
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    view.update(cx, |workspace, cx| workspace.server_answered(pico.clone(), api.clone(), drives.clone(), cx));
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home", "the late answer did not take the pane away"));

    // A server that cannot be reached keeps them, dimmed with the reason.
    view.update(cx, |workspace, cx| {
        workspace.servers.set("pico", ServerState::Offline("connection refused".into()));
        cx.notify();
    });
    assert_eq!(entries(&view, cx), named(&[("Home", ""), ("root", "Can’t reach the server"), ("gdrive-radheykalra901", "Can’t reach the server")]));

    // Without a snapshot a server is one entry, as before.
    view.update(cx, |workspace, _| workspace.servers.set_remembered(Default::default()));
    assert_eq!(entries(&view, cx).len(), 2);
}

/// Answers `/api/v1/files/stats` with each body in turn, one per request, and
/// counts the requests.
fn stats_server(bodies: Vec<String>) -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = hits.clone();
    std::thread::spawn(move || {
        for body in bodies {
            let Ok((mut stream, _)) = listener.accept() else { return };
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut request = String::new();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            reader.read_line(&mut request).unwrap();
            assert!(request.starts_with("GET /api/v1/files/stats "), "{request}");
            while reader.read_line(&mut request).is_ok_and(|read| read > 2) {}
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (url, hits)
}

#[gpui::test]
fn refresh_on_the_overview_lists_a_servers_drives_again(cx: &mut TestAppContext) {
    use crate::settings_state::SavedConnection;
    use servers::ServerState;
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    let drive = |name: &str, free: u64| format!(r#"{{"name":"{name}","totalBytes":100,"freeBytes":{free},"usedBytes":{},"usedPct":{}}}"#, 100 - free, 100 - free);
    let listed = format!(r#"{{"drives":[{},{}]}}"#, drive("media", 20), drive("usb", 90));
    let (url, hits) = stats_server(vec![listed.clone(), listed.clone(), listed]);
    let server = Arc::new(Api::new(&url, None));
    view.update(cx, |workspace, cx| {
        workspace.desktop_settings.connections.push(SavedConnection { id: "nas".into(), name: "nas".into(), url: url.clone() });
        workspace.servers.set("nas", ServerState::Ready { api: server.clone(), drives: vec![server_drive("media")] });
        workspace.navigate(String::new(), cx);
    });
    settle(cx);
    let drives = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        view.read_with(cx, |workspace, _| match workspace.servers.state("nas") {
            Some(ServerState::Ready { api, drives }) => {
                assert!(Arc::ptr_eq(api, &server), "open panes keep their server");
                drives.iter().map(|drive| (drive.name.clone(), drive.free_bytes)).collect::<Vec<_>>()
            }
            _ => panic!("the server stays connected"),
        })
    };
    assert_eq!(drives(&view, cx), [("media".to_string(), 60)]);

    // A drive added on the server, and the space another lost, show after Refresh.
    click(cx, "toolbar-refresh");
    view.read_with(cx, |workspace, _| assert!(workspace.refresh_turn(workspace.active_pane).is_some(), "the Refresh icon turns"));
    settle(cx);
    assert_eq!(drives(&view, cx), [("media".to_string(), 20), ("usb".to_string(), 90)]);
    view.read_with(cx, |workspace, _| assert!(!workspace.servers.busy()));
    // The icon waits on a listing still out, and a second Refresh meanwhile
    // asks nothing, so an older answer cannot land after a newer one.
    let before = hits.load(std::sync::atomic::Ordering::SeqCst);
    view.update(cx, |workspace, cx| {
        workspace.refresh_servers(cx);
        assert!(workspace.servers.busy(), "the server is listing its drives");
        workspace.refresh_servers(cx);
    });
    settle(cx);
    view.read_with(cx, |workspace, _| assert!(!workspace.servers.busy()));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), before + 1, "one listing at a time");
    // Done, it finishes its turn and stops.
    std::thread::sleep(std::time::Duration::from_millis(750));
    view.read_with(cx, |workspace, _| assert_eq!(workspace.refresh_turn(workspace.active_pane), None));
    let names: Vec<String> = view.read_with(cx, |workspace, _| workspace.storage().unwrap()).into_iter().filter_map(|entry| match entry {
        overview::Storage::ServerDrive { drive, .. } => Some(drive.name),
        _ => None,
    }).collect();
    assert_eq!(names, ["media", "usb"]);
}

#[gpui::test]
fn refresh_waits_on_everything_it_started(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["docs/a.txt", "b.txt"]);
    let refresh = |view: &Entity<Workspace>, cx: &mut VisualTestContext, check: fn(&Workspace)| {
        cx.update(|window, app| {
            view.update(app, |workspace, cx| {
                workspace.refresh(&Refresh, window, cx);
                check(workspace);
                assert!(workspace.refresh_turn(workspace.active_pane).is_some());
            })
        });
    };
    // A folder with a search open: the listing and the search again.
    view.update(cx, |workspace, _| workspace.pane_mut().search_input = "txt".into());
    refresh(&view, cx, |workspace| {
        assert!(workspace.pane().busy, "the listing");
        assert!(matches!(workspace.pane().search, crate::pane::Search::Loading), "the search, at once");
    });
    settle(cx);
    view.read_with(cx, |workspace, _| assert!(!workspace.pane().loading()));
    // The Overview with no servers: this computer's drives.
    view.update(cx, |workspace, cx| {
        workspace.pane_mut().search_input.clear();
        workspace.navigate(String::new(), cx);
    });
    settle(cx);
    refresh(&view, cx, |workspace| assert!(workspace.pane().drives_busy, "this computer's drives"));
    settle(cx);
    view.read_with(cx, |workspace, _| assert!(!workspace.pane().loading()));
}

#[gpui::test]
fn leaving_a_server_closes_its_tabs_even_after_it_went_offline(cx: &mut TestAppContext) {
    use crate::settings_state::SavedConnection;
    use servers::ServerState;
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    let nas = SavedConnection { id: "nas".into(), name: "nas".into(), url: "http://127.0.0.1:9/".into() };
    // The tab holds the `Api` it opened with; the state lost it when a refresh failed.
    let server = Arc::new(Api::new("http://127.0.0.1:9", None));
    view.update(cx, |workspace, cx| {
        workspace.desktop_settings.connections.push(nas.clone());
        workspace.navigate_to(workspace.active_pane, server.clone(), "media".into(), cx);
        workspace.servers.set("nas", ServerState::Offline("connection refused".into()));
    });
    view.read_with(cx, |workspace, _| assert!(Arc::ptr_eq(&workspace.pane().api, &server)));
    view.update(cx, |workspace, cx| workspace.leave_server(&nas, cx));
    view.read_with(cx, |workspace, _| {
        assert!(workspace.pane().is_at_root(), "the tab went back to the Overview");
        assert!(Arc::ptr_eq(&workspace.pane().api, &workspace.home.api));
    });
}

#[gpui::test]
fn middle_click_opens_a_folder_in_a_tab_and_side_buttons_go_back(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["docs/", "music/", "a.txt"]);
    let press = |cx: &mut VisualTestContext, selector: &'static str, button: MouseButton, modifiers: Modifiers| {
        let at = cx.debug_bounds(selector).unwrap().center();
        cx.simulate_mouse_down(at, button, modifiers);
        cx.simulate_mouse_up(at, button, modifiers);
        cx.run_until_parked();
    };
    let tabs = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        view.read_with(cx, |workspace, _| (workspace.tabs.iter().map(|pane| pane.path.clone()).collect::<Vec<_>>(), workspace.active_tab))
    };
    // Sorted by name: docs, music, a.txt.
    press(cx, "row-0-0", MouseButton::Middle, Modifiers::default());
    assert_eq!(tabs(&view, cx), (vec!["home".to_string(), "home/docs".to_string()], 1), "a new tab, in front");
    view.update(cx, |workspace, cx| workspace.activate_tab(0, cx));
    settle(cx);
    press(cx, "row-0-1", MouseButton::Middle, Modifiers::control());
    assert_eq!(tabs(&view, cx), (vec!["home".to_string(), "home/docs".to_string(), "home/music".to_string()], 0), "with Ctrl, behind");
    // A file is not a folder: nothing opens.
    press(cx, "row-0-2", MouseButton::Middle, Modifiers::default());
    assert_eq!(tabs(&view, cx).0.len(), 3);

    // The mouse's side buttons go back and forward.
    view.update(cx, |workspace, cx| workspace.navigate("home/docs".into(), cx));
    settle(cx);
    let pane = || "browser-pane-0";
    press(cx, pane(), MouseButton::Navigate(gpui::NavigationDirection::Back), Modifiers::default());
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home"));
    press(cx, pane(), MouseButton::Navigate(gpui::NavigationDirection::Forward), Modifiers::default());
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/docs"));
}

#[gpui::test]
fn a_launch_without_a_folder_opens_the_start_location_in_the_open_window(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["docs/a.txt", "music/"]);
    let (sender, launches) = async_channel::unbounded();
    cx.update(|window, app| view.update(app, |workspace, cx| workspace.receive_launches(launches, window, cx)));
    view.update(cx, |workspace, cx| {
        workspace.preferences.start_location = StartLocation::Last;
        workspace.navigate("home/docs".into(), cx);
    });
    cx.run_until_parked();
    sender.send_blocking(String::new()).unwrap();
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/docs", "the file-manager key: the last location"));
    // A folder given at launch still wins.
    sender.send_blocking("home/music".into()).unwrap();
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.pane().path, "home/music");
        assert_eq!(workspace.tabs.len(), 3);
    });
}

#[gpui::test]
fn a_launch_opens_this_computers_folder_while_a_pane_is_on_a_server(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/a.txt"]);
    let (sender, launches) = async_channel::unbounded();
    cx.update(|window, app| view.update(app, |workspace, cx| workspace.receive_launches(launches, window, cx)));
    let server = Arc::new(Api::new("http://127.0.0.1:9", None));
    view.update(cx, |workspace, cx| workspace.navigate_to(workspace.active_pane, server.clone(), "media".into(), cx));
    cx.run_until_parked();
    // As `xdg-open` hands over a real folder.
    sender.send_blocking(fixture.root.join("docs").to_string_lossy().into_owned()).unwrap();
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.pane().path, "home/docs");
        assert!(Arc::ptr_eq(&workspace.pane().api, &workspace.home.api), "on this computer, not the server");
    });
}

#[gpui::test]
fn the_last_location_is_kept_for_the_next_launch(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/a.txt"]);
    let api = Api::local_with_root(fixture.root.clone()).unwrap();
    let kept = || fs::read_to_string(crate::persist::LAST_LOCATION.path()).unwrap_or_default();
    view.update(cx, |workspace, cx| workspace.navigate("home/docs".into(), cx));
    cx.run_until_parked();
    assert_eq!(kept(), "home/docs", "written off the UI thread");
    assert_eq!(startup::start_path(&api, StartLocation::Last), "home/docs");
    assert_eq!(startup::start_path(&api, StartLocation::Overview), "");

    // The Overview is a place too.
    view.update(cx, |workspace, cx| workspace.navigate(String::new(), cx));
    cx.run_until_parked();
    assert_eq!(kept(), "");

    // A server's folder needs the server first, so it is not kept.
    let server = Arc::new(Api::new("http://127.0.0.1:9", None));
    view.update(cx, |workspace, cx| workspace.navigate_to(workspace.active_pane, server, "media".into(), cx));
    cx.run_until_parked();
    assert_eq!(kept(), "");
}

#[gpui::test]
fn the_trash_notice_undoes_its_move(cx: &mut TestAppContext) {
    let (_view, cx, fixture) = open(cx, &["docs/", "b.txt"]);
    drag(cx, "row-0-1", "sidebar-trash");
    assert!(!fixture.root.join("b.txt").exists());
    click(cx, "toast-action");
    cx.run_until_parked();
    assert!(fixture.root.join("b.txt").is_file(), "Undo put the file back");
}

#[gpui::test]
fn trash_takes_drops_shows_restored_items_and_empties(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/", "docs/a.txt", "b.txt"]);
    view.read_with(cx, |workspace, _| assert!(!workspace.trash_full));
    // Dropping a row onto Trash moves it there without a question.
    drag(cx, "row-0-1", "sidebar-trash");
    assert!(!fixture.root.join("b.txt").exists(), "the dropped file went to Trash");
    view.read_with(cx, |workspace, _| {
        assert!(workspace.confirm.is_none());
        assert!(workspace.trash_full, "the Trash icon shows it is full");
        let notices: Vec<_> = workspace.notices.visible().iter().map(|notice| (notice.message.as_str(), notice.action.as_ref().map(|(label, _)| label.to_string()))).collect();
        assert_eq!(notices, [("Moved 1 item to Trash", Some("Undo".to_string()))], "only the move to Trash is announced, with Undo");
    });
    view.update(cx, |workspace, cx| {
        let id = workspace.notices.visible()[0].id;
        workspace.dismiss_notice(id, cx);
    });

    // Trash lists like a folder: select a row, then Restore offers to show the
    // item where it went back to.
    click(cx, "sidebar-trash");
    cx.run_until_parked();
    click(cx, "trash-row-b.txt");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.selected_trash().len(), 1));
    click(cx, "restore-trash");
    cx.run_until_parked();
    assert!(fixture.root.join("b.txt").is_file());
    view.update(cx, |workspace, cx| workspace.navigate("home/docs".into(), cx));
    cx.run_until_parked();
    click(cx, "toast-action");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert!(!workspace.trash_open);
        assert_eq!(workspace.pane().path, "home");
        assert!(workspace.pane().is_selected("home/b.txt"));
        assert!(!workspace.trash_full);
    });

    // Empty Trash asks, then deletes everything, from the page or the sidebar menu.
    cx.simulate_keystrokes("delete");
    cx.run_until_parked();
    let trash = cx.debug_bounds("sidebar-trash").unwrap().center();
    cx.simulate_mouse_down(trash, MouseButton::Right, Modifiers::default());
    click(cx, "place-empty-trash");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.confirm.as_ref().unwrap().title.as_ref(), "Empty Trash"));
    click(cx, "confirm-accept");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(!workspace.trash_full));
    assert_eq!(fs::read_dir(fixture.root.join(".local/share/Trash/files")).unwrap().count(), 0);
}

#[gpui::test]
fn trash_selects_like_a_folder_and_deletes_the_selection(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt", "b.txt", "c.txt"]);
    for _ in 0..2 {
        drag(cx, "row-0-0", "sidebar-trash");
        cx.run_until_parked();
    }
    click(cx, "sidebar-trash");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.trash_items().len(), 2));
    cx.simulate_keystrokes("ctrl-a");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.selected_trash().len(), 2, "Ctrl+A selects every row"));
    // In Trash, Delete can only mean for good, so it asks first.
    cx.simulate_keystrokes("delete");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.confirm.as_ref().unwrap().title.as_ref(), "Delete permanently"));
    click(cx, "confirm-accept");
    cx.run_until_parked();
    assert_eq!(fs::read_dir(fixture.root.join(".local/share/Trash/files")).unwrap().count(), 0);
    assert_eq!(["a.txt", "b.txt", "c.txt"].iter().filter(|name| fixture.root.join(name).exists()).count(), 1, "the item left out stays");
}

#[gpui::test]
fn breadcrumbs_open_tabs_take_drops_and_fold_long_paths(cx: &mut TestAppContext) {
    let deep = "a-rather-long-folder-name/another-long-folder-name/yet-another-long-folder/and-one-more-folder/";
    let (view, cx, fixture) = open(cx, &[deep, "x.txt", "y.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate("home/a-rather-long-folder-name/another-long-folder-name".into(), cx));
    cx.run_until_parked();

    // Middle-click opens the crumb in a new tab.
    let home = cx.debug_bounds("crumb-0-0").unwrap().center();
    cx.simulate_mouse_down(home, MouseButton::Middle, Modifiers::default());
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.tabs.len(), 2);
        assert_eq!(workspace.pane().path, "home");
    });
    cx.run_until_parked();

    // Copy a file in the new tab, then paste it into a crumb's folder from its menu.
    click(cx, "row-1-1");
    cx.simulate_keystrokes("ctrl-c");
    view.update(cx, |workspace, cx| workspace.navigate("home/a-rather-long-folder-name/another-long-folder-name".into(), cx));
    cx.run_until_parked();
    let first = cx.debug_bounds("crumb-1-1").unwrap().center();
    cx.simulate_mouse_down(first, MouseButton::Right, Modifiers::default());
    click(cx, "place-paste");
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("a-rather-long-folder-name/x.txt").is_file());

    // A row dropped onto a crumb moves into that folder; resting on a crumb opens it.
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    view.update(cx, |workspace, cx| workspace.navigate("home/a-rather-long-folder-name".into(), cx));
    cx.run_until_parked();
    click(cx, "crumb-1-0");
    cx.run_until_parked();
    drag(cx, "row-1-2", "crumb-1-0");
    assert!(fixture.root.join("y.txt").is_file(), "dropping on the folder the item is in does nothing");
    view.update(cx, |workspace, cx| workspace.navigate("home/a-rather-long-folder-name/another-long-folder-name".into(), cx));
    cx.run_until_parked();
    let start = cx.debug_bounds("crumb-1-0").unwrap().center();
    // Drag the folder's only row (it holds yet-another-long-folder) and rest on "home".
    let row = cx.debug_bounds("row-1-0").unwrap().center();
    cx.simulate_mouse_down(row, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(row + gpui::point(gpui::px(10.), gpui::px(10.)), Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(start, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(start + gpui::point(gpui::px(1.), gpui::px(0.)), Some(MouseButton::Left), Modifiers::default());
    cx.update(|_, _| {});
    cx.executor().advance_clock(std::time::Duration::from_millis(800));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home", "resting on a crumb opens it"));
    let pane = cx.debug_bounds("browser-pane-1").unwrap();
    let inside = gpui::point(pane.center().x, pane.bottom() - gpui::px(60.));
    cx.simulate_mouse_move(inside, Some(MouseButton::Left), Modifiers::default());
    cx.update(|_, _| {});
    cx.simulate_mouse_up(inside, MouseButton::Left, Modifiers::default());
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("yet-another-long-folder").is_dir(), "the drop lands in the opened folder");

    // A narrow window hides the first crumbs behind "…", which lists them.
    view.update(cx, |workspace, cx| workspace.navigate("home/a-rather-long-folder-name/another-long-folder-name".into(), cx));
    cx.simulate_resize(gpui::size(gpui::px(640.), gpui::px(480.)));
    cx.run_until_parked();
    settle(cx);
    assert!(cx.debug_bounds("crumb-overflow").is_some(), "a long path folds");
    click(cx, "crumb-overflow");
    view.read_with(cx, |workspace, _| {
        let Some(crate::workspace::place_menu::PlaceMenu { place: crate::workspace::place_menu::Place::Hidden { crumbs, .. }, .. }) = &workspace.place_menu else { panic!("… lists the hidden crumbs") };
        assert_eq!(crumbs[0].1, "home");
    });
    click(cx, "hidden-crumb-0");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home"));
}

#[gpui::test]
fn recent_lists_used_files_where_they_are(cx: &mut TestAppContext) {
    let stamp = |day: u32| format!("2026-01-{day:02}T00:00:00Z");
    let files = ["docs/", "docs/report.pdf", "notes.txt"];
    let (view, cx, fixture) = open(cx, &files);
    let entry = |name: &str, day: u32| {
        format!(r#"<bookmark href="file://{}/{name}" added="{0}" modified="{0}" visited="{0}"></bookmark>"#, fixture.root.display()).replace("{0}", &stamp(day))
    };
    let xbel = format!("<xbel version=\"1.0\">{}{}</xbel>", entry("notes.txt", 1), entry("docs/report.pdf", 2));
    fs::create_dir_all(fixture.root.join(".local/share")).unwrap();
    fs::write(fixture.root.join(".local/share/recently-used.xbel"), xbel).unwrap();
    view.read_with(cx, |workspace, _| {
        let crate::pane::Load::Loaded(roots) = &workspace.pane().roots else { panic!() };
        assert!(roots.iter().any(|root| root.name == "recent") && roots.iter().any(|root| root.name == "network"));
    });
    view.update(cx, |workspace, cx| workspace.navigate("recent".into(), cx));
    cx.run_until_parked();
    let mut found = names(&view, cx);
    found.sort();
    assert_eq!(found, ["notes.txt", "report.pdf"]);
    view.read_with(cx, |workspace, _| {
        let pane = workspace.pane();
        assert!(pane.shows_locations());
        assert!(pane.items.iter().any(|item| item.path == "home/docs/report.pdf"), "items keep their own paths");
    });
    // Recent is a listing, not a folder: nothing is created in it.
    cx.simulate_keystrokes("ctrl-shift-n");
    view.read_with(cx, |workspace, _| assert!(workspace.operation_dialog.is_none()));
}

#[gpui::test]
fn local_favorites_are_gtk_bookmarks_and_reorder_by_dragging(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["music/", "photos/", "work/"]);
    // Bookmarks another app wrote, including one for a drive that is not plugged in.
    let bookmarks = fixture.root.with_extension("config").join("gtk-3.0/bookmarks");
    fs::create_dir_all(bookmarks.parent().unwrap()).unwrap();
    let away = "file:///run/media/nobody/USB/stuff";
    fs::write(&bookmarks, format!("{} Tunes\n{away}\nsftp://host/srv Server\n", crate::os_clipboard::file_uri(&fixture.root.join("music")))).unwrap();
    view.update(cx, |workspace, cx| workspace.load_bookmarks(cx));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let names: Vec<&str> = workspace.favorites.entries.iter().map(|favorite| favorite.name.as_str()).collect();
        assert_eq!(names, ["Tunes", "stuff", "Server"]);
        assert_eq!(workspace.favorites.entries[0].path, "home/music");
    });

    // Pinning a folder adds a bookmark; the others are written back as they were.
    let photos = cx.debug_bounds("row-0-1").unwrap().center();
    cx.simulate_mouse_down(photos, MouseButton::Right, Modifiers::default());
    click(cx, "context-favorite");
    cx.run_until_parked();
    let written = fs::read_to_string(&bookmarks).unwrap();
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].ends_with("/music Tunes") && lines[1] == away && lines[2] == "sftp://host/srv Server");
    assert_eq!(lines[3], crate::os_clipboard::file_uri(&fixture.root.join("photos")));

    // Dragging the new favorite onto the first moves it there.
    drag(cx, "favorite-row-3", "favorite-row-0");
    cx.run_until_parked();
    let written = fs::read_to_string(&bookmarks).unwrap();
    assert!(written.lines().next().unwrap().ends_with("/photos"), "reordered: {written}");

    // Another app's change shows up.
    fs::write(&bookmarks, format!("{}\n", crate::os_clipboard::file_uri(&fixture.root.join("work")))).unwrap();
    view.update(cx, |workspace, cx| workspace.poll_directory_changes(cx));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let names: Vec<&str> = workspace.favorites.entries.iter().map(|favorite| favorite.name.as_str()).collect();
        assert_eq!(names, ["work"]);
    });
}

#[gpui::test]
fn sidebar_entries_have_menus(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/", "a.txt"]);
    // A drive: Open in New Tab and Properties.
    let home = cx.debug_bounds("sidebar-drive-1").unwrap().center();
    cx.simulate_mouse_down(home, MouseButton::Right, Modifiers::default());
    click(cx, "place-new-tab");
    view.read_with(cx, |workspace, _| assert_eq!((workspace.tabs.len(), workspace.pane().path.as_str()), (2, "home")));
    cx.run_until_parked();
    cx.simulate_mouse_down(home, MouseButton::Right, Modifiers::default());
    click(cx, "place-properties");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.properties.as_ref().unwrap().items[0].path, "home"));
    cx.simulate_keystrokes("escape");

    // A favorite: paste into it, then remove it.
    view.update(cx, |workspace, cx| workspace.navigate("home/docs".into(), cx));
    cx.simulate_keystrokes("ctrl-b");
    cx.run_until_parked();
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    click(cx, "row-1-1");
    cx.simulate_keystrokes("ctrl-c");
    let favorite = cx.debug_bounds("favorite-row-0").unwrap().center();
    cx.simulate_mouse_down(favorite, MouseButton::Right, Modifiers::default());
    click(cx, "place-paste");
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("docs/a.txt").is_file());
    cx.simulate_mouse_down(favorite, MouseButton::Right, Modifiers::default());
    click(cx, "place-unpin");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.favorites.entries.is_empty()));

    // Recent takes no paste and has no Properties.
    let recent = cx.debug_bounds("sidebar-place-0").unwrap().center();
    cx.simulate_mouse_down(recent, MouseButton::Right, Modifiers::default());
    click(cx, "place-paste");
    view.read_with(cx, |workspace, _| assert!(workspace.place_menu.is_some(), "a disabled entry does nothing"));
}

#[gpui::test]
fn new_files_come_from_templates_and_open_for_renaming(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["Templates/Letter.odt", "Templates/.hidden", "work/", "work/Letter.odt"]);
    view.update(cx, |workspace, cx| workspace.navigate("home/work".into(), cx));
    cx.run_until_parked();
    let pane = cx.debug_bounds("browser-pane-0").unwrap();
    cx.simulate_mouse_down(gpui::point(pane.center().x, pane.bottom() - gpui::px(60.)), MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    let submenu = cx.debug_bounds("new-from-template").expect("the folder menu offers templates").center();
    cx.simulate_mouse_move(submenu, None, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.templates.len(), 1, "hidden files are not templates"));
    click(cx, "template-0");
    cx.run_until_parked();
    settle(cx);
    assert!(fixture.root.join("work/Letter 2.odt").is_file(), "a taken name is numbered");
    view.read_with(cx, |workspace, _| assert!(workspace.renaming(PaneTarget(0), "home/work/Letter 2.odt"), "the new file opens for renaming"));
    cx.simulate_keystrokes("R e p l y enter");
    cx.run_until_parked();
    assert_eq!(fs::read(fixture.root.join("work/Reply.odt")).unwrap(), b"Templates/Letter.odt");
}

fn menu_on(cx: &mut VisualTestContext, row: &'static str) {
    let at = cx.debug_bounds(row).unwrap().center();
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
}

#[gpui::test]
fn item_menus_link_compress_open_tabs_and_send(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["Desktop/", "a/", "b/", "b/inner.txt", "x.txt", "y.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    // Rows by name, ignoring case.
    assert_eq!(names(&view, cx), ["a", "b", "Desktop", "x.txt", "y.txt"]);

    // Two folders open in two new tabs.
    click(cx, "row-0-0");
    cx.simulate_keystrokes("shift-down");
    menu_on(cx, "row-0-1");
    click(cx, "context-new-tab");
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.tabs.len(), 3);
        assert_eq!(workspace.pane().path, "home/b");
    });
    cx.simulate_keystrokes("ctrl-w ctrl-w");
    cx.run_until_parked();

    // Several items compress into Archive.zip, which opens for renaming.
    click(cx, "row-0-1");
    let x = names(&view, cx).iter().position(|name| name == "x.txt").unwrap();
    let x = cx.debug_bounds(Box::leak(format!("row-0-{x}").into_boxed_str())).unwrap().center();
    cx.simulate_click(x, Modifiers { control: true, ..Default::default() });
    menu_on(cx, "row-0-1");
    click(cx, "context-compress");
    cx.run_until_parked();
    settle(cx);
    view.read_with(cx, |workspace, _| assert!(workspace.renaming(PaneTarget(0), "home/Archive.zip")));
    cx.simulate_keystrokes("b u n d l e enter");
    cx.run_until_parked();
    let archive = zip::ZipArchive::new(fs::File::open(fixture.root.join("bundle.zip")).unwrap()).unwrap();
    let mut entries: Vec<&str> = archive.file_names().collect();
    entries.sort();
    assert_eq!(entries, ["b/", "b/inner.txt", "x.txt"]);

    // Send To copies to the Desktop.
    let y = names(&view, cx).iter().position(|name| name == "y.txt").unwrap();
    let row: &'static str = Box::leak(format!("row-0-{y}").into_boxed_str());
    menu_on(cx, row);
    let send = cx.debug_bounds("send-to").unwrap().center();
    cx.simulate_mouse_move(send, None, Modifiers::default());
    click(cx, "send-to-0");
    cx.run_until_parked();
    assert!(fixture.root.join("Desktop/y.txt").is_file() && fixture.root.join("y.txt").is_file());
}

/// Waits for a started program to leave its mark.
fn wait_for(path: &Path) -> bool {
    (0..100).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(20));
        path.exists()
    })
}

#[gpui::test]
fn scripts_ask_before_running_and_launchers_need_trust(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["go.sh", "tool.desktop"]);
    let ran = fixture.root.join("ran");
    let launched = fixture.root.join("launched");
    fs::write(fixture.root.join("go.sh"), format!("#!/bin/sh\ntouch '{}'\n", ran.display())).unwrap();
    set_mode(&fixture.root.join("go.sh"), 0o755);
    fs::write(fixture.root.join("tool.desktop"), format!("[Desktop Entry]\nType=Application\nName=Tool\nExec=touch \"{}\"\n", launched.display())).unwrap();
    set_mode(&fixture.root.join("tool.desktop"), 0o644);
    cx.simulate_keystrokes("f5");
    cx.run_until_parked();

    // An executable script asks, offering to open it instead.
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        let confirm = workspace.confirm.as_ref().expect("running asks first");
        assert_eq!(confirm.title.as_ref(), "Run “go.sh”?");
        assert!(confirm.secondary.is_some());
    });
    assert!(!ran.exists(), "nothing runs before the answer");
    click(cx, "confirm-accept");
    cx.run_until_parked();
    assert!(wait_for(&ran), "Run starts the script");

    // An untrusted launcher asks; trusting it marks it executable and starts it.
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.confirm.as_ref().unwrap().title.as_ref(), "Untrusted launcher"));
    // The message wraps; the panel grows to hold it, so its buttons stay inside.
    let (panel, accept) = (cx.debug_bounds("modal-panel").unwrap(), cx.debug_bounds("confirm-accept").unwrap());
    assert!(accept.bottom() <= panel.bottom(), "the footer is inside the dialog: {accept:?} in {panel:?}");
    click(cx, "confirm-accept");
    cx.run_until_parked();
    assert!(wait_for(&launched), "the launcher's app starts");
    use std::os::unix::fs::PermissionsExt as _;
    assert_ne!(fs::metadata(fixture.root.join("tool.desktop")).unwrap().permissions().mode() & 0o100, 0);
    // Trusted now, it starts without a question.
    fs::remove_file(&launched).unwrap();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.confirm.is_none()));
    assert!(wait_for(&launched));
}


#[gpui::test]
fn trash_goes_without_asking_and_a_deletion_names_one_item_or_lists_several(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt", "b.txt", "c.txt"]);
    click(cx, "row-0-0");
    cx.simulate_keystrokes("delete");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.confirm.is_none(), "a move to Trash can be undone, so it does not ask"));
    assert!(!fixture.root.join("a.txt").exists());
    assert_eq!(notices(&view, cx), ["Moved 1 item to Trash"]);
    click(cx, "row-0-0");
    cx.simulate_keystrokes("shift-delete");
    view.read_with(cx, |workspace, _| {
        let confirm = workspace.confirm.as_ref().unwrap();
        assert_eq!(confirm.message.as_ref(), "“b.txt” will be deleted permanently. This cannot be undone.");
        assert!(confirm.items.is_empty());
    });
    cx.simulate_keystrokes("escape ctrl-a shift-delete");
    view.read_with(cx, |workspace, _| {
        let confirm = workspace.confirm.as_ref().unwrap();
        assert_eq!(confirm.message.as_ref(), "2 items will be deleted permanently. This cannot be undone.");
        let names: Vec<&str> = confirm.items.iter().map(|name| name.as_ref()).collect();
        assert_eq!(names, ["b.txt", "c.txt"]);
    });
}

#[gpui::test]
fn select_by_pattern_and_invert(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.jpg", "b.JPG", "c.txt", "notes.md"]);
    let selected = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        let mut names: Vec<String> = view.read_with(cx, |workspace, _| workspace.pane().selected_items().into_iter().map(|item| item.name).collect());
        names.sort();
        names
    };
    cx.simulate_keystrokes("ctrl-s");
    view.read_with(cx, |workspace, app| {
        assert_eq!(workspace.operation_dialog.as_ref().map(|dialog| dialog.title()), Some("Select by Pattern"));
        assert_eq!(workspace.dialog_field.read(app).text(), "*");
    });
    // The offered pattern is selected, so typing replaces it.
    cx.simulate_keystrokes("* . j p g enter");
    assert_eq!(selected(&view, cx), ["a.jpg", "b.JPG"], "case is ignored");
    view.read_with(cx, |workspace, _| assert!(workspace.operation_dialog.is_none()));

    cx.simulate_keystrokes("ctrl-i");
    assert_eq!(selected(&view, cx), ["c.txt", "notes.md"]);

    // Nothing matching keeps the dialog, and the selection, as they were.
    cx.simulate_keystrokes("ctrl-s z z z enter");
    view.read_with(cx, |workspace, _| {
        assert!(workspace.operation_dialog.is_some());
        assert_eq!(workspace.dialog_error.as_deref(), Some("Nothing here matches “zzz”"));
    });
    assert_eq!(selected(&view, cx), ["c.txt", "notes.md"]);
    // Plain text matches anywhere in the name.
    cx.simulate_keystrokes("ctrl-a o t e enter");
    assert_eq!(selected(&view, cx), ["notes.md"]);
}

#[gpui::test]
fn shift_delete_deletes_for_good_after_asking(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt", "docs/b.txt"]);
    // Sorted by name: docs, a.txt.
    click(cx, "row-0-1");
    cx.simulate_keystrokes("shift-delete");
    view.read_with(cx, |workspace, _| {
        let confirm = workspace.confirm.as_ref().expect("always asks, even with confirmation off");
        assert_eq!(confirm.title.as_ref(), "Delete Permanently");
        assert_eq!(confirm.message.as_ref(), "“a.txt” will be deleted permanently. This cannot be undone.");
    });
    click(cx, "confirm-accept");
    cx.run_until_parked();
    assert!(!fixture.root.join("a.txt").exists());
    assert!(names(&view, cx) == ["docs"]);

    // With the Trash page showing, the folder behind it is not what Delete acts on.
    click(cx, "row-0-0");
    view.update(cx, |workspace, cx| workspace.open_trash(cx));
    cx.simulate_keystrokes("delete");
    cx.simulate_keystrokes("shift-delete");
    view.read_with(cx, |workspace, _| assert!(workspace.confirm.is_none()));
    assert!(fixture.root.join("docs").is_dir());
}

#[gpui::test]
fn a_long_notice_wraps_inside_its_toast(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    let message = "Could not open 482 GB Volume: could not mount: org.freedesktop.UDisks2.Error.NotAuthorizedCanObtain: Not authorized to perform operation";
    view.update(cx, |workspace, cx| workspace.report(Err(message.into()), cx));
    settle(cx);
    let text = cx.debug_bounds("toast-message").unwrap();
    assert!(text.size.width <= gpui::px(380.), "the message stays in the 380px toast: {text:?}");
    assert!(text.size.height > gpui::px(40.), "and wraps onto more lines: {text:?}");
}

#[gpui::test]
fn a_select_opens_its_list_right_under_it(cx: &mut TestAppContext) {
    let (_view, cx, _fixture) = open(cx, &["a.txt"]);
    cx.simulate_keystrokes("ctrl-,");
    click(cx, "settings-appearance");
    click(cx, "setting-background");
    let control = cx.debug_bounds("setting-background").unwrap();
    let first = cx.debug_bounds("setting-background-0").unwrap();
    let gap = first.top() - control.bottom();
    assert!(gap >= gpui::px(0.) && gap < gpui::px(12.), "the list starts just below the control, {gap:?} away");
}

#[gpui::test]
fn a_transparent_background_replaces_the_wallpaper(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    cx.simulate_keystrokes("ctrl-,");
    click(cx, "settings-appearance");
    assert!(cx.debug_bounds("wallpaper-choose").is_none(), "no wallpaper options without a wallpaper");
    assert!(cx.debug_bounds("accent-source").is_none(), "nor an accent taken from one");
    click(cx, "setting-background");
    click(cx, "setting-background-2");
    settle(cx);
    view.read_with(cx, |workspace, _| {
        assert!(workspace.desktop_settings.transparent);
        assert_eq!(workspace.window_transparent, Some(true), "the window was told to draw see-through");
        assert_eq!(workspace.window_background().a, 0.);
        assert!(workspace.shell_background().a < 1., "panels are tinted glass over the desktop");
    });
    assert!(DesktopSettings::load().transparent, "the choice is saved");
    click(cx, "setting-background");
    click(cx, "setting-background-0");
    settle(cx);
    view.read_with(cx, |workspace, _| assert_eq!(workspace.window_transparent, Some(false)));
}

#[gpui::test]
fn bookmarks_of_places_are_not_listed_twice(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["Desktop/", "work/"]);
    let bookmarks = fixture.root.with_extension("config").join("gtk-3.0/bookmarks");
    fs::create_dir_all(bookmarks.parent().unwrap()).unwrap();
    let uri = |name: &str| crate::os_clipboard::file_uri(&fixture.root.join(name));
    fs::write(&bookmarks, format!("{}\n{}\n", uri("Desktop"), uri("work"))).unwrap();
    view.update(cx, |workspace, cx| workspace.load_bookmarks(cx));
    cx.run_until_parked();
    settle(cx);
    // Desktop is a place, so its bookmark is not repeated under Favorites; work is listed.
    assert!(cx.debug_bounds("favorite-row-1").is_some());
    // This window never drew the Desktop bookmark's row, so its bounds were never recorded.
    assert!(cx.debug_bounds("favorite-row-0").is_none());
    view.read_with(cx, |workspace, _| assert_eq!(workspace.favorites.entries.len(), 2, "both stay bookmarked"));
    // The file keeps both.
    assert_eq!(fs::read_to_string(&bookmarks).unwrap().lines().count(), 2);
}

#[gpui::test]
fn pressing_an_open_select_closes_it(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt"]);
    cx.simulate_keystrokes("ctrl-,");
    click(cx, "settings-appearance");
    click(cx, "setting-background");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.open_select, Some("setting-background")));
    click(cx, "setting-background");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.open_select, None, "a second press closes the list"));
    click(cx, "setting-background");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.open_select, Some("setting-background"), "and a third opens it again"));
    // A press elsewhere closes it, and the next press on the control opens it.
    click(cx, "settings-appearance");
    cx.run_until_parked();
    click(cx, "setting-background");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.open_select, Some("setting-background")));
}

fn selected_names(view: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    let mut names: Vec<String> = view.read_with(cx, |workspace, _| workspace.pane().selected_items().into_iter().map(|item| item.name).collect());
    names.sort();
    names
}

#[gpui::test]
fn a_band_from_empty_space_selects_the_rows_it_crosses(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt", "b.txt", "c.txt", "d.txt"]);
    let pane = cx.debug_bounds("browser-pane-0").unwrap();
    let empty = gpui::point(pane.center().x, pane.bottom() - gpui::px(60.));
    let second = cx.debug_bounds("row-0-1").unwrap().center();
    let band = |cx: &mut VisualTestContext, modifiers: Modifiers| {
        cx.simulate_mouse_down(empty, MouseButton::Left, modifiers);
        cx.simulate_mouse_move(empty - gpui::point(gpui::px(0.), gpui::px(10.)), Some(MouseButton::Left), modifiers);
        cx.simulate_mouse_move(second, Some(MouseButton::Left), modifiers);
        cx.simulate_mouse_up(second, MouseButton::Left, modifiers);
        cx.run_until_parked();
    };
    band(cx, Modifiers::default());
    assert_eq!(selected_names(&view, cx), ["b.txt", "c.txt", "d.txt"]);
    view.read_with(cx, |workspace, _| assert!(workspace.band.is_none()));

    // A click on empty space clears the selection; Ctrl+drag adds to it.
    click(cx, "row-0-0");
    cx.simulate_mouse_down(empty, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(empty, MouseButton::Left, Modifiers::default());
    assert!(selected_names(&view, cx).is_empty());
    click(cx, "row-0-0");
    band(cx, Modifiers { control: true, ..Default::default() });
    assert_eq!(selected_names(&view, cx), ["a.txt", "b.txt", "c.txt", "d.txt"]);

    // A press on a row is the row's: it selects it, and starts no band.
    let first = cx.debug_bounds("row-0-0").unwrap().center();
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::default());
    view.read_with(cx, |workspace, _| assert!(workspace.band.is_none()));
    cx.simulate_mouse_up(first, MouseButton::Left, Modifiers::default());
    assert_eq!(selected_names(&view, cx), ["a.txt"]);
}

#[gpui::test]
fn zoom_steps_the_view_and_ctrl_zero_resets_it(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt", "b.txt"]);
    let height = |cx: &mut VisualTestContext| f32::from(cx.debug_bounds("row-0-0").unwrap().size.height);
    assert_eq!(height(cx), 32.);
    cx.simulate_keystrokes("ctrl-= ctrl-=");
    cx.run_until_parked();
    assert_eq!(height(cx), 48.);
    cx.simulate_keystrokes("ctrl-=");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.preferences.list_zoom, 3, "the largest step holds"));
    cx.simulate_keystrokes("ctrl-0 ctrl--");
    cx.run_until_parked();
    assert_eq!(height(cx), 28.);

    // Ctrl+wheel up over the list zooms in instead of scrolling; the grid keeps its own step.
    let row = cx.debug_bounds("row-0-0").unwrap().center();
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: row,
        delta: gpui::ScrollDelta::Lines(gpui::point(0., 1.)),
        modifiers: Modifiers { control: true, ..Default::default() },
        touch_phase: gpui::TouchPhase::Moved,
    });
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.preferences.list_zoom, 1);
        assert_eq!(workspace.preferences.grid_zoom, 1);
    });

    // Zooming the grid keeps the grid.
    view.update(cx, |workspace, _| workspace.pane_mut().set_view_mode(ViewMode::Grid));
    cx.simulate_keystrokes("ctrl-=");
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.preferences.grid_zoom, 2);
        assert_eq!(workspace.pane().current_view_mode(), ViewMode::Grid);
    });
}

#[gpui::test]
fn a_folder_keeps_the_view_picked_in_it(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["photos/a.jpg", "photos/trip/b.jpg", "notes.txt"]);
    let mode = |view: &Entity<Workspace>, cx: &mut VisualTestContext| view.read_with(cx, |workspace, _| workspace.pane().current_view_mode());
    view.update(cx, |workspace, cx| workspace.navigate("home/photos".into(), cx));
    cx.run_until_parked();
    click(cx, "view-grid");
    assert_eq!(mode(&view, cx), ViewMode::Grid);

    // A folder never set opens in the default view, and its parent keeps its own.
    view.update(cx, |workspace, cx| workspace.navigate("home/photos/trip".into(), cx));
    cx.run_until_parked();
    assert_eq!(mode(&view, cx), ViewMode::List);
    cx.simulate_keystrokes("alt-left");
    cx.run_until_parked();
    assert_eq!(mode(&view, cx), ViewMode::Grid);
    cx.simulate_keystrokes("alt-up");
    cx.run_until_parked();
    assert_eq!(mode(&view, cx), ViewMode::List);

    // The next window opens the folder in its view: the choice is written to disk.
    let api = Api::local_with_root(fixture.root.clone()).unwrap();
    let kept = super::folder_views::FolderViews::load();
    assert_eq!(kept.mode(&api, "home/photos", ViewMode::List), ViewMode::Grid);
    assert_eq!(kept.mode(&api, "home", ViewMode::List), ViewMode::List);

    // Picking the default again forgets the folder.
    view.update(cx, |workspace, cx| workspace.navigate("home/photos".into(), cx));
    cx.run_until_parked();
    click(cx, "view-list");
    cx.run_until_parked();
    assert_eq!(super::folder_views::FolderViews::load().encode(), b"{}");
}

#[gpui::test]
fn one_view_serves_every_folder_when_views_are_not_kept_per_folder(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["photos/a.jpg", "music/b.txt"]);
    let mode = |view: &Entity<Workspace>, cx: &mut VisualTestContext| view.read_with(cx, |workspace, _| workspace.pane().current_view_mode());
    view.update(cx, |workspace, cx| {
        workspace.preferences.view_per_folder = false;
        workspace.apply_preferences(cx);
        workspace.navigate("home/photos".into(), cx);
    });
    cx.run_until_parked();
    click(cx, "view-grid");
    view.update(cx, |workspace, cx| workspace.navigate("home/music".into(), cx));
    cx.run_until_parked();
    assert_eq!(mode(&view, cx), ViewMode::Grid, "picked in one folder, kept in the next");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.preferences.default_view, ViewMode::Grid, "and kept for the next window"));

    // The Overview keeps its own view, also when a setting is saved there.
    view.update(cx, |workspace, cx| workspace.navigate(String::new(), cx));
    cx.run_until_parked();
    click(cx, "view-list");
    view.update(cx, |workspace, cx| workspace.apply_preferences(cx));
    assert_eq!(mode(&view, cx), ViewMode::List);
    view.update(cx, |workspace, cx| workspace.navigate("home/photos".into(), cx));
    cx.run_until_parked();
    assert_eq!(mode(&view, cx), ViewMode::Grid, "the Overview's view is not the folders'");
}

#[gpui::test]
fn the_overview_keeps_its_view_when_a_setting_is_saved(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["notes.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate(String::new(), cx));
    cx.run_until_parked();
    let mode = |view: &Entity<Workspace>, cx: &mut VisualTestContext| view.read_with(cx, |workspace, _| workspace.pane().current_view_mode());
    assert_eq!(mode(&view, cx), ViewMode::Grid);
    // Opening the Details panel saves the settings; the drives stay as cards.
    cx.simulate_keystrokes("f11");
    cx.run_until_parked();
    assert_eq!(mode(&view, cx), ViewMode::Grid);
}

#[gpui::test]
fn a_folder_keeps_the_sort_picked_in_it(cx: &mut TestAppContext) {
    use crate::sort::{Sort, SortDir, SortField};
    let (view, cx, fixture) = open(cx, &["docs/a.txt", "docs/b.txt", "music/c.txt", "music/d.txt"]);
    let sort = |view: &Entity<Workspace>, cx: &mut VisualTestContext| view.read_with(cx, |workspace, _| workspace.pane().sort);
    let by_name_desc = Sort { field: SortField::Name, dir: SortDir::Desc };
    view.update(cx, |workspace, cx| workspace.navigate("home/docs".into(), cx));
    cx.run_until_parked();
    view.update(cx, |workspace, cx| workspace.choose_sort(PaneTarget(0), by_name_desc, cx));
    settle(cx);
    assert_eq!(names(&view, cx), ["b.txt", "a.txt"]);

    // Another folder opens with the default sort; coming back restores the folder's own.
    view.update(cx, |workspace, cx| workspace.navigate("home/music".into(), cx));
    settle(cx);
    assert_eq!(names(&view, cx), ["c.txt", "d.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate("home/docs".into(), cx));
    settle(cx);
    assert_eq!(sort(&view, cx), by_name_desc);
    assert_eq!(names(&view, cx), ["b.txt", "a.txt"]);

    // A new default in Settings leaves the folder's sort alone.
    view.update(cx, |workspace, cx| {
        workspace.preferences.default_sort = SortField::Size;
        workspace.apply_preferences(cx);
    });
    settle(cx);
    assert_eq!(sort(&view, cx), by_name_desc);

    // Kept on disk for the next window; choosing the default forgets it.
    let api = Api::local_with_root(fixture.root.clone()).unwrap();
    let default = Sort { field: SortField::Size, dir: SortDir::Asc };
    assert_eq!(super::folder_views::FolderViews::load().sort(&api, "home/docs", default), by_name_desc);
    view.update(cx, |workspace, cx| workspace.choose_sort(PaneTarget(0), default, cx));
    cx.run_until_parked();
    assert_eq!(super::folder_views::FolderViews::load().encode(), b"{}");
}

#[gpui::test]
fn a_drive_keeps_the_name_given_it(cx: &mut TestAppContext) {
    use super::drive_names::{DriveKey, DriveNames};
    let (view, cx, fixture) = open(cx, &["a.txt"]);
    let api = Api::local_with_root(fixture.root.clone()).unwrap();
    let key = DriveKey::root(&api, "home");
    let shown = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        view.read_with(cx, |workspace, _| {
            let card = workspace.storage().unwrap().iter().map(|entry| workspace.drive_info(entry).name.to_string()).find(|name| name != "Desktop");
            (workspace.drive_names.name(&DriveKey::root(&workspace.home.api, "home"), "own").to_string(), card)
        })
    };

    // Rename from the sidebar's menu: the field starts with the drive's name, selected.
    let home = cx.debug_bounds("sidebar-drive-1").unwrap().center();
    cx.simulate_mouse_down(home, MouseButton::Right, Modifiers::default());
    assert!(cx.debug_bounds("drive-reset-name").is_none(), "a drive with its own name has nothing to reset");
    click(cx, "drive-rename");
    settle(cx);
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.dialog_field.read(cx).text(), "Home"));
    cx.simulate_keystrokes("w o r k enter");
    settle(cx);
    assert_eq!(shown(&view, cx), ("work".to_string(), Some("work".to_string())));
    assert_eq!(DriveNames::load().name(&key, "home"), "work", "kept for the next window");

    // The path bar starts with the new name too.
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.breadcrumbs(PaneTarget(0))[0].label, "work"));

    // Reset Name gives the drive its own name back and forgets the given one.
    let home = cx.debug_bounds("sidebar-drive-1").unwrap().center();
    cx.simulate_mouse_down(home, MouseButton::Right, Modifiers::default());
    click(cx, "drive-reset-name");
    cx.run_until_parked();
    assert_eq!(shown(&view, cx).0, "own");
    assert_eq!(DriveNames::load().name(&key, "home"), "home");
}

#[gpui::test]
fn properties_change_permissions(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let (view, cx, fixture) = open(cx, &["docs/inner/a.sh", "docs/b.txt", "c.txt"]);
    let mode = |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o7777;
    set_mode(&fixture.root.join("c.txt"), 0o644);
    let c = names(&view, cx).iter().position(|name| name == "c.txt").unwrap();
    click(cx, Box::leak(format!("row-0-{c}").into_boxed_str()));
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    // A text file cannot run, so it offers no Run.
    assert!(cx.debug_bounds("permission-100").is_none());
    // Group may write, then others may not read: each click is a chmod.
    click(cx, "permission-20");
    cx.run_until_parked();
    assert_eq!(mode(&fixture.root.join("c.txt")), 0o664);
    settle(cx);
    click(cx, "permission-4");
    cx.run_until_parked();
    assert_eq!(mode(&fixture.root.join("c.txt")), 0o660);
    view.read_with(cx, |workspace, _| {
        let Some(Load::Loaded(ownership)) = &workspace.properties.as_ref().unwrap().ownership else { panic!("read back") };
        assert_eq!(ownership.mode & 0o7777, 0o660);
    });
    cx.simulate_keystrokes("escape");

    // A folder's mode reaches everything inside; files keep whether they run.
    set_mode(&fixture.root.join("docs/inner/a.sh"), 0o755);
    set_mode(&fixture.root.join("docs/b.txt"), 0o644);
    set_mode(&fixture.root.join("docs"), 0o750);
    let docs = names(&view, cx).iter().position(|name| name == "docs").unwrap();
    click(cx, Box::leak(format!("row-0-{docs}").into_boxed_str()));
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    click(cx, "permissions-apply-inside");
    cx.run_until_parked();
    assert_eq!(mode(&fixture.root.join("docs/inner")), 0o750);
    assert_eq!(mode(&fixture.root.join("docs/inner/a.sh")), 0o750);
    assert_eq!(mode(&fixture.root.join("docs/b.txt")), 0o640);
}

/// Transfers run on a thread of their own; this waits until the window has
/// heard the last one finish.
pub(super) fn wait_for_transfers(view: &Entity<Workspace>, cx: &mut VisualTestContext) {
    for _ in 0..500 {
        cx.run_until_parked();
        let running = view.read_with(cx, |workspace, _| workspace.operation_status.is_busy() || workspace.jobs.iter().any(|job| matches!(job.state, super::jobs::JobState::Running)));
        if !running {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("a transfer did not finish");
}

#[gpui::test]
fn a_paste_asks_about_a_taken_name(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt", "b.txt", "dst/a.txt", "dst/b.txt"]);
    // Copy both files, then paste into a folder that has both.
    view.update(cx, |workspace, _| {
        let files: Vec<String> = workspace.pane().items.iter().filter(|item| !item.is_dir).map(|item| item.path.clone()).collect();
        workspace.pane_mut().selected = files.into_iter().collect();
    });
    cx.simulate_keystrokes("ctrl-c");
    view.update(cx, |workspace, cx| workspace.navigate("home/dst".into(), cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-v");
    let asked = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        for _ in 0..300 {
            cx.run_until_parked();
            if view.read_with(cx, |workspace, _| workspace.transfer_question.is_some()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the transfer did not ask");
    };
    asked(&view, cx);
    view.read_with(cx, |workspace, _| {
        let pending = workspace.transfer_question.as_ref().unwrap();
        assert!(matches!(&pending.question, crate::transfer::Question::Conflict { name, .. } if name == "a.txt"));
        assert!(pending.more, "a second item follows, so Apply to all is offered");
    });
    settle(cx);
    // Rename for this one and the rest.
    click(cx, "transfer-apply-all");
    click(cx, "transfer-rename");
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("dst/a (copy 1).txt").is_file());
    assert!(fixture.root.join("dst/b (copy 1).txt").is_file());
    assert_eq!(fs::read_to_string(fixture.root.join("dst/a.txt")).unwrap(), "dst/a.txt");
}

#[gpui::test]
fn a_drag_opens_folders_it_rests_on_and_links_or_asks(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt", "b.txt", "docs/inner/"]);
    let center = |cx: &mut VisualTestContext, selector: &'static str| cx.debug_bounds(selector).unwrap().center();
    let row_of = |view: &Entity<Workspace>, cx: &mut VisualTestContext, name: &str| -> &'static str {
        let index = names(view, cx).iter().position(|found| found == name).unwrap();
        Box::leak(format!("row-0-{index}").into_boxed_str())
    };
    let (a, docs) = (row_of(&view, cx, "a.txt"), row_of(&view, cx, "docs"));
    let (start, folder) = (center(cx, a), center(cx, docs));

    // Rest on the docs row: it opens, and the drag goes on inside it.
    let link = Modifiers { control: true, shift: true, ..Default::default() };
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(start + gpui::point(gpui::px(10.), gpui::px(10.)), Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(folder, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(folder + gpui::point(gpui::px(1.), gpui::px(0.)), Some(MouseButton::Left), Modifiers::default());
    cx.update(|_, _| {});
    cx.executor().advance_clock(std::time::Duration::from_millis(800));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/docs", "resting on a folder opens it"));

    // Ctrl+Shift on release: a link to the file, by its full path.
    let pane = cx.debug_bounds("browser-pane-0").unwrap();
    let empty = gpui::point(pane.center().x, pane.bottom() - gpui::px(60.));
    cx.simulate_mouse_move(empty, Some(MouseButton::Left), link);
    cx.update(|_, _| {});
    cx.simulate_mouse_up(empty, MouseButton::Left, link);
    cx.run_until_parked();
    assert_eq!(fs::read_link(fixture.root.join("docs/a.txt")).unwrap(), fixture.root.join("a.txt"));

    // Alt asks at the drop; Copy Here copies.
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    let (b, docs) = (row_of(&view, cx, "b.txt"), row_of(&view, cx, "docs"));
    let (start, folder) = (center(cx, b), center(cx, docs));
    let ask = Modifiers { alt: true, ..Default::default() };
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(start + gpui::point(gpui::px(10.), gpui::px(10.)), Some(MouseButton::Left), ask);
    cx.simulate_mouse_move(folder, Some(MouseButton::Left), ask);
    cx.simulate_mouse_move(folder + gpui::point(gpui::px(1.), gpui::px(0.)), Some(MouseButton::Left), ask);
    cx.update(|_, _| {});
    cx.simulate_mouse_up(folder, MouseButton::Left, ask);
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.drop_menu.is_some(), "Alt asks what to do"));
    click(cx, "drop-copy");
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("docs/b.txt").is_file());
    assert!(fixture.root.join("b.txt").is_file(), "a copy leaves the original");
}

#[gpui::test]
fn a_drag_near_the_bottom_scrolls_the_list(cx: &mut TestAppContext) {
    let files: Vec<String> = (0..80).map(|index| format!("file-{index:02}.txt")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (view, cx, _fixture) = open(cx, &files);
    let offset = |view: &Entity<Workspace>, cx: &mut VisualTestContext| view.read_with(cx, |workspace, _| f32::from(workspace.pane().list_scroll.0.borrow().base_handle.offset().y));
    let start = cx.debug_bounds("row-0-0").unwrap().center();
    let viewport = view.read_with(cx, |workspace, _| workspace.pane().list_scroll.0.borrow().base_handle.bounds());
    let near_bottom = gpui::point(start.x, viewport.bottom() - gpui::px(8.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(start + gpui::point(gpui::px(10.), gpui::px(10.)), Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(near_bottom, Some(MouseButton::Left), Modifiers::default());
    cx.update(|_, _| {});
    assert_eq!(offset(&view, cx), 0.);
    for _ in 0..10 {
        cx.executor().advance_clock(std::time::Duration::from_millis(16));
        cx.run_until_parked();
    }
    assert!(offset(&view, cx) < 0., "held near the bottom edge, the list scrolls down");
    cx.simulate_mouse_up(near_bottom, MouseButton::Left, Modifiers::default());
}

// F11 shows Details for the selection, as Dolphin's Information panel does, and hides it again.
#[gpui::test]
fn f11_shows_details_of_the_selection(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt", "b.txt"]);
    assert!(cx.debug_bounds("details-panel").is_none());
    cx.simulate_keystrokes("f11");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.preferences.details_panel));
    click(cx, "row-0-0");
    assert!(cx.debug_bounds("details-panel").is_some());
    cx.simulate_keystrokes("f11");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(!workspace.preferences.details_panel));
}

#[gpui::test]
fn clicks_in_the_second_pane_select_there(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt", "b.txt"]);
    cx.simulate_keystrokes("f3");
    cx.run_until_parked();
    let second = view.read_with(cx, |workspace, _| workspace.split_panes.values().next().unwrap().id);
    let row = cx.debug_bounds(Box::leak(format!("row-{second}-1").into_boxed_str())).expect("the second pane's row");
    cx.simulate_click(row.center(), Modifiers::default());
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.active_pane.0, second);
        assert_eq!(workspace.pane_by_id(second).unwrap().selected.len(), 1);
        assert!(workspace.tabs[0].selected.is_empty(), "the first pane keeps its selection");
    });
}

#[gpui::test]
fn permission_clicks_are_all_written_after_properties_closes(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt as _;
    let (view, cx, fixture) = open(cx, &["a.txt"]);
    let file = fixture.root.join("a.txt");
    set_mode(&file, 0o644);
    view.update(cx, |workspace, cx| {
        let item = workspace.pane().items[0].clone();
        workspace.show_properties(PaneTarget(0), vec![item], cx);
    });
    cx.run_until_parked();
    // The second click waits for the first write; then the dialog closes.
    view.update(cx, |workspace, cx| {
        workspace.toggle_permission(0o040, cx);
        workspace.toggle_permission(0o004, cx);
        workspace.properties = None;
    });
    cx.run_until_parked();
    assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}

#[gpui::test]
fn wallpaper_options_show_only_while_the_wallpaper_does(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt"]);
    let wallpaper = fixture.root.join("wallpaper.png");
    image::RgbImage::from_pixel(64, 64, image::Rgb([200, 40, 30])).save(&wallpaper).unwrap();
    cx.simulate_keystrokes("ctrl-,");
    click(cx, "settings-appearance");
    let default_accent = view.read_with(cx, |workspace, _| workspace.accent_hex());
    cx.update(|_, app| view.update(app, |workspace, cx| workspace.install_wallpaper(wallpaper, cx)));
    cx.run_until_parked();
    settle(cx);
    assert!(cx.debug_bounds("setting-wallpaper-fit").is_some());
    assert!(cx.debug_bounds("accent-source").is_some());
    let wallpaper_accent = view.read_with(cx, |workspace, _| workspace.accent_hex());
    assert_ne!(wallpaper_accent, default_accent, "the accent follows the wallpaper");
    // Transparent hides the wallpaper, so its options and its accent go too.
    click(cx, "setting-background");
    click(cx, "setting-background-2");
    settle(cx);
    view.read_with(cx, |workspace, _| assert!(workspace.desktop_settings.transparent));
    // The test platform keeps bounds from earlier frames, so the new row proves the redraw.
    assert!(cx.debug_bounds("accent-color-input").is_some(), "the accent is a plain color now");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.accent_hex(), default_accent));
    // Back to Wallpaper: the kept image returns without a picker.
    click(cx, "setting-background");
    click(cx, "setting-background-1");
    settle(cx);
    view.read_with(cx, |workspace, _| {
        assert!(!workspace.desktop_settings.transparent);
        assert_eq!(workspace.accent_hex(), wallpaper_accent);
    });
}

#[gpui::test]
fn custom_actions_run_on_their_files_and_terminal_ones_give_way(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["notes.txt", "work/"]);
    // `open` points XDG_CONFIG_HOME at the fixture's own config folder.
    let thunar = fixture.root.with_extension("config").join("Thunar");
    fs::create_dir_all(&thunar).unwrap();
    fs::write(
        thunar.join("uca.xml"),
        r#"<actions>
<action><name>Open Terminal Here</name><command>kitty --directory %f</command><patterns>*</patterns><directories/></action>
<action><name>Mark</name><command>sh -c 'touch "$1.marked"' sh %f</command><patterns>*.txt</patterns><text-files/></action>
</actions>"#,
    )
    .unwrap();
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    assert_eq!(names(&view, cx), ["work", "notes.txt"]);

    // A folder's menu has BoxBox's own terminal row, so the Thunar one is left out.
    menu_on(cx, "row-0-0");
    cx.run_until_parked();
    assert!(cx.debug_bounds("context-terminal").is_some());
    assert!(cx.debug_bounds("context-action-0").is_none(), "the terminal action repeats Open in Terminal");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();

    menu_on(cx, "row-0-1");
    cx.run_until_parked();
    click(cx, "context-action-0");
    let marker = fixture.root.join("notes.txt.marked");
    for _ in 0..100 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(marker.is_file(), "the action ran on the file");
    view.read_with(cx, |workspace, _| assert!(workspace.context_menu.is_none()));
}

#[gpui::test]
fn hover_cards_follow_settings_and_give_way_to_menus(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["notes.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate("home".into(), cx));
    cx.run_until_parked();
    let card = |cx: &mut VisualTestContext| {
        view.update(cx, |workspace, cx| {
            let item = workspace.pane().items[0].clone();
            workspace.hover_card(PaneTarget(0), &item, cx).is_some()
        })
    };
    assert!(card(cx), "on by default");
    view.update(cx, |workspace, cx| {
        workspace.preferences.hover_details = false;
        workspace.commit_preferences(cx);
    });
    assert!(!card(cx), "Settings turn it off");
    view.update(cx, |workspace, cx| {
        workspace.preferences.hover_details = true;
        workspace.preferences.animations = false;
        workspace.commit_preferences(cx);
    });
    assert!(card(cx));
    cx.update(|_, app| assert!(!crate::components::motion::enabled(app), "Animations off reaches every component"));
}

#[gpui::test]
fn an_open_dialog_keeps_the_folders_keys_from_acting_behind_it(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a.txt"]);
    click(cx, "row-0-0");
    cx.simulate_keystrokes("f1");
    view.read_with(cx, |workspace, _| assert!(workspace.shortcuts_open));
    cx.simulate_keystrokes("delete ctrl-a f2");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert!(workspace.confirm.is_none() && workspace.operation_dialog.is_none());
        assert!(workspace.shortcuts_open);
    });
    assert!(fixture.root.join("a.txt").is_file());
    // Escape still closes the dialog, and the keys work again.
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |workspace, _| assert!(!workspace.shortcuts_open));
    cx.simulate_keystrokes("delete");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.confirm.is_some() || !fixture.root.join("a.txt").exists()));
}

#[gpui::test]
fn clicking_the_path_bar_edits_the_real_path_and_a_typed_real_path_opens(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/a.txt"]);
    let pane = view.read_with(cx, |workspace, _| workspace.pane().id);
    click(cx, Box::leak(format!("path-bar-space-{pane}").into_boxed_str()));
    let real = fs::canonicalize(&fixture.root).unwrap().display().to_string();
    view.read_with(cx, |workspace, cx| {
        assert!(workspace.location_editor.is_some());
        assert_eq!(workspace.location_field.read(cx).text(), real, "the field holds the real path");
    });
    // All of it is selected, so typing replaces it.
    cx.simulate_input(&format!("{real}/docs"));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert!(workspace.location_editor.is_none());
        assert_eq!(workspace.pane().path, "home/docs");
    });
}

#[gpui::test]
fn details_text_selects_and_copies(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["report.txt"]);
    view.update(cx, |workspace, cx| {
        workspace.preferences.details_panel = true;
        cx.notify();
    });
    click(cx, "row-0-0");
    cx.run_until_parked();
    let name = cx.debug_bounds("details-name").unwrap();
    let press = |cx: &mut VisualTestContext, position, click_count| {
        cx.simulate_event(gpui::MouseDownEvent { button: MouseButton::Left, position, modifiers: Modifiers::default(), click_count, first_mouse: false });
    };
    let release = |cx: &mut VisualTestContext, position| {
        cx.simulate_event(gpui::MouseUpEvent { button: MouseButton::Left, position, modifiers: Modifiers::default(), click_count: 1 });
    };
    // A drag from the left edge selects the start of the name.
    let (start, middle) = (gpui::point(name.left() + gpui::px(1.), name.center().y), name.center());
    press(cx, start, 1);
    cx.simulate_event(gpui::MouseMoveEvent { position: middle, pressed_button: Some(MouseButton::Left), modifiers: Modifiers::default() });
    release(cx, middle);
    let dragged = view.read_with(cx, |workspace, _| workspace.text_selection.as_ref().and_then(|selection| selection.selected()).map(str::to_owned));
    assert!(dragged.as_deref().is_some_and(|text| !text.is_empty() && "report.txt".starts_with(text)), "{dragged:?}");
    // A double-click takes it all, and Ctrl+C copies the text, not the file.
    press(cx, middle, 2);
    release(cx, middle);
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(cx.read_from_clipboard().and_then(|item| item.text()).as_deref(), Some("report.txt"));
    // Any other key or press lets it go.
    cx.simulate_keystrokes("down");
    view.read_with(cx, |workspace, _| assert!(workspace.text_selection.is_none()));
}

#[gpui::test]
fn tab_completes_a_typed_folder_like_a_terminal(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/a.txt", "downloads/b.txt", "music/c.txt"]);
    let real = fs::canonicalize(&fixture.root).unwrap().display().to_string();
    cx.simulate_keystrokes("ctrl-l");
    cx.simulate_input(&format!("{real}/m"));
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/music/")));
    // Two folders share "do": Tab fills that, then steps through them.
    cx.simulate_input("x");
    cx.simulate_keystrokes("backspace backspace backspace backspace backspace backspace backspace d");
    // The list waits for a pause in typing.
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.location_completion.as_ref().map(|completion| completion.names.clone()), Some(vec!["docs".to_string(), "downloads".to_string()])));
    cx.simulate_keystrokes("tab");
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/do")));
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/docs")));
    cx.simulate_keystrokes("tab");
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/downloads")));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/downloads"));
}

#[gpui::test]
fn tab_in_a_folder_that_cannot_be_read_lists_it_once(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/a.txt"]);
    let real = fs::canonicalize(&fixture.root).unwrap().display().to_string();
    cx.simulate_keystrokes("ctrl-l");
    cx.simulate_input(&format!("{real}/missing/d"));
    cx.simulate_keystrokes("tab");
    // Retrying a failed read would never park.
    cx.run_until_parked();
    view.read_with(cx, |workspace, cx| {
        assert!(workspace.location_completion.is_none());
        assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/missing/d"));
    });
}

#[gpui::test]
fn properties_text_selects_and_ctrl_c_copies_it_not_the_file(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["report.txt"]);
    click(cx, "row-0-0");
    // A cut the copied text replaces, so a later paste does not move the file.
    cx.simulate_keystrokes("ctrl-x");
    view.read_with(cx, |workspace, _| assert!(workspace.file_clipboard.is_some()));
    cx.simulate_keystrokes("alt-enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(workspace.properties.is_some()));
    let kind = cx.debug_bounds("properties-type").unwrap().center();
    cx.simulate_event(gpui::MouseDownEvent { button: MouseButton::Left, position: kind, modifiers: Modifiers::default(), click_count: 2, first_mouse: false });
    cx.simulate_event(gpui::MouseUpEvent { button: MouseButton::Left, position: kind, modifiers: Modifiers::default(), click_count: 2 });
    cx.simulate_keystrokes("ctrl-c");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert!(copied.as_deref().is_some_and(|text| !text.is_empty() && !text.contains("report.txt")), "{copied:?}");
    view.read_with(cx, |workspace, _| assert!(workspace.properties.is_some() && workspace.file_clipboard.is_none()));
}

#[gpui::test]
fn up_and_down_pick_folders_in_the_scrolling_list(cx: &mut TestAppContext) {
    let folders: Vec<String> = (0..12).map(|index| format!("f{index:02}/")).collect();
    let files: Vec<&str> = folders.iter().map(String::as_str).collect();
    let (view, cx, fixture) = open(cx, &files);
    let real = fs::canonicalize(&fixture.root).unwrap().display().to_string();
    cx.simulate_keystrokes("ctrl-l");
    cx.simulate_input(&format!("{real}/"));
    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    cx.simulate_keystrokes("up");
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/f11")));
    // The last folder is past the first eight rows, so the list scrolled to it.
    view.read_with(cx, |workspace, _| assert!(workspace.location_scroll.offset().y < gpui::px(0.)));
    cx.simulate_keystrokes("down down");
    view.read_with(cx, |workspace, cx| assert_eq!(workspace.location_field.read(cx).text(), format!("{real}/f01")));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/f01"));
}

#[gpui::test]
fn the_location_field_closes_when_its_pane_goes_elsewhere(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["vids/a.mp4", "top.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate("home/vids".into(), cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-l");
    view.read_with(cx, |workspace, cx| {
        assert!(workspace.location_editor.is_some());
        assert!(workspace.location_field.read(cx).text().ends_with("/vids"));
    });
    // Back, as the toolbar button or Alt+Left: the bar shows where the pane went.
    view.update_in(cx, |workspace, window, cx| workspace.back(&Back, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.pane().path, "home");
        assert!(workspace.location_editor.is_none(), "the field for vids closed");
        assert!(workspace.location_completion.is_none());
    });
    // Opened again, it holds the new folder.
    cx.simulate_keystrokes("ctrl-l");
    view.read_with(cx, |workspace, cx| assert!(!workspace.location_field.read(cx).text().ends_with("/vids")));
}

#[gpui::test]
fn a_place_shows_the_folders_that_hold_it_in_the_path_bar(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["Desktop/sub/a.txt"]);
    view.update(cx, |workspace, cx| workspace.navigate("desktop/sub".into(), cx));
    cx.run_until_parked();
    let crumbs = view.read_with(cx, |workspace, _| workspace.breadcrumbs(PaneTarget(0)).into_iter().map(|crumb| (crumb.label, crumb.path)).collect::<Vec<_>>());
    let expected = [("Home", "home"), ("Desktop", "desktop"), ("sub", "desktop/sub")].map(|(label, path)| (label.to_string(), path.to_string()));
    assert_eq!(crumbs, expected);
}

#[gpui::test]
fn a_drag_on_a_separator_drops_in_the_crumb_before_and_resting_on_back_goes_back(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["a/b/x.txt", "a/b/y.txt"]);
    for path in ["home/a", "home/a/b"] {
        view.update(cx, |workspace, cx| workspace.navigate(path.into(), cx));
        cx.run_until_parked();
    }
    drag(cx, "row-0-0", "crumb-separator-home/a");
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("a/x.txt").is_file(), "the separator after a takes drops for a");

    let back = cx.debug_bounds("nav-back").unwrap().center();
    let row = cx.debug_bounds("row-0-0").unwrap().center();
    cx.simulate_mouse_down(row, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(row + gpui::point(gpui::px(10.), gpui::px(10.)), Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(back, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(back + gpui::point(gpui::px(1.), gpui::px(0.)), Some(MouseButton::Left), Modifiers::default());
    cx.update(|_, _| {});
    cx.executor().advance_clock(std::time::Duration::from_millis(800));
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert_eq!(workspace.pane().path, "home/a", "resting on Back goes back"));
    // Back takes no drop itself.
    cx.simulate_mouse_up(back, MouseButton::Left, Modifiers::default());
    wait_for_transfers(&view, cx);
    assert!(fixture.root.join("a/b/y.txt").is_file());
}

#[gpui::test]
fn a_folder_dropped_on_favorites_is_pinned_and_a_file_is_not(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["proj/", "f.txt"]);
    let index = |name: &str| view.read_with(cx, |workspace, _| workspace.pane().items.iter().position(|item| item.name == name).unwrap());
    let (folder, file) = (index("proj"), index("f.txt"));
    drag(cx, Box::leak(format!("row-0-{file}").into_boxed_str()), "sidebar-favorites");
    view.read_with(cx, |workspace, _| assert!(workspace.favorites.entries.is_empty(), "a file is not pinned"));
    drag(cx, Box::leak(format!("row-0-{folder}").into_boxed_str()), "sidebar-favorites");
    view.read_with(cx, |workspace, _| {
        assert_eq!(workspace.favorites.entries.iter().map(|favorite| favorite.path.as_str()).collect::<Vec<_>>(), ["home/proj"]);
    });
    assert!(fixture.root.join("proj").is_dir() && fixture.root.join("f.txt").is_file(), "pinning moves nothing");
}

#[gpui::test]
fn a_folder_tile_shows_what_it_holds_pictures_first(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["photos/a.txt", "photos/b.png", "photos/z.jpg", "photos/.hidden.png", "empty/"]);
    view.update(cx, |workspace, cx| {
        // They draw on BoxBox's folder, not the icon theme's.
        workspace.preferences.file_icons = crate::workspace::IconStyle::Tinted;
        workspace.pane_mut().set_view_mode(ViewMode::Grid);
        cx.notify();
    });
    let peek = |view: &Entity<Workspace>, cx: &mut VisualTestContext, name: &str| {
        view.read_with(cx, |workspace, _| {
            let pane = workspace.pane();
            let folder = pane.items.iter().find(|item| item.name == name).unwrap();
            workspace.folder_peek(&pane.api, folder).map(|items| items.iter().map(|item| item.name.clone()).collect::<Vec<_>>())
        })
    };
    for _ in 0..500 {
        settle(cx);
        if peek(&view, cx, "photos").is_some() && peek(&view, cx, "empty").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(peek(&view, cx, "photos").unwrap(), ["b.png", "z.jpg", "a.txt"]);
    assert_eq!(peek(&view, cx, "empty").unwrap(), Vec::<String>::new());

    // A picture edited in place leaves its folder's time alone; listed again, the
    // folder reads its preview again.
    std::fs::write(fixture.root.join("photos/b.png"), b"edited in place").unwrap();
    let size = |view: &Entity<Workspace>, cx: &mut VisualTestContext| {
        view.read_with(cx, |workspace, _| {
            let pane = workspace.pane();
            let folder = pane.items.iter().find(|item| item.name == "photos").unwrap();
            workspace.folder_peek(&pane.api, folder).and_then(|items| items.first().map(|item| item.size))
        })
    };
    let before = size(&view, cx);
    view.update(cx, |workspace, cx| workspace.load_pane(workspace.active_pane, cx));
    for _ in 0..500 {
        // The reload put back the default view; tiles ask for their previews.
        view.update(cx, |workspace, cx| {
            workspace.pane_mut().set_view_mode(ViewMode::Grid);
            cx.notify();
        });
        settle(cx);
        if size(&view, cx) == Some(15) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_ne!(before, Some(15));
    assert_eq!(size(&view, cx), Some(15));
}

#[gpui::test]
fn a_customised_folder_shows_its_icon_and_colours_by_every_path(cx: &mut TestAppContext) {
    use crate::components::Emblem;
    let (view, cx, fixture) = open(cx, &["Desktop/sub/a.txt"]);
    view.update(cx, |workspace, cx| {
        workspace.preferences.file_icons = crate::workspace::IconStyle::Tinted;
        workspace.navigate("home/Desktop".into(), cx);
    });
    cx.run_until_parked();
    let style = |view: &Entity<Workspace>, cx: &mut VisualTestContext, path: &str| {
        view.read_with(cx, |workspace, _| {
            let api = workspace.pane().api.clone();
            let style = workspace.glyph_style(&api, &api.stat(path).unwrap());
            (style.tint, style.emblem)
        })
    };
    let plain = style(&view, cx, "home/Desktop/sub");
    menu_on(cx, "row-0-0");
    click(cx, "context-customize");
    cx.simulate_input("gamepad");
    click(cx, "emblem-gamepad-2");
    click(cx, "folder-color-22c55e");
    click(cx, "emblem-color-f5f5f4");
    // Nothing changes until Done.
    assert_eq!(style(&view, cx, "home/Desktop/sub"), plain);
    click(cx, "customize-done");
    let customised = (Some(0x22c55e), Some(Emblem { name: "gamepad-2", tint: Some(0xf5f5f4) }));
    assert_eq!(style(&view, cx, "home/Desktop/sub"), customised);
    assert_eq!(style(&view, cx, "desktop/sub"), customised, "the Desktop place reaches the same folder");
    // And so does a link to Desktop, once the folder is drawn through it.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(fixture.root.join("Desktop"), fixture.root.join("shortcut")).unwrap();
        view.update(cx, |workspace, cx| workspace.navigate("home/shortcut".into(), cx));
        for _ in 0..500 {
            settle(cx);
            if style(&view, cx, "home/shortcut/sub") == customised {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(style(&view, cx, "home/shortcut/sub"), customised, "a link reaches the same folder");
        view.update(cx, |workspace, cx| workspace.navigate("home/Desktop".into(), cx));
        cx.run_until_parked();
    }

    // The desktop's icon theme keeps its own folders: no look and no menu entry.
    view.update(cx, |workspace, cx| {
        workspace.preferences.file_icons = crate::workspace::IconStyle::System;
        cx.notify();
    });
    assert_eq!(style(&view, cx, "home/Desktop/sub").1, None);
    view.read_with(cx, |workspace, _| assert!(!workspace.folder_looks_shown(), "the menu offers no Customize Folder"));
    view.update(cx, |workspace, _| workspace.preferences.file_icons = crate::workspace::IconStyle::Tinted);

    menu_on(cx, "row-0-0");
    click(cx, "context-customize");
    click(cx, "customize-reset");
    click(cx, "customize-done");
    assert_eq!(style(&view, cx, "home/Desktop/sub"), plain);
}

#[gpui::test]
fn folder_previews_are_off_when_turned_off_or_with_theme_icons(cx: &mut TestAppContext) {
    use crate::workspace::IconStyle;
    for (previews, style) in [(false, IconStyle::Tinted), (true, IconStyle::System)] {
        let (view, cx, _fixture) = open(cx, &["photos/b.png"]);
        view.update(cx, |workspace, cx| {
            workspace.preferences.folder_previews = previews;
            workspace.preferences.file_icons = style;
            workspace.pane_mut().set_view_mode(ViewMode::Grid);
            cx.notify();
        });
        for _ in 0..20 {
            settle(cx);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        view.read_with(cx, |workspace, _| {
            let pane = workspace.pane();
            assert!(workspace.folder_peek(&pane.api, &pane.items[0]).is_none(), "nothing is read for a preview with {style:?}");
        });
    }
}
