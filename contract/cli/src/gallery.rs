//! `contract gallery <app> -o <dir>`: the design tool's variant gallery
//! (LLP 1139 D5, §9 Q2 and Q5). It writes a Contract app into `<dir>` that
//! places each exported library component the app uses once per cell:
//! the first values of its variant props, then each other value of each
//! alone, never their cross product (`cells`, as apps/library-gate's
//! `cellsFor`). Every other prop gets a placeholder by its type, for
//! showing only. The app's `app.json` names the app's own design, so the
//! gallery folds the app's current tokens and params. A cell the compiler
//! refuses says so in place ("can't place: …"); the rest still show.
//! Nothing here touches a build of the app: `<dir>` is the caller's (a
//! build cache), never the app's tree.

use crate::CompileError;
use contract_syntax::{Component, File, TypeExpr};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// A component's cells by its variant props, each `(prop, option)` in
/// order: the first values, then each other value of each prop alone.
pub fn cells(variants: &[(String, Vec<String>)]) -> Vec<Vec<(String, String)>> {
    let first: Vec<(String, String)> = variants
        .iter()
        .filter_map(|(p, o)| o.first().map(|v| (p.clone(), v.clone())))
        .collect();
    let mut out = vec![first.clone()];
    for (p, options) in variants {
        for o in options.iter().skip(1) {
            out.push(
                first
                    .iter()
                    .map(|(q, v)| (q.clone(), if q == p { o.clone() } else { v.clone() }))
                    .collect(),
            );
        }
    }
    out
}

/// One cell of the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// The component's name as the gallery's `use` brings it.
    pub component: String,
    /// Its library identity (`<package>/<file>#<Name>`).
    pub identity: String,
    /// Each variant prop's value in this cell.
    pub values: Vec<(String, String)>,
    /// The one prop this cell varies from the first values, if any.
    pub varied: Option<(String, String)>,
    /// Why it can't be placed, when it can't.
    pub refused: Option<String>,
}

/// What [`generate`] wrote.
#[derive(Debug)]
pub struct Gallery {
    /// `<dir>/app.contract`.
    pub source: PathBuf,
    /// Every cell, in the order shown.
    pub cells: Vec<Cell>,
}

/// One library component the app uses: how the gallery names and calls it.
struct Placed<'a> {
    name: String,
    identity: String,
    package: String,
    component: &'a Component,
    variants: Vec<(String, Vec<String>)>,
}

/// Writes the gallery of the app at `app_contract` into `out`, and checks
/// it with the compiler, refusing in place each cell it won't take.
pub fn generate(app_contract: &Path, out: &Path) -> Result<Gallery, String> {
    let src = std::fs::read_to_string(app_contract)
        .map_err(|e| format!("{}: {e}", app_contract.display()))?;
    let root = crate::app_root(app_contract).map_err(first)?;
    let (file, sources) = crate::sources::load(app_contract, &src, &root).map_err(first)?;
    let packages: Vec<PathBuf> = crate::sources::packages::packages(&sources)
        .0
        .into_iter()
        .filter_map(|p| p.root.canonicalize().ok())
        .collect();
    let placed = placed(&file, &sources, &packages);
    // An app with no design file of its own: the gallery has none either, the packages' still folded.
    let design = crate::design_path(&root.join("app.contract"));
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = out
        .canonicalize()
        .map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(
        out.join("app.json"),
        manifest(&root, design.as_deref(), &out),
    )
    .map_err(|e| format!("{}: {e}", out.display()))?;
    let mut cells: Vec<Cell> = placed
        .iter()
        .flat_map(|p| {
            let first = cells(&p.variants).into_iter().next().unwrap_or_default();
            cells(&p.variants).into_iter().map(move |values| {
                let varied = values
                    .iter()
                    .zip(&first)
                    .find(|(v, f)| v != f)
                    .map(|(v, _)| v.clone());
                Cell {
                    component: p.name.clone(),
                    identity: p.identity.clone(),
                    values,
                    varied,
                    refused: None,
                }
            })
        })
        .collect();
    // Placeholders first: a prop no placeholder fits (a recursive shape, a unit) refuses its cells.
    for c in &mut cells {
        let p = placed
            .iter()
            .find(|p| p.identity == c.identity)
            .expect("placed");
        if let Err(why) = args(p.component, &file, &c.values, &mut Vec::new()) {
            c.refused = Some(why);
        }
    }
    let path = out.join("app.contract");
    // Then the compiler: each refusal in a cell refuses that cell, and one on a shape's `use` (a
    // shape its package's entry doesn't export) each cell that builds it, until it takes the rest.
    for _ in 0..=cells.len() * 2 {
        let Source { text, lines, uses } = source(&placed, &file, &cells);
        std::fs::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))?;
        let errors = match crate::compile_path_all(&path, false) {
            Ok(_) => {
                return Ok(Gallery {
                    source: path,
                    cells,
                })
            }
            Err(errors) => errors,
        };
        let mut moved = false;
        for e in errors.iter().filter(|e| !e.is_warning()) {
            let here = e.file.as_deref().is_some_and(|f| f == path.as_path());
            let at = lines
                .iter()
                .position(|(from, to)| here && (*from..=*to).contains(&e.span.line));
            if let Some((_, shape)) = uses.iter().find(|(line, _)| here && *line == e.span.line) {
                for c in cells.iter_mut().filter(|c| c.refused.is_none()) {
                    let p = placed
                        .iter()
                        .find(|p| p.identity == c.identity)
                        .expect("placed");
                    if needed(p.component, &file).contains(shape) {
                        c.refused = Some(format!(
                            "its shape {shape} can't be named here ({})",
                            e.message.lines().next().unwrap_or("")
                        ));
                        moved = true;
                    }
                }
                continue;
            }
            match at {
                Some(i) if cells[i].refused.is_none() => {
                    cells[i].refused = Some(e.message.lines().next().unwrap_or("").to_owned());
                    moved = true;
                }
                Some(_) => {}
                None => return Err(format!("the gallery does not compile: {}", describe(e))),
            }
        }
        if !moved {
            return Err(format!(
                "the gallery does not compile: {}",
                errors.first().map(describe).unwrap_or_default()
            ));
        }
    }
    Err("the gallery does not compile".into())
}

fn first(errors: Vec<CompileError>) -> String {
    errors
        .first()
        .map(describe)
        .unwrap_or_else(|| "the app does not load".into())
}

fn describe(e: &CompileError) -> String {
    let at = e
        .file
        .as_deref()
        .map(|f| format!("{}:{}: ", f.display(), e.span.line))
        .unwrap_or_default();
    format!("{at}{}: {}", e.id, e.message)
}

/// The exported library components the app's own files `use`, each once,
/// in the order the app first names them.
fn placed<'a>(
    file: &'a File,
    sources: &crate::sources::Sources,
    packages: &[PathBuf],
) -> Vec<Placed<'a>> {
    let mut out: Vec<Placed> = Vec::new();
    for path in sources.paths() {
        let real = path.canonicalize().unwrap_or_else(|_| path.clone());
        if packages.iter().any(|p| real.starts_with(p)) {
            continue;
        }
        let Some(own) = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| contract_syntax::parse(&s).ok())
        else {
            continue;
        };
        for u in &own.uses {
            for n in &u.names {
                let of_package = |id: &str| {
                    id.strip_prefix(u.path.as_str())
                        .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('.'))
                };
                let found = file.components.iter().find(|c| {
                    c.params.identity.as_deref().is_some_and(|id| {
                        file.library.iter().any(|l| l == id)
                            && of_package(id)
                            && (id.rsplit('#').next() == Some(n.name.as_str()) || c.name == n.name)
                    })
                });
                let Some(c) = found else { continue };
                let identity = c.params.identity.clone().unwrap_or_default();
                if out.iter().any(|p| p.identity == identity) {
                    continue;
                }
                // Two packages' components of one name: the second renamed in the gallery.
                let taken = out
                    .iter()
                    .filter(|p| p.name == n.name || p.name.starts_with(&format!("{}_", n.name)))
                    .count();
                out.push(Placed {
                    name: if taken == 0 {
                        n.name.clone()
                    } else {
                        format!("{}_{}", n.name, taken + 1)
                    },
                    identity,
                    package: u.path.clone(),
                    component: c,
                    variants: contract_lower::anatomy::described(c, file).variants,
                });
            }
        }
    }
    out
}

/// The gallery's `app.json`: the app's id and name marked as its gallery,
/// and its design by a path from `out`.
fn manifest(root: &Path, design: Option<&Path>, out: &Path) -> String {
    let app: serde_json::Value = std::fs::read_to_string(root.join("app.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let id = app["app"]["id"].as_str().unwrap_or("app");
    let name = app["app"]["name"].as_str().unwrap_or("App");
    let mut json = serde_json::json!({
        "app": { "id": format!("{id}.gallery"), "name": format!("{name} gallery") },
    });
    if let Some(design) = design {
        let design = design
            .canonicalize()
            .unwrap_or_else(|_| design.to_path_buf());
        json["tamber"] = relative(out, &design).into();
    }
    format!("{}\n", serde_json::to_string_pretty(&json).expect("json"))
}

/// `to` written from directory `from`.
fn relative(from: &Path, to: &Path) -> String {
    let a: Vec<_> = from.components().collect();
    let b: Vec<_> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".into(); a.len() - common];
    parts.extend(
        b[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// A generated source, each cell's lines (first, last), and each shape's `use` line.
struct Source {
    text: String,
    lines: Vec<(u32, u32)>,
    uses: Vec<(u32, String)>,
}

/// The gallery's source.
fn source(placed: &[Placed], file: &File, cells: &[Cell]) -> Source {
    let mut s = String::new();
    s.push_str("// Generated by `contract gallery`: each exported library component the app uses, once per\n");
    s.push_str(
        "// cell (its variants' first values, then each other value alone), every other prop a\n",
    );
    s.push_str("// placeholder. Not the app's source: regenerated on every build, never edited.\n");
    let mut shapes: Vec<(String, String)> = Vec::new();
    for c in cells.iter().filter(|c| c.refused.is_none()) {
        let p = placed
            .iter()
            .find(|p| p.identity == c.identity)
            .expect("placed");
        for shape in needed(p.component, file) {
            if !shapes.iter().any(|(n, _)| *n == shape) {
                shapes.push((shape, p.package.clone()));
            }
        }
    }
    for p in placed {
        if p.name == p.identity.rsplit('#').next().unwrap_or("") {
            let _ = writeln!(s, "use {} from {}", p.name, literal(&p.package));
        } else {
            let _ = writeln!(
                s,
                "use {} as {} from {}",
                p.identity.rsplit('#').next().unwrap_or(""),
                p.name,
                literal(&p.package)
            );
        }
    }
    let mut uses = Vec::new();
    for (shape, package) in &shapes {
        uses.push((s.lines().count() as u32 + 1, shape.clone()));
        let _ = writeln!(s, "use {shape} from {}", literal(package));
    }
    s.push_str("\ncomponent Gallery\n  action noop\n  view\n    scroll testId=\"gallery\" flex=1\n      column gap=16 padding=16\n");
    let mut lines = Vec::new();
    let mut n = 0usize;
    let mut last = String::new();
    for c in cells {
        n = if c.identity == last { n + 1 } else { 0 };
        last = c.identity.clone();
        let from = s.lines().count() as u32 + 1;
        let id = format!("gallery-{}-{n}", c.component);
        let label = match &c.varied {
            Some((prop, value)) => format!("{} · {prop}={value}", c.component),
            None => format!("{} · first values", c.component),
        };
        let _ = writeln!(s, "        column gap=4 testId={}", literal(&id));
        let _ = writeln!(
            s,
            "          text {} font=\"-exact-footnote\"",
            literal(&label)
        );
        match &c.refused {
            Some(why) => {
                let _ = writeln!(
                    s,
                    "          text {} testId={}",
                    literal(&format!("can't place: {why}")),
                    literal(&format!("{id}-refused"))
                );
            }
            None => {
                let p = placed
                    .iter()
                    .find(|p| p.identity == c.identity)
                    .expect("placed");
                let args = args(p.component, file, &c.values, &mut Vec::new()).unwrap_or_default();
                let _ = writeln!(s, "          {}({})", c.component, args.join(", "));
                if p.component.slot {
                    let _ = writeln!(
                        s,
                        "            box testId={} width=24 height=24",
                        literal(&format!("{id}-slot"))
                    );
                }
            }
        }
        lines.push((from, s.lines().count() as u32));
    }
    Source {
        text: s,
        lines,
        uses,
    }
}

/// Each prop's argument: a variant's value in the cell, else a placeholder.
fn args(
    c: &Component,
    file: &File,
    values: &[(String, String)],
    seen: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    c.props
        .iter()
        .map(|p| {
            let value = match values.iter().find(|(q, _)| *q == p.name) {
                Some((_, v)) if matches!(&p.ty, Some(TypeExpr::Named(t, _)) if t == "bool") => {
                    v.clone()
                }
                Some((_, v)) => literal(v),
                None => match &p.ty {
                    Some(ty) => placeholder(ty, &p.name, file, seen)
                        .map_err(|why| format!("{} ({why})", p.name))?,
                    None => return Err(format!("{} (no type written)", p.name)),
                },
            };
            Ok(format!("{}={value}", p.name))
        })
        .collect()
}

/// A value of type `ty` for showing: a string its prop's name, a number 0,
/// a bool false, an enum its first value, an option none, a list empty, an
/// action the gallery's inert `noop`, a shape each field's own.
fn placeholder(
    ty: &TypeExpr,
    name: &str,
    file: &File,
    seen: &mut Vec<String>,
) -> Result<String, String> {
    Ok(match ty {
        TypeExpr::Enum(options, _) => literal(options.first().map(String::as_str).unwrap_or("")),
        TypeExpr::Option(..) => "none".into(),
        TypeExpr::List(..) => "[]".into(),
        TypeExpr::Named(t, _) => match t.as_str() {
            "string" => literal(name),
            "number" => "0".into(),
            "bool" => "false".into(),
            "action" => "noop".into(),
            "unit" => return Err("a unit".into()),
            shape => {
                if seen.iter().any(|s| s == shape) {
                    return Err(format!("the shape {shape} holds itself"));
                }
                let Some(decl) = file.shapes.iter().find(|s| s.name == shape) else {
                    return Err(format!("no shape {shape}"));
                };
                seen.push(shape.to_owned());
                let fields = decl
                    .fields
                    .iter()
                    .map(|f| {
                        placeholder(&f.ty, &f.name, file, seen).map(|v| format!("{}={v}", f.name))
                    })
                    .collect::<Result<Vec<_>, _>>();
                seen.pop();
                format!("{shape}({})", fields?.join(", "))
            }
        },
    })
}

/// The shapes a component's placeholders build, which the gallery must `use`.
fn needed(c: &Component, file: &File) -> Vec<String> {
    fn walk(ty: &TypeExpr, file: &File, out: &mut Vec<String>) {
        if let TypeExpr::Named(t, _) = ty {
            if matches!(t.as_str(), "string" | "number" | "bool" | "action" | "unit")
                || out.contains(t)
            {
                return;
            }
            out.push(t.clone());
            if let Some(decl) = file.shapes.iter().find(|s| s.name == *t) {
                for f in &decl.fields {
                    walk(&f.ty, file, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for p in &c.props {
        if let Some(ty) = &p.ty {
            walk(ty, file, &mut out);
        }
    }
    out
}

/// `s` as a Contract string literal.
fn literal(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(p: &str, o: &[&str]) -> (String, Vec<String>) {
        (p.into(), o.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn cells_are_the_first_values_then_each_other_value_alone() {
        let got = cells(&[
            v("tone", &["plain", "accent"]),
            v("compact", &["false", "true"]),
        ]);
        let want: Vec<Vec<(String, String)>> = [
            [("tone", "plain"), ("compact", "false")],
            [("tone", "accent"), ("compact", "false")],
            [("tone", "plain"), ("compact", "true")],
        ]
        .iter()
        .map(|c| {
            c.iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        })
        .collect();
        assert_eq!(got, want);
        // No variants: one cell, of no values.
        assert_eq!(cells(&[]), vec![Vec::<(String, String)>::new()]);
    }

    #[test]
    fn a_shape_that_holds_itself_refuses_the_cell_and_an_option_or_list_breaks_the_cycle() {
        let file = contract_syntax::parse(
            "shape Node\n  label: string\n  next: Node\n\nshape Chain\n  label: string\n  next: option<Chain>\n  rest: Chain[]\n\n\
             component Tree\n  props\n    root: Node\n  view\n    text \"t\"\n\n\
             component Line\n  props\n    head: Chain\n    nothing: unit\n  view\n    text \"l\"\n",
        )
        .unwrap();
        let tree = file.components.iter().find(|c| c.name == "Tree").unwrap();
        assert_eq!(
            args(tree, &file, &[], &mut Vec::new()),
            Err("root (the shape Node holds itself)".into())
        );
        let line = file.components.iter().find(|c| c.name == "Line").unwrap();
        // The option and the list end the walk; a unit has no placeholder.
        assert_eq!(
            args(line, &file, &[], &mut Vec::new()),
            Err("nothing (a unit)".into())
        );
        let only_head = Component {
            props: line.props[..1].to_vec(),
            ..line.clone()
        };
        assert_eq!(
            args(&only_head, &file, &[], &mut Vec::new()),
            Ok(vec![
                r#"head=Chain(label="label", next=none, rest=[])"#.to_string()
            ])
        );
    }

    #[test]
    fn a_path_from_the_gallery_to_the_design() {
        assert_eq!(
            relative(
                Path::new("/a/b/target/g"),
                Path::new("/a/b/apps/x/app.tamber")
            ),
            "../../apps/x/app.tamber"
        );
    }
}
