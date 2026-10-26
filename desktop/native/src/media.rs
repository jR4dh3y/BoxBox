use crate::render::RenderError;
use std::{
    io::{self, Read},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
};

const MAX_FRAME_BYTES: u64 = 2 << 20;
const DEMUXERS: &str =
    "mov,matroska,webm,avi,mpeg,mpegts,ogg,asf,flv,mp3,wav,flac,aac,ac3,aiff,ape,wv,amr";

struct MediaChild(Child);
impl Drop for MediaChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn output(program: &str, args: &[String]) -> Result<Vec<u8>, RenderError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Reset Bun's inherited stack size so media threads fit the fixed address-space limit.
    unsafe {
        command.pre_exec(|| {
            for (resource, maximum) in [
                (libc::RLIMIT_AS, 512 << 20),
                (libc::RLIMIT_CPU, 3),
                (libc::RLIMIT_STACK, 8 << 20),
                (libc::RLIMIT_CORE, 0),
            ] {
                let limit = libc::rlimit {
                    rlim_cur: maximum,
                    rlim_max: maximum,
                };
                if libc::setrlimit(resource, &limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = MediaChild(command.spawn().map_err(|_| RenderError::Invalid)?);
    let mut bytes = Vec::new();
    child
        .0
        .stdout
        .take()
        .ok_or(RenderError::Invalid)?
        .take(MAX_FRAME_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| RenderError::Invalid)?;
    if bytes.len() as u64 > MAX_FRAME_BYTES {
        return Err(RenderError::Limit);
    }
    if bytes.is_empty() || !child.0.wait().map_err(|_| RenderError::Invalid)?.success() {
        return Err(RenderError::Invalid);
    }
    Ok(bytes)
}

pub fn frame(kind: u32, fd: u32, width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    let source = format!("/proc/{}/fd/{fd}", std::process::id());
    if kind == 3 {
        return output(
            "pdftoppm",
            &[
                "-f".into(),
                "1".into(),
                "-l".into(),
                "1".into(),
                "-singlefile".into(),
                "-scale-to".into(),
                width.max(height).to_string(),
                "-png".into(),
                source,
            ],
        );
    }
    let input: Vec<String> = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-max_alloc",
        "134217728",
        "-filter_threads",
        "1",
        "-filter_complex_threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
        "-format_whitelist",
        DEMUXERS,
        "-probesize",
        "8388608",
        "-analyzeduration",
        "3000000",
        "-max_pixels",
        "32000000",
        "-threads",
        "1",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let encode: Vec<String> = [
        "-frames:v",
        "1",
        "-c:v",
        "png",
        "-threads",
        "1",
        "-f",
        "image2pipe",
        "pipe:1",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let mut args = input.clone();
    args.extend([
        "-i".into(),
        source.clone(),
        "-map".into(),
        "0:v:0".into(),
        "-an".into(),
        "-sn".into(),
        "-dn".into(),
        "-vf".into(),
        format!("scale={width}:{height}:force_original_aspect_ratio=decrease"),
    ]);
    args.extend(encode.clone());
    match output("ffmpeg", &args) {
        Ok(bytes) => return Ok(bytes),
        Err(RenderError::Invalid) if kind == 2 => {}
        Err(error) => return Err(error),
    }
    let mut args = input;
    args.extend(["-t".into(), "30".into(), "-i".into(), source, "-filter_complex".into(),
        format!("aformat=channel_layouts=mono,showwavespic=s={width}x{height}:colors=0x60a5fa:scale=sqrt")]);
    args.extend(encode);
    output("ffmpeg", &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_children_inherit_memory_and_cpu_ceilings() {
        let result = output(
            "/bin/sh",
            &["-c".into(), "ulimit -v; ulimit -t; ulimit -s".into()],
        )
        .unwrap();
        assert_eq!(String::from_utf8(result).unwrap(), "524288\n3\n8192\n");
    }
}
