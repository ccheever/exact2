//! Cargo invalidation for optional asset roots; the owned filesystem gate
//! remains the authority for which bytes a bake may read (LLP 1030.002 D1).

use std::path::{Path, PathBuf};

/// buildBake recomputes this inventory before each Cargo invocation, so its
/// environment fingerprint detects first creation without scanning app-local
/// outputs. Direct Cargo has no such inventory and keeps conservative watches.
pub(super) fn asset_tree(
    app: &Path,
    root: &str,
    outputs: &[PathBuf],
    declared: Option<&str>,
) -> Option<PathBuf> {
    let path = app.join(root);
    if declared.is_some_and(|roots| !roots.split(',').any(|name| name == root))
        && std::fs::symlink_metadata(&path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        return None;
    }
    Some(optional_tree(&path, outputs))
}

/// A missing Cargo input is perpetually dirty. Watch its nearest existing
/// ancestor instead, unless Cargo's recursive scan would include build output.
/// In that layout keep the conservative missing input: first creation must
/// never be missed, and scanning our own outputs cannot make a build warm.
pub(super) fn optional_tree(path: &Path, outputs: &[PathBuf]) -> PathBuf {
    let mut parent = path;
    loop {
        match std::fs::symlink_metadata(parent) {
            Ok(_) if parent == path => return path.into(),
            Ok(meta) if meta.is_dir() => break,
            Ok(_) => return path.into(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let Some(next) = parent.parent() else {
                    return path.into();
                };
                parent = next;
            }
            Err(_) => return path.into(),
        }
    }
    let Ok(resolved_parent) = parent.canonicalize() else {
        return path.into();
    };
    for output in outputs {
        // Output destinations can be absent or reached through a target-dir
        // symlink. Resolution here only rules OUT a broad watch; it never
        // authorizes an asset read or changes the watched source spelling.
        let Ok(output) = std::path::absolute(output) else {
            return path.into();
        };
        let Some(resolved) = resolve_output(&output) else {
            return path.into();
        };
        if resolved.starts_with(&resolved_parent) {
            return path.into();
        }
    }
    // Cargo follows descendant symlinks. Even an output spelled entirely
    // outside this ancestor can be reachable through an in-tree alias. Do
    // not broaden over links (or unreadable subtrees); inspect names/types
    // only, leaving asset contents to the owned filesystem gate.
    if contains_symlink(parent).unwrap_or(true) {
        return path.into();
    }
    parent.into()
}

fn contains_symlink(root: &Path) -> std::io::Result<bool> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Ok(true);
            }
            if kind.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(false)
}

fn resolve_output(path: &Path) -> Option<PathBuf> {
    if let Ok(path) = path.canonicalize() {
        return Some(path);
    }
    let mut resolved = resolve_output(path.parent()?)?;
    match path.components().next_back()? {
        std::path::Component::ParentDir => {
            resolved.pop();
        }
        std::path::Component::CurDir => {}
        std::path::Component::Normal(name) => resolved.push(name),
        _ => return None,
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "exact-asset-watch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("app/consumer")).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn broad_watches_exclude_nested_and_aliased_outputs() {
        let f = Fixture::new();
        let app = f.0.join("app");
        let missing = app.join("gpu/shaders");
        assert_eq!(optional_tree(&missing, &[f.0.join("target")]), app);
        assert_eq!(
            optional_tree(&missing, &[app.join("consumer/../bake/new")]),
            missing
        );
        fs::create_dir(app.join("gpu")).unwrap();
        assert_eq!(
            optional_tree(&missing, &[app.join("target")]),
            app.join("gpu")
        );
        assert_eq!(optional_tree(&missing, &[app.join("gpu/out")]), missing);
        fs::write(app.join("invalid"), "file").unwrap();
        let invalid = app.join("invalid/shaders");
        assert_eq!(optional_tree(&invalid, &[f.0.join("target")]), invalid);
        #[cfg(unix)]
        {
            let alias = f.0.join("output-alias");
            std::os::unix::fs::symlink(app.join("gpu"), &alias).unwrap();
            assert_eq!(optional_tree(&missing, &[alias.join("new")]), missing);
            let dangling = app.join("dangling");
            std::os::unix::fs::symlink(f.0.join("absent"), &dangling).unwrap();
            assert_eq!(optional_tree(&dangling, &[f.0.join("target")]), dangling);
            fs::remove_file(dangling).unwrap();
            fs::create_dir(f.0.join("target")).unwrap();
            let inside_alias = app.join("output-alias");
            std::os::unix::fs::symlink(f.0.join("target"), &inside_alias).unwrap();
            let absent = app.join("deck");
            // Both output spellings, and an otherwise unknown descendant
            // alias, must avoid Cargo's recursive traversal into that link.
            for output in [inside_alias, f.0.join("target"), f.0.join("other")] {
                assert_eq!(optional_tree(&absent, &[output]), absent);
            }
        }
    }

    // Exercise Cargo itself: a path-selection unit test cannot prove that a
    // second invocation stays warm or that a previously absent root is noticed.
    #[test]
    fn inventoried_roots_stay_warm_with_app_local_outputs() {
        let f = Fixture::new();
        let app = f.0.join("app");
        let package = app.join("consumer");
        fs::write(package.join("Cargo.toml"), "[package]\nname='asset-inventory-fixture'\nversion='0.0.0'\nedition='2024'\n[workspace]\n").unwrap();
        fs::create_dir(package.join("src")).unwrap();
        fs::write(package.join("src/lib.rs"), "pub fn value() {}\n").unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt/watch.rs");
        fs::write(package.join("build.rs"), format!(r#"
#[path = {source:?}] mod watch;
use std::path::PathBuf;
fn main() {{
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=EXACT_ASSET_ROOTS");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let declared = std::env::var("EXACT_ASSET_ROOTS").ok();
    for root in ["assets", "deck"] {{
        if let Some(path) = watch::asset_tree(std::path::Path::new(".."), root, &[out.clone()], declared.as_deref()) {{
            println!("cargo:rerun-if-changed={{}}", path.display());
        }}
    }}
    let record = PathBuf::from(std::env::var_os("WATCH_RECORD").unwrap());
    let previous = std::fs::read_to_string(&record).unwrap_or_default();
    std::fs::write(record, format!("{{previous}}baked\n")).unwrap();
}}
"#)).unwrap();
        let record = app.join("builds");
        let build = |expected: usize| {
            let roots = ["assets", "deck"]
                .into_iter()
                .filter(|root| app.join(root).exists())
                .collect::<Vec<_>>()
                .join(",");
            let output = std::process::Command::new(env!("CARGO"))
                .args(["build", "--quiet", "--offline", "--manifest-path"])
                .arg(package.join("Cargo.toml"))
                .arg("--target-dir")
                .arg(app.join("target"))
                .env("WATCH_RECORD", &record)
                .env("EXACT_ASSET_ROOTS", roots)
                .env_remove("CARGO_BUILD_BUILD_DIR")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("RUSTFLAGS")
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                fs::read_to_string(&record).unwrap().lines().count(),
                expected
            );
        };
        build(1);
        build(1);
        fs::create_dir(app.join("deck")).unwrap();
        fs::write(app.join("deck/card"), "red").unwrap();
        build(2);
        build(2);
        fs::write(app.join("deck/card"), "blue").unwrap();
        build(3);
        fs::remove_file(app.join("deck/card")).unwrap();
        build(4);
        fs::remove_dir(app.join("deck")).unwrap();
        build(5);
        build(5);
        fs::create_dir(app.join("deck")).unwrap();
        build(6);
        build(6);
        // A direct Cargo invocation still watches absence conservatively.
        assert_eq!(
            asset_tree(&app, "assets", &[app.join("target")], None),
            Some(app.join("assets"))
        );
        // Even an incomplete supplied inventory never hides an existing root.
        assert_eq!(
            asset_tree(&app, "deck", &[app.join("target")], Some("")),
            Some(app.join("deck"))
        );
    }

    #[test]
    fn cargo_rebakes_asset_changes_but_not_unchanged_missing_roots() {
        let f = Fixture::new();
        let app = f.0.join("app");
        let package = app.join("consumer");
        fs::write(
            package.join("Cargo.toml"),
            "[package]\nname='asset-watch-fixture'\nversion='0.0.0'\nedition='2024'\n[workspace]\n",
        )
        .unwrap();
        fs::create_dir(package.join("src")).unwrap();
        fs::write(package.join("src/lib.rs"), "pub fn value() {}\n").unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/receipt/watch.rs");
        fs::write(package.join("build.rs"), format!(r#"
#[path = {source:?}] mod watch;
use std::path::PathBuf;
fn main() {{
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let app = PathBuf::from("..");
    for root in ["assets", "gpu/shaders"] {{
        println!("cargo:rerun-if-changed={{}}", watch::optional_tree(&app.join(root), &[out.clone()]).display());
    }}
    let record = PathBuf::from(std::env::var_os("WATCH_RECORD").unwrap());
    let previous = std::fs::read_to_string(&record).unwrap_or_default();
    std::fs::write(record, format!("{{previous}}baked\n")).unwrap();
}}
"#)).unwrap();
        let record = f.0.join("builds");
        let build = |expected: usize| {
            let output = std::process::Command::new(env!("CARGO"))
                .args(["build", "--quiet", "--offline", "--manifest-path"])
                .arg(package.join("Cargo.toml"))
                .arg("--target-dir")
                .arg(f.0.join("target"))
                .env("WATCH_RECORD", &record)
                .env_remove("CARGO_BUILD_BUILD_DIR")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("RUSTFLAGS")
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                fs::read_to_string(&record).unwrap().lines().count(),
                expected
            );
        };
        build(1);
        build(1);
        fs::create_dir_all(app.join("assets/nested")).unwrap();
        fs::write(app.join("assets/nested/theme"), "red").unwrap();
        build(2);
        build(2);
        fs::write(app.join("assets/nested/theme"), "blue").unwrap();
        build(3);
        fs::remove_file(app.join("assets/nested/theme")).unwrap();
        build(4);
        fs::remove_dir_all(app.join("assets")).unwrap();
        build(5);
        build(5);
        fs::create_dir(app.join("assets")).unwrap();
        build(6);
        fs::create_dir(app.join("gpu")).unwrap();
        build(7);
        build(7);
        fs::create_dir(app.join("gpu/shaders")).unwrap();
        build(8);
        fs::remove_dir_all(app.join("gpu")).unwrap();
        build(9);
        build(9);
    }
}
