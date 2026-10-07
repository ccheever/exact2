//! Every target with a pinned Hermes bundle links the engine: `build.rs`
//! builds the shim for its operating system. Read from the bundle table
//! itself, so a target added there without the shim fails here.

#[path = "../../engine_os.rs"]
mod engine_os;

use engine_os::ENGINE_OS;

/// The Rust target triples in `hermes-lean-sys`'s `PINNED_BUNDLES`.
fn pinned_targets() -> Vec<String> {
    let table = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../vendor/ibex/crates/hermes-lean-sys/build_support.rs");
    let source = std::fs::read_to_string(&table).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (git submodule update --init vendor/ibex)",
            table.display()
        )
    });
    let start = source
        .find("PINNED_BUNDLES")
        .expect("PINNED_BUNDLES in build_support.rs");
    let end = start
        + source[start..]
            .find("];")
            .expect("the end of PINNED_BUNDLES");
    source[start..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("target: \""))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect()
}

/// A target triple's operating system, as Cargo's `target_os` names it.
fn target_os(triple: &str) -> &'static str {
    if triple.contains("-apple-darwin") {
        "macos"
    } else if triple.contains("-apple-ios") {
        "ios"
    } else if triple.contains("-apple-tvos") {
        "tvos"
    } else if triple.contains("-linux-") {
        "linux"
    } else if triple.contains("-windows-") {
        "windows"
    } else {
        panic!("{triple}: no operating system known to this test")
    }
}

#[test]
fn every_pinned_bundle_target_links_the_engine() {
    let targets = pinned_targets();
    assert!(
        targets.iter().any(|t| t == "aarch64-apple-tvos-sim"),
        "{targets:?}"
    );
    let missing: Vec<_> = targets
        .iter()
        .filter(|t| !ENGINE_OS.contains(&target_os(t)))
        .collect();
    assert!(
        missing.is_empty(),
        "pinned Hermes bundles whose OS build.rs skips: {missing:?}"
    );
}
