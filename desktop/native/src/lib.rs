mod media;
mod render;

use render::{MAX_SOURCE_BYTES, RenderError};
use std::{
    fs::File,
    io::{BufReader, Cursor},
    panic::catch_unwind,
};

#[unsafe(no_mangle)]
pub extern "C" fn boxbox_thumbnail_abi() -> u32 {
    1
}

fn publish(result: Result<Vec<u8>, RenderError>, output: *mut u8, capacity: usize) -> i32 {
    match result {
        Ok(bytes) if bytes.len() <= capacity => {
            // The caller retains this fixed output buffer throughout the synchronous call.
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
            }
            i32::try_from(bytes.len()).unwrap_or(-3)
        }
        Ok(_) => -3,
        Err(RenderError::Invalid) => -1,
        Err(RenderError::Limit) => -2,
    }
}

/// # Safety
/// `output` must point to `capacity` writable bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn boxbox_thumbnail_fd(
    fd: u32,
    kind: u32,
    width: u32,
    height: u32,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if output.is_null()
        || capacity == 0
        || kind > 3
        || !(16..=512).contains(&width)
        || !(16..=512).contains(&height)
    {
        return -1;
    }
    let result = catch_unwind(|| {
        let file = File::open(format!("/proc/self/fd/{fd}")).map_err(|_| RenderError::Invalid)?;
        let metadata = file.metadata().map_err(|_| RenderError::Invalid)?;
        if !metadata.is_file() {
            return Err(RenderError::Invalid);
        }
        if kind != 0 {
            let bytes = media::frame(kind, fd, width, height)?;
            return render::render(Cursor::new(bytes), width, height);
        }
        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(RenderError::Limit);
        }
        render::render(BufReader::new(file), width, height)
    });
    result
        .map(|result| publish(result, output, capacity))
        .unwrap_or(-4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::fd::AsRawFd};

    #[test]
    fn ffi_rejects_bad_descriptors_and_small_buffers_without_overwriting() {
        let path = std::env::temp_dir().join(format!("boxbox-ffi-test-{}.png", std::process::id()));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(32, 16)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let mut file = File::create(&path).unwrap();
        file.write_all(encoded.get_ref()).unwrap();
        drop(file);
        let file = File::open(&path).unwrap();
        let fd = u32::try_from(file.as_raw_fd()).unwrap();
        let mut output = [0x5a_u8; 2];
        unsafe {
            assert_eq!(
                boxbox_thumbnail_fd(fd, 0, 32, 32, output.as_mut_ptr(), 1),
                -3
            );
            assert_eq!(output, [0x5a; 2]);
            assert_eq!(
                boxbox_thumbnail_fd(u32::MAX, 0, 32, 32, output.as_mut_ptr(), 2),
                -1
            );
            assert_eq!(
                boxbox_thumbnail_fd(fd, 4, 32, 32, output.as_mut_ptr(), 2),
                -1
            );
            assert_eq!(
                boxbox_thumbnail_fd(fd, 0, 32, 32, std::ptr::null_mut(), 0),
                -1
            );
        }
        file.set_len(MAX_SOURCE_BYTES + 1).unwrap_err();
        drop(file);
        let large = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        large.set_len(MAX_SOURCE_BYTES + 1).unwrap();
        let file = File::open(&path).unwrap();
        unsafe {
            assert_eq!(
                boxbox_thumbnail_fd(
                    u32::try_from(file.as_raw_fd()).unwrap(),
                    0,
                    32,
                    32,
                    output.as_mut_ptr(),
                    2
                ),
                -2
            );
        }
        drop(file);
        drop(large);
        std::fs::remove_file(path).unwrap();
    }
}
