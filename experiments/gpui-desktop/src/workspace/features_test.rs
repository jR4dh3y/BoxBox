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
        Workspace::new(api, Fonts { sans: "sans-serif".into(), mono: "monospace".into() }, "home".into(), window, cx)
    });
    cx.update(|_, app| {
        view.update(app, |workspace, cx| {
            workspace.preferences = Preferences { default_sort: crate::sort::SortField::Name, default_sort_direction: crate::sort::SortDir::Asc, ..Preferences::default() };
            workspace.set_pane_sort(PaneTarget(0), Sort { field: crate::sort::SortField::Name, dir: crate::sort::SortDir::Asc }, cx);
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
        crate::components::GridMetrics::new(false).columns(workspace.browser_width(window))
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
        assert_eq!(menu.places, ["home/a/b", "home/a", "home"]);
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
fn trash_takes_drops_shows_restored_items_and_empties(cx: &mut TestAppContext) {
    let (view, cx, fixture) = open(cx, &["docs/", "docs/a.txt", "b.txt"]);
    view.read_with(cx, |workspace, _| assert!(!workspace.trash_full));
    // Dropping a row onto Trash moves it there without a question.
    drag(cx, "row-0-1", "sidebar-trash");
    assert!(!fixture.root.join("b.txt").exists(), "the dropped file went to Trash");
    view.read_with(cx, |workspace, _| {
        assert!(workspace.confirm.is_none());
        assert!(workspace.trash_full, "the Trash icon shows it is full");
    });

    // Restoring offers to show the item where it went back to.
    click(cx, "sidebar-trash");
    cx.run_until_parked();
    click(cx, "restore-b.txt");
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
    click(cx, "confirm-accept");
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
    cx.run_until_parked();
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
    cx.run_until_parked();
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
    cx.run_until_parked();
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

    // A link sits beside its target and holds just its name.
    menu_on(cx, "row-0-3");
    click(cx, "context-link");
    cx.run_until_parked();
    let link = fixture.root.join("Link to x.txt");
    assert_eq!(fs::read_link(&link).unwrap(), std::path::PathBuf::from("x.txt"));

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
fn delete_names_one_item_or_counts_and_lists_several(cx: &mut TestAppContext) {
    let (view, cx, _fixture) = open(cx, &["a.txt", "b.txt"]);
    click(cx, "row-0-0");
    cx.simulate_keystrokes("delete");
    view.read_with(cx, |workspace, _| {
        let confirm = workspace.confirm.as_ref().unwrap();
        assert_eq!(confirm.message.as_ref(), "“a.txt” will be moved to Trash.");
        assert!(confirm.items.is_empty());
    });
    cx.simulate_keystrokes("escape ctrl-a delete");
    view.read_with(cx, |workspace, _| {
        let confirm = workspace.confirm.as_ref().unwrap();
        assert_eq!(confirm.message.as_ref(), "2 items will be moved to Trash.");
        let names: Vec<&str> = confirm.items.iter().map(|name| name.as_ref()).collect();
        assert_eq!(names, ["a.txt", "b.txt"]);
    });
}
