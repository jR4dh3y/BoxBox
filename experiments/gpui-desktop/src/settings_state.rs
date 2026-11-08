use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::api::{SharePermissions, ShareSummary};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WallpaperFit {
    Fit,
    #[default]
    Fill,
    Stretch,
    Center,
    Tile,
}

impl WallpaperFit {
    pub(crate) const ALL: [Self; 5] = [Self::Fill, Self::Fit, Self::Stretch, Self::Center, Self::Tile];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Fit => "Contain",
            Self::Fill => "Cover",
            Self::Stretch => "Stretch",
            Self::Center => "Center",
            Self::Tile => "Tile",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccentColor {
    #[default]
    Blue,
    Purple,
    Green,
    Orange,
    Rose,
}

impl AccentColor {
    pub(crate) const ALL: [Self; 5] = [
        Self::Blue,
        Self::Purple,
        Self::Green,
        Self::Orange,
        Self::Rose,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Green => "Green",
            Self::Orange => "Orange",
            Self::Rose => "Rose",
        }
    }

    pub(crate) const fn hex(self) -> u32 {
        match self {
            Self::Blue => 0x4a9eff,
            Self::Purple => 0xa78bfa,
            Self::Green => 0x2ecc71,
            Self::Orange => 0xf39c12,
            Self::Rose => 0xf472b6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SavedConnection {
    #[serde(default = "new_connection_id")]
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) url: String,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct DesktopSettings {
    pub(crate) wallpaper: Option<PathBuf>,
    pub(crate) wallpaper_fit: WallpaperFit,
    pub(crate) accent: AccentColor,
    pub(crate) custom_accent: Option<u32>,
    pub(crate) accent_from_wallpaper: bool,
    pub(crate) connections: Vec<SavedConnection>,
    #[serde(skip)]
    pub(crate) load_error: Option<String>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            wallpaper: None,
            wallpaper_fit: WallpaperFit::default(),
            accent: AccentColor::default(),
            custom_accent: None,
            accent_from_wallpaper: true,
            connections: Vec::new(),
            load_error: None,
        }
    }
}

impl DesktopSettings {
    pub(crate) fn load() -> Self {
        let path = settings_path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                return Self {
                    load_error: Some(format!("Could not read desktop settings: {error}")),
                    ..Self::default()
                }
            }
        };

        let mut settings = match serde_json::from_slice::<Self>(&bytes) {
            Ok(settings) => settings,
            Err(error) => {
                return Self {
                    load_error: Some(format!("Could not read desktop settings: {error}")),
                    ..Self::default()
                }
            }
        };

        let original_len = settings.connections.len();
        settings.connections.retain_mut(|connection| {
            if connection.id.is_empty() {
                return false;
            }
            let Ok(name) = validate_connection_name(&connection.name) else {
                return false;
            };
            let Ok(url) = validate_server_url(&connection.url) else {
                return false;
            };
            connection.name = name.to_string();
            connection.url = url;
            true
        });
        if settings.connections.len() != original_len {
            settings.load_error = Some(
                "Some saved connections were invalid and have been omitted.".to_string(),
            );
        }
        settings
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        for connection in &self.connections {
            if connection.id.is_empty() {
                return Err("Could not save desktop settings: saved connection has no id.".into());
            }
            validate_connection_name(&connection.name)
                .map_err(|error| format!("Could not save desktop settings: {error}"))?;
            validate_server_url(&connection.url)
                .map_err(|error| format!("Could not save desktop settings: {error}"))?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("Could not encode desktop settings: {error}"))?;
        atomic_write(&settings_path(), &bytes)
            .map_err(|error| format!("Could not save desktop settings: {error}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConnectionField {
    Name,
    Url,
    Token,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShareField {
    Path,
    UploadLimit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ShareExpiry {
    #[default]
    Never,
    OneDay,
    OneWeek,
    OneMonth,
}

impl ShareExpiry {
    pub(crate) const ALL: [Self; 4] = [Self::Never, Self::OneDay, Self::OneWeek, Self::OneMonth];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::OneDay => "1 day",
            Self::OneWeek => "7 days",
            Self::OneMonth => "30 days",
        }
    }

    pub(crate) const fn seconds(self) -> Option<i64> {
        match self {
            Self::Never => None,
            Self::OneDay => Some(86_400),
            Self::OneWeek => Some(604_800),
            Self::OneMonth => Some(2_592_000),
        }
    }
}

pub(crate) enum SettingsDialog {
    EditAccent { color: String },
    EditConnection {
        index: Option<usize>,
        name: String,
        url: String,
        focused: ConnectionField,
    },
    ConnectSaved {
        connection: SavedConnection,
        token: String,
        focused: ConnectionField,
    },
    RemoveConnection {
        index: usize,
        name: String,
    },
    CreateShare {
        path: String,
        permissions: SharePermissions,
        expires: ShareExpiry,
        focused: ShareField,
    },
    EditShare {
        id: String,
        label: String,
        permissions: SharePermissions,
        max_upload_bytes: String,
        focused: ShareField,
    },
    RevokeShare {
        id: String,
        label: String,
    },
}

pub(crate) enum ShareListState {
    Idle,
    Loading {
        server: String,
        provider: Arc<crate::api::Api>,
    },
    Loaded {
        server: String,
        provider: Arc<crate::api::Api>,
        shares: Vec<ShareSummary>,
    },
    Failed {
        server: String,
        provider: Arc<crate::api::Api>,
        error: String,
    },
}

impl Default for ShareListState {
    fn default() -> Self {
        Self::Idle
    }
}

pub(crate) struct SettingsPanelState {
    pub(crate) dialog: Option<SettingsDialog>,
    pub(crate) shares: ShareListState,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) notice: Option<String>,
    pub(crate) share_request: u64,
    pub(crate) input_selected: bool,
}

impl Default for SettingsPanelState {
    fn default() -> Self {
        Self {
            dialog: None,
            shares: ShareListState::Idle,
            busy: false,
            error: None,
            notice: None,
            share_request: 0,
            input_selected: false,
        }
    }
}

pub(crate) fn validate_connection(name: &str, url: &str) -> Result<SavedConnection, String> {
    let name = validate_connection_name(name)?;
    let url = validate_server_url(url)?;
    Ok(SavedConnection {
        id: new_connection_id(),
        name: name.to_string(),
        url,
    })
}

fn validate_connection_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Connection name cannot be empty.".to_string());
    }
    if name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(
            "Connection name must be at most 80 characters without control characters."
                .to_string(),
        );
    }
    Ok(name)
}

pub(crate) fn validate_server_url(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Server URL cannot be empty.".to_string());
    }
    if input.chars().any(char::is_whitespace) || input.chars().any(char::is_control) {
        return Err("Server URL cannot contain whitespace or control characters.".to_string());
    }
    if input.contains('?') || input.contains('#') {
        return Err("Server URL cannot include a query string or fragment.".to_string());
    }

    let (scheme, rest) = input
        .split_once("://")
        .ok_or_else(|| "Server URL must start with http:// or https://.".to_string())?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "http" => "http",
        "https" => "https",
        _ => return Err("Server URL must use http:// or https://.".to_string()),
    };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let path = &rest[authority_end..];
    if authority.is_empty() || authority.contains('@') {
        return Err("Server URL must contain a host and cannot include user information.".to_string());
    }
    validate_authority(authority)?;
    Ok(format!("{scheme}://{authority}{path}"))
}

fn validate_authority(authority: &str) -> Result<(), String> {
    let port = if let Some(bracketed) = authority.strip_prefix('[') {
        let (address, suffix) = bracketed
            .split_once(']')
            .ok_or_else(|| "Server URL contains an invalid IPv6 host.".to_string())?;
        address
            .parse::<std::net::Ipv6Addr>()
            .map_err(|_| "Server URL contains an invalid IPv6 host.".to_string())?;
        if suffix.is_empty() {
            None
        } else {
            Some(
                suffix
                    .strip_prefix(':')
                    .ok_or_else(|| "Server URL contains an invalid port.".to_string())?,
            )
        }
    } else {
        let mut pieces = authority.split(':');
        let host = pieces.next().unwrap_or_default();
        let port = pieces.next();
        if pieces.next().is_some() {
            return Err("IPv6 server addresses must use brackets.".to_string());
        }
        if host.is_empty() || !valid_host(host) {
            return Err("Server URL contains an invalid host.".to_string());
        }
        port
    };

    if let Some(port) = port {
        let port = port
            .parse::<u16>()
            .map_err(|_| "Server URL contains an invalid port.".to_string())?;
        if port == 0 {
            return Err("Server URL port must be between 1 and 65535.".to_string());
        }
    }
    Ok(())
}

fn new_connection_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp:x}-{:x}-{sequence:x}", std::process::id())
}

fn valid_host(host: &str) -> bool {
    if host.len() > 253 {
        return false;
    }
    let labels = host.strip_suffix('.').unwrap_or(host).split('.');
    let mut found_label = false;
    for label in labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return false;
        }
        found_label = true;
    }
    found_label
}


/// Keep wallpaper bytes in app storage so moving the selected source cannot break startup.
pub(crate) fn store_wallpaper(bytes: &[u8], digest: &str) -> Result<PathBuf, String> {
    let path = settings_path().with_file_name(format!("wallpaper-{digest}.image"));
    atomic_write(&path, bytes).map_err(|error| format!("Could not save wallpaper: {error}"))?;
    Ok(path)
}

fn settings_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("boxbox-gpui")
        .join("desktop-settings.json")
}

fn atomic_write(path: &Path, contents: &[u8]) -> io::Result<()> {
    static TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "settings path has no parent"))?;
    fs::create_dir_all(parent)?;

    let (temporary, mut file) = loop {
        let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".desktop-settings-{}-{id}.tmp", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };

    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
