use std::fs;
use std::path::{Path, PathBuf};

use super::validate_name;
use crate::{api::Api, workspace::launch_path};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("BoxBox Windows {} {stamp}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn windows_drive_letters_and_file_uris_open_the_same_folder() {
    let fixture = Fixture::new();
    let folder = fixture.0.join("café #1");
    fs::create_dir(&folder).unwrap();
    let file = folder.join("notes.txt");
    fs::write(&file, "hello").unwrap();
    let api = Api::local_with_root(fixture.0.clone()).unwrap();

    assert_eq!(launch_path(&api, folder.to_str().unwrap()), "home/café #1");
    assert_eq!(launch_path(&api, file.to_str().unwrap()), "home/café #1");
    let uri = crate::os_clipboard::file_uri(&fs::canonicalize(folder).unwrap());
    assert_eq!(launch_path(&api, &uri), "home/café #1");
    assert_eq!(api.list("home/café #1").unwrap()[0].name, "notes.txt");
}

#[test]
fn windows_local_operations_keep_existing_files_and_restore_trash() {
    let fixture = Fixture::new();
    let files = crate::local::LocalFiles::with_root(fixture.0.clone()).unwrap();
    assert!(files.drives().unwrap()[0].total_bytes > 0);
    assert!(crate::local::writable(&fixture.0));

    files.create_file("home", "a.txt", "original").unwrap();
    files.create_file("home", "b.txt", "existing").unwrap();
    assert!(files.rename("home/a.txt", "home/b.txt").is_err());
    assert_eq!(
        fs::read_to_string(fixture.0.join("b.txt")).unwrap(),
        "existing"
    );

    files.rename("home/a.txt", "home/café.txt").unwrap();
    files.copy("home/café.txt", "home/copy.txt").unwrap();
    let item = files.move_to_trash("home/café.txt").unwrap();
    assert!(item.can_restore);
    assert!(!fixture.0.join("café.txt").exists());
    assert_eq!(files.trash().unwrap()[0].id, item.id);
    files.restore_trash(&item.id).unwrap();
    assert_eq!(
        fs::read_to_string(fixture.0.join("café.txt")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::read_to_string(fixture.0.join("copy.txt")).unwrap(),
        "original"
    );
}

#[test]
fn rejects_windows_device_names_and_path_components() {
    for name in [
        "C:outside",
        r"folder\child",
        "notes.txt:stream",
        "NUL.txt",
        "com2 .log",
        "LPT¹",
        "folder.",
        "space ",
        "file*",
    ] {
        assert!(validate_name(name).is_err(), "{name}");
    }
    for name in ["com10.txt", "folder name", "café.txt", ".hidden"] {
        assert!(validate_name(name).is_ok(), "{name}");
    }
    assert_eq!(
        super::shell_path(Path::new(r"\\?\UNC\server\share")),
        super::wide(r"\\server\share")
    );
}

#[test]
fn windows_remote_programs_are_not_opened_as_documents() {
    let api = Api::new("http://127.0.0.1:1", None);
    let item = crate::api::FileInfo {
        name: "hello.exe.".into(),
        path: "home/hello.exe.".into(),
        size: 0,
        is_dir: false,
        hidden: false,
        mod_time: None,
    };
    let error = crate::media::open_file(&api, &item).unwrap_err();
    assert_eq!(error.to_string(), "Download this program before running it");
}

#[test]
fn windows_hidden_attribute_survives_the_folder_listing() {
    use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN};
    let fixture = Fixture::new();
    let path = fixture.0.join("hidden.txt");
    fs::write(&path, "hidden").unwrap();
    let wide = super::wide(&path);
    super::ensure_win32(unsafe { SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN) })
        .unwrap();
    let files = crate::local::LocalFiles::with_root(fixture.0.clone()).unwrap();
    assert!(files.list("home").unwrap()[0].is_hidden());
}

#[test]
fn shell_calls_can_enter_sta_from_mta_workers() {
    use windows_sys::Win32::System::Com::*;
    let result = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
    assert!(result >= 0);
    let sta = super::with_shell(|| {
        let mut kind = 0;
        let mut qualifier = 0;
        assert!(unsafe { CoGetApartmentType(&mut kind, &mut qualifier) } >= 0);
        Ok(matches!(kind, APTTYPE_STA | APTTYPE_MAINSTA))
    })
    .unwrap();
    unsafe { CoUninitialize() };
    assert!(sta);
}

#[test]
fn windows_smb_urls_become_confined_share_paths() {
    let path = crate::devices::gvfs::smb_path("smb://test-server/My%20Files/Docs").unwrap();
    assert_eq!(path, Path::new(r"\\test-server\My Files\Docs"));
    assert!(crate::devices::gvfs::smb_path("smb://test-server").is_err());
    assert!(crate::devices::gvfs::smb_path("smb://test-server/share/a%5Cb").is_err());
    assert!(crate::devices::gvfs::smb_path("smb://name:pass@test-server/share").is_err());
}

#[test]
fn windows_file_icons_come_from_the_shell() {
    use crate::file_icons::IconKey;
    let pixels = super::icons::render(&IconKey::Folder("folder"), 32).unwrap();
    assert_eq!(pixels.dimensions(), (32, 32));
    assert!(pixels.pixels().any(|pixel| pixel[3] != 0));
}

#[test]
#[ignore = "requires a real user Recycle Bin"]
fn windows_recycle_bin_round_trip() {
    let fixture = Fixture::new();
    let path = fixture.0.join("native recycle café.txt");
    fs::write(&path, "hello").unwrap();
    let item = super::recycle_bin::recycle(&path).unwrap();
    assert!(!path.exists());
    assert!(super::recycle_bin::list()
        .unwrap()
        .iter()
        .any(|row| row.id == item.id));
    super::recycle_bin::restore(&item.id).unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "hello");
}
