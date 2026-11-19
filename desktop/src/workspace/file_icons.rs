//! The window's icons: one image per source and pixel size, made off the UI
//! thread the first time a row or tile asks. A package's own icon wins once
//! ready; until then, or without one, views show the selected icon style.

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum IconSource {
    Theme(IconKey),
    /// A local AppImage, deb or rpm's own icon.
    #[cfg(target_os = "linux")]
    Package(crate::package_icons::Package),
}

type CacheKey = (IconSource, u32);

#[derive(Default)]
pub(crate) struct FileIcons {
    slots: HashMap<CacheKey, Slot>,
    /// Virtual paths of the home and user folders on the local provider.
    special: HashMap<String, &'static str>,
    /// The display's scale, so icons are made at device pixels.
    pub scale: f32,
}

impl FileIcons {
    fn theme_key(&self, api: &Api, item: &FileInfo) -> IconKey {
        let special = if item.is_dir && api.is_local() { self.special.get(&item.path).copied() } else { None };
        IconKey::for_entry(&item.name, item.is_dir, special)
    }

    /// The file's own icon, in every icon style, and the theme's.
    fn keys(&self, api: &Api, item: &FileInfo, size: f32, style: super::IconStyle) -> [Option<CacheKey>; 2] {
        let pixels = bucket(size, self.scale.max(1.));
        let theme = (style == super::IconStyle::System).then(|| (IconSource::Theme(self.theme_key(api, item)), pixels));
        #[cfg(target_os = "linux")]
        let embedded = crate::package_icons::Package::of(api, item).map(|package| (IconSource::Package(package), pixels));
        #[cfg(not(target_os = "linux"))]
        let embedded = None;
        [embedded, theme]
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

    /// What `item`'s icon slot shows at `size`: its own icon when ready, else
    /// the theme's icon, nothing while it is made, or BoxBox's glyph when the
    /// theme has none or BoxBox's set is chosen.
    pub(crate) fn item_icon(&self, api: &Api, item: &FileInfo, size: f32) -> EntryIcon {
        let [embedded, theme] = self.file_icons.keys(api, item, size, self.preferences.file_icons);
        if let Some(Slot::Ready(image)) = embedded.and_then(|key| self.file_icons.slots.get(&key)) {
            return EntryIcon::Theme(image.clone());
        }
        let Some(theme) = theme else { return EntryIcon::Glyph };
        match self.file_icons.slots.get(&theme) {
            Some(Slot::Ready(image)) => EntryIcon::Theme(image.clone()),
            Some(Slot::Missing) => EntryIcon::Glyph,
            Some(Slot::Pending) | None => EntryIcon::Waiting,
        }
    }

    /// What a drag to another app shows for `item`: its thumbnail when one is
    /// made, else the icon the views draw for it. Made here when not ready: one
    /// small icon, once per drag. A package's own icon is used only when already
    /// read, as reading it can take seconds.
    pub(crate) fn drag_picture(&self, api: &Arc<Api>, item: &FileInfo) -> Option<Arc<RenderImage>> {
        const PIXELS: u32 = 128;
        if let Some(thumbnail) = self.thumbnail(api, item) {
            return Some(thumbnail);
        }
        #[cfg(target_os = "linux")]
        if let Some(image) = crate::package_icons::Package::of(api, item)
            .and_then(|package| crate::package_icons::ready(api, &package))
            .and_then(|bytes| crate::file_icons::package_image(&bytes, PIXELS))
        {
            return Some(image);
        }
        let key = self.file_icons.theme_key(api, item);
        if self.preferences.file_icons == crate::workspace::IconStyle::System {
            if let Some(image) = crate::file_icons::render(&key, PIXELS) {
                return Some(image);
            }
        }
        // BoxBox's glyph, in the colour the views give it.
        let kind = crate::format::Kind::of(&item.name, item.is_dir);
        let tint = self.kind_tint(kind).unwrap_or(if item.is_dir { crate::theme::FOLDER } else { crate::theme::TEXT_SECONDARY });
        let svg = gpui::AssetSource::load(&crate::assets::Assets, &format!("icons/{}.svg", kind.glyph())).ok()??;
        let svg = String::from_utf8_lossy(&svg).replace("currentColor", &format!("#{tint:06x}"));
        crate::file_icons::svg_image(svg.as_bytes(), PIXELS).ok()
    }

    /// Starts making the icon of a pane's entry, drawn at `size`, if no one has.
    pub(crate) fn request_entry_icon(&mut self, target: PaneTarget, index: usize, size: f32, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let Some(item) = pane.items.get(index) else { return };
        let keys = self.file_icons.keys(&pane.api, item, size, self.preferences.file_icons);
        let api = pane.api.clone();
        for key in keys.into_iter().flatten() {
            self.request_icon(key, &api, cx);
        }
    }

    /// Starts making the icons of `item` drawn at `size`, if no one has.
    pub(crate) fn request_item_icon(&mut self, api: &Arc<Api>, item: &FileInfo, size: f32, cx: &mut Context<Self>) {
        let keys = self.file_icons.keys(api, item, size, self.preferences.file_icons);
        for key in keys.into_iter().flatten() {
            self.request_icon(key, api, cx);
        }
    }

    fn request_icon(&mut self, key: CacheKey, _api: &Arc<Api>, cx: &mut Context<Self>) {
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
        #[cfg(target_os = "linux")]
        let api = _api.clone();
        cx.spawn(async move |this, cx| {
            let image = match icon {
                #[cfg(target_os = "linux")]
                IconSource::Package(package) if package.is_slow() => {
                    let bytes = crate::package_icons::queued(api, package).await;
                    cx.background_executor().spawn(async move { bytes.and_then(|bytes| crate::file_icons::package_image(&bytes, pixels)) }).await
                }
                icon => {
                    cx.background_executor()
                        .spawn(async move {
                            match icon {
                                IconSource::Theme(key) => crate::file_icons::render(&key, pixels),
                                #[cfg(target_os = "linux")]
                                IconSource::Package(package) => crate::package_icons::icon(&api, &package).and_then(|bytes| crate::file_icons::package_image(&bytes, pixels)),
                            }
                        })
                        .await
                }
            };
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
        GlyphStyle { glyph: kind.glyph(), tint: self.kind_tint(kind), emblem: special.and_then(emblem) }
    }

    /// The glyph's colour under the chosen icon style; `None` keeps it grey.
    pub(crate) fn kind_tint(&self, kind: crate::format::Kind) -> Option<u32> {
        match (self.preferences.file_icons, kind) {
            (crate::workspace::IconStyle::Tinted, crate::format::Kind::Folder) => Some(self.accent_hex()),
            (crate::workspace::IconStyle::Tinted, kind) => kind.tint(),
            _ => None,
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use gpui::{TestAppContext, VisualTestContext};

    use super::super::features_test::{open, settle};
    use super::*;

    #[gpui::test]
    fn a_deb_row_draws_the_packages_own_icon_over_boxbox_glyphs(cx: &mut TestAppContext) {
        let (view, cx, fixture) = open(cx, &["notes.txt"]);
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(64, 64, image::Rgba([200, 30, 30, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let deb = crate::package_icons::deb_fixture(&[("./usr/share/applications/app.desktop", b"[Desktop Entry]\nIcon=app\n"), ("./usr/share/pixmaps/app.png", &png)], &[]);
        std::fs::write(fixture.root.join("app.deb"), deb).unwrap();
        cx.update(|window, app| {
            view.update(app, |workspace, cx| {
                workspace.preferences.file_icons = super::super::IconStyle::Tinted;
                workspace.refresh(&super::super::Refresh, window, cx);
            })
        });
        cx.run_until_parked();
        // Drawing the rows asks for their icons. A deb is read on its own thread,
        // which the test executor does not wait for.
        settle(cx);
        let size = view.read_with(cx, |workspace, _| crate::components::ListMetrics::new(workspace.preferences.list_zoom).icon + 4.);
        let deb_icon = |cx: &mut VisualTestContext| {
            view.read_with(cx, |workspace, _| {
                let pane = workspace.pane();
                workspace.item_icon(&pane.api, pane.items.iter().find(|item| item.name == "app.deb").unwrap(), size)
            })
        };
        for _ in 0..200 {
            cx.run_until_parked();
            if matches!(deb_icon(cx), EntryIcon::Theme(_)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        view.read_with(cx, |workspace, _| {
            let pane = workspace.pane();
            let icon = |name: &str| workspace.item_icon(&pane.api, pane.items.iter().find(|item| item.name == name).unwrap(), size);
            let EntryIcon::Theme(image) = icon("app.deb") else { panic!("the deb shows its own icon") };
            assert_eq!(image.as_bytes(0).unwrap()[..4], [30, 30, 200, 255], "BGRA pixels of the package's PNG");
            assert!(matches!(icon("notes.txt"), EntryIcon::Glyph));
        });
    }
}
