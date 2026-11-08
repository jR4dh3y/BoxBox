use std::{collections::{HashMap, HashSet, VecDeque}, sync::Arc};

use gpui::{Image, ImageFormat};

use crate::{api::{Api, FileInfo}, format::extension};

use super::{favorites::FavoriteProvider, operations::TextBuffer};

const THUMBNAIL_CACHE_CAPACITY: usize = 96;

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
    images: HashMap<ThumbnailKey, Arc<Image>>,
    insertion_order: VecDeque<ThumbnailKey>,
    pending: HashMap<ThumbnailKey, u64>,
    next_generation: u64,
    failed: HashSet<ThumbnailKey>,
}

impl ThumbnailCache {
    pub fn get(&self, key: &ThumbnailKey) -> Option<Arc<Image>> {
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

    pub fn finish(
        &mut self,
        key: ThumbnailKey,
        generation: u64,
        result: anyhow::Result<Vec<u8>>,
    ) {
        if self.pending.get(&key) != Some(&generation) {
            return;
        }
        self.pending.remove(&key);
        let Ok(bytes) = result else {
            if self.failed.len() >= THUMBNAIL_CACHE_CAPACITY { self.failed.clear(); }
            self.failed.insert(key);
            return;
        };
        while self.images.len() >= THUMBNAIL_CACHE_CAPACITY {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.images.remove(&oldest);
        }
        self.images.insert(
            key.clone(),
            Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
        );
        self.insertion_order.push_back(key);
    }

    pub fn invalidate(&mut self) {
        self.images.clear();
        self.insertion_order.clear();
        self.pending.clear();
        self.failed.clear();
    }
}

pub(crate) enum PreviewContent {
    Loading,
    Text { buffer: TextBuffer, dirty: bool },
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
