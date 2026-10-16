//! Document watches (@ref LLP 1069.010.000 D1, D2). The answers' watch
//! topics name `doc:` paths; this keeps one inotify watch per folder they
//! live in (and, for a recursive watch, per folder beneath it), and each
//! change announces the watches it covers. The runner then asks again the
//! answers that watch them (LLP 1016.002). Watching the folder, not the
//! file, sees an atomic replace (a new inode moved onto the name). The
//! host built for macOS (headless) has no inotify: there a watch is
//! accepted and never announces.

use exact_data::documents::{self, Watched};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The most folders one session watches beneath recursive watches; past it
/// the deepest go unwatched.
const MAX_FOLDERS: usize = 4096;

pub(crate) struct DocWatch {
    owner: u64,
    shared: Arc<Shared>,
    #[cfg(target_os = "linux")]
    inotify: Option<inotify::Inotify>,
}

// Read by the inotify thread, which only Linux has.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct Shared {
    watches: Mutex<Vec<Watched>>,
    folders: Mutex<HashMap<i32, PathBuf>>,
    native: exact_runner::Native,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl Shared {
    fn changed(&self, path: &Path) {
        let topics: Vec<String> = self
            .watches
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|w| documents::covers(w, path))
            .map(|w| w.topic.clone())
            .collect();
        for topic in topics {
            self.native.changed(&topic);
        }
    }
}

/// Follow the answers' watches, once the host watches documents at all.
pub(crate) fn sync<D: exact_runner::DataSource>(
    watch: &mut Option<DocWatch>,
    runner: &mut exact_runner::Runner<D>,
) {
    if let Some(watch) = watch {
        watch.sync(runner);
    }
}

impl DocWatch {
    /// Watch, as `owner`'s, the documents `runner`'s answers watch: a
    /// change announces through its native slot, so the host must listen.
    pub(crate) fn new<D: exact_runner::DataSource>(
        owner: u64,
        runner: &mut exact_runner::Runner<D>,
    ) -> Self {
        let mut watch = Self::announcing(owner, runner.native_slot());
        watch.sync(runner);
        watch
    }

    /// Follow the topics `runner`'s current answers watch.
    pub(crate) fn sync<D: exact_runner::DataSource>(
        &mut self,
        runner: &mut exact_runner::Runner<D>,
    ) {
        let topics = runner.watched_topics();
        self.follow(&topics, runner.data().grants());
    }

    fn announcing(owner: u64, native: exact_runner::Native) -> Self {
        Self {
            owner,
            shared: Arc::new(Shared {
                watches: Mutex::new(Vec::new()),
                folders: Mutex::new(HashMap::new()),
                native,
            }),
            #[cfg(target_os = "linux")]
            inotify: None,
        }
    }

    /// Keep the watches among `topics` that `grants` let the app read
    /// (`fs.read doc:/`).
    fn follow(&mut self, topics: &[String], grants: &str) {
        let grants = exact_grants::GrantSet::parse(&exact_runner::io_grants(grants)).ok();
        let now: Vec<Watched> = topics
            .iter()
            .filter_map(|t| documents::watched(t, self.owner))
            .filter(|w| {
                grants.as_ref().is_some_and(|g| {
                    g.permits(&exact_grants::Operation::FsRead {
                        path: w.path.clone(),
                    })
                })
            })
            .collect();
        let mut folders = BTreeSet::new();
        for w in &now {
            folders.insert(w.folder.clone());
            if w.recursive && w.real.is_dir() {
                beneath(&w.real, &mut folders);
            }
        }
        *self
            .shared
            .watches
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = now;
        #[cfg(target_os = "linux")]
        {
            if folders.is_empty() && self.inotify.is_none() {
                return;
            }
            if self.inotify.is_none() {
                self.inotify = inotify::Inotify::start(self.shared.clone());
            }
            if let Some(inotify) = &self.inotify {
                inotify.watch_exactly(&folders, &self.shared);
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = folders;
    }

    /// The watched topics.
    #[cfg(all(test, target_os = "linux"))]
    fn watched(&self) -> Vec<String> {
        self.shared
            .watches
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|w| w.topic.clone())
            .collect()
    }
}

/// Every folder beneath `root`, breadth first, until the bound.
fn beneath(root: &Path, folders: &mut BTreeSet<PathBuf>) {
    let mut queue = std::collections::VecDeque::from([root.to_path_buf()]);
    while let Some(folder) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            if folders.len() >= MAX_FOLDERS {
                return;
            }
            // `file_type` does not follow a link: a linked folder stays unwatched.
            if entry.file_type().is_ok_and(|t| t.is_dir()) && folders.insert(entry.path()) {
                queue.push_back(entry.path());
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod inotify {
    #![allow(unsafe_code)] // inotify, eventfd and poll through libc.
    use super::Shared;
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::sync::Arc;

    pub(super) struct Inotify {
        fd: i32,
        stop: i32,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Inotify {
        pub(super) fn start(shared: Arc<Shared>) -> Option<Self> {
            let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
            if fd < 0 {
                return None;
            }
            let stop = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
            if stop < 0 {
                unsafe { libc::close(fd) };
                return None;
            }
            let thread = std::thread::Builder::new()
                .name("exact-doc-watch".into())
                .spawn(move || read_events(fd, stop, &shared))
                .ok();
            Some(Self { fd, stop, thread })
        }

        /// Watch exactly `folders`: add the new ones, remove the rest.
        pub(super) fn watch_exactly(&self, folders: &BTreeSet<PathBuf>, shared: &Shared) {
            use std::os::unix::ffi::OsStrExt;
            const MASK: u32 = libc::IN_CREATE
                | libc::IN_DELETE
                | libc::IN_MODIFY
                | libc::IN_CLOSE_WRITE
                | libc::IN_MOVED_FROM
                | libc::IN_MOVED_TO
                | libc::IN_ATTRIB
                | libc::IN_DELETE_SELF
                | libc::IN_MOVE_SELF
                | libc::IN_ONLYDIR;
            let mut watched = shared.folders.lock().unwrap_or_else(|e| e.into_inner());
            watched.retain(|wd, folder| {
                let keep = folders.contains(folder);
                if !keep {
                    unsafe { libc::inotify_rm_watch(self.fd, *wd) };
                }
                keep
            });
            let have: BTreeSet<PathBuf> = watched.values().cloned().collect();
            for folder in folders.difference(&have) {
                let Ok(c) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
                    continue;
                };
                let wd = unsafe { libc::inotify_add_watch(self.fd, c.as_ptr(), MASK) };
                if wd >= 0 {
                    watched.insert(wd, folder.clone());
                }
            }
        }
    }

    impl Drop for Inotify {
        fn drop(&mut self) {
            let one: u64 = 1;
            unsafe { libc::write(self.stop, (&one as *const u64).cast(), 8) };
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
            unsafe {
                libc::close(self.fd);
                libc::close(self.stop);
            }
        }
    }

    fn read_events(fd: i32, stop: i32, shared: &Shared) {
        use std::os::unix::ffi::OsStrExt;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let mut polls = [
                libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: stop,
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if unsafe { libc::poll(polls.as_mut_ptr(), 2, -1) } < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return;
            }
            if polls[1].revents != 0 {
                return;
            }
            let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
            if read <= 0 {
                continue;
            }
            let mut changed = Vec::new();
            {
                let folders = shared.folders.lock().unwrap_or_else(|e| e.into_inner());
                let header = std::mem::size_of::<libc::inotify_event>();
                let mut at = 0usize;
                while at + header <= read as usize {
                    // The kernel packs events back to back; one may sit unaligned.
                    let event: libc::inotify_event =
                        unsafe { std::ptr::read_unaligned(buffer.as_ptr().add(at).cast()) };
                    let name = &buffer[at + header..at + header + event.len as usize];
                    let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];
                    if let Some(folder) = folders.get(&event.wd) {
                        changed.push(if name.is_empty() {
                            folder.clone()
                        } else {
                            folder.join(std::ffi::OsStr::from_bytes(name))
                        });
                    }
                    at += header + event.len as usize;
                }
            }
            for path in changed {
                shared.changed(&path);
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::DocWatch;
    use exact_data::documents;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// A file written into a watched folder, and an atomic replace of a
    /// watched file, each announce the watch; a sibling folder's change
    /// announces nothing, and a grant that does not cover the path watches
    /// nothing (LLP 1069.010.000 D1).
    #[test]
    fn inotify_announces_a_change_in_a_watched_folder() {
        let root = std::env::temp_dir().join(format!("exact-linux-watch-{}", std::process::id()));
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::create_dir_all(root.join("other")).unwrap();
        std::fs::write(root.join("notes/a.md"), "a").unwrap();
        let folder = documents::mint(&root.join("notes"), 7301).unwrap();
        let file = documents::open_route(root.join("notes/a.md").to_str().unwrap(), 7301).unwrap();
        let heard = Arc::new(Mutex::new(Vec::<String>::new()));
        let native = exact_runner::Native::default();
        let sink = heard.clone();
        native.on_changed(Some(Arc::new(move |topic: &str| {
            sink.lock().unwrap().push(topic.to_owned())
        })));
        let topics = [
            documents::watch_topic(&folder, false),
            documents::watch_topic(&file, false),
        ];
        let wait_for = |topic: &str| {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(5) {
                if heard.lock().unwrap().iter().any(|t| t == topic) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        };
        let mut watch = DocWatch::announcing(7301, native);
        watch.follow(&topics, "fs.read app:/data");
        assert!(watch.watched().is_empty(), "no fs.read doc:/, no watch");
        watch.follow(&topics, "fs.read doc:/");
        assert_eq!(watch.watched().len(), 2);
        std::fs::write(root.join("other/b.md"), "b").unwrap();
        std::fs::write(root.join("notes/c.md"), "c").unwrap();
        assert!(
            wait_for(&topics[0]),
            "the folder's new entry: {:?}",
            heard.lock().unwrap()
        );
        heard.lock().unwrap().clear();
        std::fs::write(root.join("notes/a.md.tmp"), "replaced").unwrap();
        std::fs::rename(root.join("notes/a.md.tmp"), root.join("notes/a.md")).unwrap();
        assert!(
            wait_for(&topics[1]),
            "the file replaced: {:?}",
            heard.lock().unwrap()
        );
        assert!(!heard.lock().unwrap().iter().any(|t| t.contains("other")));
        watch.follow(&[], "fs.read doc:/");
        assert!(watch.watched().is_empty());
        drop(watch);
        documents::forget(7301);
        std::fs::remove_dir_all(root).unwrap();
    }
}
