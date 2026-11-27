//! Lucide icons extracted from the frontend's lucide-svelte by `scripts/extract-icons.ts`;
//! `build.rs` embeds every file in `assets/icons`. The BoxBox logo is the web app's own file.

use std::borrow::Cow;
use std::io::Read as _;
use std::sync::OnceLock;

use anyhow::Result;
use flate2::read::DeflateDecoder;
use gpui::{AssetSource, SharedString};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

static ICONS_COMPRESSED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icons.deflate"));

fn icon_blob() -> &'static [u8] {
    static BLOB: OnceLock<Box<[u8]>> = OnceLock::new();
    BLOB.get_or_init(|| {
        let mut out = Vec::with_capacity(64 * 1024);
        DeflateDecoder::new(ICONS_COMPRESSED).read_to_end(&mut out).expect("decompress icons");
        out.into_boxed_slice()
    })
}

/// Full colour, so it is rasterized (`file_icons::svg_image`) and drawn with `img`.
pub const LOGO: &[u8] = include_bytes!("../../frontend/static/logo-boxbox-3d.svg");

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("emblems/") {
            return Ok(crate::emblems::svg(path).map(Cow::Owned));
        }
        Ok(ICONS
            .binary_search_by_key(&path, |(name, _, _)| *name)
            .ok()
            .map(|index| {
                let (_, start, end) = ICONS[index];
                Cow::Borrowed(&icon_blob()[start..end])
            }))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _, _)| name.starts_with(path))
            .map(|(name, _, _)| SharedString::from(*name))
            .collect())
    }
}

