//! Windows drives are exposed as local roots.

use std::path::PathBuf;

use anyhow::{bail, Result};
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

pub fn watch(_: Sender<Update>) {}

pub fn mount_device(_: &Volume, _: Option<&str>, _: Prompts) -> Result<PathBuf> {
    bail!("Use Windows to mount this device")
}

pub fn remove(_: &Volume) -> Result<()> {
    bail!("Use Windows to eject this device")
}

pub fn mount(_: &str, _: Prompts) -> Result<PathBuf> {
    bail!("Map the network share to a Windows drive first")
}

pub mod spec {
    pub fn is_uri(text: &str) -> bool {
        text.split_once("://")
            .is_some_and(|(scheme, _)| !scheme.eq_ignore_ascii_case("file"))
    }

    pub fn smb_server_only(_: &str) -> Option<&str> {
        None
    }
}
