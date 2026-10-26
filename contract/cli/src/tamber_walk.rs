//! The design server's one walk of the app's Contract sources (LLP 1121
//! D15, stage 8 D): every snapshot lists the app's own `.contract` files
//! from it, and every request's params, synthetic tokens and retention are
//! keyed by it. Walking parses the app (~200 ms on the Bluesky clone), so a
//! walk is kept while what it read reads the same:
//!
//! - each source and each path that resolved its imports (package
//!   manifests, the paths tried), the design files, the root and `app.json`
//!   (which picks the design) included, there or not, by its stamp
//!   (length, modification and change times, device and inode) and where
//!   it leads. A stamp that moved on the same file is read again and its
//!   digest compared (equal, the stamp is kept); another file at the path,
//!   one come or gone, or the path leading elsewhere walks again, whatever
//!   its bytes, since equal bytes elsewhere resolve otherwise;
//! - what decides which of those files are a package's and the identity
//!   its params are named by ([`owned`], `sources::home`): each
//!   `node_modules` entry (an `@scope`'s too) and where it leads, and each
//!   `package.json` a source's identity looks for, there or not; their
//!   digests are in the key;
//! - a walk that did not load whole, or with a file changed once it began
//!   (its change time, which no copy keeps), is used once and walked again.
//!
//! It is checked once per request. A change to design files alone (the
//! app's design, a package's: `.tamber`), whose Contract sources read the
//! same, keeps the walk's Contract half ([`contract::GraphBase`]) and
//! folds the rest of the graph again from the designs as they are now (the
//! icon SVGs they name): no Contract file is parsed again (the third hands-on
//! pass's latency: a value edit's three parses of the clone took ~500 ms).
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

/// A line of what the walk and the params did (`EXACT_TAMBER_WALK_LOG`, a
/// test's: `walk full`, `walk designs`, `params loaded`, `params kept`).
fn said(line: &str) {
    if let Some(log) = std::env::var_os("EXACT_TAMBER_WALK_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Whether what a design-only change kept is checked against a cold walk
/// and load (`EXACT_TAMBER_WALK_VERIFY`, a test's: theme2's condition (c)).
fn verifying() -> bool {
    std::env::var_os("EXACT_TAMBER_WALK_VERIFY").is_some()
}

/// `path`'s digest as a walk reads it (none, empty, for a file not there):
/// what the background whole build checks its inputs by.
pub(super) fn digest_of(path: &Path) -> String {
    read(path)
}

/// [`Walk::facts`] read again from disk: the `package.json`s at `manifests`
/// and `dir`'s `node_modules` entries.
pub(super) fn facts(dir: &Path, manifests: &[PathBuf]) -> String {
    let digests: Vec<String> = manifests.iter().map(|p| read(p)).collect();
    facts_text(manifests, &digests, &Walk::modules(dir))
}

fn facts_text(
    manifests: &[PathBuf],
    digests: &[String],
    modules: &[(PathBuf, Option<PathBuf>)],
) -> String {
    let mut out = String::new();
    for (p, d) in manifests.iter().zip(digests) {
        out.push_str(&format!("manifest\t{}\t{d}\n", p.display()));
    }
    for (p, t) in modules {
        out.push_str(&format!("module\t{}\t{t:?}\n", p.display()));
    }
    out
}

/// A file a walk read: its path, its stamp then, its digest, and whether
/// it is a design file (which the synthetic tokens' key leaves out).
struct Read {
    path: PathBuf,
    stamp: Option<Stamp>,
    target: Option<PathBuf>,
    digest: String,
    design: bool,
}

/// What a walk's files read now ([`Walk::fresh`]).
#[derive(Debug, PartialEq)]
enum Fresh {
    Same,
    /// Only design files moved: the graph is folded again from the walk's
    /// Contract half.
    Designs,
    Stale,
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
    /// The graph it read, and its Contract half to fold a design-only change
    /// from (none when the root could not be read).
    pub(super) graph: contract::SourceGraph,
    base: Option<contract::GraphBase>,
    /// Not to be kept: it did not load whole, or a file moved as it walked.
    once: bool,
    /// The request it was last checked for.
    checked: u64,
}

impl Walk {
    /// What the app's params read of the design files: each package's design file and its digest, and the facts the
    /// load reads from the app's own design (all of it, when the root could not be read).
    pub(super) fn params_design(&self, dir: &Path) -> String {
        let Some(base) = &self.base else {
            return self.design.clone();
        };
        // (By the walk's own paths, which are the root's canonical directory's.)
        let root = dir.join("app.contract");
        let app = contract::design_path(&root.canonicalize().unwrap_or(root));
        let mut out = String::new();
        // (The app's own design is in the facts; one that is also a package's design is whole in the params too.)
        let package = |p: &PathBuf| self.graph.sources.iter().filter(|s| s.path == *p).count() > 1;
        for f in self
            .files
            .iter()
            .filter(|f| f.design && (Some(&f.path) != app.as_ref() || package(&f.path)))
        {
            out.push_str(&f.path.to_string_lossy());
            out.push('\0');
            out.push_str(&f.digest);
            out.push('\n');
        }
        out.push_str(&base.design_facts());
        out
    }

    /// Each file it read, with its digest then: a whole build's inputs.
    pub(super) fn inputs(&self) -> Vec<(PathBuf, String)> {
        self.files
            .iter()
            .map(|f| (f.path.clone(), f.digest.clone()))
            .collect()
    }

    /// What else it read that a whole build's loading reads: each
    /// `package.json` above a source (its digest, empty when none) and each
    /// `node_modules` entry with where it leads, as [`facts`] reads them
    /// again.
    pub(super) fn facts(&self) -> (Vec<PathBuf>, String) {
        let paths: Vec<PathBuf> = self.manifests.iter().map(|(p, _)| p.clone()).collect();
        let digests: Vec<String> = self
            .manifests
            .iter()
            .map(|(_, d)| d.clone().unwrap_or_default())
            .collect();
        (paths.clone(), facts_text(&paths, &digests, &self.modules))
    }

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

    /// The `package.json` files a source's identity looks for, as
    /// `sources::home` does: each directory above it short of the app's
    /// directory and those above that.
    fn manifests(dir: &Path, files: &[Read]) -> Vec<(PathBuf, Option<String>)> {
        let app = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let mut out: Vec<(PathBuf, Option<String>)> = Vec::new();
        for f in files.iter().filter(|f| !f.design) {
            let path = f.target.as_deref().unwrap_or(&f.path);
            for d in path.ancestors().skip(1) {
                if app.starts_with(d) {
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

    /// Whether what it read reads the same ([`Fresh::Same`]), or only its
    /// design files read otherwise (one edited, come or gone: [`Fresh::Designs`]);
    /// a moved stamp whose bytes are the same is taken as the file's now.
    fn fresh(&mut self, dir: &Path) -> Fresh {
        if self.once {
            return Fresh::Stale;
        }
        let mut designs = false;
        for f in &mut self.files {
            if f.path.canonicalize().ok() != f.target {
                // (A design file come or gone: its graph is the designs' to fold again.)
                if f.design && (f.target.is_none() || !f.path.exists()) {
                    designs = true;
                    continue;
                }
                return Fresh::Stale;
            }
            let now = stamp(&f.path);
            if now == f.stamp {
                continue;
            }
            let same_file = matches!((&now, &f.stamp), (Some(a), Some(b)) if a.3 == b.3);
            if same_file && read(&f.path) == f.digest {
                f.stamp = now;
                continue;
            }
            if !f.design {
                return Fresh::Stale;
            }
            designs = true;
        }
        let manifests = self
            .manifests
            .iter()
            .all(|(p, d)| *d == p.is_file().then(|| read(p)));
        if !manifests || self.modules != Walk::modules(dir) {
            return Fresh::Stale;
        }
        match designs && self.base.is_some() {
            true => Fresh::Designs,
            false if designs => Fresh::Stale,
            false => Fresh::Same,
        }
    }
}

impl Server {
    /// Check the walk again within this request (a write found a file
    /// moved since its snapshot).
    pub(super) fn recheck(&self) {
        if let Some(w) = self.graph.borrow_mut().as_mut() {
            w.checked = 0;
        }
    }

    /// The app's Contract sources ([`Walk`]), walked again only when what
    /// the last walk read reads otherwise; checked once per request.
    pub(super) fn walk(&self) -> std::cell::Ref<'_, Walk> {
        let asked = self.asked.get();
        // Before anything is checked: a file that moves from here on, its change time this or later, is one the walk
        // may have read as it was before (whether it keeps its Contract half or walks again).
        let began = SystemTime::now();
        let fresh = match self.graph.borrow_mut().as_mut() {
            Some(w) if w.checked == asked && asked != 0 => Fresh::Same,
            Some(w) => {
                let fresh = w.fresh(&self.dir);
                w.checked = asked;
                fresh
            }
            None => Fresh::Stale,
        };
        if fresh != Fresh::Same {
            let root = self.dir.join("app.contract");
            // Design files alone moved: the kept Contract half, the rest folded again from the designs now.
            let kept = match fresh {
                Fresh::Designs => self.graph.borrow_mut().take().and_then(|w| w.base),
                _ => None,
            };
            said(if kept.is_some() {
                "walk designs"
            } else {
                "walk full"
            });
            let folded = kept.is_some();
            let (graph, base) = match kept
                .map(Ok)
                .unwrap_or_else(|| contract::source_graph_base(&root))
            {
                Ok(base) => (base.tail(), Some(base)),
                Err(refused) => (refused, None),
            };
            // (A test's: the graph a design-only change folded is a cold walk's, byte for byte.)
            if folded && verifying() {
                let cold = format!("{:?}", contract::source_graph(&root));
                assert_eq!(
                    format!("{graph:?}"),
                    cold,
                    "a design-only change's graph is a cold walk's"
                );
            }
            let manifest = self.dir.join("app.json");
            // The default design's path too, there or not (`app.json` naming none): a design made there again is
            // a design file come, which the walk sees.
            let default = root
                .canonicalize()
                .ok()
                .and_then(|r| r.parent().map(|d| d.join("app.tamber")))
                .unwrap_or_else(|| self.dir.join("app.tamber"));
            let mut paths: Vec<&PathBuf> = [&root, &manifest, &default]
                .into_iter()
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
                    target: p.canonicalize().ok(),
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
            let manifests = Walk::manifests(&self.dir, &files);
            for (path, d) in &manifests {
                key.push_str(&path.to_string_lossy());
                key.push('\0');
                key.push_str(d.as_deref().unwrap_or(""));
                key.push('\n');
            }
            // A change time is the kernel's own clock, which no copy or
            // rename keeps: one since the walk began is a file it may have
            // read as it was before.
            let began = began
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let since = |(s, n): (i64, i64)| {
                (s, n) >= (began.as_secs() as i64, i64::from(began.subsec_nanos()))
            };
            let moved = files.iter().any(|f| f.stamp.is_some_and(|s| since(s.2)));
            *self.graph.borrow_mut() = Some(Walk {
                modules: Walk::modules(&self.dir),
                manifests,
                files,
                key,
                design,
                owned: crate::tamber_contract::owned(&self.dir, &graph),
                once: !graph.errors.is_empty() || moved,
                graph,
                base,
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
    /// sources' key moves, a package's design file (whole in them), or what
    /// the load reads from the app's design ([`contract::GraphBase::design_facts`]:
    /// a native component's params, a motion token), never for a value no
    /// param reads (the third hands-on pass's latency).
    pub(super) fn app_params(&self) -> std::rc::Rc<contract::tamber_params::AppParams> {
        let key = format!(
            "{}{}",
            self.sources_key(),
            self.walk().params_design(&self.dir)
        );
        let mut params = self.params.borrow_mut();
        let root = self.dir.join("app.contract");
        if params.0 != key {
            said("params loaded");
            *params = (
                key,
                std::rc::Rc::new(contract::tamber_params::params_of(&root)),
            );
        } else {
            said("params kept");
            // (A test's: the params kept are what loading them again gives, byte for byte.)
            if verifying() {
                let cold = format!("{:?}", contract::tamber_params::params_of(&root));
                assert_eq!(
                    format!("{:?}", params.1),
                    cold,
                    "the params a key kept are a cold load's"
                );
            }
        }
        params.1.clone()
    }

    /// The exported library components' slot chains (LLP 1139 D4), from
    /// the params' IR `library` block, built again only when the params
    /// are (each chain's tokens read at check time, closed over references
    /// by the checker itself, so no design fold here).
    /// None when no component is the library's.
    pub(super) fn slot_chains(
        &self,
    ) -> Option<std::rc::Rc<contract_lower::anatomy::slot::SlotChains>> {
        let app = self.app_params();
        let mut slots = self.slots.borrow_mut();
        if !matches!(&*slots, Some((of, _)) if std::rc::Rc::ptr_eq(of, &app)) {
            let mut library = contract::tamber_params::library(&app, None);
            // Indexed by the names each row's own value says (a bare name, or
            // each a `calc(…)` names: a name as the lexer reads one, never a
            // function's before its `(` or a word in quotes): what those
            // reach is the candidate fold's to say (check_slots closes over
            // it), never the packages' defaults an app line may have replaced.
            for node in library
                .as_array_mut()
                .into_iter()
                .flatten()
                .flat_map(|e| e["chain"].as_array_mut().into_iter().flatten())
            {
                let mut direct: Vec<serde_json::Value> = Vec::new();
                for value in node["rows"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| r.get(1).and_then(|v| v.as_str()))
                {
                    for name in row_names(value) {
                        let name = serde_json::Value::from(name);
                        if !direct.contains(&name) {
                            direct.push(name);
                        }
                    }
                }
                node["tokens"] = serde_json::Value::Array(direct);
            }
            let any = library.as_array().is_some_and(|all| {
                all.iter()
                    .any(|e| e["library"] == serde_json::Value::Bool(true))
            });
            let chains = any.then(|| {
                std::rc::Rc::new(contract_lower::anatomy::slot::SlotChains::from_ir(&library))
            });
            *slots = Some((app, chains));
        }
        slots.as_ref().and_then(|(_, c)| c.clone())
    }
}

/// What of the designs (`texts`: the app's and each package's) the plan's
/// retention and synthetic tokens can depend on, parsed, never compiled
/// (stage 12 B1, theme2's sign-off): each token's and icon's name, written
/// kind, and the names its value and `when` values reference (whole aliases
/// and words naming a token, an icon or a dial of any of them), with its
/// `when` conditions; the `params` entries, name to value; each component's
/// params with their defaults; the axes, dials and `exports`. A value that
/// names nothing (a hex, a length) is left out, so editing it keeps the
/// server's cached retention. A design that does not parse is keyed whole.
pub(super) fn retention_key(texts: &[&str]) -> String {
    let parsed: Vec<Option<exact_tamber::Design>> =
        texts.iter().map(|t| exact_tamber::parse(t).ok()).collect();
    // A name as the checker resolves one: a role's spelling in any case and
    // with or without `-exact-` (`CANVAS` is `Canvas`), so a reference is
    // found by its folded form (re-check: a role alias moved by case alone
    // kept the key) and keyed as written (re-check 2: `Foo` and `foo`).
    let fold = |name: &str| {
        let lower = name.to_ascii_lowercase();
        lower
            .strip_prefix("-exact-")
            .map_or(lower.clone(), str::to_string)
    };
    let names: std::collections::HashSet<String> = parsed
        .iter()
        .flatten()
        .flat_map(|d| {
            d.tokens
                .iter()
                .chain(&d.icons)
                .map(|t| fold(&t.name.value))
                .chain(d.dials.iter().map(|x| fold(&x.name.value)))
        })
        .collect();
    let refs = |text: &str| -> String {
        // Found folded, keyed as written: `Foo` and `foo` are two tokens, so
        // a reference moved between them changes the key (re-check 2).
        let words = text
            .split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
            .filter(|w| names.contains(&fold(w)));
        words.collect::<Vec<_>>().join(" ")
    };
    // A value's rough kind, as an unwritten kind is inferred from it: a
    // value edit that turns a colour into a length moves which rows may read
    // the token, so it keys too.
    let class = |text: &str| {
        let t = text.trim();
        let digits = t
            .trim_start_matches(['-', '+'])
            .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
        let colour = [
            "#",
            "rgb",
            "hsl",
            "oklch",
            "oklab",
            "lab(",
            "lch(",
            "color",
            "light-dark",
        ];
        if colour.iter().any(|p| t.starts_with(p)) {
            "colour"
        } else if !digits.is_empty() && digits.parse::<f64>().is_ok() {
            if t.len() == digits.len() {
                "number"
            } else {
                "length"
            }
        } else {
            "other"
        }
    };
    // A value as the key holds it (theme2's pre-approved fallback, after the
    // fourth re-check found generated `param.…` names and a header the
    // reference scan missed): a literal (one that names nothing: a hex, a
    // length, `light-dark(#fff, #000)`) by its rough kind only, so editing
    // it keeps the cache; anything else verbatim, so every name it could
    // reach keys, however it is spelled.
    // A literal names nothing: no word in it is a name of the designs (a
    // token, an icon, a dial, as folded) and none is dotted (a package's
    // token, or a component's generated `param.…` name, which the designs
    // don't declare). `light-dark(#fff, #000)` and `rgba(…)` are literals.
    let literal = |text: &str| {
        refs(text).is_empty()
            && !text
                .split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
                .any(|w| w.contains('.') && w.trim_matches('.').parse::<f64>().is_err())
    };
    let value = |text: &str| {
        if literal(text) {
            class(text).to_string()
        } else {
            format!("={:?} [{}]", text.trim(), refs(text))
        }
    };
    let mut key = String::new();
    for (text, design) in texts.iter().zip(&parsed) {
        let Some(d) = design else {
            key.push_str(&format!("whole {}\n", digest(text.as_bytes())));
            continue;
        };
        // Its name: a package design's header decides whether its tokens
        // join (fourth re-check).
        key.push_str(&format!("design {}\n", d.name));
        // Each row says its section: a name moved from `icons` to `theme`
        // changes what the views may read (re-check).
        let sections = d
            .tokens
            .iter()
            .map(|t| ("token", t))
            .chain(d.icons.iter().map(|t| ("icon", t)));
        for (section, t) in sections {
            let (v, n) = (&t.value.value, &t.name.value);
            key.push_str(&format!("{section} {n:?} {:?} {}", t.kind, value(v)));
            for w in &t.whens {
                key.push_str(&format!(" when {:?} {}", w.conds, value(&w.value.value)));
            }
            // Its metadata, verbatim and quoted (`pair=` another token; a
            // quoted value can't spell another row: fifth re-check).
            for (k, v) in &t.meta {
                key.push_str(&format!(" {k:?}={v:?}"));
            }
            key.push('\n');
        }
        for e in &d.params {
            key.push_str(&format!("param {:?} = {:?}", e.name.value, e.value.value));
            for w in &e.whens {
                key.push_str(&format!(" when {:?} {:?}", w.conds, w.value.value));
            }
            key.push('\n');
        }
        for c in &d.components {
            key.push_str(&format!("component {:?}", c.name.value));
            for p in &c.params {
                key.push_str(&format!(
                    " {:?}:{:?}={:?} {:?}",
                    p.name.value, p.kind, p.value.value, p.range
                ));
            }
            key.push('\n');
        }
        let axes: Vec<_> = d.axes.iter().map(|a| (&a.name.value, &a.values)).collect();
        let dials: Vec<_> = d.dials.iter().map(|x| &x.name.value).collect();
        let exports: Option<Vec<_>> = d
            .exports
            .as_ref()
            .map(|e| e.iter().map(|x| &x.value).collect());
        key.push_str(&format!(
            "axes {axes:?} dials {dials:?} exports {exports:?}\n"
        ));
    }
    key
}

#[cfg(test)]
mod tests {
    use super::retention_key;

    /// The key's fourth re-check (Astra), answered by theme2's verbatim key:
    /// a value that names anything keys as written (a component's generated
    /// `param.…` name the scan can't know included); a pure literal's edit
    /// keeps the key; a design's header keys (a package's repaired header
    /// joins its tokens).
    #[test]
    fn a_non_literal_value_keys_verbatim_a_literal_does_not_and_the_header_keys() {
        let design = |pick: &str, ink: &str, name: &str| {
            format!("tamber {name}\n\ntheme\n  ink = {ink}\n  pick = {pick}\n")
        };
        let key = |d: String| retention_key(&[&d]);
        let base = key(design("param.f0.Card.a", "#111111", "t"));
        assert_ne!(
            base,
            key(design("param.f0.Card.b", "#111111", "t")),
            "a generated param reference moved"
        );
        assert_eq!(
            base,
            key(design("param.f0.Card.a", "#121212", "t")),
            "a hex edit"
        );
        assert_eq!(
            key(design("x", "8px", "t")),
            key(design("x", "10px", "t")),
            "a length edit"
        );
        assert_ne!(
            key(design("x", "8px", "t")),
            key(design("x", "#111111", "t")),
            "a length became a colour"
        );
        assert_ne!(
            base,
            key(design("param.f0.Card.a", "#111111", "wrong")),
            "the header"
        );
    }

    /// A literal colour expression keeps the key; a dotted name the designs
    /// don't declare keys; and (fifth re-check) a metadata value can't spell
    /// another row, being quoted.
    #[test]
    fn literals_dotted_names_and_metadata() {
        let design =
            |pick: &str, ink: &str| format!("tamber t\n\ntheme\n  ink = {ink}\n  pick = {pick}\n");
        let key = |d: String| retention_key(&[&d]);
        assert_eq!(
            key(design("x", "light-dark(#111111, #eeeeee)")),
            key(design("x", "light-dark(#121212, #eeeeee)")),
            "a colour expression naming nothing"
        );
        assert_ne!(
            key(design("a.b", "#111111")),
            key(design("a.c", "#111111")),
            "a dotted name"
        );
        let real =
            "tamber t\n\ntheme\n  anchor = #111111  label=\"x\"\n  Canvas = old\n  old = #222222\n";
        let fake = "tamber t\n\ntheme\n  anchor = #111111  label=\"x\\ntoken Canvas None =old [old]\"\n  old = #222222\n";
        assert!(
            exact_tamber::parse(fake).is_ok(),
            "the impersonating design parses"
        );
        assert_ne!(
            retention_key(&[real]),
            retention_key(&[fake]),
            "metadata spelling a row"
        );
    }
}

/// The token names a row's value says: each run of identifiers joined by
/// `.` (an identifier a letter or `_` first, then letters, digits, `_` and
/// `-`, a trailing `-` its own, as the lexer reads one), but a function's
/// name (its `(` next) and anything in quotes.
fn row_names(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let (mut out, mut i, mut quote) = (Vec::new(), 0, None);
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            quote = (c != q).then_some(q);
            i += 1;
            continue;
        }
        if c == b'"' || c == b'\'' {
            quote = Some(c);
            i += 1;
            continue;
        }
        let starts = (c.is_ascii_alphabetic() || c == b'_')
            && (i == 0
                || !(bytes[i - 1].is_ascii_alphanumeric()
                    || matches!(bytes[i - 1], b'_' | b'-' | b'.')));
        if !starts {
            i += 1;
            continue;
        }
        let from = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'-' | b'.'))
        {
            i += 1;
        }
        let name = value[from..i].trim_end_matches('.');
        if bytes.get(i) != Some(&b'(') && !name.is_empty() {
            out.push(name);
        }
    }
    out
}

#[cfg(test)]
mod row_names_tests {
    #[test]
    fn a_rows_names_are_its_references_only() {
        assert_eq!(super::row_names("lib.w"), ["lib.w"]);
        assert_eq!(
            super::row_names("calc(_lib.w * 2 + cards.pad-x)"),
            ["_lib.w", "cards.pad-x"]
        );
        assert_eq!(
            super::row_names("calc(2px + max(a.b, 1px)) 'quoted.name' \"x.y\""),
            ["a.b"]
        );
        assert_eq!(super::row_names("12px"), Vec::<&str>::new());
        assert_eq!(super::row_names("calc(lib.w- * 2)"), ["lib.w-"]);
    }
}
