//! The Shell's OLE drag source uses the same data object format as Explorer.

use std::{
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::Arc,
};

use anyhow::Result;
use gpui::{point, px, App, Bounds, DragMoveEvent, Global, Window};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        System::{
            Com::{CoTaskMemFree, IDataObject},
            Ole::{DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE},
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
            Shell::{
                BHID_DataObject, Common::ITEMIDLIST, SHCreateShellItemArrayFromIDLists,
                SHDoDragDrop, SHParseDisplayName,
            },
        },
    },
};

use super::{stage, DraggedItem, Prepare};
use crate::api::{Api, FileInfo};

#[derive(Default)]
struct Pending {
    sequence: u64,
    paths: Option<(bool, Prepare)>,
}

impl Global for Pending {}

pub(super) fn prepare(files: Vec<FileInfo>, api: Arc<Api>, cx: &mut App) {
    if !cx.has_global::<Pending>() {
        cx.set_global(Pending::default());
    }
    let pending = cx.global_mut::<Pending>();
    pending.sequence = pending.sequence.wrapping_add(1);
    pending.paths = (!files.is_empty()).then(|| {
        let local = api.is_local();
        let prepare: Prepare = Box::new(move || stage(&api, &files));
        (local, prepare)
    });
}

pub(super) fn moved(event: &DragMoveEvent<DraggedItem>, window: &mut Window, cx: &mut App) {
    let viewport = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
    if viewport.contains(&event.event.position) || !cx.has_global::<Pending>() {
        return;
    }
    let pending = cx.global_mut::<Pending>();
    let Some((local, prepare)) = pending.paths.take() else {
        return;
    };
    let sequence = pending.sequence;
    let handle = window.window_handle();
    cx.stop_active_drag(window);

    cx.spawn(async move |cx| {
        let result = cx
            .background_executor()
            .spawn(async move { prepare() })
            .await;
        let paths = match result {
            Ok(paths) => paths,
            Err(error) => {
                log::warn!("Could not prepare files for Windows drag: {error:#}");
                return;
            }
        };
        handle
            .update(cx, |_, window, cx| {
                let same_drag = cx.global::<Pending>().sequence == sequence;
                let held = unsafe { GetAsyncKeyState(i32::from(VK_LBUTTON.0)) } < 0;
                if same_drag && held {
                    if let Err(error) = drag(window, &paths, local) {
                        log::warn!("Could not drag files to another app: {error:#}");
                    }
                }
            })
            .ok();
    })
    .detach();
}

struct Pidl(*mut ITEMIDLIST);

impl Pidl {
    fn parse(path: &Path) -> Result<Self> {
        let wide = crate::windows::shell_path(path);
        let mut pidl = null_mut();
        unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None)? };
        Ok(Self(pidl))
    }
}

impl Drop for Pidl {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.cast())) };
    }
}

fn data_object(paths: &[PathBuf]) -> Result<IDataObject> {
    let pidls: Vec<_> = paths
        .iter()
        .map(|path| Pidl::parse(path))
        .collect::<Result<_>>()?;
    let pointers: Vec<_> = pidls.iter().map(|pidl| pidl.0.cast_const()).collect();
    unsafe {
        let items = SHCreateShellItemArrayFromIDLists(&pointers)?;
        Ok(items.BindToHandler(None, &BHID_DataObject)?)
    }
}

fn drag(window: &Window, paths: &[PathBuf], local: bool) -> Result<()> {
    let owner = crate::windows::window_id(window).map(|id| HWND(id as _));
    let data = data_object(paths)?;
    // GPUI initialized OLE on the window thread when it created the app.
    unsafe {
        let effects = if local {
            DROPEFFECT_COPY | DROPEFFECT_MOVE | DROPEFFECT_LINK
        } else {
            DROPEFFECT_COPY
        };
        SHDoDragDrop(owner, &data, None, effects)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorer_can_read_the_drag_as_files() {
        use windows::Win32::System::{
            Com::{DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL},
            Ole::CF_HDROP,
        };

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drag café.txt");
        std::fs::write(&path, "drag data").unwrap();
        crate::windows::with_shell(|| {
            let object = data_object(&[path])?;
            let format = FORMATETC {
                cfFormat: CF_HDROP.0,
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as u32,
                ..Default::default()
            };
            unsafe { object.QueryGetData(&format).ok()? };
            Ok(())
        })
        .unwrap();
    }
}
