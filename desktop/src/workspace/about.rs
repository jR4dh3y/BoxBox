//! Settings → About: the version this build came from, when it was installed and
//! on what system, read off the UI thread the first time About opens.

use gpui::Context;

use super::{Load, Workspace};

/// The square the logo is drawn in, in pixels at scale 1.
pub(crate) const LOGO_SIZE: f32 = 56.;
pub(crate) const REPOSITORY: &str = "https://github.com/jR4dh3y/BoxBox";
/// From `.github/FUNDING.yml`.
pub(crate) const SPONSOR: &str = "https://github.com/sponsors/jR4dh3y";
pub(crate) const ISSUES: &str = "https://github.com/jR4dh3y/BoxBox/issues";

pub(crate) struct AboutInfo {
    /// The BoxBox logo at device pixels.
    pub logo: Option<std::sync::Arc<gpui::RenderImage>>,
    /// When the running binary was written, in milliseconds since the epoch.
    pub installed: Option<i64>,
    /// `PRETTY_NAME` from os-release.
    pub system: Option<String>,
}

/// `v0.3.2` stays `0.3.2`; a build past the tag, `v0.3.2-169-g094620b`, reads
/// `0.3.2 (dev, 094620b)`, and a bare commit stays as it is.
pub(crate) fn version_label(described: &str) -> String {
    let (described, dirty) = match described.strip_suffix("-dirty") {
        Some(rest) => (rest, ", modified"),
        None => (described, ""),
    };
    let mut parts = described.rsplitn(3, '-');
    let (hash, ahead, tag) = (parts.next(), parts.next(), parts.next());
    match (tag, ahead, hash.and_then(|hash| hash.strip_prefix('g'))) {
        (Some(tag), Some(ahead), Some(hash)) if ahead.chars().all(|ch| ch.is_ascii_digit()) => {
            format!("{} (dev, {hash}{dirty})", tag.trim_start_matches('v'))
        }
        _ if dirty.is_empty() => described.trim_start_matches('v').to_string(),
        _ => format!("{} (modified)", described.trim_start_matches('v')),
    }
}

/// `PRETTY_NAME`, unquoted, from os-release text.
fn pretty_name(os_release: &str) -> Option<String> {
    os_release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|name| !name.is_empty())
}

fn read(logo_pixels: u32) -> AboutInfo {
    let logo = crate::file_icons::svg_image(crate::assets::LOGO, logo_pixels).map_err(|error| log::warn!("BoxBox logo: {error:#}")).ok();
    let installed = std::env::current_exe()
        .and_then(std::fs::metadata)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(crate::api::mod_time);
    // os-release(5): /etc first, then the vendor's copy.
    let system = ["/etc/os-release", "/usr/lib/os-release"].iter().find_map(|path| std::fs::read_to_string(path).ok()).and_then(|text| pretty_name(&text));
    AboutInfo { logo, installed, system }
}

impl Workspace {
    pub(crate) fn load_about(&mut self, cx: &mut Context<Self>) {
        if self.about.is_some() {
            return;
        }
        self.about = Some(Load::Loading);
        let logo_pixels = (LOGO_SIZE * self.file_icons.scale.max(1.)).ceil() as u32;
        cx.spawn(async move |this, cx| {
            let info = cx.background_executor().spawn(async move { read(logo_pixels) }).await;
            this.update(cx, |this, cx| {
                this.about = Some(Load::Loaded(info));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_read_as_a_release_or_a_dev_build() {
        assert_eq!(version_label("v0.3.2"), "0.3.2");
        assert_eq!(version_label("v0.3.2-169-g094620b"), "0.3.2 (dev, 094620b)");
        assert_eq!(version_label("v0.3.2-169-g094620b-dirty"), "0.3.2 (dev, 094620b, modified)");
        assert_eq!(version_label("094620b"), "094620b");
    }

    #[test]
    fn the_system_is_os_release_pretty_name() {
        assert_eq!(pretty_name("NAME=\"CachyOS\"\nPRETTY_NAME=\"CachyOS Linux\"\nID=cachyos\n").as_deref(), Some("CachyOS Linux"));
        assert_eq!(pretty_name("NAME=Arch\n"), None);
    }
}
