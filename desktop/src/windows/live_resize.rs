//! Keeps the drawn content the same size as the window while the user drags its border.
//!
//! Windows applies new window bounds before it sends `WM_SIZE`, and GPUI presents
//! through a DirectComposition swap chain that DWM composes on its own clock. If DWM
//! composes while GPUI is still rendering the new size, the frame shows the larger
//! window with the old, shorter content and the backdrop shows through the gap.
//! Linux needs none of this: X11 uses `_NET_WM_SYNC_REQUEST` and Wayland sizes the
//! window from the committed buffer.

use std::mem::size_of;

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::{
        Dwm::DwmFlush,
        Gdi::{RedrawWindow, RDW_INVALIDATE, RDW_NOERASE, RDW_UPDATENOW},
    },
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            GetGUIThreadInfo, GUITHREADINFO, GUI_INMOVESIZE, SIZE_MINIMIZED, WM_EXITSIZEMOVE,
            WM_NCDESTROY, WM_SIZE,
        },
    },
};

pub(crate) fn install(window: &gpui::Window) {
    if let Some(owner) = super::window_id(window) {
        let result = unsafe { SetWindowSubclass(HWND(owner as _), Some(resize_proc), 0, 0).ok() };
        if let Err(error) = result {
            log::warn!("Could not install Windows live resize redraw: {error}");
        }
    }
}

unsafe extern "system" fn resize_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(resize_proc), id) };
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    let dragging =
        message == WM_SIZE && wparam.0 != SIZE_MINIMIZED as usize && in_size_move(hwnd);
    if dragging || message == WM_EXITSIZEMOVE {
        // GPUI has updated its viewport and swap chain. Paint the new size now.
        let _ = unsafe {
            RedrawWindow(
                Some(hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_UPDATENOW | RDW_NOERASE,
            )
        };
    }
    if dragging {
        // Hold the drag until DWM has shown this frame. The next bounds change then
        // starts just after a composition, which leaves a full refresh to render it.
        let _ = unsafe { DwmFlush() };
    }
    result
}

fn in_size_move(hwnd: HWND) -> bool {
    let mut info = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    (unsafe { GetGUIThreadInfo(0, &mut info).is_ok() })
        && info.flags.contains(GUI_INMOVESIZE)
        && info.hwndMoveSize == hwnd
}
