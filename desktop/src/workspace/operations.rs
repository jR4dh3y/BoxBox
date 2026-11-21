use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, bail, Context as _};
use gpui::{Context, Window};

use crate::os_clipboard;
use super::confirm::Confirm;
use super::undo::Undo;
use crate::api::{Api, FileInfo};
use super::thumbnails::ThumbnailKey;
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

impl FileClipboard {
    /// Whether both hold the same items for the same operation.
    pub fn same_items(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.entries.len() == other.entries.len()
            && self.entries.iter().zip(&other.entries).all(|(left, right)| Arc::ptr_eq(&left.api, &right.api) && left.item.path == right.item.path)
    }
}

/// The one-field dialog: a name for something new in a folder, or a pattern
/// to select by.
pub(crate) struct OperationDialog {
    pub target_id: usize,
    pub kind: DialogKind,
}

pub(crate) enum DialogKind {
    New { parent: String, folder: bool },
    SelectPattern,
    /// The name shown for a drive; `own` is the drive's own name.
    RenameDrive { key: super::drive_names::DriveKey, own: String },
}

impl OperationDialog {
    pub fn title(&self) -> &'static str {
        match self.kind {
            DialogKind::New { folder: true, .. } => "New folder",
            DialogKind::New { folder: false, .. } => "New file",
            DialogKind::SelectPattern => "Select by Pattern",
            DialogKind::RenameDrive { .. } => "Rename Drive",
        }
    }

    pub fn confirm_label(&self) -> &'static str {
        match self.kind {
            DialogKind::New { .. } => "Create",
            DialogKind::SelectPattern => "Select",
            DialogKind::RenameDrive { .. } => "Rename",
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

pub(crate) struct ContextMenuState {
    pub target_id: usize,
    pub items: Vec<FileInfo>,
    pub position: gpui::Point<gpui::Pixels>,
    /// Open With and custom actions.
    #[cfg(target_os = "linux")]
    pub extras: super::menu_extras::MenuExtras,
    /// The Shell's own commands for the items or folder.
    #[cfg(windows)]
    pub windows: super::windows_menu::WindowsMenu,
}

impl ContextMenuState {
    fn new(target_id: usize, items: Vec<FileInfo>, position: gpui::Point<gpui::Pixels>) -> Self {
        Self {
            target_id,
            items,
            position,
            #[cfg(target_os = "linux")]
            extras: Default::default(),
            #[cfg(windows)]
            windows: Default::default(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct LocationEditor {
    pub target_id: usize,
    /// The folder it was opened on. It belongs to that folder, so it closes
    /// when the pane goes elsewhere, by Back, the sidebar or anything else.
    place: (Arc<Api>, String),
}

impl LocationEditor {
    pub(crate) fn on(pane: &crate::pane::Pane) -> Self {
        Self { target_id: pane.id, place: (pane.api.clone(), pane.path.clone()) }
    }

    pub(crate) fn still_on(&self, pane: &crate::pane::Pane) -> bool {
        Arc::ptr_eq(&self.place.0, &pane.api) && self.place.1 == pane.path
    }
}

impl Workspace {
    pub(crate) fn begin_operation(&mut self, label: impl Into<String>) -> u64 {
        self.mutation_generation = self.mutation_generation.wrapping_add(1);
        self.operation_status = OperationStatus::Running(label.into());
        self.mutation_generation
    }

    pub(crate) fn finish_operation(
        &mut self,
        generation: u64,
        result: anyhow::Result<()>,
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

    /// Whether nothing may be created, renamed or removed in the pane's folder:
    /// The Overview, a folder without write permission, or a read-only server mount.
    pub(crate) fn read_only(&self, target: PaneTarget) -> bool {
        let Some(pane) = self.pane_by_id(target.0) else { return true };
        let root = pane.path.split('/').next().unwrap_or("");
        let read_only_drive = match &pane.drives {
            Load::Loaded(drives) => drives.iter().any(|drive| drive.name == root && drive.read_only),
            _ => false,
        };
        pane.is_at_root() || !pane.writable || read_only_drive
    }

    /// Says so and returns false when the pane's folder cannot be changed.
    pub(crate) fn ensure_writable(&mut self, target: PaneTarget, cx: &mut Context<Self>) -> bool {
        if !self.read_only(target) {
            return true;
        }
        self.notify(crate::components::ToastKind::Info, "This location is read-only or unavailable", cx);
        false
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
        if !self.ensure_writable(self.active_pane, cx) {
            return;
        }
        let pane = self.pane();
        self.open_dialog(OperationDialog { target_id: pane.id, kind: DialogKind::New { parent: pane.path.clone(), folder: true } }, window, cx);
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
        if !self.ensure_writable(self.active_pane, cx) {
            return;
        }
        let pane = self.pane();
        self.open_dialog(OperationDialog { target_id: pane.id, kind: DialogKind::New { parent: pane.path.clone(), folder: false } }, window, cx);
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
        let mut items = self.pane().selected_items();
        if items.len() != 1 {
            self.set_operation_error("Select exactly one item to rename.", cx);
            return;
        }
        if !self.ensure_writable(self.active_pane, cx) {
            return;
        }
        self.start_rename(self.active_pane, items.remove(0), window, cx);
    }

    /// Opens the dialog: a new item's field holds a free name with the part before
    /// the extension selected; the pattern field holds the last pattern, selected.
    pub(super) fn open_dialog(&mut self, dialog: OperationDialog, window: &mut Window, cx: &mut Context<Self>) {
        let (text, selection) = match &dialog.kind {
            DialogKind::New { folder, .. } => {
                let taken: HashSet<String> = self.pane_by_id(dialog.target_id).map(|pane| pane.items.iter().map(|item| item.name.clone()).collect()).unwrap_or_default();
                let suggestion = if *folder { "New folder" } else { "untitled.txt" };
                let text = super::names::unused_name(suggestion, *folder, &|name| taken.contains(name));
                let end = super::names::basename_end(&text, *folder);
                (text, 0..end)
            }
            DialogKind::SelectPattern => {
                let text = self.last_pattern.clone();
                let end = text.len();
                (text, 0..end)
            }
            DialogKind::RenameDrive { key, own } => {
                let text = self.drive_names.name(key, own).to_string();
                let end = text.len();
                (text, 0..end)
            }
        };
        self.operation_dialog = Some(dialog);
        self.dialog_error = None;
        self.context_menu = None;
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
                self.location_completion = None;
                self.submit_location(window, cx);
                if self.operation_status.is_busy() {
                    self.focus_handle.focus(window);
                }
            }
            InputEvent::Cancel => {
                self.location_editor = None;
                self.location_completion = None;
                self.focus_handle.focus(window);
                cx.notify();
            }
            // The folders that fit, once typing pauses.
            InputEvent::Changed => self.refresh_location_completion(std::time::Duration::from_millis(80), window, cx, None),
        }
    }
    /// Ctrl+S: asks for a pattern, then selects the entries it matches.
    pub(crate) fn open_select_pattern(&mut self, _: &SelectPattern, window: &mut Window, cx: &mut Context<Self>) {
        if self.trash_open || self.pane().is_at_root() {
            return;
        }
        self.open_dialog(OperationDialog { target_id: self.active_pane.0, kind: DialogKind::SelectPattern }, window, cx);
    }

    /// Ctrl+I.
    pub(crate) fn invert_selection(&mut self, _: &InvertSelection, _: &mut Window, cx: &mut Context<Self>) {
        if self.trash_open || self.pane().is_at_root() {
            return;
        }
        self.pane_mut().invert_selection();
        cx.notify();
    }

    /// Selects what `pattern` matches; a pattern that matches nothing keeps the
    /// dialog open to be changed.
    fn select_pattern(&mut self, dialog: OperationDialog, pattern: String, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_mut_by_id(dialog.target_id) else { return };
        if pane.select_matching(|name| crate::search::matches_pattern(&pattern, name)) == 0 {
            self.dialog_error = Some(if pattern.is_empty() { "Type a pattern, such as *.jpg".into() } else { format!("Nothing here matches “{pattern}”") });
            self.operation_dialog = Some(dialog);
        } else {
            self.last_pattern = pattern;
        }
        cx.notify();
    }

    fn submit_operation_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.operation_dialog.take() else {
            return;
        };
        let input = self.dialog_field.read(cx).text().trim().to_string();
        if let DialogKind::RenameDrive { key, own } = &dialog.kind {
            // An empty name gives the drive its own back.
            self.rename_drive(key, Some(&input), own, cx);
            return;
        }
        let Some((pane, api, _)) = self.operation_target(PaneTarget(dialog.target_id)) else {
            self.set_operation_error("The destination pane is no longer open.", cx);
            return;
        };
        let (parent, folder) = match &dialog.kind {
            DialogKind::New { parent, folder } => (parent.clone(), *folder),
            DialogKind::SelectPattern => {
                self.select_pattern(dialog, input, cx);
                return;
            }
            DialogKind::RenameDrive { .. } => return,
        };
        if let Some(problem) = super::names::name_problem(&input, pane.items.iter().map(|item| item.name.as_str())) {
            self.operation_dialog = Some(dialog);
            self.dialog_error = Some(problem);
            cx.notify();
            return;
        }
        let generation = self.begin_operation(format!("Creating {input}…"));
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if folder {
                        api.create_directory(&parent, &input).map(|_| ())
                    } else {
                        api.create_file(&parent, &input, "").map(|_| ())
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
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
        // Close the topmost thing: a dialog, the location field, then a menu.
        if self.operation_dialog.take().is_none() && self.location_editor.take().is_none() {
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
        self.edit_location(self.active_pane, window, cx);
    }

    /// Turns a pane's path bar into a field holding its location, all selected
    /// so it copies at once: the real path on this computer, else BoxBox's path.
    pub(crate) fn edit_location(&mut self, target: PaneTarget, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let path = match (&pane.real_path, pane.api.is_local()) {
            (Some(real), true) => real.display().to_string(),
            _ => pane.path.clone(),
        };
        let length = path.len();
        self.location_editor = Some(LocationEditor::on(pane));
        self.location_completion = None;
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
        let local = self.pane_by_id(target.0).is_some_and(|pane| pane.api.is_local());
        let path = if local { super::location_completion::expand_home(&path) } else { path };
        // A network location, such as smb://nas/media, is mounted and then opened.
        if crate::devices::gvfs::spec::is_uri(&path) {
            self.connect_location(target, path, cx);
            return;
        }
        if path.contains('\0') || path.split('/').any(|part| part == "..") {
            self.set_operation_error("Enter a valid provider path.", cx);
            return;
        }
        let Some((_, api, _)) = self.operation_target(target) else {
            self.location_editor = None;
            self.set_operation_error("The destination pane is no longer open.", cx);
            return;
        };
        let Some(view) = self.pane_by_id(target.0).map(|pane| pane.view()) else { return };
        let generation = self.begin_operation(format!("Opening {path}…"));
        let journey = crate::profile::journey("open folder", || format!("{path}  (location)"));
        let validation_api = api.clone();
        cx.spawn(async move |this, cx| {
            // The listing that proves the path opens is the one the pane shows.
            let result = cx
                .background_executor()
                .spawn(async move {
                    if path.is_empty() {
                        return Ok((path, None));
                    }
                    // A real path, as the field shows on this computer, or a file:// link.
                    let path = if validation_api.is_local() && (Path::new(&path).is_absolute() || path.starts_with("file://")) {
                        let real = if path.starts_with("file://") {
                            crate::os_clipboard::path_from_uri(&path).context("Invalid file URI")?
                        } else {
                            PathBuf::from(path)
                        };
                        validation_api.virtual_path(&real).context("The location is unavailable or outside the listed drives")?
                    } else {
                        path
                    };
                    let mut read = super::startup::read_listing(&validation_api, &path, view);
                    let items = std::mem::replace(&mut read.items, Ok(Vec::new())).with_context(|| format!("cannot open {path}"))?;
                    read.items = Ok(items);
                    Ok((path, Some(read)))
                })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok((path, read)) if this.mutation_generation == generation => {
                    this.location_editor = None;
                    match read {
                        Some(read) => this.navigate_with(target, path, view, read, journey, cx),
                        None => this.navigate_in(target, path, cx),
                    }
                    this.finish_operation(generation, Ok(()), cx);
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
        // Text selected in the Details panel is what the user means to copy.
        if self.copy_text_selection(cx) {
            return;
        }
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
        if mode == ClipboardMode::Cut && !self.ensure_writable(target, cx) {
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
                        // The entries themselves: a cut link moves the link, not its target.
                        .map(|path| api.entry_path(path)?.with_context(|| format!("{path} is not on this computer")))
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
    pub(crate) fn paste_into(&mut self, target: PaneTarget, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() || !self.ensure_writable(target, cx) {
            return;
        }
        let Some(folder) = self.pane_by_id(target.0).map(|pane| pane.path.clone()) else { return };
        self.paste_into_folder(target, folder, cx);
    }

    /// Pastes into `folder` on `target`'s provider, which need not be the folder it shows.
    pub(crate) fn paste_into_folder(&mut self, target: PaneTarget, folder: String, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let system = cx.background_executor().spawn(async move { os_clipboard::read() }).await;
            this.update(cx, |this, cx| match system {
                Ok(Some(files)) if this.published_files.as_ref() != Some(&files) => this.paste_system_files(target, folder, files, cx),
                Ok(_) => this.paste_in_app(target, folder, cx),
                Err(error) => {
                    log::warn!("Could not read the system clipboard: {error:#}");
                    this.paste_in_app(target, folder, cx)
                }
            })
            .ok();
        })
        .detach();
    }

    fn paste_system_files(&mut self, target: PaneTarget, parent: String, files: os_clipboard::Files, cx: &mut Context<Self>) {
        let Some((_, destination, _)) = self.operation_target(target) else {
            return;
        };
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before pasting.", cx);
            return;
        }
        self.receive_files(target, destination, parent, files, cx);
    }

    /// Files from another app, pasted or dropped. Paths this provider can reach go
    /// through the normal transfer, so a cut moves them; anything else is copied in.
    pub(crate) fn receive_files(&mut self, target: PaneTarget, destination: Arc<Api>, parent: String, files: os_clipboard::Files, cx: &mut Context<Self>) {
        let mode = match files.op {
            os_clipboard::FileOp::Copy => ClipboardMode::Copy,
            os_clipboard::FileOp::Cut => ClipboardMode::Cut,
        };
        let (api, paths) = (destination.clone(), files.paths.clone());
        cx.spawn(async move |this, cx| {
            // Off the window thread: resolving a path can wait on a slow network share.
            let entries = cx
                .background_executor()
                .spawn(async move {
                    // The entries themselves: cutting a link moves the link, not what it points to.
                    let virtual_paths = paths.iter().map(|path| api.virtual_entry_path(path)).collect::<Option<Vec<_>>>()?;
                    Some(
                        virtual_paths
                            .iter()
                            .map(|path| api.stat(path).map(|item| ClipboardEntry { api: api.clone(), item }))
                            .collect::<anyhow::Result<Vec<_>>>(),
                    )
                })
                .await;
            this.update(cx, |this, cx| match entries {
                Some(Ok(entries)) => this.run_paste(target, FileClipboard { mode, entries }, destination, parent, cx),
                Some(Err(error)) => this.report(Err(format!("Could not paste: {error:#}")), cx),
                // Not reachable through this provider: copied in instead.
                None => this.import_paths_to(destination, parent, files.paths, cx),
            })
            .ok();
        })
        .detach();
    }

    fn paste_in_app(&mut self, target: PaneTarget, parent: String, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        let Some(clipboard) = self.file_clipboard.clone() else {
            self.notify(crate::components::ToastKind::Info, "The clipboard has no files to paste", cx);
            return;
        };
        let Some((_, destination, _)) = self.operation_target(target) else {
            return;
        };
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before pasting.", cx);
            return;
        }
        self.start_transfer(target, clipboard, destination, parent, cx);
    }

    /// Copies or moves `clipboard` into `parent`, asking first when a move would
    /// delete originals on a remote provider. Paste and drag and drop both end here.
    pub(crate) fn start_transfer(
        &mut self,
        target: PaneTarget,
        clipboard: FileClipboard,
        destination: Arc<Api>,
        parent: String,
        cx: &mut Context<Self>,
    ) {
        let requires_confirmation = clipboard.mode == ClipboardMode::Cut
            && clipboard.entries.iter().any(|entry| {
                !entry.api.is_local() && !same_provider(&entry.api, &destination)
            });
        if requires_confirmation {
            let confirm = Confirm::new(
                "Move from a remote provider?",
                "This copies the items, then permanently deletes the remote originals.",
                "Move",
                move |this, cx| this.run_paste(target, clipboard, destination, parent, cx),
            );
            self.ask(confirm, cx);
        } else {
            self.run_paste(target, clipboard, destination, parent, cx);
        }
    }

    pub(crate) fn paste_clipboard(
        &mut self,
        _: &PasteInto,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.paste_into(self.active_pane, cx);
    }

    pub(crate) fn delete_selected(
        &mut self,
        _: &DeleteSelected,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // In Trash, Delete can only mean for good; the folder behind it is not on screen.
        if self.trash_open {
            return self.delete_trash_items(self.selected_trash(), cx);
        }
        let target = self.active_pane;
        let items = self.pane().selected_items();
        self.delete_items(target, items, false, cx);
    }

    /// Shift+Delete: gone without passing through Trash, after asking.
    pub(crate) fn delete_permanently(&mut self, _: &DeletePermanently, _window: &mut Window, cx: &mut Context<Self>) {
        if self.trash_open {
            return self.delete_trash_items(self.selected_trash(), cx);
        }
        let target = self.active_pane;
        let items = self.pane().selected_items();
        self.delete_items(target, items, true, cx);
    }

    /// Moves `items` to Trash, or deletes them for good when `permanent` or on a
    /// server, which has no Trash.
    pub(crate) fn delete_items(
        &mut self,
        target: PaneTarget,
        items: Vec<FileInfo>,
        permanent: bool,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        if items.is_empty() {
            self.set_operation_error("Select at least one item first.", cx);
            return;
        }
        if !self.ensure_writable(target, cx) {
            return;
        }
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        let to_trash = api.is_local() && !permanent;
        // Only a move to Trash, which can be undone, may skip the question.
        if to_trash && !self.preferences.confirm_delete && items.iter().all(|item| !item.is_dir) {
            self.run_delete(target, api, items, false, cx);
            return;
        }
        // `BrowseDialogs.svelte`: one name in the sentence, or a count and the names.
        let fate = if to_trash { "will be moved to Trash." } else { "will be deleted permanently. This cannot be undone." };
        let (message, names) = match items.as_slice() {
            [item] => (format!("“{}” {fate}", item.name), Vec::new()),
            items => (format!("{} items {fate}", items.len()), items.iter().map(|item| item.name.clone().into()).collect()),
        };
        let (title, action) = if to_trash { ("Move to Trash", "Move to Trash") } else { ("Delete Permanently", "Delete") };
        let confirm = Confirm::new(title, message, action, move |this, cx| this.run_delete(target, api, items, permanent, cx))
        .items(names);
        self.ask(confirm, cx);
    }

    pub(crate) fn run_delete(
        &mut self,
        target: PaneTarget,
        api: Arc<Api>,
        items: Vec<FileInfo>,
        permanent: bool,
        cx: &mut Context<Self>,
    ) {
        if self.operation_status.is_busy() {
            return;
        }
        let count = items.len();
        let to_trash = api.is_local() && !permanent;
        let generation = self.begin_operation(if to_trash {
            format!("Moving {count} item(s) to Trash…")
        } else {
            format!("Deleting {count} item(s)…")
        });
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let is_local = api.is_local();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut trashed = Vec::new();
                    for item in &items {
                        if to_trash {
                            trashed.push(api.move_to_trash(&item.path)?.id);
                        } else {
                            api.delete(&item.path, true)?;
                        }
                    }
                    Ok(trashed)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                let result = result.map(|trashed| {
                    if !trashed.is_empty() {
                        this.last_undo = Some(Undo::Trash { api: refresh_api.clone(), ids: trashed.clone() });
                        // The items leave the view, so say where they went and offer them back.
                        let message = if count == 1 { "Moved 1 item to Trash".to_string() } else { format!("Moved {count} items to Trash") };
                        let api = refresh_api.clone();
                        this.notify_with_action(crate::components::ToastKind::Success, message, "Undo", move |this, cx| this.undo_trash(api.clone(), trashed.clone(), cx), cx);
                    }
                });
                this.refresh_provider(refresh_api.clone(), cx);
                if is_local {
                    this.refresh_trash_state(cx);
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
                    Ok(directory)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                // The files land outside this window, so say where.
                if let Ok(directory) = &result {
                    let message = format!("Downloaded {count} item{} to {}", if count == 1 { "" } else { "s" }, directory.display());
                    this.notify(crate::components::ToastKind::Success, message, cx);
                }
                this.finish_operation(generation, result.map(|_| ()), cx);
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
        let title = match paths.as_slice() {
            [path] => path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            paths => format!("{} items", paths.len()),
        };
        let folder = self.folder_label(&api, &parent);
        let (job, progress) = self.start_job(title, folder, false, cx);
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    progress.scope(|| {
                        paths.iter().for_each(|path| crate::progress::measure(path));
                        import_paths_into(&api, &paths, &parent)
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_provider(refresh_api, cx);
                this.finish_transfer(job, generation, result, cx);
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
        let (api, path) = (pane.api.clone(), pane.path.clone());
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        self.pin_folder(&api, name, path, cx);
        self.save_favorites(true, cx);
    }

    /// Adds a folder of `api` to Favorites; the caller saves them.
    pub(crate) fn pin_folder(&mut self, api: &Api, name: String, path: String, cx: &mut Context<Self>) {
        let connection_id = self
            .desktop_settings
            .connections
            .iter()
            .find(|connection| api.server_url() == Some(connection.url.as_str()))
            .map(|connection| connection.id.clone());
        self.favorites.add(Favorite { name, path, provider: FavoriteProvider::for_api(api), connection_id, uri: None });
        cx.notify();
    }

    pub(crate) fn remove_favorite(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(favorite) = self.favorites.entries.get(index).cloned() else {
            return;
        };
        self.favorites.remove(&favorite);
        self.save_favorites(false, cx);
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
                    return;
                }
                // The saved connection carries the credential; a bare address has none.
                let normalized = Api::new(&url, None).server_url().map(str::to_string);
                let connection = self.desktop_settings.connections.iter().find(|connection| {
                    favorite.connection_id.as_deref() == Some(connection.id.as_str())
                        || Api::new(&connection.url, None).server_url().map(str::to_string) == normalized
                });
                match connection.cloned() {
                    // Opens in this pane, as a server drive in the sidebar does.
                    Some(connection) => self.open_server_drive_when_ready(connection, Some(favorite.path), cx),
                    None => self.report(Err(format!("{url} is no longer a saved server")), cx),
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

    /// The menu for the folder a pane shows, which clears the selection first.
    pub(crate) fn show_folder_context_menu(&mut self, target: PaneTarget, position: gpui::Point<gpui::Pixels>, window: &Window, cx: &mut Context<Self>) {
        if let Some(pane) = self.pane_mut_by_id(target.0) {
            pane.clear_selection();
        }
        self.open_submenu = None;
        self.refresh_templates(cx);
        self.open_context_menu(ContextMenuState::new(target.0, Vec::new(), position), window, cx);
    }

    pub(crate) fn show_item_context_menu(
        &mut self,
        target: PaneTarget,
        index: usize,
        position: gpui::Point<gpui::Pixels>,
        window: &Window,
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
        self.open_submenu = None;
        self.open_context_menu(ContextMenuState::new(target.0, items, position), window, cx);
    }

    /// Shows `menu` and starts reading what the system adds to it.
    fn open_context_menu(&mut self, menu: ContextMenuState, window: &Window, cx: &mut Context<Self>) {
        self.context_menu = Some(menu);
        #[cfg(target_os = "linux")]
        self.load_menu_extras(window, cx);
        #[cfg(windows)]
        self.load_windows_context_menu(window, cx);
        cx.notify();
    }

    pub(crate) fn context_selection(&self) -> Option<(PaneTarget, Vec<FileInfo>)> {
        self.context_menu
            .as_ref()
            .map(|menu| (PaneTarget(menu.target_id), menu.items.clone()))
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
        let Some(api) = self.pane_by_id(target.0).map(|pane| pane.api.clone()) else { return; };
        self.pin_folder(&api, item.name.clone(), item.path.clone(), cx);
        self.save_favorites(false, cx);
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .context_menu
            .as_ref()
            .map(|menu| PaneTarget(menu.target_id))
            .unwrap_or(self.active_pane);
        self.context_menu = None;
        self.paste_into(target, cx);
    }

    pub(crate) fn context_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        if items.len() == 1 {
            if self.ensure_writable(target, cx) {
                self.start_rename(target, items.into_iter().next().unwrap(), window, cx);
            }
        } else {
            self.set_operation_error("Select exactly one item to rename.", cx);
        }
    }

    pub(crate) fn context_delete(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((target, items)) = self.context_selection() else {
            return;
        };
        self.context_menu = None;
        self.delete_items(target, items, false, cx);
    }

    pub(crate) fn context_download(&mut self, cx: &mut Context<Self>) {
        if let Some((target, items)) = self.context_selection() {
            self.context_menu = None;
            self.download_items(target, items, cx);
        }
    }

    pub(crate) fn context_properties(&mut self, cx: &mut Context<Self>) {
        if let Some((target, items)) = self.context_selection() {
            self.show_properties(target, items, cx);
        }
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

    pub(crate) fn context_open_with(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else { return };
        let [item] = items.as_slice() else { return };
        self.active_pane = target;
        self.open_with_dialog(item.clone(), window, cx);
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
        let Some((_, api, path)) = self.operation_target(target) else {
            return;
        };
        // The menu for empty space opens the folder itself.
        let path = items.into_iter().next().map_or(path, |item| item.path);
        let generation = self.begin_operation("Opening terminal…");
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let path = api
                        .local_path(&path)?
                        .ok_or_else(|| anyhow!("Open Terminal is only available for local files."))?;
                    let directory = if path.is_dir() {
                        path
                    } else {
                        path.parent()
                            .map(Path::to_path_buf)
                            .ok_or_else(|| anyhow!("item has no parent directory"))?
                    };
                    crate::launch::terminal_in(&directory)
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
        cx: &mut Context<Self>,
    ) {
        if item.is_dir {
            self.navigate_in(target, item.path, cx);
        } else {
            self.open_item(target, item, cx);
        }
    }

    /// Opens a file in its app, without asking whether to run it.
    pub(crate) fn open_document(&mut self, target: PaneTarget, item: FileInfo, cx: &mut Context<Self>) {
        let Some((_, api, _)) = self.operation_target(target) else {
            return;
        };
        let generation = self.begin_operation(format!("Opening {}…", item.name));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::media::open_file(&api, &item).map(|_| ())
                })
                .await;
            let _ = this.update(cx, |this, cx| this.finish_operation(generation, result, cx));
        })
        .detach();
    }

    /// Drops every thumbnail, releasing their GPU textures.
    pub(crate) fn clear_thumbnails(&mut self, cx: &mut Context<Self>) {
        for image in self.thumbnail_cache.invalidate() {
            cx.drop_image(image, None);
        }
        self.folder_peeks.clear();
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
        let edge = self.grid_metrics().thumbnail;
        let key = ThumbnailKey::new(&api, &item, edge);
        let Some(generation) = self.thumbnail_cache.request(key.clone()) else {
            return;
        };
        let rendered = crate::thumbnail_queue::render(move || {
            crate::media::thumbnail(&api, &item, edge, edge * 3 / 4).and_then(|png| super::thumbnails::decode_thumbnail(&png))
        });
        cx.spawn(async move |this, cx| {
            let result = rendered.await;
            let _ = this.update(cx, |this, cx| match result {
                Some(result) => {
                    for image in this.thumbnail_cache.finish(key, generation, result) {
                        cx.drop_image(image, None);
                    }
                    cx.notify();
                }
                None => this.thumbnail_cache.forget(&key, generation),
            });
        })
        .detach();
    }

    pub(crate) fn thumbnail(
        &self,
        api: &Arc<Api>,
        item: &FileInfo,
    ) -> Option<Arc<gpui::RenderImage>> {
        let key = ThumbnailKey::new(api, item, self.grid_metrics().thumbnail);
        self.thumbnail_cache.get(&key)
    }


    pub(crate) fn open_active(
        &mut self,
        _: &OpenActive,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.pane().active_item() {
            self.activate_item(self.active_pane, item, cx);
        }
    }
}

fn destination_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

pub(crate) use crate::transfer::same_provider;

/// Drops the paths that already sit directly in `folder`: dropping a file where it is does nothing.
pub(crate) fn outside_folder(folder: &Path, paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|path| path.parent() != Some(folder))
        .collect()
}

fn import_paths_into(api: &Arc<Api>, paths: &[PathBuf], parent: &str) -> anyhow::Result<()> {
    let existing = api.list(parent)?;
    let mut names: HashSet<String> = existing.into_iter().map(|item| item.name).collect();
    // A folder copied into itself would keep finding its own copy inside.
    let destination = api.local_path(parent)?;
    for path in paths {
        if let Some(destination) = &destination {
            let source = fs::canonicalize(path).with_context(|| format!("cannot resolve {}", path.display()))?;
            if destination.starts_with(&source) {
                bail!("“{}” cannot be copied into itself", path.display());
            }
        }
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

#[cfg(test)]
mod transfer_tests {
    use super::*;

    #[test]
    fn a_drop_skips_files_already_in_the_folder() {
        let paths = ["/a/b/here.txt", "/a/other/there.txt", "/a/b/sub/deep.txt"].map(PathBuf::from);
        assert_eq!(
            outside_folder(Path::new("/a/b"), paths.to_vec()),
            [PathBuf::from("/a/other/there.txt"), PathBuf::from("/a/b/sub/deep.txt")]
        );
    }
}
