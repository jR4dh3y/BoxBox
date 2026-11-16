//! The window's theme icons: one image per icon key and pixel size, made off
//! the UI thread the first time a row or tile asks. Views read what is ready
//! and draw their own glyph until then, when the theme has no icon, or when
//! Settings ask for BoxBox's icons.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Context, RenderImage};

use super::operations::PaneTarget;
use super::Workspace;
use crate::api::{Api, FileInfo};
use crate::components::EntryIcon;
use crate::file_icons::{bucket, IconKey};

/// Enough for every type in a few folders at two zoom steps; past it, all are
/// dropped and the drawn ones asked for again.
const CAPACITY: usize = 512;

enum Slot {
    Pending,
    Ready(Arc<RenderImage>),
    /// The theme has no icon for it.
    Missing,
}

#[derive(Default)]
pub(crate) struct FileIcons {
    slots: HashMap<(IconKey, u32), Slot>,
    /// Virtual paths of the home and user folders on the local provider.
    special: HashMap<String, &'static str>,
    /// The display's scale, so icons are made at device pixels.
    pub scale: f32,
}

impl FileIcons {
    fn key(&self, api: &Api, item: &FileInfo, size: f32) -> (IconKey, u32) {
        let special = if item.is_dir && api.is_local() { self.special.get(&item.path).copied() } else { None };
        (IconKey::for_entry(&item.name, item.is_dir, special), bucket(size, self.scale.max(1.)))
    }
}

impl Workspace {
    /// Reads the user folders once, for their icons.
    pub(crate) fn load_special_folders(&mut self, cx: &mut Context<Self>) {
        let api = self.home.api.clone();
        cx.spawn(async move |this, cx| {
            let special = cx.background_executor().spawn(async move { api.special_folders() }).await;
            this.update(cx, |this, cx| {
                this.file_icons.special = special.into_iter().collect();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// What `item`'s icon slot shows at `size`: the theme's icon, nothing while
    /// it is made, or BoxBox's glyph when the theme has none or BoxBox's set is chosen.
    pub(crate) fn item_icon(&self, api: &Api, item: &FileInfo, size: f32) -> EntryIcon {
        if self.preferences.file_icons != crate::workspace::IconStyle::System {
            return EntryIcon::Glyph;
        }
        match self.file_icons.slots.get(&self.file_icons.key(api, item, size)) {
            Some(Slot::Ready(image)) => EntryIcon::Theme(image.clone()),
            Some(Slot::Missing) => EntryIcon::Glyph,
            Some(Slot::Pending) | None => EntryIcon::Waiting,
        }
    }

    /// Starts making the icon of a pane's entry, drawn at `size`, if no one has.
    pub(crate) fn request_entry_icon(&mut self, target: PaneTarget, index: usize, size: f32, cx: &mut Context<Self>) {
        if self.preferences.file_icons != crate::workspace::IconStyle::System {
            return;
        }
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let Some(item) = pane.items.get(index) else { return };
        let key = self.file_icons.key(&pane.api, item, size);
        self.request_icon(key, cx);
    }

    fn request_icon(&mut self, key: (IconKey, u32), cx: &mut Context<Self>) {
        if self.file_icons.slots.contains_key(&key) {
            return;
        }
        if self.file_icons.slots.len() >= CAPACITY {
            for (_, slot) in self.file_icons.slots.drain() {
                if let Slot::Ready(image) = slot {
                    cx.drop_image(image, None);
                }
            }
        }
        self.file_icons.slots.insert(key.clone(), Slot::Pending);
        let (icon, pixels) = key.clone();
        cx.spawn(async move |this, cx| {
            let image = cx.background_executor().spawn(async move { crate::file_icons::render(&icon, pixels) }).await;
            this.update(cx, |this, cx| {
                // Dropped meanwhile: the image is not wanted.
                let Some(slot) = this.file_icons.slots.get_mut(&key) else {
                    if let Some(image) = image {
                        cx.drop_image(image, None);
                    }
                    return;
                };
                *slot = image.map_or(Slot::Missing, Slot::Ready);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// The small glyph a special folder carries inside BoxBox's folder icon.
fn emblem(theme_icon: &str) -> Option<&'static str> {
    Some(match theme_icon {
        "user-home" => "home",
        "user-desktop" => "monitor",
        "folder-documents" => "file-text",
        "folder-download" => "download",
        "folder-music" => "music",
        "folder-pictures" => "image",
        "folder-videos" => "video",
        "folder-templates" => "layout-template",
        "folder-publicshare" => "share-2",
        _ => return None,
    })
}

/// How BoxBox's own icon for an entry looks.
pub(crate) struct GlyphStyle {
    pub glyph: &'static str,
    /// Overrides the plain grey (or folder colour) when tinted.
    pub tint: Option<u32>,
    pub emblem: Option<&'static str>,
}

impl Workspace {
    /// BoxBox's glyph for `item`: in the tinted set folders take the accent
    /// and every other kind keeps its own colour.
    pub(crate) fn glyph_style(&self, api: &Api, item: &FileInfo) -> GlyphStyle {
        let kind = crate::format::Kind::of(&item.name, item.is_dir);
        let special = if item.is_dir && api.is_local() { self.file_icons.special.get(&item.path).copied() } else { None };
        let accent = self.accent_hex();
        let tint = match (self.preferences.file_icons, kind) {
            (crate::workspace::IconStyle::Tinted, crate::format::Kind::Folder) => Some(accent),
            (crate::workspace::IconStyle::Tinted, kind) => kind.tint(),
            _ => None,
        };
        GlyphStyle { glyph: kind.glyph(), tint, emblem: special.and_then(emblem) }
    }
}
