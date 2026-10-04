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

/// Compile one source text and emit `def <name> : Contract.Program`.
pub fn lean(src: &str, name: &str) -> Result<String, CompileError> {
    // The plan backend first: what it refuses is not a program.
    crate::compile(src)?;
    let mut file = contract_syntax::parse(src)?;
    contract_syntax::resolve_clock_timelines(&mut file)?;
    emit_file(&file, name)
}

/// [`lean`] for a file, resolving its `use`s as `compile_path` does.
pub fn lean_path(path: &Path, name: &str) -> Result<String, CompileError> {
    let src = std::fs::read_to_string(path).map_err(|e| CompileError {
        pass: "io",
        id: "contract-unreadable".into(),
        message: format!("{}: {e}", path.display()),
        span: Span::default(),
        file: Some(path.into()),
        related: Box::new([]),
    })?;
    crate::compile_path_source(path, &src)?;
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let app_root = root.canonicalize().map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{}: {e}", root.display()),
        span: Span::default(),
        file: Some(path.into()),
        related: Box::new([]),
    })?;
    let (file, _) =
        crate::sources::load(path, &src, &app_root).map_err(|mut all| all.swap_remove(0))?;
    emit_file(&file, name)
}

fn emit_file(file: &File, name: &str) -> Result<String, CompileError> {
    let checked = contract_types::check_all(file, false, contract_lower::tags::style, None)
        .map_err(|mut all| CompileError::from(all.swap_remove(0)))?;
    emit_checked(&checked, name)
}

/// Emit a checked expansion as `def <name> : Contract.Program`. The
/// differential type test (`difftest types`) also calls this with an
/// expansion the checker refused, typed by the program it was mutated
/// from, to give the Lean checker the mutant to judge.
pub fn emit_checked(checked: &Checked, name: &str) -> Result<String, CompileError> {
    let mut e = Emitter {
        out: String::new(),
        checked,
    };
    e.program(name)?;
    Ok(e.out)
}

struct Emitter<'a> {
    out: String,
    checked: &'a Checked<'a>,
}

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

fn list<T>(
    items: &[T],
    mut f: impl FnMut(&T) -> Result<String, CompileError>,
) -> Result<String, CompileError> {
    let parts = items.iter().map(&mut f).collect::<Result<Vec<_>, _>>()?;
    Ok(format!("[{}]", parts.join(", ")))
}

impl Emitter<'_> {
    fn root(&self) -> &Component {
        &self.checked.expanded.root
    }

    fn program(&mut self, name: &str) -> Result<(), CompileError> {
        let root = self.root();
        let types = &self.checked.types;
        let ct = &types.components[0];
        let shapes = &types.shapes;
        let mut o = String::new();
        let _ = writeln!(o, "def {name} : Contract.Program := {{");
        // Shapes: every shape the checker knows, the compiler's own too (a
        // source may answer a `Router`).
        let shape_rows: Vec<String> = shapes
            .map
            .iter()
            .map(|(s, fields)| {
                let fs: Vec<String> = fields
                    .iter()
                    .map(|(f, t)| format!("{{ name := {}, ty := {} }}", string(f), ty(t)))
                    .collect();
                format!("{{ name := {}, fields := [{}] }}", string(s), fs.join(", "))
            })
            .collect();
        let _ = writeln!(o, "  shapes := [{}],", shape_rows.join(",\n    "));
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
        let _ = writeln!(o, "  fns := [{}],", fns.join(",\n    "));
        let owners = &self.checked.expanded.owners;
        let mut states = Vec::new();
        for (i, s) in root.states.iter().enumerate() {
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
        let _ = writeln!(o, "  states := [{}],", states.join(",\n    "));
        let mut derives = Vec::new();
        for (i, d) in root.derives.iter().enumerate() {
            derives.push(format!(
                "{{ name := {}, ty := {}, body := {} }}",
                string(&d.name),
                ty(&ct.derives[i]),
                self.expr(&d.expr)?
            ));
        }
        let _ = writeln!(o, "  derives := [{}],", derives.join(",\n    "));
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
        let _ = writeln!(o, "  resources := [{}],", resources.join(",\n    "));
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
                format!(
                    "{{ name := {}, ty := {}, refreshes := [{}], andThen := {then} }}",
                    string(&m.name),
                    ty(&ct.mutations[i]),
                    refreshes.join(", ")
                )
            })
            .collect();
        let _ = writeln!(o, "  mutations := [{}],", mutations.join(",\n    "));
        let mut actions = Vec::new();
        for (i, a) in root.actions.iter().enumerate() {
            let ps: Vec<String> = a
                .params
                .iter()
                .zip(&ct.actions[i])
                .map(|(p, t)| format!("({}, {})", string(&p.name), ty(t)))
                .collect();
            actions.push(format!(
                "{{ name := {}, params := [{}], body := {} }}",
                string(&a.name),
                ps.join(", "),
                self.stmts(&a.body)?
            ));
        }
        let _ = writeln!(o, "  actions := [{}],", actions.join(",\n    "));
        let mut tasks = Vec::new();
        for t in &root.tasks {
            let kind = match t.kind {
                TaskKind::Every => ".every",
                TaskKind::After => ".after",
                TaskKind::Frame => ".frame",
            };
            tasks.push(format!(
                "{{ name := {}, kind := {kind}, ms := {}, action := {} }}",
                string(&t.name),
                self.expr(&t.timer.0)?,
                string(&t.timer.1)
            ));
        }
        let _ = writeln!(o, "  tasks := [{}],", tasks.join(",\n    "));
        let _ = writeln!(o, "  view := {},", self.nodes(&root.view)?);
        // The checked route table (LLP 1038 D2) and the slot `routes` names.
        let routes: Vec<String> = shapes
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
        let _ = writeln!(o, "  routes := [{}],", routes.join(",\n    "));
        let router = match &self.checked.file.routes {
            Some(r) => format!(".some {}", string(&r.slot)),
            None => ".none".into(),
        };
        let _ = writeln!(o, "  router := {router}");
        o.push_str("}\n");
        self.out.push_str(&o);
        Ok(())
    }

    fn expr(&self, e: &Expr) -> Result<String, CompileError> {
        let shapes = &self.checked.types.shapes;
        Ok(match e {
            Expr::Number(n, _) => format!("(.num 0x{:016x})", n.to_bits()),
            Expr::Str(s, _) => format!("(.str {})", string(s)),
            Expr::Bool(b, _) => format!("(.bool {b})"),
            Expr::None(_) => ".none".into(),
            Expr::EmptyList(_) => ".emptyList".into(),
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
            Expr::Call(name, args, _) => {
                format!("(.call {} {})", string(name), list(args, |a| self.expr(a))?)
            }
            Expr::NamedArg(n, v, _) => format!("(.named {} {})", string(n), self.expr(v)?),
            Expr::Typed(e, t, _) => {
                let t = self
                    .checked
                    .types
                    .shapes
                    .resolve(t)
                    .map_or_else(|_| ".unknown".to_string(), |t| ty(&t));
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
        // A tail call's type check (LLP 1017 §11) runs nothing: lowering
        // drops it, so the semantics never sees it.
        let run: Vec<&Stmt> = body
            .iter()
            .filter(|s| {
                !matches!(s, Stmt::Command { name, .. }
                    if name.starts_with(contract_syntax::inline::tail::CHECK))
            })
            .collect();
        list(&run, |s| self.stmt(s))
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
                let mut props = Vec::new();
                let mut handlers = Vec::new();
                for a in attrs {
                    match contract_lower::tags::attr(&a.name) {
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
