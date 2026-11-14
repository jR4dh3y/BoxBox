//! The sidebar's right-click menu: which entry it is for, and where.

use gpui::{Pixels, Point};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SidebarEntry {
    Trash,
}

pub(crate) struct SidebarMenu {
    pub entry: SidebarEntry,
    pub position: Point<Pixels>,
}
