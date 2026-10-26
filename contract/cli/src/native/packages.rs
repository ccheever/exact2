//! The Rust host builder (LLP 1133.001 D5's `rust-static`, built by LLP
//! 1133.022 D10 item 5): the Linux or Windows implementation bound to each
//! interface the plan's modules name, compiled as a module of the app's own
//! host crate, and the table the host binds (`exact_linux::package`).
//!
//! Each implementation is chosen as the other builders choose (LLP 1133.001
//! D4, `implementationFor` in `js/bake/src/typescript.mjs`): the one
//! `app.json` `implementations` names for the target, else the app's one
//! direct dependency that implements the interface there, else the one the
//! interface's package carries. It declares `"sources": {"rust": [root]}`
//! and `"class"`, a type in that root implementing `Package`.
//!
//! The root joins the app's crate by a `#[path]` module, as an app's own
//! hatches join it by `include!` (LLP 1075.003.000.001 §5): its submodules
//! resolve beside it, and it names exact2's crates the app's crate already
//! depends on (`exact_linux::package`). No Cargo manifest is generated, so
//! a package's Rust takes no crate of its own yet.

use std::path::{Path, PathBuf};

use crate::package_views::{dependencies, export, located, read};
use serde_json::Value;

/// One bound implementation: its extension, the root file, the class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The interface's name after its scope (`gamepad`).
    pub extension: String,
    /// The implementation's root Rust file.
    pub root: PathBuf,
    /// The type in it that implements `exact_linux::package::Package`.
    pub class: String,
}

/// The implementations bound for `target` (`linux` or `windows`) to the
/// interfaces `extensions` names, each extension once; a refusal by name
/// when the choice is ambiguous or an implementation is missing what the
/// builder reads. An extension with no Rust implementation for the target
/// is left out: it answers as absent there.
pub fn bound(app_root: &Path, extensions: &[&str], target: &str) -> Result<Vec<Bound>, String> {
    let mut out = Vec::new();
    for (name, root) in interfaces(app_root) {
        let extension = name.rsplit('/').next().unwrap_or(&name).to_owned();
        if !extensions.contains(&extension.as_str())
            || out.iter().any(|b: &Bound| b.extension == extension)
        {
            continue;
        }
        let Some((dir, json)) = implementation_for(app_root, &name, &root, target)? else {
            continue;
        };
        if !matches!(json["platform"].as_str(), Some("linux" | "windows")) {
            continue;
        }
        let Some(file) = json["sources"]["rust"][0].as_str() else {
            continue;
        };
        let class = json["class"].as_str().ok_or_else(|| {
            format!(
                "bake-implementation-class: {} declares Rust sources and no \"class\"",
                dir.join("exact.json").display()
            )
        })?;
        // Absolute: the `#[path]` is read from the build's OUT_DIR.
        let root = std::fs::canonicalize(dir.join(file)).map_err(|e| {
            format!(
                "bake-implementation-source: {} names {file}: {e}",
                dir.join("exact.json").display()
            )
        })?;
        out.push(Bound {
            extension,
            root,
            class: class.to_owned(),
        });
    }
    Ok(out)
}

/// The text an app's host crate includes for `target`: each bound root as a
/// `#[path]` module and `exact_packages()`, the table `exact_linux::Composed`
/// takes. With nothing bound the table is empty.
pub fn rust_packages(app_root: &Path, extensions: &[&str], target: &str) -> Result<String, String> {
    let bound = match target {
        "linux" | "windows" => bound(app_root, extensions, target)?,
        _ => Vec::new(),
    };
    rerun_if_changed(app_root);
    let mut text = String::from(
        "// The packages' Rust implementations (LLP 1133.022 D10 item 5), written by the build; never edited.\n",
    );
    let mut entries = String::new();
    for b in &bound {
        let module = format!(
            "exact_package_{}",
            b.extension
                .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        );
        if let Some(dir) = b.root.parent() {
            println!(
                "cargo:rerun-if-changed={}",
                dir.join("exact.json").display()
            );
        }
        text += &format!(
            "#[path = {:?}]\n#[allow(dead_code, missing_docs, unused_imports, clippy::all)]\nmod {module};\n",
            b.root.display().to_string()
        );
        entries += &format!(
            "        exact_linux::package::Entry::of::<{module}::{}>({:?}),\n",
            b.class, b.extension
        );
    }
    text += "/// The packages this build composes, for `exact_linux::Composed`.\n#[allow(dead_code)]\nfn exact_packages() -> Vec<exact_linux::package::Entry> {\n    vec![\n";
    text += &entries;
    text += "    ]\n}\n";
    Ok(text)
}

/// [`rust_packages`] for the interfaces `plan`'s modules name, on the host
/// Cargo's `TARGET` triple builds for: `windows` or `linux`, and on Android
/// none, since its implementations are Kotlin.
pub fn plan_packages(
    app_root: &Path,
    plan: &exact_plan::Plan,
    triple: &str,
) -> Result<String, String> {
    let mut extensions: Vec<&str> = (plan.modules.iter())
        .map(|m| plan.str(m.extension))
        .collect();
    extensions.sort_unstable();
    extensions.dedup();
    let target = if triple.contains("android") {
        "android"
    } else if triple.contains("windows") {
        "windows"
    } else {
        "linux"
    };
    rust_packages(app_root, &extensions, target)
}

/// The app's manifests the choice reads: a new dependency or a new
/// `implementations` entry binds again.
fn rerun_if_changed(app_root: &Path) {
    for file in ["app.json", "package.json"] {
        let path = app_root.join(file);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

/// Every module interface installed above `app_root`, the nearest first:
/// its package's name and directory.
fn interfaces(app_root: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for dir in app_root.ancestors() {
        let modules = dir.join("node_modules");
        let Ok(entries) = std::fs::read_dir(&modules) else {
            continue;
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('@') {
                for inner in std::fs::read_dir(entry.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    names.push(format!("{name}/{}", inner.file_name().to_string_lossy()));
                }
            } else {
                names.push(name);
            }
        }
        names.sort();
        for name in names {
            if out.iter().any(|(n, ..)| *n == name) {
                continue;
            }
            if let Some((root, json)) = located(app_root, &name) {
                if json["kind"] == "interface" {
                    out.push((name, root));
                }
            }
        }
    }
    out
}

/// The implementation of interface `name` bound for `target`, with its
/// directory: LLP 1133.001 D4's order.
fn implementation_for(
    app_root: &Path,
    name: &str,
    root: &Path,
    target: &str,
) -> Result<Option<(PathBuf, Value)>, String> {
    let covers = |json: &Value| {
        json["kind"] == "implementation"
            && json["implements"]
                .as_str()
                .and_then(|i| i.rsplit_once('@'))
                .map_or(json["implements"].as_str() == Some(name), |(i, _)| {
                    i == name
                })
            && (json["targets"].as_array().into_iter().flatten()).any(|t| t == target)
    };
    let at = |package: &str| {
        let (dir, _) = located(app_root, package)?;
        let manifest = read(&dir.join("package.json"))?;
        let main = export(&manifest["exports"], ".")?;
        let file = dir.join(main);
        Some((file.parent()?.to_path_buf(), read(&file)?))
    };
    let manifest = read(&app_root.join("app.json")).unwrap_or_default();
    if let Some(named) = manifest["implementations"][name][target].as_str() {
        return match at(named) {
            Some((dir, json)) if covers(&json) => Ok(Some((dir, json))),
            found => Err(format!(
                "bake-implementation-missing: app.json names {named} for {name} on {target}, and {}",
                if found.is_some() {
                    format!("it does not implement it for {target}")
                } else {
                    "it is not installed".to_owned()
                }
            )),
        };
    }
    let direct: Vec<(String, PathBuf, Value)> = (dependencies(app_root).into_iter())
        .filter(|d| d != name)
        .filter_map(|d| at(&d).map(|(dir, json)| (d, dir, json)))
        .filter(|(_, _, json)| covers(json))
        .collect();
    if direct.len() > 1 {
        return Err(format!(
            "bake-implementation-ambiguous: {} and {} each implement {name} for {target}; name one in app.json: \"implementations\": {{ \"{name}\": {{ \"{target}\": \"{}\" }} }}",
            direct[0].0, direct[1].0, direct[0].0
        ));
    }
    if let Some((_, dir, json)) = direct.into_iter().next() {
        return Ok(Some((dir, json)));
    }
    let Some(manifest) = read(&root.join("package.json")) else {
        return Ok(None);
    };
    let exports = manifest["exports"].as_object().cloned().unwrap_or_default();
    for key in exports.keys().filter(|k| k.as_str() != ".") {
        let Some(file) = export(&manifest["exports"], key) else {
            continue;
        };
        let path = root.join(file);
        if let (Some(json), Some(dir)) = (read(&path), path.parent()) {
            if file.ends_with("exact.json") && covers(&json) {
                return Ok(Some((dir.to_path_buf(), json)));
            }
        }
    }
    Ok(None)
}

/// Writes `packages.rs`, the Rust implementations a desktop host composes
/// (LLP 1133.022 D10 item 5), into a baked Linux or Windows crate's `out`
/// from its encoded plan (on another platform, nothing), leaving an
/// unchanged file alone so Cargo does not rebuild for it.
pub fn write_desktop_packages(
    platform: &str,
    app_root: &Path,
    plan: &[u8],
    triple: &str,
    out: &Path,
) -> Result<(), String> {
    if !matches!(platform, "linux" | "windows") {
        return Ok(());
    }
    let plan = exact_plan::Plan::decode(plan).map_err(|e| format!("{e:?}"))?;
    let text = plan_packages(app_root, &plan, triple)?;
    let path = out.join("packages.rs");
    if std::fs::read(&path).ok().as_deref() == Some(text.as_bytes()) {
        return Ok(());
    }
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}
