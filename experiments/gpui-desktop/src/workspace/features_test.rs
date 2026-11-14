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
    click(cx, "sidebar-empty-trash");
    view.read_with(cx, |workspace, _| assert_eq!(workspace.confirm.as_ref().unwrap().title.as_ref(), "Empty Trash"));
    click(cx, "confirm-accept");
    cx.run_until_parked();
    view.read_with(cx, |workspace, _| assert!(!workspace.trash_full));
    assert_eq!(fs::read_dir(fixture.root.join(".local/share/Trash/files")).unwrap().count(), 0);
}
