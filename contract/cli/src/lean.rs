//! `contract lean`: the program as a term of the Lean semantics
//! (`semantics/Contract/Syntax.lean`).
//!
//! The backend runs after the whole compiler accepts the program — a source
//! the plan backend refuses is refused here too — and emits the expanded
//! root (`contract_syntax::expand`), the file's shapes and `fn`s, with the
//! checker's types: a deep embedding whose meaning is
//! `semantics/Contract/Runtime.lean`. Names stay names; the semantics
//! scopes them itself.

use crate::CompileError;
use contract_syntax::{
    BinOp, Component, Expr, File, Node, Span, Stmt, TaskKind, TemplatePart, UnOp,
};
use contract_types::{Checked, Ty};
use std::fmt::Write as _;
use std::path::Path;

mod components;
pub use components::{lean_components, lean_components_path};

/// Compile one source text and emit `def <name> : Contract.Program`.
pub fn lean(src: &str, name: &str) -> Result<String, CompileError> {
    lean_source(src, name, true)
}

/// [`lean`] for a file, resolving its `use`s as `compile_path` does.
pub fn lean_path(path: &Path, name: &str) -> Result<String, CompileError> {
    lean_file(path, name, true)
}

/// [`lean`] of the expansion the plan compiler makes, its uses' arguments
/// not ascribed (`difftest expansion` compares it with the Lean expander).
pub fn lean_plain(src: &str, name: &str) -> Result<String, CompileError> {
    lean_source(src, name, false)
}

/// [`lean_plain`] for a file.
pub fn lean_path_plain(path: &Path, name: &str) -> Result<String, CompileError> {
    lean_file(path, name, false)
}

fn lean_source(src: &str, name: &str, ascribed: bool) -> Result<String, CompileError> {
    // The plan backend first: what it refuses is not a program.
    crate::compile(src)?;
    let mut file = contract_syntax::parse(src)?;
    contract_syntax::resolve_clock_timelines(&mut file)?;
    emit_file(&file, name, None, ascribed)
}

fn lean_file(path: &Path, name: &str, ascribed: bool) -> Result<String, CompileError> {
    let src = read(path)?;
    crate::compile_path_source(path, &src)?;
    emit_file(&load(path, &src)?, name, strings_beside(path)?, ascribed)
}

/// The strings tables beside the root source, as the compile reads them.
fn strings_beside(
    path: &Path,
) -> Result<Option<std::sync::Arc<contract_types::strings::Strings>>, CompileError> {
    crate::strings::load(&app_root(path)?, path).map_err(|mut all| all.swap_remove(0))
}

/// The file at `path` with its `use`s merged in, as `compile_path` loads it.
fn load(path: &Path, src: &str) -> Result<File, CompileError> {
    let (file, _) =
        crate::sources::load(path, src, &app_root(path)?).map_err(|mut all| all.swap_remove(0))?;
    Ok(file)
}

/// The directory of the root source, canonical: where its `use`s and
/// strings tables are found.
fn app_root(path: &Path) -> Result<std::path::PathBuf, CompileError> {
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    root.canonicalize().map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{}: {e}", root.display()),
        span: Span::default(),
        file: Some(path.into()),
        related: Box::new([]),
    })
}

/// Read a source file.
fn read(path: &Path) -> Result<String, CompileError> {
    std::fs::read_to_string(path).map_err(|e| CompileError {
        pass: "io",
        id: "contract-unreadable".into(),
        message: format!("{}: {e}", path.display()),
        span: Span::default(),
        file: Some(path.into()),
        related: Box::new([]),
    })
}

fn emit_file(
    file: &File,
    name: &str,
    strings: Option<std::sync::Arc<contract_types::strings::Strings>>,
    ascribed: bool,
) -> Result<String, CompileError> {
    let checked = contract_types::check_all(file, false, contract_lower::tags::style, strings)
        .map_err(|mut all| CompileError::from(all.swap_remove(0)))?;
    if ascribed {
        emit_checked(&checked, name)
    } else {
        let mut e = Emitter {
            out: String::new(),
            checked: &checked,
            root: checked.expanded.root.clone(),
        };
        e.program(name)?;
        Ok(e.out)
    }
}

/// Emit a checked expansion as `def <name> : Contract.Program`. The
/// differential type test (`difftest types`) also calls this with an
/// expansion the checker refused, typed by the program it was mutated
/// from, to give the Lean checker the mutant to judge.
pub fn emit_checked(checked: &Checked, name: &str) -> Result<String, CompileError> {
    // The same expansion, every prop's and inject's argument ascribed its
    // declared type: the checker's judgment of each use rides along.
    let (typed, _) = contract_syntax::expand_typed(checked.file);
    let mut e = Emitter {
        out: String::new(),
        checked,
        root: typed.root,
    };
    e.program(name)?;
    Ok(e.out)
}

struct Emitter<'a> {
    out: String,
    checked: &'a Checked<'a>,
    /// The expanded root with its uses' arguments ascribed
    /// (`contract_syntax::expand_typed`).
    root: Component,
}

/// The name of the resolved locale's slot, as the plan names it.
const LOCALE: &str = "#locale";

fn refuse(message: impl Into<String>, span: Span) -> CompileError {
    CompileError {
        pass: "lean",
        id: "lean-unexpanded".into(),
        message: message.into(),
        span,
        file: None,
        related: Box::new([]),
    }
}

/// A Lean string literal: printable text as itself, the rest escaped.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A type as a `Contract.Ty`.
pub fn ty(t: &Ty) -> String {
    match t {
        Ty::Number => ".number".into(),
        Ty::String => ".string".into(),
        Ty::Bool => ".bool".into(),
        Ty::Unit => ".unit".into(),
        Ty::Option(t) => format!("(.option {})", ty(t)),
        Ty::List(t) => format!("(.list {})", ty(t)),
        Ty::Record(s) => format!("(.record {})", string(s)),
        Ty::Action(_) | Ty::Unknown => ".unknown".into(),
    }
}

/// Every `"…"` that follows `prefix` in `text` (an embedding's names, which
/// are identifiers: no escaped quote inside, and a string literal's own
/// quotes are escaped, so it never matches).
fn quoted_after(text: &str, prefix: &str) -> Vec<String> {
    text.match_indices(prefix)
        .filter_map(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            rest.find('"').map(|end| rest[..end].to_string())
        })
        .collect()
}

/// The shapes a type names.
fn record_names(t: &Ty, out: &mut Vec<String>) {
    match t {
        Ty::Record(s) => out.push(s.clone()),
        Ty::Option(t) | Ty::List(t) => record_names(t, out),
        Ty::Action(ts) => ts.iter().for_each(|t| record_names(t, out)),
        _ => {}
    }
}

fn list<T>(
    items: &[T],
    mut f: impl FnMut(&T) -> Result<String, CompileError>,
) -> Result<String, CompileError> {
    let parts = items.iter().map(&mut f).collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", parts.join(", ")))
}

impl Emitter<'_> {
    fn root(&self) -> &Component {
        &self.root
    }

    /// Whether `name(args)` is the strings call `t("key", …)`.
    fn is_text_call(&self, name: &str, args: &[Expr]) -> bool {
        name == "t"
            && self.checked.types.shapes.strings.is_some()
            && matches!(args.first(), Some(Expr::Str(..)))
            && !self.checked.file.fns.iter().any(|f| f.name == "t")
    }

    fn program(&mut self, name: &str) -> Result<(), CompileError> {
        let root = self.root();
        let types = &self.checked.types;
        let ct = &types.components[0];
        let mut o = String::new();
        let _ = writeln!(o, "  fns := {},", self.fns_list()?);
        let owners = &self.checked.expanded.owners;
        let mut states = Vec::new();
        // The resolved locale's slot (LLP 1060 D4), when the app has strings
        // tables: initialized before every other, to the base locale; after
        // the router's, which the expander puts first and boot fills from
        // the launch, not an initializer.
        let locale_at = usize::from(self.checked.file.routes.is_some());
        for (i, s) in root.states.iter().enumerate() {
            if i == locale_at {
                states.extend(self.locale_state());
            }
            let (owner, late) = match owners.get(i).copied() {
                Some(contract_syntax::Owner::Arm { tag, arm }) => {
                    (format!(".some ({tag}, {arm})"), false)
                }
                Some(contract_syntax::Owner::Instance) => (".none".into(), true),
                Some(contract_syntax::Owner::Root) | None => (".none".into(), false),
            };
            states.push(format!(
                "{{ name := {}, ty := {}, init := {}, owner := {owner}, late := {late} }}",
                string(&s.name),
                ty(&ct.slots[i]),
                self.expr(&s.expr)?
            ));
        }
        if root.states.len() <= locale_at {
            states.extend(self.locale_state());
        }
        let _ = writeln!(o, "  states := [{}],", states.join(",\n    "));
        let _ = writeln!(o, "  derives := {},", self.derives_list(root, ct)?);
        let _ = writeln!(o, "  resources := {},", self.resources_list(root, ct)?);
        let _ = writeln!(o, "  mutations := {},", self.mutations_list(root, ct));
        let _ = writeln!(o, "  actions := {},", self.actions_list(&root.actions, ct)?);
        let _ = writeln!(o, "  tasks := {},", self.tasks_list(root)?);
        let _ = writeln!(o, "  view := {},", self.nodes(&root.view)?);
        let _ = writeln!(o, "  routes := {},", self.routes_list());
        let _ = writeln!(o, "  router := {},", self.router());
        let _ = writeln!(o, "  strings := {},", self.strings_list());
        let _ = writeln!(o, "  locale := {},", self.locale());
        let _ = writeln!(o, "  sources := {}", self.sources_list(ct));
        o.push_str("}\n");
        let _ = writeln!(self.out, "def {name} : Contract.Program := {{");
        let _ = writeln!(self.out, "  shapes := {},", self.shapes_list(&o));
        self.out.push_str(&o);
        Ok(())
    }

    /// The resolved locale's slot, when the app has strings tables.
    fn locale_state(&self) -> Option<String> {
        let st = self.checked.types.shapes.strings.as_deref()?;
        Some(format!(
            "{{ name := {}, ty := .string, init := (.str {}), owner := .none, late := false }}",
            string(LOCALE),
            string(&st.base)
        ))
    }

    /// `Program.locale`: the resolved locale's slot, when there are tables.
    fn locale(&self) -> String {
        match self.checked.types.shapes.strings {
            Some(_) => format!(".some {}", string(LOCALE)),
            None => ".none".into(),
        }
    }

    /// The strings tables, the base first and the rest in name order, as
    /// the plan bakes them.
    fn strings_list(&self) -> String {
        let tables: Vec<String> = self
            .checked
            .types
            .shapes
            .strings
            .as_deref()
            .map(|st| {
                let base = st.tables.get_key_value(&st.base);
                let others = st.tables.iter().filter(|(l, _)| **l != st.base);
                base.into_iter()
                    .chain(others)
                    .map(|(locale, table)| {
                        let texts: Vec<String> = table
                            .iter()
                            .map(|(k, t)| format!("({}, {})", string(k), string(t)))
                            .collect();
                        format!("({}, [{}])", string(locale), texts.join(", "))
                    })
                    .collect()
            })
            .unwrap_or_default();
        format!("[{}]", tables.join(",\n    "))
    }

    /// Each source's one signature (`type-source-signature`), but those the
    /// runner answers itself, whose readers each have their own.
    fn sources_list(&self, ct: &contract_types::ComponentTypes) -> String {
        let sources: Vec<String> = ct
            .sources
            .iter()
            .filter(|(s, _)| !exact_plan::runner_owned_source(s))
            .map(|(s, (params, result))| {
                let ps: Vec<String> = params.iter().map(ty).collect();
                format!("({}, [{}], {})", string(s), ps.join(", "), ty(result))
            })
            .collect();
        format!("[{}]", sources.join(",\n    "))
    }

    /// The shapes `body` (the rest of the embedding) reaches: every shape
    /// it names (a type `(.record "S")`, a record built `(.record "S" …)`),
    /// those of the roster entries it calls (`Router`, `Entry`, …), the
    /// router's four when there are routes, and the shapes their fields
    /// name, in the checker's order. Not every shape the checker knows: a
    /// compiler shape the program never reaches (an event's, say) would
    /// make every embedding stale when the compiler gains one.
    fn shapes_list(&self, body: &str) -> String {
        let map = &self.checked.types.shapes.map;
        let mut seen = std::collections::BTreeSet::new();
        let mut todo: Vec<String> = quoted_after(body, ".record \"");
        for name in quoted_after(body, ".call \"") {
            if name == "failure" {
                // `failure(x)` answers the compiler's `Failure` (LLP 1109 D3).
                todo.push("Failure".into());
            }
            if let Some(f) = exact_plan::Stdlib::from_name(&name) {
                for spec in f.params().iter().chain([&f.returns()]) {
                    record_names(&Ty::from_roster(spec), &mut todo);
                }
            }
        }
        if self.checked.file.routes.is_some() {
            todo.extend(["Router", "Entry", "Tab", "Params"].map(String::from));
        }
        while let Some(s) = todo.pop() {
            if let Some(fields) = map.get(&s) {
                if seen.insert(s) {
                    for (_, t) in fields {
                        record_names(t, &mut todo);
                    }
                }
            }
        }
        let shape_rows: Vec<String> = map
            .iter()
            .filter(|(s, _)| seen.contains(*s))
            .map(|(s, fields)| {
                let fs: Vec<String> = fields
                    .iter()
                    .map(|(f, t)| format!("{{ name := {}, ty := {} }}", string(f), ty(t)))
                    .collect();
                format!("{{ name := {}, fields := [{}] }}", string(s), fs.join(", "))
            })
            .collect();
        format!("[{}]", shape_rows.join(",\n    "))
    }

    fn fns_list(&self) -> Result<String, CompileError> {
        let shapes = &self.checked.types.shapes;
        let mut fns = Vec::new();
        for f in &self.checked.file.fns {
            let (params, ret) = &shapes.fns[&f.name];
            let ps: Vec<String> = f
                .params
                .iter()
                .zip(params)
                .map(|(p, t)| format!("({}, {})", string(&p.name), ty(t)))
                .collect();
            fns.push(format!(
                "{{ name := {}, params := [{}], ret := {}, body := {} }}",
                string(&f.name),
                ps.join(", "),
                ty(ret),
                self.expr(&f.body)?
            ));
        }
        Ok(format!("[{}]", fns.join(",\n    ")))
    }

    fn derives_list(
        &self,
        c: &Component,
        ct: &contract_types::ComponentTypes,
    ) -> Result<String, CompileError> {
        let mut derives = Vec::new();
        for (i, d) in c.derives.iter().enumerate() {
            derives.push(format!(
                "{{ name := {}, ty := {}, body := {} }}",
                string(&d.name),
                ct.derives.get(i).map_or(".unknown".into(), ty),
                self.expr(&d.expr)?
            ));
        }
        Ok(format!("[{}]", derives.join(",\n    ")))
    }

    fn resources_list(
        &self,
        root: &Component,
        ct: &contract_types::ComponentTypes,
    ) -> Result<String, CompileError> {
        let mut resources = Vec::new();
        for (i, r) in root.resources.iter().enumerate() {
            resources.push(format!(
                "{{ name := {}, ty := {}, source := {}, args := {} }}",
                string(&r.name),
                ty(&ct.resources[i]),
                string(&r.source),
                list(&r.args, |a| self.expr(a))?
            ));
        }
        // A declared placeholder is a resource row of its own, after every
        // authored row, its arguments read in no scope (LLP 1048.003 D6);
        // `empty(…)` rides its resource's row and adds none.
        for (i, r) in root.resources.iter().enumerate() {
            let Some(p) = &r.placeholder else { continue };
            if p.source == contract_types::placeholder::EMPTY {
                continue;
            }
            resources.push(format!(
                "{{ name := {}, ty := {}, source := {}, args := {} }}",
                string(&format!("{}#else", r.name)),
                ty(&ct.resources[i]),
                string(&p.source),
                list(&p.args, |a| self.expr(a))?
            ));
        }
        Ok(format!("[{}]", resources.join(",\n    ")))
    }

    fn mutations_list(&self, root: &Component, ct: &contract_types::ComponentTypes) -> String {
        let mutations: Vec<String> = root
            .mutations
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let refreshes: Vec<String> = m.refreshes.iter().map(|(r, _)| string(r)).collect();
                let then = match &m.then {
                    Some((a, _)) => format!(".some {}", string(a)),
                    None => ".none".into(),
                };
                // `queue` (LLP 1092 D1) only where declared: the embedding
                // of every other program is as it was.
                let queue = if m.queue { ", queue := true" } else { "" };
                format!(
                    "{{ name := {}, ty := {}, refreshes := [{}], andThen := {then}{queue} }}",
                    string(&m.name),
                    ty(&ct.mutations[i]),
                    refreshes.join(", ")
                )
            })
            .collect();
        format!("[{}]", mutations.join(",\n    "))
    }

    /// Actions with their parameters' checked types (`ct.actions`, by
    /// position; `?` past its end).
    fn actions_list(
        &self,
        actions: &[contract_syntax::Action],
        ct: &contract_types::ComponentTypes,
    ) -> Result<String, CompileError> {
        let mut out = Vec::new();
        for (i, a) in actions.iter().enumerate() {
            let tys = ct.actions.get(i);
            let ps: Vec<String> = a
                .params
                .iter()
                .enumerate()
                .map(|(j, p)| {
                    let t = tys.and_then(|t| t.get(j)).map_or(".unknown".into(), ty);
                    format!("({}, {t})", string(&p.name))
                })
                .collect();
            out.push(format!(
                "{{ name := {}, params := [{}], body := {} }}",
                string(&a.name),
                ps.join(", "),
                self.stmts(&a.body)?
            ));
        }
        Ok(format!("[{}]", out.join(",\n    ")))
    }

    fn tasks_list(&self, root: &Component) -> Result<String, CompileError> {
        let mut tasks = Vec::new();
        for t in &root.tasks {
            let kind = match t.kind {
                TaskKind::Every => ".every",
                TaskKind::After => ".after",
                TaskKind::Frame => ".frame",
            };
            // A gated task's gate and key (LLP 1092 D12), as D9 lowers them.
            let mut gate = String::new();
            if let Some(g) = &t.gate {
                gate += &format!(", gate := .some ({})", self.expr(g)?);
            }
            if let Some(k) = &t.key {
                gate += &format!(", key := .some ({})", self.expr(k)?);
            }
            tasks.push(format!(
                "{{ name := {}, kind := {kind}, ms := {}, action := {}{gate} }}",
                string(&t.name),
                self.expr(&t.timer.0)?,
                string(&t.timer.1)
            ));
        }
        Ok(format!("[{}]", tasks.join(",\n    ")))
    }

    /// The checked route table (LLP 1038 D2).
    fn routes_list(&self) -> String {
        let routes: Vec<String> = self
            .checked
            .types
            .shapes
            .routes
            .iter()
            .flat_map(|t| &t.routes)
            .map(|r| {
                format!(
                    "{{ name := {}, pattern := {}, parent := {}, tab := {}, notfound := {} }}",
                    string(&r.name),
                    string(&r.pattern),
                    r.parent.map_or(".none".into(), |p| format!(".some {p}")),
                    r.tab,
                    r.notfound
                )
            })
            .collect();
        format!("[{}]", routes.join(",\n    "))
    }

    /// The slot `routes` names.
    fn router(&self) -> String {
        match &self.checked.file.routes {
            Some(r) => format!(".some {}", string(&r.slot)),
            None => ".none".into(),
        }
    }

    fn expr(&self, e: &Expr) -> Result<String, CompileError> {
        let shapes = &self.checked.types.shapes;
        Ok(match e {
            Expr::Number(n, _) => format!("(.num 0x{:016x})", n.to_bits()),
            Expr::Str(s, _) => format!("(.str {})", string(s)),
            Expr::Bool(b, _) => format!("(.bool {b})"),
            Expr::None(_) => ".none".into(),
            Expr::List(items, _) => {
                let items = items
                    .iter()
                    .map(|x| self.expr(x))
                    .collect::<Result<Vec<_>, _>>()?;
                format!("(.list [{}])", items.join(", "))
            }
            Expr::Some(inner, _) => format!("(.some {})", self.expr(inner)?),
            Expr::Template(parts, _) => {
                let ps = parts
                    .iter()
                    .map(|p| match p {
                        TemplatePart::Text(t) => Ok(format!("(.str {})", string(t))),
                        TemplatePart::Expr(x) => self.expr(x),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                format!("(.template [{}])", ps.join(", "))
            }
            Expr::Ident(name, _) => format!("(.var {})", string(name)),
            Expr::Member(obj, field, _) => {
                format!("(.member {} {})", self.expr(obj)?, string(field))
            }
            Expr::Call(name, args, _) if contract_types::records::is_record_call(name, shapes) => {
                let base = match contract_types::records::base(args) {
                    Some(b) => format!("(.some {})", self.expr(b)?),
                    None => ".none".into(),
                };
                let mut fields = Vec::new();
                for a in args {
                    if let Expr::NamedArg(n, v, _) = a {
                        fields.push(format!("({}, {})", string(n), self.expr(v)?));
                    }
                }
                format!("(.record {} {base} [{}])", string(name), fields.join(", "))
            }
            Expr::Call(name, args, _) if self.is_text_call(name, args) => {
                // `t("key", name=value, …)` as the compiler lowers it: the
                // locale slot, the key, then each placeholder's name and its
                // value as `toString` prints it.
                let mut parts = vec![format!("(.var {})", string(LOCALE)), self.expr(&args[0])?];
                for a in &args[1..] {
                    let Expr::NamedArg(n, v, _) = a else {
                        return Err(refuse("a placeholder is filled by name", a.span()));
                    };
                    parts.push(format!("(.str {})", string(n)));
                    parts.push(format!("(.call \"toString\" [{}])", self.expr(v)?));
                }
                format!("(.call \"t\" [{}])", parts.join(", "))
            }
            // `pending(x)`/`failed(x)` name a resource or a mutation: a
            // prop's ascription around the name is not an expression here.
            Expr::Call(name, args, _)
                if matches!(name.as_str(), "pending" | "failed" | "failure")
                    && matches!(args.as_slice(), [Expr::Typed(x, _, _)] if matches!(**x, Expr::Ident(..))) =>
            {
                let [Expr::Typed(x, _, _)] = args.as_slice() else {
                    unreachable!("matched above")
                };
                format!("(.call {} [{}])", string(name), self.expr(x)?)
            }
            Expr::Call(name, args, _) => {
                let mut parts = args
                    .iter()
                    .map(|a| self.expr(a))
                    .collect::<Result<Vec<_>, _>>()?;
                // A roster call may leave its trailing optional parameters
                // out (LLP 1088 D2); the semantics takes the full arity, so
                // their defaults are written here as lowering writes them.
                // No `fn` takes a roster name, and the semantics evaluates
                // no action in an expression.
                if let Some(f) =
                    exact_plan::Stdlib::from_name(name).filter(|_| !shapes.fns.contains_key(name))
                {
                    for d in f.omitted(args.len()) {
                        parts.push(format!("(.num 0x{:016x})", d.to_bits()));
                    }
                }
                format!("(.call {} [{}])", string(name), parts.join(", "))
            }
            Expr::NamedArg(n, v, _) => format!("(.named {} {})", string(n), self.expr(v)?),
            Expr::Typed(e, t, _) => {
                let resolved = self.checked.types.shapes.resolve(t).ok();
                // A literal at its own type is the literal: the ascription
                // changes nothing, and the compiler reads literals as
                // literals (a template's text, an attribute's value).
                let literal = match **e {
                    Expr::Number(..) => Some(Ty::Number),
                    Expr::Str(..) => Some(Ty::String),
                    Expr::Bool(..) => Some(Ty::Bool),
                    _ => None,
                };
                if literal.is_some() && literal == resolved {
                    return self.expr(e);
                }
                let t = resolved.map_or_else(|| ".unknown".to_string(), |t| ty(&t));
                format!("(.typed {} {t})", self.expr(e)?)
            }
            Expr::Unary(op, inner, _) => {
                let op = match op {
                    UnOp::Neg => ".neg",
                    UnOp::Not => ".not",
                };
                format!("(.unary {op} {})", self.expr(inner)?)
            }
            Expr::Binary(op, a, b, _) => {
                let op = match op {
                    BinOp::Add => ".add",
                    BinOp::Sub => ".sub",
                    BinOp::Mul => ".mul",
                    BinOp::Div => ".div",
                    BinOp::Rem => ".rem",
                    BinOp::Eq => ".eq",
                    BinOp::Ne => ".ne",
                    BinOp::Lt => ".lt",
                    BinOp::Le => ".le",
                    BinOp::Gt => ".gt",
                    BinOp::Ge => ".ge",
                    BinOp::And => ".and",
                    BinOp::Or => ".or",
                };
                format!("(.binary {op} {} {})", self.expr(a)?, self.expr(b)?)
            }
            Expr::Ternary(c, a, b, _) => format!(
                "(.ternary {} {} {})",
                self.expr(c)?,
                self.expr(a)?,
                self.expr(b)?
            ),
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => format!(
                "(.matchOpt {} {} {} {})",
                self.expr(subject)?,
                string(var),
                self.expr(some)?,
                self.expr(none)?
            ),
            Expr::Arrow { params, body, .. } => {
                let ps: Vec<String> = params.iter().map(|p| string(p)).collect();
                format!("(.arrow [{}] {})", ps.join(", "), self.expr(body)?)
            }
            Expr::Let {
                name, value, body, ..
            } => format!(
                "(.letE {} {} {})",
                string(name),
                self.expr(value)?,
                self.expr(body)?
            ),
        })
    }

    fn stmts(&self, body: &[Stmt]) -> Result<String, CompileError> {
        list(body, |s| self.stmt(s))
    }

    fn stmt(&self, s: &Stmt) -> Result<String, CompileError> {
        Ok(match s {
            Stmt::Let { name, expr, .. } => {
                format!("(.letS {} {})", string(name), self.expr(expr)?)
            }
            Stmt::Assign { target, expr, .. } => {
                format!("(.assign {} {})", string(target), self.expr(expr)?)
            }
            Stmt::Command { name, args, .. } => {
                // The arguments in the order the runner receives them, `none`
                // where a named one is left to its default.
                let ordered = contract_lower::expr::command_args(name, args);
                let parts = ordered
                    .iter()
                    .map(|a| match a {
                        Some(e) => self.expr(e),
                        None => Ok(".none".to_string()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                format!("(.command {} [{}])", string(name), parts.join(", "))
            }
            Stmt::Send {
                target,
                source,
                args,
                ..
            } => format!(
                "(.send {} {} {})",
                string(target),
                string(source),
                list(args, |a| self.expr(a))?
            ),
            Stmt::Refresh { target, .. } => format!("(.refresh {})", string(target)),
            // A call by its callee's name and whole argument list (LLP 1089
            // D9), never the body the Rust compiler expanded: the semantics
            // gives the call its own meaning, so difftest checks the
            // expansion against it.
            Stmt::Call { action, args, .. } => format!(
                "(.call {} {})",
                string(action),
                list(args, |a| self.expr(a))?
            ),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => format!(
                "(.ifS {} {} {})",
                self.expr(cond)?,
                self.stmts(then)?,
                self.stmts(otherwise)?
            ),
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => format!(
                "(.matchS {} {} {} {})",
                self.expr(subject)?,
                string(&some.0),
                self.stmts(&some.1)?,
                self.stmts(none)?
            ),
        })
    }

    /// An element's attributes: its values, and its handlers as (event,
    /// action, curried arguments).
    fn attrs(
        &self,
        attrs: &[contract_syntax::Attr],
    ) -> Result<(Vec<String>, Vec<String>), CompileError> {
        let mut props = Vec::new();
        let mut handlers = Vec::new();
        for a in attrs {
            match contract_lower::tags::attr_valued(&a.name, &a.value) {
                Some(contract_lower::tags::AttrTarget::Handler(event)) => {
                    let (action, args): (&str, &[Expr]) = match &a.value {
                        Expr::Ident(action, _) => (action, &[]),
                        Expr::Call(action, args, _) => (action, args),
                        _ => return Err(refuse("a handler names an action", a.span)),
                    };
                    handlers.push(format!(
                        "({}, {}, {})",
                        string(event),
                        string(action),
                        list(args, |x| self.expr(x))?
                    ));
                }
                _ => props.push(format!("({}, {})", string(&a.name), self.expr(&a.value)?)),
            }
        }
        Ok((props, handlers))
    }

    fn nodes(&self, nodes: &[Node]) -> Result<String, CompileError> {
        list(nodes, |n| self.node(n))
    }

    fn node(&self, n: &Node) -> Result<String, CompileError> {
        Ok(match n {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                let (props, handlers) = self.attrs(attrs)?;
                format!(
                    "(.element {} {} [{}] [{}] {})",
                    string(tag),
                    list(positional, |x| self.expr(x))?,
                    props.join(", "),
                    handlers.join(", "),
                    self.nodes(children)?
                )
            }
            Node::When {
                tag,
                cond,
                then,
                otherwise,
                ..
            } => format!(
                "(.when {tag} {} {} {})",
                self.expr(cond)?,
                self.nodes(then)?,
                self.nodes(otherwise)?
            ),
            Node::Each {
                tag,
                var,
                index,
                list: items,
                key,
                body,
                ..
            } => format!(
                "(.each {tag} {} {} {} {} {})",
                string(var),
                match index {
                    Some(i) => format!("(.some {})", string(i)),
                    None => ".none".into(),
                },
                self.expr(items)?,
                self.expr(key)?,
                self.nodes(body)?
            ),
            Node::Match {
                tag,
                subject,
                some,
                none,
                ..
            } => format!(
                "(.matchN {tag} {} {} {} {})",
                self.expr(subject)?,
                string(&some.0),
                self.nodes(&some.1)?,
                self.nodes(none)?
            ),
            Node::Use { name, span, .. } => {
                return Err(refuse(format!("`{name}` was not expanded"), *span))
            }
            Node::Children { span } => return Err(refuse("`children` was not expanded", *span)),
        })
    }
}
