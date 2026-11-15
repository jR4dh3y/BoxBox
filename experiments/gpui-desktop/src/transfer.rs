//! A copy or move of items into a folder, run on its own thread, that asks as
//! it goes, as Thunar and Nautilus do: a name already taken (Skip, Rename,
//! Replace, or Merge for two folders), an item that failed (Retry or Skip),
//! and too little space before it starts. A remembered "Apply to all" answers
//! the rest of the same kind. Names are made valid on FAT and exFAT drives.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context as _, Result};

use crate::api::{Api, FileInfo};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Copy,
    Move,
}

/// What is already there, or what is coming, for the conflict dialog.
#[derive(Clone, Debug)]
pub struct Side {
    pub is_dir: bool,
    pub size: u64,
    pub mod_time: Option<i64>,
}

impl Side {
    fn of(item: &FileInfo) -> Self {
        Self { is_dir: item.is_dir, size: item.size, mod_time: item.mod_time }
    }
}

#[derive(Clone, Debug)]
pub enum Question {
    /// `name` is taken in the folder at virtual path `parent`. `rename_to` is the free name Rename uses.
    Conflict { name: String, parent: String, existing: Side, incoming: Side, rename_to: String },
    Failed { name: String, error: String },
    /// The items need `needed` bytes and the destination has `free`.
    NoSpace { needed: u64, free: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Skip,
    Rename,
    Replace,
    Merge,
    Retry,
    /// Go on despite too little space.
    Continue,
    Cancel,
}

/// Asks the user and waits; the bool is "Apply to all", offered when `more` is set.
pub type Ask<'a> = dyn Fn(Question, bool) -> (Answer, bool) + 'a;

pub struct Job {
    pub mode: Mode,
    pub items: Vec<(Arc<Api>, FileInfo)>,
    pub destination: Arc<Api>,
    pub parent: String,
}

/// What a run did, for the notice and the refresh.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub done: usize,
    pub skipped: usize,
}

pub fn run(job: &Job, ask: &Ask) -> Result<Outcome> {
    let mut run = Run { job, ask, policy: Policy::default(), outcome: Outcome::default(), fat: false };
    if let Ok(Some(folder)) = job.destination.local_path(&job.parent) {
        run.fat = is_fat(&folder);
        run.check_space(&folder)?;
    }
    let count = job.items.len();
    for (index, (source, item)) in job.items.iter().enumerate() {
        crate::progress::check()?;
        run.put(source, item, &job.parent, index + 1 < count)?;
    }
    Ok(run.outcome)
}

/// Answers kept by "Apply to all", one per kind of question.
#[derive(Default)]
struct Policy {
    file_conflict: Option<Answer>,
    folder_conflict: Option<Answer>,
    failure: Option<Answer>,
}

struct Run<'a> {
    job: &'a Job,
    ask: &'a Ask<'a>,
    policy: Policy,
    outcome: Outcome,
    /// The destination is FAT or exFAT, so names lose what those refuse.
    fat: bool,
}

impl Run<'_> {
    fn dest(&self) -> &Arc<Api> {
        &self.job.destination
    }

    /// Warns before starting when the items cannot fit. A move within one
    /// filesystem takes no space.
    fn check_space(&mut self, folder: &Path) -> Result<()> {
        let Some(free) = free_space(folder) else { return Ok(()) };
        let mut needed = 0;
        for (source, item) in &self.job.items {
            let local = source.local_path(&item.path).ok().flatten();
            if self.job.mode == Mode::Move && local.as_deref().is_some_and(|path| same_device(path, folder)) {
                continue;
            }
            needed += match (&local, item.is_dir) {
                (Some(path), true) => tree_size(path),
                _ => item.size,
            };
        }
        if needed <= free {
            return Ok(());
        }
        match (self.ask)(Question::NoSpace { needed, free }, false).0 {
            Answer::Continue => Ok(()),
            _ => bail!("Cancelled"),
        }
    }

    /// Puts `item` into `parent` on the destination, asking about a taken name.
    /// `more` is whether items of this kind may follow, so "Apply to all" helps.
    fn put(&mut self, source: &Arc<Api>, item: &FileInfo, parent: &str, more: bool) -> Result<()> {
        let mut name = if self.fat { fat_safe(&item.name) } else { item.name.clone() };
        let target = join(parent, &name);
        let same = same_provider(source, self.dest());
        if let Ok(existing) = self.dest().stat(&target) {
            // A copy pasted where its original is: Nautilus names it, without asking.
            let answer = if same && existing.path == item.path {
                Answer::Rename
            } else {
                self.conflict(item, &existing, parent, &name, more)?
            };
            match answer {
                Answer::Skip => {
                    self.outcome.skipped += 1;
                    return Ok(());
                }
                Answer::Rename => name = self.free_name(parent, &name)?,
                Answer::Replace => {
                    if !self.attempt(&item.name, || self.job.destination.delete(&target, true))? {
                        return Ok(());
                    }
                }
                Answer::Merge => return self.merge_into(source, item, &target),
                _ => bail!("Cancelled"),
            }
        }
        // FAT needs every name inside checked too, so folders are walked there.
        if self.fat && item.is_dir {
            if !self.attempt(&item.name, || self.job.destination.create_directory(parent, &name).map(|_| ()))? {
                return Ok(());
            }
            return self.merge_into(source, item, &join(parent, &name));
        }
        let (job, fat) = (self.job, self.fat);
        if self.attempt(&item.name, || Self::transfer_one(job, fat, source, item, parent, &name))? {
            self.outcome.done += 1;
        }
        Ok(())
    }

    /// Two folders become one: each item goes inside, asking again where names meet.
    fn merge_into(&mut self, source: &Arc<Api>, item: &FileInfo, target: &str) -> Result<()> {
        let Some(children) = self.attempt_value(&item.name, || source.list(&item.path))? else { return Ok(()) };
        for child in &children {
            crate::progress::check()?;
            self.put(source, child, target, true)?;
        }
        // A moved folder is gone once everything left it; a skipped item keeps it.
        if self.job.mode == Mode::Move && source.list(&item.path).is_ok_and(|left| left.is_empty()) {
            let _ = source.delete(&item.path, false);
        }
        self.outcome.done += 1;
        Ok(())
    }

    fn conflict(&mut self, item: &FileInfo, existing: &FileInfo, parent: &str, name: &str, more: bool) -> Result<Answer> {
        let folders = item.is_dir && existing.is_dir;
        let remembered = if folders { self.policy.folder_conflict } else { self.policy.file_conflict };
        if let Some(answer) = remembered {
            return Ok(answer);
        }
        let question = Question::Conflict {
            name: name.to_string(),
            parent: parent.to_string(),
            existing: Side::of(existing),
            incoming: Side::of(item),
            rename_to: self.free_name(parent, name)?,
        };
        let (mut answer, all) = (self.ask)(question, more);
        // Merge only joins two folders; for anything else it means Replace.
        if answer == Answer::Merge && !folders {
            answer = Answer::Replace;
        }
        if all {
            match folders {
                true => self.policy.folder_conflict = Some(answer),
                false => self.policy.file_conflict = Some(answer),
            }
        }
        Ok(answer)
    }

    /// Runs `step` until it works, or the user skips it (`false`) or cancels.
    fn attempt(&mut self, name: &str, step: impl Fn() -> Result<()>) -> Result<bool> {
        Ok(self.attempt_value(name, step)?.is_some())
    }

    fn attempt_value<T>(&mut self, name: &str, step: impl Fn() -> Result<T>) -> Result<Option<T>> {
        loop {
            let error = match step() {
                Ok(value) => return Ok(Some(value)),
                Err(error) => error,
            };
            crate::progress::check()?;
            let answer = match self.policy.failure {
                Some(answer) => answer,
                None => {
                    let (answer, all) = (self.ask)(Question::Failed { name: name.to_string(), error: format!("{error:#}") }, true);
                    if all && answer == Answer::Skip {
                        self.policy.failure = Some(answer);
                    }
                    answer
                }
            };
            match answer {
                Answer::Retry => continue,
                Answer::Skip => {
                    self.outcome.skipped += 1;
                    return Ok(None);
                }
                _ => bail!("Cancelled"),
            }
        }
    }

    /// One item to `parent/name`, which is free.
    fn transfer_one(job: &Job, fat: bool, source: &Arc<Api>, item: &FileInfo, parent: &str, name: &str) -> Result<()> {
        let target = join(parent, name);
        let dest = &job.destination;
        if same_provider(source, dest) {
            return match job.mode {
                Mode::Copy => dest.copy(&item.path, &target).with_context(|| format!("cannot copy {}", item.name)),
                Mode::Move => match dest.rename(&item.path, &target) {
                    Ok(_) => Ok(()),
                    // Another filesystem on the same provider, such as a USB drive.
                    Err(error) if crosses_devices(&error) => {
                        dest.copy(&item.path, &target).with_context(|| format!("cannot copy {}", item.name))?;
                        dest.delete(&item.path, true).with_context(|| format!("cannot remove the original {}", item.name))
                    }
                    Err(error) => Err(error).with_context(|| format!("cannot move {}", item.name)),
                },
            };
        }
        Self::between_providers(job, fat, source, item, parent, name)?;
        // Across providers a move is a copy, then the original goes: to Trash
        // locally, for good on a server.
        if job.mode == Mode::Move {
            match source.is_local() {
                true => source.move_to_trash(&item.path).map(|_| ()).with_context(|| format!("cannot move {} to Trash", item.name))?,
                false => source.delete(&item.path, true).with_context(|| format!("cannot delete {}", item.name))?,
            }
        }
        Ok(())
    }

    /// Copies `item` into `parent` on another provider, as `name`.
    fn between_providers(job: &Job, fat: bool, source: &Arc<Api>, item: &FileInfo, parent: &str, name: &str) -> Result<()> {
        let dest = &job.destination;
        if item.is_dir {
            dest.create_directory(parent, name).with_context(|| format!("cannot create folder {name}"))?;
            let inside = join(parent, name);
            for child in source.list(&item.path).with_context(|| format!("cannot list {}", item.path))? {
                let child_name = if fat { fat_safe(&child.name) } else { child.name.clone() };
                Self::between_providers(job, fat, source, &child, &inside, &child_name)?;
            }
            return Ok(());
        }
        // Imports take the file's own name, so it is staged under the name it gets.
        let temp = temp_dir()?;
        let stage = temp.0.clone();
        let staged = stage.join(name);
        match source.local_path(&item.path)? {
            Some(path) => {
                fs::copy(&path, &staged).with_context(|| format!("cannot stage {}", item.name))?;
            }
            None => {
                let downloaded = source.download(item, &stage).with_context(|| format!("cannot download {}", item.name))?;
                if !downloaded.starts_with(&stage) || !downloaded.is_file() {
                    bail!("the server sent {} somewhere unexpected", item.name);
                }
                if downloaded != staged {
                    fs::rename(&downloaded, &staged)?;
                }
            }
        }
        // Dropping `temp` removes the staged copy.
        dest.import_file(&staged, parent).map(|_| ()).with_context(|| format!("cannot copy {}", item.name))
    }

    /// `name (copy N)`, free in `parent`.
    fn free_name(&self, parent: &str, name: &str) -> Result<String> {
        let taken: HashSet<String> = self.dest().list(parent)?.into_iter().map(|item| item.name).collect();
        Ok(keep_both_name(name, &taken))
    }
}

pub fn same_provider(left: &Api, right: &Api) -> bool {
    left.is_local() == right.is_local() && left.server_url() == right.server_url()
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_string() } else { format!("{parent}/{name}") }
}

/// `stem (copy N).ext`, counting from 1; the extension is the last dot not at either end.
pub fn keep_both_name(name: &str, taken: &HashSet<String>) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 && dot < name.len() - 1 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (1..).map(|number| format!("{stem} (copy {number}){extension}")).find(|candidate| !taken.contains(candidate)).unwrap_or_default()
}

/// A name FAT and exFAT accept: no `"*/:<>?\|` or control characters, and no
/// trailing dots or spaces, each replaced with `_`.
pub fn fat_safe(name: &str) -> String {
    let mut safe: String = name.chars().map(|letter| if letter.is_control() || "\"*/:<>?\\|".contains(letter) { '_' } else { letter }).collect();
    let kept = safe.trim_end_matches(['.', ' ']).len();
    if kept < safe.len() {
        let removed = safe.len() - kept;
        safe.truncate(kept);
        safe.extend(std::iter::repeat_n('_', removed));
    }
    safe
}

fn is_fat(folder: &Path) -> bool {
    const MSDOS: i64 = 0x4d44;
    const EXFAT: i64 = 0x2011_bab0;
    let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_encoded_bytes()) else { return false };
    // SAFETY: `path` is NUL-terminated and `stats` outlives the call.
    unsafe {
        let mut stats: libc::statfs = std::mem::zeroed();
        libc::statfs(path.as_ptr(), &mut stats) == 0 && matches!(stats.f_type as i64, MSDOS | EXFAT)
    }
}

fn free_space(folder: &Path) -> Option<u64> {
    let path = std::ffi::CString::new(folder.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: as in `is_fat`.
    unsafe {
        let mut stats: libc::statvfs = std::mem::zeroed();
        (libc::statvfs(path.as_ptr(), &mut stats) == 0).then(|| stats.f_bavail as u64 * stats.f_frsize as u64)
    }
}

fn same_device(path: &Path, folder: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    matches!((fs::symlink_metadata(path), fs::metadata(folder)), (Ok(left), Ok(right)) if left.dev() == right.dev())
}

fn tree_size(path: &Path) -> u64 {
    let mut bytes = 0;
    let mut pending = vec![path.to_path_buf()];
    while let Some(path) = pending.pop() {
        let Ok(metadata) = fs::symlink_metadata(&path) else { continue };
        if metadata.is_dir() {
            pending.extend(fs::read_dir(&path).into_iter().flatten().flatten().map(|entry| entry.path()));
        } else {
            bytes += metadata.len();
        }
    }
    bytes
}

fn crosses_devices(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.downcast_ref::<std::io::Error>().is_some_and(|io| io.raw_os_error() == Some(libc::EXDEV)))
}

/// A staging folder, removed when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn next_stage() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn temp_dir() -> Result<TempDir> {
    for _ in 0..100 {
        let path = std::env::temp_dir().join(format!("boxbox-transfer-{}-{}", std::process::id(), next_stage()));
        let mut builder = fs::DirBuilder::new();
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(TempDir(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("could not make a staging folder")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(files: &[&str]) -> (Fixture, Arc<Api>) {
        let root = std::env::temp_dir().join(format!("boxbox-transfer-test-{}-{}", std::process::id(), next_stage()));
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            if file.ends_with('/') {
                fs::create_dir_all(&path).unwrap();
            } else {
                fs::write(&path, file).unwrap();
            }
        }
        fs::create_dir_all(&root).unwrap();
        let api = Arc::new(Api::local_with_root(root.clone()).unwrap());
        (Fixture(root), api)
    }

    fn job(api: &Arc<Api>, mode: Mode, items: &[&str], parent: &str) -> Job {
        Job { mode, items: items.iter().map(|path| (api.clone(), api.stat(&format!("home/{path}")).unwrap())).collect(), destination: api.clone(), parent: format!("home/{parent}") }
    }

    /// Answers from a script and records the questions.
    fn scripted(answers: Vec<(Answer, bool)>) -> (RefCell<Vec<Question>>, RefCell<std::vec::IntoIter<(Answer, bool)>>) {
        (RefCell::new(Vec::new()), RefCell::new(answers.into_iter()))
    }

    #[test]
    fn conflicts_skip_rename_replace_and_apply_to_all() {
        let (root, api) = fixture(&["src/a.txt", "src/b.txt", "src/c.txt", "dst/a.txt", "dst/b.txt", "dst/c.txt"]);
        let (asked, answers) = scripted(vec![(Answer::Rename, false), (Answer::Replace, true)]);
        let ask = |question: Question, _| {
            asked.borrow_mut().push(question);
            answers.borrow_mut().next().unwrap()
        };
        let outcome = run(&job(&api, Mode::Copy, &["src/a.txt", "src/b.txt", "src/c.txt"], "dst"), &ask).unwrap();
        // The second answer applied to the third item without asking.
        assert_eq!(asked.borrow().len(), 2);
        assert_eq!(outcome, Outcome { done: 3, skipped: 0 });
        assert_eq!(fs::read_to_string(root.0.join("dst/a (copy 1).txt")).unwrap(), "src/a.txt");
        assert_eq!(fs::read_to_string(root.0.join("dst/a.txt")).unwrap(), "dst/a.txt");
        assert_eq!(fs::read_to_string(root.0.join("dst/c.txt")).unwrap(), "src/c.txt");
    }

    #[test]
    fn merging_folders_asks_again_inside_and_a_move_empties_the_source() {
        let (root, api) = fixture(&["src/docs/new.txt", "src/docs/same.txt", "dst/docs/same.txt", "dst/docs/old.txt"]);
        let (asked, answers) = scripted(vec![(Answer::Merge, false), (Answer::Skip, false)]);
        let ask = |question: Question, _| {
            asked.borrow_mut().push(question);
            answers.borrow_mut().next().unwrap()
        };
        let outcome = run(&job(&api, Mode::Move, &["src/docs"], "dst"), &ask).unwrap();
        assert!(matches!(&asked.borrow()[1], Question::Conflict { name, .. } if name == "same.txt"));
        assert_eq!(outcome.skipped, 1);
        assert_eq!(fs::read_to_string(root.0.join("dst/docs/new.txt")).unwrap(), "src/docs/new.txt");
        assert_eq!(fs::read_to_string(root.0.join("dst/docs/same.txt")).unwrap(), "dst/docs/same.txt");
        assert!(root.0.join("dst/docs/old.txt").exists());
        // The skipped file keeps its folder.
        assert!(root.0.join("src/docs/same.txt").exists());
        assert!(!root.0.join("src/docs/new.txt").exists());
    }

    #[test]
    fn a_failure_can_be_retried_or_skipped() {
        use std::os::unix::fs::PermissionsExt as _;
        let (root, api) = fixture(&["src/a.txt", "src/b.txt", "dst/"]);
        let set_mode = |mode| fs::set_permissions(root.0.join("dst"), fs::Permissions::from_mode(mode)).unwrap();
        set_mode(0o500);
        let asked = std::cell::Cell::new(0);
        // Skip the first; for the second, fix the folder and retry.
        let ask = |question: Question, _| {
            assert!(matches!(question, Question::Failed { .. }));
            asked.set(asked.get() + 1);
            match asked.get() {
                1 => (Answer::Skip, false),
                _ => {
                    set_mode(0o700);
                    (Answer::Retry, false)
                }
            }
        };
        let outcome = run(&job(&api, Mode::Copy, &["src/a.txt", "src/b.txt"], "dst"), &ask).unwrap();
        assert_eq!(outcome, Outcome { done: 1, skipped: 1 });
        assert!(!root.0.join("dst/a.txt").exists());
        assert!(root.0.join("dst/b.txt").exists());
    }

    #[test]
    fn copying_into_its_own_folder_names_the_copy() {
        let (root, api) = fixture(&["dst/a.txt"]);
        let ask = |_: Question, _| panic!("nothing to ask");
        run(&job(&api, Mode::Copy, &["dst/a.txt"], "dst"), &ask).unwrap();
        assert!(root.0.join("dst/a (copy 1).txt").exists());
    }

    #[test]
    fn fat_names() {
        assert_eq!(fat_safe("a:b?.txt"), "a_b_.txt");
        assert_eq!(fat_safe("notes. "), "notes__");
        assert_eq!(fat_safe("plain.md"), "plain.md");
        let taken: HashSet<String> = ["a (copy 1).txt".to_string()].into();
        assert_eq!(keep_both_name("a.txt", &taken), "a (copy 2).txt");
    }
}
