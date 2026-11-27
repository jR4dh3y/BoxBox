use std::{collections::{HashMap, HashSet}, sync::Arc, time::{Duration, Instant}};

use gpui::RenderImage;

use crate::{api::{Api, FileInfo}, format::extension};

use super::favorites::FavoriteProvider;

/// More than a large screen of tiles. Each 256 px thumbnail takes about 200 KB
/// of decoded pixels.
const THUMBNAIL_CACHE_CAPACITY: usize = 512;
/// An image drawn this recently is on screen, so it is kept even past the
/// capacity: a screen of small tiles with folder previews can show more, and
/// evicting one would only draw it again and evict another.
const ON_SCREEN: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct ThumbnailKey {
    pub provider: FavoriteProvider,
    pub path: String,
    pub mod_time: Option<i64>,
    pub size: u64,
    /// The longest edge it is drawn at (`GridMetrics::thumbnail`).
    pub edge: u32,
}

impl ThumbnailKey {
    pub fn new(api: &Api, item: &FileInfo, edge: u32) -> Self {
        Self {
            provider: FavoriteProvider::for_api(api),
            path: item.path.clone(),
            mod_time: item.mod_time,
            size: item.size,
            edge,
        }
    }
}

#[derive(Default)]
pub(crate) struct ThumbnailCache {
    /// Each image and when a tile last asked for it.
    images: HashMap<ThumbnailKey, (Arc<RenderImage>, Instant)>,
    pending: HashMap<ThumbnailKey, u64>,
    next_generation: u64,
    failed: HashSet<ThumbnailKey>,
}

impl ThumbnailCache {
    pub fn get(&self, key: &ThumbnailKey) -> Option<Arc<RenderImage>> {
        self.images.get(key).map(|(image, _)| image.clone())
    }

    /// Tiles ask every time they are drawn, which marks a cached image as on screen.
    pub fn request(&mut self, key: ThumbnailKey) -> Option<u64> {
        if let Some((_, used)) = self.images.get_mut(&key) {
            *used = Instant::now();
            return None;
        }
        if self.pending.contains_key(&key) || self.failed.contains(&key) {
            return None;
        }
        self.next_generation = self.next_generation.wrapping_add(1);
        let generation = self.next_generation;
        self.pending.insert(key, generation);
        Some(generation)
    }

    /// Stores a finished thumbnail. Returns the images it evicted, which the
    /// caller must release from the GPU with `drop_image`.
    #[must_use]
    pub fn finish(
        &mut self,
        key: ThumbnailKey,
        generation: u64,
        result: anyhow::Result<Arc<RenderImage>>,
    ) -> Vec<Arc<RenderImage>> {
        let mut evicted = Vec::new();
        if self.pending.get(&key) != Some(&generation) {
            return evicted;
        }
        self.pending.remove(&key);
        let Ok(image) = result else {
            if self.failed.len() >= THUMBNAIL_CACHE_CAPACITY { self.failed.clear(); }
            self.failed.insert(key);
            return evicted;
        };
        let now = Instant::now();
        while self.images.len() >= THUMBNAIL_CACHE_CAPACITY {
            let Some((oldest, _)) = self.images.iter().min_by_key(|(_, (_, used))| *used).filter(|(_, (_, used))| now - *used >= ON_SCREEN) else {
                break;
            };
            let oldest = oldest.clone();
            evicted.extend(self.images.remove(&oldest).map(|(image, _)| image));
        }
        self.images.insert(key, (image, now));
        evicted
    }

    /// A request that was dropped unrendered; the tile asks again when it is drawn.
    pub fn forget(&mut self, key: &ThumbnailKey, generation: u64) {
        if self.pending.get(key) == Some(&generation) {
            self.pending.remove(key);
        }
    }

    /// Forgets everything. Returns the images to release from the GPU.
    #[must_use]
    pub fn invalidate(&mut self) -> Vec<Arc<RenderImage>> {
        self.pending.clear();
        self.failed.clear();
        self.images.drain().map(|(_, (image, _))| image).collect()
    }
}

/// Decodes a PNG thumbnail into GPUI's BGRA pixels. A `RenderImage` stays out of
/// GPUI's asset map, which never frees what it holds, so the cache owns its lifetime.
pub(crate) fn decode_thumbnail(png: &[u8]) -> anyhow::Result<Arc<RenderImage>> {
    let mut pixels =
        boxbox_thumbnail::decode_png(std::io::Cursor::new(png), image::Limits::default())?.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}

pub(crate) fn has_inline_thumbnail(name: &str) -> bool {
    matches!(
        extension(name).to_ascii_lowercase().as_str(),
        "bmp" | "gif" | "ico" | "jpg" | "jpeg" | "png" | "qoi" | "tif" | "tiff" | "webp"
            | "pdf" | "mp4" | "webm" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "m4v"
            | "ogv" | "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(index: usize) -> ThumbnailKey {
        ThumbnailKey { provider: FavoriteProvider::Local, path: format!("home/{index}.png"), mod_time: None, size: 0, edge: 128 }
    }

    fn image() -> anyhow::Result<Arc<RenderImage>> {
        Ok(Arc::new(RenderImage::new(vec![image::Frame::new(image::RgbaImage::new(1, 1))])))
    }

    fn add(cache: &mut ThumbnailCache, index: usize) -> usize {
        let generation = cache.request(key(index)).unwrap();
        cache.finish(key(index), generation, image()).len()
    }

    #[test]
    fn images_on_screen_are_kept_and_the_least_recently_drawn_go_first() {
        let mut cache = ThumbnailCache::default();
        for index in 0..THUMBNAIL_CACHE_CAPACITY {
            assert_eq!(add(&mut cache, index), 0);
        }
        // All of them were just drawn: one more grows the cache instead of evicting.
        assert_eq!(add(&mut cache, THUMBNAIL_CACHE_CAPACITY), 0);
        assert_eq!(cache.images.len(), THUMBNAIL_CACHE_CAPACITY + 1);
        // Off screen for a while, except 0, which a tile drew again.
        let past = Instant::now() - ON_SCREEN * 2;
        for (_, used) in cache.images.values_mut() {
            *used = past;
        }
        assert_eq!(cache.request(key(0)), None);
        assert_eq!(add(&mut cache, THUMBNAIL_CACHE_CAPACITY + 1), 2, "back under the capacity");
        assert!(cache.get(&key(0)).is_some());
    }
}
