//! The storage a native module is handed (LLP 1154 D3): a SQLite file
//! admitted under the app's grants and held by one SQLite at a time.
#![cfg(exact_js_engine)]
use super::storage::{call, Root, GRANTS};
use exact_js::{NativeStorage, SqliteFile};
use exact_runner::{DataSource, Store};
use std::path::Path;

/// The module asks for `app:/data/notes.db` (granted) or `app:/data/other.db`
/// (not) on `open` and `ungranted`, and answers the real path's file name.
/// It holds the file open with a POSIX lock on it, as its own SQLite would
/// while writing, until `close`: the open file before the guard, so the
/// file closes first.
#[derive(Default)]
struct Native {
    storage: Option<NativeStorage>,
    file: Option<(std::fs::File, SqliteFile)>,
}
impl exact_js::NativeModule for Native {
    fn configure_storage(&mut self, storage: NativeStorage) -> Result<(), String> {
        self.storage = Some(storage);
        Ok(())
    }
    fn call(&mut self, request: &exact_js::Json) -> Result<exact_js::Json, String> {
        let storage = self.storage.as_ref().expect("configured before any call");
        let path = match request["value"].as_str() {
            Some("close") => {
                self.file = None;
                return Ok(exact_js::json!({"text": "closed"}));
            }
            Some("open") => "app:/data/notes.db",
            _ => "app:/data/other.db",
        };
        let file = storage.sqlite(path)?;
        let name = file.path().file_name().unwrap().to_string_lossy();
        let text = name.into_owned();
        self.file = Some((locked(file.path()), file));
        Ok(exact_js::json!({ "text": text }))
    }
}

/// The file at `path`, created if need be, with a write lock on all of it.
fn locked(path: &Path) -> std::fs::File {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: a zeroed `flock` is a valid argument once its fields are set.
        let mut lock: libc::flock = unsafe { std::mem::zeroed() };
        lock.l_type = libc::F_WRLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        // SAFETY: the descriptor is open and `lock` outlives the call.
        assert_eq!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
            0
        );
    }
    file
}

/// Whether this process holds a lock on `path`, as another process sees it
/// (a process never sees its own locks): a forked child asks with `F_GETLK`.
#[cfg(unix)]
fn locked_here(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: the child calls only async-signal-safe functions (open, fcntl,
    // _exit) on memory prepared before the fork.
    unsafe {
        let child = libc::fork();
        assert!(child >= 0, "fork");
        if child == 0 {
            let fd = libc::open(name.as_ptr(), libc::O_RDWR);
            let mut lock: libc::flock = std::mem::zeroed();
            lock.l_type = libc::F_WRLCK as _;
            lock.l_whence = libc::SEEK_SET as _;
            let asked = fd >= 0 && libc::fcntl(fd, libc::F_GETLK, &mut lock) == 0;
            libc::_exit(
                if asked && i64::from(lock.l_type) != i64::from(libc::F_UNLCK) {
                    0
                } else {
                    1
                },
            );
        }
        let mut status = 0;
        assert_eq!(libc::waitpid(child, &mut status, 0), child);
        libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0
    }
}

/// Each order of the two SQLites refuses the second by name, without
/// opening the file (whose closing would drop the holder's locks), and
/// each reservation ends when its holder lets the file go.
#[test]
fn a_native_modules_sqlite_file_is_granted_and_held_by_one_sqlite_at_a_time() {
    let root = Root::new();
    let mut m = root.module().with_native(|_| Box::new(Native::default()));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    m.activate().unwrap();
    let mut ask = |op: &str, value: &str| call(&mut m, &mut s, op, value);
    let notes = root.0.join("data/notes.db");

    let refused = ask("native", "ungranted");
    assert!(
        refused.contains(
            "app:/data/other.db: no grant covers it; grant `sqlite.open app:/data/other.db`"
        ),
        "{refused}"
    );
    assert!(!root.0.join("data/other.db").exists());

    // The module holds the file: the app's own open is refused and leaves
    // its lock, then is admitted once it closes.
    assert_eq!(ask("native", "open"), "notes.db");
    let refused = ask("hold", "");
    assert!(
        refused.contains("notes.db is reserved for another SQLite library"),
        "{refused}"
    );
    #[cfg(unix)]
    assert!(
        locked_here(&notes),
        "the refused open dropped the module's lock"
    );
    assert_eq!(ask("native", "close"), "closed");
    assert_eq!(ask("hold", ""), "held");

    // The app's storage holds it: the module is refused, leaving a lock on
    // the file where it was, then admitted once the app closes it.
    let lock = locked(&notes);
    let refused = ask("native", "open");
    assert!(
        refused.contains("app:/data/notes.db is open in the app's own storage"),
        "{refused}"
    );
    #[cfg(unix)]
    assert!(
        locked_here(&notes),
        "the refused reservation dropped the lock"
    );
    drop(lock);
    assert_eq!(ask("release", ""), "released");
    assert_eq!(ask("native", "open"), "notes.db");
    assert_eq!(ask("native", "close"), "closed");
}

/// A reservation follows the file, not its name: another name of the file
/// (a hard link, and a case alias where the volume ignores case) is the same
/// file to every Exact SQLite connection, the TypeScript storage's and a Rust
/// source's alike, in either order, from before the file exists; and a
/// reserved file keeps its reservation when its name is unlinked and made
/// again, which names another file.
#[test]
fn a_reserved_file_is_known_by_its_identity_under_any_name() {
    use ibex2::stdlib::sqlite::{Location, Provider};
    use ibex2_sqlite::{reserve, Refused};
    let root = Root::new();
    std::fs::create_dir_all(&root.0).unwrap();
    // SQLite refuses a path through a link (macOS's `/var`).
    let dir = std::fs::canonicalize(&root.0).unwrap();
    let (file, upper) = (dir.join("vault.sqlite"), dir.join("VAULT.sqlite"));
    let link = dir.join("vault-link.sqlite");
    let open = |path: &Path| {
        ibex2_sqlite::SqliteProvider.open(Location {
            path: path.to_path_buf(),
        })
    };
    let refused = |path: &Path| {
        let refused = open(path).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            refused.contains("is reserved for another SQLite library"),
            "{}: {refused:?}",
            path.display()
        );
    };
    std::fs::write(dir.join("probe"), "").unwrap();
    let ignores_case = dir.join("PROBE").exists();

    // Absent, the file is made when reserved, so an alias is the same file at once.
    let reserved = reserve(&file).unwrap();
    assert!(!reserved.existed() && file.exists());
    if ignores_case {
        refused(&upper);
    }
    drop(reserved);
    std::fs::remove_file(&file).unwrap();
    // Absent, the file is made before SQLite opens it, so a reservation by an alias is refused.
    if ignores_case {
        let connection = open(&upper).unwrap();
        assert_eq!(reserve(&file).unwrap_err(), Refused::Open);
        connection.close().unwrap();
        std::fs::remove_file(&file).unwrap();
    }

    // An existing file, under a hard link and a case alias, in both orders.
    let reserved = reserve(&file).unwrap();
    std::fs::hard_link(&file, &link).unwrap();
    let mut names = vec![&link];
    if ignores_case {
        names.push(&upper);
    }
    for name in &names {
        refused(name);
    }
    drop(reserved);
    let connection = open(&file).unwrap();
    for name in &names {
        assert_eq!(
            reserve(name).unwrap_err(),
            Refused::Open,
            "{}",
            name.display()
        );
    }
    connection.close().unwrap();

    // The reserved file held open and locked, its name unlinked and made
    // again: the entry keeps the file it reserved, still reachable through
    // `link`, whose open is refused and leaves the lock; the new file at the
    // name is another file, and is admitted.
    let reserved = reserve(&file).unwrap();
    assert!(reserved.existed());
    let holder = locked(&file);
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, "").unwrap();
    refused(&link);
    #[cfg(unix)]
    assert!(
        locked_here(&link),
        "the refused open dropped the holder's lock"
    );
    open(&file).unwrap().close().unwrap();
    drop(holder);
    drop(reserved);
}
