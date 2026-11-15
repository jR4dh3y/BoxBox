//! Dragging files onto the window, from inside it or from another app: which
//! folder the pointer is over, whether the drop is allowed there, and the move
//! or copy it starts.
//!
//! Every place that takes a drop registers with `drop_zone`. While a drag
//! moves, the innermost zone under the pointer becomes the hover target, which
//! the views highlight and the drop uses, so the two cannot disagree.

use std::sync::Arc;

use std::path::PathBuf;

use gpui::{App, Context, DragMoveEvent, ExternalPaths, InteractiveElement, Modifiers, Pixels, Point, WeakEntity, Window};

use super::operations::{outside_folder, same_provider, ClipboardEntry, ClipboardMode, FileClipboard, PaneTarget};
use super::Workspace;
use crate::api::{Api, FileInfo};
use crate::drag_out::DraggedItem;
use crate::pane::ViewMode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DropTarget {
    /// The folder a pane shows.
    Pane(PaneTarget),
    /// A folder's row or tile in a pane.
    Folder { pane: PaneTarget, path: String },
    /// A sidebar entry, on the active pane's provider.
    Place { path: String },
    /// The sidebar's Trash: dropped local items are moved there.
    Trash,
    /// A breadcrumb of a pane's location. Held there while dragging, it opens.
    Crumb { pane: PaneTarget, path: String },
}

/// How long a drag rests on a breadcrumb, folder or place before it opens, as
/// GTK's path bar and Thunar's folders do.
const HOVER_OPEN: std::time::Duration = std::time::Duration::from_millis(700);
/// Near the top or bottom of a list, a drag scrolls it; deeper in, faster.
const SCROLL_EDGE: f32 = 40.;
const MAX_SCROLL_STEP: f32 = 16.;

/// The items being dragged, taken when the drag starts so a refresh cannot change them.
pub(crate) struct DragSource {
    pub api: Arc<Api>,
    pub items: Vec<FileInfo>,
}

#[derive(Default)]
pub(crate) struct DragDrop {
    source: Option<DragSource>,
    hover: Option<DropTarget>,
    /// The innermost zone seen during the current pointer move.
    candidate: Option<DropTarget>,
    settling: bool,
    /// The pane under the pointer and where it is, for scrolling at the edges.
    pointer: Option<(PaneTarget, Point<Pixels>)>,
    scrolling: bool,
}

/// A drop that asked what to do: the menu at the drop point.
pub(crate) struct DropMenu {
    pub source: DragSource,
    pub pane: PaneTarget,
    pub destination: Arc<Api>,
    pub folder: String,
    pub position: Point<Pixels>,
    /// Whether moving and linking are possible here; copying always is.
    pub can_move: bool,
    pub can_link: bool,
}

/// What the modifier keys ask for, as GTK file managers read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    /// Move on the same filesystem, copy to another.
    Auto,
    Copy,
    Move,
    /// Ctrl+Shift: a link to each item.
    Link,
    /// Alt: a menu of Move, Copy and Link at the drop.
    Ask,
}

fn request(modifiers: Modifiers) -> Request {
    if modifiers.alt {
        Request::Ask
    } else if modifiers.control && modifiers.shift {
        Request::Link
    } else if modifiers.control {
        Request::Copy
    } else if modifiers.shift {
        Request::Move
    } else {
        Request::Auto
    }
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// Whether `items` may be dropped into `folder`. `same` says the folder is on the
/// items' provider; `moving` that the drop would move rather than copy.
pub(crate) fn accepts(items: &[FileInfo], same: bool, folder: &str, moving: bool) -> bool {
    if !takes_files(folder) || items.is_empty() {
        return false;
    }
    if !same {
        return true;
    }
    let onto_itself = items.iter().any(|item| item.path == folder || (item.is_dir && folder.starts_with(&format!("{}/", item.path))));
    let already_there = moving && items.iter().all(|item| parent(&item.path) == folder);
    !onto_itself && !already_there
}

/// The Overview holds drives, and Recent and Network are listings: none is a folder.
fn takes_files(folder: &str) -> bool {
    !folder.is_empty() && folder != crate::local::RECENT && folder != crate::local::NETWORK
}

/// Which drag is over the window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    /// Rows or tiles of this window; `DragDrop::source` holds them.
    Window,
    /// Files from another app.
    Outside,
}

/// Marks `element` as a place to drop onto.
pub(crate) fn drop_zone<E: InteractiveElement>(element: E, target: DropTarget, workspace: WeakEntity<Workspace>) -> E {
    fn offer<T: 'static>(origin: Origin, target: DropTarget, workspace: WeakEntity<Workspace>) -> impl Fn(&DragMoveEvent<T>, &mut Window, &mut App) {
        move |event, window, cx| {
            if event.bounds.contains(&event.event.position) {
                let position = event.event.position;
                workspace.update(cx, |this, cx| this.offer_drop(origin, target.clone(), position, window, cx)).ok();
            }
        }
    }
    element
        .on_drag_move::<DraggedItem>(offer(Origin::Window, target.clone(), workspace.clone()))
        .on_drag_move::<ExternalPaths>(offer(Origin::Outside, target, workspace))
}

impl Workspace {
    /// Remembers what a drag carries. Returns the items for the drag preview.
    pub(crate) fn begin_drag(&mut self, pane: PaneTarget, index: usize) -> Vec<FileInfo> {
        let Some(pane) = self.pane_by_id(pane.0) else { return Vec::new() };
        let items = pane.drag_items(index);
        self.drag_drop.source = Some(DragSource { api: pane.api.clone(), items: items.clone() });
        self.drag_drop.hover = None;
        items
    }

    /// Opens a breadcrumb, folder or sidebar place if the drag is still on it
    /// after a moment, so a drag can go deeper without letting go.
    fn open_on_hover(&mut self, target: DropTarget, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOVER_OPEN).await;
            this.update(cx, |this, cx| {
                if this.drag_drop.hover.as_ref() != Some(&target) || !cx.has_active_drag() {
                    return;
                }
                let (pane, path) = match target {
                    DropTarget::Crumb { pane, path } | DropTarget::Folder { pane, path } => (pane, path),
                    DropTarget::Place { path } => (this.active_pane, path),
                    DropTarget::Pane(_) | DropTarget::Trash => return,
                };
                if this.pane_by_id(pane.0).is_some_and(|found| found.path != path) {
                    this.navigate_in(pane, path, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// While a drag is near the top or bottom of a pane, scrolls it each frame.
    fn scroll_drag_edges(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.drag_drop.scrolling, true) {
            return;
        }
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            let alive = this
                .update(cx, |this, cx| {
                    if !cx.has_active_drag() {
                        this.drag_drop.scrolling = false;
                        this.drag_drop.pointer = None;
                        return false;
                    }
                    let Some((target, pointer)) = this.drag_drop.pointer else { return true };
                    let Some(pane) = this.pane_by_id(target.0) else { return true };
                    let handle = match pane.current_view_mode() {
                        ViewMode::List => pane.list_scroll.0.borrow().base_handle.clone(),
                        ViewMode::Grid => pane.grid_scroll.0.borrow().base_handle.clone(),
                    };
                    let viewport = handle.bounds();
                    let (top, bottom) = (f32::from(pointer.y - viewport.top()), f32::from(viewport.bottom() - pointer.y));
                    let step = if top < SCROLL_EDGE && top >= 0. {
                        -(SCROLL_EDGE - top) / SCROLL_EDGE * MAX_SCROLL_STEP
                    } else if bottom < SCROLL_EDGE && bottom >= 0. {
                        (SCROLL_EDGE - bottom) / SCROLL_EDGE * MAX_SCROLL_STEP
                    } else {
                        return true;
                    };
                    let offset = handle.offset();
                    let lowest = -f32::from(handle.max_offset().height);
                    let y = (f32::from(offset.y) - step).clamp(lowest, 0.);
                    if y != f32::from(offset.y) {
                        handle.set_offset(gpui::point(offset.x, gpui::px(y)));
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
    }

    /// The target to highlight, while a drag is in the window.
    pub(crate) fn drop_hover(&self, cx: &App) -> Option<&DropTarget> {
        self.drag_drop.hover.as_ref().filter(|_| cx.has_active_drag())
    }

    /// The first call of each pointer move, from the root: forget the last candidate
    /// and settle once every zone has had its say.
    pub(crate) fn track_drag(&mut self, cx: &mut Context<Self>) {
        self.drag_drop.candidate = None;
        // A pane under the pointer sets it again; off every pane, nothing scrolls.
        self.drag_drop.pointer = None;
        if std::mem::replace(&mut self.drag_drop.settling, true) {
            return;
        }
        let workspace = cx.entity().downgrade();
        cx.defer(move |cx| {
            workspace
                .update(cx, |this, cx| {
                    this.drag_drop.settling = false;
                    let candidate = this.drag_drop.candidate.take();
                    if this.drag_drop.hover != candidate {
                        if let Some(target @ (DropTarget::Crumb { .. } | DropTarget::Folder { .. } | DropTarget::Place { .. })) = &candidate {
                            this.open_on_hover(target.clone(), cx);
                        }
                        this.drag_drop.hover = candidate;
                        cx.notify();
                    }
                })
                .ok();
        });
    }

    /// A zone under the pointer. Zones report outermost first, so the last valid one wins.
    fn offer_drop(&mut self, origin: Origin, target: DropTarget, position: Point<Pixels>, window: &Window, cx: &mut Context<Self>) {
        if let DropTarget::Pane(pane) | DropTarget::Folder { pane, .. } = &target {
            self.drag_drop.pointer = Some((*pane, position));
            self.scroll_drag_edges(cx);
        }
        // A row scrolled half under the header still has bounds there.
        if let DropTarget::Folder { pane, .. } = &target {
            let visible = self.pane_by_id(pane.0).is_some_and(|pane| {
                let scroll = match pane.view_mode {
                    ViewMode::List => &pane.list_scroll,
                    ViewMode::Grid => &pane.grid_scroll,
                };
                scroll.0.borrow().base_handle.bounds().contains(&position)
            });
            if !visible {
                return;
            }
        }
        let allowed = match (origin, &target) {
            (Origin::Window, DropTarget::Trash) => self.drag_drop.source.as_ref().is_some_and(|source| source.api.is_local()),
            (Origin::Outside, DropTarget::Trash) => true,
            (Origin::Window, _) => self.drop_allowed(&target, request(window.modifiers())),
            // The Overview holds drives, not files; any folder takes files from outside.
            (Origin::Outside, _) => self.drop_destination(&target).is_some_and(|(_, _, folder)| takes_files(&folder)),
        };
        if allowed {
            self.drag_drop.candidate = Some(target);
        } else if !matches!(target, DropTarget::Pane(_)) {
            // A refused folder must not fall back to the pane around it.
            self.drag_drop.candidate = None;
        }
    }

    /// The provider and folder a target stands for.
    fn drop_destination(&self, target: &DropTarget) -> Option<(PaneTarget, Arc<Api>, String)> {
        match target {
            DropTarget::Pane(pane) => self.pane_by_id(pane.0).map(|found| (*pane, found.api.clone(), found.path.clone())),
            DropTarget::Folder { pane, path } | DropTarget::Crumb { pane, path } => {
                self.pane_by_id(pane.0).map(|found| (*pane, found.api.clone(), path.clone()))
            }
            DropTarget::Place { path } => Some((self.active_pane, self.active_api(), path.clone())),
            DropTarget::Trash => None,
        }
    }

    fn drop_allowed(&self, target: &DropTarget, request: Request) -> bool {
        let (Some(source), Some((_, api, folder))) = (&self.drag_drop.source, self.drop_destination(target)) else {
            return false;
        };
        let same = same_provider(&source.api, &api);
        match request {
            Request::Link => links(&source.api, &api) && takes_files(&folder),
            // The menu offers a copy, which any folder takes.
            Request::Ask | Request::Copy => accepts(&source.items, same, &folder, false),
            Request::Move | Request::Auto => accepts(&source.items, same, &folder, true),
        }
    }

    /// The button came up over the window: move, copy or link the items to the
    /// hover target, or ask which.
    pub(crate) fn drop_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (source, target) = (self.drag_drop.source.take(), self.drag_drop.hover.take());
        let (Some(source), Some(target)) = (source, target) else { return };
        if target == DropTarget::Trash {
            self.trash_dropped(source.api, source.items, cx);
            return;
        }
        let request = request(window.modifiers());
        let Some((pane, destination, folder)) = self.drop_destination(&target) else { return };
        let same = same_provider(&source.api, &destination);
        // The keys may have changed since the pointer last moved.
        match request {
            Request::Link if links(&source.api, &destination) && takes_files(&folder) => {
                self.link_items(source, destination, folder, cx);
            }
            Request::Ask if accepts(&source.items, same, &folder, false) => {
                self.drop_menu = Some(DropMenu {
                    can_move: accepts(&source.items, same, &folder, true),
                    can_link: links(&source.api, &destination),
                    source,
                    pane,
                    destination,
                    folder,
                    position: window.mouse_position(),
                });
                cx.notify();
            }
            Request::Copy if accepts(&source.items, same, &folder, false) => {
                self.drop_with(pane, source, destination, folder, ClipboardMode::Copy, cx);
            }
            Request::Move if accepts(&source.items, same, &folder, true) => {
                self.drop_with(pane, source, destination, folder, ClipboardMode::Cut, cx);
            }
            Request::Auto if accepts(&source.items, same, &folder, true) => {
                let first = source.items.first().map(|item| item.path.clone()).unwrap_or_default();
                let (from, to, inside) = (source.api.clone(), destination.clone(), folder.clone());
                cx.spawn(async move |this, cx| {
                    let one_filesystem = same && cx.background_executor().spawn(async move { same_filesystem(&from, &first, &to, &inside) }).await;
                    let mode = if one_filesystem { ClipboardMode::Cut } else { ClipboardMode::Copy };
                    this.update(cx, |this, cx| this.drop_with(pane, source, destination, folder, mode, cx)).ok();
                })
                .detach();
            }
            _ => {}
        }
    }

    pub(crate) fn drop_with(&mut self, pane: PaneTarget, source: DragSource, destination: Arc<Api>, folder: String, mode: ClipboardMode, cx: &mut Context<Self>) {
        let entries = source.items.into_iter().map(|item| ClipboardEntry { api: source.api.clone(), item }).collect();
        self.start_transfer(pane, FileClipboard { mode, entries }, destination, folder, cx);
    }

    /// A link to each item in `folder`, under the item's name, or a free one
    /// when it is taken. The new links are selected.
    pub(crate) fn link_items(&mut self, source: DragSource, destination: Arc<Api>, folder: String, cx: &mut Context<Self>) {
        let generation = self.begin_operation("Creating links…");
        let api = destination.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut taken: std::collections::HashSet<String> = api.list(&folder)?.into_iter().map(|item| item.name).collect();
                    let mut made = Vec::new();
                    for item in &source.items {
                        let name = match taken.contains(&item.name) {
                            true => crate::transfer::keep_both_name(&item.name, &taken),
                            false => item.name.clone(),
                        };
                        let link = if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") };
                        api.create_link(&item.path, &link)?;
                        taken.insert(name);
                        made.push(link);
                    }
                    anyhow::Ok(made)
                })
                .await;
            this.update(cx, |this, cx| {
                let result = result.map(|made| {
                    if let Some(pane) = this.tabs.iter_mut().chain(this.split_panes.values_mut()).find(|pane| made.first().is_some_and(|link| parent(link) == pane.path)) {
                        pane.selected = made.into_iter().collect();
                    }
                });
                this.refresh_provider(destination, cx);
                this.finish_operation(generation, result, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The Alt-drop menu's choice.
    pub(crate) fn choose_drop(&mut self, choice: Option<ClipboardMode>, link: bool, cx: &mut Context<Self>) {
        let Some(menu) = self.drop_menu.take() else { return };
        match (choice, link) {
            (_, true) => self.link_items(menu.source, menu.destination, menu.folder, cx),
            (Some(mode), false) => self.drop_with(menu.pane, menu.source, menu.destination, menu.folder, mode, cx),
            (None, false) => {}
        }
        cx.notify();
    }
}

/// Links point at real paths, so both ends must be on this computer.
fn links(source: &Api, destination: &Api) -> bool {
    source.is_local() && destination.is_local()
}

impl Workspace {
    /// Files dragged in from another app, dropped on the hover target. They are copied:
    /// GPUI does not say which action the other app offered.
    pub(crate) fn drop_external(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(target) = self.drag_drop.hover.take() else { return };
        if target == DropTarget::Trash {
            self.trash_external(paths, cx);
            return;
        }
        let Some((pane, destination, folder)) = self.drop_destination(&target).filter(|(_, _, folder)| takes_files(folder)) else { return };
        cx.spawn(async move |this, cx| {
            // A drop can come from this very folder, shown in another file manager.
            let (resolver, inside) = (destination.clone(), folder.clone());
            let paths = cx
                .background_executor()
                .spawn(async move {
                    match resolver.local_path(&inside) {
                        Ok(Some(real)) => outside_folder(&real, paths),
                        _ => paths,
                    }
                })
                .await;
            if paths.is_empty() {
                return;
            }
            let files = crate::os_clipboard::Files { op: crate::os_clipboard::FileOp::Copy, paths };
            this.update(cx, |this, cx| this.receive_files(pane, destination, folder, files, cx)).ok();
        })
        .detach();
    }
}

/// Whether a dragged item and the folder it is dropped in share a device, so that a
/// move is a rename and not a copy. Remote providers count as one filesystem each.
fn same_filesystem(from: &Api, item: &str, to: &Api, folder: &str) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let device = |api: &Api, path: &str| -> Option<Option<u64>> {
        match api.local_path(path) {
            Ok(Some(real)) => std::fs::symlink_metadata(real).ok().map(|metadata| Some(metadata.dev())),
            Ok(None) => Some(None),
            Err(_) => None,
        }
    };
    matches!((device(from, item), device(to, folder)), (Some(left), Some(right)) if left == right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, is_dir: bool) -> FileInfo {
        FileInfo { name: path.rsplit('/').next().unwrap().into(), path: path.into(), size: 0, is_dir, mod_time: None }
    }

    #[test]
    fn an_item_refuses_itself_and_its_own_inside() {
        let folder = [item("home/a", true)];
        assert!(!accepts(&folder, true, "home/a", true));
        assert!(!accepts(&folder, true, "home/a/b/c", false));
        // A sibling whose name starts the same is another folder.
        assert!(accepts(&folder, true, "home/ab", true));
        // The same path on another provider is another place.
        assert!(accepts(&folder, false, "home/a", true));
    }

    #[test]
    fn a_move_to_where_the_items_are_does_nothing_but_a_copy_duplicates() {
        let items = [item("home/a.txt", false), item("home/b.txt", false)];
        assert!(!accepts(&items, true, "home", true));
        assert!(accepts(&items, true, "home", false));
        assert!(accepts(&items, true, "home/docs", true));
        // Search results come from several folders; some of them would move.
        assert!(accepts(&[item("home/a.txt", false), item("home/x/b.txt", false)], true, "home", true));
    }

    #[test]
    fn this_pc_takes_no_files() {
        assert!(!accepts(&[item("home/a.txt", false)], true, "", false));
        assert!(!accepts(&[], true, "home", false));
    }

    #[test]
    fn control_copies_shift_moves_both_link_and_alt_asks() {
        assert_eq!(request(Modifiers::default()), Request::Auto);
        assert_eq!(request(Modifiers { control: true, ..Default::default() }), Request::Copy);
        assert_eq!(request(Modifiers { shift: true, ..Default::default() }), Request::Move);
        assert_eq!(request(Modifiers { control: true, shift: true, ..Default::default() }), Request::Link);
        assert_eq!(request(Modifiers { alt: true, control: true, ..Default::default() }), Request::Ask);
    }
}
