//! The app's generated sources (the code colours' lexers, icon data, morph plans, test vectors) are built, not
//! committed: `tools/generate/generate.mjs` writes them, and names its inputs and outputs to Cargo
//! (`rerun-if-changed`). It runs here because this crate embeds two of them (`pierre-icons.tsv`, `highlights.wasm`)
//! and every host's bake, which reads the rest, runs after this crate is built.
//!
//! A build for a named target (`--target`, as `host/apple/build.mjs` builds) compiles this crate
//! twice, as the host crate's build-dependency and for the target, and Cargo may run the two build
//! scripts at once. An exclusive lock makes the second wait, and it then finds every output written.
//! The lock is a file of its own: on Windows a locked file cannot be read, so locking the generator
//! would keep Bun from running it.

fn main() {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("t3-code-generate.lock"))
        .expect("open the generator's lock file");
    lock.lock().expect("lock the generator's lock file");
    let status = exact_bake::bun()
        .arg("../tools/generate/generate.mjs")
        .status()
        .map_err(exact_bake::bun_error)
        .expect("run tools/generate/generate.mjs");
    assert!(status.success(), "tools/generate/generate.mjs failed");
}
