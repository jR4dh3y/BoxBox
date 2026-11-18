//! Hosts the classic Windows Shell menu for a local selection or folder background.

use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::ptr::{null_mut, NonNull};
use std::sync::Mutex;

use anyhow::{ensure, Context as _, Result};
use windows::core::{Interface, PCSTR, PCWSTR, PSTR, PWSTR};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    System::Com::CoTaskMemFree,
    UI::{
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL},
        Shell::{
            Common::ITEMIDLIST, DefSubclassProc, IContextMenu, IContextMenu2, IContextMenu3,
            ILClone, ILFindLastID, ILFree, ILIsParent, ILRemoveLastID, IShellFolder,
            RemoveWindowSubclass, SHBindToObject, SHBindToParent, SHParseDisplayName,
            SetWindowSubclass, CMF_EXTENDEDVERBS, CMF_NORMAL, CMIC_MASK_CONTROL_DOWN,
            CMIC_MASK_PTINVOKE, CMIC_MASK_SHIFT_DOWN, CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX,
            GCS_VERBW, SEE_MASK_UNICODE,
        },
        WindowsAndMessaging::{
            CreatePopupMenu, DestroyMenu, GetForegroundWindow, GetMenuItemCount, GetMenuItemInfoW,
            TrackPopupMenuEx, HMENU, MENUITEMINFOW, MFS_DISABLED, MFT_SEPARATOR, MIIM_FTYPE,
            MIIM_ID, MIIM_STATE, MIIM_STRING, MIIM_SUBMENU, SW_SHOWNORMAL, TPM_RETURNCMD,
            TPM_RIGHTBUTTON, WM_DRAWITEM, WM_INITMENUPOPUP, WM_MEASUREITEM, WM_MENUCHAR,
        },
    },
};

const FIRST_COMMAND: u32 = 1;
const LAST_COMMAND: u32 = 0x7fff;
const MENU_SUBCLASS: usize = 1;
static ACTIVE_MENU: Mutex<()> = Mutex::new(());

pub(crate) struct Target(Selection);

enum Selection {
    Folder(Pidl),
    Items(Vec<Pidl>),
}

impl Target {
    pub(crate) fn folder(path: &Path) -> Result<Self> {
        super::with_shell(|| Ok(Self(Selection::Folder(Pidl::parse(path)?))))
    }

    pub(crate) fn items(paths: &[PathBuf]) -> Result<Self> {
        super::with_shell(|| {
            let items = paths
                .iter()
                .map(|path| Pidl::parse(path))
                .collect::<Result<_>>()?;
            Ok(Self(Selection::Items(items)))
        })
    }
}

struct Pidl(NonNull<ITEMIDLIST>);

// SAFETY: A PIDL is an owned byte list allocated by the thread-safe COM task allocator.
unsafe impl Send for Pidl {}

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

fn shell_menu(owner: HWND, target: &Target) -> Result<IContextMenu> {
    match &target.0 {
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

    fn dispatch(&self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match self {
            Self::WithResult(shell) => {
                let mut result = LRESULT(0);
                unsafe { shell.HandleMenuMsg2(message, wparam, lparam, Some(&mut result)) }.ok()?;
                Some(result)
            }
            Self::WithoutResult(shell) if message != WM_MENUCHAR => {
                unsafe { shell.HandleMenuMsg(message, wparam, lparam) }.ok()?;
                Some(LRESULT(0))
            }
            _ => None,
        }
    }
}

pub(crate) enum Entry {
    Action {
        id: u32,
        label: String,
        shortcut: Option<String>,
        disabled: bool,
    },
    Submenu {
        branch: Branch,
        label: String,
        disabled: bool,
    },
    Separator,
}

#[derive(Clone, Copy)]
pub(crate) struct Branch {
    menu: HMENU,
    index: u32,
}

/// The native IDs and lazy submenus are valid only for this menu instance.
pub(crate) struct ShellMenu {
    menu: Menu,
    shell: IContextMenu,
    messages: Option<MenuMessages>,
    target: Target,
    owner: HWND,
    shift: bool,
}

impl ShellMenu {
    pub(crate) fn new(owner: isize, target: Target, shift: bool) -> Result<Self> {
        let owner = HWND(owner as _);
        let shell = shell_menu(owner, &target)?;
        let flags = if shift { CMF_EXTENDEDVERBS } else { CMF_NORMAL };
        let menu = Menu::for_shell(&shell, flags)?;
        let messages = MenuMessages::for_shell(&shell);
        Ok(Self {
            menu,
            shell,
            messages,
            target,
            owner,
            shift,
        })
    }

    pub(crate) fn entries(&self) -> Vec<Entry> {
        let mut entries = menu_entries(self.menu.0);
        if matches!(self.target.0, Selection::Items(_)) {
            // Keep BoxBox's clipboard and confirmation behavior on its standard rows.
            let mut previous_is_separator = true;
            entries.retain(|entry| {
                if matches!(entry, Entry::Action { id, .. } if self.has_boxbox_action(*id)) {
                    return false;
                }
                let separator = matches!(entry, Entry::Separator);
                let keep = !separator || !previous_is_separator;
                previous_is_separator = separator;
                keep
            });
            if matches!(entries.last(), Some(Entry::Separator)) {
                entries.pop();
            }
        }
        entries
    }

    fn has_boxbox_action(&self, command: u32) -> bool {
        let mut name = [0u16; 64];
        let result = unsafe {
            self.shell.GetCommandString(
                (command - FIRST_COMMAND) as usize,
                GCS_VERBW,
                None,
                PSTR(name.as_mut_ptr().cast()),
                name.len() as u32,
            )
        };
        if result.is_err() {
            return false;
        }
        let end = name.iter().position(|&ch| ch == 0).unwrap_or(name.len());
        String::from_utf16(&name[..end]).is_ok_and(|verb| {
            ["cut", "copy", "rename", "delete", "properties"]
                .iter()
                .any(|name| verb.eq_ignore_ascii_case(name))
        })
    }

    pub(crate) fn expand(&self, branch: Branch) -> Vec<Entry> {
        if let Some(messages) = &self.messages {
            let _ = messages.dispatch(
                WM_INITMENUPOPUP,
                WPARAM(branch.menu.0 as usize),
                LPARAM(branch.index as isize),
            );
        }
        menu_entries(branch.menu)
    }

    pub(crate) fn invoke(&self, command: u32, anchor: (i32, i32)) -> Result<()> {
        if unsafe { GetForegroundWindow() } != self.owner {
            return Ok(());
        }
        invoke(
            &self.shell,
            self.owner,
            screen_point(self.owner, anchor)?,
            command,
            self.shift,
        )
    }

    pub(crate) fn popup(&self, anchor: (i32, i32)) -> Result<()> {
        show_classic(self, anchor)
    }
}

fn menu_entries(menu: HMENU) -> Vec<Entry> {
    let count = unsafe { GetMenuItemCount(Some(menu)) }.max(0) as u32;
    let mut entries = Vec::new();
    for index in 0..count {
        let mut item = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_STRING | MIIM_FTYPE | MIIM_ID | MIIM_STATE | MIIM_SUBMENU,
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
            // Owner-drawn entries without a text label remain available in the classic menu.
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
        if !item.hSubMenu.is_invalid() {
            let branch = Branch {
                menu: item.hSubMenu,
                index,
            };
            entries.push(Entry::Submenu {
                branch,
                label,
                disabled,
            });
        } else if (FIRST_COMMAND..=LAST_COMMAND).contains(&item.wID) {
            entries.push(Entry::Action {
                id: item.wID,
                label,
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

struct MenuSubclass(HWND);

impl Drop for MenuSubclass {
    fn drop(&mut self) {
        let _ = unsafe { RemoveWindowSubclass(self.0, Some(menu_proc), MENU_SUBCLASS) };
    }
}

unsafe extern "system" fn menu_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    if matches!(message, WM_INITMENUPOPUP | WM_MENUCHAR)
        || (wparam.0 == 0 && matches!(message, WM_DRAWITEM | WM_MEASUREITEM))
    {
        // TrackPopupMenuEx dispatches on the owner thread while this borrow is live.
        let messages = unsafe { &*(data as *const MenuMessages) };
        if let Some(result) = messages.dispatch(message, wparam, lparam) {
            return result;
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn show_classic(shell: &ShellMenu, anchor: (i32, i32)) -> Result<()> {
    // GPUI initializes OLE on the window thread. Subclassing must run on that thread.
    let owner = shell.owner;
    if unsafe { GetForegroundWindow() } != owner {
        return Ok(());
    }
    // The native menu pumps messages and can re-enter the foreground executor.
    let Ok(_active) = ACTIVE_MENU.try_lock() else {
        return Ok(());
    };
    let point = screen_point(owner, anchor)?;
    let subclass = if let Some(messages) = &shell.messages {
        let data = std::ptr::from_ref(messages) as usize;
        unsafe { SetWindowSubclass(owner, Some(menu_proc), MENU_SUBCLASS, data).ok()? };
        Some(MenuSubclass(owner))
    } else {
        None
    };
    let command = unsafe {
        TrackPopupMenuEx(
            shell.menu.0,
            (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
            point.x,
            point.y,
            owner,
            None,
        )
    }
    .0;
    drop(subclass);
    if command == 0 {
        return Ok(());
    }
    invoke(
        &shell.shell,
        owner,
        point,
        u32::try_from(command)?,
        shell.shift,
    )
}

fn screen_point(owner: HWND, anchor: (i32, i32)) -> Result<POINT> {
    let mut point = POINT {
        x: anchor.0,
        y: anchor.1,
    };
    unsafe { ClientToScreen(owner, &mut point).ok()? };
    Ok(point)
}

fn invoke(
    shell: &IContextMenu,
    owner: HWND,
    point: POINT,
    command: u32,
    shift: bool,
) -> Result<()> {
    ensure!(
        (FIRST_COMMAND..=LAST_COMMAND).contains(&command),
        "invalid Shell command ID"
    );
    let verb = usize::try_from(command - FIRST_COMMAND)?;
    let mut mask = SEE_MASK_UNICODE | CMIC_MASK_PTINVOKE;
    if shift {
        mask |= CMIC_MASK_SHIFT_DOWN;
    }
    if unsafe { GetKeyState(i32::from(VK_CONTROL.0)) } < 0 {
        mask |= CMIC_MASK_CONTROL_DOWN;
    }
    let info = CMINVOKECOMMANDINFOEX {
        cbSize: size_of::<CMINVOKECOMMANDINFOEX>() as u32,
        fMask: mask,
        hwnd: owner,
        lpVerb: PCSTR(verb as _),
        lpVerbW: PCWSTR(verb as _),
        nShow: SW_SHOWNORMAL.0,
        ptInvoke: point,
        ..Default::default()
    };
    unsafe { shell.InvokeCommand(std::ptr::from_ref(&info).cast::<CMINVOKECOMMANDINFO>())? };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    };

    #[test]
    fn shell_populates_menus_for_files_and_folder_background() -> Result<()> {
        let folder = tempfile::tempdir()?;
        let first = folder.path().join("first.txt");
        let second = folder.path().join("second.txt");
        std::fs::write(&first, "")?;
        std::fs::write(&second, "")?;

        let targets = [
            Target::items(std::slice::from_ref(&first))?,
            Target::items(&[first, second])?,
            Target::folder(folder.path())?,
        ];
        crate::windows::with_shell(move || {
            for target in targets {
                assert!(!ShellMenu::new(0, target, false)?.entries().is_empty());
            }
            Ok(())
        })
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
        }, Entry::Separator, Entry::Submenu {
            label: title,
            branch,
            disabled: false,
        }] = entries.as_slice()
        else {
            panic!("expected the Shell command, separator and submenu");
        };
        assert_eq!(label, "Cut & copy");
        assert_eq!(shortcut.as_deref(), Some("Ctrl+X"));
        assert_eq!(title, "Send to");
        assert!(
            matches!(menu_entries(branch.menu).as_slice(), [Entry::Action { id: 42, label, .. }] if label == "Desktop")
        );
        Ok(())
    }
}
