//! The browser resolves the CSS `sans-serif` and `monospace` stacks through
//! fontconfig; resolve them the same way, through its library.

use gpui::SharedString;

use crate::workspace::Fonts;

pub fn resolve() -> Fonts {
    let _span = crate::profile::span("resolve fonts");
    Fonts {
        sans: family(c"sans-serif").unwrap_or_else(|| ".SystemUIFont".into()),
        mono: family(c"monospace").unwrap_or_else(|| "monospace".into()),
    }
}

#[cfg(target_os = "linux")]
fn family(generic: &std::ffi::CStr) -> Option<SharedString> {
    use fontconfig_sys::{constants::FC_FAMILY, statics::LIB_RESULT, FcMatchPattern, FcResultMatch, FcResultNoMatch};
    // GPUI's font loader opens the same library at run time; share its handle.
    let fc = LIB_RESULT.as_ref().ok()?;
    // SAFETY: each pattern is checked for null before use and destroyed once; the
    // family string belongs to `font` and is copied out before `font` is destroyed.
    unsafe {
        let pattern = (fc.FcNameParse)(generic.as_ptr().cast());
        if pattern.is_null() {
            return None;
        }
        (fc.FcConfigSubstitute)(std::ptr::null_mut(), pattern, FcMatchPattern);
        (fc.FcDefaultSubstitute)(pattern);
        let mut result = FcResultNoMatch;
        let font = (fc.FcFontMatch)(std::ptr::null_mut(), pattern, &mut result);
        (fc.FcPatternDestroy)(pattern);
        if font.is_null() {
            return None;
        }
        let mut name = std::ptr::null_mut();
        let family = ((fc.FcPatternGetString)(font, FC_FAMILY.as_ptr(), 0, &mut name) == FcResultMatch && !name.is_null())
            .then(|| std::ffi::CStr::from_ptr(name.cast()).to_string_lossy().trim().to_string())
            .filter(|family| !family.is_empty())
            .map(SharedString::from);
        (fc.FcPatternDestroy)(font);
        family
    }
}

#[cfg(not(target_os = "linux"))]
fn family(_generic: &std::ffi::CStr) -> Option<SharedString> {
    None
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn generic_families_resolve_to_installed_fonts() {
        let fonts = super::resolve();
        assert!(!fonts.sans.is_empty() && !fonts.mono.is_empty());
    }
}
