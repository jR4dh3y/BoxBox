//! Paired server credentials in the current user's Windows Credential Manager.

use std::io;
use std::ptr::null_mut;

use anyhow::{Context as _, Result};
use windows_sys::Win32::{Foundation::ERROR_NOT_FOUND, Security::Credentials::*};

fn target(server: &str) -> Vec<u16> {
    crate::windows::wide(format!("BoxBox/Server/{server}"))
}

pub fn store(server: &str, label: &str, credential: &str) -> Result<()> {
    let mut target = target(server);
    let mut label = crate::windows::wide(label);
    let mut bytes = credential.as_bytes().to_vec();
    let record = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        Comment: label.as_mut_ptr(),
        CredentialBlobSize: u32::try_from(bytes.len())?,
        CredentialBlob: bytes.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    crate::windows::ensure_win32(unsafe { CredWriteW(&record, 0) })
        .context("Windows Credential Manager could not save the credential")
}

pub fn lookup(server: &str) -> Result<Option<String>> {
    let target = target(server);
    let mut record = null_mut();
    if let Err(error) = crate::windows::ensure_win32(unsafe {
        CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut record)
    }) {
        return if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
            Ok(None)
        } else {
            Err(error.into())
        };
    }
    let value = unsafe {
        let size = (*record).CredentialBlobSize as usize;
        let bytes = if size == 0 {
            &[]
        } else {
            std::slice::from_raw_parts((*record).CredentialBlob, size)
        };
        let value = String::from_utf8(bytes.to_vec());
        CredFree(record.cast());
        value
    };
    Ok(Some(value.context("the saved credential is not UTF-8")?))
}

pub fn delete(server: &str) -> Result<()> {
    let target = target(server);
    match crate::windows::ensure_win32(unsafe {
        CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0)
    }) {
        Err(error) if error.raw_os_error() != Some(ERROR_NOT_FOUND as i32) => {
            Err(io::Error::from(error).into())
        }
        _ => Ok(()),
    }
}
