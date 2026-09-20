//! File loading and source identity shared by Contract compilation and navigation.
//! @ref LLP 1017.000 P8; LLP 1035.005 D2/D3.

use crate::CompileError;
use contract_syntax::{File, UseDecl, VisitSpans};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub(crate) struct Sources {
    paths: Vec<PathBuf>,
}
impl Sources {
    pub(crate) fn resolve(&self, mut error: CompileError) -> CompileError {
        error.file = Some(self.paths[error.span.source_id as usize].clone());
        error
    }
}

pub(crate) fn load(
    path: &Path,
    src: &str,
    app_root: &Path,
) -> Result<(File, Sources), CompileError> {
    let root_key = path.canonicalize().unwrap_or_else(|_| {
        path.file_name()
            .map(|name| app_root.join(name))
            .unwrap_or_else(|| app_root.to_path_buf())
    });
    let mut loader = Loader {
        app_root,
        sources: Sources {
            paths: vec![path.to_path_buf()],
        },
        active: vec![root_key],
        cache: HashMap::new(),
    };
    let file = loader
        .load_source(path, src, 0)
        .map_err(|e| loader.sources.resolve(e))?;
    Ok((file, loader.sources))
}

struct Loader<'a> {
    app_root: &'a Path,
    sources: Sources,
    // Keep one unmerged AST per file; caching transitive merged trees would
    // retain quadratically many declarations along a long import chain.
    // Only the active stack decides cycles, even when parsing is cached.
    active: Vec<PathBuf>,
    cache: HashMap<PathBuf, File>,
}
impl Loader<'_> {
    fn load_source(
        &mut self,
        path: &Path,
        src: &str,
        source_id: u32,
    ) -> Result<File, CompileError> {
        let file = contract_syntax::parse_source(src, source_id)?;
        self.load_file(path, file)
    }

    fn load_file(&mut self, path: &Path, mut file: File) -> Result<File, CompileError> {
        contract_analyze::check_routes_root(&file, self.active.len() == 1)?;
        let uses = std::mem::take(&mut file.uses);
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
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
                return Err(use_error(
                    "contract-use-path",
                    format!(
                        "`use {} from \"{}\"` leaves the app directory",
                        u.name, u.path
                    ),
                    u,
                ));
            }
            if key.extension().and_then(|extension| extension.to_str()) != Some("contract") {
                return Err(use_error(
                    "contract-use-path",
                    format!(
                        "`use {} from \"{}\"` resolves to a file that is not `.contract`",
                        u.name, u.path
                    ),
                    u,
                ));
            }
            if self.active.contains(&key) {
                return Err(use_error(
                    "contract-use-cycle",
                    format!(
                        "`use {} from \"{}\"` returns to a file already being loaded",
                        u.name, u.path
                    ),
                    u,
                ));
            }
            let used = if let Some(cached) = self.cache.get(&key) {
                cached.clone()
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
                let used = contract_syntax::parse_source(&used_src, source_id)?;
                self.cache.insert(key.clone(), used.clone());
                used
            };
            self.active.push(key.clone());
            let used = self.load_file(&key, used)?;
            self.active.pop();
            let known = used.components.iter().any(|c| c.name == u.name)
                || used.shapes.iter().any(|s| s.name == u.name)
                || used.styles.iter().any(|s| s.name == u.name)
                || used.fns.iter().any(|f| f.name == u.name);
            if !known {
                return Err(use_error(
                    "contract-use-unknown",
                    format!(
                        "`{}` declares no component, shape, style, or function `{}`",
                        u.path, u.name
                    ),
                    u,
                ));
            }
            merge(&mut file, used, u)?;
        }
        Ok(file)
    }
}

fn use_error(id: &str, message: String, u: &UseDecl) -> CompileError {
    CompileError {
        pass: "use",
        id: id.into(),
        message,
        span: u.span,
        file: None,
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

fn merge(into: &mut File, from: File, u: &UseDecl) -> Result<(), CompileError> {
    let dup = |what: &str, name: &str| {
        use_error(
            "contract-use-duplicate",
            format!("`use {}` brings a {what} `{name}` that this file already has, declared differently", u.name),
            u,
        )
    };
    for f in from.fonts {
        match into.fonts.iter().find(|x| x.name == f.name) {
            Some(x) if same_declaration(x, &f) => {}
            Some(_) => return Err(dup("font", &f.name)),
            None => into.fonts.push(f),
        }
    }
    for s in from.shapes {
        match into.shapes.iter().find(|x| x.name == s.name) {
            Some(x) if same_declaration(x, &s) => {}
            Some(_) => return Err(dup("shape", &s.name)),
            None => into.shapes.push(s),
        }
    }
    for s in from.styles {
        match into.styles.iter().find(|x| x.name == s.name) {
            Some(x) if same_declaration(x, &s) => {}
            Some(_) => return Err(dup("style", &s.name)),
            None => into.styles.push(s),
        }
    }
    for f in from.fns {
        match into.fns.iter().find(|x| x.name == f.name) {
            Some(x) if same_declaration(x, &f) => {}
            Some(_) => return Err(dup("fn", &f.name)),
            None => into.fns.push(f),
        }
    }
    for c in from.components {
        match into.components.iter().find(|x| x.name == c.name) {
            Some(x) if same_declaration(x, &c) => {}
            Some(_) => return Err(dup("component", &c.name)),
            None => into.components.push(c),
        }
    }
    Ok(())
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
