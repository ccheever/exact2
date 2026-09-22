//! The project harness the integration tests share.
//!
//! Extracted when `tests/loader.rs` outgrew the line cap. Splitting the
//! tests was the right answer to that cap rather than raising the baseline:
//! module loading and package resolution are separate concerns that had
//! only ever shared a file.
#![cfg(feature = "hermes")]
#![allow(dead_code)]

use std::path::PathBuf;

use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::{ModuleGrants, Root};

pub struct Project(pub PathBuf);

impl Project {
    pub fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let instance = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "ibex2-loader-{name}-{}-{instance}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("project dir");
        Self(dir)
    }

    pub fn file(&self, name: &str, source: &str) -> &Self {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(path, source).expect("write");
        self
    }

    pub fn run(&self, entry: &str, manifest: &str) -> (Vec<String>, Option<String>) {
        self.run_with(entry, manifest, None, false)
    }

    pub fn compiler(&self) -> Option<ibex2::bytecode::Compiler> {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        ibex2::bytecode::Compiler::discover(&root, self.0.join(".ibex2/cache")).ok()
    }

    pub fn engine_dir() -> std::path::PathBuf {
        match std::env::var("IBEX2_VANILLA_HERMES_DIR") {
            Ok(path) => std::path::PathBuf::from(path),
            Err(_) if cfg!(target_vendor = "apple") => {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../ios/Frameworks-vanilla")
            }
            Err(_) => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../linux/Frameworks-vanilla"),
        }
    }

    pub fn run_with(
        &self,
        entry: &str,
        manifest: &str,
        compiler: Option<ibex2::bytecode::Compiler>,
        precompiled_only: bool,
    ) -> (Vec<String>, Option<String>) {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
        assert!(rt.install_stdlib());
        rt.install_bindings().expect("bindings");
        rt.set_loader_with(
            Root::Declared(self.0.clone()),
            ModuleGrants::parse(manifest).expect("manifest"),
            compiler,
            precompiled_only,
        )
        .expect("manifest binds to the project");
        let error = rt.run_entry(entry).err().map(|e| e.to_string());
        // Not a network budget. In a *debug* test binary the first
        // NSURLSession construction in the process costs 3-9s, because dyld
        // resolves the network stack's 880-odd images against a 34MB
        // unstripped symbol table. Release pays 2ms. The budget has to clear
        // that tax or tests that touch the network fail for reasons that have
        // nothing to do with what they assert. See issues/20260828-*.
        rt.run_to_quiescence(std::time::Duration::from_secs(45));
        let output = rt.drain_console().into_iter().map(|r| r.message).collect();
        (output, error)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
