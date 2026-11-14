use std::{collections::{HashMap, HashSet, VecDeque}, sync::Arc};

use gpui::{Image, RenderImage};

use crate::{api::{Api, FileInfo}, format::extension};

use super::favorites::FavoriteProvider;

/// More than a large screen of tiles, so the visible ones are never evicted.
/// Each 256 px thumbnail takes about 200 KB of decoded pixels.
const THUMBNAIL_CACHE_CAPACITY: usize = 512;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct ThumbnailKey {
    pub provider: FavoriteProvider,
    pub path: String,
    pub mod_time: String,
    pub size: u64,
}

impl ThumbnailKey {
    pub fn new(api: &Api, item: &FileInfo) -> Self {
        Self {
            provider: FavoriteProvider::for_api(api),
            path: item.path.clone(),
            mod_time: item.mod_time.clone(),
            size: item.size,
        }
    }
}

#[derive(Default)]
pub(crate) struct ThumbnailCache {
    images: HashMap<ThumbnailKey, Arc<RenderImage>>,
    insertion_order: VecDeque<ThumbnailKey>,
    pending: HashMap<ThumbnailKey, u64>,
    next_generation: u64,
    failed: HashSet<ThumbnailKey>,
}

impl ThumbnailCache {
    pub fn get(&self, key: &ThumbnailKey) -> Option<Arc<RenderImage>> {
        self.images.get(key).cloned()
    }

    pub fn request(&mut self, key: ThumbnailKey) -> Option<u64> {
        if self.images.contains_key(&key) || self.pending.contains_key(&key) || self.failed.contains(&key) {
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
        while self.images.len() >= THUMBNAIL_CACHE_CAPACITY {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            evicted.extend(self.images.remove(&oldest));
        }
        self.images.insert(key.clone(), image);
        self.insertion_order.push_back(key);
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
        self.insertion_order.clear();
        self.pending.clear();
        self.failed.clear();
        self.images.drain().map(|(_, image)| image).collect()
    }
}

/// Decodes a PNG thumbnail into GPUI's BGRA pixels. A `RenderImage` stays out of
/// GPUI's asset map, which never frees what it holds, so the cache owns its lifetime.
pub(crate) fn decode_thumbnail(png: &[u8]) -> anyhow::Result<Arc<RenderImage>> {
    let mut pixels = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}

pub(crate) enum PreviewContent {
    Loading,
    /// Read-only: editing belongs to the file's own app.
    Text(gpui::SharedString),
    Image(Arc<Image>),
    Failed(String),
}

pub(crate) struct PreviewState {
    pub target_id: usize,
    pub api: Arc<Api>,
    pub item: FileInfo,
    pub content: PreviewContent,
}

pub(crate) fn is_text_preview(name: &str) -> bool {
    matches!(
        extension(name).to_ascii_lowercase().as_str(),
        "txt" | "log" | "csv" | "tsv" | "md" | "rst" | "json" | "yaml" | "yml"
            | "toml" | "ini" | "conf" | "env" | "xml" | "html" | "htm" | "css"
            | "scss" | "sass" | "less" | "js" | "jsx" | "ts" | "tsx" | "mjs"
            | "cjs" | "rs" | "py" | "pyi" | "sh" | "bash" | "zsh" | "fish" | "go"
            | "java" | "kt" | "kts" | "c" | "h" | "cc" | "cpp" | "hpp" | "cs"
            | "rb" | "php" | "sql" | "swift" | "scala" | "lua" | "r" | "pl" | "properties"
    )
}

pub(crate) fn has_inline_thumbnail(name: &str) -> bool {
    matches!(
        extension(name).to_ascii_lowercase().as_str(),
        "bmp" | "gif" | "ico" | "jpg" | "jpeg" | "png" | "qoi" | "tif" | "tiff" | "webp"
            | "pdf" | "mp4" | "webm" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "m4v"
            | "ogv" | "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus"
    )
}
