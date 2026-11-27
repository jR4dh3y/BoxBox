//! Disk and memory cache for WGSL reflection metadata and compiled SPIR-V.
//!
//! GPUI compiles all 8 render pipelines twice on launch (`PreMultiplied` then
//! `Ignored`), parsing and validating a 49 KB WGSL file twice and running
//! `naga::back::spv::write_vec` 32 times. Caching struct spans and SPIR-V +
//! reflection metadata eliminates the second compile on cold runs and all
//! `naga` work on warm launches. (BoxBox patch)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

const MAGIC: &[u8; 8] = b"BBSC0001";

/// Deterministic 128-bit FNV-1a-style hash (stable across Rust releases).
pub(crate) struct Hasher128 {
    a: u64,
    b: u64,
}

impl Hasher128 {
    pub(crate) fn new() -> Self {
        Self {
            a: 0xcbf29ce484222325,
            b: 0x100000001b3 ^ 0x9e3779b97f4a7c15,
        }
    }

    pub(crate) fn bytes(&mut self, data: &[u8]) {
        for &byte in data {
            self.a ^= u64::from(byte);
            self.a = self.a.wrapping_mul(0x100000001b3);
            self.b ^= u64::from(byte).wrapping_add(0x51);
            self.b = self.b.wrapping_mul(0x00000100000001b3 ^ 0x2000000000000000);
        }
        // Mix length so concatenated slices cannot alias.
        self.u64(data.len() as u64);
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.a ^= u64::from(value);
        self.a = self.a.wrapping_mul(0x100000001b3);
        self.b = self.b.rotate_left(7) ^ u64::from(value);
        self.b = self.b.wrapping_mul(0x9e3779b97f4a7c15);
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.u64(u64::from(value));
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.a ^= value;
        self.a = self.a.wrapping_mul(0x100000001b3);
        self.b ^= value.rotate_left(19);
        self.b = self.b.wrapping_mul(0x9e3779b97f4a7c15);
    }

    pub(crate) fn finish(self) -> [u64; 2] {
        [self.a, self.b]
    }
}

pub(crate) fn hash_source(source: &str, ray_query: bool) -> [u64; 2] {
    let mut hasher = Hasher128::new();
    hasher.bytes(source.as_bytes());
    hasher.u8(u8::from(ray_query));
    hasher.finish()
}

pub(crate) fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    let folder = base.join("boxbox-gpui");
    std::fs::create_dir_all(&folder).ok()?;
    Some(folder)
}

#[derive(Clone)]
pub(crate) struct CachedSpv {
    pub(crate) stage: naga::ShaderStage,
    pub(crate) wg_size: [u32; 3],
    pub(crate) spv: Vec<u32>,
    pub(crate) group_infos: Vec<CachedGroupInfo>,
    pub(crate) attribute_mappings: Vec<(u32, u32)>,
}

#[derive(Clone)]
pub(crate) struct CachedGroupInfo {
    pub(crate) visibility: u32,
    pub(crate) binding_access: Vec<u32>,
}

#[derive(Default)]
struct Store {
    structs: HashMap<([u64; 2], u32, bool), Vec<(String, u32)>>,
    spv: HashMap<[u64; 2], CachedSpv>,
}

pub(crate) struct ShaderCache {
    file: Option<PathBuf>,
    store: Mutex<Store>,
    dirty: AtomicBool,
}

impl ShaderCache {
    pub(crate) fn open() -> Self {
        let file = cache_dir().map(|folder| folder.join("shader-cache-v1.bin"));
        let store = file
            .as_ref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| decode(&bytes))
            .unwrap_or_default();
        Self {
            file,
            store: Mutex::new(store),
            dirty: AtomicBool::new(false),
        }
    }

    pub(crate) fn get_structs(
        &self,
        source_hash: [u64; 2],
        source_len: usize,
        ray_query: bool,
    ) -> Option<Vec<(String, u32)>> {
        let len = u32::try_from(source_len).ok()?;
        let store = self.store.lock().ok()?;
        store.structs.get(&(source_hash, len, ray_query)).cloned()
    }

    pub(crate) fn insert_structs(
        &self,
        source_hash: [u64; 2],
        source_len: usize,
        ray_query: bool,
        structs: Vec<(String, u32)>,
    ) {
        let Ok(len) = u32::try_from(source_len) else {
            return;
        };
        if let Ok(mut store) = self.store.lock() {
            store.structs.insert((source_hash, len, ray_query), structs);
            self.dirty.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn get_spv(&self, key: [u64; 2]) -> Option<CachedSpv> {
        self.store.lock().ok()?.spv.get(&key).cloned()
    }

    pub(crate) fn insert_spv(&self, key: [u64; 2], entry: CachedSpv) {
        if let Ok(mut store) = self.store.lock() {
            store.spv.insert(key, entry);
            self.dirty.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub(crate) fn save_if_dirty(&self) {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return;
        }
        let Some(ref file) = self.file else {
            return;
        };
        let bytes = {
            let Ok(store) = self.store.lock() else {
                return;
            };
            encode(&store)
        };
        let temporary = file.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&temporary, &bytes)
            .and_then(|()| std::fs::rename(&temporary, file))
            .is_err()
        {
            let _ = std::fs::remove_file(&temporary);
        }
    }
}

fn stage_to_u8(stage: naga::ShaderStage) -> Option<u8> {
    match stage {
        naga::ShaderStage::Vertex => Some(0),
        naga::ShaderStage::Fragment => Some(1),
        naga::ShaderStage::Compute => Some(2),
        _ => None,
    }
}

fn u8_to_stage(value: u8) -> Option<naga::ShaderStage> {
    match value {
        0 => Some(naga::ShaderStage::Vertex),
        1 => Some(naga::ShaderStage::Fragment),
        2 => Some(naga::ShaderStage::Compute),
        _ => None,
    }
}

fn encode(store: &Store) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(store.structs.len() as u32).to_le_bytes());
    for (&(hash, len, ray_query), list) in &store.structs {
        out.extend_from_slice(&hash[0].to_le_bytes());
        out.extend_from_slice(&hash[1].to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.push(u8::from(ray_query));
        out.extend_from_slice(&(list.len() as u32).to_le_bytes());
        for (name, span) in list {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&span.to_le_bytes());
        }
    }
    let valid_spv: Vec<_> = store
        .spv
        .iter()
        .filter_map(|(key, entry)| Some((*key, stage_to_u8(entry.stage)?, entry)))
        .collect();
    out.extend_from_slice(&(valid_spv.len() as u32).to_le_bytes());
    for (key, stage, entry) in valid_spv {
        out.extend_from_slice(&key[0].to_le_bytes());
        out.extend_from_slice(&key[1].to_le_bytes());
        out.push(stage);
        for dim in entry.wg_size {
            out.extend_from_slice(&dim.to_le_bytes());
        }
        out.extend_from_slice(&(entry.spv.len() as u32).to_le_bytes());
        for &word in &entry.spv {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&(entry.group_infos.len() as u32).to_le_bytes());
        for group in &entry.group_infos {
            out.extend_from_slice(&group.visibility.to_le_bytes());
            out.extend_from_slice(&(group.binding_access.len() as u32).to_le_bytes());
            for &access in &group.binding_access {
                out.extend_from_slice(&access.to_le_bytes());
            }
        }
        out.extend_from_slice(&(entry.attribute_mappings.len() as u32).to_le_bytes());
        for &(buffer_index, attribute_index) in &entry.attribute_mappings {
            out.extend_from_slice(&buffer_index.to_le_bytes());
            out.extend_from_slice(&attribute_index.to_le_bytes());
        }
    }
    out
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.0.split_at_checked(len)?;
        self.0 = tail;
        Some(head)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        std::str::from_utf8(self.take(len)?).ok().map(str::to_owned)
    }
}

fn decode(bytes: &[u8]) -> Option<Store> {
    let mut reader = Reader(bytes);
    if reader.take(8)? != MAGIC {
        return None;
    }
    let mut store = Store::default();
    let struct_count = reader.u32()? as usize;
    for _ in 0..struct_count {
        let hash = [reader.u64()?, reader.u64()?];
        let len = reader.u32()?;
        let ray_query = reader.u8()? != 0;
        let count = reader.u32()? as usize;
        let mut list = Vec::with_capacity(count.min(256));
        for _ in 0..count {
            list.push((reader.string()?, reader.u32()?));
        }
        store.structs.insert((hash, len, ray_query), list);
    }
    let spv_count = reader.u32()? as usize;
    for _ in 0..spv_count {
        let key = [reader.u64()?, reader.u64()?];
        let stage = u8_to_stage(reader.u8()?)?;
        let wg_size = [reader.u32()?, reader.u32()?, reader.u32()?];
        let spv_len = reader.u32()? as usize;
        let spv_bytes = reader.take(spv_len.checked_mul(4)?)?;
        let spv: Vec<u32> = spv_bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        let group_count = reader.u32()? as usize;
        let mut group_infos = Vec::with_capacity(group_count.min(16));
        for _ in 0..group_count {
            let visibility = reader.u32()?;
            let access_count = reader.u32()? as usize;
            let mut binding_access = Vec::with_capacity(access_count.min(32));
            for _ in 0..access_count {
                binding_access.push(reader.u32()?);
            }
            group_infos.push(CachedGroupInfo {
                visibility,
                binding_access,
            });
        }
        let attr_count = reader.u32()? as usize;
        let mut attribute_mappings = Vec::with_capacity(attr_count.min(32));
        for _ in 0..attr_count {
            attribute_mappings.push((reader.u32()?, reader.u32()?));
        }
        store.spv.insert(
            key,
            CachedSpv {
                stage,
                wg_size,
                spv,
                group_infos,
                attribute_mappings,
            },
        );
    }
    if !reader.0.is_empty() {
        return None;
    }
    Some(store)
}
