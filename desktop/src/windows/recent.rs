use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ},
        UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH},
    },
};

pub(crate) fn files(limit: usize) -> Vec<PathBuf> {
    super::with_shell(|| {
        let Some(folder) = super::known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_Recent)
        else {
            return Ok(Vec::new());
        };
        let mut links: Vec<_> = fs::read_dir(folder)?
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let metadata = entry.metadata().ok()?;
                (metadata.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")))
                .then_some((metadata.modified().ok()?, path))
            })
            .collect();
        links.sort_by(|(left, _), (right, _)| right.cmp(left));
        let mut seen = HashSet::new();
        Ok(links
            .into_iter()
            .filter_map(|(_, link)| {
                let path = target(&link).ok()?;
                (path.is_file() && seen.insert(path.clone())).then_some(path)
            })
            .take(limit)
            .collect())
    })
    .unwrap_or_default()
}

fn target(path: &Path) -> Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    let mut buffer = vec![0u16; 32_768];
    let path = super::shell_path(path);
    unsafe {
        let shortcut: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let persist: IPersistFile = shortcut.cast()?;
        persist.Load(PCWSTR(path.as_ptr()), STGM_READ)?;
        shortcut.GetPath(&mut buffer, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32)?;
    }
    let end = buffer
        .iter()
        .position(|&ch| ch == 0)
        .unwrap_or(buffer.len());
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&buffer[..end])))
}
