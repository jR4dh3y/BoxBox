use gpui::{Context, Window};
use windows_sys::Win32::UI::Shell::{SHOpenWithDialog, OAIF_EXEC, OPENASINFO};

use crate::{api::FileInfo, workspace::Workspace};

impl Workspace {
    pub(crate) fn open_with_dialog(
        &mut self,
        item: FileInfo,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let api = self.pane().api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let path = api.local_path(&item.path)?.ok_or_else(|| {
                        anyhow::anyhow!("Open With works for files on this computer")
                    })?;
                    let file = crate::windows::shell_path(&path);
                    crate::windows::with_shell(|| {
                        let info = OPENASINFO {
                            pcszFile: file.as_ptr(),
                            pcszClass: std::ptr::null(),
                            oaifInFlags: OAIF_EXEC,
                        };
                        let result = unsafe { SHOpenWithDialog(std::ptr::null_mut(), &info) };
                        // Closing the dialog returns HRESULT_FROM_WIN32(ERROR_CANCELLED).
                        if result < 0 && result != 0x800704C7_u32 as i32 {
                            return Err(std::io::Error::from_raw_os_error(result).into());
                        }
                        Ok(())
                    })
                })
                .await;
            if let Err(error) = result {
                this.update(cx, |this, cx| {
                    this.report(Err(format!("Could not choose an app: {error:#}")), cx)
                })
                .ok();
            }
        })
        .detach();
    }
}
