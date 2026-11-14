//! A typed location such as `smb://nas/media/films` as the mount spec GVfs
//! keys its mounts by, plus the path inside that mount. The mapping is GIO's
//! (`gvfsurimapper*.c`), so a share mounted here is the one other apps see.

use std::collections::HashMap;

use anyhow::{bail, ensure, Result};
use zbus::zvariant::Value;

/// `(aya{sv})`: the mount prefix, then the keys as NUL-terminated byte strings.
pub type Wire<'a> = (Vec<u8>, HashMap<&'a str, Value<'a>>);

#[derive(Debug, PartialEq, Eq)]
pub struct Location {
    /// `type` first, then what identifies the mount.
    pub spec: Vec<(&'static str, String)>,
    /// The folder inside the mount, without a leading slash.
    pub path: String,
}

impl Location {
    pub fn wire(&self) -> Wire<'_> {
        (bytestring("/"), self.spec.iter().map(|(key, value)| (*key, Value::from(bytestring(value)))).collect())
    }
}

pub fn bytestring(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

/// Whether the text names a location to mount rather than a path.
pub fn is_uri(text: &str) -> bool {
    text.split_once("://").is_some_and(|(scheme, _)| {
        !scheme.is_empty() && scheme != "file" && scheme.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
    })
}

pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = (bytes[index] == b'%').then(|| bytes.get(index + 1..index + 3)).flatten();
        match hex.and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()) {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

struct Authority {
    user: Option<String>,
    host: String,
    port: Option<u16>,
}

fn authority(text: &str) -> Result<Authority> {
    let (user, rest) = match text.rsplit_once('@') {
        Some((user, rest)) => (Some(percent_decode(user)).filter(|user| !user.is_empty()), rest),
        None => (None, text),
    };
    // An IPv6 address is bracketed, so its colons are not a port.
    let (host, port) = match rest.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        Some((host, port)) => (host, port.strip_prefix(':')),
        None => match rest.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (rest, None),
        },
    };
    let port = match port {
        Some(port) => Some(port.parse().map_err(|_| anyhow::anyhow!("{port} is not a port number"))?),
        None => None,
    };
    ensure!(!host.is_empty(), "the location needs a server name");
    Ok(Authority { user, host: percent_decode(host), port })
}

pub fn parse(uri: &str) -> Result<Location> {
    let Some((scheme, rest)) = uri.trim().split_once("://") else { bail!("{uri} is not a network location") };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = percent_decode(path.trim_matches('/'));
    let scheme = scheme.to_ascii_lowercase();
    match scheme.as_str() {
        "smb" => {
            let server = authority(host)?;
            let (share, path) = path.split_once('/').unwrap_or((&path, ""));
            ensure!(!share.is_empty(), "name the shared folder too, as in smb://{}/share", server.host);
            let mut spec = vec![("type", "smb-share".to_string()), ("server", server.host.to_lowercase()), ("share", share.to_lowercase())];
            // `domain;user`
            if let Some(user) = server.user {
                match user.split_once(';') {
                    Some((domain, user)) => {
                        spec.push(("user", user.to_string()));
                        spec.push(("domain", domain.to_string()));
                    }
                    None => spec.push(("user", user)),
                }
            }
            if let Some(port) = server.port.filter(|port| *port != 445) {
                spec.push(("port", port.to_string()));
            }
            Ok(Location { spec, path: path.to_string() })
        }
        "sftp" | "ssh" | "ftp" | "ftps" | "ftpis" => {
            let (kind, default_port) = match scheme.as_str() {
                "ftp" => ("ftp", 21),
                "ftps" => ("ftps", 21),
                "ftpis" => ("ftpis", 990),
                _ => ("sftp", 22),
            };
            let server = authority(host)?;
            let mut spec = vec![("type", kind.to_string()), ("host", server.host)];
            if let Some(user) = server.user {
                spec.push(("user", user));
            }
            if let Some(port) = server.port.filter(|port| *port != default_port) {
                spec.push(("port", port.to_string()));
            }
            Ok(Location { spec, path })
        }
        // A device on a bus: the whole authority, such as `[usb:001,005]`, names it.
        "mtp" | "gphoto2" | "afc" => {
            ensure!(!host.is_empty(), "the location needs a device name");
            let kind = match scheme.as_str() {
                "mtp" => "mtp",
                "gphoto2" => "gphoto2",
                _ => "afc",
            };
            Ok(Location { spec: vec![("type", kind.to_string()), ("host", percent_decode(host))], path })
        }
        other => bail!("{other}:// locations are not supported"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(uri: &str) -> Vec<(&'static str, String)> {
        parse(uri).unwrap().spec
    }

    fn pairs(items: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
        items.iter().map(|(key, value)| (*key, value.to_string())).collect()
    }

    #[test]
    fn a_share_keeps_the_folder_inside_it() {
        let location = parse("smb://NAS/Media/Films/2024/").unwrap();
        assert_eq!(location.spec, pairs(&[("type", "smb-share"), ("server", "nas"), ("share", "media")]));
        assert_eq!(location.path, "Films/2024");
    }

    #[test]
    fn a_share_takes_domain_user_and_port() {
        assert_eq!(
            spec("smb://WORK;ana@files:4450/docs"),
            pairs(&[("type", "smb-share"), ("server", "files"), ("share", "docs"), ("user", "ana"), ("domain", "WORK"), ("port", "4450")])
        );
        assert_eq!(spec("smb://files:445/docs"), pairs(&[("type", "smb-share"), ("server", "files"), ("share", "docs")]));
    }

    #[test]
    fn a_server_without_a_share_is_refused() {
        assert!(parse("smb://nas").unwrap_err().to_string().contains("smb://nas/share"));
    }

    #[test]
    fn remote_logins_drop_their_default_port() {
        assert_eq!(spec("ssh://me@host:22/home/me"), pairs(&[("type", "sftp"), ("host", "host"), ("user", "me")]));
        assert_eq!(spec("sftp://[::1]:2222/"), pairs(&[("type", "sftp"), ("host", "::1"), ("port", "2222")]));
        assert_eq!(spec("ftp://host:2121"), pairs(&[("type", "ftp"), ("host", "host"), ("port", "2121")]));
        assert_eq!(parse("sftp://host/srv/a%20b").unwrap().path, "srv/a b");
    }

    #[test]
    fn a_phone_is_named_by_its_whole_authority() {
        assert_eq!(spec("mtp://%5Busb%3A001,005%5D/"), pairs(&[("type", "mtp"), ("host", "[usb:001,005]")]));
        assert_eq!(spec("mtp://[usb:001,005]/"), pairs(&[("type", "mtp"), ("host", "[usb:001,005]")]));
    }

    #[test]
    fn only_locations_are_uris() {
        assert!(is_uri("smb://nas/share"));
        assert!(!is_uri("file:///home"));
        assert!(!is_uri("home/a://b"));
        assert!(!is_uri("documents"));
        assert!(parse("dav://host/").is_err());
    }
}
