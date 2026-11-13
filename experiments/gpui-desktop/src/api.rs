//! File-manager operations backed by a native filesystem or a remote BoxBox HTTP server.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::local::LocalFiles;

mod http;
use http::RemoteApi;

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
}

#[derive(Debug, Deserialize)]
struct FileList {
    path: String,
    items: Vec<FileInfo>,
}

/// `GET /search`: the server echoes the request so a response can be matched to it.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    path: String,
    query: String,
    results: Vec<FileInfo>,
    count: usize,
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

fn deserialize_expiry<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> Result<Option<String>, D::Error> {
    let raw = Option::<String>::deserialize(de)?;
    Ok(raw.filter(|value| !value.is_empty()))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub name: String,
    pub original_path: String,
    pub deleted_at: String,
    pub is_dir: bool,
    pub can_restore: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharePermissions {
    pub view: bool,
    pub download: bool,
    pub upload: bool,
    pub delete: bool,
    pub manage: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareSummary {
    pub id: String,
    pub token: String,
    pub url: String,
    pub file_name: String,
    pub path: String,
    pub permissions: SharePermissions,
    pub max_upload_bytes: i64,
    pub is_folder: bool,
    pub created_at: String,
    #[serde(default, deserialize_with = "deserialize_expiry")]
    pub expires_at: Option<String>,
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

enum Provider {
    Local(LocalFiles),
    Remote(RemoteApi),
}

pub struct Api {
    provider: Provider,
}

impl Api {
    /// `base` is the server origin, e.g. `http://127.0.0.1:8080`.
    ///
    /// This always constructs a remote provider, including when the URL is invalid.
    pub fn new(base: &str, token: Option<String>) -> Self {
        Self {
            provider: Provider::Remote(RemoteApi::new(base, token)),
        }
    }

    pub fn local() -> Result<Self> {
        Ok(Self {
            provider: Provider::Local(LocalFiles::new()?),
        })
    }

    pub fn local_with_root(root: PathBuf) -> Result<Self> {
        Ok(Self {
            provider: Provider::Local(LocalFiles::with_root(root)?),
        })
    }

    pub fn is_local(&self) -> bool {
        matches!(&self.provider, Provider::Local(_))
    }

    /// The configured, credential-free server base URL; local mode has no server URL.
    pub fn server_url(&self) -> Option<&str> {
        match &self.provider {
            Provider::Local(_) => None,
            Provider::Remote(remote) => remote.server_url(),
        }
    }

    /// Resolve a virtual local path. Remote paths are owned by the server.
    /// The virtual path of a real local path, for a local provider that can reach it.
    pub fn virtual_path(&self, real: &std::path::Path) -> Option<String> {
        match &self.provider {
            Provider::Local(local) => local.virtual_path(real),
            Provider::Remote(_) => None,
        }
    }

    pub fn local_path(&self, path: &str) -> Result<Option<PathBuf>> {
        match &self.provider {
            Provider::Local(local) => local.local_path(path).map(Some),
            Provider::Remote(_) => Ok(None),
        }
    }

    pub fn roots(&self) -> Result<Vec<MountPoint>> {
        match &self.provider {
            Provider::Local(local) => local.roots(),
            Provider::Remote(remote) => remote.roots(),
        }
    }

    pub fn drives(&self) -> Result<Vec<DriveStats>> {
        match &self.provider {
            Provider::Local(local) => local.drives(),
            Provider::Remote(remote) => remote.drives(),
        }
    }

    pub fn trash(&self) -> Result<Vec<TrashItem>> {
        match &self.provider {
            Provider::Local(local) => local.trash(),
            Provider::Remote(_) => anyhow::bail!("Trash is available only in local mode"),
        }
    }

    pub fn move_to_trash(&self, path: &str) -> Result<TrashItem> {
        match &self.provider {
            Provider::Local(local) => local.move_to_trash(path),
            Provider::Remote(_) => anyhow::bail!("Trash is available only in local mode"),
        }
    }

    pub fn restore_trash(&self, id: &str) -> Result<()> {
        match &self.provider {
            Provider::Local(local) => local.restore_trash(id),
            Provider::Remote(_) => anyhow::bail!("Trash is available only in local mode"),
        }
    }

    pub fn delete_trash(&self, id: &str) -> Result<()> {
        match &self.provider {
            Provider::Local(local) => local.delete_trash(id),
            Provider::Remote(_) => anyhow::bail!("Trash is available only in local mode"),
        }
    }

    pub fn list(&self, path: &str) -> Result<Vec<FileInfo>> {
        match &self.provider {
            Provider::Local(local) => local.list(path),
            Provider::Remote(remote) => remote.list(path),
        }
    }

    pub fn search(&self, path: &str, query: &str) -> Result<Vec<FileInfo>> {
        match &self.provider {
            Provider::Local(local) => local.search(path, query),
            Provider::Remote(remote) => remote.search(path, query),
        }
    }

    pub fn create_directory(&self, parent: &str, name: &str) -> Result<FileInfo> {
        match &self.provider {
            Provider::Local(local) => local.create_directory(parent, name),
            Provider::Remote(remote) => remote.create_directory(parent, name),
        }
    }

    pub fn create_file(&self, parent: &str, name: &str, content: &str) -> Result<FileInfo> {
        match &self.provider {
            Provider::Local(local) => local.create_file(parent, name, content),
            Provider::Remote(remote) => remote.create_file(parent, name, content),
        }
    }

    pub fn rename(&self, old_path: &str, new_path: &str) -> Result<FileInfo> {
        match &self.provider {
            Provider::Local(local) => local.rename(old_path, new_path),
            Provider::Remote(remote) => remote.rename(old_path, new_path),
        }
    }

    pub fn delete(&self, path: &str, confirm: bool) -> Result<()> {
        match &self.provider {
            Provider::Local(local) => local.delete(path, confirm),
            Provider::Remote(remote) => remote.delete(path, confirm),
        }
    }

    pub fn download(&self, item: &FileInfo, dir: &Path) -> Result<PathBuf> {
        match &self.provider {
            Provider::Local(local) => local.download(item, dir),
            Provider::Remote(remote) => remote.download(item, dir),
        }
    }

    pub fn read_text(&self, path: &str, max_bytes: usize) -> Result<String> {
        match &self.provider {
            Provider::Local(local) => local.read_text(path, max_bytes),
            Provider::Remote(remote) => remote.read_text(path, max_bytes),
        }
    }

    /// Copies `source` to the exact virtual destination path, including its new name.
    pub fn copy(&self, source: &str, destination: &str) -> Result<()> {
        match &self.provider {
            Provider::Local(local) => local.copy(source, destination),
            Provider::Remote(remote) => remote.copy(source, destination),
        }
    }

    pub fn import_file(&self, source: &Path, parent: &str) -> Result<FileInfo> {
        match &self.provider {
            Provider::Local(local) => local.import_file(source, parent),
            Provider::Remote(remote) => remote.import_file(source, parent),
        }
    }

    pub fn thumbnail(&self, path: &str, width: u32, height: u32) -> Result<Vec<u8>> {
        match &self.provider {
            Provider::Local(local) => {
                let path = local.local_path(path)?;
                boxbox_thumbnail::thumbnail_path(&path, width, height)
                    .map_err(anyhow::Error::new)
            }
            Provider::Remote(remote) => remote.thumbnail(path, width, height),
        }
    }

    pub fn list_shares(&self) -> Result<Vec<ShareSummary>> {
        match &self.provider {
            Provider::Local(_) => {
                anyhow::bail!("Public sharing requires a BoxBox server")
            }
            Provider::Remote(remote) => remote.list_shares(),
        }
    }

    pub fn create_share(
        &self,
        path: &str,
        permissions: SharePermissions,
        expires_in_seconds: Option<i64>,
    ) -> Result<ShareSummary> {
        match &self.provider {
            Provider::Local(_) => {
                anyhow::bail!("Public sharing requires a BoxBox server")
            }
            Provider::Remote(remote) => {
                remote.create_share(path, permissions, expires_in_seconds)
            }
        }
    }

    pub fn update_share(
        &self,
        id: &str,
        permissions: SharePermissions,
        max_upload_bytes: Option<i64>,
    ) -> Result<ShareSummary> {
        match &self.provider {
            Provider::Local(_) => {
                anyhow::bail!("Public sharing requires a BoxBox server")
            }
            Provider::Remote(remote) => remote.update_share(id, permissions, max_upload_bytes),
        }
    }

    pub fn revoke_share(&self, id: &str) -> Result<()> {
        match &self.provider {
            Provider::Local(_) => {
                anyhow::bail!("Public sharing requires a BoxBox server")
            }
            Provider::Remote(remote) => remote.revoke_share(id),
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

/// Unlike a listing, search entries arrive with their own path, which must stay below the
/// folder that was searched.
fn parse_search(response: SearchResponse, path: &str, query: &str) -> Result<Vec<FileInfo>> {
    anyhow::ensure!(
        response.path == path && response.query == query,
        "search response does not match the request"
    );
    anyhow::ensure!(
        response.count == response.results.len(),
        "invalid search response"
    );
    let prefix = format!("{}/", path.trim_end_matches('/'));
    response
        .results
        .into_iter()
        .map(|item| {
            let name_valid =
                !matches!(item.name.as_str(), "" | "." | "..") && !item.name.contains(['/', '\0']);
            let path_valid = item.path.starts_with(&prefix)
                && item.path.ends_with(&format!("/{}", item.name))
                && !item.path.split('/').any(|segment| segment == "..");
            anyhow::ensure!(
                name_valid && path_valid,
                "invalid search result {:?}",
                item.path
            );
            Ok(item)
        })
        .collect()
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

/// `encodeRoutePath`: each segment percent-encoded like `encodeURIComponent`.
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_component)
        .collect::<Vec<_>>()
        .join("/")
}

/// `encodeURIComponent`, which also serves query-string values.
fn encode_component(text: &str) -> String {
    text.bytes()
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
        .collect()
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

    fn found(path: &str, name: &str) -> FileInfo {
        FileInfo {
            path: path.into(),
            ..entry(name)
        }
    }

    fn response(results: Vec<FileInfo>) -> SearchResponse {
        SearchResponse {
            path: "media".into(),
            query: "cat".into(),
            count: results.len(),
            results,
        }
    }

    #[test]
    fn parses_the_search_contract() {
        let body = r#"{"path":"media","query":"cat","count":1,"results":[
            {"name":"cat.png","path":"media/pets/cat.png","size":5,"isDir":false,
             "modTime":"2026-01-01T00:00:00Z","permissions":"-rw-r--r--","mimeType":"image/png"}]}"#;
        let parsed: SearchResponse = serde_json::from_str(body).unwrap();
        let items = parse_search(parsed, "media", "cat").unwrap();
        assert_eq!(items[0].path, "media/pets/cat.png");
        assert_eq!(items[0].size, 5);
    }

    #[test]
    fn keeps_results_below_the_searched_folder() {
        let ok = response(vec![found("media/a/cat.txt", "cat.txt")]);
        assert_eq!(parse_search(ok, "media", "cat").unwrap().len(), 1);

        for (path, name) in [
            ("", "cat.txt"),
            ("other/cat.txt", "cat.txt"),
            ("media-x/cat.txt", "cat.txt"),
            ("media/../cat.txt", "cat.txt"),
            ("media/dog.txt", "cat.txt"),
            ("media/cat.txt", "../cat.txt"),
            ("media/a/b", "a/b"),
        ] {
            let bad = response(vec![found(path, name)]);
            assert!(parse_search(bad, "media", "cat").is_err(), "{path} {name}");
        }
    }

    #[test]
    fn rejects_a_response_for_another_request() {
        let ok = || response(vec![found("media/cat", "cat")]);
        assert!(parse_search(ok(), "media", "dog").is_err());
        assert!(parse_search(ok(), "music", "cat").is_err());
        let mut miscounted = ok();
        miscounted.count = 2;
        assert!(parse_search(miscounted, "media", "cat").is_err());
    }

    #[test]
    fn query_values_escape_separators() {
        assert_eq!(encode_component("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }
}
