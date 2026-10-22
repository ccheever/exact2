//! Package designs (LLP 1121 D11, stage 11 A): each linked package's
//! `<name>.tamber`, read as a source of the app's, and where one not there
//! yet would be, for a watcher.

use super::{digest, Package, Source, Sources};
use crate::resolve::{namespace, package_design, Origin};
use crate::CompileError;
use contract_syntax::Span;
use std::path::{Path, PathBuf};

/// Every package the app's sources are in, in link order: each a `use`
/// reached by its name, and a package inside one (its own named
/// `package.json`, vendored) that a relative `use` reached, as a
/// component's identity names it (`home`).
/// With each vendored package's `package.json` read, a build's input.
pub(crate) fn packages(sources: &Sources) -> (Vec<Package>, Vec<PathBuf>) {
    let mut all = sources.packages.clone();
    let mut read = Vec::new();
    for (path, origin) in sources.paths.iter().zip(&sources.origins) {
        let Origin::Package { root, .. } = origin else {
            continue;
        };
        let Some(dir) = path.parent() else { continue };
        for dir in dir
            .ancestors()
            .take_while(|d| d.starts_with(root) && *d != root.as_path())
        {
            let manifest = dir.join("package.json");
            if manifest.is_file() && !read.contains(&manifest) {
                read.push(manifest.clone());
            }
            let Some(json) = std::fs::read_to_string(&manifest)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            else {
                continue;
            };
            let Some(name) = json["name"].as_str() else {
                continue;
            };
            let root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
            if !all.iter().any(|p| p.root == root) {
                all.push(Package {
                    name: name.to_owned(),
                    version: json["version"].as_str().unwrap_or("").to_owned(),
                    manifest: root.join("package.json"),
                    root,
                });
            }
            break;
        }
    }
    // One directory installed under two names is one package here.
    let mut seen = Vec::new();
    all.retain(|p| {
        let first = !seen.contains(&p.root);
        seen.push(p.root.clone());
        first
    });
    (all, read)
}

/// The origin of a file of `package` (its design, an icon's SVG).
pub(super) fn origin(package: &Package) -> Origin {
    Origin::Package {
        name: package.name.clone(),
        version: package.version.clone(),
        root: package.root.clone(),
        manifest: package.manifest.clone(),
    }
}

/// The origin of a file of the named package `name` at `root` that no
/// resolution reached as a package (one vendored in the app, reached by a
/// relative `use`): its manifest's version, read where it is.
pub(super) fn named(name: &str, root: &Path) -> Origin {
    let manifest = root.join("package.json");
    let version = std::fs::read_to_string(&manifest)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|m| m["version"].as_str().map(str::to_string))
        .unwrap_or_default();
    Origin::Package {
        name: name.to_string(),
        version,
        root: root.to_path_buf(),
        manifest,
    }
}

fn refusal(id: &str, path: &Path, message: String) -> CompileError {
    CompileError {
        pass: "use",
        id: id.into(),
        message,
        span: Span::default(),
        file: Some(path.to_path_buf().into()),
        related: Box::new([]),
    }
}

/// The designs of the packages the app's sources are in (LLP 1121 D11), in
/// link order, each a source of its own so a refusal in it names it. A file
/// outside its package (a `"tamber"` path, or a link out) is refused, and
/// so are two that would take one namespace (the last part of their
/// names: `@exact/reading` and `@else/reading`), naming both: no app line
/// can tell their tokens apart. `bindable`: the app has code that can name
/// a token outside its views (D7), as its own design's `bindable`.
pub(super) fn designs(
    sources: &mut Sources,
    bindable: bool,
) -> Result<Vec<contract_syntax::DesignSource>, CompileError> {
    // A vendored package's `package.json` is an input as a `use`d one's is.
    let manifests = packages(sources).1;
    for m in manifests {
        if !sources.consulted.contains(&m) {
            sources.consulted.push(m);
        }
    }
    let read = read(sources, bindable)?;
    Ok(read
        .into_iter()
        .map(|(mut design, package)| {
            sources.paths.push(PathBuf::from(&design.path));
            sources.origins.push(Origin::Package {
                name: package.name,
                version: package.version,
                root: package.root,
                manifest: package.manifest,
            });
            sources.digests.push(digest(&design.text));
            sources.texts.push(design.text.as_str().into());
            design.source_id = (sources.paths.len() - 1) as u32;
            design
        })
        .collect())
}

/// [`designs`], read and checked but no source of the app's yet (each
/// source id 0): what the source graph folds for an icon's file.
pub(super) fn read(
    sources: &Sources,
    bindable: bool,
) -> Result<Vec<(contract_syntax::DesignSource, Package)>, CompileError> {
    let mut found: Vec<(contract_syntax::DesignSource, Package)> = Vec::new();
    for package in packages(sources).0 {
        let Some(design) = package_design(&package.root, &package.manifest) else {
            continue;
        };
        let path = design.path;
        if !design.named && !path.is_file() {
            continue;
        }
        let inside = path.starts_with(&package.root)
            && (!path.exists()
                || path
                    .canonicalize()
                    .is_ok_and(|real| real.starts_with(&package.root)));
        if !inside {
            return Err(refusal(
                "tamber-unreadable",
                &path,
                format!(
                    "the package `{}`'s design `{}` is outside the package",
                    design.name,
                    path.display()
                ),
            ));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| {
            refusal(
                "tamber-unreadable",
                &path,
                format!(
                    "the package `{}`'s design `{}`: {e}",
                    design.name,
                    path.display()
                ),
            )
        })?;
        let ns = namespace(&design.name);
        if let Some((other, _)) = found
            .iter()
            .find(|(d, _)| d.package.as_deref().map(namespace) == Some(ns))
        {
            return Err(refusal("tamber-package-name", &path, format!(
                "the packages `{}` and `{}` both ship a design under `{ns}.`: a package's tokens are named by the last part of its name, and no app line can tell these apart",
                other.package.as_deref().unwrap_or_default(),
                design.name
            )));
        }
        let rest = path.strip_prefix(&package.root).map_or_else(
            |_| path.display().to_string(),
            |p| p.to_string_lossy().replace('\\', "/"),
        );
        found.push((
            contract_syntax::DesignSource {
                path: path.display().to_string(),
                home: format!("{}/{rest}", design.name),
                text,
                source_id: 0,
                bindable,
                package: Some(design.name),
                root: Some(package.root.display().to_string()),
            },
            package,
        ));
    }
    Ok(found)
}

/// Each package's design as a source of the app's, and what else a build
/// of it reads: where each package without one would have it (making it
/// rebuilds), and each vendored package's `package.json`.
pub(super) fn sources(sources: &Sources) -> (Vec<Source>, Vec<PathBuf>) {
    let (all, manifests) = packages(sources);
    let (mut found, mut missing) = (Vec::new(), manifests);
    for package in all {
        let Some(design) = package_design(&package.root, &package.manifest) else {
            continue;
        };
        if design.path.is_file() {
            found.push(Source {
                path: design.path,
                origin: origin(&package),
            });
        } else {
            missing.push(design.path);
        }
    }
    (found, missing)
}
