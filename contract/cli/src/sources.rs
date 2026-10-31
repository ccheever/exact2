//! File loading and source identity shared by Contract compilation and navigation.
//! @ref LLP 1017.000 P8; LLP 1035.005 D2/D3; LLP 1091 (module scope).

mod home;
mod library;
mod load;
mod members;
pub(crate) use members::standalone_members;
pub(crate) mod packages;
mod refused;
mod tail;
mod uses;
pub use tail::GraphBase;

pub(crate) use refused::Refused;

use crate::resolve::{resolve, Origin};
use crate::CompileError;
use contract_syntax::scope::{rescope, Elsewhere, Kind, Scope};
use contract_syntax::{File, NameSpans, Span, UseDecl, UseName};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

pub(crate) struct Sources {
    paths: Vec<PathBuf>,
    /// Where each source came from, by source id (LLP 1091 D10).
    pub(crate) origins: Vec<Origin>,
    /// Every name a `use` brought, resolved, for navigation.
    pub(crate) imports: Vec<Import>,
    /// Every package a `use` reached, by each name it was reached by (one
    /// directory installed under two names is two), in order.
    pub(crate) packages: Vec<Package>,
    /// Every `package.json` a resolution read, refused or not.
    pub(crate) consulted: Vec<PathBuf>,
    /// Each source's text as compiled, its SHA-256 (hex), by source id: the
    /// source map says them, so a reader knows a file has not moved since
    /// (LLP 1122 D5, D6's fail-closed rule).
    pub(crate) digests: Vec<String>,
    /// Each source's home, by source id, as its components' identities
    /// name it (`home`; LLP 1121 D15): the source map says it, so a reader
    /// names a param by its full name from the file it reads it in.
    pub(crate) homes: Vec<Option<String>>,
    /// Each source's text as compiled, by source id: the source map checks
    /// a row's written place against it (LLP 1122 D6), never the disk.
    pub(crate) texts: Vec<std::sync::Arc<str>>,
    /// The literals a motion token was written into, or that a component's
    /// argument names one in (LLP 1121 D3): what `uses` reports for one.
    pub(crate) motion: Vec<contract_lower::motion::Site>,
    /// Each `var()` whose fallback the loader dropped (LLP 1153 D6): one
    /// warning says them.
    pub(crate) var_fallbacks: Vec<(Span, String)>,
}

/// One name a `use` brought: where it is written, and the declaration it
/// means, by namespace and program-unique name.
pub(crate) struct Import {
    pub(crate) span: Span,
    pub(crate) kind: Kind,
    pub(crate) name: String,
}
impl Sources {
    /// Map each source from where it was captured to where it lives: the
    /// first `(captured, original)` pair whose captured directory holds it
    /// (a bake's stage, and each package mirrored into the stage's
    /// `node_modules`, listed first; LLP 1091 D10).
    /// Each source's path, by source id.
    pub(crate) fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub(crate) fn relocate(&mut self, moves: &[(PathBuf, PathBuf)]) -> Result<(), String> {
        let moves = moves
            .iter()
            .map(|(captured, original)| {
                let canonical = captured.canonicalize().map_err(|e| e.to_string())?;
                Ok((captured, canonical, original))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let paths = self
            .paths
            .iter()
            .map(|path| {
                if !path.is_absolute() {
                    // An `exact:` module: compiled in, nowhere on disk.
                    return Ok(path.clone());
                }
                moves
                    .iter()
                    .find_map(|(captured, canonical, original)| {
                        path.strip_prefix(captured)
                            .or_else(|_| path.strip_prefix(canonical))
                            .ok()
                            .map(|relative| original.join(relative))
                    })
                    .ok_or_else(|| {
                        format!(
                            "source {} is outside captured root {}",
                            path.display(),
                            moves
                                .last()
                                .map(|(captured, ..)| captured.display().to_string())
                                .unwrap_or_default()
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.paths = paths;
        Ok(())
    }

    /// Every source's path and the SHA-256 of its text as compiled.
    pub(crate) fn digested(&self) -> impl Iterator<Item = (&Path, &str)> + '_ {
        self.paths
            .iter()
            .map(PathBuf::as_path)
            .zip(self.digests.iter().map(String::as_str))
    }

    /// Source `id`'s home, as its components' identities name it.
    pub(crate) fn home(&self, id: usize) -> Option<&str> {
        self.homes.get(id).and_then(|h| h.as_deref())
    }

    pub(crate) fn path(&self, span: contract_syntax::Span) -> &Path {
        &self.paths[span.source_id as usize]
    }
    pub(crate) fn resolve(&self, mut error: CompileError) -> CompileError {
        error.file = Some(self.path(error.span).into());
        for related in error.related.iter_mut() {
            related.file = Some(self.path(related.span).to_path_buf());
        }
        error
    }
}

pub(crate) fn load(
    path: &Path,
    src: &str,
    app_root: &Path,
) -> Result<(File, Sources), Vec<CompileError>> {
    let mut refused = Refused::default();
    let loaded = load_partial(path, src, app_root, &mut refused);
    refused.join(loaded)
}

/// [`load`], going on past a file's syntax refusals to check what parsed:
/// those refusals and what they hid are in `refused`, and the program is
/// returned only to be checked, never to be compiled into a plan.
pub(crate) fn load_partial(
    path: &Path,
    src: &str,
    app_root: &Path,
    refused: &mut Refused,
) -> Result<(File, Sources), Vec<CompileError>> {
    let root_key = path.canonicalize().unwrap_or_else(|_| {
        path.file_name()
            .map(|name| app_root.join(name))
            .unwrap_or_else(|| app_root.to_path_buf())
    });
    let mut loader = Loader {
        app_root,
        root_given: path.to_path_buf(),
        sources: Sources {
            paths: vec![path.to_path_buf()],
            origins: vec![Origin::App],
            digests: vec![digest(src)],
            homes: Vec::new(),
            texts: vec![src.into()],
            imports: Vec::new(),
            packages: Vec::new(),
            consulted: Vec::new(),
            motion: Vec::new(),
            var_fallbacks: Vec::new(),
        },
        active: vec![root_key.clone()],
        units: Vec::new(),
        cache: HashMap::new(),
        fixes: Vec::new(),
        proposed: Vec::new(),
        declared: Vec::new(),
        motion: Vec::new(),
        vars: None,
        refused: Refused::default(),
    };
    let loaded = loader.load_all(path, src, root_key);
    *refused = std::mem::take(&mut loader.refused);
    loaded.map(|file| (file, loader.sources))
}

impl Loader<'_> {
    /// The program rooted at `path` (canonically `root_key`), loaded and scoped.
    fn load_all(
        &mut self,
        path: &Path,
        src: &str,
        root_key: PathBuf,
    ) -> Result<File, Vec<CompileError>> {
        let resolve = |sources: &Sources, all: Vec<CompileError>| {
            all.into_iter()
                .map(|e| sources.resolve(e))
                .collect::<Vec<_>>()
        };
        if let Err(all) = self.load_source(&root_key, src, 0) {
            return Err(resolve(&self.sources, all));
        }
        // Each component's `param.` names, as their tokens (LLP 1121 D15),
        // before anything reads a row.
        let mut params_refused: Vec<CompileError> = Vec::new();
        let mut owned = Vec::new();
        for unit in &mut self.units {
            owned.extend(contract_lower::params::owned(&unit.file));
            params_refused.extend(
                contract_lower::params::rewrite(&mut unit.file)
                    .into_iter()
                    .map(CompileError::from),
            );
            // Read by a row, before a motion param is written into it.
            params_refused.extend(
                contract_lower::params::unused(&unit.file)
                    .into_iter()
                    .map(CompileError::from),
            );
        }
        let params: Vec<_> = self
            .units
            .iter()
            .flat_map(|u| contract_lower::params::tokens(&u.file))
            .collect();
        params_refused.extend(
            contract_lower::params::collisions(&params)
                .into_iter()
                .map(CompileError::from),
        );
        if !params_refused.is_empty() {
            return Err(resolve(&self.sources, params_refused));
        }
        // The design, read before scoping: its motion tokens, and the params of
        // those kinds, are written into the views first (`contract_lower::motion`).
        let design = design(path, &mut self.sources).map_err(|e| vec![e])?;
        let package_designs =
            packages::designs(&mut self.sources, code_beside(path)).map_err(|e| vec![e])?;
        // The design's components that aren't Contract, and its packages'
        // designs' (stage 12 B1), named and checked as Contract params are
        // (D15).
        if design.is_some() || !package_designs.is_empty() {
            let (mut designed, mut refused) = (Vec::new(), Vec::new());
            for d in design.iter().chain(&package_designs) {
                let (tokens, errors) = contract_lower::params::design_tokens(d);
                designed.extend(tokens);
                refused.extend(errors);
            }
            let all: Vec<_> = params.iter().chain(&designed).cloned().collect();
            refused.extend(contract_lower::params::collisions(&all));
            if !refused.is_empty() {
                return Err(resolve(
                    &self.sources,
                    refused.into_iter().map(CompileError::from).collect(),
                ));
            }
        }
        let identities: Vec<String> = self
            .units
            .iter()
            .flat_map(|u| contract_lower::params::identities(&u.file))
            .collect();
        self.motion = contract_lower::motion::tokens(
            design.as_ref().map(|d| d.text.as_str()),
            &package_designs,
            &params,
            &identities,
        );
        self.vars = Some(contract_lower::var_tokens::Tokens::of(
            design.as_ref().map(|d| d.text.as_str()),
            &package_designs,
            self.units.first().and_then(|u| u.file.theme.as_ref()),
        ));
        let library = library::read(&self.sources, self.app_root);
        match self.scope() {
            Ok(mut file) => {
                let escaped = contract_lower::params::escaped(&file, &owned);
                if !escaped.is_empty() {
                    let all = escaped.into_iter().map(CompileError::from).collect();
                    return Err(resolve(&self.sources, all));
                }
                file.library = library.components;
                file.library_packages = library.packages;
                file.design = design;
                file.package_designs = package_designs;
                crate::modules::desugar(&mut file).map_err(|all| resolve(&self.sources, all))?;
                Ok(file)
            }
            Err(all) => Err(resolve(&self.sources, all)),
        }
    }
}

/// The refusal of a component file outside the app in no named package.
const UNNAMED: &str = "contract-package-unnamed";

/// [`UNNAMED`]'s words: the package as its `use` named it, never its path.
fn unnamed(package: &str) -> String {
    format!("the package `{package}` is outside the app and its `package.json` has no `name`: a package outside the app needs a name in its package.json")
}

/// Where the app's design is (LLP 1121 D2): the file `app.json`'s
/// `"tamber"` names, else `app.tamber` beside the app's `app.contract`.
pub fn design_path(root: &Path) -> Option<PathBuf> {
    let dir = root.parent()?;
    let named = std::fs::read_to_string(dir.join("app.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|app| app.get("tamber")?.as_str().map(|p| dir.join(p)));
    named.or_else(|| {
        let beside = dir.join("app.tamber");
        (root.file_name()? == "app.contract" && beside.is_file()).then_some(beside)
    })
}

std::thread_local! {
    /// A scrub's design text (LLP 1121 stage 4, commit 6), compiled in
    /// place of the file's while [`with_design_text`] runs.
    static DESIGN_TEXT: std::cell::RefCell<Option<(PathBuf, String)>> =
        const { std::cell::RefCell::new(None) };
}

std::thread_local! {
    /// The design server's planned `.contract` texts (LLP 1121 stage 8 D),
    /// compiled in place of the files' while [`with_contract_texts`] runs.
    static CONTRACT_TEXT: std::cell::RefCell<Vec<(PathBuf, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `f` with each `(path, text)` read as that Contract source, written
/// to no file: a write the server checks before it commits.
pub fn with_contract_texts<R>(texts: Vec<(PathBuf, String)>, f: impl FnOnce() -> R) -> R {
    /// Puts the texts before back however `f` ends, a panic included.
    struct Restore(Vec<(PathBuf, String)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = std::mem::take(&mut self.0);
            CONTRACT_TEXT.with(|t| *t.borrow_mut() = before);
        }
    }
    let keyed = texts
        .into_iter()
        .map(|(p, t)| (p.canonicalize().unwrap_or(p), t))
        .collect();
    let _restore = Restore(CONTRACT_TEXT.with(|t| t.replace(keyed)));
    f()
}

/// The text the server planned for Contract source `path`, if any.
pub(crate) fn contract_text(path: &Path) -> Option<String> {
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    CONTRACT_TEXT.with(|t| {
        t.borrow()
            .iter()
            .find(|(p, _)| *p == key)
            .map(|(_, s)| s.clone())
    })
}

/// Run `f` with `text` read as the design at `path`, written to no file:
/// a scrub's preview, which the dev loop compiles and never publishes.
pub fn with_design_text<R>(path: &Path, text: String, f: impl FnOnce() -> R) -> R {
    /// Puts the text before back however `f` ends, a panic included.
    struct Restore(Option<(PathBuf, String)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            DESIGN_TEXT.with(|t| *t.borrow_mut() = before);
        }
    }
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let _restore = Restore(DESIGN_TEXT.with(|t| t.replace(Some((key, text)))));
    f()
}

/// The app's design, read (LLP 1121 D2), and a source of its own, so a
/// refusal in it names it.
fn design(
    root: &Path,
    sources: &mut Sources,
) -> Result<Option<contract_syntax::DesignSource>, CompileError> {
    let Some(path) = design_path(root) else {
        return Ok(None);
    };
    let key = path.canonicalize().unwrap_or_else(|_| path.clone());
    let scrubbed = DESIGN_TEXT.with(|t| {
        t.borrow()
            .as_ref()
            .and_then(|(p, text)| (*p == key).then(|| text.clone()))
    });
    let read = scrubbed.map_or_else(|| std::fs::read_to_string(&path), Ok);
    let text = read.map_err(|e| CompileError {
        pass: "use",
        id: "tamber-unreadable".into(),
        message: format!("the app's design `{}`: {e}", path.display()),
        span: Span::default(),
        file: Some(path.clone().into()),
        related: Box::new([]),
    })?;
    sources.paths.push(path.clone());
    sources.origins.push(Origin::App);
    sources.digests.push(digest(&text));
    sources.texts.push(text.as_str().into());
    let home = root
        .parent()
        .and_then(|dir| path.strip_prefix(dir).ok())
        .map_or_else(
            || {
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            },
            |p| p.to_string_lossy().replace('\\', "/"),
        );
    Ok(Some(contract_syntax::DesignSource {
        path: path.display().to_string(),
        home,
        text,
        source_id: (sources.paths.len() - 1) as u32,
        bindable: code_beside(root),
        package: None,
        root: None,
    }))
}

/// @ref LLP 1121 D7 — whether the app at `root` (its `app.contract`) has code
/// beside it that can name a design token outside its views: a TypeScript
/// data module (`app.ts`), a Rust data crate (`data/Cargo.toml`), or native
/// modules or hatches its `app.json` declares. The driver's half of
/// `contract_lower::names_tokens_outside_views`.
pub fn code_beside(root: &Path) -> bool {
    let dir = root.parent().unwrap_or(Path::new("."));
    if dir.join("app.ts").is_file() || dir.join("data/Cargo.toml").is_file() {
        return true;
    }
    let manifest = std::fs::read_to_string(dir.join("app.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    manifest.is_some_and(|m| {
        let declared = |key: &str| match &m[key] {
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(o) => !o.is_empty(),
            _ => false,
        };
        declared("modules") || declared("hatches")
    })
}

use home::{home, linked_home, named_home};
pub(crate) use uses::{apply as apply_uses, unreached, use_fixes};

/// One source a compilation reads (LLP 1091 D10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Its canonical path, or its `exact:` name.
    pub path: PathBuf,
    /// Where it came from.
    pub origin: Origin,
}

/// A package a `use` reached: the name it was reached by, and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The name `use` wrote.
    pub name: String,
    /// Its version, as its `package.json` says.
    pub version: String,
    /// Its directory, canonical.
    pub root: PathBuf,
    /// Its `package.json`, canonical.
    pub manifest: PathBuf,
}

/// Every source a compilation of one root reads, in load order, and why it
/// stopped, if it did: a watcher needs the files read before a failure too.
#[derive(Debug, Clone)]
pub struct SourceGraph {
    /// The sources, the root first.
    pub sources: Vec<Source>,
    /// Every package reached, by each name it was reached by.
    pub packages: Vec<Package>,
    /// Every `package.json` resolution read, including one whose `exports`
    /// then refused: what a fix would edit; and an icon's SVG the build
    /// looked for and did not find (LLP 1121 D10).
    pub consulted: Vec<PathBuf>,
    /// The loader's refusals; empty when every source loaded and scoped.
    pub errors: Vec<CompileError>,
    /// Each named package whose `assets/` an icon's SVG is read from (LLP
    /// 1121 stage 13), the SVG there or not (one vendored in the app
    /// included): what a watcher watches and digests for it.
    pub owners: Vec<Package>,
}

/// [`graph`]'s Contract half: every source loaded and scoped, which reads no design file (the design server's walk keeps
/// it across a design-only edit and re-runs [`GraphBase::tail`] alone: the third hands-on pass's latency).
pub(crate) fn graph_base(path: &Path, src: &str, app_root: &Path) -> GraphBase {
    let root_key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut loader = Loader {
        app_root,
        root_given: path.to_path_buf(),
        sources: Sources {
            paths: vec![root_key.clone()],
            origins: vec![Origin::App],
            digests: vec![digest(src)],
            homes: Vec::new(),
            texts: vec![src.into()],
            imports: Vec::new(),
            packages: Vec::new(),
            consulted: Vec::new(),
            motion: Vec::new(),
            var_fallbacks: Vec::new(),
        },
        active: vec![root_key.clone()],
        units: Vec::new(),
        cache: HashMap::new(),
        fixes: Vec::new(),
        proposed: Vec::new(),
        declared: Vec::new(),
        motion: Vec::new(),
        vars: None,
        refused: Refused::default(),
    };
    let loaded = loader.load_source(&root_key, src, 0);
    // The components' params (LLP 1121 D15), named as the build names them
    // (an icon param naming another): an icon param's SVG is read too.
    for unit in &mut loader.units {
        let _ = contract_lower::params::rewrite(&mut unit.file);
    }
    // The Contract components' params (D15), before scoping: the tail adds its package designs' (and so reads none here).
    let contract_params: Vec<_> = loader
        .units
        .iter()
        .flat_map(|u| contract_lower::params::tokens(&u.file))
        .collect();
    // And their identities before scoping, as the compile load reads them for its motion tokens.
    let loaded_identities: Vec<String> = loader
        .units
        .iter()
        .flat_map(|u| contract_lower::params::identities(&u.file))
        .collect();
    let errors = match loaded {
        Err(all) => all,
        Ok(()) if loader.refused.errors.is_empty() => loader.scope().err().unwrap_or_default(),
        Ok(()) => Vec::new(),
    };
    let errors = std::mem::take(&mut loader.refused.errors)
        .into_iter()
        .chain(errors.into_iter().map(|e| loader.sources.resolve(e)))
        .collect();
    let base: Vec<Source> = loader
        .sources
        .paths
        .iter()
        .zip(&loader.sources.origins)
        .map(|(path, origin)| Source {
            path: path.clone(),
            origin: origin.clone(),
        })
        .collect();
    // The components' identities, read after scoping as the icons' fold always read them (a lone unit scoped is gone).
    let identities: Vec<String> = loader
        .units
        .iter()
        .flat_map(|u| contract_lower::params::identities(&u.file))
        .collect();
    GraphBase {
        root_key,
        sources: loader.sources,
        base,
        params: contract_params,
        loaded_identities,
        identities,
        errors,
        app_root: app_root.to_path_buf(),
    }
}

/// One loaded file and, for each of its `use` lines, the file it names.
struct Unit {
    file: File,
    targets: Vec<usize>,
    /// What its components' `members` lines loaded (LLP 1091.002).
    parts: Vec<members::Part>,
}

struct Loader<'a> {
    app_root: &'a Path,
    /// The root file as it was named: one linked into the app's tree is
    /// named where the link sits ([`linked_home`]).
    root_given: PathBuf,
    sources: Sources,
    // Each file is loaded once, whichever paths reach it; a unit's index is
    // its source id. Only the active stack decides cycles.
    active: Vec<PathBuf>,
    units: Vec<Unit>,
    cache: HashMap<PathBuf, usize>,
    /// What `scope` found each file lacks, when it refused for that.
    fixes: Vec<uses::UseFix>,
    /// Every `use` edge §21's fixes would add, `(from, to)`: a cycle may be
    /// two new edges, neither on disk yet.
    proposed: Vec<(usize, usize)>,
    /// Each unit's declarations as written, before renaming, for §21's
    /// fixes to judge what a `use` would bring.
    declared: Vec<Vec<(Kind, String)>>,
    /// The app's duration and easing tokens (LLP 1121 D3), written into the
    /// motion rows before scoping.
    motion: Vec<(String, String)>,
    /// The app's tokens as a `var()` names them (LLP 1153 D6), once its
    /// design is read: each file's style rows are written with them before
    /// scoping.
    vars: Option<contract_lower::var_tokens::Tokens>,
    /// The syntax refusals of the files read so far, resolved.
    refused: Refused,
}
impl Loader<'_> {
    /// Parse `src` as unit `source_id`, then every file its uses name, depth
    /// first: the order today's merge put declarations in.
    fn load_source(
        &mut self,
        path: &Path,
        src: &str,
        source_id: u32,
    ) -> Result<(), Vec<CompileError>> {
        // Every syntax refusal in this file, not only its first; what parsed
        // is still loaded and checked, unless the file's declarations
        // cannot be told apart.
        let mut file = match contract_syntax::parse_source_partial(src, source_id) {
            Ok(file) => file,
            Err(partial) => {
                let sources = &self.sources;
                let errors = partial.errors.into_iter();
                (self.refused.errors).extend(errors.map(|e| sources.resolve(e.into())));
                self.refused.hidden.extend(partial.hidden);
                match partial.file {
                    Some(file) => *file,
                    None => {
                        self.refused.whole = true;
                        return Err(Vec::new());
                    }
                }
            }
        };
        // Each component's identity, its file relative to its root and its
        // name (LLP 1121 D15): what its params are named by.
        let origin = &self.sources.origins[source_id as usize];
        let found = named_home(path, origin, self.app_root);
        let linked = || match source_id {
            0 => linked_home(&self.root_given, origin, self.app_root).map(|h| (h, None)),
            _ => None,
        };
        let Some((home, owner)) = found.or_else(linked) else {
            // `link` refuses it at the `use` first; this is the backstop.
            let span = Span {
                source_id,
                ..Default::default()
            };
            let name = match origin {
                Origin::Package { name, .. } => name.as_str(),
                _ => "",
            };
            return Err(vec![name_error(UNNAMED, unnamed(name), span)]);
        };
        // A package's components know their package, whose `assets/` a
        // literal icon default reads (stage 13): the named package their
        // identity names, vendored ones included (round 2, Astra 1).
        let (package, root) = match owner {
            Some((name, dir)) => (Some(name), Some(dir.display().to_string())),
            None => (None, None),
        };
        for c in &mut file.components {
            c.params.identity = Some(format!("{home}#{}", c.name));
            c.params.package = package.clone();
            c.params.root = root.clone();
        }
        let homes = &mut self.sources.homes;
        if homes.len() <= source_id as usize {
            homes.resize(source_id as usize + 1, None);
        }
        homes[source_id as usize] = Some(home);
        contract_analyze::check_routes_root(&file, self.active.len() == 1)
            .map_err(|e| vec![CompileError::from(e)])?;
        self.units.push(Unit {
            file,
            targets: Vec::new(),
            parts: Vec::new(),
        });
        self.link(path, source_id as usize)
    }

    /// Load each file unit `index`'s `use` lines name, depth first.
    fn link(&mut self, path: &Path, index: usize) -> Result<(), Vec<CompileError>> {
        let uses = self.units[index].file.uses.clone();
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let from = self.sources.origins[index].clone();
        for u in &uses {
            let resolved = resolve(
                &u.path,
                &dir,
                &from,
                self.app_root,
                &mut self.sources.consulted,
            )
            .map_err(|(id, message)| {
                vec![use_error(id, format!("{}: {message}", described(u)), u)]
            })?;
            if let Origin::Package { name, .. } = &resolved.origin {
                if home(&resolved.key, &resolved.origin, self.app_root).is_none() {
                    let message = format!("{}: {}", described(u), unnamed(name));
                    return Err(vec![use_error(UNNAMED, message, u)]);
                }
            }
            if let Origin::Package {
                name,
                version,
                root,
                manifest,
            } = &resolved.origin
            {
                let package = Package {
                    name: name.clone(),
                    version: version.clone(),
                    root: root.clone(),
                    manifest: manifest.clone(),
                };
                if !self.sources.packages.contains(&package) {
                    self.sources.packages.push(package);
                }
            }
            let used = self.load_resolved(resolved, &described(u), u.span)?;
            self.units[index].targets.push(used);
        }
        self.link_members(path, index)
    }

    /// @ref LLP 1153 D6 — `file`'s style rows with each `var()` that names a
    /// token written as the name, before a motion token is written in or a
    /// keyframes name is scoped; a row naming none is refused beside the
    /// syntax's refusals, and dropped.
    fn vars(&mut self, file: &mut File) {
        let Some(tokens) = &self.vars else { return };
        let found = contract_lower::var_tokens::rewrite(file, tokens);
        let sources = &self.sources;
        let refused = found.refused.into_iter();
        (self.refused.errors).extend(refused.map(|e| sources.resolve(e.into())));
        self.sources.var_fallbacks.extend(found.fallbacks);
    }

    /// Give every declaration its program-unique name (D4), rewrite each
    /// file through its own scope (D1, D5), and merge the files into the one
    /// `File` every later pass reads.
    fn scope(&mut self) -> Result<File, Vec<CompileError>> {
        self.prepare_members()?;
        if self.units.len() == 1 {
            let mut file = self.units.pop().unwrap().file;
            self.vars(&mut file);
            self.sources.motion = contract_lower::motion::substitute(&mut file, &self.motion);
            contract_syntax::resolve_clock_timelines(&mut file).map_err(|e| vec![e.into()])?;
            file.uses.clear();
            return Ok(file);
        }
        let unique = self.unique_names();
        let mut scopes: Vec<Option<Scope>> = (0..self.units.len()).map(|_| None).collect();
        for index in 0..self.units.len() {
            self.scope_of(index, &unique, &mut scopes)
                .map_err(|e| vec![e])?;
        }
        let elsewhere = self.declared_elsewhere(&unique);
        self.declared = self
            .units
            .iter()
            .map(|unit| {
                declarations(&unit.file)
                    .map(|(k, n)| (k, n.to_owned()))
                    .collect()
            })
            .collect();
        // Every file's references to names it does not see, refused together.
        let mut missing = Vec::new();
        let mut refused: Vec<CompileError> = Vec::new();
        let shape_owners: Vec<(usize, Vec<String>, String)> = self
            .units
            .iter()
            .enumerate()
            .flat_map(|(index, unit)| {
                let path = &self.sources.paths[index];
                let shown = path
                    .strip_prefix(self.app_root)
                    .unwrap_or(path)
                    .display()
                    .to_string();
                let unique = &unique;
                unit.file.shapes.iter().map(move |s| {
                    let generated = unique[index]
                        .get(&(Kind::Call, s.name.clone()))
                        .cloned()
                        .unwrap_or_else(|| s.name.clone());
                    (index, vec![s.name.clone(), generated], shown.clone())
                })
            })
            .collect();
        // @ref LLP 1121 D3 — the design's duration and easing tokens are
        // written into each motion row before scoping reads an animation's
        // keyframes name (the root's design, the app's).
        let motion = std::mem::take(&mut self.motion);
        let mut units = std::mem::take(&mut self.units);
        for unit in units.iter_mut() {
            self.vars(&mut unit.file);
            let sites = contract_lower::motion::substitute(&mut unit.file, &motion);
            self.sources.motion.extend(sites);
        }
        self.units = units;
        for (index, unit) in self.units.iter_mut().enumerate() {
            let mut scope = scopes[index].take().expect("every unit is scoped");
            scope.elsewhere = elsewhere
                .iter()
                .filter(|(key, e)| e.unit != index && !scope.names.contains_key(*key))
                .map(|(key, e)| (key.clone(), e.clone()))
                .collect();
            // Every other file's shape this file does not see as a shape, by
            // its declared and its generated name.
            scope.foreign_shapes = shape_owners
                .iter()
                .filter(|(owner, _, _)| *owner != index)
                .flat_map(|(_, names, file)| names.iter().map(move |n| (n.clone(), file.clone())))
                .filter(|(n, _)| {
                    !scope
                        .names
                        .get(&(Kind::Call, n.clone()))
                        .is_some_and(|to| scope.shapes.contains(to))
                })
                .collect();
            let mut aliases: Vec<_> = scope
                .names
                .iter()
                .filter(|((kind, _), _)| *kind == Kind::Decoration)
                .map(|((_, alias), target)| (alias.clone(), target.clone()))
                .collect();
            aliases.sort();
            unit.file.decoration_scopes.insert(index as u32, aliases);
            let mut atoms: Vec<_> = scope
                .names
                .iter()
                .filter(|((kind, _), _)| *kind == Kind::Atom)
                .map(|((_, alias), target)| (alias.clone(), target.clone()))
                .collect();
            atoms.sort();
            unit.file.atom_scopes.insert(index as u32, atoms);
            rescope(&mut unit.file, &scope).map_err(|e| vec![e.into()])?;
            // Refusals that are no `use` line's to fix, reported with the rest.
            refused.extend(scope.refused.take().into_iter().map(CompileError::from));
            let misses = scope.missing.take();
            if !misses.is_empty() {
                let found: Vec<_> = misses
                    .into_iter()
                    .map(|m| {
                        let e = scope.elsewhere[&(m.kind, m.name.clone())].clone();
                        (m, e)
                    })
                    .collect();
                missing.push((index, found));
                continue;
            }
            let timelines = scope
                .names
                .iter()
                .filter(|((kind, _), _)| *kind == Kind::Timeline)
                .map(|((_, local), to)| (local.clone(), to.clone()))
                .collect();
            contract_syntax::resolve_clock_timelines_in(&mut unit.file, &timelines)
                .map_err(|e| vec![e.into()])?;
        }
        if !missing.is_empty() || !refused.is_empty() {
            // Only the edges a fix would write: one declarer, no clash, a
            // specifier that reaches it.
            self.proposed = missing
                .iter()
                .flat_map(|(index, found)| {
                    found
                        .iter()
                        .filter(|(m, e)| {
                            e.declared == m.name
                                && e.declaring.iter().filter(|&&u| u != *index).count() == 1
                                && self.clash(*index, e.unit, &m.name).is_none()
                                && self.specifier(*index, e.unit).is_some()
                        })
                        .map(move |(_, e)| (*index, e.unit))
                })
                .collect();
            self.fixes = missing
                .into_iter()
                .map(|(index, found)| self.use_fix(index, found))
                .collect();
            return Err(self
                .fixes
                .iter()
                .map(|f| f.error.clone())
                .chain(refused)
                .collect());
        }
        self.splice_members();
        Ok(self.merge())
    }

    /// Each unit's own declarations, by namespace and name, to their
    /// program-unique names: the declared name when no earlier file (the root
    /// first, then depth-first `use` order) has it in that namespace, else
    /// `{name}__{file stem}`, numbered until unique. One file's two
    /// declarations of a name keep it, so the passes that refuse that refuse it.
    fn unique_names(&self) -> Vec<HashMap<(Kind, String), String>> {
        let mut taken: HashSet<(Kind, String)> = HashSet::new();
        // Each file's local bindings: another file's `fn` or shape of one of
        // these names is renamed, or the binding's calls would reach it. A
        // members block's are its listing file's (LLP 1091.002 D4).
        let listed = self.listed_by();
        let (bound, in_blocks) = self.bindings_by_file();
        let called: Vec<HashSet<String>> = self
            .units
            .iter()
            .map(|unit| contract_syntax::scope::calls(&mut unit.file.clone()))
            .collect();
        self.units
            .iter()
            .enumerate()
            .map(|(index, unit)| {
                let stem: String = self.sources.paths[index]
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("file")
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let mut own = HashMap::new();
                for (kind, name) in declarations(&unit.file) {
                    let key = (kind, name.to_owned());
                    if own.contains_key(&key) {
                        continue;
                    }
                    let mut unique = name.to_owned();
                    let mut n = 1;
                    // A used file's shape named like a roster function is
                    // renamed, or every file's call of the roster function
                    // would construct it.
                    // A name the compiler answers (`length`, `path`, …) that
                    // another file calls without declaring or naming it: this
                    // file's declaration would capture that call.
                    let captures = |name: &str| {
                        compiler_call(name)
                            && self.units.iter().enumerate().any(|(other, unit)| {
                                other != index
                                    && called[other].contains(name)
                                    // Named by a use of a callable: a shape or `fn`
                                    // the used file declares (a style of the name
                                    // is not one).
                                    && !unit.file.uses.iter().zip(&unit.targets).any(|(u, &t)| {
                                        u.names.iter().any(|n| {
                                            n.local() == name
                                                && declarations(&self.units[t].file).any(
                                                    |(kind, d)| kind == Kind::Call && d == n.name,
                                                )
                                        })
                                    })
                                    && !unit.file.fns.iter().any(|f| f.name == name)
                                    && !unit.file.shapes.iter().any(|s| s.name == name)
                            })
                    };
                    // A shape named `action` would be what every other file's bare
                    // `action` type reads (the checker prefers a shape).
                    let roster = |name: &str| {
                        kind == Kind::Call
                            // An intrinsic's spelling: a `fn` of it is never
                            // what a call of the name reads, so one imported
                            // under another name must not take it back.
                            && (matches!(name, "action" | "pending" | "failed" | "failure")
                                // A shape spelled `path`: its constructor is
                                // the router's `path()` under any alias.
                                || (name == "path"
                                    && unit.file.shapes.iter().any(|s| s.name == name))
                                || (index > 0 && exact_plan::Stdlib::from_name(name).is_some())
                                || captures(name)
                                || bound.iter().enumerate().any(|(other, names)| {
                                    other != index && names.contains(name)
                                })
                                || in_blocks.iter().enumerate().any(|(other, names)| {
                                    listed[other] != Some(index) && names.contains(name)
                                }))
                    };
                    // A generated name is no binding's in any file either, or
                    // the binding's calls (in its own file too) would reach it.
                    let bound_anywhere = |unique: &str| {
                        let mut all = bound.iter().chain(&in_blocks);
                        unique != name && all.any(|names| names.contains(unique))
                    };
                    while taken.contains(&(kind, unique.clone()))
                        || roster(&unique)
                        || bound_anywhere(&unique)
                    {
                        n += 1;
                        unique = if n == 2 {
                            format!("{name}__{stem}")
                        } else {
                            format!("{name}__{stem}_{}", n - 1)
                        };
                    }
                    taken.insert((kind, unique.clone()));
                    own.insert(key, unique);
                }
                own
            })
            .collect()
    }

    /// A unit's scope: its own declarations and the names its uses bring.
    /// A used file's scope is what can be named from it, its uses included,
    /// so a file can gather a library's names for others (LLP 1091 D3).
    fn scope_of(
        &mut self,
        index: usize,
        unique: &[HashMap<(Kind, String), String>],
        scopes: &mut [Option<Scope>],
    ) -> Result<(), CompileError> {
        if scopes[index].is_some() {
            return Ok(());
        }
        let targets = self.units[index].targets.clone();
        for &target in &targets {
            self.scope_of(target, unique, scopes)?;
        }
        let mut names = unique[index].clone();
        let unit = &self.units[index];
        for (u, &target) in unit.file.uses.iter().zip(&targets) {
            let from = scopes[target]
                .as_ref()
                .expect("a used file is scoped before its user");
            for n in &u.names {
                let found: Vec<_> = from
                    .names
                    .iter()
                    .filter(|((_, local), _)| *local == n.name)
                    .map(|((kind, _), to)| (*kind, to.clone()))
                    .collect();
                if found.is_empty() {
                    return Err(unknown(u, n, from));
                }
                if let Some(m) = self.data_only(unique, &found) {
                    // Bytes are data code's (LLP 1133.006 D8 item 1; LLP
                    // 1133.022 D19 item 4).
                    if m.substitute.contains(r#""bytes":true"#) {
                        return Err(name_error(
                            "contract-module-bytes",
                            format!(
                                "`{}` of `{}` carries bytes, which Contract has none of: call it in `app.ts` and send Contract what it needs (LLP 1133.006 D8)",
                                m.declared, m.package
                            ),
                            n.span,
                        ));
                    }
                    return Err(name_error(
                        "module-name-data-only",
                        format!(
                            "`{}` of `{}` is called from data code alone (LLP 1133.011 D12): its answer would become a mutation's value in `state`; call it in `app.ts`, and send what the server answers",
                            m.declared, m.package
                        ),
                        n.span,
                    ));
                }
                for (kind, to) in found {
                    let key = (kind, n.local().to_owned());
                    match names.get(&key) {
                        Some(prior) if *prior == to => {}
                        Some(_) if unique[index].contains_key(&key) => {
                            return Err(name_error(
                                "contract-use-shadows",
                                format!(
                                    "`use {}` brings a {} `{}` that this file declares too; rename one with `as`",
                                    n.name,
                                    kind.what(),
                                    n.local()
                                ),
                                n.span,
                            ))
                        }
                        Some(prior) => {
                            // Named by the files that declare them: two
                            // files' declarations are two however alike
                            // their text (LLP 1091 D7; splitter rough 1
                            // read "different" as a different text).
                            let file = |name: &str| {
                                unique
                                    .iter()
                                    .position(|own| own.values().any(|v| v == name))
                                    .map(|i| self.shown(i))
                                    .unwrap_or_default()
                            };
                            return Err(name_error(
                                "contract-use-duplicate",
                                format!(
                                    "`use {}` brings the {} `{}` declared in {}, and another `use` already brought the one declared in {}: a declaration is its file's, however alike the two read (LLP 1091 D7); declare it once and `use` it from that file, or rename one with `as`",
                                    n.name,
                                    kind.what(),
                                    n.local(),
                                    file(&to),
                                    file(prior)
                                ),
                                n.span,
                            ))
                        }
                        None => {
                            names.insert(key, to.clone());
                        }
                    }
                    self.sources.imports.push(Import {
                        span: n.span,
                        kind,
                        name: to,
                    });
                }
            }
        }
        scopes[index] = Some(Scope {
            names,
            elsewhere: HashMap::new(),
            // The roster plus the intrinsics (`pending`, `failed`, `path`, `t`).
            roster: Some(compiler_call),
            builtin_types: builtin_types(),
            foreign_shapes: HashMap::new(),
            shapes: self
                .units
                .iter()
                .enumerate()
                .flat_map(|(u, unit)| {
                    unit.file
                        .shapes
                        .iter()
                        .filter_map(move |s| unique[u].get(&(Kind::Call, s.name.clone())))
                })
                .cloned()
                .collect(),
            modules: self
                .units
                .iter()
                .enumerate()
                .flat_map(|(u, unit)| {
                    unit.file
                        .modules
                        .iter()
                        .filter_map(move |m| unique[u].get(&(Kind::Call, m.name.clone())))
                })
                .cloned()
                .collect(),
            ..Scope::default()
        });
        Ok(())
    }

    /// Every declared name, by namespace, to the first unit that declares it
    /// and that unit's path, for refusing a name a file does not see — the
    /// generated ones too, so `Helper__ui` is no way around a `use`.
    fn declared_elsewhere(
        &self,
        unique: &[HashMap<(Kind, String), String>],
    ) -> HashMap<(Kind, String), Elsewhere> {
        let mut declaring: HashMap<(Kind, &str), Vec<usize>> = HashMap::new();
        for (index, own) in unique.iter().enumerate() {
            for (kind, declared) in own.keys() {
                declaring.entry((*kind, declared)).or_default().push(index);
            }
        }
        let mut out = HashMap::new();
        for (index, unit) in self.units.iter().enumerate() {
            let file = self.shown(index);
            let at = |kind: Kind, declared: &str| Elsewhere {
                file: file.clone(),
                unit: index,
                declared: declared.to_owned(),
                declaring: declaring
                    .get(&(kind, declared))
                    .cloned()
                    .unwrap_or_default(),
            };
            for (kind, name) in declarations(&unit.file) {
                out.entry((kind, name.to_owned()))
                    .or_insert_with(|| at(kind, name));
            }
            for ((kind, declared), generated) in &unique[index] {
                out.entry((*kind, generated.clone()))
                    .or_insert_with(|| at(*kind, declared));
            }
        }
        out
    }

    fn merge(&mut self) -> File {
        let mut files: Vec<File> = self.units.drain(..).map(|unit| unit.file).collect();
        let mut names = NameSpans::default();
        for file in &mut files {
            names.names.extend(std::mem::take(&mut file.names.names));
            names
                .sources
                .extend(std::mem::take(&mut file.names.sources));
        }
        let faces = |font: &contract_syntax::FontDecl| {
            let mut faces: Vec<_> = font
                .faces
                .iter()
                .map(|f| (f.weight, f.italic, f.source.clone()))
                .collect();
            faces.sort();
            faces
        };
        // Fonts stay app-global, as `@font-face` is (D6): one family declared
        // alike in two files is one; declared differently, lowering refuses it.
        let mut fonts: Vec<contract_syntax::FontDecl> = Vec::new();
        for font in files
            .iter_mut()
            .flat_map(|file| std::mem::take(&mut file.fonts))
        {
            let alike = |prior: &contract_syntax::FontDecl| {
                prior.name == font.name && faces(prior) == faces(&font)
            };
            if !fonts.iter().any(alike) {
                fonts.push(font);
            }
        }
        // A module's `sound` lines join its user's (LLP 1096 D1), one per path.
        let mut sounds: Vec<contract_syntax::SoundDecl> = Vec::new();
        for sound in files
            .iter_mut()
            .flat_map(|file| std::mem::take(&mut file.sounds))
        {
            if !sounds.iter().any(|prior| prior.source == sound.source) {
                sounds.push(sound);
            }
        }
        macro_rules! all {
            ($field:ident) => {
                files
                    .iter_mut()
                    .flat_map(|file| std::mem::take(&mut file.$field))
                    .collect()
            };
        }
        File {
            names,
            routes: files[0].routes.take(),
            tests: std::mem::take(&mut files[0].tests),
            properties: std::mem::take(&mut files[0].properties),
            launch: std::mem::take(&mut files[0].launch),
            uses: Vec::new(),
            fonts,
            sounds,
            shapes: all!(shapes),
            styles: all!(styles),
            decorations: all!(decorations),
            decoration_scopes: files
                .iter_mut()
                .flat_map(|file| std::mem::take(&mut file.decoration_scopes))
                .collect(),
            atoms: all!(atoms),
            atom_scopes: files
                .iter_mut()
                .flat_map(|file| std::mem::take(&mut file.atom_scopes))
                .collect(),
            keyframes: all!(keyframes),
            timelines: all!(timelines),
            // CSS `@color-profile`s are global by name too (LLP 1100 D3).
            color_profiles: all!(color_profiles),
            // The app's theme is its root file's (LLP 1095 D4).
            theme: files[0].theme.take(),
            // The design is the app's, read beside its root (`design`).
            design: None,
            package_designs: Vec::new(),
            library: Vec::new(),
            library_packages: Vec::new(),
            fns: all!(fns),
            modules: all!(modules),
            components: all!(components),
            // Each block is spliced into its component (LLP 1091.002 D4).
            members: None,
            deferred: Vec::new(),
        }
    }
}

/// The types the compiler declares for every program, read from the type
/// checker's own declarations of an empty file.
fn builtin_types() -> HashSet<String> {
    contract_types::check_declarations(&File::default())
        .map(|shapes| shapes.map.into_keys().collect())
        .unwrap_or_default()
}

/// A call the compiler answers itself: the roster, and the intrinsics the
/// type checker reads before any `fn` (`pending`, `failed`, `path`, `t`).
fn compiler_call(name: &str) -> bool {
    exact_plan::Stdlib::from_name(name).is_some()
        || matches!(name, "pending" | "failed" | "failure" | "path" | "t")
}

/// A file's own top-level names, by namespace, in source order.
fn declarations(file: &File) -> impl Iterator<Item = (Kind, &str)> {
    (file.components.iter().map(|c| (Kind::Component, &*c.name)))
        .chain(file.shapes.iter().map(|s| (Kind::Call, &*s.name)))
        .chain(file.fns.iter().map(|f| (Kind::Call, &*f.name)))
        .chain(file.modules.iter().map(|m| (Kind::Call, &*m.name)))
        .chain(file.styles.iter().map(|s| (Kind::Style, &*s.name)))
        .chain(
            file.decorations
                .iter()
                .map(|s| (Kind::Decoration, &*s.name)),
        )
        .chain(file.atoms.iter().map(|s| (Kind::Atom, &*s.name)))
        .chain(file.keyframes.iter().map(|k| (Kind::Keyframes, &*k.name)))
        .chain(file.timelines.iter().map(|t| (Kind::Timeline, &*t.name)))
}

/// `use A, B from "path"` as a refusal names it.
fn described(u: &UseDecl) -> String {
    let names = u
        .names
        .iter()
        .map(|n| n.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("`use {names} from \"{}\"`", u.path)
}

fn unknown(u: &UseDecl, n: &UseName, from: &Scope) -> CompileError {
    let mut choices = Vec::new();
    for (kind, label) in [
        (Kind::Component, "components"),
        (Kind::Call, "shapes and functions"),
        (Kind::Style, "styles"),
        (Kind::Decoration, "decorations"),
        (Kind::Atom, "atoms"),
        (Kind::Keyframes, "keyframes"),
        (Kind::Timeline, "timelines"),
    ] {
        let mut names: Vec<_> = from
            .names
            .keys()
            .filter(|(k, _)| *k == kind)
            .map(|(_, name)| format!("`{name}`"))
            .collect();
        names.sort();
        if !names.is_empty() {
            choices.push(format!("{label}: {}", names.join(", ")));
        }
    }
    let available = if choices.is_empty() {
        "this file declares nothing that can be used".to_owned()
    } else {
        format!("available {}", choices.join("; "))
    };
    name_error(
        "contract-use-unknown",
        format!(
            "`{}` declares no component, shape, style, function, keyframes, or timeline `{}`; {available}",
            u.path, n.name
        ),
        n.span,
    )
}

fn name_error(id: &str, message: String, span: Span) -> CompileError {
    CompileError {
        pass: "use",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    }
}

fn use_error(id: &str, message: String, u: &UseDecl) -> CompileError {
    name_error(id, message, u.span)
}

/// The SHA-256 of a source's text, hex.
fn digest(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
