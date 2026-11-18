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
        mod_time: None,
    };
    let error = crate::media::open_file(&api, &item).unwrap_err();
    assert_eq!(error.to_string(), "Download this program before running it");
}
