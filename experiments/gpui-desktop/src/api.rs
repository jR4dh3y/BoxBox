//! Typed client for the BoxBox HTTP API (`/api/v1`), mirroring `frontend/src/lib/api/files.ts`.

use anyhow::{Context as _, Result};
use serde::{de::DeserializeOwned, Deserialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub mod_time: String,
}

#[derive(Debug, Deserialize)]
pub struct FileList {
    pub items: Vec<FileInfo>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RootKind {
    #[default]
    Drive,
    Place,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MountPoint {
    pub name: String,
    pub label: Option<String>,
    #[serde(default, deserialize_with = "root_kind")]
    pub kind: RootKind,
}

impl MountPoint {
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

/// The server sends `""` for drives, so anything but "place" is a drive.
fn root_kind<'de, D: serde::Deserializer<'de>>(de: D) -> Result<RootKind, D::Error> {
    let raw = Option::<String>::deserialize(de)?;
    Ok(match raw.as_deref() {
        Some("place") => RootKind::Place,
        _ => RootKind::Drive,
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveStats {
    pub name: String,
    pub label: Option<String>,
    pub device: Option<String>,
    pub fs_type: Option<String>,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub used_bytes: u64,
    pub used_pct: f64,
    #[serde(default)]
    pub read_only: bool,
}

impl DriveStats {
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Deserialize)]
struct RootsResponse {
    roots: Vec<MountPoint>,
}

#[derive(Deserialize)]
struct DriveStatsResponse {
    drives: Vec<DriveStats>,
}

pub struct Api {
    base: String,
    agent: ureq::Agent,
    token: Option<String>,
}

impl Api {
    /// `base` is the server origin, e.g. `http://127.0.0.1:8080`.
    pub fn new(base: &str, token: Option<String>) -> Self {
        Self {
            base: format!("{}/api/v1", base.trim_end_matches('/')),
            agent: ureq::AgentBuilder::new().build(),
            token,
        }
    }

    pub fn roots(&self) -> Result<Vec<MountPoint>> {
        Ok(self.get::<RootsResponse>("/files/")?.roots)
    }

    pub fn drives(&self) -> Result<Vec<DriveStats>> {
        Ok(self.get::<DriveStatsResponse>("/files/stats")?.drives)
    }

    pub fn list(&self, path: &str) -> Result<Vec<FileInfo>> {
        Ok(self.get::<FileList>(&format!("/files/list/{}", encode_path(path)))?.items)
    }

    fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = format!("{}{endpoint}", self.base);
        let mut request = self.agent.get(&url);
        if let Some(token) = &self.token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        match request.call() {
            Ok(response) => response.into_json().with_context(|| format!("invalid JSON from {url}")),
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                anyhow::bail!("{}", server_message(&body).unwrap_or(format!("HTTP {code}")))
            }
            Err(error) => Err(error).with_context(|| format!("cannot reach {url}")),
        }
    }
}

fn server_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct ErrorBody {
        error: String,
    }
    serde_json::from_str::<ErrorBody>(body).ok().map(|body| body.error)
}

/// `encodeRoutePath`: each segment percent-encoded like `encodeURIComponent`.
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            segment
                .bytes()
                .map(|byte| match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~'
                    | b'*' | b'\'' | b'(' | b')' => (byte as char).to_string(),
                    _ => format!("%{byte:02X}"),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}
