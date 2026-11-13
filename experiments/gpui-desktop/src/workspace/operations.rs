use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, bail, Context as _};
use gpui::{Context, PromptLevel, Window};

use crate::os_clipboard;
use crate::api::{Api, FileInfo};
use super::preview::ThumbnailKey;
use super::*;


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PaneTarget(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone)]
pub(crate) struct ClipboardEntry {
    pub api: Arc<Api>,
    pub item: FileInfo,
}

#[derive(Clone)]
pub(crate) struct FileClipboard {
    pub mode: ClipboardMode,
    pub entries: Vec<ClipboardEntry>,
}

pub(crate) enum OperationDialog {
    CreateFolder { target_id: usize, parent: String },
    CreateFile { target_id: usize, parent: String },
    Rename { target_id: usize, path: String },
}

impl OperationDialog {
    pub fn title(&self) -> &'static str {
        match self {
            Self::CreateFolder { .. } => "New folder",
            Self::CreateFile { .. } => "New file",
            Self::Rename { .. } => "Rename",
        }
    }

    pub fn placeholder(&self) -> &'static str {
        match self {
            Self::CreateFolder { .. } => "New folder",
            Self::CreateFile { .. } => "untitled.txt",
            Self::Rename { .. } => "",
        }
    }

    pub fn confirm_label(&self) -> &'static str {
        match self {
            Self::CreateFolder { .. } | Self::CreateFile { .. } => "Create",
            Self::Rename { .. } => "Rename",
        }
    }
}

/// Whether an operation is running. Its outcome is shown as a notice (`report`).
#[derive(Default)]
pub(crate) enum OperationStatus {
    #[default]
    Idle,
    Running(String),
}

impl OperationStatus {
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Running(_))
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Idle => None,
            Self::Running(message) => Some(message),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ContextMenuState {
    pub target_id: usize,
    pub items: Vec<FileInfo>,
    pub position: gpui::Point<gpui::Pixels>,
}

#[derive(Clone)]
pub(crate) struct LocationEditor {
    pub target_id: usize,
}

#[derive(Clone)]
pub(crate) struct PropertiesState {
    pub item: FileInfo,
    pub provider_label: String,
}
impl Workspace {
    pub(crate) fn begin_operation(&mut self, label: impl Into<String>) -> u64 {
        self.mutation_generation = self.mutation_generation.wrapping_add(1);
        self.operation_status = OperationStatus::Running(label.into());
        self.mutation_generation
    }

    fn finish_operation(
        &mut self,
        generation: u64,
        result: anyhow::Result<String>,
        cx: &mut Context<Self>,
    ) {
        if self.mutation_generation != generation {
            return;
        }
        self.report(result.map_err(|error| format!("{error:#}")), cx);
    }

    pub(crate) fn set_operation_error(
        &mut self,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.report(Err(message.into()), cx);
    }

    fn operation_target(&self, target: PaneTarget) -> Option<(&Pane, Arc<Api>, String)> {
        self.pane_by_id(target.0)
            .map(|pane| (pane, pane.api.clone(), pane.path.clone()))
    }

    pub(crate) fn create_folder(
        &mut self,
        _: &CreateFolder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let pane = self.pane();
        if pane.path.is_empty() {
            self.set_operation_error("Choose a folder before creating a new folder.", cx);
            return;
        }
        self.open_dialog(OperationDialog::CreateFolder { target_id: pane.id, parent: pane.path.clone() }, "", 0..0, window, cx);
    }

    pub(crate) fn create_file(
        &mut self,
        _: &CreateFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let pane = self.pane();
        if pane.path.is_empty() {
            self.set_operation_error("Choose a folder before creating a new file.", cx);
            return;
        }
        self.open_dialog(OperationDialog::CreateFile { target_id: pane.id, parent: pane.path.clone() }, "", 0..0, window, cx);
    }

    pub(crate) fn rename_selected(
        &mut self,
        _: &RenameSelected,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let pane = self.pane();
        let items = pane.selected_items();
        if items.len() != 1 {
            self.set_operation_error("Select exactly one item to rename.", cx);
            return;
        }
        self.start_rename(pane.id, items.into_iter().next().unwrap(), window, cx);
    }

    fn start_rename(
        &mut self,
        target_id: usize,
        item: FileInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Select the name without its extension, as file managers do.
        let stem = match item.name.rfind('.') {
            Some(dot) if dot > 0 && !item.is_dir => dot,
            _ => item.name.len(),
        };
        let name = item.name.clone();
        self.open_dialog(OperationDialog::Rename { target_id, path: item.path }, &name, 0..stem, window, cx);
    }

    fn open_dialog(
        &mut self,
        dialog: OperationDialog,
        text: &str,
        selection: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.operation_dialog = Some(dialog);
        self.dialog_error = None;
        self.context_menu = None;
        let text = text.to_string();
        self.dialog_field.update(cx, |field, cx| field.set_text(text, selection, cx));
        self.dialog_field.read(cx).focus(window);
        cx.notify();
    }

    /// Enter and Escape in the dialog's name field.
    pub(crate) fn on_dialog_field(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Submit => {
                self.submit_operation_dialog(cx);
                if self.operation_dialog.is_none() {
                    self.focus_handle.focus(window);
                }
            }
            InputEvent::Cancel => self.close_dialog(window, cx),
            InputEvent::Changed => {}
        }
    }

    pub(crate) fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation_dialog.take().is_some() {
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    /// Enter and Escape in the location field.
    pub(crate) fn on_location_field(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Submit => {
                self.submit_location(window, cx);
                if self.operation_status.is_busy() {
                    self.focus_handle.focus(window);
                }
            }
            InputEvent::Cancel => {
                self.location_editor = None;
                self.focus_handle.focus(window);
                cx.notify();
            }
            InputEvent::Changed => {}
        }
    }
    fn submit_operation_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.operation_dialog.take() else {
            return;
        };
        let input = self.dialog_field.read(cx).text().trim().to_string();
        if input.is_empty()
            || input == "."
            || input == ".."
            || input.chars().any(|ch| matches!(ch, '/' | '\\' | '\0'))
        {
            self.operation_dialog = Some(dialog);
            self.dialog_error = Some("Enter a valid file or folder name.".into());
            cx.notify();
            return;
        }
        let (target_id, parent, kind, path) = match dialog {
            OperationDialog::CreateFolder { target_id, parent, .. } => {
                (target_id, parent, "folder", None)
            }
            OperationDialog::CreateFile { target_id, parent, .. } => {
                (target_id, parent, "file", None)
            }
            OperationDialog::Rename { target_id, path, .. } => {
                let parent = parent_path(&path).to_string();
                (target_id, parent, "rename", Some(path))
            }
        };
        let Some((_, api, _)) = self.operation_target(PaneTarget(target_id)) else {
            self.set_operation_error("The destination pane is no longer open.", cx);
            return;
        };
        let generation = self.begin_operation(match kind {
            "folder" => format!("Creating {input}…"),
            "file" => format!("Creating {input}…"),
            _ => format!("Renaming to {input}…"),
        });
        let refresh_api = api.clone();
        let renamed_path = (kind == "rename").then(|| destination_path(&parent, &input));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match (kind, path) {
                        ("folder", None) => api
                            .create_directory(&parent, &input)
                            .map(|_| format!("Created folder “{input}”")),
                        ("file", None) => api
                            .create_file(&parent, &input, "")
                            .map(|_| format!("Created file “{input}”")),
                        (_, Some(path)) => api
                            .rename(&path, &destination_path(&parent, &input))
                            .map(|_| format!("Renamed to “{input}”")),
                        _ => bail!("invalid operation"),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.operation_dialog = None;
                if result.is_ok() {
                    if let Some(path) = renamed_path {
                        if let Some(pane) = this.pane_mut_by_id(target_id) {
                            pane.selected.clear(); pane.selected.insert(path);
                        }
                    }
                }
                this.refresh_provider(refresh_api, cx);
                this.finish_operation(generation, result, cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn operation_escape(
        &mut self,
        _: &OperationEscape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        // Close the topmost thing: a dialog, the location field, the preview, then a menu.
        if self.operation_dialog.take().is_none() && self.location_editor.take().is_none() && self.preview.take().is_none() {
            self.context_menu = None;
        }
        self.focus_handle.focus(window);
        cx.notify();
    }

    pub(crate) fn focus_location(
        &mut self,
        _: &FocusLocation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane();
        let target_id = pane.id;
        let path = pane.path.clone();
        let length = path.len();
        self.location_editor = Some(LocationEditor { target_id });
        self.location_field.update(cx, |field, cx| field.set_text(path, 0..length, cx));
        self.location_field.read(cx).focus(window);
        cx.notify();
    }

    fn submit_location(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.location_editor.as_ref() else {
            return;
        };
        let path = self.location_field.read(cx).text().trim().to_string();
        let target = PaneTarget(editor.target_id);
        if path.contains('\0') || path.split('/').any(|part| part == "..") {
            self.set_operation_error("Enter a valid provider path.", cx);
            return;
        }
        let Some((_, api, _)) = self.operation_target(target) else {
            self.location_editor = None;
            self.set_operation_error("The destination pane is no longer open.", cx);
            return;
        };
        let generation = self.begin_operation(format!("Opening {path}…"));
        let validation_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if !path.is_empty() {
                        validation_api
                            .list(&path)
                            .with_context(|| format!("cannot open {path}"))?;
                    }
                    Ok(path)
                })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(path) if this.mutation_generation == generation => {
                    this.location_editor = None;
                    this.navigate_in(target, path, cx);
                    this.finish_operation(generation, Ok("Location opened".into()), cx);
                }
                Err(error) => {
                    this.finish_operation(generation, Err(error), cx);
                }
                _ => {}
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn copy_selected(
        &mut self,
        _: &CopySelected,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.active_pane;
        let items = self.pane().selected_items();
        self.copy_items(target, items, ClipboardMode::Copy, cx);
    }

    pub(crate) fn cut_selected(
        &mut self,
        _: &CutSelected,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.active_pane;
        let items = self.pane().selected_items();
        self.copy_items(target, items, ClipboardMode::Cut, cx);
    }

    fn copy_items(
        &mut self,
        target: PaneTarget,
        items: Vec<FileInfo>,
        mode: ClipboardMode,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        if items.is_empty() {
            self.set_operation_error("Select at least one item first.", cx);
            return;
        }
        if api.is_local() {
            self.publish_files(api.clone(), &items, mode, cx);
        }
        self.file_clipboard = Some(FileClipboard {
            mode,
            entries: items
                .into_iter()
                .map(|item| ClipboardEntry {
                    api: api.clone(),
                    item,
                })
                .collect(),
        });
        self.report(Ok(match mode {
            ClipboardMode::Copy => "Items copied".into(),
            ClipboardMode::Cut => "Items marked to move".into(),
        }), cx);
        self.context_menu = None;
        cx.notify();
    }

    /// Puts local items on the system clipboard, so other apps can paste them.
    fn publish_files(&mut self, api: Arc<Api>, items: &[FileInfo], mode: ClipboardMode, cx: &mut Context<Self>) {
        let virtual_paths: Vec<String> = items.iter().map(|item| item.path.clone()).collect();
        let op = match mode {
            ClipboardMode::Copy => os_clipboard::FileOp::Copy,
            ClipboardMode::Cut => os_clipboard::FileOp::Cut,
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let paths = virtual_paths
                        .iter()
                        .map(|path| api.local_path(path)?.with_context(|| format!("{path} is not on this computer")))
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    let files = os_clipboard::Files { op, paths };
                    os_clipboard::publish(&files)?;
                    Ok::<_, anyhow::Error>(files)
                })
                .await;
            this.update(cx, |this, _| match result {
                Ok(files) => this.published_files = Some(files),
                // Copy and paste inside BoxBox still work.
                Err(error) => log::warn!("Files were not put on the system clipboard: {error:#}"),
            })
            .ok();
        })
        .detach();
    }

    /// Pastes what another app copied when the system clipboard holds its files;
    /// otherwise BoxBox's own clipboard.
    pub(crate) fn paste_into(&mut self, target: PaneTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let system = cx.background_executor().spawn(async move { os_clipboard::read() }).await;
            cx.update_window(window_handle, |_, window, cx| {
                this.update(cx, |this, cx| match system {
                    Ok(Some(files)) if this.published_files.as_ref() != Some(&files) => this.paste_system_files(target, files, cx),
                    Ok(_) => this.paste_in_app(target, window, cx),
                    Err(error) => {
                        log::warn!("Could not read the system clipboard: {error:#}");
                        this.paste_in_app(target, window, cx)
                    }
                })
                .ok();
            })
            .ok();
        })
        .detach();
    }

    /// Files another app copied. Paths this provider can reach go through the normal
    /// transfer, so a cut moves them; anything else is copied in.
    fn paste_system_files(&mut self, target: PaneTarget, files: os_clipboard::Files, cx: &mut Context<Self>) {
        let Some((_, destination, parent)) = self.operation_target(target) else {
            return;
        };
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before pasting.", cx);
            return;
        }
        let reachable: Option<Vec<String>> = files.paths.iter().map(|path| destination.virtual_path(path)).collect();
        let Some(virtual_paths) = reachable else {
            self.import_paths_to(destination, parent, files.paths, cx);
            return;
        };
        let mode = match files.op {
            os_clipboard::FileOp::Copy => ClipboardMode::Copy,
            os_clipboard::FileOp::Cut => ClipboardMode::Cut,
        };
        let count = virtual_paths.len();
        let api = destination.clone();
        cx.spawn(async move |this, cx| {
            let entries = cx
                .background_executor()
                .spawn(async move {
                    virtual_paths
                        .iter()
                        .map(|path| api.stat(path).map(|item| ClipboardEntry { api: api.clone(), item }))
                        .collect::<anyhow::Result<Vec<_>>>()
                })
                .await;
            this.update(cx, |this, cx| match entries {
                Ok(entries) => this.run_paste(target, FileClipboard { mode, entries }, destination, parent, count, cx),
                Err(error) => this.report(Err(format!("Could not paste: {error:#}")), cx),
            })
            .ok();
        })
        .detach();
    }

    fn paste_in_app(&mut self, target: PaneTarget, window: &mut Window, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        let Some(clipboard) = self.file_clipboard.clone() else {
            self.set_operation_error("The file clipboard is empty.", cx);
            return;
        };
        let Some((_, destination, parent)) = self.operation_target(target) else {
            return;
        };
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before pasting.", cx);
            return;
        }
        let requires_confirmation = clipboard.mode == ClipboardMode::Cut
            && clipboard.entries.iter().any(|entry| {
                !entry.api.is_local() && !same_provider(&entry.api, &destination)
            });
        if requires_confirmation {
            let count = clipboard.entries.len();
            self.operation_status =
                OperationStatus::Running("Waiting for remote move confirmation…".into());
            cx.notify();
            let answer = window.prompt(
                PromptLevel::Warning,
                "Move from a remote provider?",
                Some("This copies the items, then permanently deletes the remote originals."),
                &["Move", "Cancel"],
                cx,
            );
            cx.spawn(async move |this, cx| {
                let confirmed = answer.await.unwrap_or(1) == 0;
                let _ = this.update(cx, |this, cx| {
                    this.operation_status = OperationStatus::Idle;
                    if confirmed {
                        this.run_paste(target, clipboard, destination, parent, count, cx);
                    } else {
                        cx.notify();
                    }
                });
            })
            .detach();
        } else {
            let count = clipboard.entries.len();
            self.run_paste(target, clipboard, destination, parent, count, cx);
        }
    }

    fn run_paste(
        &mut self,
        target: PaneTarget,
        clipboard: FileClipboard,
        destination: Arc<Api>,
        parent: String,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let generation = self.begin_operation(format!("Transferring {count} item(s)…"));
        let mut affected = Vec::<Arc<Api>>::new();
        for entry in &clipboard.entries {
            if !affected.iter().any(|api| same_provider(api, &entry.api)) {
                affected.push(entry.api.clone());
            }
        }
        if !affected.iter().any(|api| same_provider(api, &destination)) {
            affected.push(destination.clone());
        }
        let refresh = affected.clone();
        let transfer = clipboard.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    transfer_clipboard(&transfer, &destination, &parent)?;
                    Ok(format!("Transferred {count} item(s)"))
                })
                .await;
            let _ = this.update(cx, move |this, cx| {
                for api in refresh {
                    this.refresh_provider(api, cx);
                }
                if result.is_ok() && clipboard.mode == ClipboardMode::Cut {
                    this.file_clipboard = None;
                }
                this.finish_operation(generation, result, cx);
                if this.pane_by_id(target.0).is_none() {
                    this.report(Err("The destination pane was closed during the transfer.".into()), cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn paste_clipboard(
        &mut self,
        _: &PasteInto,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.paste_into(self.active_pane, window, cx);
    }

    pub(crate) fn delete_selected(
        &mut self,
        _: &DeleteSelected,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.active_pane;
        let items = self.pane().selected_items();
        self.delete_items(target, items, window, cx);
    }

    pub(crate) fn delete_items(
        &mut self,
        target: PaneTarget,
        items: Vec<FileInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        if items.is_empty() {
            self.set_operation_error("Select at least one item first.", cx);
            return;
        }
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        if api.is_local() && !self.preferences.confirm_delete && items.iter().all(|item| !item.is_dir) {
            self.run_delete(target, api, items, cx);
            return;
        }
        let title = if api.is_local() {
            "Move to Trash?"
        } else {
            "Delete permanently?"
        };
        let message = if api.is_local() {
            "The selected items will be moved to Trash."
        } else {
            "This permanently deletes the selected remote items."
        };
        self.operation_status = OperationStatus::Running("Waiting for delete confirmation…".into());
        cx.notify();
        let answer = window.prompt(
            PromptLevel::Warning,
            title,
            Some(message),
            &[if api.is_local() { "Move to Trash" } else { "Delete permanently" }, "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            let confirmed = answer.await.unwrap_or(1) == 0;
            let _ = this.update(cx, |this, cx| {
                this.operation_status = OperationStatus::Idle;
                if confirmed {
                    this.run_delete(target, api, items, cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn run_delete(
        &mut self,
        target: PaneTarget,
        api: Arc<Api>,
        items: Vec<FileInfo>,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let count = items.len();
        let generation = self.begin_operation(if api.is_local() {
            format!("Moving {count} item(s) to Trash…")
        } else {
            format!("Deleting {count} remote item(s)…")
        });
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let is_local = api.is_local();
            let result = cx
                .background_executor()
                .spawn(async move {
                    for item in &items {
                        if is_local {
                            api.move_to_trash(&item.path)?;
                        } else {
                            api.delete(&item.path, true)?;
                        }
                    }
                    Ok(if is_local {
                        format!("Moved {count} item(s) to Trash")
                    } else {
                        format!("Permanently deleted {count} item(s)")
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_provider(refresh_api.clone(), cx);
                if is_local
                    && this
                        .trash_provider
                        .as_ref()
                        .is_some_and(|trash_api| same_provider(trash_api, &refresh_api))
                {
                    this.load_trash(refresh_api, cx);
                }
                if this.pane_by_id(target.0).is_none() && result.is_ok() {
                    this.report(Err("The source pane was closed during deletion.".into()), cx);
                    cx.notify();
                } else {
                    this.finish_operation(generation, result, cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn download_selected(
        &mut self,
        _: &DownloadSelected,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.active_pane;
        let items = self.pane().selected_items();
        self.download_items(target, items, cx);
    }

    pub(crate) fn download_items(
        &mut self,
        target: PaneTarget,
        items: Vec<FileInfo>,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        if items.is_empty() {
            self.set_operation_error("Select at least one item to download.", cx);
            return;
        }
        self.operation_status = OperationStatus::Running("Choose a download folder…".into());
        cx.notify();
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Download to".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = paths.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    this.operation_status = OperationStatus::Idle;
                    this.run_download(api, items, paths[0].clone(), cx);
                }
                Ok(Ok(None)) => {
                    this.operation_status = OperationStatus::Idle;
                    cx.notify();
                }
                Ok(Err(error)) => this.set_operation_error(
                    format!("Could not choose a download folder: {error:#}"),
                    cx,
                ),
                Err(error) => this.set_operation_error(
                    format!("Download folder picker failed: {error}"),
                    cx,
                ),
                _ => this.set_operation_error("No download folder was selected.", cx),
            });
        })
        .detach();
    }

    fn run_download(
        &mut self,
        api: Arc<Api>,
        items: Vec<FileInfo>,
        directory: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let count = items.len();
        let generation = self.begin_operation(format!("Downloading {count} item(s)…"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    for item in &items {
                        api.download(item, &directory)
                            .with_context(|| format!("cannot download {}", item.name))?;
                    }
                    Ok(format!(
                        "Downloaded {count} item(s) to {}",
                        directory.display()
                    ))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.finish_operation(generation, result, cx);
            });
        })
        .detach();
    }

    pub(crate) fn import_with_picker(
        &mut self,
        target: PaneTarget,
        directories: bool,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let Some((_, api, parent)) = self.operation_target(target) else {
            return;
        };
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before importing.", cx);
            return;
        }
        let prompt = if directories { "Import folders" } else { "Import files" };
        self.operation_status = OperationStatus::Running("Choose items to import…".into());
        cx.notify();
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: !directories,
            directories,
            multiple: true,
            prompt: Some(prompt.into()),
        });
        cx.spawn(async move |this, cx| {
            let result = paths.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    this.operation_status = OperationStatus::Idle;
                    this.import_paths_to(api, parent, paths, cx);
                }
                Ok(Ok(None)) => {
                    this.operation_status = OperationStatus::Idle;
                    cx.notify();
                }
                Ok(Err(error)) => this.set_operation_error(
                    format!("Could not choose items to import: {error:#}"),
                    cx,
                ),
                Err(error) => this.set_operation_error(
                    format!("Import file picker failed: {error}"),
                    cx,
                ),
                _ => this.set_operation_error("No items were selected for import.", cx),
            });
        })
        .detach();
    }

    pub(crate) fn import_paths(
        &mut self,
        target: PaneTarget,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() || paths.is_empty() {
            return;
        }
        let Some((_, api, parent)) = self.operation_target(target) else {
            return;
        };
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before importing.", cx);
            return;
        }
        self.import_paths_to(api, parent, paths, cx);
    }

    fn import_paths_to(
        &mut self,
        api: Arc<Api>,
        parent: String,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() || paths.is_empty() {
            return;
        }
        let count = paths.len();
        let generation = self.begin_operation(format!("Importing {count} item(s)…"));
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    import_paths_into(&api, &paths, &parent)?;
                    Ok(format!("Imported {count} item(s)"))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_provider(refresh_api, cx);
                this.finish_operation(generation, result, cx);
            });
        })
        .detach();
    }

    pub(crate) fn add_favorite(
        &mut self,
        _: &AddFavorite,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane();
        if pane.path.is_empty() {
            self.set_operation_error("The provider root cannot be added as a favorite.", cx);
            return;
        }
        let provider = FavoriteProvider::for_api(&pane.api);
        let connection_id = self
            .desktop_settings
            .connections
            .iter()
            .find(|connection| {
                pane.api
                    .server_url()
                    .is_some_and(|url| url == connection.url)
            })
            .map(|connection| connection.id.clone());
        let favorite = Favorite {
            name: pane.path.rsplit('/').next().unwrap_or(&pane.path).to_string(),
            path: pane.path.clone(),
            provider,
            connection_id,
        };
        self.favorites.add(favorite);
        self.save_favorites(Some("Favorite added"), cx);
        cx.notify();
    }

    pub(crate) fn remove_favorite(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(favorite) = self.favorites.entries.get(index).cloned() else {
            return;
        };
        self.favorites.remove(&favorite);
        self.save_favorites(None, cx);
        cx.notify();
    }

    pub(crate) fn open_favorite(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(favorite) = self.favorites.entries.get(index).cloned() else {
            return;
        };
        match favorite.provider {
            FavoriteProvider::Local => {
                let local_id = self
                    .tabs
                    .iter()
                    .chain(self.split_panes.values())
                    .find(|pane| pane.api.is_local())
                    .map(|pane| pane.id);
                let target = if let Some(id) = local_id {
                    self.activate_pane_id(id, cx);
                    PaneTarget(id)
                } else {
                    match Api::local() {
                        Ok(api) => {
                            self.open_provider(api, cx);
                            self.active_pane
                        }
                        Err(error) => {
                            self.set_operation_error(
                                format!("Cannot open local favorite: {error:#}"),
                                cx,
                            );
                            return;
                        }
                    }
                };
                self.navigate_in(target, favorite.path, cx);
            }
            FavoriteProvider::Remote(url) => {
                let target = self
                    .tabs
                    .iter()
                    .chain(self.split_panes.values())
                    .find(|pane| pane.api.server_url() == Some(url.as_str()))
                    .map(|pane| pane.id);
                if let Some(id) = target {
                    self.activate_pane_id(id, cx);
                    self.navigate_in(PaneTarget(id), favorite.path, cx);
                } else {
                    self.open_provider(Api::new(&url, None), cx);
                    let target = self.active_pane;
                    self.navigate_in(target, favorite.path, cx);
                }
            }
        }
    }

    fn activate_pane_id(&mut self, id: usize, cx: &mut Context<Self>) {
        let tab_index = self
            .tabs
            .iter()
            .position(|pane| pane.id == id)
            .or_else(|| {
                self.split_panes
                    .iter()
                    .find_map(|(primary, pane)| (pane.id == id).then_some(*primary))
                    .and_then(|primary| self.tabs.iter().position(|pane| pane.id == primary))
            });
        if let Some(tab_index) = tab_index {
            self.active_tab = tab_index;
            self.active_pane = PaneTarget(id);
            self.trash_open = false;
            self.context_menu = None;
            cx.notify();
        }
    }

    pub(crate) fn show_item_context_menu(
        &mut self,
        target: PaneTarget,
        index: usize,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.pane_mut_by_id(target.0) else {
            return;
        };
        if !pane.items.get(index).is_some_and(|item| pane.is_selected(&item.path)) {
            pane.click(index, gpui::Modifiers::default());
        }
        let items = pane.selected_items();
        self.active_pane = target;
        self.context_menu = Some(ContextMenuState {
            target_id: target.0,
            items,
            position,
        });
        cx.notify();
    }

    fn context_selection(&self) -> Option<(PaneTarget, Vec<FileInfo>)> {
        self.context_menu
            .as_ref()
            .map(|menu| (PaneTarget(menu.target_id), menu.items.clone()))
    }

    pub(crate) fn context_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        if let Some(item) = items.into_iter().next() {
            self.preview_item(target, item, window, cx);
        }
    }

    pub(crate) fn context_copy(&mut self, cx: &mut Context<Self>) {
        if let Some((target, items)) = self.context_selection() {
            self.copy_items(target, items, ClipboardMode::Copy, cx);
        }
    }

    pub(crate) fn context_favorite_index(&self) -> Option<usize> {
        let (target, items) = self.context_selection()?;
        let [item] = items.as_slice() else { return None; };
        let provider = FavoriteProvider::for_api(&self.pane_by_id(target.0)?.api);
        self.favorites.entries.iter().position(|favorite| favorite.path == item.path && favorite.provider == provider)
    }

    pub(crate) fn context_toggle_favorite(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.context_favorite_index() {
            self.context_menu = None;
            self.remove_favorite(index, cx);
            return;
        }
        let Some((target, items)) = self.context_selection() else { return; };
        let [item] = items.as_slice() else { return; };
        if !item.is_dir { return; }
        let Some(pane) = self.pane_by_id(target.0) else { return; };
        let provider = FavoriteProvider::for_api(&pane.api);
        let connection_id = self.desktop_settings.connections.iter()
            .find(|connection| pane.api.server_url() == Some(connection.url.as_str()))
            .map(|connection| connection.id.clone());
        self.favorites.add(Favorite { name: item.name.clone(), path: item.path.clone(), provider, connection_id });
        self.save_favorites(None, cx);
        self.context_menu = None;
        cx.notify();
    }

    pub(crate) fn context_cut(&mut self, cx: &mut Context<Self>) {
        if let Some((target, items)) = self.context_selection() {
            self.copy_items(target, items, ClipboardMode::Cut, cx);
        }
    }

    pub(crate) fn context_paste(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .context_menu
            .as_ref()
            .map(|menu| PaneTarget(menu.target_id))
            .unwrap_or(self.active_pane);
        self.context_menu = None;
        self.paste_into(target, window, cx);
    }

    pub(crate) fn context_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        if items.len() == 1 {
            self.start_rename(target.0, items.into_iter().next().unwrap(), window, cx);
        } else {
            self.set_operation_error("Select exactly one item to rename.", cx);
        }
    }

    pub(crate) fn context_delete(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        self.delete_items(target, items, window, cx);
    }

    pub(crate) fn context_download(&mut self, cx: &mut Context<Self>) {
        if let Some((target, items)) = self.context_selection() {
            self.context_menu = None;
            self.download_items(target, items, cx);
        }
    }

    pub(crate) fn context_properties(&mut self, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        let Some(item) = items.into_iter().next() else {
            return;
        };
        let provider_label = self
            .pane_by_id(target.0)
            .map(|pane| {
                pane.api
                    .server_url()
                    .unwrap_or("Local storage")
                    .to_string()
            })
            .unwrap_or_else(|| "Unknown provider".into());
        self.properties = Some(PropertiesState {
            item,
            provider_label,
        });
        cx.notify();
    }

    pub(crate) fn context_share(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else { return; };
        let [item] = items.as_slice() else { return; };
        let Some(pane) = self.pane_by_id(target.0) else { return; };
        if pane.api.is_local() { return; }
        let path = item.path.clone();
        self.active_pane = target;
        self.context_menu = None;
        self.settings_open = true;
        self.settings_section = SettingsSection::Links;
        self.open_create_share(path, cx);
        let _ = window;
    }

    pub(crate) fn context_create_folder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.context_menu.as_ref().map(|menu| PaneTarget(menu.target_id));
        self.context_menu = None;
        if let Some(target) = target {
            self.active_pane = target;
        }
        self.create_folder(&CreateFolder, window, cx);
    }

    pub(crate) fn context_create_file(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.context_menu.as_ref().map(|menu| PaneTarget(menu.target_id));
        self.context_menu = None;
        if let Some(target) = target {
            self.active_pane = target;
        }
        self.create_file(&CreateFile, window, cx);
    }

    pub(crate) fn context_open_terminal(&mut self, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        let Some(item) = items.into_iter().next() else {
            return;
        };
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        let generation = self.begin_operation("Opening terminal…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let path = api
                        .local_path(&item.path)?
                        .ok_or_else(|| anyhow!("Open Terminal is only available for local files."))?;
                    let directory = if path.is_dir() {
                        path
                    } else {
                        path.parent()
                            .map(Path::to_path_buf)
                            .ok_or_else(|| anyhow!("item has no parent directory"))?
                    };
                    open_terminal(&directory)?;
                    Ok(format!("Opened terminal in {}", directory.display()))
                })
                .await;
            let _ = this.update(cx, |this, cx| this.finish_operation(generation, result, cx));
        })
        .detach();
    }

    pub(crate) fn activate_item(
        &mut self,
        target: PaneTarget,
        item: FileInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if item.is_dir {
            self.navigate_in(target, item.path, cx);
        } else if self.preferences.preview_on_single_click {
            self.preview_item(target, item, window, cx);
        } else {
            self.open_item(target, item, cx);
        }
    }

    pub(crate) fn open_item(&mut self, target: PaneTarget, item: FileInfo, cx: &mut Context<Self>) {
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        let generation = self.begin_operation(format!("Opening {}…", item.name));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::media::open_file(&api, &item)
                        .map(|path| format!("Opened {} ({})", item.name, path.display()))
                })
                .await;
            let _ = this.update(cx, |this, cx| this.finish_operation(generation, result, cx));
        })
        .detach();
    }

    pub(crate) fn preview_item(
        &mut self,
        target: PaneTarget,
        item: FileInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        if item.is_dir {
            self.set_operation_error("Folders cannot be previewed.", cx);
            return;
        }
        self.context_menu = None;
        self.mutation_generation = self.mutation_generation.wrapping_add(1);
        let generation = self.mutation_generation;
        let text_preview = is_text_preview(&item.name);
        let media_preview = has_inline_thumbnail(&item.name);
        let content = if text_preview || media_preview {
            PreviewContent::Loading
        } else {
            PreviewContent::Failed("No in-app preview is available for this file type.".into())
        };
        self.preview = Some(PreviewState {
            target_id: target.0,
            api: api.clone(),
            item: item.clone(),
            content,
        });
        self.preview_focus.focus(window);
        cx.notify();

        if text_preview {
            let read_api = api.clone();
            let read_item = item.clone();
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { crate::media::read_text_preview(&read_api, &read_item) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.mutation_generation == generation {
                        if let Some(preview) = this.preview.as_mut().filter(|preview| {
                            preview.target_id == target.0
                                && preview.item.path == item.path
                                && Arc::ptr_eq(&preview.api, &api)
                        }) {
                            preview.content = match result {
                                Ok(text) => PreviewContent::Text(text.into()),
                                Err(error) => PreviewContent::Failed(format!("{error:#}")),
                            };
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        } else if media_preview {
            let thumbnail_api = api.clone();
            let thumbnail_item = item.clone();
            cx.spawn(async move |this, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { crate::media::thumbnail(&thumbnail_api, &thumbnail_item, 512, 512) })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.mutation_generation == generation {
                        if let Some(preview) = this.preview.as_mut().filter(|preview| {
                            preview.target_id == target.0
                                && preview.item.path == item.path
                                && Arc::ptr_eq(&preview.api, &api)
                        }) {
                            preview.content = match result {
                                Ok(bytes) => PreviewContent::Image(Arc::new(
                                    gpui::Image::from_bytes(gpui::ImageFormat::Png, bytes),
                                )),
                                Err(error) => PreviewContent::Failed(format!("{error:#}")),
                            };
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub(crate) fn request_thumbnail(
        &mut self,
        target: PaneTarget,
        item: FileInfo,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.pane_by_id(target.0) else {
            return;
        };
        if !has_inline_thumbnail(&item.name) {
            return;
        }
        let api = pane.api.clone();
        let key = ThumbnailKey::new(&api, &item);
        let Some(generation) = self.thumbnail_cache.request(key.clone()) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::media::thumbnail(&api, &item, 160, 120) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.thumbnail_cache.finish(key, generation, result);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn thumbnail(
        &self,
        api: &Arc<Api>,
        item: &FileInfo,
    ) -> Option<Arc<gpui::Image>> {
        let key = ThumbnailKey::new(api, item);
        self.thumbnail_cache.get(&key)
    }


    pub(crate) fn open_active(
        &mut self,
        _: &OpenActive,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.pane().active_item() {
            self.activate_item(self.active_pane, item, window, cx);
        }
    }

    pub(crate) fn preview_active(
        &mut self,
        _: &PreviewActive,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.pane().active_item() {
            self.preview_item(self.active_pane, item, window, cx);
        }
    }
}

fn parent_path(path: &str) -> &str {
    path.rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("")
}

fn destination_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}
fn same_provider(left: &Api, right: &Api) -> bool {
    FavoriteProvider::for_api(left) == FavoriteProvider::for_api(right)
}


fn transfer_clipboard(
    clipboard: &FileClipboard,
    destination: &Arc<Api>,
    parent: &str,
) -> anyhow::Result<()> {
    if clipboard.entries.is_empty() {
        bail!("The file clipboard is empty");
    }
    if parent.is_empty() {
        bail!("Choose a destination folder before pasting");
    }
    let existing = destination
        .list(parent)
        .with_context(|| format!("cannot inspect destination {parent}"))?;
    let mut names: HashSet<String> = existing.into_iter().map(|item| item.name).collect();
    for entry in &clipboard.entries {
        if !names.insert(entry.item.name.clone()) {
            let same_source_folder = clipboard.mode == ClipboardMode::Cut
                && same_provider(&entry.api, destination)
                && parent_path(&entry.item.path) == parent;
            if !same_source_folder {
                bail!("“{}” already exists in the destination", entry.item.name);
            }
        }
        if entry.item.is_dir
            && same_provider(&entry.api, destination)
            && (parent == entry.item.path
                || parent.starts_with(&format!("{}/", entry.item.path)))
        {
            bail!("A folder cannot be copied or moved into itself");
        }
    }

    let temp_root = create_transfer_temp_dir()?;
    let mut transfer_number = 0usize;
    let mut already_in_folder = 0usize;
    for entry in &clipboard.entries {
        if clipboard.mode == ClipboardMode::Cut
            && same_provider(&entry.api, destination)
            && parent_path(&entry.item.path) == parent
        {
            already_in_folder += 1;
            continue;
        }
        if same_provider(&entry.api, destination) {
            match clipboard.mode {
                ClipboardMode::Copy => {
                    destination
                        .copy(
                            &entry.item.path,
                            &destination_path(parent, &entry.item.name),
                        )
                        .with_context(|| format!("cannot copy {}", entry.item.name))?;
                }
                ClipboardMode::Cut => {
                    destination
                        .rename(
                            &entry.item.path,
                            &destination_path(parent, &entry.item.name),
                        )
                        .with_context(|| format!("cannot move {}", entry.item.name))?;
                }
            }
        } else {
            transfer_between_providers(
                &entry.api,
                destination,
                &entry.item,
                parent,
                &temp_root.0,
                &mut transfer_number,
            )?;
        }
    }

    if clipboard.mode == ClipboardMode::Cut {
        for entry in &clipboard.entries {
            if same_provider(&entry.api, destination)
                && parent_path(&entry.item.path) == parent
            {
                continue;
            }
            if !same_provider(&entry.api, destination) {
                if entry.api.is_local() {
                    entry
                        .api
                        .move_to_trash(&entry.item.path)
                        .with_context(|| format!("cannot move {} to Trash", entry.item.name))?;
                } else {
                    entry
                        .api
                        .delete(&entry.item.path, true)
                        .with_context(|| {
                            format!("cannot permanently delete {}", entry.item.name)
                        })?;
                }
            }
        }
    }
    if already_in_folder == clipboard.entries.len() {
        bail!("All selected items are already in this folder");
    }
    Ok(())
}

fn transfer_between_providers(
    source: &Arc<Api>,
    destination: &Arc<Api>,
    item: &FileInfo,
    parent: &str,
    temp_root: &Path,
    transfer_number: &mut usize,
) -> anyhow::Result<()> {
    if item.is_dir {
        destination
            .create_directory(parent, &item.name)
            .with_context(|| format!("cannot create folder {}", item.name))?;
        let child_parent = destination_path(parent, &item.name);
        let children = source
            .list(&item.path)
            .with_context(|| format!("cannot list source folder {}", item.path))?;
        for child in children {
            transfer_between_providers(
                source,
                destination,
                &child,
                &child_parent,
                temp_root,
                transfer_number,
            )?;
        }
    } else if let Some(local_path) = source
        .local_path(&item.path)
        .with_context(|| format!("cannot resolve source {}", item.path))?
    {
        destination
            .import_file(&local_path, parent)
            .with_context(|| format!("cannot import {}", item.name))?;
    } else {
        let transfer_dir = temp_root.join(transfer_number.to_string());
        *transfer_number += 1;
        fs::create_dir(&transfer_dir)?;
        let downloaded = source
            .download(item, &transfer_dir)
            .with_context(|| format!("cannot download {}", item.name))?;
        if !downloaded.starts_with(&transfer_dir) || !downloaded.is_file() {
            bail!("provider returned an unexpected download path for {}", item.name);
        }
        destination
            .import_file(&downloaded, parent)
            .with_context(|| format!("cannot import {}", item.name))?;
        let _ = fs::remove_dir_all(transfer_dir);
    }
    Ok(())
}

fn import_paths_into(api: &Arc<Api>, paths: &[PathBuf], parent: &str) -> anyhow::Result<()> {
    let existing = api.list(parent)?;
    let mut names: HashSet<String> = existing.into_iter().map(|item| item.name).collect();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty() && *name != "." && *name != "..")
            .ok_or_else(|| anyhow!("{} has no valid file name", path.display()))?
            .to_string();
        if !names.insert(name.clone()) {
            bail!("“{name}” already exists in the destination");
        }
    }
    for path in paths {
        import_path_recursive(api, path, parent)?;
    }
    Ok(())
}

fn import_path_recursive(api: &Arc<Api>, path: &Path, parent: &str) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("cannot inspect {}", path.display()))?;
    let file_type = metadata.file_type();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("{} has no valid file name", path.display()))?;
    if file_type.is_symlink() {
        bail!("symbolic links are not imported: {}", path.display());
    }
    if file_type.is_dir() {
        api.create_directory(parent, name)
            .with_context(|| format!("cannot create folder {name}"))?;
        let child_parent = destination_path(parent, name);
        let entries = fs::read_dir(path)
            .with_context(|| format!("cannot read folder {}", path.display()))?;
        for entry in entries {
            let entry = entry?;
            import_path_recursive(api, &entry.path(), &child_parent)?;
        }
    } else if file_type.is_file() {
        api.import_file(path, parent)
            .with_context(|| format!("cannot import {}", path.display()))?;
    } else {
        bail!("unsupported item type: {}", path.display());
    }
    Ok(())
}

struct TransferTempDir(PathBuf);

impl Drop for TransferTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_transfer_temp_dir() -> anyhow::Result<TransferTempDir> {
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
    for _ in 0..100 {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "boxbox-transfer-{}-{id}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(TransferTempDir(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("could not allocate a temporary transfer directory")
}

fn open_terminal(directory: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let candidates: [(&str, &[&str]); 4] = [
            ("kgx", &["--working-directory"]),
            ("gnome-terminal", &["--working-directory"]),
            ("konsole", &["--workdir"]),
            ("xfce4-terminal", &["--working-directory"]),
        ];
        for (program, args) in candidates {
            let mut command = Command::new(program);
            for arg in args {
                command.arg(arg);
            }
            command.arg(directory);
            if command.spawn().is_ok() {
                return Ok(());
            }
        }
        bail!("no supported terminal application was found")
    }
    #[cfg(not(target_os = "linux"))]
    {
        let terminal = std::env::var_os("TERMINAL")
            .ok_or_else(|| anyhow!("set TERMINAL to use Open Terminal"))?;
        Command::new(terminal)
            .current_dir(directory)
            .spawn()
            .context("could not open terminal")?;
        Ok(())
    }
}
