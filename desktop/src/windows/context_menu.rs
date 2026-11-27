//! Builds Windows Shell context menus on one STA thread and hands their rows to the UI.
//!
//! Shell extensions load DLLs and can take tens of milliseconds per menu, so no
//! Shell call runs on the UI thread. The thread owns every `IContextMenu`; the UI
//! holds a `ShellMenu` handle, draws the rows itself and sends commands back.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut, NonNull};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};

use anyhow::{anyhow, ensure, Context as _, Result};
use windows::core::{w, Interface, PCSTR, PCWSTR, PSTR, PWSTR};
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::{
        ClientToScreen, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    },
    System::{Com::CoTaskMemFree, Ole::OleInitialize},
    UI::{
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL},
        Shell::{
            Common::ITEMIDLIST, IContextMenu, IContextMenu2, IContextMenu3, IEnumShellItems,
            ILClone, ILFindLastID, ILFree, ILIsParent, ILRemoveLastID, IShellFolder, IShellItem,
            SHBindToObject, SHBindToParent, SHGetKnownFolderItem, SHParseDisplayName,
            BHID_EnumItems, FOLDERID_SendTo, KF_FLAG_DEFAULT, SIGDN_NORMALDISPLAY,
            CMF_EXTENDEDVERBS, CMF_NORMAL,
            CMIC_MASK_CONTROL_DOWN, CMIC_MASK_PTINVOKE, CMIC_MASK_SHIFT_DOWN, CMINVOKECOMMANDINFO,
            CMINVOKECOMMANDINFOEX, GCS_VERBW, SEE_MASK_UNICODE,
        },
        WindowsAndMessaging::{
            CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
            GetMenuItemCount, GetMenuItemInfoW, GetMessageW, PostMessageW, RegisterClassW,
            TranslateMessage, HMENU, HWND_MESSAGE, MENUITEMINFOW, MFS_DISABLED, MFT_SEPARATOR,
            MIIM_BITMAP, MIIM_CHECKMARKS, MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING,
            MIIM_SUBMENU, MSG, SW_SHOWNORMAL,
            WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_INITMENUPOPUP, WNDCLASSW,
        },
    },
};

const FIRST_COMMAND: u32 = 1;
const LAST_COMMAND: u32 = 0x7fff;
/// Tells the Shell thread's host window that jobs are queued.
const WM_RUN_JOBS: u32 = WM_APP + 1;

/// The local paths a Shell menu is for.
pub(crate) enum Target {
    Folder(PathBuf),
    Items(Vec<PathBuf>),
}

pub(crate) enum Entry {
    Action {
        id: u32,
        label: String,
        icon: Option<Icon>,
        shortcut: Option<String>,
        disabled: bool,
    },
    Submenu {
        branch: Branch,
        label: String,
        icon: Option<Icon>,
        disabled: bool,
    },
    Separator,
}

/// A row's bitmap as straight-alpha BGRA, the layout GPUI's `RenderImage` takes.
pub(crate) struct Icon {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// A submenu of one `ShellMenu`. The handle is only used on the Shell thread.
#[derive(Clone, Copy)]
pub(crate) struct Branch {
    menu: isize,
    index: u32,
    /// The popup row's own command ID, which some handlers give a verb.
    command: u32,
}

/// A menu that lives on the Shell thread until this handle drops.
pub(crate) struct ShellMenu {
    id: u64,
}

impl ShellMenu {
    pub(crate) async fn open(
        owner: isize,
        target: Target,
        shift: bool,
    ) -> Result<(Self, Vec<Entry>)> {
        let id = next_id();
        // Created before the request, so an abandoned request still releases the menu.
        let menu = Self { id };
        let entries = request(|reply| Job::Open {
            id,
            owner,
            target,
            shift,
            reply,
        })
        .await?;
        Ok((menu, entries))
    }

    pub(crate) async fn expand(&self, branch: Branch) -> Result<Vec<Entry>> {
        let id = self.id;
        request(|reply| Job::Expand { id, branch, reply }).await
    }

    /// `anchor` is in physical pixels relative to the owner window's client area.
    pub(crate) async fn invoke(&self, command: u32, anchor: (i32, i32)) -> Result<()> {
        let id = self.id;
        request(|reply| Job::Invoke {
            id,
            command,
            anchor,
            reply,
        })
        .await
    }
}

impl Drop for ShellMenu {
    fn drop(&mut self) {
        let _ = send(Job::Release(self.id));
    }
}

/// Starts the Shell thread and loads extension DLLs before the first right-click.
/// Measured here: a cold first menu takes about 0.7 s, a warm one about 60 ms.
pub(crate) fn warm() {
    let _ = std::thread::Builder::new()
        .name("windows-shell-warm".into())
        .spawn(|| {
            let file = std::env::current_exe()?;
            let folder = file.parent().context("the app has no folder")?.to_path_buf();
            for target in [Target::Items(vec![file]), Target::Folder(folder)] {
                let id = next_id();
                let (reply, _) = async_channel::bounded(1);
                send(Job::Open {
                    id,
                    owner: 0,
                    target,
                    shift: false,
                    reply,
                })?;
                send(Job::Release(id))?;
            }
            anyhow::Ok(())
        });
}

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

type Reply<T> = async_channel::Sender<Result<T>>;

enum Job {
    Open {
        id: u64,
        owner: isize,
        target: Target,
        shift: bool,
        reply: Reply<Vec<Entry>>,
    },
    Expand {
        id: u64,
        branch: Branch,
        reply: Reply<Vec<Entry>>,
    },
    Invoke {
        id: u64,
        command: u32,
        anchor: (i32, i32),
        reply: Reply<()>,
    },
    Release(u64),
}

async fn request<T>(job: impl FnOnce(Reply<T>) -> Job) -> Result<T> {
    let (reply, response) = async_channel::bounded(1);
    send(job(reply))?;
    response
        .recv()
        .await
        .map_err(|_| anyhow!("the Windows Shell thread stopped"))?
}

struct ShellThread {
    jobs: mpsc::Sender<Job>,
    host: isize,
}

fn send(job: Job) -> Result<()> {
    static THREAD: OnceLock<std::result::Result<ShellThread, String>> = OnceLock::new();
    let thread = THREAD
        .get_or_init(start)
        .as_ref()
        .map_err(|error| anyhow!("{error}"))?;
    thread
        .jobs
        .send(job)
        .map_err(|_| anyhow!("the Windows Shell thread stopped"))?;
    // A window message, unlike a thread message, still arrives while a Shell
    // dialog runs its own modal loop on that thread.
    unsafe {
        PostMessageW(
            Some(HWND(thread.host as _)),
            WM_RUN_JOBS,
            WPARAM(0),
            LPARAM(0),
        )?
    };
    Ok(())
}

thread_local! {
    static JOBS: OnceCell<mpsc::Receiver<Job>> = const { OnceCell::new() };
    static MENUS: RefCell<HashMap<u64, Rc<BuiltMenu>>> = RefCell::new(HashMap::new());
}

fn start() -> std::result::Result<ShellThread, String> {
    let (jobs, queue) = mpsc::channel();
    let (ready, started) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("windows-shell".into())
        .spawn(move || {
            let host = host_window();
            let failed = host.is_err();
            let _ = ready.send(host.map_err(|error| format!("{error:#}")));
            if failed {
                return;
            }
            JOBS.with(|jobs| jobs.set(queue).ok());
            let mut message = MSG::default();
            while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        })
        .map_err(|error| error.to_string())?;
    let host = started
        .recv()
        .map_err(|_| "the Windows Shell thread did not start".to_owned())??;
    Ok(ShellThread { jobs, host })
}

fn host_window() -> Result<isize> {
    // Shell extensions expect an OLE apartment for drag and drop and the clipboard.
    unsafe { OleInitialize(None)? };
    let instance = HINSTANCE(unsafe {
        windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(null())
    });
    let class = WNDCLASSW {
        lpfnWndProc: Some(host_proc),
        hInstance: instance,
        lpszClassName: w!("BoxBoxShellHost"),
        ..Default::default()
    };
    ensure!(
        unsafe { RegisterClassW(&class) } != 0,
        "could not register the Shell host window"
    );
    let host = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            None,
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )?
    };
    Ok(host.0 as isize)
}

unsafe extern "system" fn host_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message != WM_RUN_JOBS {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // Jobs nest when an extension pumps messages. No job holds the MENUS borrow
    // across a Shell call, and the Rc keeps a menu alive while it is in use.
    while let Some(job) = JOBS.with(|jobs| jobs.get().and_then(|queue| queue.try_recv().ok())) {
        run(job);
    }
    LRESULT(0)
}

fn run(job: Job) {
    match job {
        Job::Open {
            id,
            owner,
            target,
            shift,
            reply,
        } => {
            let result = BuiltMenu::new(HWND(owner as _), &target, shift).map(|menu| {
                let entries = menu.entries();
                MENUS.with_borrow_mut(|menus| menus.insert(id, Rc::new(menu)));
                entries
            });
            let _ = reply.try_send(result);
        }
        Job::Expand { id, branch, reply } => {
            let _ = reply.try_send(built(id).map(|menu| menu.expand(branch)));
        }
        Job::Invoke {
            id,
            command,
            anchor,
            reply,
        } => {
            let _ = reply.try_send(built(id).and_then(|menu| menu.invoke(command, anchor)));
        }
        Job::Release(id) => {
            MENUS.with_borrow_mut(|menus| menus.remove(&id));
        }
    }
}

fn built(id: u64) -> Result<Rc<BuiltMenu>> {
    MENUS
        .with_borrow(|menus| menus.get(&id).cloned())
        .context("this Windows menu is closed")
}

enum Selection {
    Folder(Pidl),
    Items(Vec<Pidl>),
}

impl Selection {
    fn parse(target: &Target) -> Result<Self> {
        Ok(match target {
            Target::Folder(path) => Self::Folder(Pidl::parse(path)?),
            Target::Items(paths) => Self::Items(
                paths
                    .iter()
                    .map(|path| Pidl::parse(path))
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

struct Pidl(NonNull<ITEMIDLIST>);

impl Pidl {
    fn parse(path: &Path) -> Result<Self> {
        let name = super::shell_path(path);
        let mut pidl = null_mut();
        unsafe { SHParseDisplayName(PCWSTR(name.as_ptr()), None, &mut pidl, 0, None)? };
        Ok(Self(
            NonNull::new(pidl).context("the Shell returned an empty item ID")?,
        ))
    }
}

impl Drop for Pidl {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.as_ptr().cast())) };
    }
}

fn shell_menu(owner: HWND, selection: &Selection) -> Result<IContextMenu> {
    match selection {
        Selection::Folder(folder) => {
            let shell: IShellFolder = unsafe { SHBindToObject(None, folder.0.as_ptr(), None)? };
            Ok(unsafe { shell.CreateViewObject(owner)? })
        }
        Selection::Items(children) => {
            let (first, rest) = children.split_first().context("no files selected")?;
            let mut relative = null_mut();
            let folder: IShellFolder =
                unsafe { SHBindToParent(first.0.as_ptr(), Some(&mut relative))? };
            let mut relatives = vec![relative.cast_const()];
            if !rest.is_empty() {
                // GetUIObjectOf requires child IDs from a single Shell folder.
                let parent = unsafe { ILClone(first.0.as_ptr()) };
                ensure!(!parent.is_null(), "could not copy the Shell item ID");
                let same_folder = unsafe {
                    let removed = ILRemoveLastID(Some(parent)).as_bool();
                    let same = removed
                        && rest
                            .iter()
                            .all(|child| ILIsParent(parent, child.0.as_ptr(), true).as_bool());
                    ILFree(Some(parent));
                    same
                };
                ensure!(
                    same_folder,
                    "select items from the same folder to use Windows options"
                );
                relatives.extend(
                    rest.iter()
                        .map(|child| unsafe { ILFindLastID(child.0.as_ptr()).cast_const() }),
                );
            }
            Ok(unsafe { folder.GetUIObjectOf(owner, &relatives, None)? })
        }
    }
}

struct Menu(HMENU);

impl Menu {
    fn for_shell(shell: &IContextMenu, flags: u32) -> Result<Self> {
        let menu = Self(unsafe { CreatePopupMenu()? });
        unsafe {
            shell
                .QueryContextMenu(menu.0, 0, FIRST_COMMAND, LAST_COMMAND, flags)
                .ok()?
        };
        Ok(menu)
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        let _ = unsafe { DestroyMenu(self.0) };
    }
}

/// Lets a lazy submenu fill itself before its rows are read.
enum MenuMessages {
    WithResult(IContextMenu3),
    WithoutResult(IContextMenu2),
}

impl MenuMessages {
    fn for_shell(shell: &IContextMenu) -> Option<Self> {
        shell
            .cast::<IContextMenu3>()
            .map(Self::WithResult)
            .or_else(|_| shell.cast::<IContextMenu2>().map(Self::WithoutResult))
            .ok()
    }

    fn init_popup(&self, branch: Branch) {
        let (wparam, lparam) = (WPARAM(branch.menu as usize), LPARAM(branch.index as isize));
        let _ = match self {
            Self::WithResult(shell) => unsafe {
                shell.HandleMenuMsg2(WM_INITMENUPOPUP, wparam, lparam, None)
            },
            Self::WithoutResult(shell) => unsafe {
                shell.HandleMenuMsg(WM_INITMENUPOPUP, wparam, lparam)
            },
        };
    }
}

/// One Shell menu on the Shell thread. Its command IDs and submenus belong to it.
struct BuiltMenu {
    menu: Menu,
    shell: IContextMenu,
    messages: Option<MenuMessages>,
    // Kept alive only because the Shell menu refers to these item IDs.
    _selection: Selection,
    owner: HWND,
    shift: bool,
    /// Every item is a folder, which BoxBox opens itself.
    folders: bool,
}

impl BuiltMenu {
    fn new(owner: HWND, target: &Target, shift: bool) -> Result<Self> {
        let folders = matches!(target, Target::Items(paths) if paths.iter().all(|path| path.is_dir()));
        let selection = Selection::parse(target)?;
        let shell = shell_menu(owner, &selection)?;
        let flags = if shift { CMF_EXTENDEDVERBS } else { CMF_NORMAL };
        let menu = Menu::for_shell(&shell, flags)?;
        let messages = MenuMessages::for_shell(&shell);
        Ok(Self {
            menu,
            shell,
            messages,
            _selection: selection,
            owner,
            shift,
            folders,
        })
    }

    /// The rows BoxBox does not already show itself. Its own rows keep BoxBox's
    /// clipboard, transfer and confirmation behavior.
    fn entries(&self) -> Vec<Entry> {
        let mut send_to: Option<Vec<String>> = None;
        let mut entries = menu_entries(self.menu.0);
        entries.retain(|entry| match entry {
            Entry::Action { id, .. } => !self.verb(*id).is_some_and(|verb| is_boxbox_verb(&verb, self.folders)),
            Entry::Submenu { branch, .. } => match self.verb(branch.command) {
                Some(verb) => !is_boxbox_verb(&verb, self.folders),
                // "Send to" has no verb. Its rows are the items of the SendTo folder.
                None => !self.lists_any(*branch, send_to.get_or_insert_with(send_to_names)),
            },
            Entry::Separator => true,
        });
        tidy_separators(&mut entries);
        entries
    }

    /// The language-independent name of a command, when its handler gives one.
    fn verb(&self, command: u32) -> Option<String> {
        if !(FIRST_COMMAND..=LAST_COMMAND).contains(&command) {
            return None;
        }
        let mut name = [0u16; 64];
        unsafe {
            self.shell.GetCommandString(
                (command - FIRST_COMMAND) as usize,
                GCS_VERBW,
                None,
                PSTR(name.as_mut_ptr().cast()),
                name.len() as u32,
            )
        }
        .ok()?;
        let end = name.iter().position(|&ch| ch == 0).unwrap_or(name.len());
        String::from_utf16(&name[..end]).ok().filter(|verb| !verb.is_empty())
    }

    fn lists_any(&self, branch: Branch, names: &[String]) -> bool {
        !names.is_empty()
            && self.expand(branch).iter().any(|entry| match entry {
                Entry::Action { label, .. } | Entry::Submenu { label, .. } => names.contains(label),
                Entry::Separator => false,
            })
    }

    fn expand(&self, branch: Branch) -> Vec<Entry> {
        if let Some(messages) = &self.messages {
            messages.init_popup(branch);
        }
        menu_entries(HMENU(branch.menu as _))
    }

    fn invoke(&self, command: u32, anchor: (i32, i32)) -> Result<()> {
        ensure!(
            (FIRST_COMMAND..=LAST_COMMAND).contains(&command),
            "invalid Shell command ID"
        );
        let mut point = POINT {
            x: anchor.0,
            y: anchor.1,
        };
        unsafe { ClientToScreen(self.owner, &mut point).ok()? };
        let verb = usize::try_from(command - FIRST_COMMAND)?;
        let mut mask = SEE_MASK_UNICODE | CMIC_MASK_PTINVOKE;
        if self.shift {
            mask |= CMIC_MASK_SHIFT_DOWN;
        }
        if unsafe { GetKeyState(i32::from(VK_CONTROL.0)) } < 0 {
            mask |= CMIC_MASK_CONTROL_DOWN;
        }
        let info = CMINVOKECOMMANDINFOEX {
            cbSize: size_of::<CMINVOKECOMMANDINFOEX>() as u32,
            fMask: mask,
            hwnd: self.owner,
            lpVerb: PCSTR(verb as _),
            lpVerbW: PCWSTR(verb as _),
            nShow: SW_SHOWNORMAL.0,
            ptInvoke: point,
            ..Default::default()
        };
        unsafe {
            self.shell
                .InvokeCommand(std::ptr::from_ref(&info).cast::<CMINVOKECOMMANDINFO>())?
        };
        Ok(())
    }
}

/// BoxBox shows these as its own rows: clipboard, rename, trash, properties,
/// Open With and Favorites. `pintohome*` pins to Explorer's Quick Access, which
/// BoxBox's Favorites would be confused with. For `folders`, BoxBox's Open and
/// Open in New Tab replace the Shell's `open` and `explore`, which start Explorer.
fn is_boxbox_verb(verb: &str, folders: bool) -> bool {
    const ROWS: [&str; 9] = [
        "cut",
        "copy",
        "paste",
        "rename",
        "delete",
        "properties",
        "openas",
        "pintohome",
        "pintohomefile",
    ];
    const FOLDER_ROWS: [&str; 2] = ["open", "explore"];
    let named = |names: &[&str]| names.iter().any(|name| verb.eq_ignore_ascii_case(name));
    named(&ROWS) || (folders && named(&FOLDER_ROWS))
}

/// Drops leading, trailing and repeated separators left by removed rows.
fn tidy_separators(entries: &mut Vec<Entry>) {
    let mut previous_is_separator = true;
    entries.retain(|entry| {
        let separator = matches!(entry, Entry::Separator);
        let keep = !separator || !previous_is_separator;
        previous_is_separator = separator;
        keep
    });
    if matches!(entries.last(), Some(Entry::Separator)) {
        entries.pop();
    }
}

/// The display names of the user's SendTo folder, as the Send to submenu lists them.
fn send_to_names() -> Vec<String> {
    let read = || -> Result<Vec<String>> {
        let folder: IShellItem = unsafe { SHGetKnownFolderItem(&FOLDERID_SendTo, KF_FLAG_DEFAULT, None)? };
        let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems)? };
        let mut names = Vec::new();
        loop {
            let mut item = [None];
            let mut fetched = 0;
            unsafe { items.Next(&mut item, Some(&mut fetched))? };
            let Some(item) = item[0].take().filter(|_| fetched == 1) else {
                break;
            };
            let name = unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY)? };
            names.push(menu_label(&String::from_utf16_lossy(unsafe { name.as_wide() })));
            unsafe { CoTaskMemFree(Some(name.0.cast())) };
        }
        Ok(names)
    };
    read().unwrap_or_else(|error| {
        log::warn!("Could not read the Send to folder: {error:#}");
        Vec::new()
    })
}

/// A row's bitmap. Shell menus set it as the item bitmap or, as 7-Zip does,
/// as the unchecked check mark.
fn row_icon(item: &MENUITEMINFOW) -> Option<Icon> {
    // 0, -1 (HBMMENU_CALLBACK, drawn by the handler) and 1..=11 are not bitmaps.
    let real = |bitmap: HBITMAP| !(-1..=11).contains(&(bitmap.0 as isize));
    [item.hbmpItem, item.hbmpUnchecked]
        .into_iter()
        .filter(|bitmap| real(*bitmap))
        .find_map(bitmap_icon)
}

fn bitmap_icon(bitmap: HBITMAP) -> Option<Icon> {
    let mut header = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            bitmap.into(),
            size_of::<BITMAP>() as i32,
            Some(std::ptr::from_mut(&mut header).cast()),
        )
    };
    let (width, height) = (header.bmWidth, header.bmHeight.abs());
    if read == 0 || !(1..=64).contains(&width) || !(1..=64).contains(&height) {
        return None;
    }
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // Negative: top-down rows.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; 4 * width as usize * height as usize];
    let screen = unsafe { GetDC(None) };
    let lines = unsafe {
        GetDIBits(
            screen,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, screen) };
    if lines == 0 {
        return None;
    }
    let (pixels, _) = bgra.as_chunks_mut::<4>();
    if header.bmBitsPixel < 32 || pixels.iter().all(|pixel| pixel[3] == 0) {
        // No alpha channel: the bitmap is opaque.
        pixels.iter_mut().for_each(|pixel| pixel[3] = 255);
    } else {
        // 32-bit menu bitmaps are premultiplied; GPUI takes straight alpha.
        for pixel in pixels {
            let alpha = u16::from(pixel[3]);
            if (1..255).contains(&alpha) {
                for channel in &mut pixel[..3] {
                    *channel = (u16::from(*channel) * 255 / alpha).min(255) as u8;
                }
            }
        }
    }
    Some(Icon {
        width: width as u32,
        height: height as u32,
        bgra,
    })
}

fn menu_entries(menu: HMENU) -> Vec<Entry> {
    let count = unsafe { GetMenuItemCount(Some(menu)) }.max(0) as u32;
    let mut entries = Vec::new();
    for index in 0..count {
        let mut item = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_STRING
                | MIIM_FTYPE
                | MIIM_ID
                | MIIM_STATE
                | MIIM_SUBMENU
                | MIIM_BITMAP
                | MIIM_CHECKMARKS,
            ..Default::default()
        };
        if unsafe { GetMenuItemInfoW(menu, index, true, &mut item) }.is_err() {
            continue;
        }
        if item.fType.contains(MFT_SEPARATOR) {
            if entries
                .last()
                .is_some_and(|entry| !matches!(entry, Entry::Separator))
            {
                entries.push(Entry::Separator);
            }
            continue;
        }
        if item.cch == 0 {
            // Owner-drawn entries without a text label cannot be shown as rows.
            continue;
        }
        let mut text = vec![0; item.cch as usize + 1];
        item.dwTypeData = PWSTR(text.as_mut_ptr());
        item.cch += 1;
        if unsafe { GetMenuItemInfoW(menu, index, true, &mut item) }.is_err() {
            continue;
        }
        let text = String::from_utf16_lossy(&text[..item.cch as usize]);
        let (label, shortcut) = text
            .split_once('\t')
            .map_or((text.as_str(), None), |(label, keys)| {
                (label, Some(keys.to_owned()))
            });
        let label = menu_label(label);
        if label.is_empty() {
            continue;
        }
        let disabled = item.fState.0 & MFS_DISABLED.0 != 0;
        let icon = row_icon(&item);
        if !item.hSubMenu.is_invalid() {
            let branch = Branch {
                menu: item.hSubMenu.0 as isize,
                index,
                command: item.wID,
            };
            entries.push(Entry::Submenu {
                branch,
                label,
                icon,
                disabled,
            });
        } else if (FIRST_COMMAND..=LAST_COMMAND).contains(&item.wID) {
            entries.push(Entry::Action {
                id: item.wID,
                label,
                icon,
                shortcut,
                disabled,
            });
        }
    }
    if matches!(entries.last(), Some(Entry::Separator)) {
        entries.pop();
    }
    entries
}

fn menu_label(label: &str) -> String {
    let mut chars = label.chars().peekable();
    let mut plain = String::new();
    while let Some(ch) = chars.next() {
        if ch == '&' && chars.peek() != Some(&'&') {
            continue;
        }
        if ch == '&' {
            chars.next();
        }
        plain.push(ch);
    }
    plain.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    };

    fn wait<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
                return value;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn shell_thread_builds_and_expands_menus_for_files_and_folder_background() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let first = folder.path().join("first.txt");
        let second = folder.path().join("second.txt");
        std::fs::write(&first, "")?;
        std::fs::write(&second, "")?;

        let targets = [
            Target::Items(vec![first.clone()]),
            Target::Items(vec![first, second]),
            Target::Folder(folder.path().to_path_buf()),
        ];
        for target in targets {
            let (menu, entries) = wait(ShellMenu::open(0, target, false))?;
            assert!(!entries.is_empty());
            if let Some(branch) = entries.iter().find_map(|entry| match entry {
                Entry::Submenu { branch, .. } => Some(*branch),
                _ => None,
            }) {
                wait(menu.expand(branch))?;
            }
            drop(menu);
        }
        Ok(())
    }

    #[test]
    fn file_menus_leave_out_rows_boxbox_already_has() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let file = folder.path().join("notes.txt");
        std::fs::write(&file, "")?;
        let inner = folder.path().join("inner");
        std::fs::create_dir(&inner)?;
        crate::windows::with_shell(move || {
            let names = send_to_names();
            assert!(!names.is_empty(), "this Windows profile has a SendTo folder");
            for (item, folders) in [(file, false), (inner, true)] {
                let menu = BuiltMenu::new(HWND(null_mut()), &Target::Items(vec![item]), false)?;
                for entry in menu.entries() {
                    let command = match &entry {
                        Entry::Action { id, .. } => *id,
                        Entry::Submenu { branch, .. } => {
                            assert!(!menu.lists_any(*branch, &names), "Send to is BoxBox's row");
                            branch.command
                        }
                        Entry::Separator => continue,
                    };
                    if let Some(verb) = menu.verb(command) {
                        assert!(!is_boxbox_verb(&verb, folders), "{verb} is BoxBox's row");
                    }
                }
            }
            Ok(())
        })
    }

    #[test]
    fn premultiplied_menu_bitmaps_become_straight_bgra() -> Result<()> {
        use windows::Win32::Graphics::Gdi::{CreateDIBSection, DeleteObject};
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 2,
                biHeight: -1,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = null_mut();
        let bitmap = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
        // BGRA: a half-transparent red, premultiplied, then an opaque blue.
        unsafe { std::ptr::copy_nonoverlapping([0u8, 0, 128, 128, 255, 0, 0, 255].as_ptr(), bits.cast(), 8) };
        let icon = bitmap_icon(bitmap);
        let _ = unsafe { DeleteObject(bitmap.into()) };
        let icon = icon.context("the bitmap converts")?;
        assert_eq!((icon.width, icon.height), (2, 1));
        assert_eq!(icon.bgra, [0, 0, 255, 128, 255, 0, 0, 255]);
        Ok(())
    }

    #[test]
    fn a_released_menu_refuses_commands() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let (menu, _) = wait(ShellMenu::open(
            0,
            Target::Folder(folder.path().to_path_buf()),
            false,
        ))?;
        let stale = ShellMenu { id: menu.id };
        drop(menu);
        let error = wait(stale.invoke(FIRST_COMMAND, (0, 0))).unwrap_err();
        assert!(error.to_string().contains("closed"), "{error:#}");
        Ok(())
    }

    #[test]
    fn reads_shell_labels_command_ids_and_submenus() -> Result<()> {
        let root = Menu(unsafe { CreatePopupMenu()? });
        let submenu = Menu(unsafe { CreatePopupMenu()? });
        unsafe {
            AppendMenuW(root.0, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(root.0, MF_STRING | MF_GRAYED, 7, w!("&Cut && copy\tCtrl+X"))?;
            AppendMenuW(root.0, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(root.0, MF_POPUP, submenu.0 .0 as usize, w!("Send &to"))?;
        }
        let handle = submenu.0;
        std::mem::forget(submenu); // The root menu now owns this submenu.
        unsafe { AppendMenuW(handle, MF_STRING, 42, w!("Desktop"))? };
        let entries = menu_entries(root.0);
        let [Entry::Action {
            id: 7,
            label,
            shortcut,
            disabled: true,
            ..
        }, Entry::Separator, Entry::Submenu {
            label: title,
            branch,
            disabled: false,
            ..
        }] = entries.as_slice()
        else {
            panic!("expected the Shell command, separator and submenu");
        };
        assert_eq!(label, "Cut & copy");
        assert_eq!(shortcut.as_deref(), Some("Ctrl+X"));
        assert_eq!(title, "Send to");
        assert!(
            matches!(menu_entries(HMENU(branch.menu as _)).as_slice(), [Entry::Action { id: 42, label, .. }] if label == "Desktop")
        );
        Ok(())
    }
}
