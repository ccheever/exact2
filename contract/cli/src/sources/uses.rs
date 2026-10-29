//! The `use` lines a file lacks (LLP 1091 D1): named by `contract-use-missing`
//! and written by `contract fmt --uses`. Since a file sees only what it names,
//! an app written before module scope (19 of the x2apps apps) must name every
//! declaration it reaches in another file. Which line brings a name is
//! mechanical: the specifier this file already uses for the declaring file,
//! else a package's or an `exact:` module's name another file uses for it
//! (it reads the same from anywhere), else the relative path to it inside
//! the same root (D9). What is not mechanical is left to the author, and
//! said: a name two files declare (which one is meant), a generated name
//! (`Card__ui` is `Card`, named), a file no specifier reaches.

use super::{digest, Loader, Origin, Refused, Sources};
use crate::CompileError;
use crate::RelatedLocation;
use contract_syntax::scope::{Elsewhere, Kind, Missing};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

/// One line a file needs: an existing `use` line rewritten with the names
/// it lacks (`line`, one-based), or a new one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseLine {
    /// The existing line it replaces; `None` for a line to add.
    pub line: Option<u32>,
    /// The line in full, `use A, B from "./f.contract"`.
    pub text: String,
}

/// What one file lacks, and the refusal that says so.
#[derive(Debug, Clone)]
pub(crate) struct UseFix {
    /// The file's unit.
    pub(crate) unit: usize,
    pub(crate) path: PathBuf,
    pub(crate) lines: Vec<UseLine>,
    /// Names no line can bring by rule, each with why.
    pub(crate) unresolved: Vec<String>,
    /// The units the lines name: the edges writing them adds.
    pub(crate) brings: Vec<usize>,
    pub(crate) error: CompileError,
}

impl Loader<'_> {
    /// The module name for data code alone (`"contract": false`, LLP
    /// 1133.011 D12) that a `use` would bring, if one of `found` is.
    pub(super) fn data_only(
        &self,
        unique: &[HashMap<(Kind, String), String>],
        found: &[(Kind, String)],
    ) -> Option<&contract_syntax::ModuleDecl> {
        let brought = |u: usize, m: &contract_syntax::ModuleDecl| {
            let to = unique[u].get(&(Kind::Call, m.name.clone()));
            to.is_some_and(|to| found.iter().any(|(k, f)| *k == Kind::Call && f == to))
        };
        (self.units.iter().enumerate())
            .flat_map(|(u, unit)| unit.file.modules.iter().map(move |m| (u, m)))
            .find(|&(u, m)| m.data_only && brought(u, m))
            .map(|(_, m)| m)
    }

    /// The fix for unit `index`, whose references `misses` name declarations
    /// of other files, in the order written.
    pub(super) fn use_fix(&self, index: usize, misses: Vec<(Missing, Elsewhere)>) -> UseFix {
        let unit = &self.units[index];
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        let mut unresolved: Vec<String> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut brings: Vec<usize> = Vec::new();
        for (m, e) in &misses {
            if seen.contains(&m.name) {
                continue;
            }
            seen.push(m.name.clone());
            let declaring: Vec<usize> = e
                .declaring
                .iter()
                .copied()
                .filter(|&u| u != index)
                .collect();
            if e.declared != m.name {
                unresolved.push(format!(
                    "`{}` is a name the compiler gave `{}` of `{}`: write `{}` and name it in a `use`",
                    m.name, e.declared, e.file, e.declared
                ));
            } else if declaring.len() > 1 {
                let files: Vec<String> = declaring
                    .iter()
                    .map(|&u| format!("`{}`", self.shown(u)))
                    .collect();
                unresolved.push(format!(
                    "`{}` is declared in {}: name it from the one meant",
                    m.name,
                    files.join(" and ")
                ));
            } else if let Some(clash) = self.clash(index, e.unit, &m.name) {
                unresolved.push(clash);
            } else if self.reaches(e.unit, index) {
                unresolved.push(format!(
                    "`{}` is declared in `{}`, which uses this file: naming it here would make a cycle; move it to a file both can use",
                    m.name, e.file
                ));
            } else if let Some(spec) = self.specifier(index, e.unit) {
                if !brings.contains(&e.unit) {
                    brings.push(e.unit);
                }
                match groups.iter_mut().find(|(s, _)| *s == spec) {
                    Some((_, names)) => names.push(m.name.clone()),
                    None => groups.push((spec, vec![m.name.clone()])),
                }
            } else {
                unresolved.push(format!(
                    "`{}` is declared in `{}`, which no `use` here can reach: name it through a file of this one's root that uses it",
                    m.name, e.file
                ));
            }
        }
        let lines: Vec<UseLine> = groups
            .into_iter()
            .map(
                |(spec, names)| match unit.file.uses.iter().find(|u| u.path == spec) {
                    Some(u) => {
                        let mut all: Vec<String> = u
                            .names
                            .iter()
                            .map(|n| match &n.alias {
                                Some(alias) => format!("{} as {alias}", n.name),
                                None => n.name.clone(),
                            })
                            .collect();
                        all.extend(names);
                        UseLine {
                            line: Some(u.span.line),
                            text: format!("use {} from \"{spec}\"", all.join(", ")),
                        }
                    }
                    None => UseLine {
                        line: None,
                        text: format!("use {} from \"{spec}\"", names.join(", ")),
                    },
                },
            )
            .collect();
        let (first, elsewhere) = &misses[0];
        let others = seen.len() - 1;
        let mut message = format!(
            "`{}` is a {} declared in `{}`, which this file does not name (LLP 1091 D1)",
            first.name,
            first.kind.what(),
            elsewhere.file
        );
        if others > 0 {
            message.push_str(&format!(
                ", and {others} more name{} here {} another file's",
                if others == 1 { "" } else { "s" },
                if others == 1 { "is" } else { "are" }
            ));
        }
        let root = self.sources.paths[0]
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "app.contract".into());
        match (lines.as_slice(), unresolved.as_slice()) {
            ([one], []) => {
                message.push_str(&match one.line {
                    Some(line) => format!(": change line {line} to `{}`", one.text),
                    None => format!(": add `{}`", one.text),
                });
                message.push_str(&format!("; `contract fmt --uses {root}` writes it"));
            }
            _ => {
                message.push_str(". This file needs:");
                for l in &lines {
                    message.push_str(&match l.line {
                        Some(line) => format!("\n  line {line}: {}", l.text),
                        None => format!("\n  a new line: {}", l.text),
                    });
                }
                for why in &unresolved {
                    message.push_str(&format!("\n  by hand: {why}"));
                }
                if !lines.is_empty() {
                    message.push_str(&format!("\n`contract fmt --uses {root}` writes the lines"));
                }
            }
        }
        // Each other name where it is first written.
        let mut related = Vec::new();
        let mut noted: Vec<&str> = vec![&first.name];
        for (m, e) in &misses {
            if noted.contains(&m.name.as_str()) {
                continue;
            }
            noted.push(&m.name);
            related.push(RelatedLocation {
                span: m.span,
                file: None,
                note: format!(
                    "`{}` is a {} declared in `{}`",
                    m.name,
                    m.kind.what(),
                    e.file
                ),
            });
        }
        UseFix {
            unit: index,
            path: self.sources.paths[index].clone(),
            lines,
            unresolved,
            brings,
            error: CompileError {
                pass: "use",
                id: "contract-use-missing".into(),
                message,
                span: first.span,
                file: None,
                related: related.into(),
            },
        }
    }

    /// A unit's path as a refusal shows it: from the app's root.
    pub(super) fn shown(&self, unit: usize) -> String {
        let path = &self.sources.paths[unit];
        path.strip_prefix(self.app_root)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    /// The specifier that brings unit `to` into unit `from`.
    /// Why naming `name` from unit `to` in unit `index` is not mechanical: a
    /// `use` brings every declaration of the name (a component and a style
    /// alike), and one of those may be a name this file declares or already
    /// brings from elsewhere.
    pub(super) fn clash(&self, index: usize, to: usize, name: &str) -> Option<String> {
        let theirs: Vec<Kind> = self.declared[to]
            .iter()
            .filter(|(_, n)| n == name)
            .map(|(k, _)| *k)
            .collect();
        let here = &self.units[index];
        for kind in theirs {
            if self.declared[index]
                .iter()
                .any(|(k, n)| *k == kind && n == name)
            {
                return Some(format!(
                    "naming `{name}` from `{}` would also bring its {} `{name}`, which this file declares: rename one",
                    self.shown(to),
                    kind.what()
                ));
            }
            // Or one this file already names from another file.
            let brought = here.file.uses.iter().zip(&here.targets).any(|(u, &t)| {
                t != to
                    && u.names.iter().any(|n| {
                        n.local() == name
                            && self.declared[t]
                                .iter()
                                .any(|(k, d)| *k == kind && *d == n.name)
                    })
            });
            if brought {
                return Some(format!(
                    "naming `{name}` from `{}` would also bring its {} `{name}`, which this file already names from another file: rename one with `as`",
                    self.shown(to),
                    kind.what()
                ));
            }
        }
        None
    }

    /// Whether unit `from` loads unit `to`, through any chain of `use` and
    /// `members` lines.
    fn reaches(&self, from: usize, to: usize) -> bool {
        let mut seen = vec![false; self.units.len()];
        let mut stack = vec![from];
        while let Some(u) = stack.pop() {
            if u == to {
                return true;
            }
            if !std::mem::replace(&mut seen[u], true) {
                stack.extend(self.units[u].loads());
                // And the edges the other fixes would add.
                stack.extend(
                    self.proposed
                        .iter()
                        .filter(|(f, _)| *f == u)
                        .map(|(_, t)| *t),
                );
            }
        }
        false
    }

    /// The specifier a `use` in unit `from` names unit `to` by — checked by
    /// resolving it from `from`, since a package's name need not lead to the
    /// same install from every directory.
    pub(super) fn specifier(&self, from: usize, to: usize) -> Option<String> {
        let spec = self.candidate_specifier(from, to)?;
        let dir = self.sources.paths[from]
            .canonicalize()
            .ok()?
            .parent()?
            .to_path_buf();
        let resolved = crate::resolve::resolve(
            &spec,
            &dir,
            &self.sources.origins[from],
            self.app_root,
            &mut Vec::new(),
        )
        .ok()?;
        let target = &self.sources.paths[to];
        let same =
            resolved.key == *target || target.canonicalize().is_ok_and(|t| t == resolved.key);
        same.then_some(spec)
    }

    fn candidate_specifier(&self, from: usize, to: usize) -> Option<String> {
        let named = |unit: usize| {
            let u = &self.units[unit];
            u.file
                .uses
                .iter()
                .zip(&u.targets)
                .filter(move |(_, &t)| t == to)
                .map(|(u, _)| u)
        };
        if let Some(u) = named(from).next() {
            return Some(u.path.clone());
        }
        // A package's or a built-in's name reads the same from any file.
        if let Some(u) = (0..self.units.len())
            .flat_map(named)
            .find(|u| !u.path.starts_with('.'))
        {
            return Some(u.path.clone());
        }
        // A relative path stays inside one root (D9).
        let origins = &self.sources.origins;
        let (a, b) = (
            origins[from].root(self.app_root)?,
            origins[to].root(self.app_root)?,
        );
        if a != b {
            return None;
        }
        let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| self.app_root.join(p));
        let from_dir = canonical(&self.sources.paths[from]).parent()?.to_path_buf();
        relative(&from_dir, &canonical(&self.sources.paths[to]))
    }
}

/// `to` as a `use` writes it from a file in `dir`: `./x.contract`,
/// `./lib/x.contract`, `../x.contract`.
fn relative(dir: &Path, to: &Path) -> Option<String> {
    let names = |p: &Path| -> Vec<String> {
        p.components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    let (dir, to) = (names(dir), names(to));
    let common = dir.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let rest = to[common..].join("/");
    if rest.is_empty() {
        return None;
    }
    Some(match dir.len() - common {
        0 => format!("./{rest}"),
        up => format!("{}{rest}", "../".repeat(up)),
    })
}

/// `src` with `lines` written: each existing `use` line replaced (its
/// indentation kept, and whatever follows its closing quote, a comment),
/// and the new ones after the last `use` line, else after the comments
/// that open the file.
pub(crate) fn apply(src: &str, lines: &[UseLine]) -> String {
    let mut out: Vec<String> = src.lines().map(str::to_owned).collect();
    for l in lines {
        let Some(n) = l.line.map(|n| n as usize) else {
            continue;
        };
        if let Some(at) = out.get_mut(n.wrapping_sub(1)) {
            let indent = &at[..at.len() - at.trim_start().len()];
            // What follows the path's closing quote: `from "` opens it.
            let tail = at
                .find("from \"")
                .and_then(|from| at[from + 6..].find('"').map(|q| from + 6 + q + 1))
                .map(|end| at[end..].to_owned())
                .unwrap_or_default();
            *at = format!("{indent}{}{tail}", l.text);
        }
    }
    let added: Vec<String> = lines
        .iter()
        .filter(|l| l.line.is_none())
        .map(|l| l.text.clone())
        .collect();
    if !added.is_empty() {
        let last_use = out.iter().rposition(|l| l.trim_start().starts_with("use "));
        let at = match last_use {
            Some(i) => i + 1,
            None => out
                .iter()
                .position(|l| !l.trim_start().starts_with("//"))
                .unwrap_or(out.len()),
        };
        for (k, text) in added.into_iter().enumerate() {
            out.insert(at + k, text);
        }
    }
    let mut text = out.join("\n");
    if src.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// A loader holding the root at `root_key` alone, `src` its text.
fn root_loader<'a>(app_root: &'a Path, root: &Path, root_key: &Path, src: &str) -> Loader<'a> {
    Loader {
        app_root,
        root_given: root.to_path_buf(),
        sources: Sources {
            paths: vec![root_key.to_path_buf()],
            origins: vec![Origin::App],
            digests: vec![digest(src)],
            homes: Vec::new(),
            texts: vec![src.into()],
            imports: Vec::new(),
            packages: Vec::new(),
            consulted: Vec::new(),
            motion: Vec::new(),
        },
        active: vec![root_key.to_path_buf()],
        units: Vec::new(),
        cache: HashMap::new(),
        fixes: Vec::new(),
        proposed: Vec::new(),
        declared: Vec::new(),
        motion: Vec::new(),
        refused: Refused::default(),
    }
}

/// The `use` lines each file of the program rooted at `path` lacks (LLP
/// 1091 D1), for `contract fmt --uses`; the loader's other refusals as
/// they are.
///
/// The root's neighbours no `use` reaches yet are read too, as candidates
/// that declare what the root lacks (euxy FRICTION 1: a root split into
/// seven files named none). A fix is kept only for a file the root reaches,
/// through its `use` lines or the ones the fixes add, so an unrelated
/// neighbour (another entry, a broken fixture) is never written; one whose
/// own refusal stops the program is left out and the rest tried again.
pub(crate) fn use_fixes(
    path: &Path,
    src: &str,
    app_root: &Path,
) -> Result<Vec<UseFix>, Vec<CompileError>> {
    let root_key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut left_out: HashSet<PathBuf> = HashSet::new();
    loop {
        let mut loader = root_loader(app_root, path, &root_key, src);
        let resolve = |sources: &Sources, all: Vec<CompileError>| {
            all.into_iter()
                .map(|e| sources.resolve(e))
                .collect::<Vec<_>>()
        };
        let loaded = loader.load_source(&root_key, src, 0);
        if !loader.refused.errors.is_empty() {
            return Err(std::mem::take(&mut loader.refused.errors));
        }
        if let Err(all) = loaded {
            return Err(resolve(&loader.sources, all));
        }
        let reached = loader.units.len();
        let beside = loader.load_beside(&root_key, &left_out);
        match loader.scope() {
            Ok(_) => return Ok(Vec::new()),
            Err(all) => {
                // A refusal in what only a neighbour brought: leave it out.
                let from = |e: &CompileError| {
                    let id = e.span.source_id as usize;
                    beside.iter().find(|(_, ids)| ids.contains(&id))
                };
                let culprits: Vec<_> = all.iter().filter_map(from).collect();
                if loader.fixes.is_empty() && !culprits.is_empty() {
                    left_out.extend(culprits.into_iter().map(|(key, _)| key.clone()));
                    continue;
                }
                if loader.fixes.is_empty() {
                    return Err(resolve(&loader.sources, all));
                }
                let reach = loader.reached_with_fixes(reached);
                let sources = &loader.sources;
                return Ok(std::mem::take(&mut loader.fixes)
                    .into_iter()
                    .filter(|fix| reach.contains(&fix.unit))
                    .map(|mut fix| {
                        // Only the app's own files are written: a package's,
                        // or one a link leads outside the app to, is said.
                        let own = sources.origins[fix.unit] == Origin::App;
                        if !own || !fix.path.starts_with(app_root) {
                            fix.lines.clear();
                        }
                        fix.error = sources.resolve(fix.error);
                        fix
                    })
                    .collect());
            }
        }
    }
}

impl Loader<'_> {
    /// Load each neighbour of the root that nothing reached and `left_out`
    /// does not name, as a unit no `use` names; for each, the source ids its
    /// load added. One that does not load is put back as it was.
    fn load_beside(
        &mut self,
        root_key: &Path,
        left_out: &HashSet<PathBuf>,
    ) -> Vec<(PathBuf, std::ops::Range<usize>)> {
        let mut beside = Vec::new();
        for key in unreached(root_key, |key| self.cache.contains_key(key)) {
            let Ok(text) = std::fs::read_to_string(&key) else {
                continue;
            };
            if left_out.contains(&key) || self.cache.contains_key(&key) {
                continue;
            }
            let (units, cache) = (self.units.len(), self.cache.clone());
            let (refused, hidden) = (self.refused.errors.len(), self.refused.hidden.len());
            self.sources.paths.push(key.clone());
            self.sources.origins.push(Origin::App);
            self.sources.digests.push(digest(&text));
            self.sources.texts.push(text.as_str().into());
            self.active.push(key.clone());
            let loaded = self.load_source(&key, &text, units as u32);
            self.active.pop();
            if loaded.is_err() || self.refused.errors.len() > refused || self.refused.whole {
                self.units.truncate(units);
                self.refused.errors.truncate(refused);
                self.refused.hidden.truncate(hidden);
                self.refused.whole = false;
                self.cache = cache;
                let s = &mut self.sources;
                s.paths.truncate(units);
                s.origins.truncate(units);
                s.digests.truncate(units);
                s.homes.truncate(units);
                s.texts.truncate(units);
            } else {
                self.cache.insert(key.clone(), units);
                beside.push((key, units..self.units.len()));
            }
        }
        beside
    }

    /// The units the root reaches through the `use` and `members` lines written and the
    /// ones the fixes write (`brings`, never an edge a fix refused, such as
    /// a cycle's); the first `reached` units are the root's own program.
    fn reached_with_fixes(&self, reached: usize) -> HashSet<usize> {
        let mut seen: HashSet<usize> = (0..reached).collect();
        let mut todo: Vec<usize> = seen.iter().copied().collect();
        while let Some(at) = todo.pop() {
            let written = self.units[at].loads();
            let fixes = self.fixes.iter().filter(|f| f.unit == at);
            let added = fixes.flat_map(|f| f.brings.iter().copied());
            for next in written.chain(added).collect::<Vec<_>>() {
                if seen.insert(next) {
                    todo.push(next);
                }
            }
        }
        seen
    }
}

/// The `.contract` files beside `root` (not its tests, nor `root`) that
/// `reached` says no `use` reaches, sorted; a link to a file outside
/// `root`'s directory is not one of them.
pub(crate) fn unreached(root: &Path, reached: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let dir = root.parent().unwrap_or(Path::new("."));
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.path().canonicalize().ok())
        .filter(|p| p.starts_with(dir) && p.is_file())
        .filter(|p| p.extension().is_some_and(|e| e == "contract"))
        .filter(|p| !p.to_string_lossy().ends_with(".test.contract"))
        .filter(|p| p != root && !reached(p))
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_path_is_written_as_a_use_writes_one() {
        let at = |d: &str, t: &str| relative(Path::new(d), Path::new(t));
        assert_eq!(
            at("/a/app", "/a/app/shapes.contract").as_deref(),
            Some("./shapes.contract")
        );
        assert_eq!(
            at("/a/app", "/a/app/lib/x.contract").as_deref(),
            Some("./lib/x.contract")
        );
        assert_eq!(
            at("/a/app/lib", "/a/app/x.contract").as_deref(),
            Some("../x.contract")
        );
        assert_eq!(
            at("/a/app/lib", "/a/app/ui/x.contract").as_deref(),
            Some("../ui/x.contract")
        );
    }

    #[test]
    fn lines_extend_their_use_or_follow_the_last() {
        let src = "// A note.\nuse A from \"./a.contract\" // the first\nuse B from \"./b.contract\"\ncomponent App\n";
        let out = apply(
            src,
            &[
                UseLine {
                    line: Some(2),
                    text: "use A, C from \"./a.contract\"".into(),
                },
                UseLine {
                    line: None,
                    text: "use D from \"./d.contract\"".into(),
                },
            ],
        );
        assert_eq!(out, "// A note.\nuse A, C from \"./a.contract\" // the first\nuse B from \"./b.contract\"\nuse D from \"./d.contract\"\ncomponent App\n");
        let out = apply(
            "// A note.\n// More.\ncomponent App\n",
            &[UseLine {
                line: None,
                text: "use D from \"./d.contract\"".into(),
            }],
        );
        assert_eq!(
            out,
            "// A note.\n// More.\nuse D from \"./d.contract\"\ncomponent App\n"
        );
    }
}
