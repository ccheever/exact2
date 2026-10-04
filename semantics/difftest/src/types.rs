//! `difftest types`: the Lean type checker (`Contract.check`,
//! `semantics/Contract/TypeCheck.lean`, proved sound for the typing
//! judgments) against the Rust one (`contract_types::check_all`).
//!
//! Each generated program is one the Rust compiler accepts; its embedding
//! must be accepted by the Lean checker. Each also yields one mutant: an
//! expression of the program's AST changed so it likely no longer types (a
//! literal of another type, a field the shape lacks, an argument dropped,
//! added or of another type, `==` across types, a condition that is not a
//! bool). The Rust checker judges the mutant. A mutant it accepts is
//! emitted with its own types, and the Lean checker must accept it too; a
//! mutant it refuses cannot pass through `contract lean`, so its expansion
//! is emitted with the types of the program it came from
//! (`contract::lean::emit_checked`), and the Lean checker must refuse it.

use crate::gen;
use crate::leanrun;
use crate::rng::Rng;
use contract_syntax::{BinOp, Expr, File, Node, Stmt, TemplatePart};
use std::path::Path;
use std::process::Command;

/// How a mutant was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A literal replaced by one of another type.
    Literal,
    /// A member read of a field no shape has.
    Field,
    /// A call's last argument dropped.
    ArgDrop,
    /// An argument added to a call.
    ArgAdd,
    /// A call's argument replaced by a literal of another type.
    ArgType,
    /// `==`/`!=` with a right operand of another type.
    Eq,
    /// A condition (`?:`, `if`, `when`) that is a number.
    Cond,
}

const KINDS: [Kind; 7] = [
    Kind::Literal,
    Kind::Field,
    Kind::ArgDrop,
    Kind::ArgAdd,
    Kind::ArgType,
    Kind::Eq,
    Kind::Cond,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Plain,
    Cond,
}

/// Visit every expression of a file in preorder, with whether it stands as
/// a condition. `f` returns `true` to stop.
fn walk_file(file: &mut File, f: &mut dyn FnMut(&mut Expr, Role) -> bool) -> bool {
    for d in &mut file.fns {
        if walk_expr(&mut d.body, Role::Plain, f) {
            return true;
        }
    }
    for c in &mut file.components {
        let exprs = c
            .states
            .iter_mut()
            .chain(c.derives.iter_mut())
            .chain(c.provides.iter_mut())
            .map(|b| &mut b.expr)
            .chain(c.resources.iter_mut().flat_map(|r| r.args.iter_mut()))
            .chain(c.tasks.iter_mut().map(|t| &mut t.timer.0));
        for e in exprs {
            if walk_expr(e, Role::Plain, f) {
                return true;
            }
        }
        for a in &mut c.actions {
            if walk_stmts(&mut a.body, f) {
                return true;
            }
        }
        if walk_nodes(&mut c.view, f) {
            return true;
        }
    }
    false
}

fn walk_stmts(ss: &mut [Stmt], f: &mut dyn FnMut(&mut Expr, Role) -> bool) -> bool {
    for s in ss {
        let stop = match s {
            Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => walk_expr(expr, Role::Plain, f),
            Stmt::Command { args, .. } | Stmt::Send { args, .. } => {
                args.iter_mut().any(|a| walk_expr(a, Role::Plain, f))
            }
            Stmt::Refresh { .. } => false,
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => walk_expr(cond, Role::Cond, f) || walk_stmts(then, f) || walk_stmts(otherwise, f),
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                walk_expr(subject, Role::Plain, f)
                    || walk_stmts(&mut some.1, f)
                    || walk_stmts(none, f)
            }
        };
        if stop {
            return true;
        }
    }
    false
}

fn walk_nodes(ns: &mut [Node], f: &mut dyn FnMut(&mut Expr, Role) -> bool) -> bool {
    for n in ns {
        let stop = match n {
            // Only what the semantics evaluates of an element is mutated: a
            // text's text, the testId and handlers. Presentation is carried
            // but not typed by the judgments (semantics/README.md).
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                let text = matches!(tag.as_str(), "text" | "tspan" | "option");
                positional
                    .iter_mut()
                    .take(usize::from(text))
                    .any(|e| walk_expr(e, Role::Plain, f))
                    || attrs
                        .iter_mut()
                        .filter(|a| {
                            a.name == "testId"
                                || matches!(
                                    contract_lower::tags::attr(&a.name),
                                    Some(contract_lower::tags::AttrTarget::Handler(_))
                                )
                        })
                        .any(|a| walk_expr(&mut a.value, Role::Plain, f))
                    || walk_nodes(children, f)
            }
            Node::Use { args, children, .. } => {
                args.iter_mut()
                    .any(|a| walk_expr(&mut a.value, Role::Plain, f))
                    || walk_nodes(children, f)
            }
            Node::Children { .. } => false,
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => walk_expr(cond, Role::Cond, f) || walk_nodes(then, f) || walk_nodes(otherwise, f),
            Node::Each {
                list, key, body, ..
            } => {
                walk_expr(list, Role::Plain, f)
                    || walk_expr(key, Role::Plain, f)
                    || walk_nodes(body, f)
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                walk_expr(subject, Role::Plain, f)
                    || walk_nodes(&mut some.1, f)
                    || walk_nodes(none, f)
            }
        };
        if stop {
            return true;
        }
    }
    false
}

fn walk_expr(e: &mut Expr, role: Role, f: &mut dyn FnMut(&mut Expr, Role) -> bool) -> bool {
    if f(e, role) {
        return true;
    }
    match e {
        Expr::Number(..)
        | Expr::Str(..)
        | Expr::Bool(..)
        | Expr::None(_)
        | Expr::EmptyList(_)
        | Expr::Ident(..) => false,
        Expr::Template(parts, _) => parts.iter_mut().any(|p| match p {
            TemplatePart::Expr(x) => walk_expr(x, Role::Plain, f),
            TemplatePart::Text(_) => false,
        }),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Unary(_, x, _)
        | Expr::Typed(x, _, _) => walk_expr(x, Role::Plain, f),
        Expr::Call(_, args, _) => args.iter_mut().any(|a| walk_expr(a, Role::Plain, f)),
        Expr::Binary(_, a, b, _) => walk_expr(a, Role::Plain, f) || walk_expr(b, Role::Plain, f),
        Expr::Ternary(c, a, b, _) => {
            walk_expr(c, Role::Cond, f)
                || walk_expr(a, Role::Plain, f)
                || walk_expr(b, Role::Plain, f)
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            walk_expr(subject, Role::Plain, f)
                || walk_expr(some, Role::Plain, f)
                || walk_expr(none, Role::Plain, f)
        }
        Expr::Arrow { body, .. } => walk_expr(body, Role::Plain, f),
        Expr::Let { value, body, .. } => {
            walk_expr(value, Role::Plain, f) || walk_expr(body, Role::Plain, f)
        }
    }
}

fn applies(kind: Kind, e: &Expr, role: Role) -> bool {
    match kind {
        Kind::Literal => matches!(e, Expr::Number(..) | Expr::Str(..) | Expr::Bool(..)),
        Kind::Field => matches!(e, Expr::Member(..)),
        Kind::ArgAdd => matches!(e, Expr::Call(..)),
        Kind::ArgDrop | Kind::ArgType => matches!(e, Expr::Call(_, args, _) if !args.is_empty()),
        Kind::Eq => matches!(e, Expr::Binary(BinOp::Eq | BinOp::Ne, ..)),
        Kind::Cond => role == Role::Cond,
    }
}

/// A literal of another type than `e` (any type when `e` is no literal).
fn other_literal(e: &Expr, rng: &mut Rng) -> Expr {
    let span = e.span();
    let num = Expr::Number(7.0, span);
    let text = Expr::Str("zz".into(), span);
    let truth = Expr::Bool(true, span);
    let mut choices = match e {
        Expr::Number(..) => vec![text, truth],
        Expr::Str(..) | Expr::Template(..) => vec![num, truth],
        Expr::Bool(..) => vec![num, text],
        _ => vec![num, text, truth],
    };
    let i = rng.below(choices.len() as u64) as usize;
    choices.swap_remove(i)
}

fn describe(e: &Expr) -> String {
    match e {
        Expr::Number(n, _) => format!("{n}"),
        Expr::Str(s, _) => format!("{s:?}"),
        Expr::Bool(b, _) => format!("{b}"),
        Expr::Member(_, f, _) => format!(".{f}"),
        Expr::Call(n, args, _) => format!("{n}(…{} args)", args.len()),
        Expr::Binary(op, ..) => format!("{op:?}"),
        _ => "an expression".into(),
    }
}

/// Mutate `file` once with `kind` at a random site; what was done, or
/// `None` when the file has no such site.
fn mutate(file: &mut File, kind: Kind, rng: &mut Rng) -> Option<String> {
    let shapes: Vec<String> = file.shapes.iter().map(|s| s.name.clone()).collect();
    let mut count = 0usize;
    walk_file(file, &mut |e, role| {
        count += usize::from(applies(kind, e, role));
        false
    });
    if count == 0 {
        return None;
    }
    let target = rng.below(count as u64) as usize;
    let mut seen = 0usize;
    let mut what = None;
    walk_file(file, &mut |e, role| {
        if !applies(kind, e, role) {
            return false;
        }
        if seen < target {
            seen += 1;
            return false;
        }
        let span = e.span();
        what = Some(match (kind, &mut *e) {
            (Kind::Literal, _) => {
                let new = other_literal(e, rng);
                let w = format!("literal {} → {}", describe(e), describe(&new));
                *e = new;
                w
            }
            (Kind::Field, Expr::Member(_, field, _)) => {
                let w = format!("field .{field} → .{field}Zz");
                field.push_str("Zz");
                w
            }
            (Kind::ArgDrop, Expr::Call(name, args, _)) => {
                args.pop();
                format!("`{name}` given one argument fewer")
            }
            // A record's arguments are fields (`Shape(base, field=value)`): an
            // added one names a field the shape lacks, and a retyped one keeps
            // its name.
            (Kind::ArgAdd, Expr::Call(name, args, _)) if shapes.contains(name) => {
                args.push(Expr::NamedArg(
                    "zz".into(),
                    Box::new(Expr::Number(1.0, span)),
                    span,
                ));
                format!("`{name}` given a field `zz`")
            }
            (Kind::ArgAdd, Expr::Call(name, args, _)) => {
                args.push(Expr::Number(1.0, span));
                format!("`{name}` given one argument more")
            }
            (Kind::ArgType, Expr::Call(name, args, _)) => {
                let i = rng.below(args.len() as u64) as usize;
                if let Expr::NamedArg(field, value, _) = &mut args[i] {
                    let new = other_literal(value, rng);
                    let w = format!("field `{field}` of `{name}` → {}", describe(&new));
                    **value = new;
                    w
                } else {
                    let new = other_literal(&args[i], rng);
                    let w = format!("argument {} of `{name}` → {}", i + 1, describe(&new));
                    args[i] = new;
                    w
                }
            }
            (Kind::Eq, Expr::Binary(op, a, b, _)) => {
                let new = other_literal(a, rng);
                let w = format!("{op:?} with right operand {}", describe(&new));
                **b = new;
                w
            }
            (Kind::Cond, _) => {
                let w = format!("condition {} → 7", describe(e));
                *e = Expr::Number(7.0, span);
                w
            }
            _ => unreachable!("`applies` chose the site"),
        });
        true
    });
    what
}

/// Rust refusals the judgments do not model, by id: the embedding does not
/// carry what they read. A prop's or an inject's declared type (its
/// argument is substituted, untyped, into the expansion) and a data
/// source's one signature across its uses (the semantics leaves a source's
/// signature free). A mutant refused only for one of these is well typed in
/// the expansion Lean judges, so Lean accepts it.
const OUTSIDE: [&str; 3] = ["type-prop", "type-provide", "type-source-signature"];

/// One program for the Lean checker, and what it should say.
struct Item {
    label: String,
    source: String,
    what: String,
    /// The Rust checker's verdict: `Ok` accepted, `Err` its refusal.
    rust: Result<(), String>,
    program: String,
}

/// The mutant of `source`: a mutation kind tried in a random order until
/// one applies, the Rust verdict, and the embedding to give Lean.
fn mutant(source: &str, seed: u64, name: &str) -> Option<(String, Result<(), String>, String)> {
    let file = contract_syntax::parse(source).ok()?;
    let style = contract_lower::tags::style;
    let original = contract_types::check_all(&file, false, style, None).ok()?;
    let unchanged = contract::lean::emit_checked(&original, name).ok()?;
    let mut rng = Rng::new(seed);
    let mut kinds = KINDS.to_vec();
    while !kinds.is_empty() {
        let kind = kinds.swap_remove(rng.below(kinds.len() as u64) as usize);
        let mut m = file.clone();
        let Some(what) = mutate(&mut m, kind, &mut rng) else {
            continue;
        };
        let (rust, program) = match contract_types::check_all(&m, false, style, None) {
            Ok(checked) => (Ok(()), contract::lean::emit_checked(&checked, name)),
            Err(errors) => {
                let (expanded, _) = contract_syntax::expand_all(&m, false);
                let checked = contract_types::Checked {
                    file: &m,
                    types: original.types.clone(),
                    expanded,
                };
                let why = errors
                    .first()
                    .map_or_else(String::new, |e| format!("[{}] {}", e.id, e.message));
                (Err(why), contract::lean::emit_checked(&checked, name))
            }
        };
        // An expansion the emitter cannot write (a mutated handler that no
        // longer names an action) is not a mutant Lean can judge.
        let Ok(program) = program else { continue };
        // A mutation the expansion leaves out (in a component nothing uses)
        // changes no program the semantics sees.
        if program == unchanged {
            continue;
        }
        return Some((what, rust, program));
    }
    None
}

/// `difftest types`: `count` generated programs from `seed` and a mutant
/// of each. Prints each disagreement and a summary; whether all agreed.
pub fn run(seed: u64, count: usize, dir: &Path) -> Result<bool, String> {
    leanrun::build()?;
    build_checker()?;
    // The corpus's programs, then the generated ones.
    let mut sources: Vec<(String, String)> = Vec::new();
    let mut files = Vec::new();
    crate::collect(&leanrun::project().join("corpus"), &mut files)?;
    files.sort();
    for f in files {
        let src = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let name = f
            .strip_prefix(leanrun::project())
            .unwrap_or(&f)
            .display()
            .to_string();
        sources.push((name, src));
    }
    let size = gen::Size::default();
    for i in 0..count as u64 {
        let case = gen::case(seed.wrapping_add(i), &size);
        sources.push((case.name, case.source));
    }
    let mut items = Vec::new();
    let mut refused = 0;
    for (i, (label, source)) in sources.into_iter().enumerate() {
        let name = format!("o{i}");
        match contract::lean::lean(&source, &name) {
            Ok(program) => items.push(Item {
                label: label.clone(),
                source: source.clone(),
                what: "the program as written".into(),
                rust: Ok(()),
                program,
            }),
            Err(e) => {
                refused += 1;
                println!("REFUSED {label}: {e}");
                continue;
            }
        }
        let name = format!("m{i}");
        if let Some((what, rust, program)) = mutant(&source, crate::seed_of(&label) ^ seed, &name) {
            items.push(Item {
                label: format!("{label} mutant"),
                source,
                what,
                rust,
                program,
            });
        }
    }
    let verdicts = lean_checks(&items, dir)?;
    std::fs::create_dir_all(dir.join("types")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (mut agree, mut lean_refuses, mut lean_accepts, mut outside) = (0, 0, 0, 0);
    let (mut refused_by_both, mut mutants) = (0, 0);
    for (k, (item, (ok, failure))) in items.iter().zip(&verdicts).enumerate() {
        if item.label.ends_with("mutant") {
            mutants += 1;
            if item.rust.is_err() && !ok {
                refused_by_both += 1;
            }
        }
        if item.rust.is_ok() == *ok {
            agree += 1;
            continue;
        }
        if let Err(why) = &item.rust {
            if *ok && OUTSIDE.iter().any(|id| why.starts_with(&format!("[{id}]"))) {
                outside += 1;
                println!("OUTSIDE {} ({}): rust {why}", item.label, item.what);
                continue;
            }
        }
        let stem = dir
            .join("types")
            .join(format!("{}-{k}", std::process::id()));
        let note = format!(
            "{}\nmutation: {}\nrust: {}\nlean: {}\n",
            item.label,
            item.what,
            item.rust
                .as_ref()
                .map_or_else(|e| format!("refused {e}"), |_| "accepted".into()),
            if *ok {
                "accepted".to_string()
            } else {
                format!("refused at {failure}")
            }
        );
        let _ = std::fs::write(stem.with_extension("contract"), &item.source);
        let _ = std::fs::write(stem.with_extension("lean"), &item.program);
        let _ = std::fs::write(stem.with_extension("txt"), &note);
        if *ok {
            lean_accepts += 1;
            println!(
                "LEAN ACCEPTS {} ({}): rust {}\n  kept: {}.{{contract,lean,txt}}",
                item.label,
                item.what,
                item.rust.as_ref().err().map_or("", String::as_str),
                stem.display()
            );
        } else {
            lean_refuses += 1;
            println!(
                "LEAN REFUSES {} ({}) at {failure}\n  kept: {}.{{contract,lean,txt}}",
                item.label,
                item.what,
                stem.display()
            );
        }
    }
    println!(
        "difftest types: {} programs ({mutants} mutants, {refused_by_both} refused by both): {agree} agree, \
         {lean_refuses} refused by Lean alone, {lean_accepts} accepted by Lean alone, \
         {outside} refused by Rust for what the embedding does not carry, {refused} refused by the compiler",
        items.len()
    );
    Ok(lean_refuses == 0 && lean_accepts == 0 && refused == 0)
}

/// Build `Contract.TypeCheck`, which the checker modules import.
pub fn build_checker() -> Result<(), String> {
    let out = Command::new(leanrun::lake())
        .args(["build", "Contract.TypeCheck"])
        .current_dir(leanrun::project())
        .output()
        .map_err(|e| format!("lake build: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "lake build Contract.TypeCheck failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// The Lean checker's verdict on each item, in one Lean process per 100:
/// accepted, or the part it refused.
fn lean_checks(items: &[Item], dir: &Path) -> Result<Vec<(bool, String)>, String> {
    let mut out = Vec::with_capacity(items.len());
    for (b, chunk) in items.chunks(100).enumerate() {
        let mut module = String::from(
            "import Contract.TypeCheck\nopen Contract\n\nset_option maxRecDepth 100000\n\n",
        );
        for item in chunk {
            module.push_str(&item.program);
            module.push('\n');
        }
        module.push_str("def main : IO Unit := do\n");
        for item in chunk {
            let name = item
                .program
                .strip_prefix("def ")
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("?");
            module.push_str(&format!(
                "  IO.println s!\"#check {{check {name}}} {{(checkFailure {name}).getD \"\"}}\"\n"
            ));
        }
        if chunk.is_empty() {
            module.push_str("  pure ()\n");
        }
        let lines = leanrun::run_module(&module, dir, &format!("types-{b}"))?;
        for l in lines.iter().filter_map(|l| l.strip_prefix("#check ")) {
            let (verdict, failure) = l.split_once(' ').unwrap_or((l, ""));
            out.push((verdict == "true", failure.to_string()));
        }
    }
    if out.len() != items.len() {
        return Err(format!(
            "expected {} verdicts, got {}",
            items.len(),
            out.len()
        ));
    }
    Ok(out)
}
