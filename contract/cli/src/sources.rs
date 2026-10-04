//! File loading and source identity shared by Contract compilation and navigation.
//! @ref LLP 1017.000 P8; LLP 1035.005 D2/D3.

use crate::CompileError;
use contract_syntax::{File, NameSpans, UseDecl, VisitSpans};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};

pub(crate) struct Sources {
    paths: Vec<PathBuf>,
    pub(crate) imports: Vec<UseDecl>,
}
impl Sources {
    pub(crate) fn relocate(&mut self, captured: &Path, original: &Path) -> Result<(), String> {
        let canonical = captured.canonicalize().map_err(|e| e.to_string())?;
        let paths = self
            .paths
            .iter()
            .map(|path| {
                path.strip_prefix(captured)
                    .or_else(|_| path.strip_prefix(&canonical))
                    .map(|relative| original.join(relative))
                    .map_err(|_| {
                        format!(
                            "source {} is outside captured root {}",
                            path.display(),
                            captured.display()
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
            imports: Vec::new(),
        },
        active: vec![root_key],
        files: Vec::new(),
        cache: HashMap::new(),
    };
    let exports = loader.load_source(path, src, 0).map_err(|all| {
        all.into_iter()
            .map(|e| loader.sources.resolve(e))
            .collect::<Vec<_>>()
    })?;
    let file = loader.materialize(&exports);
    Ok((file, loader.sources))
}

struct Loader<'a> {
    app_root: &'a Path,
    sources: Sources,
    // Keep each source AST once. Completed imports share declaration indices,
    // never copies of transitive syntax trees. Only the active stack decides cycles.
    active: Vec<PathBuf>,
    files: Vec<File>,
    cache: HashMap<PathBuf, Rc<Exports>>,
}
impl Loader<'_> {
    fn load_source(
        &mut self,
        path: &Path,
        src: &str,
        source_id: u32,
    ) -> Result<Rc<Exports>, Vec<CompileError>> {
        // Every syntax refusal in this file, not only its first.
        let file = contract_syntax::parse_source_all(src, source_id)
            .map_err(|all| all.into_iter().map(CompileError::from).collect::<Vec<_>>())?;
        contract_analyze::check_routes_root(&file, self.active.len() == 1)
            .map_err(CompileError::from)?;
        self.sources.imports.extend(file.uses.iter().cloned());
        let mut exports = Exports::new(&file, source_id as usize);
        let uses = file.uses.clone();
        self.files.push(file);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut merged = HashSet::new();
        for u in &uses {
            validate_use_path(u)?;
            let target = dir.join(&u.path);
            let key = target.canonicalize().map_err(|e| {
                use_error(
                    "contract-use-unreadable",
                    format!(
                        "`use {} from \"{}\"`: {}: {e}",
                        u.name,
                        u.path,
                        target.display()
                    ),
                    u,
                )
            })?;
            if !key.starts_with(self.app_root) {
                return Err(vec![use_error(
                    "contract-use-path",
                    format!(
                        "`use {} from \"{}\"` leaves the app directory",
                        u.name, u.path
                    ),
                    u,
                )]);
            }
            if key.extension().and_then(|extension| extension.to_str()) != Some("contract") {
                return Err(vec![use_error(
                    "contract-use-path",
                    format!(
                        "`use {} from \"{}\"` resolves to a file that is not `.contract`",
                        u.name, u.path
                    ),
                    u,
                )]);
            }
            if self.active.contains(&key) {
                return Err(vec![use_error(
                    "contract-use-cycle",
                    format!(
                        "`use {} from \"{}\"` returns to a file already being loaded",
                        u.name, u.path
                    ),
                    u,
                )]);
            }
            let used = if let Some(cached) = self.cache.get(&key) {
                Rc::clone(cached)
            } else {
                let used_src = std::fs::read_to_string(&key).map_err(|e| {
                    use_error(
                        "contract-use-unreadable",
                        format!(
                            "`use {} from \"{}\"`: {}: {e}",
                            u.name,
                            u.path,
                            key.display()
                        ),
                        u,
                    )
                })?;
                let source_id = self.sources.paths.len() as u32;
                self.sources.paths.push(key.clone());
                self.active.push(key.clone());
                let used = self.load_source(&key, &used_src, source_id)?;
                self.active.pop();
                self.cache.insert(key.clone(), Rc::clone(&used));
                used
            };
            check_use_name(u, &used, &self.files)?;
            if merged.insert(key) {
                exports.merge(&used, &self.files, u)?;
            }
        }
        Ok(Rc::new(exports))
    }

    fn materialize(&mut self, exports: &Exports) -> File {
        if self.files.len() == 1 {
            let mut file = self.files.pop().unwrap();
            file.uses.clear();
            return file;
        }
        let mut names = NameSpans::default();
        for file in &mut self.files {
            names.names.extend(std::mem::take(&mut file.names.names));
            names
                .sources
                .extend(std::mem::take(&mut file.names.sources));
        }
        macro_rules! declarations {
            ($field:ident) => {
                take_declarations(&mut self.files, &exports.$field, |file| &mut file.$field)
            };
        }
        File {
            names,
            routes: self.files[0].routes.take(),
            tests: std::mem::take(&mut self.files[0].tests),
            uses: Vec::new(),
            fonts: declarations!(fonts),
            shapes: declarations!(shapes),
            styles: declarations!(styles),
            // CSS `@keyframes` are global by name: every loaded file's.
            keyframes: self
                .files
                .iter_mut()
                .flat_map(|file| std::mem::take(&mut file.keyframes))
                .collect(),
            fns: declarations!(fns),
            components: declarations!(components),
        }
    }
}

fn check_use_name(u: &UseDecl, exports: &Exports, files: &[File]) -> Result<(), CompileError> {
    if exports.contains(files, &u.name) {
        Ok(())
    } else {
        // Only the referenced file's resolved exports can satisfy this use.
        // Keep successful imports on the existing lookup path.
        let mut choices = Vec::new();
        macro_rules! choices {
            ($field:ident, $kind:literal) => {
                let mut seen = HashSet::new();
                let names: Vec<_> = exports
                    .$field
                    .iter()
                    .filter_map(|&(s, i)| {
                        let name = &files[s].$field[i].name;
                        seen.insert(name).then(|| format!("`{name}`"))
                    })
                    .collect();
                if !names.is_empty() {
                    choices.push(format!("{}: {}", $kind, names.join(", ")));
                }
            };
        }
        choices!(components, "components");
        choices!(shapes, "shapes");
        choices!(styles, "styles");
        choices!(fns, "functions");
        let available = if choices.is_empty() {
            "this file exports no components, shapes, styles, or functions".to_owned()
        } else {
            format!("available {}", choices.join("; "))
        };
        Err(use_error(
            "contract-use-unknown",
            format!(
                "`{}` declares no component, shape, style, or function `{}`; {available}",
                u.path, u.name
            ),
            u,
        ))
    }
}

fn use_error(id: &str, message: String, u: &UseDecl) -> CompileError {
    CompileError {
        pass: "use",
        id: id.into(),
        message,
        span: u.span,
        file: None,
        related: Box::new([]),
    }
}

fn validate_use_path(u: &UseDecl) -> Result<(), CompileError> {
    let Some(relative) = u.path.strip_prefix("./") else {
        return Err(use_error(
            "contract-use-path",
            format!(
                "`use {} from \"{}\"` needs a portable path beginning `./`",
                u.name, u.path
            ),
            u,
        ));
    };
    if relative.is_empty()
        || u.path.contains('\\')
        || relative
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(use_error(
            "contract-use-path",
            format!(
                "`use {} from \"{}\"` must stay below its file with no `..` segments",
                u.name, u.path
            ),
            u,
        ));
    }
    Ok(())
}

// (source identity, declaration index within that source's namespace).
type Declaration = (usize, usize);
struct Exports {
    fonts: Vec<Declaration>,
    shapes: Vec<Declaration>,
    styles: Vec<Declaration>,
    fns: Vec<Declaration>,
    components: Vec<Declaration>,
}
impl Exports {
    fn new(file: &File, source: usize) -> Self {
        let indices = |len| (0..len).map(|index| (source, index)).collect();
        Self {
            fonts: indices(file.fonts.len()),
            shapes: indices(file.shapes.len()),
            styles: indices(file.styles.len()),
            fns: indices(file.fns.len()),
            components: indices(file.components.len()),
        }
    }

    fn contains(&self, files: &[File], name: &str) -> bool {
        self.components
            .iter()
            .any(|&(s, i)| files[s].components[i].name == name)
            || self
                .shapes
                .iter()
                .any(|&(s, i)| files[s].shapes[i].name == name)
            || self
                .styles
                .iter()
                .any(|&(s, i)| files[s].styles[i].name == name)
            || self.fns.iter().any(|&(s, i)| files[s].fns[i].name == name)
    }

    fn merge(&mut self, from: &Self, files: &[File], u: &UseDecl) -> Result<(), CompileError> {
        macro_rules! merge {
            ($field:ident, $what:literal) => {
                merge_declarations(
                    files,
                    &mut self.$field,
                    &from.$field,
                    |file| &file.$field,
                    |decl| &decl.name,
                    $what,
                    u,
                )?;
            };
        }
        merge!(fonts, "font");
        merge!(shapes, "shape");
        merge!(styles, "style");
        merge!(fns, "fn");
        merge!(components, "component");
        Ok(())
    }
}

fn merge_declarations<T: Clone + PartialEq + VisitSpans>(
    files: &[File],
    into: &mut Vec<Declaration>,
    from: &[Declaration],
    declarations: fn(&File) -> &[T],
    name: fn(&T) -> &str,
    what: &str,
    u: &UseDecl,
) -> Result<(), CompileError> {
    if from.is_empty() {
        return Ok(());
    }
    let mut existing = HashMap::with_capacity(into.len() + from.len());
    for &(source, index) in into.iter() {
        existing
            .entry(name(&declarations(&files[source])[index]))
            .or_insert((source, index));
    }
    for &(source, index) in from {
        let incoming = &declarations(&files[source])[index];
        match existing.get(name(incoming)) {
            Some(&(s, i)) if (s, i) == (source, index)
                || same_declaration(&declarations(&files[s])[i], incoming) => {}
            Some(_) => return Err(use_error(
                "contract-use-duplicate",
                format!("`use {}` brings a {what} `{}` that this file already has, declared differently", u.name, name(incoming)),
                u,
            )),
            None => {
                into.push((source, index));
                existing.insert(name(incoming), (source, index));
            }
        }
    }
    Ok(())
}

// Each selected declaration appears once, so the final AST can take ownership
// from its source instead of cloning the same syntax at every import edge.
fn take_declarations<T>(
    files: &mut [File],
    selected: &[Declaration],
    declarations: fn(&mut File) -> &mut Vec<T>,
) -> Vec<T> {
    let mut slots: Vec<Vec<Option<T>>> = files
        .iter_mut()
        .map(|file| {
            std::mem::take(declarations(file))
                .into_iter()
                .map(Some)
                .collect()
        })
        .collect();
    selected
        .iter()
        .map(|&(source, index)| {
            slots[source][index]
                .take()
                .expect("unique declaration identity")
        })
        .collect()
}

fn same_declaration<T: Clone + PartialEq + VisitSpans>(a: &T, b: &T) -> bool {
    if a == b {
        return true;
    }
    let (mut a, mut b) = (a.clone(), b.clone());
    let mut original_position = |span: &mut contract_syntax::Span| {
        span.source_id = 0;
        span.end_col = 0;
    };
    a.visit_spans(&mut original_position);
    b.visit_spans(&mut original_position);
    a == b
}
