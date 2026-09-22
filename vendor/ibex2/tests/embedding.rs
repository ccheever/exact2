//! Storage installed into an independently created runtime, without its loader.
#![cfg(feature = "hermes")]
use ibex2::{bindings::Context, grant::GrantSet, stdlib::app_fs::AppDirectories};
use std::{
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
extern "C" {
    fn storage_consumer_create(
        queue: *const c_void,
        grants: *const c_void,
        factory: *const u8,
        len: usize,
        harden: *const u8,
        harden_len: usize,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn storage_consumer_eval(
        h: *mut c_void,
        data: *const u8,
        len: usize,
        out: *mut *mut c_char,
    ) -> i32;
    fn storage_consumer_step(h: *mut c_void, deliver: bool, out: *mut *mut c_char) -> i32;
    fn storage_consumer_detach(h: *mut c_void);
    fn storage_consumer_destroy(h: *mut c_void);
    fn storage_consumer_free(s: *mut c_char);
}
struct Consumer {
    handle: *mut c_void,
    context: Context,
    directory: PathBuf,
    wakes: Arc<AtomicUsize>,
}
fn take(s: *mut c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    let result = unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned();
    unsafe { storage_consumer_free(s) };
    result
}
impl Consumer {
    fn new(grants: &str) -> Self {
        Self::configured(grants, true)
    }
    fn configured(grants: &str, hardened: bool) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "ibex2-embed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["data", "cache", "tmp"] {
            std::fs::create_dir_all(directory.join(name)).unwrap();
        }
        let context = Context::new(GrantSet::parse(grants).unwrap());
        context
            .set_app_directories(
                AppDirectories::new(
                    directory.join("data"),
                    directory.join("cache"),
                    directory.join("tmp"),
                )
                .unwrap(),
            )
            .unwrap();
        context
            .set_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
            .unwrap();
        let wakes = Arc::new(AtomicUsize::new(0));
        let observed = wakes.clone();
        context.set_wake(Arc::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        }));
        let factory = include_bytes!(concat!(env!("OUT_DIR"), "/sqlite.hbc"));
        let harden = include_bytes!(concat!(env!("OUT_DIR"), "/harden.hbc"));
        let mut error = std::ptr::null_mut();
        let handle = unsafe {
            storage_consumer_create(
                context.state_ptr(),
                context.grants_ptr(),
                factory.as_ptr(),
                factory.len(),
                harden.as_ptr(),
                if hardened { harden.len() } else { 0 },
                &mut error,
            )
        };
        assert!(!handle.is_null(), "{}", take(error));
        Self {
            handle,
            context,
            directory,
            wakes,
        }
    }
    fn eval(&self, source: &str) -> Result<String, String> {
        let input = self.directory.join("test.js");
        let output = self.directory.join("test.hbc");
        std::fs::write(&input, source).unwrap();
        let compiler = std::env::var("IBEX2_HERMESC")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let arch = if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x64"
                };
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tools/hermes-vanilla/hermesc-{}-{arch}",
                    std::env::consts::OS
                ))
            });
        assert!(std::process::Command::new(compiler)
            .args(["-O", "-emit-binary", "-out"])
            .arg(&output)
            .arg(&input)
            .output()
            .unwrap()
            .status
            .success());
        let bytes = std::fs::read(output).unwrap();
        let mut out = std::ptr::null_mut();
        let status =
            unsafe { storage_consumer_eval(self.handle, bytes.as_ptr(), bytes.len(), &mut out) };
        let text = take(out);
        if status == 0 {
            Ok(text)
        } else {
            Err(text)
        }
    }
    fn step(&self, deliver: bool) -> i32 {
        let mut out = std::ptr::null_mut();
        let n = unsafe { storage_consumer_step(self.handle, deliver, &mut out) };
        assert!(n >= 0, "{}", take(out));
        n
    }
    fn finish(&self) -> String {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            self.step(false);
            let result = self.eval("globalThis.result || ''").unwrap();
            if !result.is_empty() {
                return result;
            }
            assert!(Instant::now() < end, "application did not settle");
            self.context.wait(Duration::from_millis(50));
            self.step(true);
        }
    }
}
impl Drop for Consumer {
    fn drop(&mut self) {
        unsafe { storage_consumer_destroy(self.handle) };
        // Context shutdown follows destruction; all tests explicitly close DBs.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn caller_owns_checkpoints_and_storage_is_typed_and_granted() {
    let c = Consumer::new("fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/db");
    c.eval(r#"globalThis.result = ''; storage.fs.atomicWriteFile('app:/data/a\nb', new Uint8Array([1,2])).then(function(){ result = 'written'; });"#).unwrap();
    assert!(c.context.wait(Duration::from_secs(5)));
    assert_eq!(c.eval("result").unwrap(), "");
    assert_eq!(c.step(true), 1);
    assert_eq!(
        c.eval("result").unwrap(),
        "",
        "delivery must not drain microtasks"
    );
    c.step(false);
    assert_eq!(c.eval("result").unwrap(), "written");
    c.eval(r#"result = ''; (async function(){
      const names = await storage.fs.readdir('app:/data');
      if (names.length !== 1 || names[0] !== 'a\nb') throw Error('filename');
      const stat = await storage.fs.stat('app:/data/a\nb');
      if (!stat.isFile || stat.isDirectory || stat.size !== 2) throw Error('stat');
      const bytes = await storage.fs.readFile('app:/data/a\nb');
      if (!(bytes instanceof ArrayBuffer) || new Uint8Array(bytes)[1] !== 2) throw Error('bytes');
      const db = await storage.sqlite.open('app:/data/db');
      await db.execute('CREATE TABLE notes(body TEXT)');
      await db.transaction([{sql:'INSERT INTO notes VALUES (?)',params:['remember']}]);
      await db.close();
      const again = await storage.sqlite.open('app:/data/db');
      const rows = await again.query('SELECT body, 9223372036854775807 FROM notes');
      await again.close();
      if(rows.rows[0][0] !== 'remember' || rows.rows[0][1] !== BigInt('9223372036854775807')) throw Error('persistence');
      try { await storage.fs.writeFile('app:/cache/no', new Uint8Array([0])); throw Error('leaked'); }
      catch(e) { if(e.message === 'leaked') throw e; }
      result='ok';
    })().catch(e => result=String(e));"#).unwrap();
    assert_eq!(c.finish(), "ok");
    assert!(c.wakes.load(Ordering::SeqCst) > 0);
    assert!(!c.directory.join("cache/no").exists());
}

#[test]
fn detached_capabilities_and_pending_work_do_not_reach_dead_runtime() {
    let c = Consumer::new("fs.write app:/data");
    c.eval("storage.fs.writeFile('app:/data/file', new Uint8Array([1]));")
        .unwrap();
    unsafe { storage_consumer_detach(c.handle) };
    assert!(c
        .eval("storage.fs.writeFile('app:/data/other', new Uint8Array([1]));")
        .is_err());
    assert!(!c.directory.join("data/other").exists());
}

#[test]
fn empty_grants_refuse_files_and_sqlite_before_creation() {
    let c = Consumer::new("");
    c.eval(r#"globalThis.result = ''; (async function(){
      let refused = 0;
      try { await storage.fs.writeFile('app:/data/no', new Uint8Array([1])); } catch(e) { refused++; }
      try { await storage.sqlite.open('app:/data/db'); } catch(e) { refused++; }
      result = String(refused);
    })();"#).unwrap();
    assert_eq!(c.finish(), "2");
    assert_eq!(
        std::fs::read_dir(c.directory.join("data")).unwrap().count(),
        0
    );
}

#[test]
fn sqlite_refuses_a_caller_that_has_not_hardened_its_intrinsics() {
    let c = Consumer::configured("sqlite.open app:/data/db", false);
    c.eval("globalThis.result=''; storage.sqlite.open('app:/data/db').then(() => result='opened', e => result=String(e));").ok();
    // The adapter refuses synchronously before publishing native work.
    let error = c.eval("storage.sqlite.open('app:/data/db')").unwrap_err();
    assert!(error.contains("harden"), "{error}");
    assert!(!c.directory.join("data/db").exists());
}

#[test]
fn freezing_modified_intrinsics_does_not_satisfy_the_installation_contract() {
    let c = Consumer::configured("sqlite.open app:/data/db", false);
    c.eval("WeakMap.prototype.get = function () { return undefined; };")
        .unwrap();
    c.eval(include_str!("../src/bindings/harden.js")).unwrap();
    let error = c.eval("storage.sqlite.open('app:/data/db')").unwrap_err();
    assert!(error.contains("harden"), "{error}");
    assert!(!c.directory.join("data/db").exists());
}
