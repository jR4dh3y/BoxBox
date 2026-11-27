//! Waveforms for Windows audio files whose Shell handler has no cover art.

use std::{io::Cursor, path::Path, ptr::null_mut};

use anyhow::{ensure, Result};
use windows::{
    core::{Interface, HRESULT, PCWSTR},
    Win32::Media::MediaFoundation::*,
};

const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;

struct MediaFoundation(unsafe extern "system" fn() -> HRESULT);
impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe { self.0() };
    }
}

pub(super) fn waveform(path: &Path, size: u32) -> Result<Vec<u8>> {
    use crate::windows::SystemLibrary;
    use std::ffi::c_void;
    use windows_sys::core::w;

    // Optional on Windows N and Windows Server. Load it only for an audio request.
    let platform = SystemLibrary::open(w!("mfplat.dll"))?;
    let media_reader = SystemLibrary::open(w!("mfreadwrite.dll"))?;
    let startup: unsafe extern "system" fn(u32, u32) -> HRESULT =
        unsafe { std::mem::transmute(platform.symbol(c"MFStartup")?) };
    let shutdown = unsafe { std::mem::transmute(platform.symbol(c"MFShutdown")?) };
    let create_type: unsafe extern "system" fn(*mut *mut c_void) -> HRESULT =
        unsafe { std::mem::transmute(platform.symbol(c"MFCreateMediaType")?) };
    let reader_from_url: unsafe extern "system" fn(
        PCWSTR,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT =
        unsafe { std::mem::transmute(media_reader.symbol(c"MFCreateSourceReaderFromURL")?) };
    unsafe { startup(MF_VERSION, MFSTARTUP_LITE).ok()? };
    let _media = MediaFoundation(shutdown);
    let name = crate::windows::shell_path(path);
    let mut reader = null_mut();
    let mut pcm = null_mut();
    unsafe { reader_from_url(PCWSTR(name.as_ptr()), null_mut(), &mut reader).ok()? };
    let reader = unsafe { IMFSourceReader::from_raw(reader) };
    unsafe { create_type(&mut pcm).ok()? };
    let pcm = unsafe { IMFMediaType::from_raw(pcm) };
    unsafe {
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(AUDIO, true)?;
        pcm.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        pcm.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
        pcm.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        reader.SetCurrentMediaType(AUDIO, None, &pcm)?;
    }
    let format = unsafe { reader.GetCurrentMediaType(AUDIO)? };
    let rate = unsafe { format.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)? };
    let channels = unsafe { format.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)? };
    ensure!(
        (1..=384_000).contains(&rate)
            && (1..=32).contains(&channels)
            && unsafe { format.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE)? } == 16,
        "unexpected audio format"
    );
    let step = ((rate / 100).max(1) * channels) as usize;
    let limit = (rate * channels * 30) as usize;
    let mut counts = 0usize;
    let mut peaks = Vec::new();

    while counts < limit {
        let mut sample = None;
        let mut flags = 0;
        unsafe { reader.ReadSample(AUDIO, 0, None, Some(&mut flags), None, Some(&mut sample))? };
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        ensure!(
            flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 == 0,
            "audio format changed"
        );
        let Some(sample) = sample else { continue };
        let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
        let mut bytes = null_mut();
        let mut length = 0;
        unsafe { buffer.Lock(&mut bytes, None, Some(&mut length))? };
        if !bytes.is_null() && length > 0 {
            let count = (length as usize / 2).min(limit - counts);
            let data = unsafe { std::slice::from_raw_parts(bytes, count * 2) };
            for value in data.chunks_exact(2) {
                let index = counts / step;
                if index == peaks.len() {
                    peaks.push(0u16);
                }
                let magnitude = i16::from_le_bytes([value[0], value[1]]).unsigned_abs();
                peaks[index] = peaks[index].max(magnitude);
                counts += 1;
            }
        }
        unsafe { buffer.Unlock()? };
    }

    ensure!(!peaks.is_empty(), "there were no audio samples");
    let mut picture = image::RgbImage::from_pixel(size, size, image::Rgb([24, 32, 52]));
    let middle = size / 2;
    for x in 0..size {
        let start = x as usize * peaks.len() / size as usize;
        let end = ((x + 1) as usize * peaks.len() / size as usize).max(start + 1);
        let peak = peaks[start..end].iter().copied().max().unwrap_or(0);
        let extent = (u32::from(peak) * (middle - 1) / 32_768).max(1);
        for y in middle - extent..=middle + extent {
            picture.put_pixel(x, y, image::Rgb([96, 165, 250]));
        }
    }
    let mut png = Cursor::new(Vec::new());
    picture.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}
