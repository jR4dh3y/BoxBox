use std::{io::{Cursor, Read}, path::{Path, PathBuf}, sync::Arc};

use anyhow::{bail, Context as _, Result};
use gpui::{div, img, prelude::*, px, Context, Image, ImageFormat, ObjectFit, Rgba, Window, WindowBackgroundAppearance};
use image::{GenericImageView, ImageReader, Limits};
use sha2::{Digest, Sha256};

use crate::{settings_state::WallpaperFit, theme::*};
use super::Workspace;

#[derive(Default)]
pub(crate) enum WallpaperState {
    #[default]
    Empty,
    Loading,
    Ready { image: Arc<Image>, frosted: Arc<Image>, tile: Arc<Image>, frosted_tile: Arc<Image>, dimensions: (u32, u32), tile_dimensions: (u32, u32), tint: u32, accent: Option<u32> },
    Failed(String),
}

// Decode and blur away from the UI thread, with the same crop for both variants.
fn load_wallpaper(path: &Path) -> Result<WallpaperState> {
    decode_wallpaper(&read_wallpaper(path)?)
}

fn read_wallpaper(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 32 * 1024 * 1024 { bail!("Wallpaper must be smaller than 32 MB."); }
    Ok(bytes)
}

fn decode_wallpaper(bytes: &[u8]) -> Result<WallpaperState> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(160 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().context("Choose a supported image file")?;
    let accent = super::palette::wallpaper_accent(&decoded);
    let tint = accent.unwrap_or(0x4a9eff);
    let dimensions = decoded.dimensions();
    // Expand tiny tiles before rendering, so a 1px image cannot create millions of elements.
    let tile_dimensions = (dimensions.0.max(64), dimensions.1.max(64));
    let mut tile = image::RgbaImage::new(tile_dimensions.0, tile_dimensions.1);
    let rgba = decoded.to_rgba8();
    for (x, y, pixel) in tile.enumerate_pixels_mut() {
        *pixel = *rgba.get_pixel(x % dimensions.0, y % dimensions.1);
    }
    let softened = decoded.resize(1280, 1280, image::imageops::FilterType::Triangle).blur(24.);
    // Blur a repeated border as well, so the tile's opposite edges remain continuous.
    let padding = 72;
    let bordered = image::RgbaImage::from_fn(tile.width() + padding * 2, tile.height() + padding * 2, |x, y| {
        *tile.get_pixel((x + tile.width() - padding % tile.width()) % tile.width(), (y + tile.height() - padding % tile.height()) % tile.height())
    });
    let frosted_tile = image::imageops::crop_imm(&image::imageops::blur(&bordered, 24.), padding, padding, tile.width(), tile.height()).to_image();
    let encode = |image: image::DynamicImage| -> Result<Arc<Image>> {
        let mut output = Cursor::new(Vec::new());
        image.write_to(&mut output, image::ImageFormat::Png)?;
        Ok(Arc::new(Image::from_bytes(ImageFormat::Png, output.into_inner())))
    };
    let image = if decoded.dimensions().0.max(decoded.dimensions().1) > 4096 {
        decoded.resize(4096, 4096, image::imageops::FilterType::Triangle)
    } else { decoded };
    Ok(WallpaperState::Ready { image: encode(image)?, frosted: encode(softened)?, tile: encode(tile.into())?, frosted_tile: encode(frosted_tile.into())?, dimensions, tile_dimensions, tint, accent })
}

impl Workspace {
    pub(crate) fn install_wallpaper(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.wallpaper_generation = self.wallpaper_generation.wrapping_add(1);
        let generation = self.wallpaper_generation;
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                let bytes = read_wallpaper(&path)?;
                let image = decode_wallpaper(&bytes)?;
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let stored = crate::settings_state::store_wallpaper(&bytes, &digest).map_err(anyhow::Error::msg)?;
                Ok::<_, anyhow::Error>((stored, image))
            }).await;
            let _ = this.update(cx, |this, cx| {
                if this.wallpaper_generation != generation { return; }
                match result {
                    Ok((path, image)) => {
                        this.desktop_settings.wallpaper = Some(path);
                        this.wallpaper = image;
                        this.save_desktop_settings(cx);
                    }
                    Err(error) => { this.settings_panel.error = Some(format!("Could not use wallpaper: {error:#}")); cx.notify(); }
                }
            });
        }).detach();
    }

    pub(crate) fn reload_wallpaper(&mut self, cx: &mut Context<Self>) {
        log::debug!("Reloading wallpaper, configured={}, settings_error={}", self.desktop_settings.wallpaper.is_some(), self.desktop_settings.load_error.is_some());
        self.wallpaper_generation = self.wallpaper_generation.wrapping_add(1);
        let generation = self.wallpaper_generation;
        let Some(path) = self.desktop_settings.wallpaper.clone() else {
            self.wallpaper = WallpaperState::Empty;
            cx.notify();
            return;
        };
        self.wallpaper = WallpaperState::Loading;
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { load_wallpaper(&path) }).await;
            let _ = this.update(cx, |this, cx| {
                if this.wallpaper_generation != generation { return; }
                this.wallpaper = result.unwrap_or_else(|error| {
                    log::warn!("Could not load wallpaper: {error:#}");
                    WallpaperState::Failed(format!("Could not load wallpaper: {error:#}"))
                });
                log::debug!("Wallpaper ready={}", this.has_wallpaper());
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn accent_hex(&self) -> u32 {
        if self.desktop_settings.accent_from_wallpaper {
            if let WallpaperState::Ready { accent: Some(accent), .. } = self.wallpaper { return accent; }
        }
        self.desktop_settings.custom_accent.unwrap_or_else(|| self.desktop_settings.accent.hex())
    }

    pub(crate) fn selection_color(&self, hover: bool) -> Rgba {
        c(mix(self.accent_hex(), 0x15171c, if hover { 0.36 } else { 0.30 }))
    }

    pub(crate) fn has_wallpaper(&self) -> bool { matches!(self.wallpaper, WallpaperState::Ready { .. }) }

    /// The window shows what is behind it instead of a wallpaper: the compositor's
    /// blur of the desktop, when it blurs.
    pub(crate) fn transparent(&self) -> bool {
        self.desktop_settings.transparent
    }

    /// Whether the panels are translucent over something: a wallpaper or the desktop.
    fn see_through(&self) -> bool {
        self.transparent() || self.has_wallpaper()
    }

    /// The window's own fill, under the panels.
    pub(crate) fn window_background(&self) -> Rgba {
        if self.transparent() { alpha(0, 0.) } else { c(SURFACE_PRIMARY) }
    }

    /// Tells the window to draw see-through or opaque, when that changed.
    pub(crate) fn apply_window_background(&mut self, window: &mut Window) {
        let transparent = self.transparent();
        if self.window_transparent != Some(transparent) {
            self.window_transparent = Some(transparent);
            // Blurred asks for KDE's blur protocol where there is one; Hyprland and niri
            // blur transparent windows by their own rules.
            window.set_background_appearance(if transparent { WindowBackgroundAppearance::Blurred } else { WindowBackgroundAppearance::Opaque });
        }
    }

    pub(crate) fn shell_background(&self) -> Rgba {
        if self.transparent() {
            return alpha(0x121418, 0.68);
        }
        if let WallpaperState::Ready { tint, .. } = self.wallpaper {
            if self.preferences.wallpaper_tint { return alpha(mix(tint, 0x121418, 0.18), 0.8); }
            alpha(0x121418, 0.68)
        } else { c(SURFACE_PRIMARY) }
    }

    pub(crate) fn chrome_background(&self) -> Rgba {
        if self.see_through() { alpha(0x121418, 0.3) } else { c(SURFACE_SECONDARY) }
    }

    pub(crate) fn file_background(&self) -> Rgba {
        if self.see_through() { alpha(0, 0.) } else { c(SURFACE_PRIMARY) }
    }

    pub(crate) fn render_wallpaper(&self, window: &Window) -> impl IntoElement {
        let mut layer = div().absolute().inset_0().size_full().overflow_hidden();
        if self.transparent() {
            return layer;
        }
        if let WallpaperState::Ready { image, frosted, tile, frosted_tile, dimensions, tile_dimensions, .. } = &self.wallpaper {
            let fit = match self.desktop_settings.wallpaper_fit {
                WallpaperFit::Fit => ObjectFit::Contain,
                WallpaperFit::Fill => ObjectFit::Cover,
                WallpaperFit::Stretch => ObjectFit::Fill,
                WallpaperFit::Center | WallpaperFit::Tile => ObjectFit::None,
            };
            let source = if self.preferences.frosted_glass { frosted.clone() } else { image.clone() };
            if self.desktop_settings.wallpaper_fit == WallpaperFit::Tile {
                let tile = if self.preferences.frosted_glass { frosted_tile } else { tile };
                let (width, height) = (tile_dimensions.0 as f32, tile_dimensions.1 as f32);
                let viewport = window.viewport_size();
                for y in 0..(f32::from(viewport.height) / height).ceil() as u32 {
                    for x in 0..(f32::from(viewport.width) / width).ceil() as u32 {
                        layer = layer.child(img(tile.clone()).absolute().left(px(x as f32 * width)).top(px(y as f32 * height)).w(px(width)).h(px(height)));
                    }
                }
                return layer.child(div().absolute().inset_0().bg(alpha(0x080a0e, 0.1)));
            }
            if self.desktop_settings.wallpaper_fit == WallpaperFit::Center {
                return layer.flex().items_center().justify_center()
                    .child(img(source).flex_none().w(px(dimensions.0 as f32)).h(px(dimensions.1 as f32)).object_fit(ObjectFit::Fill))
                    .child(div().absolute().inset_0().bg(alpha(0x080a0e, 0.1)));
            }
            layer = layer.child(img(source)
                .size_full().object_fit(fit))
                .child(div().absolute().size_full().bg(alpha(0x080a0e, 0.1)));
        }
        layer
    }
}

fn mix(color: u32, base: u32, amount: f32) -> u32 {
    let channel = |shift: u32| {
        let foreground = ((color >> shift) & 255u32) as f32;
        let background = ((base >> shift) & 255u32) as f32;
        (foreground * amount + background * (1. - amount)).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}
