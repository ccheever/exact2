//! Where a `use` specifier leads (LLP 1091 D8/D9): `./` and `../` paths
//! inside the using file's root (its app, or its package), `exact:` modules
//! compiled into the compiler, and packages found through `node_modules` as
//! Node finds them.

use std::path::{Path, PathBuf};

/// The modules every app can name without installing anything.
const BUILTINS: &[(&str, &str)] = &[("exact:motion", include_str!("../../lib/motion.contract"))];

/// Where a loaded source came from: what bounds its own relative uses, and
/// what a capture or a watch must take with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// A file of the app, bounded by the app's directory.
    App,
    /// A file of an installed package, bounded by the package's directory.
    Package {
        /// The package's name, as `use` writes it.
        name: String,
        /// The package's version, as its `package.json` says.
        version: String,
        /// The package's directory, canonical.
        root: PathBuf,
        /// The `package.json` the resolution read.
        manifest: PathBuf,
    },
    /// An `exact:` module, compiled in.
    Builtin,
}

impl Origin {
    /// The directory this source's relative uses must stay inside.
    pub(crate) fn root<'a>(&'a self, app_root: &'a Path) -> Option<&'a Path> {
        match self {
            Origin::App => Some(app_root),
            Origin::Package { root, .. } => Some(root),
            Origin::Builtin => None,
        }
    }
}

/// A resolved specifier: the file's identity (its canonical path, or the
/// `exact:` name), its text when compiled in, and its origin.
pub(crate) struct Resolved {
    pub(crate) key: PathBuf,
    pub(crate) builtin: Option<&'static str>,
    pub(crate) origin: Origin,
}

/// A refusal: its id and message; the caller adds the span.
pub(crate) type Refusal = (&'static str, String);

/// Resolve `spec`, written in a file in `dir` whose origin is `from`.
/// Every `package.json` a resolution reads is pushed to `consulted`, even
/// when it then refuses: a watcher must see the manifest a fix will edit.
pub(crate) fn resolve(
    spec: &str,
    dir: &Path,
    from: &Origin,
    app_root: &Path,
    consulted: &mut Vec<PathBuf>,
) -> Result<Resolved, Refusal> {
    if spec.starts_with("exact:") {
        return BUILTINS
            .iter()
            .find(|(name, _)| *name == spec)
            .map(|(name, text)| Resolved {
                key: PathBuf::from(name),
                builtin: Some(text),
                origin: Origin::Builtin,
            })
            .ok_or_else(|| {
                let known: Vec<_> = BUILTINS.iter().map(|(n, _)| format!("`{n}`")).collect();
                (
                    "contract-use-builtin",
                    format!(
                        "no built-in module `{spec}`; the built-ins are {}",
                        known.join(", ")
                    ),
                )
            });
    }
    if spec.starts_with("./") || spec.starts_with("../") {
        return relative(spec, dir, from, app_root, consulted);
    }
    if spec.starts_with('/') || spec.contains('\\') || spec.is_empty() {
        return Err((
            "contract-use-path",
            format!("`{spec}` is not a portable specifier: write `./file.contract`, `exact:name`, or a package name"),
        ));
    }
    if spec
        .split('/')
        .next()
        .is_some_and(|first| first.ends_with(".contract"))
    {
        return Err((
            "contract-use-path",
            format!("`{spec}` is a file, not a package: write `./{spec}`"),
        ));
    }
    if *from == Origin::Builtin {
        return Err((
            "contract-use-path",
            format!("a built-in module uses only built-ins, not `{spec}`"),
        ));
    }
    package(spec, dir, consulted)
}

fn relative(
    spec: &str,
    dir: &Path,
    from: &Origin,
    app_root: &Path,
    consulted: &mut Vec<PathBuf>,
) -> Result<Resolved, Refusal> {
    let Some(root) = from.root(app_root) else {
        return Err((
            "contract-use-path",
            format!("a built-in module uses only built-ins, not `{spec}`"),
        ));
    };
    if spec
        .split('/')
        .enumerate()
        .any(|(i, part)| part.is_empty() || (part == "." && i > 0))
    {
        return Err((
            "contract-use-path",
            format!("`{spec}` has an empty or `.` segment"),
        ));
    }
    let target = dir.join(spec);
    // Watched by the path written (without its `.` segments, as watchers
    // name it): creating it, or retargeting a link there, builds again.
    consulted.push(lexical(&target));
    let key = target.canonicalize().map_err(|e| {
        (
            "contract-use-unreadable",
            format!("{}: {e}", target.display()),
        )
    })?;
    if !key.starts_with(root) {
        let what = match from {
            Origin::Package { name, .. } => format!("the package `{name}`"),
            _ => "the app directory".to_owned(),
        };
        return Err(("contract-use-path", format!("`{spec}` leaves {what}")));
    }
    contract_file(spec, &key)?;
    Ok(Resolved {
        key,
        builtin: None,
        origin: from.clone(),
    })
}

/// `path` with its `.` and `..` segments folded as written, the way a
/// watcher names the file (Node's `resolve`): the path to watch, not the
/// file to read, which resolution canonicalizes.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn contract_file(spec: &str, key: &Path) -> Result<(), Refusal> {
    if key.extension().and_then(|extension| extension.to_str()) == Some("contract") {
        Ok(())
    } else {
        Err((
            "contract-use-path",
            format!("`{spec}` resolves to a file that is not `.contract`"),
        ))
    }
}

/// `@scope/name[/sub]` or `name[/sub]`, found in the nearest `node_modules`
/// above `dir` that has it, then mapped through its `package.json`.
fn package(spec: &str, dir: &Path, consulted: &mut Vec<PathBuf>) -> Result<Resolved, Refusal> {
    let parts: Vec<&str> = spec.split('/').collect();
    let scoped = spec.starts_with('@');
    let take = if scoped { 2 } else { 1 };
    if parts.len() < take
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err((
            "contract-use-path",
            format!("`{spec}` is not a package name"),
        ));
    }
    let name = parts[..take].join("/");
    let sub = parts[take..].join("/");
    // Every nearer place an install would win from, watched even when one
    // farther up resolves: installing there changes what the name means.
    let mut nearer = Vec::new();
    let found = dir
        .ancestors()
        .map(|a| a.join("node_modules").join(&name))
        .find(|candidate| {
            let manifest = candidate.join("package.json");
            let here = manifest.is_file();
            if !here {
                nearer.push(manifest);
            }
            here
        })
        .ok_or_else(|| {
            // Where an install would put it, nearest first, watched so that
            // installing it builds again.
            consulted.extend(
                dir.ancestors()
                    .map(|a| a.join("node_modules").join(&name).join("package.json")),
            );
            (
                "contract-use-package",
                format!(
                    "no package `{name}` in a `node_modules` above {}: add it to the app's `package.json` and install (`bun add {name}`, or `\"{name}\": \"file:../path\"` for a local library)",
                    dir.display()
                ),
            )
        })?;
    // The manifest is the package's, where it really is: a linked directory,
    // or Bun's `file:` install of per-file links, leads to the library's
    // own `package.json`, and the package is the directory that holds it.
    consulted.extend(nearer);
    // The install's own path too: relinking it to another directory changes
    // what it resolves to without touching either directory's files.
    consulted.push(found.join("package.json"));
    let manifest = found.join("package.json").canonicalize().map_err(|e| {
        (
            "contract-use-unreadable",
            format!("{}: {e}", found.display()),
        )
    })?;
    consulted.push(manifest.clone());
    let root = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
    let text = std::fs::read_to_string(&manifest).map_err(|e| {
        (
            "contract-use-unreadable",
            format!("{}: {e}", manifest.display()),
        )
    })?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        (
            "contract-use-package",
            format!("{}: not JSON: {e}", manifest.display()),
        )
    })?;
    let version = json["version"].as_str().unwrap_or("").to_owned();
    let exports = &json["exports"];
    let entry = match entry(exports, &sub) {
        Some(entry) => entry,
        // As Node: a package with `exports` offers only what it lists.
        None if !exports.is_null() => {
            return Err((
                "contract-use-package",
                format!(
                    "the package `{name}` does not export `{}` (its `package.json` `exports`)",
                    if sub.is_empty() { "." } else { &sub }
                ),
            ))
        }
        None if sub.is_empty() => "./index.contract".to_owned(),
        None => format!("./{sub}"),
    };
    let Some(relative) = entry.strip_prefix("./").filter(|r| {
        !r.split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    }) else {
        return Err((
            "contract-use-package",
            format!("`{name}`'s `exports` maps `{spec}` to `{entry}`, which is not a `./` path inside the package"),
        ));
    };
    let target = found.join(relative);
    // The export by the path the package offers it at: retargeting a link
    // there changes the file without touching the old one.
    consulted.push(target.clone());
    let key = target.canonicalize().map_err(|e| {
        (
            "contract-use-unreadable",
            format!("`{spec}`: {}: {e}", target.display()),
        )
    })?;
    // An exported file that leads out of the package (a link into another)
    // is not the package's to offer.
    if !key.starts_with(&root) {
        return Err((
            "contract-use-path",
            format!("`{spec}` leaves the package `{name}`"),
        ));
    }
    contract_file(spec, &key)?;
    Ok(Resolved {
        key,
        builtin: None,
        origin: Origin::Package {
            name,
            version,
            root,
            manifest,
        },
    })
}

/// What `exports` maps a subpath to: a string, or the `contract` or
/// `default` condition of an object, for `.` or `./sub`.
fn entry(exports: &serde_json::Value, sub: &str) -> Option<String> {
    let key = if sub.is_empty() {
        ".".to_owned()
    } else {
        format!("./{sub}")
    };
    let target = match exports {
        serde_json::Value::String(_) if sub.is_empty() => Some(exports),
        serde_json::Value::Object(map) if map.keys().any(|k| k.starts_with('.')) => map.get(&key),
        serde_json::Value::Object(_) if sub.is_empty() => Some(exports),
        _ => None,
    }?;
    condition(target)
}

fn condition(target: &serde_json::Value) -> Option<String> {
    match target {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => ["contract", "default"]
            .iter()
            .filter_map(|c| map.get(*c))
            .find_map(condition),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exports_map_a_subpath_through_a_string_an_object_or_a_condition() {
        assert_eq!(
            entry(&json!("./ui.contract"), ""),
            Some("./ui.contract".into())
        );
        assert_eq!(entry(&json!("./ui.contract"), "button"), None);
        let map = json!({".": {"contract": "./index.contract", "default": "./index.js"}, "./button": "./src/button.contract"});
        assert_eq!(entry(&map, ""), Some("./index.contract".into()));
        assert_eq!(entry(&map, "button"), Some("./src/button.contract".into()));
        assert_eq!(entry(&map, "card"), None);
        assert_eq!(
            entry(&json!({"default": "./a.contract"}), ""),
            Some("./a.contract".into())
        );
    }
}
