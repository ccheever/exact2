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
    target: Option<PathBuf>,
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

    /// Whether what it read reads the same; a moved stamp whose bytes are
    /// the same is taken as the file's now.
    fn fresh(&mut self, dir: &Path) -> bool {
        if self.once {
            return false;
        }
        for f in &mut self.files {
            if f.path.canonicalize().ok() != f.target {
                return false;
            }
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
            let manifest = self.dir.join("app.json");
            let mut paths: Vec<&PathBuf> = [&root, &manifest]
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
