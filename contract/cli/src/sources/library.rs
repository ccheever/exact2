//! LLP 1139 D0: which components are an app's library's. In each package
//! the app links that is `"private": true` and ships a design, each
//! component its entry (`exports` "." or `index.contract`) declares, and
//! each its entry re-exports: a name a `use` in it brings from another file
//! of the package, through any chain of such files and renames (`use Card
//! as Fancy`), a `use` naming the package itself by a subpath too. Read
//! from the package's files on disk, so a component an app reaches by a
//! subpath, past the entry, is the library's all the same; each file read
//! is a build input. A file of another package nested inside it (vendored,
//! its own `node_modules`) is that package's, never this one's.

use super::{home, packages, Sources};
use crate::resolve::{package_design, package_entry, package_subpath, Origin};
use std::path::{Path, PathBuf};

/// What [`read`] finds.
#[derive(Default)]
pub(super) struct Library {
    /// The identities (`<package>/<file>#<Name>`) of the exported library
    /// components.
    pub components: Vec<String>,
    /// Each file read to find them: a build input.
    pub read: Vec<PathBuf>,
    /// The library packages' names.
    pub packages: Vec<String>,
}

/// The library of the packages `sources` links.
pub(super) fn read(sources: &Sources, app_root: &Path) -> Library {
    let all = packages::packages(sources).0;
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let roots: Vec<PathBuf> = all.iter().map(|p| real(&p.root)).collect();
    let mut out = Library::default();
    for p in &all {
        let Some(json) = std::fs::read_to_string(&p.manifest)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            continue;
        };
        let ships = package_design(&p.root, &p.manifest).is_some_and(|d| d.path.is_file());
        if json["private"] != serde_json::Value::Bool(true) || !ships {
            continue;
        }
        // By its own name, as identities and designs name it, never the
        // one a `use` reached it by (`books` installed as `@acme/reading`).
        let name = json["name"].as_str().unwrap_or(&p.name).to_owned();
        out.packages.push(name.clone());
        let root = real(&p.root);
        let written = p.root.join(package_entry(&json));
        out.read.push(written.clone());
        let entry = real(&written);
        let origin = Origin::Package {
            name: name.clone(),
            version: p.version.clone(),
            root: p.root.clone(),
            manifest: p.manifest.clone(),
        };
        let package = Package {
            name: &name,
            root: &root,
            roots: &roots,
            manifest: &json,
        };
        out.read.push(entry.clone());
        let Some(file) = parse(&entry) else { continue };
        let mut found: Vec<(PathBuf, String)> = file
            .components
            .iter()
            .map(|c| (entry.clone(), c.name.clone()))
            .collect();
        for u in &file.uses {
            for n in &u.names {
                let mut seen = vec![entry.clone()];
                let mut read = Vec::new();
                if let Some(at) = package.declared(&entry, &u.path, &n.name, &mut seen, &mut read) {
                    found.push(at);
                }
                out.read.extend(seen);
                out.read.extend(read);
            }
        }
        // A file with no home is refused as `UNNAMED` where it loads.
        for (path, name) in found {
            if let Some(home) = home(&path, &origin, app_root) {
                out.components.push(format!("{home}#{name}"));
            }
        }
    }
    out.components.sort();
    out.components.dedup();
    out.read.sort();
    out.read.dedup();
    out
}

fn parse(path: &Path) -> Option<contract_syntax::File> {
    contract_syntax::parse(&std::fs::read_to_string(path).ok()?).ok()
}

/// A library package, as its re-exports are followed.
struct Package<'a> {
    name: &'a str,
    root: &'a Path,
    roots: &'a [PathBuf],
    manifest: &'a serde_json::Value,
}

impl Package<'_> {
    /// The file `used` names from `from`, as written and as it really is:
    /// relative, or the package's own name with a subpath, through its
    /// `exports` as the loader maps it (`@acme/lib/card` →
    /// `./src/card.contract`), else the subpath itself; with `.contract`
    /// added where the path has none.
    fn target(&self, from: &Path, used: &str) -> Option<(PathBuf, PathBuf)> {
        let base = match used.strip_prefix(&format!("{}/", self.name)) {
            Some(sub) => match package_subpath(self.manifest, sub) {
                Some(mapped) => self.root.join(mapped.strip_prefix("./").unwrap_or(&mapped)),
                None => self.root.join(sub),
            },
            None if used.starts_with('.') => from.parent()?.join(used),
            None => return None,
        };
        let with = PathBuf::from(format!("{}.contract", base.display()));
        let written = [base, with].into_iter().find(|t| t.is_file())?;
        let real = written.canonicalize().ok()?;
        Some((written, real))
    }

    /// The file and name of the component `name`, used from `used`
    /// (relative to `from`), declares, following that file's own `use`
    /// lines (renames included); `None` outside this package (or in one
    /// nested in it), in a cycle, or for a name no component has. Each file
    /// read is added to `seen`.
    fn declared(
        &self,
        from: &Path,
        used: &str,
        name: &str,
        seen: &mut Vec<PathBuf>,
        read: &mut Vec<PathBuf>,
    ) -> Option<(PathBuf, String)> {
        let (written, target) = self.target(from, used)?;
        // Both: a link retargeted is a changed input.
        read.push(written);
        let owner = self
            .roots
            .iter()
            .filter(|r| target.starts_with(r))
            .max_by_key(|r| r.as_os_str().len())?;
        if owner != self.root || seen.contains(&target) {
            return None;
        }
        seen.push(target.clone());
        let file = parse(&target)?;
        if file.components.iter().any(|c| c.name == name) {
            return Some((target, name.to_string()));
        }
        file.uses.iter().find_map(|u| {
            u.names
                .iter()
                .find(|n| n.alias.as_deref().unwrap_or(&n.name) == name)
                .and_then(|n| self.declared(&target, &u.path, &n.name, seen, read))
        })
    }
}
