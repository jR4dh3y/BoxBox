//! Lucide icons extracted from the frontend's lucide-svelte by `scripts/extract-icons.ts`.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $(
                (concat!("icons/", $name, ".svg"), include_bytes!(concat!("../assets/icons/", $name, ".svg"))),
                (concat!("icons/thin/", $name, ".svg"), include_bytes!(concat!("../assets/icons/thin/", $name, ".svg"))),
            )*
        ];
    };
}

icons!(
    "arrow-up-from-line",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "columns-2",
    "download",
    "file",
    "file-archive",
    "file-audio",
    "file-code",
    "file-image",
    "file-json",
    "file-spreadsheet",
    "file-text",
    "file-video",
    "folder",
    "folder-open",
    "globe",
    "hard-drive",
    "home",
    "image",
    "layout-grid",
    "list",
    "monitor",
    "music",
    "palette",
    "panel-left",
    "plus",
    "refresh-cw",
    "search",
    "server",
    "settings",
    "star",
    "trash-2",
    "video",
    "x",
);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}
