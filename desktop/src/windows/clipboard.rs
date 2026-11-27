//! File copy and cut interoperate with Explorer through CF_HDROP.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use anyhow::{ensure, Context as _, Result};
use windows_sys::core::w;
use windows_sys::Win32::{
    Foundation::{GlobalFree, HGLOBAL, HWND},
    System::{
        DataExchange::*,
        Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
        Ole::{CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_MOVE},
    },
    UI::{
        Shell::{DragQueryFileW, DROPFILES},
        WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOp {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    pub op: FileOp,
    pub paths: Vec<PathBuf>,
}

pub fn file_uri(path: &std::path::Path) -> String {
    url::Url::from_file_path(crate::windows::shell_path_buf(path))
        .map(Into::into)
        .unwrap_or_default()
}

pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    url::Url::parse(uri).ok()?.to_file_path().ok()
}

struct Clipboard(HWND);

impl Clipboard {
    fn open(writable: bool) -> Result<Self> {
        // EmptyClipboard needs an owner window before SetClipboardData can succeed.
        let owner = if writable {
            let window = unsafe {
                CreateWindowExW(
                    0,
                    w!("STATIC"),
                    null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    null_mut(),
                    null(),
                )
            };
            ensure!(
                !window.is_null(),
                "could not create clipboard owner: {}",
                io::Error::last_os_error()
            );
            window
        } else {
            null_mut()
        };
        for _ in 0..10 {
            if unsafe { OpenClipboard(owner) } != 0 {
                return Ok(Self(owner));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let error = io::Error::last_os_error();
        if !owner.is_null() {
            unsafe { DestroyWindow(owner) };
        }
        Err(error).context("could not open the Windows clipboard")
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
            if !self.0.is_null() {
                DestroyWindow(self.0);
            }
        }
    }
}

struct Memory(HGLOBAL);

impl Memory {
    fn new(bytes: usize) -> Result<Self> {
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) };
        ensure!(!handle.is_null(), "could not allocate clipboard data");
        Ok(Self(handle))
    }

    fn write<T>(&self, write: impl FnOnce(*mut u8) -> T) -> Result<T> {
        let memory = unsafe { GlobalLock(self.0) };
        ensure!(!memory.is_null(), "could not lock clipboard data");
        let result = write(memory.cast());
        unsafe { GlobalUnlock(self.0) };
        Ok(result)
    }

    fn publish(mut self, format: u32) -> Result<()> {
        let result = unsafe { SetClipboardData(format, self.0) };
        ensure!(
            !result.is_null(),
            "could not publish clipboard format: {}",
            io::Error::last_os_error()
        );
        self.0 = null_mut();
        Ok(())
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { GlobalFree(self.0) };
        }
    }
}

fn effect_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) }
}

pub fn publish(files: &Files) -> Result<()> {
    ensure!(!files.paths.is_empty(), "no files to copy");
    if cfg!(test) {
        return Ok(());
    }
    let names: Vec<_> = files
        .paths
        .iter()
        .flat_map(|path| crate::windows::shell_path(path))
        .chain([0])
        .collect();
    let header_size = std::mem::size_of::<DROPFILES>();
    let data = Memory::new(header_size + std::mem::size_of_val(names.as_slice()))?;
    data.write(|memory| unsafe {
        memory.cast::<DROPFILES>().write(DROPFILES {
            pFiles: header_size as u32,
            fWide: 1,
            ..Default::default()
        });
        std::ptr::copy_nonoverlapping(names.as_ptr(), memory.add(header_size).cast(), names.len());
    })?;
    let effect = match files.op {
        FileOp::Copy => DROPEFFECT_COPY,
        FileOp::Cut => DROPEFFECT_MOVE,
    };
    let operation = Memory::new(std::mem::size_of_val(&effect))?;
    operation.write(|memory| unsafe { memory.cast::<u32>().write(effect) })?;
    let format = effect_format();
    ensure!(
        format != 0,
        "could not register the Windows clipboard file operation"
    );
    let _clipboard = Clipboard::open(true)?;
    crate::windows::ensure_win32(unsafe { EmptyClipboard() })?;
    operation.publish(format)?;
    data.publish(u32::from(CF_HDROP))
}

pub fn clear() -> Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    let _clipboard = Clipboard::open(true)?;
    crate::windows::ensure_win32(unsafe { EmptyClipboard() })?;
    Ok(())
}

pub fn read() -> Result<Option<Files>> {
    if cfg!(test) {
        return Ok(None);
    }
    let _clipboard = Clipboard::open(false)?;
    let drop = unsafe { GetClipboardData(u32::from(CF_HDROP)) };
    if drop.is_null() {
        return Ok(None);
    }
    let count = unsafe { DragQueryFileW(drop, u32::MAX, null_mut(), 0) };
    ensure!(count <= 100_000, "clipboard file list is too large");
    let paths: Vec<_> = (0..count)
        .filter_map(|index| {
            let len = unsafe { DragQueryFileW(drop, index, null_mut(), 0) };
            if len == 0 || len >= 32_768 {
                return None;
            }
            let mut buffer = vec![0; len as usize + 1];
            let read = unsafe { DragQueryFileW(drop, index, buffer.as_mut_ptr(), len + 1) };
            (read == len).then(|| PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
        })
        .collect();

    let format = effect_format();
    let data = if format == 0 {
        null_mut()
    } else {
        unsafe { GetClipboardData(format) }
    };
    let mut op = FileOp::Copy;
    if !data.is_null() && unsafe { GlobalSize(data) } >= std::mem::size_of::<u32>() {
        let memory = unsafe { GlobalLock(data) };
        if !memory.is_null() {
            if unsafe { memory.cast::<u32>().read() } == DROPEFFECT_MOVE {
                op = FileOp::Cut;
            }
            unsafe { GlobalUnlock(data) };
        }
    }
    Ok((!paths.is_empty()).then_some(Files { op, paths }))
}
