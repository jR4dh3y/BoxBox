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

/// How long a drag rests on a breadcrumb before it opens, as GTK's path bar does.
const HOVER_OPEN: std::time::Duration = std::time::Duration::from_millis(700);

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
}

/// What the modifier keys ask for, as in other Linux file managers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    /// Move on the same filesystem, copy to another.
    Auto,
    Copy,
    Move,
}

fn request(modifiers: Modifiers) -> Request {
    if modifiers.control {
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
    // This PC holds drives, not files.
    if folder.is_empty() || items.is_empty() {
        return false;
    }
    if !same {
        return true;
    }
    let onto_itself = items.iter().any(|item| item.path == folder || (item.is_dir && folder.starts_with(&format!("{}/", item.path))));
    let already_there = moving && items.iter().all(|item| parent(&item.path) == folder);
    !onto_itself && !already_there
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
                workspace.update(cx, |this, _| this.offer_drop(origin, target.clone(), position, window)).ok();
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

    /// Opens a breadcrumb's folder if the drag is still on it after a moment.
    fn open_on_hover(&mut self, target: DropTarget, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOVER_OPEN).await;
            this.update(cx, |this, cx| {
                if this.drag_drop.hover.as_ref() != Some(&target) || !cx.has_active_drag() {
                    return;
                }
                if let DropTarget::Crumb { pane, path } = target {
                    if this.pane_by_id(pane.0).is_some_and(|found| found.path != path) {
                        this.navigate_in(pane, path, cx);
                    }
                }
            })
            .ok();
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
                        if let Some(DropTarget::Crumb { pane, path }) = &candidate {
                            this.open_on_hover(DropTarget::Crumb { pane: *pane, path: path.clone() }, cx);
                        }
                        this.drag_drop.hover = candidate;
                        cx.notify();
                    }
                })
                .ok();
        });
    }

    /// A zone under the pointer. Zones report outermost first, so the last valid one wins.
    fn offer_drop(&mut self, origin: Origin, target: DropTarget, position: Point<Pixels>, window: &Window) {
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
            // This PC holds drives, not files; any folder takes files from outside.
            (Origin::Outside, _) => self.drop_destination(&target).is_some_and(|(_, _, folder)| !folder.is_empty()),
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
        accepts(&source.items, same_provider(&source.api, &api), &folder, request != Request::Copy)
    }

    /// The button came up over the window: move or copy the items to the hover target.
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
        if !accepts(&source.items, same, &folder, request != Request::Copy) {
            return;
        }
        let entries: Vec<ClipboardEntry> = source.items.into_iter().map(|item| ClipboardEntry { api: source.api.clone(), item }).collect();
        let first = entries.first().map(|entry| entry.item.path.clone()).unwrap_or_default();
        let (from, to) = (source.api, destination.clone());
        cx.spawn(async move |this, cx| {
            let mode = match request {
                Request::Copy => ClipboardMode::Copy,
                Request::Move => ClipboardMode::Cut,
                Request::Auto => {
                    let (to, folder) = (to.clone(), folder.clone());
                    let one_filesystem = same && cx.background_executor().spawn(async move { same_filesystem(&from, &first, &to, &folder) }).await;
                    if one_filesystem { ClipboardMode::Cut } else { ClipboardMode::Copy }
                }
            };
            this.update(cx, |this, cx| this.start_transfer(pane, FileClipboard { mode, entries }, to, folder, cx)).ok();
        })
        .detach();
    }
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
        let Some((pane, destination, folder)) = self.drop_destination(&target).filter(|(_, _, folder)| !folder.is_empty()) else { return };
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
        FileInfo { name: path.rsplit('/').next().unwrap().into(), path: path.into(), size: 0, is_dir, mod_time: String::new() }
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
    fn control_copies_and_shift_moves() {
        assert_eq!(request(Modifiers::default()), Request::Auto);
        assert_eq!(request(Modifiers { control: true, ..Default::default() }), Request::Copy);
        assert_eq!(request(Modifiers { shift: true, ..Default::default() }), Request::Move);
    }
}
