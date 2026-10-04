//! The module loader: resolution, wrapping, and per-module authority.
//!
//! This is where LLP 0067's properties are actually enforced. R1 (nothing
//! capability-bearing on the global object), R2 (each module receives its
//! bindings as parameters of its own scope), and R5 (the global name list is
//! asserted) are properties of *this* file, not of the boundary below it.
//!
//! **v1 is CommonJS-shaped and loads from source.** Not because that is the
//! destination — LLP 0067 R3 requires wrappers compiled ahead of time, and
//! LLP 0057 §1 is a complaint about exactly the per-launch parsing this does —
//! but because it is the smallest loader that makes per-module injection real,
//! and because the boot measurement it enables is what motivates the AOT step.
//! ESM waits on a parser; see LLP 0026.
//!
//! @ref LLP 0058.000.000#6-module-binding-globals-and-bootstrap — module binding, globals, and bootstrap
//! @ref LLP 0067#1-five-properties — R1 and R2: capability-bearing bindings are parameters, never ambient

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
#[cfg(feature = "loader")]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

use crate::grant::GrantSet;

/// What a module is allowed to reach, and what it is called.
#[derive(Debug, Clone)]
pub struct ModuleGrants {
    /// Grants keyed by module specifier, resolved relative to the root.
    per_module: BTreeMap<String, GrantSet>,
    /// Grants keyed by package name. Which files those are is decided by
    /// `bind`, from the install the project actually has — never from a
    /// path's spelling, and never from a package's own `package.json`.
    per_package: BTreeMap<String, GrantSet>,
    /// Canonical directory prefix -> package name, filled by `bind`: the
    /// install `<root>/node_modules/<name>` resolves to, after symlinks.
    bound_packages: BTreeMap<String, String>,
    /// Grants keyed by directory (`./src/`): every module under it, the
    /// longest prefix winning. How first-party trees and workspace packages
    /// are granted.
    per_directory: BTreeMap<String, GrantSet>,
    /// Applied to any module without its own entry.
    default_grants: GrantSet,
}

/// What a manifest section names.
#[derive(Debug, PartialEq, Eq)]
enum Section {
    Default,
    Module(String),
    Directory(String),
    Package(String),
}

impl Section {
    /// `*`, `./file.js`, `./dir/`, or a package name — and nothing else. A
    /// section that is none of these is refused rather than stored under a
    /// key nothing will ever match, because a manifest that silently grants
    /// nothing is the worst kind of wrong.
    fn parse(name: &str) -> Result<Self, String> {
        if name == "*" {
            return Ok(Section::Default);
        }
        if let Some(rest) = name.strip_prefix("./") {
            if rest.is_empty() || rest.split('/').any(|s| s == "..") {
                return Err(format!(
                    "manifest section [{name}] is not a path inside the project"
                ));
            }
            return Ok(if name.ends_with('/') {
                Section::Directory(name.to_string())
            } else {
                Section::Module(name.to_string())
            });
        }
        if is_package_name(name) {
            return Ok(Section::Package(name.to_string()));
        }
        Err(format!(
            "manifest section [{name}] is neither a module (./x.js), a directory (./dir/), \
             a package name (react, @scope/pkg), nor *"
        ))
    }
}

fn is_package_name(name: &str) -> bool {
    let ok_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    let parts: Vec<&str> = name.split('/').collect();
    let unscoped =
        |part: &str| !part.is_empty() && !part.starts_with('.') && part.chars().all(ok_char);
    match parts.as_slice() {
        [bare] => !bare.starts_with('@') && unscoped(bare),
        [scope, bare] => scope.strip_prefix('@').is_some_and(unscoped) && unscoped(bare),
        _ => false,
    }
}

impl ModuleGrants {
    /// Nothing granted to anyone. The correct default: a module that was not
    /// named in the manifest reaches nothing, rather than inheriting whatever
    /// the application happened to hold.
    pub fn none() -> Self {
        Self {
            per_module: BTreeMap::new(),
            per_package: BTreeMap::new(),
            bound_packages: BTreeMap::new(),
            per_directory: BTreeMap::new(),
            default_grants: GrantSet::none(),
        }
    }

    pub fn with_default(mut self, grants: GrantSet) -> Self {
        self.default_grants = grants;
        self
    }

    pub fn with_module(mut self, specifier: &str, grants: GrantSet) -> Self {
        self.per_module.insert(specifier.to_string(), grants);
        self
    }

    pub fn with_package(mut self, name: &str, grants: GrantSet) -> Self {
        self.per_package.insert(name.to_string(), grants);
        self
    }

    pub fn with_directory(mut self, prefix: &str, grants: GrantSet) -> Self {
        self.per_directory.insert(prefix.to_string(), grants);
        self
    }

    /// The grant set for one module: its own section if it has one, else its
    /// package's, else the longest directory prefix naming it, else the
    /// default. Replacement, not union — a module has one grant set, from the
    /// most specific section that names it, so an explicit empty section still
    /// means "nothing".
    ///
    /// A package section applies to the files under the install `bind`
    /// resolved for it and to nothing else. The first version of this matched
    /// any path with a `node_modules/<name>/` segment — so a dependency could
    /// vendor a directory *named* `react` inside itself and hold `[react]`'s
    /// authority. Identity by spelling was the same mistake §4.1 had already
    /// fixed for files, one level up.
    ///
    /// @ref LLP 0065#42-packages-are-granted-by-name-first-party-code-by-path
    pub fn for_module(&self, specifier: &str) -> &GrantSet {
        if let Some(grants) = self.per_module.get(specifier) {
            return grants;
        }
        if let Some(grants) = self
            .bound_packages
            .iter()
            .filter(|(prefix, _)| specifier.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .and_then(|(_, name)| self.per_package.get(name))
        {
            return grants;
        }
        self.per_directory
            .iter()
            .filter(|(prefix, _)| specifier.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, grants)| grants)
            .unwrap_or(&self.default_grants)
    }

    /// The package names the manifest grants.
    pub fn packages(&self) -> impl Iterator<Item = &str> {
        self.per_package.keys().map(String::as_str)
    }

    /// Where each package section was bound, after `bind`: canonical
    /// directory prefix to package name.
    pub fn bound_packages(&self) -> impl Iterator<Item = (&str, &str)> {
        self.bound_packages
            .iter()
            .map(|(dir, name)| (dir.as_str(), name.as_str()))
    }

    /// Bind the manifest to a project: every section is resolved to the file,
    /// directory, or install it names, canonically, before any module loads.
    ///
    /// - A **package** section binds to the directory
    ///   `<root>/node_modules/<name>` resolves to after symlinks — the same
    ///   place for an in-place install, pnpm's store for a pnpm one, the
    ///   project's own tree for a workspace package. Files under that
    ///   directory hold the grant; nothing else does. A copy of the package
    ///   nested under another package is a different install and is granted
    ///   by its directory, if at all.
    /// - **File** and **directory** sections are re-keyed by their canonical
    ///   path, because module identity is canonical (LLP 0065 §4.1) and a
    ///   section spelt `./LOCKED.js` on a case-insensitive filesystem named
    ///   `./locked.js` in every respect but the one that decided its grants.
    ///
    /// Anything a section names that does not exist is refused. That is a
    /// usability rule, not a safety one — a section that matches nothing
    /// grants nothing — kept because a manifest that silently does nothing is
    /// the worst kind of wrong.
    pub fn bind(&mut self, root: &Path) -> Result<(), String> {
        let canonical_root = root
            .canonicalize()
            .map_err(|e| format!("project root {} cannot be resolved: {e}", root.display()))?;
        let relative_to_root = |path: &Path, what: &str| -> Result<String, String> {
            let canonical = path
                .canonicalize()
                .map_err(|_| format!("the manifest names {what}, which does not exist"))?;
            let relative = canonical.strip_prefix(&canonical_root).map_err(|_| {
                format!(
                    "the manifest names {what}, which resolves outside the project root: {}",
                    canonical.display()
                )
            })?;
            Ok(relative.to_string_lossy().replace('\\', "/"))
        };

        let names: Vec<String> = self.per_package.keys().cloned().collect();
        for name in names {
            let installed = canonical_root.join("node_modules").join(&name);
            let relative = relative_to_root(&installed, &format!("package {name:?}")).map_err(|_| {
                format!(
                    "the manifest grants package {name:?}, which is not installed under {}/node_modules. \
                     Install it, or grant a copy nested under another package by its directory",
                    canonical_root.display()
                )
            })?;
            self.bound_packages.insert(format!("./{relative}/"), name);
        }

        let files: Vec<(String, GrantSet)> =
            std::mem::take(&mut self.per_module).into_iter().collect();
        for (key, grants) in files {
            let relative = relative_to_root(
                &canonical_root.join(key.trim_start_matches("./")),
                &format!("module [{key}]"),
            )?;
            self.per_module.insert(format!("./{relative}"), grants);
        }
        let directories: Vec<(String, GrantSet)> = std::mem::take(&mut self.per_directory)
            .into_iter()
            .collect();
        for (key, grants) in directories {
            let relative = relative_to_root(
                &canonical_root.join(key.trim_start_matches("./")),
                &format!("directory [{key}]"),
            )?;
            self.per_directory.insert(format!("./{relative}/"), grants);
        }
        Ok(())
    }

    /// Parse a manifest: sections of grant lines.
    ///
    /// ```text
    /// # applies to every module without its own section
    /// [*]
    /// # (nothing)
    ///
    /// [./net.js]                  # one module
    /// net.fetch https://api.example.com
    ///
    /// [./src/telemetry/]          # every module under a directory
    /// net.fetch https://telemetry.example.com
    ///
    /// [react]                     # every file of a package, every copy
    /// env.read NODE_ENV
    /// [@w/ui]                     # a scoped package, workspace or installed
    /// ```
    ///
    /// A module gets the most specific section naming it — its own, then its
    /// package's, then the longest directory, then `*` — and nothing is
    /// combined. Package sections are checked against the project by `bind`.
    ///
    /// Provisional, and LLP 0062 OQ1 is where that is recorded: whether grants
    /// belong in a manifest, the build graph, or import sites is undecided, and
    /// this is the simplest form that makes the loader real without foreclosing
    /// the others.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut grants = ModuleGrants::none();
        let mut section: Option<Section> = None;
        let mut buffer = String::new();

        let flush = |grants: &mut ModuleGrants,
                     section: &Option<Section>,
                     buffer: &str|
         -> Result<(), String> {
            let Some(section) = section else {
                return Ok(());
            };
            let parsed = GrantSet::parse(buffer)?;
            match section {
                Section::Default => grants.default_grants = parsed,
                Section::Module(name) => {
                    grants.per_module.insert(name.clone(), parsed);
                }
                Section::Directory(prefix) => {
                    grants.per_directory.insert(prefix.clone(), parsed);
                }
                Section::Package(name) => {
                    grants.per_package.insert(name.clone(), parsed);
                }
            }
            Ok(())
        };

        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                flush(&mut grants, &section, &buffer)?;
                buffer.clear();
                section = Some(Section::parse(name.trim())?);
                continue;
            }
            buffer.push_str(line);
            buffer.push('\n');
        }
        flush(&mut grants, &section, &buffer)?;
        Ok(grants)
    }
}

/// The conditions honored when resolving a package's `exports`.
///
/// A policy choice, not a mechanism.
///
/// **This is a set, not a preference order.** Matching walks the *package's*
/// `exports` keys and takes the first one this set contains, so listing
/// `import` before `require` here expresses nothing — a package that writes
/// `require` first gets CommonJS either way. The order is kept only because it
/// reads as intent to a human; `a_packages_own_key_order_decides_between_import_and_require`
/// pins the real behaviour.
///
/// **`node` is deliberately absent.** LLP 0059 §6 deleted Node's server
/// surface, so a package selecting a Node build would get one written against
/// modules that do not exist here. The cost is real and not a fallback: a
/// package exporting *only* a `node` condition does not quietly resolve to
/// `default`, it fails to resolve at all.
///
/// @ref LLP 0065#1-conditions-and-the-one-that-is-missing — why `node` is omitted
pub const CONDITIONS: &[&str] = &["import", "require", "default"];

/// Resolve `specifier` as written inside `from`, against `root`.
///
/// Relative specifiers resolve lexically and are contained. **Containment is
/// enforced**: a resolved path must stay under the root, so
/// `require('../../../../etc/passwd')` fails at resolution rather than becoming
/// an fs.read question. Path reach is a capability concern (LLP 0059.000
/// §3.11), and the loader is not an exception to it.
///
/// Bare specifiers go through `oxc_resolver` — Node resolution is a real
/// algorithm with `exports` maps and condition matching, and the ecosystem
/// already has the answer (LLP 0028). The result is still contained: a package
/// resolving outside the root is refused, which is what makes `node_modules`
/// part of the project rather than a hole in it.
///
/// **Containment is checked against the canonical path**, so a symlink in
/// `node_modules` pointing out of the project is refused like any other escape.
/// A workspace package therefore resolves to its real location and is reachable
/// only when that location is itself inside the root.
/// The containment boundary, and — the part that matters — where it came from.
///
/// Relative specifiers need only a file's neighbours. **Package resolution
/// needs a project**: Node resolution walks *up* looking for `node_modules`,
/// so it is meaningful only against a boundary that someone chose. The entry
/// file's own directory is not that. It is where a file happens to sit, and in
/// a monorepo it is one or two levels below where the packages actually live.
///
/// Rather than guess — every rule for inferring a project root from
/// `package.json` or `node_modules` can be induced to widen it further than
/// the author expected, and a stray `package.json` in a home directory should
/// not make the home directory reachable — an undeclared root simply refuses
/// bare specifiers and says so. Relative resolution is unaffected.
///
/// @ref LLP 0065#5-the-root-must-be-declared — why this is not inferred
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    /// Declared by the author (`--root`). The project is what they said it is,
    /// so packages resolve and containment is enforced against it.
    Declared(PathBuf),
    /// Nothing was declared, so the boundary fell back to the entry's own
    /// directory. Enough to run a self-contained program; not enough to say
    /// where a package would come from.
    EntryDirectory(PathBuf),
}

impl Root {
    pub fn path(&self) -> &Path {
        match self {
            Root::Declared(path) | Root::EntryDirectory(path) => path,
        }
    }

    /// Whether a bare specifier may be resolved against this root.
    pub fn packages_resolvable(&self) -> bool {
        matches!(self, Root::Declared(_))
    }
}

impl std::ops::Deref for Root {
    type Target = Path;
    fn deref(&self) -> &Path {
        self.path()
    }
}

impl AsRef<Path> for Root {
    fn as_ref(&self) -> &Path {
        self.path()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

/// One directory as the filesystem reported it: its real path and its
/// entries by on-disk spelling.
#[derive(Debug)]
struct DirListing {
    real: Option<PathBuf>,
    entries: HashMap<String, EntryKind>,
    /// Lower-cased on-disk name to on-disk name, consulted only when an exact
    /// lookup misses: a case-folding filesystem may hold the file under
    /// another spelling, and one `exists` then settles it.
    folded: HashMap<String, String>,
}

/// What resolution asks the filesystem, remembered for the life of one loader.
///
/// A 500-module graph paid two `realpath(3)` and up to five `stat(2)` calls
/// per module — 26 of its 36 µs — for answers that do not change while it
/// loads. One `readdir` and one `realpath` per directory answer every
/// extension probe in it and give each file its on-disk spelling; a full
/// `canonicalize` is kept for the cases that need it — a symlink entry, or a
/// spelling that differs from the listing's on a case-folding filesystem — so
/// containment (LLP 0065 §4.1) decides exactly what it did. The package
/// resolver is one per loader too, rather than one per call, so its own cache
/// of `package.json` reads is kept.
///
/// A file created after its directory was first listed is not seen until the
/// loader is set again. A module graph does not grow while it loads.
#[derive(Default)]
pub struct ResolveCache {
    canonical_root: Mutex<Option<PathBuf>>,
    dirs: Mutex<HashMap<PathBuf, Arc<DirListing>>>,
    #[cfg(feature = "loader")]
    resolver: OnceLock<oxc_resolver::Resolver>,
}

impl std::fmt::Debug for ResolveCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dirs = self.dirs.lock().map(|d| d.len()).unwrap_or(0);
        f.debug_struct("ResolveCache")
            .field("directories", &dirs)
            .finish()
    }
}

impl ResolveCache {
    fn canonical_root(&self, root: &Path) -> Result<PathBuf, String> {
        let mut slot = self.canonical_root.lock().expect("resolve cache poisoned");
        if let Some(canonical) = slot.as_ref() {
            return Ok(canonical.clone());
        }
        let canonical = root
            .canonicalize()
            .map_err(|e| format!("project root {} cannot be resolved: {e}", root.display()))?;
        *slot = Some(canonical.clone());
        Ok(canonical)
    }

    fn listing(&self, dir: &Path) -> Arc<DirListing> {
        if let Some(listing) = self.dirs.lock().expect("resolve cache poisoned").get(dir) {
            return Arc::clone(listing);
        }
        let mut entries = HashMap::new();
        let mut folded = HashMap::new();
        if let Ok(read) = std::fs::read_dir(dir) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let kind = match entry.file_type() {
                    Ok(kind) if kind.is_symlink() => EntryKind::Symlink,
                    Ok(kind) if kind.is_file() => EntryKind::File,
                    Ok(kind) if kind.is_dir() => EntryKind::Dir,
                    _ => EntryKind::Other,
                };
                folded.insert(name.to_lowercase(), name.clone());
                entries.insert(name, kind);
            }
        }
        let listing = Arc::new(DirListing {
            real: dir.canonicalize().ok(),
            entries,
            folded,
        });
        self.dirs
            .lock()
            .expect("resolve cache poisoned")
            .insert(dir.to_path_buf(), Arc::clone(&listing));
        listing
    }

    /// The canonical path of the regular file `dir/name`, or `None` when there
    /// is no such file.
    fn file_at(&self, dir: &Path, name: &str) -> Option<PathBuf> {
        let listing = self.listing(dir);
        match listing.entries.get(name) {
            Some(EntryKind::File) => listing.real.as_ref().map(|real| real.join(name)),
            // A link is followed all the way, and may lead out of the root —
            // containment decides that, not this.
            Some(EntryKind::Symlink) => {
                let canonical = std::fs::canonicalize(dir.join(name)).ok()?;
                canonical.is_file().then_some(canonical)
            }
            Some(_) => None,
            None => match listing.folded.get(&name.to_lowercase()) {
                Some(on_disk) if on_disk != name && dir.join(name).exists() => {
                    self.file_at(dir, on_disk)
                }
                _ => None,
            },
        }
    }

    fn has_case_variant(&self, dir: &Path, name: &str) -> bool {
        self.listing(dir)
            .folded
            .get(&name.to_lowercase())
            .is_some_and(|on_disk| on_disk != name)
    }

    #[cfg(feature = "loader")]
    fn resolver(&self) -> &oxc_resolver::Resolver {
        self.resolver.get_or_init(|| {
            oxc_resolver::Resolver::new(oxc_resolver::ResolveOptions {
                condition_names: CONDITIONS.iter().map(|c| (*c).to_string()).collect(),
                extensions: EXTENSIONS.iter().map(|e| (*e).to_string()).collect(),
                // `module` before `main`: the ESM entry where a package offers one.
                main_fields: vec!["module".to_string(), "main".to_string()],
                ..oxc_resolver::ResolveOptions::default()
            })
        })
    }
}

/// Reduce a resolved path to the project-relative specifier that identifies it,
/// refusing anything that does not live inside the root.
///
/// **This is the one place a path becomes a module identity**, and both arms of
/// `resolve` go through it, because two things depend on a file having exactly
/// one name:
///
/// - *Containment.* Comparing canonical paths is what makes a symlink out of
///   the project an escape. A lexical check compares the *spelling*, and a
///   spelling can stay inside the root while the bytes come from outside it.
/// - *Authority.* Grants are keyed by specifier (LLP 0067 §2). If one file has
///   two names it has two grant sets, and a module locked down under one name
///   holds the default's authority under the other. That is a capability
///   bypass, not a cosmetic inconsistency.
///
/// Canonicalizing also settles case on a case-insensitive filesystem, so
/// `./LOCKED.js` and `./locked.js` are one module rather than two.
///
/// Fails closed: a path that cannot be canonicalized is refused rather than
/// falling back to its spelling.
///
/// @ref LLP 0065#2-node_modules-is-inside-the-project-not-a-hole-in-it
fn contain(
    cache: &ResolveCache,
    root: &Path,
    path: &Path,
    specifier: &str,
) -> Result<String, String> {
    let canonical_root = cache.canonical_root(root)?;
    let (dir, name) = match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => (dir, name.to_string_lossy().into_owned()),
        _ => return Err(format!("cannot resolve {specifier:?}: not a file path")),
    };
    let canonical = cache.file_at(dir, &name).ok_or_else(|| {
        format!(
            "cannot resolve {specifier:?}: {} is not a file",
            path.display()
        )
    })?;
    let relative = canonical.strip_prefix(&canonical_root).map_err(|_| {
        format!(
            "{specifier:?} resolves outside the project root: {}",
            canonical.display()
        )
    })?;
    Ok(format!(
        "./{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

pub fn resolve(root: &Root, from: &str, specifier: &str) -> Result<String, String> {
    resolve_in(&ResolveCache::default(), root, from, specifier)
}

/// `resolve`, remembering what it asked the filesystem in `cache`. The
/// loader holds one cache for its life; `resolve` above is the same code with
/// a cache that lives for one call.
pub fn resolve_in(
    cache: &ResolveCache,
    root: &Root,
    from: &str,
    specifier: &str,
) -> Result<String, String> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        refuse_schemes(from, specifier)?;
        return resolve_bare(cache, root, from, specifier);
    }

    let from_dir = Path::new(from).parent().unwrap_or(Path::new(""));
    let joined = from_dir.join(specifier);

    // Normalize without touching the filesystem: `..` is resolved lexically so
    // a symlink cannot change what containment means.
    let mut parts: Vec<String> = Vec::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(format!("{specifier:?} escapes the project root"));
                }
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("{specifier:?} must be relative"))
            }
        }
    }

    let relative = parts.join("/");
    // Belt and braces: the lexical walk above already refuses escapes, and this
    // catches anything a future change to it might miss.
    if !root.join(&relative).starts_with(root) {
        return Err(format!("{specifier:?} escapes the project root"));
    }

    // An extension may be omitted, and TypeScript's is not the one on disk:
    // `import './x'` in a TypeScript project means `x.ts`, and `import './x.js'`
    // conventionally means `x.ts` too, because TypeScript makes you write the
    // OUTPUT extension in the source. Both have to resolve or a TypeScript
    // codebase cannot import anything.
    if let Some(resolved) = probe_extensions(cache, root, &relative) {
        // Through `contain`, not straight out: the lexical walk above proves
        // the *spelling* stays inside the root, which says nothing about where
        // a symlink at that spelling points. Returning here directly is how a
        // package could `require('./payload')` and execute bytes from outside
        // the project under an inside name.
        return contain(cache, root, &root.join(&resolved), specifier);
    }

    // On a case-sensitive filesystem a different-case entry must be a
    // refusal, not a successful fallback under a second module identity.
    // Case-folding filesystems took the `contain` arm above and returned the
    // on-disk spelling.
    let unresolved = root.join(&relative);
    if let (Some(dir), Some(name)) = (unresolved.parent(), unresolved.file_name()) {
        let name = name.to_string_lossy();
        if cache.has_case_variant(dir, &name) {
            return Err(format!(
                "cannot resolve {specifier:?}: {} is not a file",
                unresolved.display()
            ));
        }
    }

    // Nothing on disk. Return the specifier as written so the error names what
    // the author asked for rather than the last candidate tried.
    let fallback = if EXTENSIONS.iter().any(|ext| relative.ends_with(ext)) {
        relative
    } else {
        format!("{relative}.js")
    };
    Ok(format!("./{fallback}"))
}

/// Anything with a scheme is not a package name, and the two kinds fail for
/// different reasons — so they say different things. `node:`/`bun:` name
/// builtin namespaces this runtime does not have; a URL names a module to be
/// fetched, which no amount of node_modules searching will find. Reporting
/// either as "not installed" sends the reader looking in the wrong place.
/// Policy, not resolution: a run-only build says the same thing.
fn refuse_schemes(from: &str, specifier: &str) -> Result<(), String> {
    if let Some((scheme, _)) = specifier.split_once(':') {
        return Err(match scheme {
            "node" | "bun" => format!(
                "{specifier:?} names the {scheme:?} builtin namespace, which this runtime does \
                 not provide (LLP 0059 §6)"
            ),
            _ => format!(
                "cannot resolve {specifier:?} from {from}: modules are loaded from the project, \
                 not over {scheme:?}"
            ),
        });
    }
    Ok(())
}

/// Resolve a package specifier through Node resolution, then contain it.
///
/// @ref LLP 0065#2-node_modules-is-inside-the-project-not-a-hole-in-it — containment
/// still applies, and matters more here because resolution walks upward
#[cfg(feature = "loader")]
fn resolve_bare(
    cache: &ResolveCache,
    root: &Root,
    from: &str,
    specifier: &str,
) -> Result<String, String> {
    // A package name is a question about the project, and without a declared
    // root there is no project to ask — only the directory this file happens
    // to sit in. Refuse, and say what would fix it. Guessing here is how a
    // containment boundary silently becomes the home directory.
    if !root.packages_resolvable() {
        return Err(format!(
            "cannot resolve package {specifier:?}: no project root was declared, so there is \
             nowhere to look for it. Pass --root <dir> naming the directory that contains both \
             {from} and node_modules"
        ));
    }

    let from_dir = root.join(
        Path::new(from.trim_start_matches("./"))
            .parent()
            .unwrap_or(Path::new("")),
    );

    let resolved = cache
        .resolver()
        .resolve(&from_dir, specifier)
        .map_err(|e| format!("cannot resolve {specifier:?} from {from}: {e}"))?;

    // `path()`, not `full_path()`: the latter appends `?query` and `#fragment`
    // to the filesystem path, so a package whose exports target is
    // `./index.js?raw` — a real bundler convention — would "resolve" to a name
    // no file has, pass containment on the laundered string, and only fail
    // later at the read.
    //
    // Containment matters more on this arm than the other: Node resolution
    // walks UP the directory tree, so without it a package could resolve to a
    // node_modules outside the project entirely.
    contain(cache, root, resolved.path(), specifier)
}

/// A run-only build carries no package resolver: every edge a program needs
/// was resolved by the build and recorded in the manifest, and a bare
/// specifier the manifest does not have is refused rather than searched for.
#[cfg(not(feature = "loader"))]
fn resolve_bare(
    _cache: &ResolveCache,
    _root: &Root,
    from: &str,
    specifier: &str,
) -> Result<String, String> {
    Err(format!(
        "cannot resolve package {specifier:?} from {from}: this build resolves nothing at run time, \
         and the build manifest does not name this edge"
    ))
}

/// Extensions tried, in order. TypeScript first: in a project that has both,
/// the `.ts` is the source and the `.js` is build output.
/// `.json` is last, as in Node: a `.js` sibling wins for an extensionless
/// specifier. It is here at all because Exact's boot graph imports JSON —
/// `@exact/core`'s colour policy is the first module `main.tsx` reaches that
/// is not JavaScript — and a resolver that cannot find `data.json` behind
/// `require('pkg/data')` reports a missing module for a file that exists.
pub const EXTENSIONS: &[&str] = &[
    ".ts", ".tsx", ".mts", ".js", ".mjs", ".jsx", ".cjs", ".json",
];

/// Find the file a specifier names, allowing for an omitted or rewritten
/// extension.
fn probe_extensions(cache: &ResolveCache, root: &Path, relative: &str) -> Option<String> {
    let exists = |candidate: &str| {
        let path = root.join(candidate);
        let dir = path.parent()?;
        let name = path.file_name()?.to_string_lossy().into_owned();
        cache.file_at(dir, &name).map(|_| candidate.to_string())
    };

    // Exactly as written.
    if let Some(found) = exists(relative) {
        return Some(found);
    }

    // `./x.js` where the source is `./x.ts` — TypeScript's own convention of
    // writing the emitted extension in the import.
    for js in [".js", ".mjs", ".jsx"] {
        if let Some(stem) = relative.strip_suffix(js) {
            for ts in [".ts", ".tsx", ".mts"] {
                if let Some(found) = exists(&format!("{stem}{ts}")) {
                    return Some(found);
                }
            }
        }
    }

    // No extension at all.
    if !EXTENSIONS.iter().any(|ext| relative.ends_with(ext)) {
        for ext in EXTENSIONS {
            if let Some(found) = exists(&format!("{relative}{ext}")) {
                return Some(found);
            }
        }
        // A directory's index file.
        for ext in EXTENSIONS {
            if let Some(found) = exists(&format!("{relative}/index{ext}")) {
                return Some(found);
            }
        }
    }
    None
}

/// The names every module receives, in a fixed order.
///
/// Capability-bearing names are here — as PARAMETERS — and correspondingly not
/// on the global object. That is R1 and R2 in one list.
pub const MODULE_PARAMETERS: &[&str] = &[
    "module",
    "exports",
    "require",
    "fetch",
    "fs",
    "process",
    "__ibex2_meta",
    "sqlite",
];

/// Wrap module source in a function of its injected bindings.
///
/// The result is a function *expression*, evaluated by the host to obtain a
/// callable. `new Function` cannot be used: dynamic code is closed at
/// construction (LLP 0067 §4), which is also why this cannot be done from
/// JavaScript.
///
/// The trailing newline before `})` matters: a module whose last line is a
/// `//` comment would otherwise swallow the closing brace.
pub fn wrap(source: &str) -> String {
    format!(
        "(function ({}) {{\n{}\n}})",
        MODULE_PARAMETERS.join(", "),
        source
    )
}

/// Lower ES module syntax, then wrap.
///
/// One function, because the artifact key is computed over the wrapper text and
/// the wrapper must therefore be the LOWERED form. Two call sites producing the
/// wrapper differently would produce two keys for one module.
#[cfg(feature = "loader")]
pub fn lower_and_wrap(source: &str, specifier: &str) -> Result<String, String> {
    let javascript = to_javascript(source, specifier)?;
    // Named, because the error reaches the author through a dynamic import's
    // rejection or a build failure, and "overlapping module declarations"
    // with no module is a search rather than a diagnosis.
    let lowered = crate::esm::lower(&javascript).map_err(|e| format!("{specifier}: {e}"))?;
    Ok(wrap(&lowered))
}

/// A run-only build has no loader: it runs bytecode built elsewhere and
/// turns no source into anything.
#[cfg(not(feature = "loader"))]
pub fn lower_and_wrap(_source: &str, specifier: &str) -> Result<String, String> {
    Err(format!(
        "{specifier}: this build has no loader; it runs precompiled artifacts only (run `ibex2 build` with a full build)"
    ))
}

/// A module's source as JavaScript, before ESM lowering.
///
/// TypeScript first: the ESM lowering parses JavaScript, and a `.ts` module is
/// not JavaScript. Type stripping deliberately leaves module syntax alone, so
/// `esm::lower` still sees the imports and exports.
///
/// **Public because the build walk must scan this text, not the original.** The
/// JSX transform *injects* `import { jsx } from "react/jsx-runtime"`, which
/// does not appear in the `.tsx` a developer wrote. A dependency scan over the
/// original source cannot see that edge, so the module never gets compiled and
/// `run --precompiled` fails on a graph the build reported as complete.
pub fn to_javascript(source: &str, specifier: &str) -> Result<String, String> {
    if specifier.ends_with(".json") {
        return Ok(json_module(source));
    }
    let typescript = [".ts", ".tsx", ".mts"]
        .iter()
        .any(|ext| specifier.ends_with(ext));
    if typescript {
        #[cfg(feature = "loader")]
        {
            return crate::typescript::strip(source, specifier);
        }
        #[cfg(not(feature = "loader"))]
        {
            return Err(format!(
                "{specifier}: this build has no loader and cannot strip TypeScript"
            ));
        }
    }
    Ok(source.to_string())
}

/// A JSON module: a CommonJS module whose `module.exports` is the parsed
/// value, so `import data from './x.json'` binds it through the default
/// interop and `require('./x.json')` returns it directly.
///
/// Parsed with `JSON.parse` at load rather than pasted in as an object
/// literal, because the two disagree: a literal `{"__proto__": …}` sets the
/// prototype where JSON gives an own property, and JSON's grammar is not a
/// subset of an expression's in every engine. Compiled to bytecode like any
/// other module, so a large JSON file is a string constant in the artifact and
/// costs one parse at load — the same as it would in Node.
///
/// Import attributes (`with { type: 'json' }`) are accepted and not required.
/// Node and the browser require them; Exact's own imports are split between
/// the two forms, and a bundler-targeted codebase does not get to be strict
/// about what its bundler never enforced. Named imports from a JSON module
/// are permitted for the same reason — the lowering destructures
/// `module.exports` — where the specification allows only `default`.
fn json_module(text: &str) -> String {
    format!("module.exports = JSON.parse({});", js_string_literal(text))
}

/// `text` as a double-quoted JavaScript string literal.
///
/// U+2028 and U+2029 are escaped even though ES2019 admits them in literals,
/// because the same text is also a build-time scan target and a bytecode
/// constant, and a line terminator inside a literal is what every tool that
/// splits on lines gets wrong.
fn js_string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The explicit snapshot of names the ordinary runtime adds to the engine.
/// Capability-bearing names are deliberately absent: they arrive as module
/// parameters. Keep this as the one inventory; per-group views partition it.
pub const DEFAULT_ADDED_GLOBALS: &[&str] = &[
    "__ibex2_default",
    "__ibex2_dynamic_import",
    "__ibex2_export_all",
    "__ibex2_fire_timer",
    "console",
    "setTimeout",
    "setInterval",
    "clearTimeout",
    "clearInterval",
    "performance",
    "queueMicrotask",
    "Headers",
    "DOMException",
    "QuotaExceededError",
    "Crypto",
    "CryptoKey",
    "SubtleCrypto",
    "AbortController",
    "AbortSignal",
    "crypto",
    "URL",
    "URLSearchParams",
    "structuredClone",
];

const GLOBAL_PARTITION: &[(Option<crate::bindings::Groups>, &[usize])] = &[
    (None, &[0, 1, 2]),
    (
        Some(crate::bindings::Groups::TIMERS),
        &[3, 5, 6, 7, 8, 9, 10],
    ),
    (Some(crate::bindings::Groups::CONSOLE), &[4]),
    (
        Some(crate::bindings::Groups::PURE),
        &[11, 12, 13, 20, 21, 22],
    ),
    (Some(crate::bindings::Groups::CRYPTO), &[14, 15, 16, 19]),
    (Some(crate::bindings::Groups::ABORT), &[17, 18]),
];

/// The global names a module may see for one installed group set. Anything
/// outside this list on `globalThis` after boot is an R1 violation.
// @ref LLP 0057.000#51-included-gated-or-a-crate — R5 follows the runtime's actual install groups
pub fn allowed_globals(groups: crate::bindings::Groups) -> Vec<&'static str> {
    DEFAULT_ADDED_GLOBALS
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            GLOBAL_PARTITION
                .iter()
                .find(|(_, members)| members.contains(&index))
                .and_then(|(group, _)| {
                    group
                        .is_none_or(|group| groups.contains(group))
                        .then_some(*name)
                })
        })
        .collect()
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
