//! Typed client for the BoxBox HTTP API (`/api/v1`), mirroring `frontend/src/lib/api/files.ts`.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use serde::{de::DeserializeOwned, Deserialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    /// Not sent by the list endpoint; `list` joins it from the directory path.
    #[serde(default)]
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub mod_time: String,
    #[serde(default)]
    pub permissions: String,
}

#[derive(Debug, Deserialize)]
struct FileList {
    path: String,
    items: Vec<FileInfo>,
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
    #[serde(default, rename = "readOnly")]
    pub read_only: bool,
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

/// How requests prove who they are.
#[derive(Clone)]
pub enum Auth {
    None,
    /// A server reached through `BOXBOX_URL` with an access token.
    Bearer(String),
    /// The private desktop server: its per-launch token, sent as the `boxbox_desktop` cookie.
    Desktop(String),
}

pub struct Api {
    base: String,
    agent: ureq::Agent,
    auth: Auth,
}

impl Api {
    /// `origin` is the server origin, e.g. `http://127.0.0.1:8080`.
    pub fn new(origin: &str, auth: Auth) -> Self {
        let origin = origin.trim_end_matches('/').to_string();
        Self {
            base: format!("{origin}/api/v1"),
            agent: ureq::AgentBuilder::new().build(),
            auth,
        }
    }

    pub(crate) fn authorize(&self, request: ureq::Request) -> ureq::Request {
        match &self.auth {
            Auth::None => request,
            Auth::Bearer(token) => request.set("Authorization", &format!("Bearer {token}")),
            Auth::Desktop(token) => request.set("Cookie", &format!("boxbox_desktop={token}")),
        }
    }

    pub fn roots(&self) -> Result<Vec<MountPoint>> {
        Ok(self.get::<RootsResponse>("/files/")?.roots)
    }

    pub fn drives(&self) -> Result<Vec<DriveStats>> {
        Ok(self.get::<DriveStatsResponse>("/files/stats")?.drives)
    }

    pub fn list(&self, path: &str) -> Result<Vec<FileInfo>> {
        parse_directory(self.get::<FileList>(&format!("/files/list/{}", encode_path(path)))?)
    }

    /// Saves a file, or a folder as the server's ZIP archive (like the web download), into `dir`.
    pub fn download(&self, item: &FileInfo, dir: &Path) -> Result<PathBuf> {
        let (endpoint, name) = if item.is_dir {
            ("/stream/archive", format!("{}.zip", item.name))
        } else {
            ("/stream/download", item.name.clone())
        };
        let response = self.call(&format!("{endpoint}/{}", encode_path(&item.path)))?;
        // `parse_directory` already rejected names that could leave `dir`.
        let target = dir.join(name);
        let mut file = std::fs::File::create(&target)
            .with_context(|| format!("cannot create {}", target.display()))?;
        std::io::copy(&mut response.into_reader(), &mut file)
            .with_context(|| format!("cannot download {}", item.path))?;
        Ok(target)
    }

    /// `POST /files/<dir>`: never overwrites; 409 when the name exists.
    pub fn create(&self, dir: &str, name: &str, directory: bool) -> Result<()> {
        let kind = if directory { "directory" } else { "file" };
        let body = serde_json::json!({ "name": name, "type": kind });
        self.send("POST", &format!("/files/{}", encode_path(dir)), Some(body))?;
        Ok(())
    }

    /// `PUT /files/<path>`: rename or move within one source.
    pub fn rename(&self, path: &str, new_path: &str) -> Result<()> {
        let body = serde_json::json!({ "newPath": new_path });
        self.send("PUT", &format!("/files/{}", encode_path(path)), Some(body))?;
        Ok(())
    }

    /// Permanent delete; folders need `confirm=true`.
    pub fn delete(&self, path: &str) -> Result<()> {
        self.send(
            "DELETE",
            &format!("/files/{}?confirm=true", encode_path(path)),
            None,
        )?;
        Ok(())
    }

    pub fn trash(&self, path: &str) -> Result<TrashItem> {
        #[derive(Deserialize)]
        struct Trashed {
            item: TrashItem,
        }
        let body = serde_json::json!({ "path": path });
        Ok(self
            .send("POST", "/desktop/trash", Some(body))?
            .into_json::<Trashed>()?
            .item)
    }

    pub fn trash_items(&self) -> Result<Vec<TrashItem>> {
        #[derive(Deserialize)]
        struct Listing {
            items: Vec<TrashItem>,
        }
        Ok(self.get::<Listing>("/desktop/trash")?.items)
    }

    pub fn restore(&self, id: &str) -> Result<()> {
        self.send(
            "POST",
            "/desktop/trash/restore",
            Some(serde_json::json!({ "id": id })),
        )?;
        Ok(())
    }

    pub fn delete_from_trash(&self, id: &str) -> Result<()> {
        self.send(
            "POST",
            "/desktop/trash/delete",
            Some(serde_json::json!({ "id": id })),
        )?;
        Ok(())
    }

    pub fn clipboard(&self) -> Result<crate::clipboard::ClipContents> {
        self.get("/desktop/clipboard")
    }

    pub fn set_clipboard(&self, operation: &str, paths: &[String]) -> Result<()> {
        let body = serde_json::json!({ "operation": operation, "paths": paths });
        self.send("POST", "/desktop/clipboard", Some(body))?;
        Ok(())
    }

    pub fn create_job(&self, kind: &str, source: &str, dest: &str) -> Result<crate::jobs::Job> {
        let body = serde_json::json!({ "type": kind, "sourcePath": source, "destPath": dest });
        Ok(self.send("POST", "/jobs", Some(body))?.into_json()?)
    }

    /// A long-lived response body, e.g. an event stream; no read timeout applies.
    pub fn stream(&self, endpoint: &str) -> Result<Box<dyn std::io::Read + Send>> {
        Ok(self.send("GET", endpoint, None)?.into_reader())
    }

    pub fn preferences(&self) -> Result<Option<crate::preferences::Preferences>> {
        self.get("/desktop/preferences")
    }

    pub fn save_preferences(&self, preferences: &crate::preferences::Preferences) -> Result<()> {
        self.send(
            "PUT",
            "/desktop/preferences",
            Some(serde_json::to_value(preferences)?),
        )?;
        Ok(())
    }

    pub fn open(&self, path: &str) -> Result<()> {
        self.send(
            "POST",
            "/desktop/open",
            Some(serde_json::json!({ "path": path })),
        )?;
        Ok(())
    }

    pub fn terminal(&self, path: &str) -> Result<()> {
        self.send(
            "POST",
            "/desktop/terminal",
            Some(serde_json::json!({ "path": path })),
        )?;
        Ok(())
    }

    pub fn apps(&self, path: &str) -> Result<crate::desktop::Apps> {
        self.get(&format!("/desktop/apps?path={}", encode_query(path)))
    }

    pub fn open_with(&self, path: &str, app: &str) -> Result<()> {
        self.send(
            "POST",
            "/desktop/open-with",
            Some(serde_json::json!({ "path": path, "appId": app })),
        )?;
        Ok(())
    }

    /// PR #211: sets the user's MIME default for the file's type.
    pub fn set_default_app(&self, path: &str, app: &str) -> Result<()> {
        self.send(
            "POST",
            "/desktop/apps/default",
            Some(serde_json::json!({ "path": path, "appId": app })),
        )?;
        Ok(())
    }

    /// PR #211: the app's icon as PNG.
    pub fn app_icon(&self, app: &str) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(
            &mut self
                .call(&format!("/desktop/apps/icon?appId={}", encode_query(app)))?
                .into_reader(),
            &mut bytes,
        )?;
        Ok(bytes)
    }

    fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        self.call(endpoint)?
            .into_json()
            .with_context(|| format!("invalid JSON from {}{endpoint}", self.base))
    }

    fn call(&self, endpoint: &str) -> Result<ureq::Response> {
        self.send("GET", endpoint, None)
    }

    pub(crate) fn send(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<ureq::Response> {
        let url = format!("{}{endpoint}", self.base);
        let request = self.authorize(self.agent.request(method, &url));
        let sent = match body {
            Some(body) => request.send_json(body),
            None => request.call(),
        };
        match sent {
            Ok(response) => Ok(response),
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                anyhow::bail!(
                    "{}",
                    server_message(&body).unwrap_or(format!("HTTP {code}"))
                )
            }
            Err(error) => Err(error).with_context(|| format!("cannot reach {url}")),
        }
    }
}

/// `parseDirectory`: entries carry only names under the shared parent path, so a name that
/// could escape or nest under that path is rejected.
fn parse_directory(listing: FileList) -> Result<Vec<FileInfo>> {
    anyhow::ensure!(!listing.path.is_empty(), "invalid directory path");
    listing
        .items
        .into_iter()
        .map(|item| {
            let valid =
                !matches!(item.name.as_str(), "" | "." | "..") && !item.name.contains(['/', '\0']);
            anyhow::ensure!(valid, "invalid directory entry {:?}", item.name);
            Ok(FileInfo {
                path: format!("{}/{}", listing.path, item.name),
                ..item
            })
        })
        .collect()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub name: String,
    pub original_path: String,
    pub deleted_at: String,
    pub is_dir: bool,
    #[serde(default)]
    pub can_restore: bool,
}

fn server_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct ErrorBody {
        error: String,
    }
    serde_json::from_str::<ErrorBody>(body)
        .ok()
        .map(|body| body.error)
}

/// `encodeURIComponent` for one query value.
pub fn encode_query(value: &str) -> String {
    encode_path(value).replace('/', "%2F")
}

/// `encodeRoutePath`: each segment percent-encoded like `encodeURIComponent`.
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            segment
                .bytes()
                .map(|byte| match byte {
                    b'A'..=b'Z'
                    | b'a'..=b'z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'_'
                    | b'.'
                    | b'!'
                    | b'~'
                    | b'*'
                    | b'\''
                    | b'('
                    | b')' => (byte as char).to_string(),
                    _ => format!("%{byte:02X}"),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> FileInfo {
        FileInfo {
            name: name.into(),
            path: String::new(),
            size: 12,
            is_dir: false,
            mod_time: String::new(),
            permissions: String::new(),
        }
    }

    #[test]
    fn joins_entry_paths_under_the_listing() {
        let items = parse_directory(FileList {
            path: "media/My Photos".into(),
            items: vec![entry("été #1.txt")],
        })
        .unwrap();
        assert_eq!(items[0].path, "media/My Photos/été #1.txt");
    }

    #[test]
    fn rejects_unsafe_entry_names() {
        for name in ["", ".", "..", "nested/file", "bad\0name"] {
            assert!(
                parse_directory(FileList {
                    path: "media".into(),
                    items: vec![entry(name)]
                })
                .is_err(),
                "{name:?}"
            );
        }
        assert!(parse_directory(FileList {
            path: String::new(),
            items: vec![]
        })
        .is_err());
    }

    #[test]
    fn encodes_each_segment_like_encode_uri_component() {
        assert_eq!(
            encode_path("media/My Photos/été #1?.txt"),
            "media/My%20Photos/%C3%A9t%C3%A9%20%231%3F.txt"
        );
        assert_eq!(encode_path("a/(b)!~*'"), "a/(b)!~*'");
    }
}
