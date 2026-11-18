//! File type icons from the same Shell image list Explorer uses.

use std::{mem::size_of, ptr::null_mut};

use anyhow::{ensure, Context as _, Result};
use windows::{
    core::PCWSTR,
    Win32::{
        Graphics::Gdi::*,
        Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL},
        UI::{
            Controls::{IImageList, ILD_TRANSPARENT},
            Shell::{
                SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_SYSICONINDEX,
                SHGFI_USEFILEATTRIBUTES, SHIL_JUMBO,
            },
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
        },
    },
};

use crate::file_icons::IconKey;

pub(crate) fn render(key: &IconKey, size: u32) -> Result<image::RgbaImage> {
    ensure!((16..=256).contains(&size), "invalid icon size");
    super::with_shell(|| {
        let (name, attributes) = match key {
            IconKey::File(extension) => (
                super::wide(format!("boxbox.{extension}")),
                FILE_ATTRIBUTE_NORMAL,
            ),
            IconKey::Folder(_) => (super::wide("BoxBox Folder"), FILE_ATTRIBUTE_DIRECTORY),
        };
        let mut info = SHFILEINFOW::default();
        let list = unsafe {
            SHGetFileInfoW(
                PCWSTR(name.as_ptr()),
                attributes,
                Some(&mut info),
                size_of::<SHFILEINFOW>() as u32,
                SHGFI_SYSICONINDEX | SHGFI_USEFILEATTRIBUTES,
            )
        };
        ensure!(list != 0, "Windows did not return the file type icon");
        let list: IImageList = unsafe { SHGetImageList(SHIL_JUMBO as i32)? };
        let icon = Icon(unsafe { list.GetIcon(info.iIcon, ILD_TRANSPARENT.0)? });
        pixels(&icon, size)
    })
}

struct Icon(HICON);
impl Drop for Icon {
    fn drop(&mut self) {
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        let _ = unsafe { DeleteObject(self.0.into()) };
    }
}

struct Dc(HDC);
impl Drop for Dc {
    fn drop(&mut self) {
        let _ = unsafe { DeleteDC(self.0) };
    }
}

fn pixels(icon: &Icon, size: u32) -> Result<image::RgbaImage> {
    let px = i32::try_from(size)?;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: px,
            biHeight: -px,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let dc = Dc(unsafe { CreateCompatibleDC(None) });
    ensure!(
        !dc.0 .0.is_null(),
        "cannot create a Shell icon drawing context"
    );
    let mut data = null_mut();
    let bitmap =
        Bitmap(unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut data, None, 0)? });
    ensure!(!data.is_null(), "the bitmap has no pixels");
    let bytes = 4 * size as usize * size as usize;
    unsafe { std::ptr::write_bytes(data.cast::<u8>(), 0, bytes) };
    let previous = unsafe { SelectObject(dc.0, bitmap.0.into()) };
    ensure!(!previous.0.is_null(), "cannot select icon bitmap");
    let drawn = unsafe { DrawIconEx(dc.0, 0, 0, icon.0, px, px, 0, None, DI_NORMAL) };
    unsafe { SelectObject(dc.0, previous) };
    drawn?;
    let mut rgba = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), bytes) }.to_vec();
    // The DIB uses premultiplied BGRA, GPUI expects straight alpha.
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        let alpha = u16::from(pixel[3]);
        if (1..255).contains(&alpha) {
            for channel in &mut pixel[..3] {
                *channel = (u16::from(*channel) * 255 / alpha).min(255) as u8;
            }
        }
    }
    ensure!(
        rgba.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "the Shell icon is empty"
    );
    image::RgbaImage::from_raw(size, size, rgba).context("cannot read the Shell icon's pixels")
}
