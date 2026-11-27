//! The window's icons: one image per source and pixel size, made off the UI
//! thread the first time a row or tile asks. A file's own icon (a Linux
//! package's, a Windows executable's) wins once ready; until then, or without
//! one, views show the selected icon style.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Context, RenderImage};

use super::operations::PaneTarget;
use super::Workspace;
use crate::api::{Api, FileInfo};
use crate::components::EntryIcon;
use crate::file_icons::{bucket, IconKey};

/// Enough for every type in a few folders at two zoom steps; past it, the
/// icons not drawn in the last two frames are dropped.
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
    /// The icon of the app that opens files of this kind.
    #[cfg(target_os = "linux")]
    App(IconKey),
    /// A local `.exe`'s embedded icon; its size and time make a changed file a new key.
    #[cfg(windows)]
    Executable { path: String, bytes: u64, modified: Option<i64> },
}

type CacheKey = (IconSource, u32);

#[derive(Default)]
pub(crate) struct FileIcons {
    slots: HashMap<CacheKey, Slot>,
    /// The frame each key was last asked for in. Views ask for what they draw
    /// every frame, so a recent frame means the icon is on screen.
    drawn: HashMap<CacheKey, u64>,
    /// Counts the window's renders.
    pub frame: u64,
    /// The size that starts the next pass, past `CAPACITY` when more are on screen.
    limit: usize,
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

    /// The file's own icon, in every icon style; its app's, when Settings ask; and the theme's.
    fn keys(&self, api: &Api, item: &FileInfo, size: f32, preferences: &super::Preferences) -> [Option<CacheKey>; 3] {
        let pixels = bucket(size, self.scale.max(1.));
        let theme = (preferences.file_icons == super::IconStyle::System).then(|| (IconSource::Theme(self.theme_key(api, item)), pixels));
        #[cfg(target_os = "linux")]
        let embedded = crate::package_icons::Package::of(api, item).map(|package| (IconSource::Package(package), pixels));
        #[cfg(target_os = "linux")]
        let app = (preferences.app_icons && !item.is_dir).then(|| (IconSource::App(self.theme_key(api, item)), pixels));
        #[cfg(windows)]
        let embedded = (api.is_local() && !item.is_dir
            && std::path::Path::new(&item.name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe")))
            .then(|| (IconSource::Executable { path: item.path.clone(), bytes: item.size, modified: item.mod_time }, pixels));
        #[cfg(not(target_os = "linux"))]
        let app = None;
        [embedded, app, theme]
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
        let [embedded, app, theme] = self.file_icons.keys(api, item, size, &self.preferences);
        if let Some(Slot::Ready(image)) = embedded.and_then(|key| self.file_icons.slots.get(&key)) {
            return EntryIcon::Theme(image.clone());
        }
        // The app's icon is quick to find; waiting for it keeps the glyph from flashing first.
        match app.as_ref().map(|key| self.file_icons.slots.get(key)) {
            Some(Some(Slot::Ready(image))) => return EntryIcon::Theme(image.clone()),
            Some(Some(Slot::Pending) | None) => return EntryIcon::Waiting,
            Some(Some(Slot::Missing)) | None => {}
        }
        let Some(theme) = theme else { return EntryIcon::Glyph };
        match self.file_icons.slots.get(&theme) {
            Some(Slot::Ready(image)) => EntryIcon::Theme(image.clone()),
            Some(Slot::Missing) => EntryIcon::Glyph,
            Some(Slot::Pending) | None => EntryIcon::Waiting,
        }
    }

    /// Makes app icons again whenever file associations change, such as a new
    /// default set in Open With or in another app.
    pub(crate) fn watch_associations(&mut self, cx: &mut Context<Self>) {
        let (changed, changes) = async_channel::unbounded();
        #[cfg(target_os = "linux")]
        crate::associations::watch(changed);
        #[cfg(windows)]
        crate::windows::associations::watch(changed);
        cx.spawn(async move |this, cx| {
            while changes.recv().await.is_ok() {
                if this.update(cx, |this, cx| this.forget_app_icons(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Drops every app icon; the views ask for them again as they draw, and an
    /// icon still being made is dropped when it arrives. Windows has no separate
    /// app icon: the Shell's file type icon is the default app's.
    fn forget_app_icons(&mut self, cx: &mut Context<Self>) {
        self.file_icons.slots.retain(|(source, _), slot| {
            #[cfg(target_os = "linux")]
            let app = matches!(source, IconSource::App(_));
            #[cfg(windows)]
            let app = matches!(source, IconSource::Theme(IconKey::File(_)));
            if let (true, Slot::Ready(image)) = (app, &*slot) {
                cx.drop_image(image.clone(), None);
            }
            !app
        });
        cx.notify();
    }

    /// The icon of the app that opens `item`, once made at `size` for its own
    /// slot, to mark its thumbnail. `None` when Settings turn app icons off.
    pub(crate) fn app_badge(&self, api: &Api, item: &FileInfo, size: f32) -> Option<Arc<RenderImage>> {
        let [_, app, _] = self.file_icons.keys(api, item, size, &self.preferences);
        match self.file_icons.slots.get(&app?)? {
            Slot::Ready(image) => Some(image.clone()),
            Slot::Pending | Slot::Missing => None,
        }
    }

    /// What a drag to another app shows for `item`: its thumbnail when one is
    /// made, else the icon the views draw for it. Made here when not ready: one
    /// small icon, once per drag. A package's own icon is used only when already
    /// read, as reading it can take seconds.
    #[cfg(target_os = "linux")]
    pub(crate) fn drag_picture(&self, api: &Arc<Api>, item: &FileInfo) -> Option<Arc<RenderImage>> {
        const PIXELS: u32 = 128;
        if let Some(thumbnail) = self.thumbnail(api, item) {
            return Some(thumbnail);
        }
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
        // The glyphs carry a black stroke, since GPUI draws them as masks (`extract-icons.ts`).
        let svg = String::from_utf8_lossy(&svg).replace(r##"stroke="#000""##, &format!(r##"stroke="#{tint:06x}""##));
        crate::file_icons::svg_image(svg.as_bytes(), PIXELS).ok()
    }

    /// Starts making the icon of a pane's entry, drawn at `size`, if no one has.
    pub(crate) fn request_entry_icon(&mut self, target: PaneTarget, index: usize, size: f32, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let Some(item) = pane.items.get(index).cloned() else { return };
        let api = pane.api.clone();
        self.request_item_icon(&api, &item, size, cx);
    }

    /// Starts making the icons of `item` drawn at `size`, if no one has.
    pub(crate) fn request_item_icon(&mut self, api: &Arc<Api>, item: &FileInfo, size: f32, cx: &mut Context<Self>) {
        self.resolve_folder_look(api, item, cx);
        let keys = self.file_icons.keys(api, item, size, &self.preferences);
        for key in keys.into_iter().flatten() {
            self.request_icon(key, api, cx);
        }
    }

    fn request_icon(&mut self, key: CacheKey, api: &Arc<Api>, cx: &mut Context<Self>) {
        let icons = &mut self.file_icons;
        let frame = icons.frame;
        match icons.drawn.get_mut(&key) {
            Some(seen) => *seen = frame,
            None => {
                icons.drawn.insert(key.clone(), frame);
            }
        }
        if icons.slots.contains_key(&key) {
            return;
        }
        if icons.slots.len() >= icons.limit.max(CAPACITY) {
            // This frame may not have reached every row yet, so the last one counts too.
            let on_screen = |drawn: &HashMap<CacheKey, u64>, key: &CacheKey| drawn.get(key).is_some_and(|seen| *seen + 1 >= frame);
            let drawn = &icons.drawn;
            icons.slots.retain(|key, slot| {
                let keep = on_screen(drawn, key);
                if let (false, Slot::Ready(image)) = (keep, &*slot) {
                    cx.drop_image(image.clone(), None);
                }
                keep
            });
            let slots = &icons.slots;
            icons.drawn.retain(|key, _| slots.contains_key(key));
            icons.drawn.insert(key.clone(), frame);
            // Room for twice what is on screen, so a screen of more than
            // `CAPACITY` icons does not start a pass every frame.
            icons.limit = icons.slots.len() * 2;
        }
        self.file_icons.slots.insert(key.clone(), Slot::Pending);
        let (icon, pixels) = key.clone();
        let api = api.clone();
        cx.spawn(async move |this, cx| {
            #[cfg_attr(not(target_os = "linux"), allow(clippy::match_single_binding))]
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
                                #[cfg(target_os = "linux")]
                                IconSource::App(key) => crate::file_icons::render_app(&key, pixels),
                                #[cfg(windows)]
                                IconSource::Executable { path, .. } => crate::file_icons::render_executable(&api, &path, pixels),
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
    pub emblem: Option<crate::components::Emblem>,
}

impl Workspace {
    /// BoxBox's glyph for `item`: in the tinted set folders take the accent
    /// and every other kind keeps its own colour.
    pub(crate) fn glyph_style(&self, api: &Api, item: &FileInfo) -> GlyphStyle {
        let kind = crate::format::Kind::of(&item.name, item.is_dir);
        if !item.is_dir {
            return GlyphStyle { glyph: kind.glyph(), tint: self.kind_tint(kind), emblem: None };
        }
        let look = self.folder_look(api, &item.path);
        let emblem = Self::look_emblem(look, self.special_emblem(api, item));
        GlyphStyle { glyph: kind.glyph(), tint: look.and_then(|look| look.color).or(self.kind_tint(kind)), emblem }
    }

    /// The emblem a special folder such as Downloads carries.
    pub(crate) fn special_emblem(&self, api: &Api, folder: &FileInfo) -> Option<&'static str> {
        let theme_icon = api.is_local().then(|| self.file_icons.special.get(&folder.path)).flatten()?;
        Some(crate::emblems::find(emblem(theme_icon)?)?.name)
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
                // The text file's glyph is the control; its app's icon would depend on this machine.
                workspace.preferences.app_icons = false;
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn executable_icons_use_the_files_resources_in_every_icon_style() {
        let root = tempfile::tempdir().unwrap();
        let windows = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap());
        for (source, folder) in [(windows.join("System32/cmd.exe"), "first"), (windows.join("regedit.exe"), "second")] {
            let folder = root.path().join(folder);
            std::fs::create_dir(&folder).unwrap();
            std::fs::copy(source, folder.join("APP.EXE")).unwrap();
        }
        std::fs::write(root.path().join("plain.exe"), "no icon resources").unwrap();
        let api = Api::local_with_root(root.path().to_path_buf()).unwrap();
        let icons = FileIcons::default();
        let first = api.stat("home/first/APP.EXE").unwrap();
        let second = api.stat("home/second/APP.EXE").unwrap();
        let preferences = |file_icons| super::super::Preferences { file_icons, ..Default::default() };
        for style in [super::super::IconStyle::System, super::super::IconStyle::Tinted, super::super::IconStyle::Grey] {
            let [first_key, ..] = icons.keys(&api, &first, 96., &preferences(style));
            let [second_key, ..] = icons.keys(&api, &second, 96., &preferences(style));
            assert!(first_key.is_some() && second_key.is_some());
            assert_ne!(first_key, second_key);
        }
        let mut changed = first.clone();
        changed.size += 1;
        let tinted = preferences(super::super::IconStyle::Tinted);
        assert_ne!(icons.keys(&api, &first, 96., &tinted), icons.keys(&api, &changed, 96., &tinted));

        let first_icon = crate::file_icons::render_executable(&api, &first.path, 96).unwrap();
        let second_icon = crate::file_icons::render_executable(&api, &second.path, 96).unwrap();
        assert_ne!(first_icon.as_bytes(0), second_icon.as_bytes(0));
        assert!(crate::file_icons::render_executable(&api, "home/plain.exe", 96).is_none());
    }
}
