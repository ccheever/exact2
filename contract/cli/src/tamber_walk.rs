//! The design server's one walk of the app's Contract sources (LLP 1121
//! D15, stage 8 D): every snapshot lists the app's own `.contract` files
//! from it, and every request's params, synthetic tokens and retention are
//! keyed by it. Walking parses the app (~200 ms on the Bluesky clone), so a
//! walk is kept while what it read reads the same:
//!
//! - each source and each path that resolved its imports (package
//!   manifests, the paths tried), the design files and the root included
//!   (the root even when it does not read), by its stamp (length,
//!   modification and change times, device and inode). A stamp that moved
//!   on the same file is read again and its digest compared (equal, the
//!   stamp is kept); another file at the path, or one come or gone, walks
//!   again, whatever its bytes, since equal bytes elsewhere resolve
//!   otherwise;
//! - what decides which of those files are a package's ([`owned`]): each
//!   `node_modules` entry (an `@scope`'s too) and where it leads, and each
//!   `package.json` above a source inside the app, there or not;
//! - a walk that did not load whole, or a file modified once it began, is
//!   used once and walked again.
//!
//! It is checked once per request.
//!
//! [`owned`]: crate::tamber_contract::owned

use super::{digest, identity, Server};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A file's length, modification and change times, and device and inode.
type Stamp = (u64, SystemTime, (i64, i64), (u64, u64));

fn stamp(path: &Path) -> Option<Stamp> {
    let m = std::fs::metadata(path).ok()?;
    #[cfg(unix)]
    let changed = {
        use std::os::unix::fs::MetadataExt;
        (m.ctime(), m.ctime_nsec())
    };
    #[cfg(not(unix))]
    let changed = (0, 0);
    Some((m.len(), m.modified().ok()?, changed, identity(&m)))
}

fn read(path: &Path) -> String {
    std::fs::read(path).map(|b| digest(&b)).unwrap_or_default()
}

/// A file a walk read: its path, its stamp then, its digest, and whether
/// it is a design file (which the synthetic tokens' key leaves out).
struct Read {
    path: PathBuf,
    stamp: Option<Stamp>,
    digest: String,
    design: bool,
}

/// The app's Contract sources as one walk found them.
pub(super) struct Walk {
    files: Vec<Read>,
    /// Each `node_modules` entry (an `@scope`'s entries too), and where it
    /// leads.
    modules: Vec<(PathBuf, Option<PathBuf>)>,
    /// Each `package.json` above a source inside the app: its digest, or
    /// none when there is none.
    manifests: Vec<(PathBuf, Option<String>)>,
    /// The Contract files and their digests, as one string (design files
    /// aside).
    pub(super) key: String,
    /// The design files and their digests.
    pub(super) design: String,
    /// The app's own `.contract` files, and whether the graph loaded
    /// whole ([`crate::tamber_contract::owned`]).
    pub(super) owned: (Vec<String>, bool),
    /// Not to be kept: it did not load whole, or a file moved as it walked.
    once: bool,
    /// The request it was last checked for.
    checked: u64,
}

impl Walk {
    fn modules(dir: &Path) -> Vec<(PathBuf, Option<PathBuf>)> {
        let entries = |d: &Path| -> Vec<PathBuf> {
            std::fs::read_dir(d)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect()
        };
        let mut out = Vec::new();
        for path in entries(&dir.join("node_modules")) {
            let scope = path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('@'));
            let all = if scope { entries(&path) } else { vec![path] };
            out.extend(all.into_iter().map(|p| {
                let target = p.canonicalize().ok();
                (p, target)
            }));
        }
        out.sort();
        out
    }

    fn manifests(dir: &Path, files: &[Read]) -> Vec<(PathBuf, Option<String>)> {
        let app = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let mut out: Vec<(PathBuf, Option<String>)> = Vec::new();
        for f in files {
            for d in f.path.ancestors().skip(1) {
                if !d.starts_with(&app) || d == app {
                    break;
                }
                let manifest = d.join("package.json");
                if !out.iter().any(|(p, _)| *p == manifest) {
                    let d = manifest.is_file().then(|| read(&manifest));
                    out.push((manifest, d));
                }
            }
        }
        out
    }

    /// Whether what it read reads the same; a moved stamp whose bytes are
    /// the same is taken as the file's now.
    fn fresh(&mut self, dir: &Path) -> bool {
        if self.once {
            return false;
        }
        for f in &mut self.files {
            let now = stamp(&f.path);
            if now == f.stamp {
                continue;
            }
            let same_file = matches!((&now, &f.stamp), (Some(a), Some(b)) if a.3 == b.3);
            if !same_file || read(&f.path) != f.digest {
                return false;
            }
            f.stamp = now;
        }
        let manifests = self
            .manifests
            .iter()
            .all(|(p, d)| *d == p.is_file().then(|| read(p)));
        manifests && self.modules == Walk::modules(dir)
    }
}

impl Server {
    /// The app's Contract sources ([`Walk`]), walked again only when what
    /// the last walk read reads otherwise; checked once per request.
    pub(super) fn walk(&self) -> std::cell::Ref<'_, Walk> {
        let asked = self.asked.get();
        let fresh = match self.graph.borrow_mut().as_mut() {
            Some(w) if w.checked == asked && asked != 0 => true,
            Some(w) => {
                let fresh = w.fresh(&self.dir);
                w.checked = asked;
                fresh
            }
            None => false,
        };
        if !fresh {
            let root = self.dir.join("app.contract");
            let began = SystemTime::now();
            let graph = contract::source_graph(&root);
            let mut paths: Vec<&PathBuf> = std::iter::once(&root)
                .chain(graph.sources.iter().map(|s| &s.path))
                .chain(&graph.consulted)
                .collect();
            paths.sort();
            paths.dedup();
            let files: Vec<Read> = paths
                .into_iter()
                .map(|p| Read {
                    path: p.clone(),
                    stamp: stamp(p),
                    digest: read(p),
                    design: p.extension().is_some_and(|e| e == "tamber"),
                })
                .collect();
            let (mut key, mut design) = (String::new(), String::new());
            for f in &files {
                let into = if f.design { &mut design } else { &mut key };
                into.push_str(&f.path.to_string_lossy());
                into.push('\0');
                into.push_str(&f.digest);
                into.push('\n');
            }
            let moved = files.iter().any(|f| f.stamp.is_some_and(|s| s.1 >= began));
            *self.graph.borrow_mut() = Some(Walk {
                modules: Walk::modules(&self.dir),
                manifests: Walk::manifests(&self.dir, &files),
                files,
                key,
                design,
                owned: crate::tamber_contract::owned(&self.dir, &graph),
                once: !graph.errors.is_empty() || moved,
                checked: asked,
            });
        }
        std::cell::Ref::map(self.graph.borrow(), |w| w.as_ref().expect("walked above"))
    }

    /// The key of the app's Contract sources ([`Walk::key`]), and whether
    /// code beside the app can name a token outside its views (D7's
    /// retention: app.ts, a data crate, app.json's modules or hatches),
    /// which none of the sources say.
    pub(super) fn sources_key(&self) -> String {
        let root = self.dir.join("app.contract");
        format!(
            "{}bindable {}\n",
            self.walk().key,
            contract::code_beside(&root)
        )
    }

    /// The app's components' params (D15), loaded again only when its
    /// sources' key or its design files move (a native component's params
    /// are declared in the design).
    pub(super) fn app_params(&self) -> std::rc::Rc<contract::tamber_params::AppParams> {
        let key = format!("{}{}", self.sources_key(), self.walk().design);
        let mut params = self.params.borrow_mut();
        if params.0 != key {
            let root = self.dir.join("app.contract");
            *params = (
                key,
                std::rc::Rc::new(contract::tamber_params::params_of(&root)),
            );
        }
        params.1.clone()
    }
}
