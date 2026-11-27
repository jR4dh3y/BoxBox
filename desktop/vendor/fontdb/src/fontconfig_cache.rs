//! The system font list from fontconfig's cache instead of parsing every font file.
//!
//! GPUI builds its font database on the UI thread before the first window. Upstream
//! fontdb opens and parses each installed font (about 1,250 faces here): 26 ms with
//! the files in the page cache and 80 ms without, every launch. fontconfig already
//! keeps those facts in its on-disk cache, which lists them in about 9 ms. Files are
//! mapped only once text uses them.
//!
//! Variable and oblique fonts are still parsed: fontconfig gives a variable font's
//! weight as a range, and cannot say whether an oblique face sets the italic bit.
//! When libfontconfig cannot be loaded, `load` returns false and the caller falls
//! back to upstream's file scan.

use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use fontconfig_sys::constants::*;
use fontconfig_sys::statics::LIB_RESULT;
use fontconfig_sys::{Fc, FcPattern, FcResultMatch, FcSetSystem};

use crate::{Database, FaceInfo, Language, Source, Stretch, Style, Weight, ID};

/// Adds every face fontconfig lists. False when fontconfig is unavailable.
pub(crate) fn load(db: &mut Database) -> bool {
    let Ok(fc) = LIB_RESULT.as_ref() else { return false };
    // SAFETY: the font set belongs to the current config, which outlives this call;
    // patterns are only read, and each string is copied before the next call.
    let patterns = unsafe {
        let config = (fc.FcConfigGetCurrent)();
        if config.is_null() {
            return false;
        }
        let set = (fc.FcConfigGetFonts)(config, FcSetSystem);
        if set.is_null() || (*set).fonts.is_null() {
            return false;
        }
        std::slice::from_raw_parts((*set).fonts, (*set).nfont.max(0) as usize)
    };
    let mut files: HashMap<PathBuf, Arc<dyn AsRef<[u8]> + Send + Sync>> = HashMap::new();
    let mut parse: HashSet<PathBuf> = HashSet::new();
    for &pattern in patterns {
        let Some(face) = (unsafe { Pattern { fc, pattern }.face() }) else { continue };
        match face {
            Face::Parse(path) => {
                parse.insert(path);
            }
            Face::Static { path, info } => {
                let data = files.entry(path.clone()).or_insert_with(|| Arc::new(LazyMap::new(path.clone()))).clone();
                db.push_face_info(FaceInfo { source: Source::SharedFile(path, data), ..info });
            }
        }
    }
    for path in parse {
        if let Err(error) = db.load_font_file(&path) {
            log::warn!("Failed to load '{}' cause {}.", path.display(), error);
        }
    }
    true
}

enum Face {
    Static { path: PathBuf, info: FaceInfo },
    /// fontconfig does not hold everything upstream reads from this file.
    Parse(PathBuf),
}

struct Pattern<'a> {
    fc: &'a Fc,
    pattern: *mut FcPattern,
}

impl Pattern<'_> {
    /// SAFETY: `pattern` must be a live fontconfig pattern.
    unsafe fn face(&self) -> Option<Face> {
        let path = PathBuf::from(self.string(FC_FILE, 0)?);
        // The same file types upstream's directory scan reads.
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        if !matches!(extension.as_str(), "ttf" | "ttc" | "otf" | "otc") {
            return None;
        }
        let index = self.integer(FC_INDEX).unwrap_or(0);
        // A named instance of a variable font; upstream lists only the default.
        if index >> 16 != 0 {
            return None;
        }
        let (Some(weight), Some(width), Some(post_script_name)) =
            (self.number(FC_WEIGHT), self.number(FC_WIDTH), self.string(FC_POSTSCRIPT_NAME, 0))
        else {
            return Some(Face::Parse(path));
        };
        let style = match self.integer(FC_SLANT) {
            Some(FC_SLANT_ITALIC) => Style::Italic,
            Some(FC_SLANT_OBLIQUE) => return Some(Face::Parse(path)),
            _ => Style::Normal,
        };
        let mut families = Vec::new();
        while let Some(family) = self.string(FC_FAMILY, families.len() as i32) {
            let language = match self.string(FC_FAMILYLANG, families.len() as i32).as_deref() {
                Some("en") => Language::English_UnitedStates,
                _ => Language::Unknown,
            };
            families.push((family, language));
        }
        if families.is_empty() {
            return None;
        }
        let info = FaceInfo {
            id: ID::dummy(),
            source: Source::File(path.clone()),
            index: index as u32,
            families,
            post_script_name,
            style,
            weight: Weight(open_type_weight(weight)),
            stretch: stretch(width),
            monospaced: self.integer(FC_SPACING) == Some(FC_MONO),
        };
        Some(Face::Static { path, info })
    }

    unsafe fn string(&self, object: &CStr, n: i32) -> Option<String> {
        let mut value = std::ptr::null_mut();
        ((self.fc.FcPatternGetString)(self.pattern, object.as_ptr(), n, &mut value) == FcResultMatch && !value.is_null())
            .then(|| CStr::from_ptr(value.cast()).to_string_lossy().into_owned())
    }

    unsafe fn integer(&self, object: &CStr) -> Option<i32> {
        let mut value = 0;
        ((self.fc.FcPatternGetInteger)(self.pattern, object.as_ptr(), 0, &mut value) == FcResultMatch).then_some(value)
    }

    /// A single number; a variable font's axis is a range and gives None.
    unsafe fn number(&self, object: &CStr) -> Option<f64> {
        let mut value = 0.0;
        ((self.fc.FcPatternGetDouble)(self.pattern, object.as_ptr(), 0, &mut value) == FcResultMatch).then_some(value)
    }
}

/// fontconfig's weight scale back to OpenType's, inverting `FcWeightFromOpenTypeDouble`.
fn open_type_weight(weight: f64) -> u16 {
    const MAP: [(f64, f64); 12] = [
        (0., 100.), (40., 200.), (50., 300.), (55., 350.), (75., 380.), (80., 400.),
        (100., 500.), (180., 600.), (200., 700.), (205., 800.), (210., 900.), (215., 1000.),
    ];
    if weight <= 0. {
        return 100;
    }
    for pair in MAP.windows(2) {
        let ((fc_low, ot_low), (fc_high, ot_high)) = (pair[0], pair[1]);
        if weight <= fc_high {
            return (ot_low + (weight - fc_low) * (ot_high - ot_low) / (fc_high - fc_low)).round() as u16;
        }
    }
    1000
}

/// fontconfig's width percentage to the nearest OpenType width class.
fn stretch(width: f64) -> Stretch {
    const CLASSES: [(f64, Stretch); 9] = [
        (50., Stretch::UltraCondensed), (62.5, Stretch::ExtraCondensed), (75., Stretch::Condensed),
        (87.5, Stretch::SemiCondensed), (100., Stretch::Normal), (112.5, Stretch::SemiExpanded),
        (125., Stretch::Expanded), (150., Stretch::ExtraExpanded), (200., Stretch::UltraExpanded),
    ];
    CLASSES.iter().min_by(|a, b| (a.0 - width).abs().total_cmp(&(b.0 - width).abs())).map_or(Stretch::Normal, |class| class.1)
}

/// A font file mapped the first time its bytes are read.
struct LazyMap {
    path: PathBuf,
    map: OnceLock<Option<memmap2::Mmap>>,
}

impl LazyMap {
    fn new(path: PathBuf) -> Self {
        Self { path, map: OnceLock::new() }
    }
}

impl AsRef<[u8]> for LazyMap {
    /// A file that cannot be mapped reads as empty, which no font parser accepts.
    fn as_ref(&self) -> &[u8] {
        self.map.get_or_init(|| map(&self.path)).as_deref().unwrap_or(&[])
    }
}

fn map(path: &Path) -> Option<memmap2::Mmap> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: as upstream: another process may change the file under the mapping.
    unsafe { memmap2::MmapOptions::new().map(&file).ok() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache gives what parsing every file gives, apart from the monospace flag:
    /// fontconfig measures advances where upstream reads the `post` table.
    #[test]
    fn matches_parsing_the_files() {
        let mut cached = Database::new();
        assert!(load(&mut cached), "libfontconfig is not available");
        let mut parsed = Database::new();
        parsed.load_fontconfig();
        let key = |face: &FaceInfo| match &face.source {
            Source::File(path) | Source::SharedFile(path, _) => (path.clone(), face.index),
            Source::Binary(_) => unreachable!(),
        };
        let cached: HashMap<_, _> = cached.faces().map(|face| (key(face), face.clone())).collect();
        assert!(!cached.is_empty());
        assert_eq!(cached.len(), parsed.faces().count());
        for face in parsed.faces() {
            let found = cached.get(&key(face)).unwrap_or_else(|| panic!("missing {:?}", key(face)));
            assert_eq!(found.families[0].0, face.families[0].0, "{:?}", key(face));
            assert_eq!(found.post_script_name, face.post_script_name, "{:?}", key(face));
            assert_eq!((found.style, found.weight, found.stretch), (face.style, face.weight, face.stretch), "{:?}", key(face));
        }
    }

    #[test]
    fn weights_round_trip() {
        for (fc, ot) in [(0., 100), (80., 400), (100., 500), (200., 700), (210., 900), (90., 450)] {
            assert_eq!(open_type_weight(fc), ot);
        }
    }
}
