//! The data module's native slot filled from a package (LLP 1154 D2, D3):
//! the one direct dependency whose `exact.json` declares
//! `"native": {"rust": <root file>, "class": <type>}`. The bake joins the
//! root to the app's crate by a `#[path]` module, as `packages.rs` joins a
//! composed implementation, and writes the factory `with_native` takes.
//! The other fields of `native`, and `web`, are the library's and the web
//! build's: nothing here reads them.

use crate::package_views::{dependencies, export, read};
use std::path::{Path, PathBuf};

/// The package that fills the app's native slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSlot {
    /// The package's name, for a refusal's message.
    pub package: String,
    /// The shim's root file, absolute: the `#[path]` is read from OUT_DIR.
    pub root: PathBuf,
    /// The type in it that implements `exact_js::NativeModule` and has
    /// `fn new(app: &str) -> Self`.
    pub class: String,
}

/// What [`native_slot`] chose, and every file it read to choose: a change to
/// any of them asks for the choice again (`cargo:rerun-if-changed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelection {
    /// The package that fills the slot, if one does.
    pub slot: Option<NativeSlot>,
    /// The app's `package.json`, each direct dependency's `package.json` and
    /// the `exact.json` its `exports` "." names, and the chosen shim's root.
    pub watched: Vec<PathBuf>,
}

/// The package among the app's direct dependencies that declares `native`,
/// if one does; refused by name when two do (the slot is one per app) or
/// when the declaration lacks what the bake writes. A dependency that is not
/// installed (`bake-native-missing`), or whose exported `exact.json` is not
/// there (`bake-native-declaration`), is refused too: Cargo cannot watch a
/// missing file without running the script on every build, so every file
/// the choice reads must exist.
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
        // Refused rather than skipped: a missing file cannot be watched, so an
        // install that later declared `native` would not ask for the choice again.
        let dir = (app_root.ancestors())
            .map(|a| a.join("node_modules").join(&name))
            .find(|candidate| candidate.join("package.json").is_file())
            .ok_or_else(|| {
                format!(
                    "bake-native-missing: the app depends on {name}, which is not installed: run bun install"
                )
            })?;
        watched.push(dir.join("package.json"));
        // The declaration `exports` "." names; its paths are relative to it.
        let Some(declared) = (read(&dir.join("package.json")).as_ref())
            .and_then(|manifest| export(&manifest["exports"], ".").map(|main| dir.join(main)))
            .filter(|main| main.ends_with("exact.json"))
        else {
            continue;
        };
        if !declared.is_file() {
            return Err(format!(
                "bake-native-declaration: {name}'s package.json exports {}, which does not exist",
                declared.display()
            ));
        }
        watched.push(declared.clone());
        let json = read(&declared).unwrap_or_default();
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
        let base = declared.parent().unwrap_or(&dir);
        let file = native["rust"].as_str().ok_or_else(|| {
            format!(
                "bake-native-source: {} declares \"native\" with no \"rust\", the shim's root file",
                declared.display()
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
                    declared.display()
                )
            })?;
        let root = std::fs::canonicalize(base.join(file)).map_err(|e| {
            format!(
                "bake-native-source: {} names {file}: {e}",
                declared.display()
            )
        })?;
        slot = Some(NativeSlot {
            package: name,
            root,
            class: class.to_owned(),
        });
    }
    watched.extend(slot.as_ref().map(|slot| slot.root.clone()));
    Ok(NativeSelection { slot, watched })
}
