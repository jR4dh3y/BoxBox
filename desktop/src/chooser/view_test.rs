//! The dialog driven through its rendered window on a temporary folder.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use zbus::zvariant::Value;

use super::super::request::{Kind, Request, Response};
use super::Chooser;
use crate::sort::{Sort, SortDir, SortField};
use crate::workspace::parity_test::TestConfig;

struct Fixture {
    root: PathBuf,
    _config: TestConfig,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn nul(path: &std::path::Path) -> Vec<u8> {
    let mut bytes = path.display().to_string().into_bytes();
    bytes.push(0);
    bytes
}

/// A dialog for `kind` opened on a folder holding `files`, sorted by name.
fn open<'a>(
    cx: &'a mut TestAppContext,
    kind: Kind,
    files: &[&str],
    options: Vec<(&str, Value<'static>)>,
) -> (Entity<Chooser>, &'a mut VisualTestContext, async_channel::Receiver<Response>, Fixture) {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("boxbox-chooser-view-{}-{stamp}", std::process::id()));
    let config = TestConfig::new(root.with_extension("config"));
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if file.ends_with('/') {
            std::fs::create_dir_all(&path).unwrap();
        } else {
            std::fs::write(&path, file).unwrap();
        }
    }
    std::fs::create_dir_all(&root).unwrap();
    let mut options: HashMap<String, zbus::zvariant::OwnedValue> = options.into_iter().map(|(key, value)| (key.to_string(), value.try_to_owned().unwrap())).collect();
    options.insert("current_folder".into(), Value::from(nul(&root)).try_to_owned().unwrap());
    let request = Request::parse(kind, "org.example.Test", "", &options).unwrap();
    let (reply, answer) = async_channel::bounded(1);
    cx.update(crate::components::text_field_bind_keys);
    let (view, cx) = cx.add_window_view(|window, cx| Chooser::new(request, reply, window, cx));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|_, app| {
        view.update(app, |chooser, cx| {
            chooser.sort = Sort { field: SortField::Name, dir: SortDir::Asc };
            chooser.refilter(cx);
        })
    });
    cx.run_until_parked();
    (view, cx, answer, Fixture { root, _config: config })
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} must be rendered"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn a_double_click_opens_the_file_under_the_filter(cx: &mut TestAppContext) {
    let text: (String, Vec<(u32, String)>) = ("Text".into(), vec![(0, "*.txt".into())]);
    let (view, cx, answer, fixture) = open(cx, Kind::Open, &["notes.txt", "photo.png", "sub/"], vec![("filters", Value::from(vec![text]))]);
    let names: Vec<String> = view.read_with(cx, |chooser, _| chooser.visible.iter().map(|&index| chooser.items[index].name.clone()).collect());
    assert_eq!(names, vec!["sub", "notes.txt"], "folders, then files the filter allows");
    let row = cx.debug_bounds("chooser-row-1").unwrap();
    cx.simulate_click(row.center(), Modifiers::default());
    let (position, modifiers, button) = (row.center(), Modifiers::default(), gpui::MouseButton::Left);
    cx.simulate_event(gpui::MouseDownEvent { position, modifiers, button, click_count: 2, first_mouse: false });
    cx.simulate_event(gpui::MouseUpEvent { position, modifiers, button, click_count: 2 });
    cx.run_until_parked();
    let response = answer.try_recv().expect("the app was answered");
    assert_eq!(response.code, 0);
    assert_eq!(response.uris, vec![crate::os_clipboard::file_uri(&fixture.root.join("notes.txt"))]);
    assert_eq!(response.filter.map(|filter| filter.name), Some("Text".to_string()));
}

#[gpui::test]
fn saving_over_a_file_asks_first(cx: &mut TestAppContext) {
    let (view, cx, answer, fixture) = open(cx, Kind::Save, &["taken.txt"], vec![("current_name", Value::from("taken.txt"))]);
    click(cx, "chooser-accept");
    assert!(answer.try_recv().is_err(), "nothing is answered before Replace");
    view.read_with(cx, |chooser, _| assert!(chooser.replace.is_some(), "the dialog asks"));
    click(cx, "chooser-accept");
    let response = answer.try_recv().expect("Replace answers");
    assert_eq!(response.uris, vec![crate::os_clipboard::file_uri(&fixture.root.join("taken.txt"))]);
}

#[gpui::test]
fn a_typed_name_saves_in_the_folder(cx: &mut TestAppContext) {
    let (_view, cx, answer, fixture) = open(cx, Kind::Save, &["sub/"], vec![("current_name", Value::from("draft.md"))]);
    // The stem is selected, so typing replaces it and keeps ".md".
    cx.simulate_input("report");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let response = answer.try_recv().expect("Enter in the name saves");
    assert_eq!(response.uris, vec![crate::os_clipboard::file_uri(&fixture.root.join("report.md"))]);
}

#[gpui::test]
fn escape_cancels_and_closing_answers(cx: &mut TestAppContext) {
    let (_view, cx, answer, _fixture) = open(cx, Kind::Open, &["a.txt"], vec![]);
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert_eq!(answer.try_recv().unwrap().code, 1);
}

#[gpui::test]
fn closing_the_window_still_answers(cx: &mut TestAppContext) {
    let (view, cx, answer, _fixture) = open(cx, Kind::Open, &["a.txt"], vec![]);
    drop(view);
    cx.update(|window, _| window.remove_window());
    cx.run_until_parked();
    cx.run_until_parked();
    assert_eq!(answer.try_recv().unwrap().code, 1);
}

#[gpui::test]
fn the_buttons_keep_the_right_edge(cx: &mut TestAppContext) {
    let (_view, cx, _answer, _fixture) = open(cx, Kind::Open, &["a.txt"], vec![]);
    let window = cx.update(|window, _| window.viewport_size());
    let buttons = cx.debug_bounds("chooser-buttons").unwrap();
    let search = cx.debug_bounds("chooser-location").unwrap();
    assert_eq!(buttons.right(), window.width - gpui::px(12.), "12px from the edge");
    assert!(search.right() <= buttons.right());
}

#[gpui::test]
fn bookmarks_of_places_are_not_repeated(cx: &mut TestAppContext) {
    let (view, cx, _answer, _fixture) = open(cx, Kind::Open, &[], vec![]);
    view.read_with(cx, |chooser, _| {
        for bookmark in &chooser.bookmarks {
            assert!(!chooser.places.iter().chain(&chooser.drives).any(|place| place.path == bookmark.path), "{} is listed twice", bookmark.label);
        }
    });
}

#[gpui::test]
fn the_grid_switch_shows_tiles_that_arrows_move_through(cx: &mut TestAppContext) {
    let files: Vec<String> = (0..40).map(|n| format!("f{n:02}.txt")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (view, cx, _answer, _fixture) = open(cx, Kind::Open, &files, vec![]);
    view.update(cx, |chooser, _| chooser.view = crate::pane::ViewMode::List);
    cx.run_until_parked();
    click(cx, "chooser-view-grid");
    view.read_with(cx, |chooser, _| assert!(chooser.view == crate::pane::ViewMode::Grid, "the switch picks Grid"));
    click(cx, "chooser-tile-0");
    let columns = cx.update(|window, app| view.read(app).grid_columns(window));
    assert!(columns > 1, "a dialog-wide grid has several columns");
    cx.simulate_keystrokes("down");
    view.read_with(cx, |chooser, _| assert_eq!(chooser.active, Some(columns), "down goes a row of tiles"));
    cx.simulate_keystrokes("left");
    view.read_with(cx, |chooser, _| assert_eq!(chooser.active, Some(columns - 1), "left goes one tile"));
    click(cx, "chooser-view-list");
    view.read_with(cx, |chooser, _| assert!(chooser.view == crate::pane::ViewMode::List, "the switch goes back to List"));
}

#[gpui::test]
fn ctrl_plus_and_minus_size_the_view_shown(cx: &mut TestAppContext) {
    let (view, cx, _answer, _fixture) = open(cx, Kind::Open, &["a.txt"], vec![]);
    view.update(cx, |chooser, _| {
        chooser.view = crate::pane::ViewMode::Grid;
        chooser.grid_zoom = 1;
        chooser.list_zoom = 1;
    });
    cx.simulate_keystrokes("ctrl-= ctrl-=");
    view.read_with(cx, |chooser, _| assert_eq!((chooser.grid_zoom, chooser.list_zoom), (3, 1), "only the grid grows"));
    cx.simulate_keystrokes("ctrl-0");
    view.read_with(cx, |chooser, _| assert_eq!(chooser.grid_zoom, 1, "Ctrl+0 resets"));
    click(cx, "chooser-view-list");
    cx.simulate_keystrokes("ctrl-- ctrl--");
    view.read_with(cx, |chooser, _| assert_eq!(chooser.list_zoom, 0, "the list shrinks to its smallest step and stops"));
}

#[gpui::test]
fn a_picture_shows_its_thumbnail_in_the_grid(cx: &mut TestAppContext) {
    let (view, cx, _answer, fixture) = open(cx, Kind::Open, &[], vec![]);
    image::RgbaImage::from_pixel(64, 48, image::Rgba([200, 40, 40, 255])).save(fixture.root.join("red.png")).unwrap();
    view.update(cx, |chooser, cx| {
        chooser.view = crate::pane::ViewMode::Grid;
        let location = chooser.location.clone();
        chooser.open(location, cx);
    });
    let drawn = |cx: &mut VisualTestContext| view.read_with(cx, |chooser, _| chooser.visible.first().and_then(|&index| chooser.thumbnails.get(&chooser.thumbnail_key(&chooser.items[index]))).is_some());
    for _ in 0..100 {
        cx.run_until_parked();
        if drawn(cx) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(drawn(cx), "the tile's thumbnail was rendered");
}
