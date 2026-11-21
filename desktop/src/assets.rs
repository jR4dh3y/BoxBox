//! Lucide icons extracted from the frontend's lucide-svelte by `scripts/extract-icons.ts`;
//! `build.rs` embeds every file in `assets/icons`. The BoxBox logo is the web app's own file.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{AssetSource, SharedString};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// Full colour, so it is rasterized (`file_icons::svg_image`) and drawn with `img`.
pub const LOGO: &[u8] = include_bytes!("../../frontend/static/logo-boxbox-3d.svg");

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("emblems/") {
            return Ok(crate::emblems::svg(path).map(Cow::Owned));
        }
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

