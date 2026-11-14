//! Shared components, one per Svelte component in `frontend/src/lib/components`,
//! with the same names, props and states. Views compose these and do not
//! style controls themselves, so the two UIs cannot drift apart. The gallery
//! (`--gallery`) renders each one from `frontend/src/routes/dev/gallery/gallery.json`
//! for comparison against the web app.

mod button;
mod context_menu;
mod drive_card;
mod file_grid;
mod file_list;
mod input;
mod modal;
mod progress_bar;
mod select;
mod settings;
mod sidebar;
mod text_field;
mod toast;
mod toggle;
mod tooltip;

pub use button::{Button, ButtonSize, ButtonVariant};
pub use drive_card::{usage_bar, usage_text, DriveCard};
pub use context_menu::{ContextMenu, MenuItem};
pub use file_grid::{FileTile, GridMetrics};
pub use file_list::{rename_box, FileListHeader, FileRow, Selection};
pub use input::Input;
pub use modal::{Modal, ModalSize};
pub use progress_bar::{ProgressBar, ProgressSize, ProgressVariant};
pub use select::Select;
pub use settings::{settings_rows, SettingsRow, SettingsTab};
pub use text_field::{bind_keys as text_field_bind_keys, InputEvent, TextField};
pub use sidebar::{NavItem, SidebarSection};
pub use toast::{Toast, ToastKind};
pub use toggle::Toggle;
pub use tooltip::{Tooltip, UiFont};

use gpui::{px, Styled};

/// Tailwind's `rounded`: 0.25rem.
pub const ROUNDED: f32 = 4.;
/// `rounded-lg`: 0.5rem.
pub const ROUNDED_LG: f32 = 8.;

/// Tailwind's type scale, with the line height each class sets. `text-[13px]`
/// and `text-[11px]` set no line height, so they inherit the body's 1.5.
pub trait Type: Styled + Sized {
    /// `text-xs`
    fn type_xs(self) -> Self {
        self.text_size(px(12.)).line_height(px(16.))
    }
    /// `text-[13px]`
    fn type_13(self) -> Self {
        self.text_size(px(13.)).line_height(px(19.5))
    }
    /// `text-sm`
    fn type_sm(self) -> Self {
        self.text_size(px(14.)).line_height(px(20.))
    }
    /// `text-[11px]`
    fn type_11(self) -> Self {
        self.text_size(px(11.)).line_height(px(16.5))
    }
}

impl<T: Styled> Type for T {}
