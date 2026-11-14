use std::{
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context as _, Result};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    encode_component, encode_path, parse_directory, parse_search, DriveStats, DriveStatsResponse,
    FileInfo, FileList, MountPoint, RootsResponse, SearchResponse, SharePermissions, ShareSummary,
};

const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 64 * 1024;
const MAX_THUMBNAIL_BYTES: usize = 32 * 1024 * 1024;
const UPLOAD_CHUNK_SIZE: usize = 1024 * 1024;
const COPY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const COPY_TIMEOUT: Duration = Duration::from_secs(30 * 60);
static NEXT_UPLOAD_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct RemoteApi {
    server_url: Option<String>,
    api_base: Option<String>,
    agent: ureq::Agent,
    job_agent: ureq::Agent,
    token: Option<String>,
}

impl RemoteApi {
    pub(super) fn new(base: &str, token: Option<String>) -> Self {
        let server_url = validate_server_url(base).ok();
        let api_base = server_url
            .as_ref()
            .map(|url| format!("{}/api/v1", url.trim_end_matches('/')));
        Self {
            server_url,
            api_base,
            agent: ureq::AgentBuilder::new().build(),
            job_agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(30))
                .build(),
            token,
        }
    }

    pub(super) fn server_url(&self) -> Option<&str> {
        self.server_url.as_deref()
    }

    pub(super) fn roots(&self) -> Result<Vec<MountPoint>> {
        Ok(self.get::<RootsResponse>("/files/")?.roots)
    }

    pub(super) fn drives(&self) -> Result<Vec<DriveStats>> {
        Ok(self.get::<DriveStatsResponse>("/files/stats")?.drives)
    }

    pub(super) fn list(&self, path: &str) -> Result<Vec<FileInfo>> {
        validate_remote_path(path)?;
        let endpoint = format!("/files/list/{}", encode_path(path));
        parse_directory(self.get::<FileList>(&endpoint)?)
    }

    /// The server matches one piece of text anywhere in the name, so it is asked for
    /// the query's longest word and the answer is matched and ranked here.
    pub(super) fn search(&self, request: &crate::search::Request) -> Result<Vec<FileInfo>> {
        let path = request.path;
        validate_remote_path(path)?;
        let Some(filter) = request.query.broadest_filter() else {
            bail!("Search needs a word without * or ? on a server");
        };
        let endpoint = format!("/search?path={}&q={}", encode_component(path), encode_component(filter));
        let listed = parse_search(self.get::<SearchResponse>(&endpoint)?, path, filter)?;
        Ok(crate::search::rank_listed(listed, request))
    }

    pub(super) fn create_directory(&self, parent: &str, name: &str) -> Result<FileInfo> {
        validate_remote_path(parent)?;
        validate_file_name(name)?;
        let endpoint = format!("/files/{}", encode_path(parent));
        self.mutate_json(
            "POST",
            &endpoint,
            json!({ "type": "directory", "name": name }),
        )
    }

    pub(super) fn create_file(
        &self,
        parent: &str,
        name: &str,
        content: &str,
    ) -> Result<FileInfo> {
        validate_remote_path(parent)?;
        validate_file_name(name)?;
        let endpoint = format!("/files/{}", encode_path(parent));
        self.mutate_json(
            "POST",
            &endpoint,
            json!({ "type": "file", "name": name, "content": content }),
        )
    }

    pub(super) fn rename(&self, old_path: &str, new_path: &str) -> Result<FileInfo> {
        validate_remote_path(old_path)?;
        validate_remote_path(new_path)?;
        let endpoint = format!("/files/{}", encode_path(old_path));
        self.mutate_json("PUT", &endpoint, json!({ "newPath": new_path }))
    }

    pub(super) fn delete(&self, path: &str, confirm: bool) -> Result<()> {
        validate_remote_path(path)?;
        let endpoint = format!(
            "/files/{}?confirm={confirm}",
            encode_path(path)
        );
        self.mutate_empty("DELETE", &endpoint, None)
    }

    pub(super) fn download(&self, item: &FileInfo, dir: &Path) -> Result<PathBuf> {
        validate_remote_path(&item.path)?;
        validate_file_name(&item.name)?;
        let (route, name) = if item.is_dir {
            ("/stream/archive", format!("{}.zip", item.name))
        } else {
            ("/stream/download", item.name.clone())
        };
        let endpoint = format!("{route}/{}", encode_path(&item.path));
        let response = self.call("GET", &endpoint, None)?;
        let target = dir.join(name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .with_context(|| {
                format!(
                    "cannot create {} without overwriting an existing file",
                    target.display()
                )
            })?;
        if let Err(error) = std::io::copy(&mut response.into_reader(), &mut file) {
            drop(file);
            let _ = fs::remove_file(&target);
            return Err(error).with_context(|| format!("cannot download {}", item.path));
        }
        Ok(target)
    }

    pub(super) fn read_text(&self, path: &str, max_bytes: usize) -> Result<String> {
        anyhow::ensure!(
            max_bytes <= MAX_TEXT_BYTES,
            "text preview limit exceeds the {}-byte cap",
            MAX_TEXT_BYTES
        );
        validate_remote_path(path)?;
        let endpoint = format!("/stream/preview/{}", encode_path(path));
        let response = self.call("GET", &endpoint, None)?;
        let bytes = read_limited(response, max_bytes, "text preview")?;
        String::from_utf8(bytes).context("text preview is not valid UTF-8")
    }

    pub(super) fn thumbnail(&self, path: &str, width: u32, height: u32) -> Result<Vec<u8>> {
        validate_remote_path(path)?;
        let endpoint = format!(
            "/stream/thumbnail/{}?width={width}&height={height}",
            encode_path(path)
        );
        let response = self.call("GET", &endpoint, None)?;
        read_limited(response, MAX_THUMBNAIL_BYTES, "thumbnail")
    }

    pub(super) fn copy(&self, source: &str, destination: &str) -> Result<()> {
        validate_remote_path(source)?;
        validate_remote_path(destination)?;
        let mut job: JobStatus = self.mutate_json_with_agent(
            &self.job_agent,
            "POST",
            "/jobs",
            json!({ "type": "copy", "sourcePath": source, "destPath": destination }),
        )?;
        anyhow::ensure!(!job.id.is_empty(), "server returned a copy job without an id");
        let deadline = Instant::now() + COPY_TIMEOUT;
        loop {
            match job.state {
                JobState::Completed => return Ok(()),
                JobState::Failed => anyhow::bail!(
                    "copy failed: {}",
                    job.error.unwrap_or_else(|| "server reported an unspecified error".into())
                ),
                JobState::Cancelled => anyhow::bail!("copy was cancelled on the server"),
                JobState::Pending | JobState::Running => {}
            }
            let now = Instant::now();
            if now >= deadline {
                let endpoint = format!("/jobs/{}", encode_component(&job.id));
                match self.mutate_empty_with_agent(&self.job_agent, "DELETE", &endpoint, None) {
                    Ok(()) => anyhow::bail!(
                        "copy did not finish before the 30-minute timeout; server cancellation was requested"
                    ),
                    Err(error) => anyhow::bail!(
                        "copy did not finish before the 30-minute timeout; cancellation request failed: {error:#}"
                    ),
                }
            }
            thread::sleep(COPY_POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
            let endpoint = format!("/jobs/{}", encode_component(&job.id));
            job = self.get_with_agent(&self.job_agent, &endpoint)?;
        }
    }

    pub(super) fn import_file(&self, source: &Path, parent: &str) -> Result<FileInfo> {
        validate_remote_path(parent)?;
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("import source must have a valid UTF-8 file name")?;
        validate_file_name(name)?;
        let metadata = fs::symlink_metadata(source)
            .with_context(|| format!("cannot inspect import source {}", source.display()))?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "symbolic links cannot be imported"
        );
        let target = join_remote_path(parent, name);
        self.import_path(source, &target)?;
        Ok(file_info_from_metadata(name, &target, &metadata))
    }

    pub(super) fn list_shares(&self) -> Result<Vec<ShareSummary>> {
        Ok(self.get::<ShareListResponse>("/shares")?.shares)
    }

    pub(super) fn create_share(
        &self,
        path: &str,
        permissions: SharePermissions,
        expires_in_seconds: Option<i64>,
    ) -> Result<ShareSummary> {
        validate_remote_path(path)?;
        if let Some(seconds) = expires_in_seconds {
            anyhow::ensure!(
                seconds > 0,
                "share expiry must be a positive number of seconds"
            );
        }
        let mut request = json!({ "path": path, "permissions": permissions });
        if let Some(seconds) = expires_in_seconds {
            request["expiresInSeconds"] = json!(seconds);
        }
        let created: CreatedShare = self.mutate_json("POST", "/shares", request)?;
        Ok(ShareSummary {
            id: created.id,
            token: created.token,
            url: created.url,
            file_name: created.file_name,
            path: path.to_owned(),
            permissions: created.permissions,
            max_upload_bytes: created.max_upload_bytes,
            is_folder: created.is_folder,
            created_at: created.created_at,
            expires_at: created.expires_at,
        })
    }

    pub(super) fn update_share(
        &self,
        id: &str,
        permissions: SharePermissions,
        max_upload_bytes: Option<i64>,
    ) -> Result<ShareSummary> {
        anyhow::ensure!(!id.is_empty(), "share id is required");
        let endpoint = format!("/shares/{}", encode_component(id));
        let mut request = json!({ "permissions": permissions });
        if let Some(bytes) = max_upload_bytes {
            request["maxUploadBytes"] = json!(bytes);
        }
        self.mutate_json("PATCH", &endpoint, request)
    }

    pub(super) fn revoke_share(&self, id: &str) -> Result<()> {
        anyhow::ensure!(!id.is_empty(), "share id is required");
        let endpoint = format!("/shares/{}", encode_component(id));
        self.mutate_empty("DELETE", &endpoint, None)
    }

    fn import_path(&self, source: &Path, target: &str) -> Result<()> {
        let metadata = fs::symlink_metadata(source)
            .with_context(|| format!("cannot inspect import source {}", source.display()))?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "symbolic links cannot be imported"
        );
        if metadata.is_dir() {
            self.create_upload_directory(target)?;
            for entry in fs::read_dir(source)
                .with_context(|| format!("cannot read import directory {}", source.display()))?
            {
                let entry = entry.with_context(|| {
                    format!("cannot read import directory {}", source.display())
                })?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("import source contains a non-UTF-8 file name"))?;
                validate_file_name(&name)?;
                self.import_path(&entry.path(), &join_remote_path(target, &name))?;
            }
            return Ok(());
        }
        anyhow::ensure!(
            metadata.is_file(),
            "import source is not a regular file or directory"
        );
        self.upload_file(source, target, metadata.len())
    }

    fn create_upload_directory(&self, path: &str) -> Result<()> {
        let endpoint = format!("/stream/upload/{}", encode_path(path));
        let headers = [("X-Create-Only", "true"), ("X-Upload-Type", "directory")];
        let response = self.call_bytes("POST", &endpoint, &headers, &[])?;
        let result: UploadResponse = self.decode_json(response)?;
        anyhow::ensure!(result.complete, "server did not complete directory creation");
        Ok(())
    }

    fn upload_file(&self, source: &Path, target: &str, size: u64) -> Result<()> {
        let total_size =
            i64::try_from(size).context("file is too large for the server upload protocol")?;
        let chunk_size = UPLOAD_CHUNK_SIZE as i64;
        let total_chunks = if total_size == 0 {
            1
        } else {
            1 + (total_size - 1) / chunk_size
        };
        let checksum = checksum_file(source)?;
        let checksum_header = format!("sha256:{checksum}");
        let upload_id = new_upload_id();
        let endpoint = format!("/stream/upload/{}", encode_path(target));
        let chunks_header = total_chunks.to_string();
        let chunk_size_header = chunk_size.to_string();
        let total_size_header = total_size.to_string();
        let mut file = File::open(source)
            .with_context(|| format!("cannot open import source {}", source.display()))?;
        let mut buffer = vec![0u8; UPLOAD_CHUNK_SIZE];

        for index in 0..total_chunks {
            let offset = index * chunk_size;
            let bytes_this_chunk = (total_size - offset).min(chunk_size) as usize;
            file.read_exact(&mut buffer[..bytes_this_chunk])
                .with_context(|| format!("cannot read import source {}", source.display()))?;
            if index == total_chunks - 1 {
                let mut extra = [0u8; 1];
                anyhow::ensure!(
                    file.read(&mut extra)? == 0,
                    "import source changed while it was being uploaded"
                );
            }

            let index_header = index.to_string();
            let headers = [
                ("Content-Type", "application/octet-stream"),
                ("X-Upload-ID", upload_id.as_str()),
                ("X-Chunk-Index", index_header.as_str()),
                ("X-Total-Chunks", chunks_header.as_str()),
                ("X-Chunk-Size", chunk_size_header.as_str()),
                ("X-Total-Size", total_size_header.as_str()),
                ("X-Create-Only", "true"),
                ("X-Checksum", checksum_header.as_str()),
            ];
            let header_count = if index == total_chunks - 1 { 8 } else { 7 };
            let response = self.call_bytes(
                "POST",
                &endpoint,
                &headers[..header_count],
                &buffer[..bytes_this_chunk],
            )?;
            let result: UploadResponse = self.decode_json(response)?;
            if index == total_chunks - 1 {
                anyhow::ensure!(result.complete, "server did not complete file upload");
            }
        }
        Ok(())
    }

    fn get<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        self.get_with_agent(&self.agent, endpoint)
    }

    fn get_with_agent<T: DeserializeOwned>(
        &self,
        agent: &ureq::Agent,
        endpoint: &str,
    ) -> Result<T> {
        let response = self.call_with_agent(agent, "GET", endpoint, None)?;
        self.decode_json(response)
    }

    fn mutate_json<T: DeserializeOwned>(
        &self,
        method: &str,
        endpoint: &str,
        body: Value,
    ) -> Result<T> {
        self.mutate_json_with_agent(&self.agent, method, endpoint, body)
    }

    fn mutate_json_with_agent<T: DeserializeOwned>(
        &self,
        agent: &ureq::Agent,
        method: &str,
        endpoint: &str,
        body: Value,
    ) -> Result<T> {
        let response = self.call_with_agent(agent, method, endpoint, Some(body))?;
        self.decode_json(response)
    }

    fn mutate_empty(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<()> {
        self.mutate_empty_with_agent(&self.agent, method, endpoint, body)
    }

    fn mutate_empty_with_agent(
        &self,
        agent: &ureq::Agent,
        method: &str,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<()> {
        let response = self.call_with_agent(agent, method, endpoint, body)?;
        let _ = read_limited(response, MAX_JSON_BYTES, "server response")?;
        Ok(())
    }

    fn decode_json<T: DeserializeOwned>(&self, response: ureq::Response) -> Result<T> {
        let bytes = read_limited(response, MAX_JSON_BYTES, "server response")?;
        serde_json::from_slice(&bytes).context("invalid JSON response from BoxBox server")
    }

    fn call(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<ureq::Response> {
        self.call_with_agent(&self.agent, method, endpoint, body)
    }

    fn call_with_agent(
        &self,
        agent: &ureq::Agent,
        method: &str,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<ureq::Response> {
        let request = self.request_with_agent(agent, method, endpoint)?;
        let result = match body {
            Some(body) => request.send_json(body),
            None => request.call(),
        };
        handle_result(result)
    }

    fn call_bytes(
        &self,
        method: &str,
        endpoint: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<ureq::Response> {
        let mut request = self.request(method, endpoint)?;
        for &(name, value) in headers {
            request = request.set(name, value);
        }
        handle_result(request.send_bytes(body))
    }

    fn request(&self, method: &str, endpoint: &str) -> Result<ureq::Request> {
        self.request_with_agent(&self.agent, method, endpoint)
    }

    fn request_with_agent(
        &self,
        agent: &ureq::Agent,
        method: &str,
        endpoint: &str,
    ) -> Result<ureq::Request> {
        let base = self.api_base.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "BOXBOX_URL must be an http(s) URL without credentials, query, or fragment"
            )
        })?;
        let url = format!("{base}{endpoint}");
        let mut request = agent.request(method, &url);
        if let Some(token) = &self.token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        Ok(request)
    }
}

fn handle_result(
    result: std::result::Result<ureq::Response, ureq::Error>,
) -> Result<ureq::Response> {
    match result {
        Ok(response) => Ok(response),
        Err(ureq::Error::Status(code, response)) => {
            let body =
                read_limited(response, MAX_ERROR_BYTES, "error response").unwrap_or_default();
            let message = std::str::from_utf8(&body)
                .ok()
                .and_then(super::server_message)
                .unwrap_or_else(|| format!("HTTP {code}"));
            anyhow::bail!(message)
        }
        Err(ureq::Error::Transport(_)) => anyhow::bail!("cannot reach BoxBox server"),
    }
}

fn read_limited(response: ureq::Response, max_bytes: usize, label: &str) -> Result<Vec<u8>> {
    let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(limit)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read {label}"))?;
    anyhow::ensure!(
        bytes.len() <= max_bytes,
        "{label} exceeds the {max_bytes}-byte limit"
    );
    Ok(bytes)
}

fn validate_server_url(base: &str) -> Result<String> {
    let value = base.trim_end_matches('/');
    let (scheme, remainder) = value
        .split_once("://")
        .context("BOXBOX_URL must include an http(s) scheme")?;
    anyhow::ensure!(
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"),
        "BOXBOX_URL must use http or https"
    );
    anyhow::ensure!(
        !value.contains(['?', '#'])
            && !value.chars().any(|character| character.is_control() || character.is_whitespace()),
        "BOXBOX_URL cannot contain a query, fragment, or whitespace"
    );
    let authority = remainder.split('/').next().unwrap_or_default();
    anyhow::ensure!(!authority.is_empty(), "BOXBOX_URL must include a host");
    anyhow::ensure!(
        !authority.contains('@'),
        "BOXBOX_URL must not contain user credentials"
    );
    Ok(value.to_owned())
}

fn validate_remote_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        !path.starts_with('/')
            && !path.ends_with('/')
            && !path.contains('\\')
            && !path.contains('\0'),
        "invalid remote path"
    );
    for segment in path.split('/') {
        anyhow::ensure!(
            !matches!(segment, "" | "." | "..") && !segment.chars().any(char::is_control),
            "invalid remote path"
        );
    }
    Ok(())
}

fn validate_file_name(name: &str) -> Result<()> {
    anyhow::ensure!(
        !matches!(name, "" | "." | "..")
            && !name.contains(['/', '\\', '\0'])
            && !name.chars().any(char::is_control),
        "unsafe file name"
    );
    Ok(())
}

fn join_remote_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn checksum_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .with_context(|| format!("cannot open import source {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; UPLOAD_CHUNK_SIZE];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("cannot read import source {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok(output)
}

fn new_upload_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = NEXT_UPLOAD_ID.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", std::process::id(), timestamp, counter)
}

fn file_info_from_metadata(name: &str, path: &str, metadata: &fs::Metadata) -> FileInfo {
    let mod_time = metadata
        .modified()
        .ok()
        .map(|time| DateTime::<Utc>::from(time).to_rfc3339())
        .unwrap_or_default();
    FileInfo {
        name: name.to_owned(),
        path: path.to_owned(),
        size: if metadata.is_file() { metadata.len() } else { 0 },
        is_dir: metadata.is_dir(),
        mod_time,
    }
}

#[derive(Deserialize)]
struct ShareListResponse {
    shares: Vec<ShareSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedShare {
    id: String,
    token: String,
    url: String,
    file_name: String,
    permissions: super::SharePermissions,
    max_upload_bytes: i64,
    is_folder: bool,
    created_at: String,
    #[serde(default, deserialize_with = "super::deserialize_expiry")]
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobStatus {
    id: String,
    state: JobState,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum JobState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Debug, Deserialize)]
struct UploadResponse {
    complete: bool,
}
