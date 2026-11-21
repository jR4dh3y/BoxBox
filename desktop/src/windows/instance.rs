//! A per-profile, local-only pipe forwards folder launches to the running window.

use std::{
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Read, Write},
    os::windows::io::{AsRawHandle, FromRawHandle},
    path::{Path, PathBuf},
    ptr::null,
};

use anyhow::{ensure, Context as _, Result};
use windows_sys::Win32::{
    Foundation::{ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW,
        PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    },
};

const MAX_MESSAGE: u64 = 8 * 1024;

pub enum Instance {
    Primary(
        async_channel::Receiver<String>,
        async_channel::Sender<String>,
    ),
    Forwarded,
}

pub fn claim(argument: &str) -> Result<Instance> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        std::env::var_os("BOXBOX_URL"),
        std::env::var_os("BOXBOX_ROOT"),
    )
        .hash(&mut hasher);
    let path = crate::persist::state_dir().join(format!("instance-{:x}", hasher.finish()));
    claim_at(&path, argument)
}

fn claim_at(lock_path: &Path, argument: &str) -> Result<Instance> {
    fs::create_dir_all(lock_path.parent().context("instance directory")?)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    lock.lock()?;
    let address = lock_path.with_extension("pipe");
    if let Ok(file) = File::open(&address) {
        let mut token = String::new();
        if file.take(33).read_to_string(&mut token).is_ok()
            && token.len() == 32
            && token.bytes().all(|ch| ch.is_ascii_hexdigit())
            && forward(&pipe_name(&token), argument)?
        {
            return Ok(Instance::Forwarded);
        }
    }

    // A process outside this user's state directory cannot guess this pipe.
    let mut random = [0u8; 16];
    getrandom::fill(&mut random)
        .map_err(|error| anyhow::anyhow!("could not choose the instance pipe: {error}"))?;
    let token = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let path = pipe_name(&token);
    let wide = crate::windows::wide(&path);
    let pipe = unsafe {
        CreateNamedPipeW(
            wide.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            MAX_MESSAGE as u32,
            MAX_MESSAGE as u32,
            500,
            null(),
        )
    };
    if pipe == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error()).context("cannot create the instance pipe");
    }
    let pipe = unsafe { File::from_raw_handle(pipe) };
    crate::persist::atomic_write(&address, token.as_bytes())?;
    let (sender, receiver) = async_channel::unbounded();
    let launches = sender.clone();
    std::thread::Builder::new()
        .name("instance".into())
        .spawn(move || serve(pipe, sender))?;
    Ok(Instance::Primary(receiver, launches))
}

fn pipe_name(token: &str) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\BoxBox-{token}"))
}

fn forward(path: &Path, argument: &str) -> Result<bool> {
    let open = || OpenOptions::new().read(true).write(true).open(path);
    let mut pipe = match open() {
        Ok(pipe) => pipe,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
            let name = crate::windows::wide(path);
            crate::windows::ensure_win32(unsafe { WaitNamedPipeW(name.as_ptr(), 500) })?;
            open()?
        }
        Err(error) => return Err(error.into()),
    };
    let payload = serde_json::to_string(argument)?;
    ensure!(
        payload.len() < MAX_MESSAGE as usize,
        "launch path is too long"
    );
    writeln!(pipe, "{payload}")?;
    let mut reply = String::new();
    BufReader::new(pipe.take(16)).read_line(&mut reply)?;
    Ok(reply.trim() == "ok")
}

fn serve(mut pipe: File, launches: async_channel::Sender<String>) {
    loop {
        let handle = pipe.as_raw_handle();
        if unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } == 0
            && std::io::Error::last_os_error().raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32)
        {
            break;
        }
        let mut request = String::new();
        let read = BufReader::new((&pipe).take(MAX_MESSAGE)).read_line(&mut request);
        if read.is_ok() {
            if let Ok(path) = serde_json::from_str::<String>(request.trim()) {
                if launches.send_blocking(path).is_err() {
                    break;
                }
                if writeln!(pipe, "ok").is_ok() {
                    // A disconnect discards unread bytes. Wait for the client to read its reply.
                    unsafe { windows_sys::Win32::Storage::FileSystem::FlushFileBuffers(handle) };
                }
            }
        }
        unsafe { DisconnectNamedPipe(handle) };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn new_launch_reaches_the_primary_window() {
        use super::*;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("instance");
        let Instance::Primary(receive, _) = claim_at(&path, "").unwrap() else {
            panic!("first launch was not primary")
        };
        assert!(matches!(
            claim_at(&path, "C:\\BoxBox Test").unwrap(),
            Instance::Forwarded
        ));
        assert_eq!(receive.try_recv().unwrap(), "C:\\BoxBox Test");
    }
}
