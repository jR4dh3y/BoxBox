use std::mem::size_of;

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{RedrawWindow, RDW_INVALIDATE, RDW_NOERASE, RDW_UPDATENOW},
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
    if message == WM_EXITSIZEMOVE
        || (message == WM_SIZE && wparam.0 != SIZE_MINIMIZED as usize && in_size_move(hwnd))
    {
        // GPUI has updated its viewport and swap chain. Paint before DWM displays the new bounds.
        let _ = unsafe {
            RedrawWindow(
                Some(hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_UPDATENOW | RDW_NOERASE,
            )
        };
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
