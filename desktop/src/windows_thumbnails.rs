//! Local Windows thumbnails use the bounded raster decoder, the Shell's shared
//! cache and handlers, and the built-in PDF renderer.

use std::{
    io::{Read, Seek, SeekFrom, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{ensure, Context as _, Result};

#[path = "windows/audio_thumbnail.rs"]
mod audio_thumbnail;
use windows::{
    core::HSTRING,
    Data::Pdf::{PdfDocument, PdfPageRenderOptions},
    Storage::{
        FileProperties::{ThumbnailMode, ThumbnailOptions, ThumbnailType},
        StorageFile,
        Streams::{DataReader, IInputStream, InMemoryRandomAccessStream},
    },
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};

pub fn thumbnail(path: &Path, width: u32, height: u32) -> Result<Vec<u8>> {
    match boxbox_thumbnail::thumbnail_path(path, width, height) {
        Ok(bytes) => return Ok(bytes),
        Err(boxbox_thumbnail::ThumbnailError::UnsupportedFormat) => {}
        Err(error) => return Err(error.into()),
    }
    let mut output = tempfile::tempfile()?;
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--thumbnail-worker")
        .arg(width.to_string())
        .arg(height.to_string())
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::null())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("the Windows thumbnail provider timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    ensure!(
        status.success(),
        "the Windows thumbnail provider returned {status}"
    );
    output.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    output.take(MAX_STREAM + 1).read_to_end(&mut bytes)?;
    ensure!(
        (1..=MAX_STREAM as usize).contains(&bytes.len()),
        "invalid Windows thumbnail"
    );
    Ok(bytes)
}

const MAX_STREAM: u64 = 8 * 1024 * 1024;

/// The helper is the app executable so the portable ZIP needs only one binary.
pub(crate) fn dispatch_worker() {
    let mut args = std::env::args_os().skip(1);
    if !args.next().is_some_and(|arg| arg == "--thumbnail-worker") {
        return;
    }
    let mut run = || -> Result<()> {
        let size =
            |arg: Option<std::ffi::OsString>| arg.and_then(|arg| arg.to_str()?.parse::<u32>().ok());
        let width = size(args.next()).context("invalid thumbnail width")?;
        let height = size(args.next()).context("invalid thumbnail height")?;
        let path = PathBuf::from(args.next().context("missing thumbnail path")?);
        ensure!(args.next().is_none(), "unexpected thumbnail argument");
        let bytes = native_thumbnail(&path, width, height)?;
        let mut output = std::io::stdout().lock();
        output.write_all(&bytes)?;
        output.flush()?;
        Ok(())
    };
    let result = run();
    if let Err(error) = &result {
        eprintln!("{error:#}");
    }
    let code = u32::from(result.is_err());
    // Some graphics drivers crash during PDF renderer DLL shutdown. The helper has
    // already closed the page and flushed its output, so end it without DLL detach.
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        TerminateProcess(GetCurrentProcess(), code);
    }
    std::process::exit(code as i32);
}

fn native_thumbnail(path: &Path, width: u32, height: u32) -> Result<Vec<u8>> {
    ensure!(
        (16..=512).contains(&width) && (16..=512).contains(&height),
        "invalid thumbnail size"
    );

    // The thumbnail worker owns this MTA for the duration of the request.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = Apartment;

    let path_wide = crate::windows::shell_path(path);
    let name = HSTRING::from_wide(&path_wide[..path_wide.len() - 1]);
    let file = StorageFile::GetFileFromPathAsync(&name)?.get()?;
    let size = width.max(height);
    let bytes = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
    {
        pdf_thumbnail(&file, size).context("cannot render the first PDF page")?
    } else {
        match shell_thumbnail(&file, size) {
            Ok(bytes) => bytes,
            Err(_)
                if path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus"
                        )
                    }) =>
            {
                audio_thumbnail::waveform(path, size)
                    .context("Windows could not decode the audio")?
            }
            Err(error) => return Err(error).context("Windows could not make a thumbnail"),
        }
    };
    Ok(boxbox_thumbnail::thumbnail_bytes(&bytes, width, height)?)
}

fn shell_thumbnail(file: &StorageFile, size: u32) -> Result<Vec<u8>> {
    let thumbnail = file
        .GetThumbnailAsync(ThumbnailMode::SingleItem, size, ThumbnailOptions::None)?
        .get()?;
    ensure!(
        thumbnail.Type()? == ThumbnailType::Image,
        "the file has no thumbnail"
    );
    read_stream(thumbnail.GetInputStreamAt(0)?, thumbnail.Size()?)
}

fn pdf_thumbnail(file: &StorageFile, size: u32) -> Result<Vec<u8>> {
    let document = PdfDocument::LoadFromFileAsync(file)?.get()?;
    let page = document.GetPage(0)?;
    let bounds = page.Size()?;
    let options = PdfPageRenderOptions::new()?;
    if bounds.Width >= bounds.Height {
        options.SetDestinationWidth(size)?;
    } else {
        options.SetDestinationHeight(size)?;
    }
    options.SetBitmapEncoderId(windows::Graphics::Imaging::BitmapEncoder::PngEncoderId()?)?;
    let stream = InMemoryRandomAccessStream::new()?;
    page.RenderWithOptionsToStreamAsync(&stream, &options)?
        .get()?;
    page.Close()?;
    read_stream(stream.GetInputStreamAt(0)?, stream.Size()?)
}

fn read_stream(stream: IInputStream, size: u64) -> Result<Vec<u8>> {
    ensure!(
        (1..=MAX_STREAM).contains(&size),
        "invalid thumbnail stream size"
    );
    let size = u32::try_from(size)?;
    let reader = DataReader::CreateDataReader(&stream)?;
    ensure!(
        reader.LoadAsync(size)?.get()? == size,
        "incomplete thumbnail"
    );
    let mut bytes = vec![0; size as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}
