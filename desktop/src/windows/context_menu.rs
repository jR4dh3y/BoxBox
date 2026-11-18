//! Hosts the classic Windows Shell menu for a local selection or folder background.

use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::ptr::{null_mut, NonNull};
use std::sync::Mutex;

use anyhow::{ensure, Context as _, Result};
use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ClientToScreen,
    System::Com::CoTaskMemFree,
    UI::{
        Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT},
        Shell::{
            Common::ITEMIDLIST, DefSubclassProc, IContextMenu, IContextMenu2, IContextMenu3,
            ILClone, ILFindLastID, ILFree, ILIsParent, ILRemoveLastID, IShellFolder,
            RemoveWindowSubclass, SHBindToObject, SHBindToParent, SHParseDisplayName,
            SetWindowSubclass, CMF_EXTENDEDVERBS, CMF_NORMAL, CMIC_MASK_CONTROL_DOWN,
            CMIC_MASK_PTINVOKE, CMIC_MASK_SHIFT_DOWN, CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX,
            SEE_MASK_UNICODE,
        },
        WindowsAndMessaging::{
            CreatePopupMenu, DestroyMenu, GetForegroundWindow, TrackPopupMenuEx, HMENU,
            SW_SHOWNORMAL, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_DRAWITEM, WM_INITMENUPOPUP,
            WM_MEASUREITEM, WM_MENUCHAR,
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

struct MenuSubclass(HWND);

impl Drop for MenuSubclass {
    fn drop(&mut self) {
        unsafe { RemoveWindowSubclass(self.0, Some(menu_proc), MENU_SUBCLASS) };
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
        match unsafe { &*(data as *const MenuMessages) } {
            MenuMessages::WithResult(shell) => {
                let mut result = LRESULT(0);
                if unsafe { shell.HandleMenuMsg2(message, wparam, lparam, Some(&mut result)) }
                    .is_ok()
                {
                    return result;
                }
            }
            MenuMessages::WithoutResult(shell) if message != WM_MENUCHAR => {
                if unsafe { shell.HandleMenuMsg(message, wparam, lparam) }.is_ok() {
                    return LRESULT(0);
                }
            }
            _ => {}
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(crate) fn show(owner: isize, anchor: (i32, i32), target: &Target) -> Result<()> {
    // GPUI initializes OLE on the window thread. Subclassing must run on that thread.
    let owner = HWND(owner as _);
    if unsafe { GetForegroundWindow() } != owner {
        return Ok(());
    }
    // The native menu pumps messages and can re-enter the foreground executor.
    let Ok(_active) = ACTIVE_MENU.try_lock() else {
        return Ok(());
    };
    let mut point = POINT {
        x: anchor.0,
        y: anchor.1,
    };
    unsafe { ClientToScreen(owner, &mut point).ok()? };
    let shell = shell_menu(owner, target)?;
    let shift = unsafe { GetKeyState(i32::from(VK_SHIFT.0)) } < 0;
    let flags = if shift { CMF_EXTENDEDVERBS } else { CMF_NORMAL };
    let menu = Menu::for_shell(&shell, flags)?;
    let messages = shell
        .cast::<IContextMenu3>()
        .map(MenuMessages::WithResult)
        .or_else(|_| {
            shell
                .cast::<IContextMenu2>()
                .map(MenuMessages::WithoutResult)
        })
        .ok();
    let subclass = if let Some(messages) = &messages {
        let data = std::ptr::from_ref(messages) as usize;
        unsafe { SetWindowSubclass(owner, Some(menu_proc), MENU_SUBCLASS, data).ok()? };
        Some(MenuSubclass(owner))
    } else {
        None
    };
    let command = unsafe {
        TrackPopupMenuEx(
            menu.0,
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
    let command = u32::try_from(command)?;
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
    use windows::Win32::UI::WindowsAndMessaging::GetMenuItemCount;

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
                let shell = shell_menu(HWND::default(), &target)?;
                let menu = Menu::for_shell(&shell, CMF_NORMAL)?;
                assert!(unsafe { GetMenuItemCount(Some(menu.0)) } > 0);
            }
            Ok(())
        })
    }
}
