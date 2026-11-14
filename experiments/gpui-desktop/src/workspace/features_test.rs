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
        view.update(app, |workspace, _| {
            workspace.preferences = Preferences { default_sort: crate::sort::SortField::Name, default_sort_direction: crate::sort::SortDir::Asc, ..Preferences::default() };
            workspace.pane_mut().sort = Sort { field: crate::sort::SortField::Name, dir: crate::sort::SortDir::Asc };
        })
    });
    cx.run_until_parked();
    (view, cx, Fixture { root, _config: config })
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
