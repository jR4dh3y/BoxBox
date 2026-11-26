//! Open a draft in the configured Simple MAPI client, with files attached.

use std::{ffi::c_void, path::PathBuf};

use anyhow::{ensure, Result};

#[repr(C)]
#[derive(Default)]
struct Attachment {
    reserved: u32,
    flags: u32,
    position: u32,
    path: *mut u16,
    name: *mut u16,
    file_type: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
struct Message {
    reserved: u32,
    subject: *mut u16,
    note: *mut u16,
    message_type: *mut u16,
    date_received: *mut u16,
    conversation_id: *mut u16,
    flags: u32,
    originator: *mut c_void,
    recipient_count: u32,
    recipients: *mut c_void,
    file_count: u32,
    files: *mut Attachment,
}

type SendMail = unsafe extern "system" fn(usize, usize, *mut Message, u32, u32) -> u32;

pub(crate) fn compose(paths: &[PathBuf], owner: isize) -> Result<()> {
    const MAPI_LOGON_UI: u32 = 1;
    const MAPI_DIALOG: u32 = 8;
    const MAPI_USER_ABORT: u32 = 1;
    super::with_shell(|| {
        let library = super::SystemLibrary::open(windows_sys::core::w!("mapi32.dll"))?;
        // Signature and message layouts match MAPISENDMAILW from mapi.h.
        let send: SendMail = unsafe { std::mem::transmute(library.symbol(c"MAPISendMailW")?) };
        let mut names: Vec<_> = paths.iter().map(|path| super::shell_path(path)).collect();
        let mut attachments: Vec<_> = names
            .iter_mut()
            .map(|name| Attachment {
                position: u32::MAX,
                path: name.as_mut_ptr(),
                ..Default::default()
            })
            .collect();
        let mut message = Message {
            file_count: u32::try_from(attachments.len())?,
            files: attachments.as_mut_ptr(),
            ..Default::default()
        };
        // The modal compose window lets the user choose recipients and send or cancel.
        let result = unsafe {
            send(
                0,
                owner as usize,
                &mut message,
                MAPI_LOGON_UI | MAPI_DIALOG,
                0,
            )
        };
        ensure!(
            matches!(result, 0 | MAPI_USER_ABORT),
            "the Windows mail client returned MAPI error {result}"
        );
        Ok(())
    })
}
