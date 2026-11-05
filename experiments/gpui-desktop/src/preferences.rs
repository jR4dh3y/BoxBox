//! Desktop preferences (`lib/api/desktopPreferences.ts`), with the frontend's defaults.

#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub show_hidden: bool,
    pub show_extensions: bool,
    pub confirm_delete: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_hidden: false,
            show_extensions: true,
            confirm_delete: true,
        }
    }
}
