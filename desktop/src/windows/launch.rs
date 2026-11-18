use std::path::Path;

use anyhow::{Context as _, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runnable {
    Program { script: bool },
    Document,
}

pub fn program(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "exe"
            | "com"
            | "bat"
            | "cmd"
            | "scr"
            | "pif"
            | "lnk"
            | "url"
            | "appref-ms"
            | "msi"
            | "msp"
            | "ps1"
            | "vbs"
            | "vbe"
            | "js"
            | "jse"
            | "ws"
            | "wsf"
            | "wsh"
            | "hta"
            | "reg"
            | "msc"
    )
}

pub fn classify(path: &Path) -> Runnable {
    if program(path) && path.is_file() {
        Runnable::Program { script: false }
    } else {
        Runnable::Document
    }
}

pub fn run(path: &Path) -> Result<()> {
    crate::windows::open(path)
}

/// Opens `%TERMINAL%`, or `cmd.exe`, with `folder` as its working directory.
pub fn terminal_in(folder: &Path) -> Result<()> {
    let terminal = std::env::var_os("TERMINAL").unwrap_or_else(|| "cmd.exe".into());
    std::process::Command::new(terminal)
        .current_dir(crate::windows::shell_path_buf(folder))
        .spawn()
        .context("could not open terminal")?;
    Ok(())
}
