//! File loading and source identity shared by Contract compilation and navigation.
//! @ref LLP 1017.000 P8; LLP 1035.005 D2/D3; LLP 1091 (module scope).

use crate::resolve::{resolve, Origin};
use crate::CompileError;
use contract_syntax::scope::{rescope, Kind, Scope};
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
    let root_key = path.canonicalize().unwrap_or_else(|_| {
        path.file_name()
            .map(|name| app_root.join(name))
            .unwrap_or_else(|| app_root.to_path_buf())
    });
    let mut loader = Loader {
        app_root,
        sources: Sources {
            paths: vec![path.to_path_buf()],
            origins: vec![Origin::App],
            imports: Vec::new(),
            packages: Vec::new(),
            consulted: Vec::new(),
        },
        active: vec![root_key.clone()],
        units: Vec::new(),
        cache: HashMap::new(),
    };
    let resolve = |sources: &Sources, all: Vec<CompileError>| {
        all.into_iter()
            .map(|e| sources.resolve(e))
            .collect::<Vec<_>>()
    };
    if let Err(all) = loader.load_source(&root_key, src, 0) {
        return Err(resolve(&loader.sources, all));
    }
    match loader.scope() {
        Ok(file) => Ok((file, loader.sources)),
        Err(e) => Err(resolve(&loader.sources, vec![e])),
    }
}

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
    /// then refused: what a fix would edit.
    pub consulted: Vec<PathBuf>,
    /// The loader's refusals; empty when every source loaded and scoped.
    pub errors: Vec<CompileError>,
}

pub(crate) fn graph(path: &Path, src: &str, app_root: &Path) -> SourceGraph {
    let root_key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut loader = Loader {
        app_root,
        sources: Sources {
            paths: vec![root_key.clone()],
            origins: vec![Origin::App],
            imports: Vec::new(),
            packages: Vec::new(),
            consulted: Vec::new(),
        },
        active: vec![root_key.clone()],
        units: Vec::new(),
        cache: HashMap::new(),
    };
    let errors = match loader.load_source(&root_key, src, 0) {
        Err(all) => all,
        Ok(()) => loader.scope().err().into_iter().collect(),
    };
    let errors = errors
        .into_iter()
        .map(|e| loader.sources.resolve(e))
        .collect();
    let sources = loader
        .sources
        .paths
        .iter()
        .zip(&loader.sources.origins)
        .map(|(path, origin)| Source {
            path: path.clone(),
            origin: origin.clone(),
        })
        .collect();
    SourceGraph {
        sources,
        packages: loader.sources.packages,
        consulted: loader.sources.consulted,
        errors,
    }
}

/// One loaded file and, for each of its `use` lines, the file it names.
struct Unit {
    file: File,
    targets: Vec<usize>,
}

struct Loader<'a> {
    app_root: &'a Path,
    sources: Sources,
    // Each file is loaded once, whichever paths reach it; a unit's index is
    // its source id. Only the active stack decides cycles.
    active: Vec<PathBuf>,
    units: Vec<Unit>,
    cache: HashMap<PathBuf, usize>,
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
        // Every syntax refusal in this file, not only its first.
        let file = contract_syntax::parse_source_all(src, source_id)
            .map_err(|all| all.into_iter().map(CompileError::from).collect::<Vec<_>>())?;
        contract_analyze::check_routes_root(&file, self.active.len() == 1)
            .map_err(|e| vec![CompileError::from(e)])?;
        let uses = file.uses.clone();
        let index = source_id as usize;
        self.units.push(Unit {
            file,
            targets: Vec::new(),
        });
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
            let key = resolved.key;
            if self.active.contains(&key) {
                return Err(vec![use_error(
                    "contract-use-cycle",
                    format!("{} returns to a file already being loaded", described(u)),
                    u,
                )]);
            }
            let used = match self.cache.get(&key) {
                Some(&used) => used,
                None => {
                    let used_src = match resolved.builtin {
                        Some(text) => text.to_owned(),
                        None => std::fs::read_to_string(&key).map_err(|e| {
                            vec![use_error(
                                "contract-use-unreadable",
                                format!("{}: {}: {e}", described(u), key.display()),
                                u,
                            )]
                        })?,
                    };
                    let used = self.sources.paths.len();
                    self.sources.paths.push(key.clone());
                    self.sources.origins.push(resolved.origin);
                    self.active.push(key.clone());
                    self.load_source(&key, &used_src, used as u32)?;
                    self.active.pop();
                    self.cache.insert(key, used);
                    used
                }
            };
            self.units[index].targets.push(used);
        }
        Ok(())
    }

    /// Give every declaration its program-unique name (D4), rewrite each
    /// file through its own scope (D1, D5), and merge the files into the one
    /// `File` every later pass reads.
    fn scope(&mut self) -> Result<File, CompileError> {
        if self.units.len() == 1 {
            let mut file = self.units.pop().unwrap().file;
            contract_syntax::resolve_clock_timelines(&mut file)?;
            file.uses.clear();
            return Ok(file);
        }
        let unique = self.unique_names();
        let mut scopes: Vec<Option<Scope>> = (0..self.units.len()).map(|_| None).collect();
        for index in 0..self.units.len() {
            self.scope_of(index, &unique, &mut scopes)?;
        }
        let elsewhere = self.declared_elsewhere(&unique);
        for (index, unit) in self.units.iter_mut().enumerate() {
            let mut scope = scopes[index].take().expect("every unit is scoped");
            scope.elsewhere = elsewhere
                .iter()
                .filter(|(key, (owner, _))| *owner != index && !scope.names.contains_key(*key))
                .map(|(key, (_, file))| (key.clone(), file.clone()))
                .collect();
            rescope(&mut unit.file, &scope)?;
            let timelines = scope
                .names
                .iter()
                .filter(|((kind, _), _)| *kind == Kind::Timeline)
                .map(|((_, local), to)| (local.clone(), to.clone()))
                .collect();
            contract_syntax::resolve_clock_timelines_in(&mut unit.file, &timelines)?;
        }
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
        // these names is renamed, or the binding's calls would reach it.
        let bound: Vec<HashSet<String>> = self
            .units
            .iter()
            .map(|unit| contract_syntax::scope::bindings(&mut unit.file.clone()))
            .collect();
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
                    // `path` is the router's unless a `fn path` is in the
                    // program: another file's call of the router's would
                    // reach this one.
                    let path = |name: &str| {
                        name == "path"
                            && self.units.iter().enumerate().any(|(other, unit)| {
                                other != index
                                    && called[other].contains(name)
                                    && !unit
                                        .file
                                        .uses
                                        .iter()
                                        .any(|u| u.names.iter().any(|n| n.local() == name))
                                    && !unit.file.fns.iter().any(|f| f.name == name)
                            })
                    };
                    let roster = |name: &str| {
                        kind == Kind::Call
                            && ((index > 0 && exact_plan::Stdlib::from_name(name).is_some())
                                || path(name)
                                || bound
                                    .iter()
                                    .enumerate()
                                    .any(|(other, names)| other != index && names.contains(name)))
                    };
                    while taken.contains(&(kind, unique.clone())) || roster(&unique) {
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
                        Some(_) => {
                            return Err(name_error(
                                "contract-use-duplicate",
                                format!(
                                    "`use {}` brings a {} `{}` that another `use` already brought from a different declaration; rename one with `as`",
                                    n.name,
                                    kind.what(),
                                    n.local()
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
            roster: Some(compiler_call),
        });
        Ok(())
    }

    /// Every declared name, by namespace, to the first unit that declares it
    /// and that unit's path, for refusing a name a file does not see — the
    /// generated ones too, so `Helper__ui` is no way around a `use`.
    fn declared_elsewhere(
        &self,
        unique: &[HashMap<(Kind, String), String>],
    ) -> HashMap<(Kind, String), (usize, String)> {
        let mut out = HashMap::new();
        for (index, unit) in self.units.iter().enumerate() {
            let path = &self.sources.paths[index];
            let shown = path
                .strip_prefix(self.app_root)
                .unwrap_or(path)
                .display()
                .to_string();
            for (kind, name) in declarations(&unit.file) {
                out.entry((kind, name.to_owned()))
                    .or_insert_with(|| (index, shown.clone()));
            }
            for ((kind, _), generated) in &unique[index] {
                out.entry((*kind, generated.clone()))
                    .or_insert_with(|| (index, shown.clone()));
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
            launch: std::mem::take(&mut files[0].launch),
            uses: Vec::new(),
            fonts,
            shapes: all!(shapes),
            styles: all!(styles),
            keyframes: all!(keyframes),
            timelines: all!(timelines),
            fns: all!(fns),
            components: all!(components),
        }
    }
}

/// A call the compiler answers itself: the roster, and the intrinsics the
/// type checker reads before any `fn` (`pending`, `failed`, `path`, `t`).
fn compiler_call(name: &str) -> bool {
    exact_plan::Stdlib::from_name(name).is_some()
        || matches!(name, "pending" | "failed" | "path" | "t")
}

/// A file's own top-level names, by namespace, in source order.
fn declarations(file: &File) -> impl Iterator<Item = (Kind, &str)> {
    (file.components.iter().map(|c| (Kind::Component, &*c.name)))
        .chain(file.shapes.iter().map(|s| (Kind::Call, &*s.name)))
        .chain(file.fns.iter().map(|f| (Kind::Call, &*f.name)))
        .chain(file.styles.iter().map(|s| (Kind::Style, &*s.name)))
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
