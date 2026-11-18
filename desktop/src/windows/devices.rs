//! Windows drives are exposed as local roots.

use std::path::PathBuf;

use anyhow::{bail, ensure, Context as _, Result};
use async_channel::Sender;

use super::{Update, Volume};

#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub user: String,
    pub domain: String,
    pub password: String,
}

pub enum Prompt {
    Password {
        message: String,
        user: Option<String>,
        domain: Option<String>,
        reply: Sender<Answer>,
    },
    Question {
        message: String,
        choices: Vec<String>,
        reply: Sender<u32>,
    },
}

pub type Prompts = Sender<Prompt>;

pub fn watch(sender: Sender<Update>) {
    std::thread::spawn(move || {
        let mut previous = crate::windows::drive_mask();
        while !sender.is_closed() {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let mask = crate::windows::drive_mask();
            if mask != previous {
                previous = mask;
                if sender.send_blocking(Update::DrivesChanged).is_err() {
                    break;
                }
            }
        }
    });
}

pub(crate) enum DriveAction {
    Eject,
    Disconnect,
}

impl DriveAction {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Eject => "Eject",
            Self::Disconnect => "Disconnect",
        }
    }
}

pub(crate) fn drive_action(name: &str) -> Option<DriveAction> {
    use windows_sys::Win32::{Storage::FileSystem::GetDriveTypeW, System::WindowsProgramming::*};
    if !crate::windows::has_drive(name, crate::windows::drive_mask()) {
        return None;
    }
    let path = crate::windows::wide(format!("{name}:\\"));
    match unsafe { GetDriveTypeW(path.as_ptr()) } {
        DRIVE_REMOVABLE | DRIVE_CDROM => Some(DriveAction::Eject),
        DRIVE_REMOTE => Some(DriveAction::Disconnect),
        _ => None,
    }
}

pub(crate) fn release_drive(name: &str, owner: isize) -> Result<()> {
    match drive_action(name) {
        Some(DriveAction::Disconnect) => {
            use windows_sys::Win32::NetworkManagement::WNet::WNetCancelConnection2W;
            let drive = crate::windows::wide(format!("{name}:"));
            let result = unsafe { WNetCancelConnection2W(drive.as_ptr(), 0, 0) };
            ensure!(
                result == 0,
                "{}",
                std::io::Error::from_raw_os_error(result as i32)
            );
        }
        Some(DriveAction::Eject) => {
            let root = PathBuf::from(format!("{name}:\\"));
            crate::windows::shell_verb(&crate::windows::wide("eject"), &root, owner)?;
        }
        _ => bail!("the drive is unavailable or cannot be removed"),
    }
    Ok(())
}

pub fn mount_device(_: &Volume, _: Option<&str>, _: Prompts) -> Result<PathBuf> {
    bail!("Use Windows to mount this device")
}

pub fn remove(_: &Volume) -> Result<()> {
    bail!("Use Windows to eject this device")
}

pub fn smb_path(uri: &str) -> Result<PathBuf> {
    let url = url::Url::parse(uri)?;
    ensure!(
        url.scheme() == "smb",
        "Windows network paths use smb://server/share"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Enter only the SMB server and share"
    );
    let host = url.host_str().context("missing SMB server")?;
    ensure!(
        !url.path().to_ascii_lowercase().contains("%2f")
            && !url.path().to_ascii_lowercase().contains("%5c"),
        "an SMB path component contains a separator"
    );
    let path = url::Url::parse(&format!("file://{host}{}", url.path()))?
        .to_file_path()
        .map_err(|_| anyhow::anyhow!("invalid SMB path"))?;
    let root = crate::windows::mount_name(&path).context("missing SMB share")?;
    ensure!(
        root.starts_with(r"\\") && root.trim_start_matches('\\').contains('\\'),
        "missing SMB share"
    );
    Ok(path)
}

pub fn mount(uri: &str, _: Prompts) -> Result<PathBuf> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::NetworkManagement::WNet::*;

    let path = smb_path(uri)?;
    let root = crate::windows::mount_name(&path).context("missing SMB share")?;
    if !std::path::Path::new(&root).is_dir() {
        let mut target = crate::windows::wide(&root);
        let share = NETRESOURCEW {
            dwType: RESOURCETYPE_DISK,
            lpRemoteName: target.as_mut_ptr(),
            lpLocalName: null_mut(),
            ..Default::default()
        };
        let result = unsafe {
            WNetAddConnection3W(
                null_mut(),
                &share,
                null(),
                null(),
                CONNECT_TEMPORARY | CONNECT_INTERACTIVE | CONNECT_PROMPT,
            )
        };
        if result != 0 {
            return Err(std::io::Error::from_raw_os_error(result as i32))
                .context("could not connect to the network share");
        }
    }
    ensure!(path.is_dir(), "the network folder is unavailable");
    Ok(path)
}

pub mod spec {
    pub fn is_uri(text: &str) -> bool {
        text.split_once("://")
            .is_some_and(|(scheme, _)| !scheme.eq_ignore_ascii_case("file"))
    }

    pub fn smb_server_only(uri: &str) -> Option<&str> {
        let (scheme, path) = uri.split_once("://")?;
        let host = path.trim_end_matches('/');
        (scheme.eq_ignore_ascii_case("smb") && !host.is_empty() && !host.contains('/'))
            .then_some(host)
    }
}
