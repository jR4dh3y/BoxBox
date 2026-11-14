//! Moving through a folder from the keyboard, as GTK file managers do: arrows,
//! Home, End and the page keys; Shift extends the selection and Ctrl moves the
//! cursor alone; Ctrl+Space toggles; typing a name jumps to it.

use std::time::{Duration, Instant};

use gpui::{px, App, Context, KeyBinding, KeyDownEvent, Window};

use super::operations::PaneTarget;
use super::Workspace;
use crate::components::GridMetrics;
use crate::pane::{Move, ViewMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = boxbox, no_json)]
pub struct MoveCursor {
    pub step: Step,
    pub how: Move,
}

gpui::actions!(boxbox, [ToggleActive, OpenMenuKey]);

/// How long a pause ends a typed name.
const TYPE_AHEAD_PAUSE: Duration = Duration::from_millis(1000);

#[derive(Default)]
pub(crate) struct TypeAhead {
    text: String,
    last: Option<Instant>,
}

pub fn bind_keys(cx: &mut App) {
    // In a text field these keys edit the text.
    let context = Some("BrowseWorkspace && !TextField");
    let steps = [
        ("up", Step::Up),
        ("down", Step::Down),
        ("left", Step::Left),
        ("right", Step::Right),
        ("home", Step::Home),
        ("end", Step::End),
        ("pageup", Step::PageUp),
        ("pagedown", Step::PageDown),
    ];
    let mut bindings = Vec::new();
    for (key, step) in steps {
        for (modifier, how) in [("", Move::Select), ("shift-", Move::Extend), ("ctrl-", Move::Focus)] {
            bindings.push(KeyBinding::new(&format!("{modifier}{key}"), MoveCursor { step, how }, context));
        }
    }
    bindings.push(KeyBinding::new("ctrl-space", ToggleActive, context));
    bindings.push(KeyBinding::new("menu", OpenMenuKey, context));
    bindings.push(KeyBinding::new("shift-f10", OpenMenuKey, context));
    cx.bind_keys(bindings);
}

impl Workspace {
    pub(crate) fn move_cursor(&mut self, action: &MoveCursor, window: &mut Window, cx: &mut Context<Self>) {
        if self.trash_open {
            return;
        }
        let target = self.active_pane;
        let (columns, page) = self.layout(target, window);
        let pane = self.pane_mut();
        let count = pane.items.len();
        let delta = match action.step {
            Step::Up => -(columns as isize),
            Step::Down => columns as isize,
            // A list has one column; Left and Right are for the grid.
            Step::Left | Step::Right if columns == 1 => return,
            Step::Left => -1,
            Step::Right => 1,
            Step::Home => -(count as isize),
            Step::End => count as isize,
            Step::PageUp => -(page as isize),
            Step::PageDown => page as isize,
        };
        let Some(index) = pane.cursor_step(delta) else { return };
        pane.move_cursor(index, action.how);
        self.reveal(target, index, window);
        cx.notify();
    }

    /// Items per row, and items per screenful, of the pane as it is drawn.
    fn layout(&self, target: PaneTarget, window: &Window) -> (usize, usize) {
        let Some(pane) = self.pane_by_id(target.0) else { return (1, 1) };
        match pane.current_view_mode() {
            ViewMode::List => {
                let height = pane.list_scroll.0.borrow().base_handle.bounds().size.height;
                let rows = (f32::from(height) / crate::components::row_height(self.preferences.compact_rows)).floor() as usize;
                (1, rows.max(1))
            }
            ViewMode::Grid => {
                let metrics = GridMetrics::new(self.preferences.compact_rows);
                let columns = metrics.columns(self.browser_width(window));
                let height = pane.grid_scroll.0.borrow().base_handle.bounds().size.height;
                let rows = (f32::from(height) / metrics.row_height).floor() as usize;
                (columns, columns * rows.max(1))
            }
        }
    }

    pub(crate) fn toggle_active(&mut self, _: &ToggleActive, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().toggle_active();
        cx.notify();
    }

    /// The Menu key or Shift+F10: the menu for the selection, by the item under the
    /// cursor, or for the folder when nothing is selected.
    pub(crate) fn open_menu_key(&mut self, _: &OpenMenuKey, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.active_pane;
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let (scroll, item_height) = match pane.current_view_mode() {
            ViewMode::List => (pane.list_scroll.0.borrow().base_handle.clone(), crate::components::row_height(self.preferences.compact_rows)),
            ViewMode::Grid => (pane.grid_scroll.0.borrow().base_handle.clone(), GridMetrics::new(self.preferences.compact_rows).row_height),
        };
        let (bounds, offset) = (scroll.bounds(), scroll.offset());
        let (columns, _) = self.layout(target, window);
        let selected = !pane.selected.is_empty();
        let cursor = pane.active.filter(|&index| selected && pane.items.get(index).is_some_and(|item| pane.is_selected(&item.path)));
        let position = match cursor {
            Some(index) => {
                let (row, column) = (index / columns, index % columns);
                let tile = GridMetrics::new(self.preferences.compact_rows);
                let x = if columns == 1 { 40. } else { tile.padding + column as f32 * (tile.tile_width + tile.gap) + tile.tile_width / 2. };
                bounds.origin + offset + gpui::point(px(x), px((row as f32 + 0.75) * item_height))
            }
            None => bounds.origin + gpui::point(px(40.), px(8.)),
        };
        let position = gpui::point(position.x.clamp(bounds.left(), bounds.right()), position.y.clamp(bounds.top(), bounds.bottom()));
        match cursor {
            Some(index) => self.show_item_context_menu(target, index, position, cx),
            None => {
                self.pane_mut().clear_selection();
                self.context_menu = Some(super::operations::ContextMenuState { target_id: target.0, items: Vec::new(), position });
                cx.notify();
            }
        }
    }

    /// Typing a name while the file list has focus selects the first item it starts.
    /// The same letter again goes on to the next item that starts with it.
    pub(crate) fn type_ahead(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if !self.focus_handle.is_focused(window) || modifiers.control || modifiers.alt || modifiers.platform || self.trash_open {
            return;
        }
        let Some(typed) = event.keystroke.key_char.as_deref().filter(|text| text.chars().all(|ch| !ch.is_whitespace() && !ch.is_control())) else {
            return;
        };
        let now = Instant::now();
        let fresh = self.type_ahead.last.is_none_or(|last| now.duration_since(last) > TYPE_AHEAD_PAUSE);
        if fresh {
            self.type_ahead.text.clear();
        }
        self.type_ahead.last = Some(now);
        self.type_ahead.text.push_str(typed);
        let text = self.type_ahead.text.clone();
        let mut letters = text.chars();
        let repeated = letters.next().filter(|first| text.chars().count() > 1 && letters.all(|ch| ch == *first));
        let pane = self.pane_mut();
        let active = pane.active.unwrap_or(0);
        let found = match repeated {
            // "aaa" steps through the items that start with "a".
            Some(letter) if pane.find_prefix(&text, 0).is_none() => pane.find_prefix(&letter.to_string(), active + 1),
            // A new name is looked for from the top; a longer one from the current match.
            _ => pane.find_prefix(&text, if fresh { 0 } else { active }),
        };
        if let Some(index) = found {
            pane.move_cursor(index, Move::Select);
            let target = self.active_pane;
            self.reveal(target, index, window);
            cx.notify();
        }
        cx.stop_propagation();
    }
}
