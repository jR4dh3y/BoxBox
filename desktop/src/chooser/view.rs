//! The Open and Save dialog window: BoxBox's places, list and colours, with
//! the footer a GTK file chooser has. One window answers one portal request.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, uniform_list, AnchoredPositionMode, App, ClickEvent, Context, Entity, FocusHandle, Focusable, FontWeight,
    KeyDownEvent, SharedString, UniformListScrollHandle, Window,
};

use super::request::{self, Kind, Request, Response, Selection, Verdict};
use crate::api::{Api, FileInfo};
use crate::components::{
    Button, ButtonSize, ButtonVariant, ContextMenu, EntryIcon, FileListHeader, FileRow, Input, InputEvent, MenuItem, NavItem, Select, SidebarSection, TextField, Toggle,
};
use crate::format::{display_name, file_date, file_size, type_description, Kind as FileKind};
use crate::settings_state::{DesktopSettings, SavedConnection};
use crate::sort::{sort_files, Sort};
use crate::theme::*;
use crate::ui::{icon, TextScale};
use crate::workspace::{IconStyle, Preferences};

/// Where the list is: a local folder, or a folder on a paired server
/// (`path` empty is the server's list of drives).
#[derive(Clone)]
enum Location {
    Local(PathBuf),
    Server { id: String, name: String, api: Arc<Api>, path: String },
}

impl Location {
    fn is_remote(&self) -> bool {
        matches!(self, Self::Server { .. })
    }

    fn label(&self) -> String {
        match self {
            Self::Local(path) => path.display().to_string(),
            Self::Server { name, path, .. } if path.is_empty() => name.clone(),
            Self::Server { name, path, .. } => format!("{name}/{path}"),
        }
    }

    fn parent(&self) -> Option<Self> {
        match self {
            Self::Local(path) => path.parent().map(|parent| Self::Local(parent.to_path_buf())),
            Self::Server { path, .. } if path.is_empty() => None,
            Self::Server { id, name, api, path } => {
                Some(Self::Server { id: id.clone(), name: name.clone(), api: api.clone(), path: path.rsplit_once('/').map_or(String::new(), |(parent, _)| parent.to_string()) })
            }
        }
    }

    fn child(&self, item: &FileInfo) -> Self {
        match self {
            Self::Local(_) => Self::Local(PathBuf::from(&item.path)),
            Self::Server { id, name, api, .. } => Self::Server { id: id.clone(), name: name.clone(), api: api.clone(), path: item.path.clone() },
        }
    }
}

enum Listing {
    Loading,
    Ready,
    Failed(String),
}

enum ServerState {
    Connecting,
    Ready(Arc<Api>),
    Failed(String),
}

/// A sidebar row.
struct Place {
    label: String,
    icon: &'static str,
    path: PathBuf,
}

/// The inset of the toolbar and footer, so their right edges line up.
const EDGE: f32 = 12.;

/// What picking an option of a dropdown does.
type Pick = Rc<dyn Fn(&mut Chooser, usize, &mut Context<Chooser>)>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Filter,
    Choice(usize),
}

pub struct Chooser {
    request: Request,
    reply: Option<async_channel::Sender<Response>>,
    places: Vec<Place>,
    drives: Vec<Place>,
    bookmarks: Vec<Place>,
    servers: Vec<SavedConnection>,
    server_states: HashMap<String, ServerState>,
    location: Location,
    items: Vec<FileInfo>,
    /// Indices into `items` that show, in order.
    visible: Vec<usize>,
    listing: Listing,
    generation: u64,
    selected: HashSet<usize>,
    anchor: Option<usize>,
    active: Option<usize>,
    sort: Sort,
    /// The window's folder columns; sized here for this dialog only.
    columns: crate::components::ColumnLayout,
    show_hidden: bool,
    filter: Option<usize>,
    choices: Vec<(String, String)>,
    open_menu: Option<Menu>,
    /// The paths the user was asked about, and the question.
    replace: Option<(Vec<PathBuf>, String)>,
    error: Option<String>,
    busy: bool,
    /// The server download in flight; cancelled when the dialog closes.
    download: Option<crate::progress::Progress>,
    accent: u32,
    icon_style: IconStyle,
    name_field: Entity<TextField>,
    location_field: Entity<TextField>,
    search_field: Entity<TextField>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    pending_focus: Option<Entity<TextField>>,
    /// The size asked for once the window is shown.
    sized: bool,
    _subscriptions: Vec<gpui::Subscription>,
}

impl Drop for Chooser {
    /// A window closed any other way still answers the app.
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.try_send(Response::cancelled());
        }
        if let Some(download) = self.download.take() {
            download.cancel();
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// `~` and `~/…` as the home folder; `file://` as its path.
fn expand(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if let Some(path) = crate::os_clipboard::path_from_uri(text) {
        return Some(path);
    }
    let path = match text.strip_prefix('~') {
        Some("") => home(),
        Some(rest) if rest.starts_with('/') => home().join(rest.trim_start_matches('/')),
        _ => PathBuf::from(text),
    };
    path.is_absolute().then_some(path)
}

/// A folder's entries. Links show as what they point to; unreadable ones are skipped.
fn read_folder(path: &Path) -> anyhow::Result<Vec<FileInfo>> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(path)?.flatten() {
        let entry_path = entry.path();
        // The app is answered with this path rebuilt from text; a name that is
        // not UTF-8 would come back as a different file.
        let (Some(name), Some(text)) = (entry.file_name().to_str().map(str::to_string), entry_path.to_str().map(str::to_string)) else { continue };
        let Ok(metadata) = std::fs::metadata(&entry_path).or_else(|_| std::fs::symlink_metadata(&entry_path)) else { continue };
        items.push(FileInfo {
            name,
            path: text,
            size: if metadata.is_dir() { 0 } else { metadata.len() },
            is_dir: metadata.is_dir(),
            hidden: false,
            mod_time: metadata.modified().ok().and_then(crate::api::mod_time),
        });
    }
    Ok(items)
}

/// A private folder for copies of server files the app will open.
fn download_folder() -> anyhow::Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt as _;
    let cache = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).filter(|path| path.is_absolute()).unwrap_or_else(|| home().join(".cache"));
    let root = cache.join("boxbox-gpui/chosen");
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&root)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|time| time.as_nanos()).unwrap_or_default();
    let folder = root.join(format!("{}-{stamp}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&folder)?;
    Ok(folder)
}

impl Chooser {
    pub fn new(request: Request, reply: async_channel::Sender<Response>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title(&Self::title(&request));
        let preferences = Preferences::load();
        let settings = DesktopSettings::load();
        let accent = settings.custom_accent.unwrap_or_else(|| settings.accent.hex());
        let (places, drives) = local_places();
        // A bookmark of a place or drive already listed is not listed twice.
        let listed: HashSet<PathBuf> = places.iter().chain(&drives).map(|place| std::fs::canonicalize(&place.path).unwrap_or_else(|_| place.path.clone())).collect();
        let bookmarks: Vec<Place> =
            bookmark_places().into_iter().filter(|place| !listed.contains(&std::fs::canonicalize(&place.path).unwrap_or_else(|_| place.path.clone()))).collect();
        let name_field = cx.new(|cx| TextField::new("File name", cx));
        let location_field = cx.new(|cx| TextField::new("Type a path", cx));
        let search_field = cx.new(|cx| TextField::new("Search this folder", cx));
        let subscriptions = vec![
            cx.subscribe_in(&name_field, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.accept(window, cx),
                InputEvent::Cancel => this.cancel(window, cx),
                InputEvent::Changed => {
                    this.replace = None;
                    this.error = None;
                    cx.notify();
                }
            }),
            cx.subscribe_in(&location_field, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.go_to_typed(window, cx),
                InputEvent::Cancel => {
                    this.show_location(cx);
                    this.focus.focus(window);
                }
                InputEvent::Changed => {}
            }),
            cx.subscribe_in(&search_field, window, |this, _, event, window, cx| match event {
                InputEvent::Changed => this.refilter(cx),
                InputEvent::Cancel => {
                    this.search_field.update(cx, |field, cx| field.set_text("", 0..0, cx));
                    this.refilter(cx);
                    this.focus.focus(window);
                }
                InputEvent::Submit => this.focus.focus(window),
            }),
        ];
        let start = request.folder.clone().filter(|folder| folder.is_dir()).unwrap_or_else(home);
        let choices = request.choices.iter().map(|choice| (choice.id.clone(), choice.value.clone())).collect();
        let pending_focus = (request.kind == Kind::Save).then(|| name_field.clone());
        if request.kind == Kind::Save {
            let name = request.name.clone();
            // The stem is selected, so typing keeps the extension, as GTK does.
            let stem = name.rfind('.').filter(|&dot| dot > 0).unwrap_or(name.len());
            name_field.update(cx, |field, cx| field.set_text(name, 0..stem, cx));
        }
        let mut chooser = Self {
            filter: request.filter,
            request,
            reply: Some(reply),
            places,
            drives,
            bookmarks,
            servers: settings.connections.clone(),
            server_states: HashMap::new(),
            location: Location::Local(start.clone()),
            items: Vec::new(),
            visible: Vec::new(),
            listing: Listing::Loading,
            generation: 0,
            selected: HashSet::new(),
            anchor: None,
            active: None,
            sort: Sort { field: preferences.default_sort, dir: preferences.default_sort_direction },
            columns: preferences.columns(crate::components::Table::Files),
            show_hidden: preferences.show_hidden_files,
            choices,
            open_menu: None,
            replace: None,
            error: None,
            busy: false,
            download: None,
            accent,
            icon_style: preferences.file_icons,
            name_field,
            location_field,
            search_field,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            pending_focus,
            sized: false,
            _subscriptions: subscriptions,
        };
        chooser.open(Location::Local(start), cx);
        chooser
    }

    pub fn title(request: &Request) -> String {
        if !request.title.is_empty() {
            return request.title.clone();
        }
        match (request.kind, request.directory) {
            (Kind::Open, true) => "Choose a Folder",
            (Kind::Open, false) if request.multiple => "Open Files",
            (Kind::Open, false) => "Open File",
            (Kind::Save, _) => "Save File",
            (Kind::SaveFiles, _) => "Save Files",
        }
        .to_string()
    }

    // ---- Answering ----

    /// Answers the app and closes the window.
    fn finish(&mut self, response: Response, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(reply) = self.reply.take() {
            let _ = reply.try_send(response);
        }
        if let Some(download) = self.download.take() {
            download.cancel();
        }
        // After the event that answered has finished with the window.
        cx.defer_in(window, |_, window, _| window.remove_window());
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish(Response::cancelled(), window, cx);
    }

    fn selected_items(&self) -> Vec<&FileInfo> {
        let mut indices: Vec<usize> = self.selected.iter().copied().collect();
        indices.sort_unstable();
        indices.iter().filter_map(|&index| self.items.get(index)).collect()
    }

    /// Whether Accept has something to act on.
    fn accept_ready(&self, cx: &App) -> bool {
        if self.busy || !matches!(self.listing, Listing::Ready) {
            return false;
        }
        match self.request.kind {
            Kind::Open if self.request.directory => !self.location.is_remote(),
            Kind::Open => self.selected_items().iter().any(|item| !item.is_dir),
            Kind::Save => !self.location.is_remote() && !self.name_field.read(cx).text().trim().is_empty(),
            Kind::SaveFiles => !self.location.is_remote(),
        }
    }

    fn accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.accept_ready(cx) {
            return;
        }
        self.error = None;
        let folder = match &self.location {
            Location::Local(folder) => Some(folder.clone()),
            Location::Server { .. } => None,
        };
        let paths: Vec<PathBuf> = match (self.request.kind, &folder) {
            (Kind::Save, Some(folder)) => {
                let name = self.name_field.read(cx).text().trim().to_string();
                if !request::valid_name(&name) {
                    self.error = Some("Enter a file name without a slash".into());
                    cx.notify();
                    return;
                }
                vec![folder.join(name)]
            }
            (Kind::SaveFiles, Some(folder)) => vec![folder.clone()],
            (Kind::Open, Some(folder)) => {
                let wanted: Vec<PathBuf> =
                    self.selected_items().into_iter().filter(|item| item.is_dir == self.request.directory).map(|item| PathBuf::from(&item.path)).collect();
                if wanted.is_empty() && self.request.directory {
                    vec![folder.clone()]
                } else {
                    wanted
                }
            }
            (Kind::Open, None) => return self.download_and_accept(window, cx),
            _ => return,
        };
        self.check_and_answer(paths, window, cx);
    }

    fn check_and_answer(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let replace = self.replace.as_ref().is_some_and(|(asked, _)| *asked == paths);
        let selection = Selection { paths: paths.clone(), filter: self.filter, choices: self.choices.clone(), replace };
        match request::accept(&self.request, &selection) {
            Ok(Verdict::Accept(response)) => self.finish(response, window, cx),
            Ok(Verdict::AskReplace(question)) => self.replace = Some((paths, question)),
            Err(error) => self.error = Some(format!("{error:#}")),
        }
        cx.notify();
    }

    /// Server files reach the app as private local copies.
    fn download_and_accept(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Location::Server { api, .. } = &self.location else { return };
        let items: Vec<FileInfo> = self.selected_items().into_iter().filter(|item| !item.is_dir).cloned().collect();
        if items.is_empty() {
            return;
        }
        let api = api.clone();
        let progress = crate::progress::Progress::default();
        if let Some(previous) = self.download.replace(progress.clone()) {
            previous.cancel();
        }
        self.busy = true;
        cx.notify();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let running = progress.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let folder = download_folder()?;
                    let paths = running.scope(|| items.iter().map(|item| api.download(item, &folder)).collect::<anyhow::Result<Vec<PathBuf>>>());
                    match paths {
                        Ok(paths) => Ok((folder, paths)),
                        Err(error) => {
                            let _ = std::fs::remove_dir_all(&folder);
                            Err(error)
                        }
                    }
                })
                .await;
            // The copies stay only for the app that was answered with them.
            let mut kept = false;
            let folder = result.as_ref().ok().map(|(folder, _)| folder.clone());
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if progress.is_cancelled() {
                        return;
                    }
                    this.busy = false;
                    this.download = None;
                    match result {
                        Ok((_, paths)) => {
                            this.check_and_answer(paths, window, cx);
                            kept = this.reply.is_none();
                        }
                        Err(error) => {
                            this.error = Some(format!("Could not download: {error:#}"));
                            cx.notify();
                        }
                    }
                });
            });
            if let Some(folder) = folder.filter(|_| !kept) {
                let _ = std::fs::remove_dir_all(folder);
            }
        })
        .detach();
    }

    // ---- Moving around ----

    fn open(&mut self, location: Location, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.location = location.clone();
        self.listing = Listing::Loading;
        self.items.clear();
        self.visible.clear();
        self.selected.clear();
        self.anchor = None;
        self.active = None;
        self.replace = None;
        self.error = None;
        self.open_menu = None;
        self.search_field.update(cx, |field, cx| field.set_text("", 0..0, cx));
        self.show_location(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match location {
                        Location::Local(path) => read_folder(&path),
                        Location::Server { api, path, .. } if path.is_empty() => Ok(api
                            .drives()?
                            .into_iter()
                            .map(|drive| FileInfo { name: drive.label.clone().unwrap_or_else(|| drive.name.clone()), path: drive.name, size: 0, is_dir: true, hidden: false, mod_time: None })
                            .collect()),
                        Location::Server { api, path, .. } => api.list(&path),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(items) => {
                        this.items = items;
                        this.listing = Listing::Ready;
                    }
                    Err(error) => this.listing = Listing::Failed(format!("{error:#}")),
                }
                this.refilter(cx);
            });
        })
        .detach();
    }

    fn show_location(&mut self, cx: &mut Context<Self>) {
        let text = self.location.label();
        let end = text.len();
        self.location_field.update(cx, |field, cx| field.set_text(text, end..end, cx));
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.location.parent() {
            self.open(parent, cx);
        }
    }

    /// A typed folder opens; a typed file is chosen, or named for saving.
    fn go_to_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.location_field.read(cx).text().to_string();
        let Some(path) = expand(&text) else {
            self.error = Some("Type a full path, starting with / or ~".into());
            cx.notify();
            return;
        };
        if path.is_dir() {
            self.open(Location::Local(path), cx);
            self.focus.focus(window);
            return;
        }
        let (Some(parent), Some(name)) = (path.parent().filter(|parent| parent.is_dir()), path.file_name()) else {
            self.error = Some(format!("{} does not exist", path.display()));
            cx.notify();
            return;
        };
        let name = name.to_string_lossy().into_owned();
        match self.request.kind {
            Kind::Save => {
                self.open(Location::Local(parent.to_path_buf()), cx);
                let end = name.len();
                self.name_field.update(cx, |field, cx| field.set_text(name, 0..end, cx));
                self.pending_focus = Some(self.name_field.clone());
            }
            Kind::Open if path.exists() && !self.request.directory => self.check_and_answer(vec![path], window, cx),
            _ => {
                self.error = Some(format!("{} is not a folder", path.display()));
                cx.notify();
            }
        }
    }

    fn connect_server(&mut self, connection: SavedConnection, cx: &mut Context<Self>) {
        if let Some(ServerState::Ready(api)) = self.server_states.get(&connection.id) {
            let location = Location::Server { id: connection.id.clone(), name: connection.name.clone(), api: api.clone(), path: String::new() };
            return self.open(location, cx);
        }
        if matches!(self.server_states.get(&connection.id), Some(ServerState::Connecting)) {
            return;
        }
        self.server_states.insert(connection.id.clone(), ServerState::Connecting);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let id = connection.id.clone();
            let saved = connection.clone();
            let result = cx.background_executor().spawn(async move { crate::workspace::servers::connect(&saved) }).await;
            let _ = this.update(cx, |this, cx| {
                let state = match result {
                    Ok(crate::workspace::servers::Connected::Ready(api, _)) => ServerState::Ready(api),
                    Ok(crate::workspace::servers::Connected::NeedsPairing(reason)) => ServerState::Failed(format!("{reason} Pair it in BoxBox first.")),
                    Err(error) => ServerState::Failed(format!("{error:#}")),
                };
                let ready = matches!(state, ServerState::Ready(_));
                if let ServerState::Failed(message) = &state {
                    this.error = Some(message.clone());
                }
                this.server_states.insert(id, state);
                if ready {
                    this.connect_server(connection, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ---- The list ----

    fn refilter(&mut self, cx: &mut Context<Self>) {
        let query = self.search_field.read(cx).text().trim().to_lowercase();
        let mut shown: Vec<FileInfo> = Vec::new();
        let mut index_of: HashMap<String, usize> = HashMap::new();
        for (index, item) in self.items.iter().enumerate() {
            if !self.show_hidden && item.name.starts_with('.') {
                continue;
            }
            if !query.is_empty() && !item.name.to_lowercase().contains(&query) {
                continue;
            }
            if !self.request.shows(&item.name, item.is_dir, self.filter) {
                continue;
            }
            index_of.insert(item.path.clone(), index);
            shown.push(item.clone());
        }
        sort_files(&mut shown, self.sort);
        self.visible = shown.iter().filter_map(|item| index_of.get(&item.path).copied()).collect();
        let visible: HashSet<usize> = self.visible.iter().copied().collect();
        self.selected.retain(|index| visible.contains(index));
        if self.active.is_some_and(|row| row >= self.visible.len()) {
            self.active = None;
        }
        cx.notify();
    }

    fn click(&mut self, row: usize, event: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&index) = self.visible.get(row) else { return };
        self.focus.focus(window);
        self.open_menu = None;
        let item = self.items[index].clone();
        if event.click_count() >= 2 {
            if item.is_dir {
                let location = self.location.child(&item);
                return self.open(location, cx);
            }
            match self.request.kind {
                Kind::Open => {
                    if !self.request.multiple || !self.selected.contains(&index) {
                        self.selected = HashSet::from([index]);
                    }
                    return self.accept(window, cx);
                }
                Kind::Save => self.set_name(&item.name, cx),
                Kind::SaveFiles => {}
            }
            return;
        }
        let modifiers = event.modifiers();
        if self.request.multiple && modifiers.shift {
            let anchor = self.anchor.unwrap_or(row);
            let range = anchor.min(row)..=anchor.max(row);
            self.selected = range.filter_map(|row| self.visible.get(row).copied()).collect();
        } else if self.request.multiple && (modifiers.control || modifiers.platform) {
            if !self.selected.remove(&index) {
                self.selected.insert(index);
            }
            self.anchor = Some(row);
        } else {
            self.selected = HashSet::from([index]);
            self.anchor = Some(row);
        }
        self.active = Some(row);
        if self.request.kind == Kind::Save && !item.is_dir {
            self.set_name(&item.name, cx);
        }
        self.replace = None;
        cx.notify();
    }

    fn set_name(&mut self, name: &str, cx: &mut Context<Self>) {
        let name = name.to_string();
        let end = name.len();
        self.name_field.update(cx, |field, cx| field.set_text(name, 0..end, cx));
        self.replace = None;
    }

    fn move_active(&mut self, step: isize, extend: bool, cx: &mut Context<Self>) {
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() - 1;
        let row = match self.active {
            None => 0,
            Some(row) => (row as isize + step).clamp(0, last as isize) as usize,
        };
        self.active = Some(row);
        let index = self.visible[row];
        if extend && self.request.multiple {
            let anchor = self.anchor.unwrap_or(row);
            self.selected = (anchor.min(row)..=anchor.max(row)).filter_map(|row| self.visible.get(row).copied()).collect();
        } else {
            self.selected = HashSet::from([index]);
            self.anchor = Some(row);
            if self.request.kind == Kind::Save && !self.items[index].is_dir {
                let name = self.items[index].name.clone();
                self.set_name(&name, cx);
            }
        }
        self.scroll.scroll_to_item(row, gpui::ScrollStrategy::Top);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let control = modifiers.control || modifiers.platform;
        match keystroke.key.as_str() {
            "escape" if self.open_menu.is_some() => self.open_menu = None,
            "escape" => return self.cancel(window, cx),
            "l" if control => {
                self.location_field.update(cx, |field, cx| {
                    let text = field.text().to_string();
                    let end = text.len();
                    field.set_text(text, 0..end, cx);
                });
                self.location_field.read(cx).focus(window);
            }
            "f" if control => self.search_field.read(cx).focus(window),
            "h" if control => {
                self.show_hidden = !self.show_hidden;
                self.refilter(cx);
            }
            _ if !self.focus.is_focused(window) => return,
            "up" if modifiers.alt => self.go_up(cx),
            "up" => self.move_active(-1, modifiers.shift, cx),
            "down" => self.move_active(1, modifiers.shift, cx),
            "home" => self.move_active(-(self.visible.len() as isize), modifiers.shift, cx),
            "end" => self.move_active(self.visible.len() as isize, modifiers.shift, cx),
            "a" if control && self.request.multiple => {
                self.selected = self.visible.iter().copied().collect();
            }
            "backspace" => self.go_up(cx),
            "enter" => {
                let folder = self.active.and_then(|row| self.visible.get(row)).map(|&index| self.items[index].clone()).filter(|item| item.is_dir);
                match folder {
                    Some(item) => {
                        let location = self.location.child(&item);
                        self.open(location, cx);
                    }
                    None => self.accept(window, cx),
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn set_choice(&mut self, id: &str, value: &str) {
        match self.choices.iter_mut().find(|(choice, _)| choice == id) {
            Some(entry) => entry.1 = value.to_string(),
            None => self.choices.push((id.to_string(), value.to_string())),
        }
    }
}

/// Home, the user folders, then the drives, from the same mounts the main window lists.
fn local_places() -> (Vec<Place>, Vec<Place>) {
    let Ok(api) = Api::local() else { return (vec![Place { label: "Home".into(), icon: "home", path: home() }], Vec::new()) };
    let (mut places, mut drives) = (Vec::new(), Vec::new());
    for root in api.roots().unwrap_or_default() {
        let Ok(Some(path)) = api.local_path(&root.name) else { continue };
        if !path.is_dir() {
            continue;
        }
        let label = root.display_name().to_string();
        match root.kind {
            crate::api::RootKind::Drive => drives.push(Place { label, icon: "hard-drive", path }),
            _ => {
                let icon = match root.name.as_str() {
                    "home" => "home",
                    "desktop" => "monitor",
                    "downloads" => "download",
                    "documents" => "file-text",
                    "music" => "music",
                    "pictures" => "image",
                    "videos" => "video",
                    "templates" => "layout-template",
                    _ => "folder",
                };
                places.push(Place { label, icon, path });
            }
        }
    }
    (places, drives)
}

/// The GTK bookmarks that are local folders.
fn bookmark_places() -> Vec<Place> {
    let Ok(text) = std::fs::read_to_string(crate::workspace::favorites::bookmarks_path()) else { return Vec::new() };
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (uri, label) = line.split_once(' ').map_or((line, None), |(uri, label)| (uri, Some(label.trim())));
            let path = crate::os_clipboard::path_from_uri(uri)?;
            let label = label.filter(|label| !label.is_empty()).map(str::to_string).or_else(|| path.file_name().map(|name| name.to_string_lossy().into_owned()))?;
            Some(Place { label, icon: "bookmark", path })
        })
        .collect()
}

// ---- Drawing ----

impl Focusable for Chooser {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Chooser {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = match &self.location {
            Location::Local(path) => Some(path.clone()),
            Location::Server { .. } => None,
        };
        let row = |id: String, place: &Place, cx: &mut Context<Self>| {
            let path = place.path.clone();
            NavItem::new(SharedString::from(id), place.label.clone(), place.icon).active(current.as_deref() == Some(place.path.as_path())).on_click(cx.listener(
                move |this, _, window, cx| {
                    this.open(Location::Local(path.clone()), cx);
                    this.focus.focus(window);
                },
            ))
        };
        let mut sections = Vec::new();
        let mut places = SidebarSection::new("chooser-places", "Places");
        for (index, place) in self.places.iter().enumerate() {
            places = places.child(row(format!("chooser-place-{index}"), place, cx));
        }
        sections.push(places.into_any_element());
        if !self.bookmarks.is_empty() {
            let mut section = SidebarSection::new("chooser-bookmarks", "Favorites");
            for (index, place) in self.bookmarks.iter().enumerate() {
                section = section.child(row(format!("chooser-bookmark-{index}"), place, cx));
            }
            sections.push(section.into_any_element());
        }
        if !self.drives.is_empty() {
            let mut section = SidebarSection::new("chooser-drives", "Drives");
            for (index, place) in self.drives.iter().enumerate() {
                section = section.child(row(format!("chooser-drive-{index}"), place, cx));
            }
            sections.push(section.into_any_element());
        }
        if !self.servers.is_empty() {
            let mut section = SidebarSection::new("chooser-servers", "Servers");
            for connection in &self.servers {
                let active = matches!(&self.location, Location::Server { id, .. } if *id == connection.id);
                let detail = match self.server_states.get(&connection.id) {
                    Some(ServerState::Connecting) => Some("Connecting…"),
                    Some(ServerState::Failed(_)) => Some("Unavailable"),
                    _ => None,
                };
                let saved = connection.clone();
                let item = NavItem::new(SharedString::from(format!("chooser-server-{}", connection.id)), connection.name.clone(), "server")
                    .active(active)
                    .tooltip(connection.url.clone())
                    .on_click(cx.listener(move |this, _, _, cx| this.connect_server(saved.clone(), cx)));
                section = section.child(match detail {
                    Some(detail) => item.detail(detail),
                    None => item,
                });
            }
            sections.push(section.into_any_element());
        }
        div()
            .id("chooser-sidebar")
            .flex_none()
            .w(px(200.))
            .h_full()
            .overflow_y_scroll()
            .bg(c(SURFACE_SECONDARY))
            .border_r_1()
            .border_color(c(BORDER_SECONDARY))
            .children(sections)
    }

    fn render_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let location_focused = self.location_field.read(cx).is_focused(window);
        let search_focused = self.search_field.read(cx).is_focused(window);
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .px(px(EDGE))
            .h(px(48.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .child(
                Button::new("chooser-up")
                    .icon("chevron-up")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Icon)
                    .disabled(self.location.parent().is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.go_up(cx))),
            )
            .child(
                div()
                    .id("chooser-location")
                    .flex_1()
                    .min_w_0()
                    .debug_selector(|| "chooser-location".into())
                    .child(Input::new("").focused(location_focused).editor(self.location_field.clone())),
            )
            .child(div().id("chooser-search").w(px(200.)).flex_none().child(Input::new("").focused(search_focused).editor(self.search_field.clone())))
    }

    fn render_row(&self, row: usize, cx: &mut Context<Self>) -> Option<FileRow> {
        let &index = self.visible.get(row)?;
        let item = &self.items[index];
        let kind = FileKind::of(&item.name, item.is_dir);
        let tint = match (self.icon_style, kind) {
            (IconStyle::Tinted, FileKind::Folder) => Some(self.accent),
            (IconStyle::Tinted, kind) => kind.tint(),
            _ => None,
        };
        let selection = crate::components::Selection {
            background: c(crate::workspace::appearance::mix(self.accent, 0x15171c, 0.30)),
            accent: c(self.accent),
        };
        let name = display_name(&item.name, item.is_dir, true);
        // In a save dialog files are only for taking a name from.
        let dimmed = self.request.kind == Kind::Save && !item.is_dir;
        Some(
            FileRow::new(("chooser-row", row), name, item.is_dir, selection)
                .icon(kind.glyph())
                .tint(tint, None)
                .image(EntryIcon::Glyph)
                .columns(self.columns.specs(crate::components::Table::Files))
                .value(crate::components::Column::Type, if item.is_dir { "Folder".into() } else { type_description(&item.name) })
                .value(crate::components::Column::Size, if item.is_dir { "—".into() } else { file_size(item.size) })
                .value(crate::components::Column::Modified, file_date(item.mod_time))
                .selected(self.selected.contains(&index))
                .active(self.active == Some(row))
                .cut(dimmed)
                .decorate({
                    let view = cx.entity().downgrade();
                    move |element| {
                        element.debug_selector(move || format!("chooser-row-{row}")).on_click(move |event, window, cx| {
                            let _ = view.update(cx, |this, cx| this.click(row, event, window, cx));
                        })
                    }
                }),
        )
    }

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let message = match &self.listing {
            Listing::Loading => Some(("folder", "Loading…".to_string())),
            Listing::Failed(error) => Some(("alert-triangle", error.clone())),
            Listing::Ready if self.visible.is_empty() => Some(("folder-open", if self.items.is_empty() { "This folder is empty".into() } else { "Nothing here matches".into() })),
            Listing::Ready => None,
        };
        let count = self.visible.len();
        let table = crate::components::Table::Files;
        let header = FileListHeader::new(table, self.columns.clone(), 0)
            .sorted(Some((self.sort.field.column(), self.sort.dir)))
            .on_sort({
                let view = cx.entity().downgrade();
                move |column, _, cx| {
                    let Some(field) = crate::sort::SortField::of_column(column) else { return };
                    let _ = view.update(cx, |this, cx| {
                        this.sort = this.sort.toggled(field);
                        this.refilter(cx);
                    });
                }
            })
            .on_resize({
                let view = cx.entity().downgrade();
                move |columns, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.columns = columns;
                        cx.notify();
                    });
                }
            });
        let list = crate::components::table_frame(&self.columns, table)
            .child(header)
            .when_some(message, |list, (glyph, text)| {
                list.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .py(px(48.))
                        .px(px(24.))
                        .child(icon(glyph, 32., alpha(TEXT_MUTED, 0.5)))
                        .child(div().text_13().text_color(c(TEXT_MUTED)).child(text)),
                )
            })
            .when(count > 0, |list| {
                list.child(
                    uniform_list("chooser-list", count, cx.processor(|this, range: Range<usize>, _window, cx| range.filter_map(|row| this.render_row(row, cx)).collect::<Vec<_>>()))
                        .track_scroll(self.scroll.clone())
                        .flex_1(),
                )
            });
        div().flex_1().min_h_0().child(crate::components::sideways("chooser-table", list))
    }

    /// A select whose list opens above it, since the footer sits at the bottom.
    #[allow(clippy::too_many_arguments)]
    fn dropdown(
        &self,
        id: &'static str,
        menu: Menu,
        label: String,
        options: Vec<String>,
        chosen: Option<usize>,
        cx: &mut Context<Self>,
        pick: Pick,
    ) -> impl IntoElement {
        let open = self.open_menu == Some(menu);
        let slot_id = match menu {
            Menu::Filter => id.to_string(),
            Menu::Choice(index) => format!("{id}-{index}"),
        };
        let selector = slot_id.clone();
        div()
            .relative()
            .min_w(px(160.))
            .max_w(px(260.))
            .debug_selector(move || selector)
            .child(Select::new(SharedString::from(format!("{slot_id}-select")), label).on_press(cx.listener(move |this, _, _, cx| {
                this.open_menu = if this.open_menu == Some(menu) { None } else { Some(menu) };
                cx.notify();
            })))
            .when(open, |slot| {
                let items = options
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        let item = MenuItem::action(SharedString::from(format!("{slot_id}-{index}")), label);
                        let item = if chosen == Some(index) { item.icon("check") } else { item };
                        let pick = pick.clone();
                        item.on_select(cx.listener(move |this, _, _, cx| {
                            this.open_menu = None;
                            pick(this, index, cx);
                            cx.notify();
                        }))
                    })
                    .collect();
                slot.child(div().absolute().top_0().left_0().child(
                    deferred(
                        anchored().position_mode(AnchoredPositionMode::Local).position(point(px(0.), px(-4.))).anchor(gpui::Corner::BottomLeft).snap_to_window().child(
                            div()
                                .id(SharedString::from(format!("{slot_id}-menu")))
                                .occlude()
                                .min_w(px(160.))
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.open_menu = None;
                                    cx.notify();
                                }))
                                .child(ContextMenu::new(items)),
                        ),
                    )
                    .with_priority(2),
                ))
            })
    }

    fn render_footer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut left: Vec<gpui::AnyElement> = Vec::new();
        match self.request.kind {
            Kind::Save => {
                let focused = self.name_field.read(cx).is_focused(window);
                left.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .flex_1()
                        .min_w(px(220.))
                        .child(div().text_13().text_color(c(TEXT_SECONDARY)).child("Name"))
                        .child(div().id("chooser-name").debug_selector(|| "chooser-name".into()).flex_1().child(Input::new("").focused(focused).editor(self.name_field.clone())))
                        .into_any_element(),
                );
            }
            Kind::SaveFiles => {
                let count = self.request.files.len();
                left.push(div().text_13().text_color(c(TEXT_SECONDARY)).child(format!("Saves {count} {} in this folder", if count == 1 { "file" } else { "files" })).into_any_element());
            }
            Kind::Open => {}
        }
        if !self.request.filters.is_empty() && !self.request.directory {
            let label = self.filter.and_then(|index| self.request.filters.get(index)).map_or_else(|| "All files".to_string(), |filter| filter.name.clone());
            let options = self.request.filters.iter().map(|filter| filter.name.clone()).collect();
            let pick: Pick = Rc::new(|this, index, cx| {
                this.filter = Some(index);
                this.replace = None;
                this.refilter(cx);
            });
            left.push(self.dropdown("chooser-filter", Menu::Filter, label, options, self.filter, cx, pick).into_any_element());
        }
        for (position, choice) in self.request.choices.iter().enumerate() {
            let value = self.choices.iter().find(|(id, _)| *id == choice.id).map_or(choice.value.clone(), |(_, value)| value.clone());
            let id = choice.id.clone();
            if choice.options.is_empty() {
                let checked = value == "true";
                let view = cx.entity().downgrade();
                left.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(Toggle::new(SharedString::from(format!("chooser-choice-{position}")), checked).on_change(move |on, _, _, cx| {
                            let id = id.clone();
                            let _ = view.update(cx, |this, cx| {
                                this.set_choice(&id, if on { "true" } else { "false" });
                                cx.notify();
                            });
                        }))
                        .child(div().text_13().child(choice.label.clone()))
                        .into_any_element(),
                );
            } else {
                let chosen = choice.options.iter().position(|(option, _)| *option == value);
                let label = chosen.map_or_else(|| value.clone(), |index| choice.options[index].1.clone());
                let option_ids: Vec<String> = choice.options.iter().map(|(option, _)| option.clone()).collect();
                let pick: Pick = Rc::new(move |this, index, _| {
                    if let Some(option) = option_ids.get(index) {
                        this.set_choice(&id, option);
                    }
                });
                let options = choice.options.iter().map(|(_, label)| label.clone()).collect();
                left.push(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_13().text_color(c(TEXT_SECONDARY)).child(choice.label.clone()))
                        .child(self.dropdown("chooser-choice", Menu::Choice(position), label, options, chosen, cx, pick))
                        .into_any_element(),
                );
            }
        }
        let ready = self.accept_ready(cx);
        let accept_label = if self.replace.is_some() { "Replace".to_string() } else { self.request.accept_label.clone() };
        let note = if let Some((_, question)) = &self.replace {
            Some((question.clone(), WARNING))
        } else if let Some(error) = &self.error {
            Some((error.clone(), DANGER))
        } else if self.location.is_remote() && (self.request.kind != Kind::Open || self.request.directory) {
            Some(("Server folders can only open files. Choose a folder on this computer.".to_string(), TEXT_SECONDARY))
        } else if self.location.is_remote() {
            Some((if self.busy { "Downloading a copy…".to_string() } else { "Opening downloads a copy; changes stay on this computer.".to_string() }, TEXT_SECONDARY))
        } else {
            None
        };
        let hidden = self.show_hidden;
        let view = cx.entity().downgrade();
        div()
            .flex()
            .flex_col()
            .flex_none()
            .border_t_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(c(SURFACE_SECONDARY))
            .when_some(note, |footer, (text, color)| footer.child(div().id("chooser-note").debug_selector(|| "chooser-note".into()).px(px(EDGE)).pt(px(10.)).text_13().text_color(c(color)).child(text)))
            .child(
                // Options wrap on the left; the buttons keep the right edge, as in GTK and KDE dialogs.
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .px(px(EDGE))
                    .py(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .flex_wrap()
                            .items_center()
                            .gap_x(px(16.))
                            .gap_y(px(10.))
                            .children(left)
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(Toggle::new("chooser-hidden", hidden).on_change(move |on, _, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.show_hidden = on;
                                            this.refilter(cx);
                                        });
                                    }))
                                    .child(div().text_13().text_color(c(TEXT_SECONDARY)).child("Hidden files")),
                            ),
                    )
                    .child(
                        div()
                            .id("chooser-buttons")
                            .debug_selector(|| "chooser-buttons".into())
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(8.))
                            .child(Button::new("chooser-cancel").label("Cancel").variant(ButtonVariant::Secondary).on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx))))
                            .child(
                                Button::new("chooser-accept")
                                    .label(accept_label)
                                    .variant(if self.replace.is_some() { ButtonVariant::Danger } else { ButtonVariant::Primary })
                                    .disabled(!ready)
                                    .on_click(cx.listener(|this, _, window, cx| this.accept(window, cx))),
                            ),
                    ),
            )
    }
}

impl Render for Chooser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // GPUI names the window only after its first commit, so a compositor rule
        // keyed on the app id cannot size it, and tiling compositors pick a column
        // size. A dialog sizes itself once shown; a floating window keeps it.
        if !std::mem::replace(&mut self.sized, true) {
            cx.on_next_frame(window, |_, window, _| window.resize(gpui::size(px(960.), px(620.))));
        }
        match self.pending_focus.take() {
            Some(field) => field.read(cx).focus(window),
            None if window.focused(cx).is_none() => self.focus.focus(window),
            None => {}
        }
        let font = cx.try_global::<crate::components::UiFont>().map(|font| font.0.clone());
        div()
            .id("chooser")
            .key_context("Chooser")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .when_some(font, |root, font| root.font_family(font))
            .size_full()
            .flex()
            .flex_col()
            .bg(c(SURFACE_PRIMARY))
            .text_color(c(TEXT_PRIMARY))
            .text_13()
            .when(!self.request.app_id.is_empty(), |root| {
                root.child(
                    div()
                        .flex_none()
                        .px(px(16.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(c(BORDER_SECONDARY))
                        .text_xs_tw()
                        .text_color(c(TEXT_SECONDARY))
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("For {}", self.request.app_id)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(div().flex().flex_col().flex_1().min_w_0().child(self.render_toolbar(window, cx)).child(self.render_list(cx))),
            )
            .child(self.render_footer(window, cx))
    }
}

#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
