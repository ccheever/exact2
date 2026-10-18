//! Document watches (@ref LLP 1069.010.000 D1, D2). The answers' watch
//! topics name `doc:` paths; this keeps one FSEvents stream over the folders
//! they live in (file-level events, so an atomic replace of a watched file
//! is seen), and each change announces the watches it covers. The runner
//! then asks again the answers that watch them (LLP 1016.002). iOS has no
//! FSEvents, so there a watch is accepted and never announces (stage 1
//! builds macOS first).

use exact_data::documents::{self, Watched};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct DocWatch {
    owner: u64,
    shared: Arc<Shared>,
    folders: Vec<PathBuf>,
    #[cfg(target_os = "macos")]
    stream: Option<fsevents::Stream>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // FSEvents reads it.
struct Shared {
    watches: Mutex<Vec<Watched>>,
    native: exact_runner::Native,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl Shared {
    fn changed(&self, path: &std::path::Path) {
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
                native,
            }),
            folders: Vec::new(),
            #[cfg(target_os = "macos")]
            stream: None,
        }
    }

    /// Keep the watches among `topics` that `grants` let the app read
    /// (`fs.read doc:/`): the watches change at once, and the stream is
    /// replaced only when its folders do.
    fn follow(&mut self, topics: &[String], grants: &str) {
        let grants = ibex2::grant::GrantSet::parse(&exact_runner::io_grants(grants)).ok();
        let now: Vec<Watched> = topics
            .iter()
            .filter_map(|t| documents::watched(t, self.owner))
            .filter(|w| {
                grants.as_ref().is_some_and(|g| {
                    g.permits(&ibex2::grant::Operation::FsRead {
                        path: w.path.clone(),
                    })
                })
            })
            .collect();
        let mut folders: Vec<PathBuf> = now.iter().map(|w| w.folder.clone()).collect();
        folders.sort();
        folders.dedup();
        *self
            .shared
            .watches
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = now;
        if folders == self.folders {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            self.stream = None;
            if !folders.is_empty() {
                self.stream = fsevents::Stream::start(&folders, self.shared.clone());
            }
        }
        self.folders = folders;
    }
}

#[cfg(target_os = "macos")]
mod fsevents {
    #![allow(unsafe_code)] // FSEvents' C API; the stream holds its context by reference count.
    use super::Shared;
    use std::ffi::{c_char, c_void, CStr};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, OnceLock};

    type StreamRef = *mut c_void;
    type Callback =
        extern "C" fn(StreamRef, *mut c_void, usize, *mut c_void, *const u32, *const u64);

    #[repr(C)]
    struct Context {
        version: isize,
        info: *mut c_void,
        retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
        release: Option<extern "C" fn(*const c_void)>,
        copy_description: *const c_void,
    }

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn FSEventStreamCreate(
            allocator: *const c_void,
            callback: Callback,
            context: *mut Context,
            paths: *const c_void,
            since: u64,
            latency: f64,
            flags: u32,
        ) -> StreamRef;
        fn FSEventStreamSetDispatchQueue(stream: StreamRef, queue: *mut c_void);
        fn FSEventStreamStart(stream: StreamRef) -> u8;
        fn FSEventStreamStop(stream: StreamRef);
        fn FSEventStreamInvalidate(stream: StreamRef);
        fn FSEventStreamRelease(stream: StreamRef);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithBytes(
            allocator: *const c_void,
            bytes: *const u8,
            length: isize,
            encoding: u32,
            external: u8,
        ) -> *const c_void;
        fn CFArrayCreate(
            allocator: *const c_void,
            values: *const *const c_void,
            count: isize,
            callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
        static kCFTypeArrayCallBacks: c_void;
    }

    extern "C" {
        fn dispatch_queue_create(label: *const c_char, attr: *const c_void) -> *mut c_void;
    }

    const UTF8: u32 = 0x0800_0100;
    const SINCE_NOW: u64 = u64::MAX;
    // NoDefer: the first change of a quiet period is delivered at once.
    // WatchRoot: a watched folder moved or removed is reported too.
    // FileEvents: one event per file, not per folder.
    const FLAGS: u32 = 0x02 | 0x04 | 0x10;
    const LATENCY: f64 = 0.05;

    /// One serial queue for every stream in the process; never released.
    fn queue() -> *mut c_void {
        static QUEUE: OnceLock<usize> = OnceLock::new();
        *QUEUE.get_or_init(|| unsafe {
            dispatch_queue_create(c"exact.documents.watch".as_ptr(), std::ptr::null()) as usize
        }) as *mut c_void
    }

    pub(super) struct Stream(StreamRef);

    // The stream is only started and stopped; FSEvents calls back on its queue.
    unsafe impl Send for Stream {}

    extern "C" fn retain(info: *const c_void) -> *const c_void {
        unsafe { Arc::increment_strong_count(info as *const Shared) };
        info
    }

    extern "C" fn release(info: *const c_void) {
        unsafe { Arc::decrement_strong_count(info as *const Shared) };
    }

    extern "C" fn changed(
        _: StreamRef,
        info: *mut c_void,
        count: usize,
        paths: *mut c_void,
        _: *const u32,
        _: *const u64,
    ) {
        let shared = unsafe { &*(info as *const Shared) };
        let paths = paths as *const *const c_char;
        for i in 0..count {
            let path = unsafe { CStr::from_ptr(*paths.add(i)) };
            if let Ok(path) = path.to_str() {
                shared.changed(Path::new(path));
            }
        }
    }

    impl Stream {
        pub(super) fn start(folders: &[PathBuf], shared: Arc<Shared>) -> Option<Self> {
            unsafe {
                let strings: Vec<*const c_void> = folders
                    .iter()
                    .filter_map(|f| f.to_str())
                    .map(|f| {
                        CFStringCreateWithBytes(
                            std::ptr::null(),
                            f.as_ptr(),
                            f.len() as isize,
                            UTF8,
                            0,
                        )
                    })
                    .filter(|s| !s.is_null())
                    .collect();
                let array = CFArrayCreate(
                    std::ptr::null(),
                    strings.as_ptr(),
                    strings.len() as isize,
                    &kCFTypeArrayCallBacks as *const c_void,
                );
                for s in &strings {
                    CFRelease(*s);
                }
                if array.is_null() {
                    return None;
                }
                let mut context = Context {
                    version: 0,
                    info: Arc::as_ptr(&shared) as *mut c_void,
                    retain: Some(retain),
                    release: Some(release),
                    copy_description: std::ptr::null(),
                };
                let stream = FSEventStreamCreate(
                    std::ptr::null(),
                    changed,
                    &mut context,
                    array,
                    SINCE_NOW,
                    LATENCY,
                    FLAGS,
                );
                CFRelease(array);
                if stream.is_null() {
                    return None;
                }
                FSEventStreamSetDispatchQueue(stream, queue());
                if FSEventStreamStart(stream) == 0 {
                    FSEventStreamInvalidate(stream);
                    FSEventStreamRelease(stream);
                    return None;
                }
                Some(Self(stream))
            }
        }
    }

    impl Drop for Stream {
        fn drop(&mut self) {
            unsafe {
                FSEventStreamStop(self.0);
                FSEventStreamInvalidate(self.0);
                FSEventStreamRelease(self.0);
            }
        }
    }
}
