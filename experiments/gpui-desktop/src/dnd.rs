//! Drag and drop between folders, panes and sidebar places (`browse/fileDrag.ts`, `fileDrop.ts`).
//! BoxBox items move unless Ctrl is held; files from other apps are copied.

use gpui::{Context, ExternalPaths, Window};

use crate::clipboard::{ClipItem, ClipOp};
use crate::drag_out::DraggedItem;
use crate::workspace::Workspace;

/// Refuses drops that cannot work or do nothing: into an item itself, or a move to where it is.
pub fn can_drop_into(sources: &[ClipItem], dest: &str, op: ClipOp) -> bool {
    if dest.is_empty() || dest.starts_with('@') {
        return false;
    }
    if sources
        .iter()
        .any(|source| dest == source.path || dest.starts_with(&format!("{}/", source.path)))
    {
        return false;
    }
    !(op == ClipOp::Cut
        && sources
            .iter()
            .all(|source| source.path.rsplit_once('/').map(|(parent, _)| parent) == Some(dest)))
}

/// Local files from other apps as BoxBox paths, through the desktop's `/` mount.
pub fn local_paths(paths: &ExternalPaths) -> Vec<ClipItem> {
    paths
        .paths()
        .iter()
        .filter_map(|path| {
            let text = path.to_str()?;
            let name = path.file_name()?.to_str()?.to_string();
            let relative = text.strip_prefix('/')?;
            (!relative.is_empty()).then(|| ClipItem {
                path: format!("root/{relative}"),
                name,
                is_dir: path.is_dir(),
            })
        })
        .collect()
}

impl Workspace {
    /// Writable destination: an existing mount that is not read-only.
    fn can_drop_on(&self, dest: &str) -> bool {
        let mount = dest.split('/').next().unwrap_or_default();
        self.roots
            .iter()
            .any(|root| root.name == mount && !root.read_only)
    }

    pub(crate) fn drop_dragged(
        &mut self,
        dragged: &DraggedItem,
        dest: String,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.pane_by_id(dragged.pane) else {
            return;
        };
        let sources: Vec<ClipItem> = pane
            .drag_items(dragged.index)
            .into_iter()
            .map(|item| ClipItem {
                path: item.path,
                name: item.name,
                is_dir: item.is_dir,
            })
            .collect();
        let op = if window.modifiers().control {
            ClipOp::Copy
        } else {
            ClipOp::Cut
        };
        if self.can_drop_on(&dest) && can_drop_into(&sources, &dest, op) {
            self.transfer(op, sources, dest, cx);
        }
    }

    pub(crate) fn drop_external(
        &mut self,
        paths: &ExternalPaths,
        dest: String,
        cx: &mut Context<Self>,
    ) {
        let sources = local_paths(paths);
        if !sources.is_empty()
            && self.can_drop_on(&dest)
            && can_drop_into(&sources, &dest, ClipOp::Copy)
        {
            self.transfer(ClipOp::Copy, sources, dest, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str) -> ClipItem {
        ClipItem {
            path: path.into(),
            name: path.rsplit('/').next().unwrap().into(),
            is_dir: true,
        }
    }

    #[test]
    fn refuses_self_descendant_and_same_folder_moves() {
        let sources = [item("home/a"), item("home/b")];
        assert!(!can_drop_into(&sources, "home/a", ClipOp::Cut));
        assert!(!can_drop_into(&sources, "home/a/inner", ClipOp::Copy));
        assert!(!can_drop_into(&sources, "home", ClipOp::Cut));
        assert!(can_drop_into(&sources, "home", ClipOp::Copy));
        assert!(can_drop_into(&sources, "home/c", ClipOp::Cut));
        assert!(!can_drop_into(&sources, "@trash", ClipOp::Cut));
    }
}
