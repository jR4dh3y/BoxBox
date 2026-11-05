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
        parse_directory(self.get::<FileList>(&format!("/files/list/{}", encode_path(path)))?)
    }

    /// Recursive, case-insensitive name search below `path`. Results carry full virtual paths.
    pub fn search(&self, path: &str, query: &str) -> Result<Vec<FileInfo>> {
        let endpoint = format!(
            "/search?path={}&q={}",
            encode_component(path),
            encode_component(query)
        );
        parse_search(self.get::<SearchResponse>(&endpoint)?, path, query)
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

    fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        self.call(endpoint)?
            .into_json()
            .with_context(|| format!("invalid JSON from {}{endpoint}", self.base))
    }

    fn call(&self, endpoint: &str) -> Result<ureq::Response> {
        let url = format!("{}{endpoint}", self.base);
        let mut request = self.agent.get(&url);
        if let Some(token) = &self.token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        match request.call() {
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
