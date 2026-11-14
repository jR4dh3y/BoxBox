//! Grid thumbnails (`FileThumbnail.svelte`, `utils/thumbnail.ts`) and the preview overlay
//! (`FilePreview.svelte`). GPUI has no media or PDF viewer, so video, audio and documents show
//! their details with "Open natively", and PDFs show their first page as an image.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, AnyElement, Context, FocusHandle, Image, ImageFormat, Window,
};

use crate::api::{encode_query, FileInfo};
use crate::format::{extension, file_size, type_description};
use crate::theme::*;
use crate::ui::{icon, TextScale};
use crate::workspace::Workspace;

const MAX_IN_FLIGHT: usize = 4;
/// Text previews read at most this much; the rest is noted as truncated.
const TEXT_LIMIT: usize = 2 * 1024 * 1024;
const IMAGE_LIMIT: u64 = 64 * 1024 * 1024;

const RASTER: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "ico", "tiff", "tif", "qoi",
];
const VIDEO: &[&str] = &[
    "mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v", "ogv",
];
const AUDIO: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus"];
const OFFICE: &[&str] = &[
    "xls", "xlsx", "xlsm", "xlsb", "xlt", "xltx", "xltm", "ods", "ots", "doc", "docx", "docm",
    "dot", "dotx", "dotm", "odt", "ott", "rtf", "ppt", "pptx", "pptm", "pot", "potx", "potm",
    "pps", "ppsx", "ppsm", "odp", "otp",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Image,
    Video,
    Audio,
    Pdf,
    Text,
    Office,
    Notebook,
}

/// `getPreviewType`: by extension; code and plain text both preview as text.
pub fn preview_kind(name: &str) -> Option<PreviewKind> {
    let ext = extension(name);
    let ext = ext.as_str();
    Some(match ext {
        _ if RASTER.contains(&ext) || matches!(ext, "svg" | "avif" | "raw") => PreviewKind::Image,
        _ if VIDEO.contains(&ext) => PreviewKind::Video,
        _ if AUDIO.contains(&ext) => PreviewKind::Audio,
        "pdf" => PreviewKind::Pdf,
        "ipynb" => PreviewKind::Notebook,
        _ if OFFICE.contains(&ext) => PreviewKind::Office,
        _ => match type_description(name).as_str() {
            "Text File" | "Log File" | "CSV" | "TSV" => PreviewKind::Text,
            _ if crate::format::file_icon(name, false) == "file-code"
                || crate::format::file_icon(name, false) == "file-json" =>
            {
                PreviewKind::Text
            }
            _ => return None,
        },
    })
}

/// `supportsThumbnailFile` for local files: raster images, video, audio and PDF.
pub fn supports_thumbnail(name: &str) -> bool {
    let ext = extension(name);
    RASTER.contains(&ext.as_str())
        || VIDEO.contains(&ext.as_str())
        || AUDIO.contains(&ext.as_str())
        || ext == "pdf"
}

fn thumb_key(item: &FileInfo) -> String {
    format!("{}\u{0}{}\u{0}{}", item.path, item.mod_time, item.size)
}

pub enum Thumb {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

#[derive(Default)]
pub struct Thumbnails {
    cache: HashMap<String, Thumb>,
    queue: VecDeque<FileInfo>,
    in_flight: usize,
}

pub enum PreviewContent {
    Loading,
    Image(Arc<Image>),
    Text {
        text: String,
        truncated: bool,
    },
    /// Shown as details with "Open natively".
    External,
    Failed(String),
}

pub struct Preview {
    pub item: FileInfo,
    pub kind: PreviewKind,
    pub content: PreviewContent,
    pub zoom: f32,
    pub focus: FocusHandle,
}

fn sniff(bytes: &[u8]) -> ImageFormat {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => ImageFormat::Png,
        [b'G', b'I', b'F', ..] => ImageFormat::Gif,
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => ImageFormat::Webp,
        [b'B', b'M', ..] => ImageFormat::Bmp,
        [b'I', b'I', ..] | [b'M', b'M', ..] => ImageFormat::Tiff,
        _ if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") => ImageFormat::Svg,
        _ => ImageFormat::Jpeg,
    }
}

impl Workspace {
    pub(crate) fn thumbnail(&self, item: &FileInfo) -> Option<Arc<Image>> {
        match self.thumbs.cache.get(&thumb_key(item)) {
            Some(Thumb::Ready(image)) => Some(image.clone()),
            _ => None,
        }
    }

    /// Queues thumbnails for a pane's grid; already cached or failed ones are skipped.
    pub(crate) fn request_thumbnails(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(pane_id) else {
            return;
        };
        if pane.current_view_mode() != crate::pane::ViewMode::Grid {
            return;
        }
        let wanted: Vec<FileInfo> = pane
            .items
            .iter()
            .filter(|item| !item.is_dir && supports_thumbnail(&item.name))
            .take(1000)
            .cloned()
            .collect();
        for item in wanted {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.thumbs.cache.entry(thumb_key(&item))
            {
                entry.insert(Thumb::Loading);
                self.thumbs.queue.push_back(item);
            }
        }
        self.pump_thumbnails(cx);
    }

    fn pump_thumbnails(&mut self, cx: &mut Context<Self>) {
        while self.thumbs.in_flight < MAX_IN_FLIGHT {
            let Some(item) = self.thumbs.queue.pop_front() else {
                break;
            };
            self.thumbs.in_flight += 1;
            let key = thumb_key(&item);
            let version = encode_query(&format!(
                "[{:?},{:?},{}]",
                item.path, item.mod_time, item.size
            ));
            self.background(
                move |api| {
                    api.bytes(
                        &format!(
                            "/stream/thumbnail/{}?width=224&height=144&v={version}",
                            crate::api::encode_path(&item.path)
                        ),
                        u64::MAX,
                    )
                },
                move |this, result, cx| {
                    this.thumbs.in_flight -= 1;
                    let thumb = match result {
                        Ok(bytes) => {
                            Thumb::Ready(Arc::new(Image::from_bytes(sniff(&bytes), bytes)))
                        }
                        Err(_) => Thumb::Failed,
                    };
                    this.thumbs.cache.insert(key, thumb);
                    this.pump_thumbnails(cx);
                },
                cx,
            );
        }
    }

    pub(crate) fn open_preview(
        &mut self,
        item: FileInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(kind) = preview_kind(&item.name) else {
            self.toast(
                crate::notifications::ToastKind::Info,
                format!("{} cannot be previewed", item.name),
                cx,
            );
            return;
        };
        let focus = cx.focus_handle();
        window.focus(&focus);
        self.preview = Some(Preview {
            item: item.clone(),
            kind,
            content: PreviewContent::Loading,
            zoom: 1.,
            focus,
        });
        let path = item.path.clone();
        let encoded = crate::api::encode_path(&path);
        match kind {
            PreviewKind::Image if item.size <= IMAGE_LIMIT => self.background(
                move |api| api.bytes(&format!("/stream/preview/{encoded}"), IMAGE_LIMIT),
                move |this, result, _| {
                    this.set_preview(
                        &path,
                        result.map(|bytes| {
                            PreviewContent::Image(Arc::new(Image::from_bytes(sniff(&bytes), bytes)))
                        }),
                    )
                },
                cx,
            ),
            PreviewKind::Pdf => self.background(
                move |api| {
                    api.bytes(
                        &format!("/stream/thumbnail/{encoded}?width=512&height=512"),
                        u64::MAX,
                    )
                },
                move |this, result, _| {
                    let content = match result {
                        Ok(bytes) => {
                            PreviewContent::Image(Arc::new(Image::from_bytes(sniff(&bytes), bytes)))
                        }
                        Err(_) => PreviewContent::External,
                    };
                    this.set_preview(&path, Ok(content));
                },
                cx,
            ),
            PreviewKind::Text => self.background(
                move |api| api.bytes(&format!("/stream/preview/{encoded}"), TEXT_LIMIT as u64 + 1),
                move |this, result, _| {
                    this.set_preview(
                        &path,
                        result.map(|bytes| {
                            let truncated = bytes.len() > TEXT_LIMIT;
                            let text =
                                String::from_utf8_lossy(&bytes[..bytes.len().min(TEXT_LIMIT)])
                                    .into_owned();
                            PreviewContent::Text { text, truncated }
                        }),
                    )
                },
                cx,
            ),
            _ => self.set_preview(&path, Ok(PreviewContent::External)),
        }
        cx.notify();
    }

    fn set_preview(&mut self, path: &str, content: anyhow::Result<PreviewContent>) {
        if let Some(preview) = self
            .preview
            .as_mut()
            .filter(|preview| preview.item.path == path)
        {
            preview.content =
                content.unwrap_or_else(|error| PreviewContent::Failed(format!("{error:#}")));
        }
    }

    pub(crate) fn close_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    /// ←/→ over previewable files in the current order.
    pub(crate) fn step_preview(
        &mut self,
        step: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self
            .preview
            .as_ref()
            .map(|preview| preview.item.path.clone())
        else {
            return;
        };
        let files: Vec<FileInfo> = self
            .pane()
            .items
            .iter()
            .filter(|item| !item.is_dir && preview_kind(&item.name).is_some())
            .cloned()
            .collect();
        let Some(index) = files.iter().position(|item| item.path == current) else {
            return;
        };
        let next = index as isize + step;
        if let Some(item) = usize::try_from(next)
            .ok()
            .and_then(|next| files.get(next))
            .cloned()
        {
            self.pane_mut().select_path(&item.path);
            self.open_preview(item, window, cx);
        }
    }

    pub(crate) fn render_preview(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let preview = self.preview.as_ref()?;
        let header_button =
            |id: &'static str,
             glyph: &'static str,
             cx: &mut Context<Self>,
             action: fn(&mut Workspace, &mut Window, &mut Context<Workspace>)| {
                div()
                    .id(id)
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                    .child(icon(glyph, 16., c(TEXT_SECONDARY)))
                    .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
            };
        let item = preview.item.clone();
        let body: AnyElement = match &preview.content {
            PreviewContent::Loading => div()
                .text_color(c(TEXT_MUTED))
                .child("Loading preview…")
                .into_any_element(),
            PreviewContent::Failed(error) => div()
                .text_color(c(DANGER))
                .child(error.clone())
                .into_any_element(),
            PreviewContent::Image(image) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(
                    img(image.clone())
                        .max_w(gpui::relative(preview.zoom))
                        .max_h(gpui::relative(preview.zoom))
                        .object_fit(gpui::ObjectFit::Contain),
                )
                .into_any_element(),
            PreviewContent::Text { text, truncated } => div()
                .id("preview-text")
                .size_full()
                .overflow_scroll()
                .p(px(16.))
                .font_family(self.fonts.mono.clone())
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(c(TEXT_PRIMARY))
                .child(text.clone())
                .when(*truncated, |text| {
                    text.child(
                        div()
                            .pt(px(12.))
                            .text_color(c(TEXT_MUTED))
                            .child("Preview truncated at 2 MB."),
                    )
                })
                .into_any_element(),
            PreviewContent::External => {
                let label = match preview.kind {
                    PreviewKind::Video => "Video",
                    PreviewKind::Audio => "Audio",
                    PreviewKind::Office => "Document",
                    PreviewKind::Notebook => "Notebook",
                    _ => "File",
                };
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(12.))
                    .text_13()
                    .child(icon(
                        crate::format::file_icon(&item.name, false),
                        48.,
                        c(TEXT_SECONDARY),
                    ))
                    .child(div().text_color(c(TEXT_PRIMARY)).child(item.name.clone()))
                    .child(
                        div()
                            .text_color(c(TEXT_MUTED))
                            .child(format!("{label} · {}", file_size(item.size))),
                    )
                    .into_any_element()
            }
        };
        let open_item = item.clone();
        Some(
            div()
                .id("preview")
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .bg(c(0x121212))
                .occlude()
                .key_context("Preview")
                .track_focus(&preview.focus)
                .on_action(
                    cx.listener(|this, _: &crate::workspace::ClosePreview, window, cx| {
                        this.close_preview(window, cx)
                    }),
                )
                .on_action(cx.listener(
                    |this, _: &crate::workspace::PreviewPrevious, window, cx| {
                        this.step_preview(-1, window, cx)
                    },
                ))
                .on_action(
                    cx.listener(|this, _: &crate::workspace::PreviewNext, window, cx| {
                        this.step_preview(1, window, cx)
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .h(px(48.))
                        .items_center()
                        .gap(px(8.))
                        .px(px(12.))
                        .bg(c(SURFACE_SECONDARY))
                        .border_b_1()
                        .border_color(c(BORDER_PRIMARY))
                        .child(header_button(
                            "preview-prev",
                            "chevron-left",
                            cx,
                            |this, window, cx| this.step_preview(-1, window, cx),
                        ))
                        .child(header_button(
                            "preview-next",
                            "chevron-right",
                            cx,
                            |this, window, cx| this.step_preview(1, window, cx),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm_tw()
                                .child(item.name.clone()),
                        )
                        .when(
                            matches!(preview.content, PreviewContent::Image(_)),
                            |header| {
                                header
                                    .child(header_button(
                                        "zoom-out",
                                        "zoom-out",
                                        cx,
                                        |this, _, cx| this.zoom_preview(-0.25, cx),
                                    ))
                                    .child(
                                        div()
                                            .w(px(44.))
                                            .text_center()
                                            .text_xs_tw()
                                            .text_color(c(TEXT_SECONDARY))
                                            .child(format!("{}%", (preview.zoom * 100.).round())),
                                    )
                                    .child(header_button(
                                        "zoom-in",
                                        "zoom-in",
                                        cx,
                                        |this, _, cx| this.zoom_preview(0.25, cx),
                                    ))
                            },
                        )
                        .child(
                            div()
                                .id("preview-open")
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .h(px(30.))
                                .px(px(10.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_xs_tw()
                                .text_color(c(TEXT_SECONDARY))
                                .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                                .child(icon("external-link", 14., c(TEXT_SECONDARY)))
                                .child("Open natively")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_item(open_item.clone(), cx)
                                })),
                        )
                        .child(header_button(
                            "preview-close",
                            "x",
                            cx,
                            |this, window, cx| this.close_preview(window, cx),
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .p(px(16.))
                        .child(body),
                )
                .into_any_element(),
        )
    }

    fn zoom_preview(&mut self, step: f32, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.zoom = (preview.zoom + step).clamp(0.25, 5.);
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_follow_extensions() {
        assert_eq!(preview_kind("a.PNG"), Some(PreviewKind::Image));
        assert_eq!(preview_kind("clip.mkv"), Some(PreviewKind::Video));
        assert_eq!(preview_kind("main.rs"), Some(PreviewKind::Text));
        assert_eq!(preview_kind("notes.txt"), Some(PreviewKind::Text));
        assert_eq!(preview_kind("deck.pptx"), Some(PreviewKind::Office));
        assert_eq!(preview_kind("archive.zip"), None);
        assert!(supports_thumbnail("song.flac") && !supports_thumbnail("vector.svg"));
    }

    #[test]
    fn sniffs_common_formats() {
        assert_eq!(sniff(b"\x89PNG\r\n"), ImageFormat::Png);
        assert_eq!(sniff(b"\xff\xd8\xff"), ImageFormat::Jpeg);
    }
}
