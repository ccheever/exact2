//! The data module's native slot filled from a package (LLP 1154 D2, D3):
//! the one direct dependency whose `exact.json`, at the package's root,
//! declares `"native": {"rust": <root file>, "class": <type>, …}`. The
//! root file is the declaration whatever `exports` "." names, which for
//! such a package is its TypeScript driver; a module in LLP 1133.001's
//! sense is found through `exports` (`package_views.rs`). The bake joins
//! the shim's root to the app's crate by a `#[path]` module, as
//! `packages.rs` joins a composed implementation, and writes the factory
//! `with_native` takes; its library is linked by `native.library` and
//! `native.link` (`js/bake/src/library.rs`). This is the one place that
//! chooses the package, so the one place that refuses two.

use crate::package_views::dependencies;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The package that fills the app's native slot.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeSlot {
    /// The package's name, for a refusal's message.
    pub package: String,
    /// Its `exact.json`.
    pub declaration: PathBuf,
    /// The shim's root file, absolute: the `#[path]` is read from OUT_DIR.
    pub root: PathBuf,
    /// The type in it that implements `exact_js::NativeModule` and has
    /// `fn new(app: &str) -> Self`.
    pub class: String,
    /// The declaration's `native` object, whose `library` and `link` the
    /// link reads.
    pub native: Value,
}

/// What [`native_slot`] chose, and every file it read to choose: a change to
/// any of them asks for the choice again (`cargo:rerun-if-changed`).
#[derive(Debug, Clone, PartialEq)]
pub struct NativeSelection {
    /// The package that fills the slot, if one does.
    pub slot: Option<NativeSlot>,
    /// The app's `package.json`, each direct dependency's `package.json` and
    /// its `exact.json` where it has one, and the chosen shim's root.
    pub watched: Vec<PathBuf>,
}

/// The package among the app's direct dependencies that declares `native`,
/// if one does; refused by name when two do (`bake-native-twice`: the slot
/// is one per app), when the declaration lacks what the bake writes, when a
/// dependency is not installed (`bake-native-missing`), and when an
/// `exact.json` that is there cannot be read (`bake-native-declaration`).
/// Cargo cannot watch a missing file without running the script on every
/// build, so only files that exist are watched: a dependency that gains an
/// `exact.json` is seen when its `package.json` changes, as installing
/// another version changes it.
pub fn native_slot(app_root: &Path) -> Result<NativeSelection, String> {
    // Absolute, so the search walks past a relative root (`../../..`).
    let app_root =
        std::fs::canonicalize(app_root).map_err(|e| format!("{}: {e}", app_root.display()))?;
    let mut watched: Vec<PathBuf> = (app_root.ancestors())
        .map(|dir| dir.join("package.json"))
        .find(|file| file.is_file())
        .into_iter()
        .collect();
    let mut slot: Option<NativeSlot> = None;
    for name in dependencies(&app_root) {
        let dir = (app_root.ancestors())
            .map(|a| a.join("node_modules").join(&name))
            .find(|candidate| candidate.join("package.json").is_file())
            .ok_or_else(|| {
                format!(
                    "bake-native-missing: the app depends on {name}, which is not installed: run bun install"
                )
            })?;
        watched.push(dir.join("package.json"));
        let declaration = dir.join("exact.json");
        let text = match std::fs::read_to_string(&declaration) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(format!(
                    "bake-native-declaration: {name}'s exact.json ({}) cannot be read: {e}",
                    declaration.display()
                ))
            }
        };
        watched.push(declaration.clone());
        let json: Value = serde_json::from_str(&text).map_err(|e| {
            format!(
                "bake-native-declaration: {name}'s exact.json ({}) is not JSON: {e}",
                declaration.display()
            )
        })?;
        let native = &json["native"];
        if native.is_null() {
            continue;
        }
        if let Some(first) = &slot {
            return Err(format!(
                "bake-native-twice: {} and {name} each declare a native module (exact.json \"native\"), and an app has one: depend on one of them",
                first.package
            ));
        }
        let file = native["rust"].as_str().ok_or_else(|| {
            format!(
                "bake-native-source: {} declares \"native\" with no \"rust\", the shim's root file",
                declaration.display()
            )
        })?;
        let class = native["class"]
            .as_str()
            .filter(|class| {
                class.split("::").all(|part| {
                    part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                        && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
            })
            .ok_or_else(|| {
                format!(
                    "bake-native-class: {} declares \"native\" with no \"class\" naming a Rust type in {file}",
                    declaration.display()
                )
            })?;
        let root = std::fs::canonicalize(dir.join(file)).map_err(|e| {
            format!(
                "bake-native-source: {} names {file}: {e}",
                declaration.display()
            )
        })?;
        slot = Some(NativeSlot {
            package: name,
            declaration,
            root,
            class: class.to_owned(),
            native: native.clone(),
        });
    }
    watched.extend(slot.as_ref().map(|slot| slot.root.clone()));
    Ok(NativeSelection { slot, watched })
}
