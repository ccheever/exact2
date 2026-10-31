//! The data module's native slot filled from a package (LLP 1154 D2, D3):
//! the one direct dependency whose `exact.json` declares
//! `"native": {"rust": <root file>, "class": <type>}`. The bake joins the
//! root to the app's crate by a `#[path]` module, as `packages.rs` joins a
//! composed implementation, and writes the factory `with_native` takes.
//! The other fields of `native`, and `web`, are the library's and the web
//! build's: nothing here reads them.

use crate::package_views::{dependencies, export, located, read};
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

/// The package among the app's direct dependencies that declares `native`,
/// if one does; refused by name when two do (the slot is one per app) or
/// when the declaration lacks what the bake writes. Says to Cargo the files
/// whose change asks for another bake.
pub fn native_slot(app_root: &Path) -> Result<Option<NativeSlot>, String> {
    // Absolute, so the search walks past a relative root (`../../..`).
    let app_root =
        std::fs::canonicalize(app_root).map_err(|e| format!("{}: {e}", app_root.display()))?;
    if let Some(manifest) = (app_root.ancestors())
        .map(|dir| dir.join("package.json"))
        .find(|file| file.is_file())
    {
        println!("cargo:rerun-if-changed={}", manifest.display());
    }
    let mut found: Vec<NativeSlot> = Vec::new();
    for name in dependencies(&app_root) {
        let Some((dir, json)) = located(&app_root, &name) else {
            continue;
        };
        let native = &json["native"];
        if native.is_null() {
            continue;
        }
        if let Some(first) = found.first() {
            return Err(format!(
                "bake-native-twice: {} and {name} each declare a native module (exact.json \"native\"), and an app has one: depend on one of them",
                first.package
            ));
        }
        // The declaration `exports` "." names; its paths are relative to it.
        let declared = (read(&dir.join("package.json")).as_ref())
            .and_then(|manifest| export(&manifest["exports"], ".").map(|main| dir.join(main)))
            .unwrap_or_else(|| dir.join("exact.json"));
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
        println!("cargo:rerun-if-changed={}", declared.display());
        println!("cargo:rerun-if-changed={}", root.display());
        found.push(NativeSlot {
            package: name,
            root,
            class: class.to_owned(),
        });
    }
    Ok(found.pop())
}
