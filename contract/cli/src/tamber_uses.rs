//! `uses` (LLP 1121 stage 4, commit 6; LLP 1122 T2b's `outline`): where the
//! app reads a token, answered statically. The plan's style bindings whose
//! code names the token, directly, through a token whose value names it, or
//! through a synthetic token (a row's expression over tokens) that reads
//! it; each joined with the plan's source map into a source span. A
//! mounted-instance count would need a report from the builds, which the
//! build channel does not carry (LLP 1121 §9 candidate).

use exact_plan::{BindingKind, Opcode, Operand, Plan};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Whether `text` names `name` as a whole token name: not inside a longer
/// one (`accent.fill` is not in `accent.fill-2`), nor an id (`#name`).
pub fn names(text: &str, name: &str) -> bool {
    let part = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    // After `#` it is an element's id (`url(#brand)`) or a hex colour.
    text.match_indices(name).any(|(i, _)| {
        !text[..i]
            .chars()
            .next_back()
            .is_some_and(|c| part(c) || c == '#')
            && !text[i + name.len()..].chars().next().is_some_and(part)
    })
}

/// What a piece of code reads: the strings it pushes as values, and the
/// slots and derives it loads, with what each `StoreSlot` stores. A value
/// compared (`mode == "brand"`, `"brand" == a + b`) is no read: a stack walk
/// over each straight run of ops follows values into comparisons; across a
/// jump or a call it keeps them, as reads. Empty when the code does not walk
/// whole.
#[derive(Default)]
struct Reads<'a> {
    strings: Vec<&'a str>,
    slots: Vec<usize>,
    derives: Vec<usize>,
    /// Each `StoreSlot`: the slot, and what the value it stores reads.
    stores: Vec<(usize, Reads<'a>)>,
}

/// One op of a walk: the op, the string it pushed, the slot (`true`) or
/// derive it loaded, and the slot it stored or the local it loaded.
type Step<'a> = (
    Opcode,
    Option<&'a str>,
    Option<(bool, usize)>,
    Option<usize>,
);

fn reads<'a>(plan: &'a Plan, code: &[u8]) -> Reads<'a> {
    let mut r = exact_plan::bytes::Reader::new(code);
    let mut ops: Vec<Step<'a>> = Vec::new();
    while !r.is_empty() {
        let Some(op) = r.u8().ok().and_then(Opcode::from_wire) else {
            return Reads::default();
        };
        let (mut pushed, mut loaded, mut slot) = (None, None, None);
        for operand in op.operands() {
            let ok = match operand {
                Operand::U8 | Operand::Enum(_) => r.u8().is_ok(),
                Operand::U16 => r
                    .u16()
                    .map(|v| {
                        if op == Opcode::LoadLocal {
                            slot = Some(v as usize);
                        }
                    })
                    .is_ok(),
                Operand::U32 => r.u32().is_ok(),
                Operand::Idx(table) => r
                    .u32()
                    .map(|v| match op {
                        Opcode::LoadSlot | Opcode::LoadDerive => {
                            loaded = Some((*table == "slots", v as usize));
                        }
                        Opcode::StoreSlot => slot = Some(v as usize),
                        _ => {}
                    })
                    .is_ok(),
                Operand::F64 => r.f64().is_ok(),
                Operand::Str => r
                    .u32()
                    .map(|v| {
                        if op == Opcode::Str {
                            pushed = plan.strings.get(v as usize);
                        }
                    })
                    .is_ok(),
            };
            if !ok {
                return Reads::default();
            }
        }
        ops.push((op, pushed, loaded, slot));
    }
    // The stack, and the locals bound: each value, the ops (strings, loads)
    // it carries.
    let mut stack: Vec<Vec<usize>> = Vec::new();
    let mut locals: Vec<Vec<usize>> = Vec::new();
    let mut compared = vec![false; ops.len()];
    // Each store: its slot, the run of ops before it, and what the value it
    // stores carries (through a local, a concatenation).
    let mut runs: Vec<(usize, usize, usize, Vec<usize>)> = Vec::new();
    let mut start = 0;
    for (i, (op, pushed, loaded, slot)) in ops.iter().enumerate() {
        let pop = |stack: &mut Vec<Vec<usize>>| stack.pop().unwrap_or_default();
        use Opcode as O;
        match op {
            O::LoadLocal => {
                let v = slot
                    .and_then(|n| locals.get(n))
                    .cloned()
                    .unwrap_or_default();
                stack.push(v);
            }
            O::Number
            | O::Bool
            | O::Str
            | O::None
            | O::Unit
            | O::LoadSlot
            | O::LoadDerive
            | O::LoadResource
            | O::LoadParam
            | O::LoadItem
            | O::LoadBound
            | O::PendingResource
            | O::PendingMutation
            | O::FailedResource
            | O::FailureResource => {
                stack.push(if pushed.is_some() || loaded.is_some() {
                    vec![i]
                } else {
                    Vec::new()
                });
            }
            O::Neg | O::Some | O::Unwrap | O::Field | O::LoadIndex => {
                let v = pop(&mut stack);
                stack.push(v);
            }
            O::Not => {
                pop(&mut stack);
                stack.push(Vec::new());
            }
            O::Add | O::Sub | O::Mul | O::Div | O::Rem | O::Concat => {
                let (b, mut a) = (pop(&mut stack), pop(&mut stack));
                a.extend(b);
                stack.push(a);
            }
            O::Eq | O::Ne | O::Lt | O::Le | O::Gt | O::Ge => {
                for j in pop(&mut stack).into_iter().chain(pop(&mut stack)) {
                    compared[j] = true;
                }
                stack.push(Vec::new());
            }
            O::List => {
                // `List(n)`: its items stay reads.
                stack.clear();
                stack.push(Vec::new());
            }
            O::StoreSlot => {
                let v = pop(&mut stack);
                runs.push((slot.unwrap_or(usize::MAX), start, i, v));
                start = i + 1;
            }
            O::BindLocal => {
                let v = pop(&mut stack);
                locals.push(v);
                start = i + 1;
            }
            O::DropLocal => {
                locals.pop();
                start = i + 1;
            }
            O::Pop => {
                pop(&mut stack);
                start = i + 1;
            }
            // A jump, a call, a command: values on the stack are kept, as reads.
            _ => {
                stack.clear();
                if matches!(op, O::Command | O::Send | O::Return) {
                    start = i + 1;
                }
            }
        }
    }
    let read = |run: &mut dyn Iterator<Item = usize>| {
        let mut out = Reads::default();
        for j in run {
            if compared[j] {
                continue;
            }
            let (_, pushed, loaded, _) = &ops[j];
            if let Some(s) = pushed {
                out.strings.push(s);
            }
            match loaded {
                Some((true, n)) => out.slots.push(*n),
                Some((false, n)) => out.derives.push(*n),
                None => {}
            }
        }
        out
    };
    let mut out = read(&mut (0..ops.len()));
    out.stores = runs
        .into_iter()
        .map(|(slot, a, b, carried)| {
            let mut run: Vec<usize> = (a..b).chain(carried).collect();
            run.sort_unstable();
            run.dedup();
            (slot, read(&mut run.into_iter()))
        })
        .collect();
    out
}

/// Every site whose style rows read `token`. `aliases` are the tokens whose
/// value names it (closed under naming); `synthetic` the app's synthetic
/// expressions. `map` is the plan's source map as JSON (`SourceMap::json`),
/// node by node.
pub fn sites(
    plan: &Plan,
    map: &Value,
    token: &str,
    aliases: &BTreeSet<String>,
    synthetic: &[String],
) -> Vec<Value> {
    // What a row may name to read it: the token, a token that aliases it,
    // or a synthetic expression over either.
    // A role override is read by any of its spellings (`secondary-label`,
    // `-exact-secondary-label`, any case), as the compiler takes them.
    let role = |name: &str| {
        let name = name.trim();
        exact_kernel::style::roles::role(name).or_else(|| {
            (!name.starts_with('-'))
                .then(|| exact_kernel::style::roles::role(&format!("-exact-{name}")))
                .flatten()
        })
    };
    let token_role = role(token);
    let via_token = |s: &str| {
        if names(s, token) || token_role.is_some() && role(s) == token_role {
            return Some(Value::Null);
        }
        aliases
            .iter()
            .find(|a| names(s, a))
            .map(|a| json!({"token": a}))
    };
    let via_synthetic: Vec<(&str, Value)> = synthetic
        .iter()
        .filter_map(|e| via_token(e).map(|v| (e.as_str(), v)))
        .collect();
    // A string a piece of code pushes, read as the token: how it names it.
    let named = |rd: &Reads| {
        rd.strings.iter().find_map(|s| {
            // A synthetic token is its expression's text, in the row's.
            via_synthetic
                .iter()
                .find(|(e, _)| names(s, e))
                .map(|(e, v)| json!({"synthetic": e, "through": v}))
                .or_else(|| via_token(s))
        })
    };
    // The slots and derives that may hold the token: a slot whose initial
    // value or a value stored into it (an action's) names it, or loads one
    // that may; a derive whose body names it or loads one that may.
    let inits: Vec<Reads> = plan
        .slots
        .iter()
        .map(|s| reads(plan, plan.code(s.init)))
        .collect();
    let bodies: Vec<Reads> = plan
        .actions
        .iter()
        .map(|a| reads(plan, plan.code(a.body)))
        .collect();
    let derive_reads: Vec<Reads> = plan
        .derives
        .iter()
        .map(|d| reads(plan, plan.code(d.body)))
        .collect();
    let mut holds: Vec<Option<Value>> = vec![None; plan.slots.len()];
    let mut derived: Vec<Option<Value>> = vec![None; plan.derives.len()];
    let carried = |rd: &Reads, holds: &[Option<Value>], derived: &[Option<Value>]| {
        named(rd).is_some()
            || rd
                .slots
                .iter()
                .any(|s| holds.get(*s).is_some_and(Option::is_some))
            || rd
                .derives
                .iter()
                .any(|d| derived.get(*d).is_some_and(Option::is_some))
    };
    loop {
        let mut moved = false;
        for n in 0..plan.slots.len() {
            if holds[n].is_none()
                && (carried(&inits[n], &holds, &derived)
                    || bodies
                        .iter()
                        .flat_map(|b| &b.stores)
                        .any(|(slot, rd)| *slot == n && carried(rd, &holds, &derived)))
            {
                holds[n] = Some(json!({"slot": plan.str(plan.slots[n].name)}));
                moved = true;
            }
        }
        for (n, rd) in derive_reads.iter().enumerate() {
            if derived[n].is_none() && carried(rd, &holds, &derived) {
                derived[n] = Some(json!({"derive": plan.str(plan.derives[n].name)}));
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    // The kinds of the token and of the tokens naming it, as the plan keeps
    // them (a colour where it keeps none): a length inside a shadow is read
    // by the shadow's row too.
    let kind_of = |name: &str| {
        plan.theme
            .iter()
            .find(|t| plan.str(t.name) == name)
            .map_or(0, |t| t.kind)
    };
    let kinds: Vec<u8> = std::iter::once(kind_of(token))
        .chain(aliases.iter().map(|a| kind_of(a)))
        .collect();
    let mut out = Vec::new();
    // Each node's rows, then each `:hover` and `:focus-visible` look's, read
    // at the node (LLP 1121 D8).
    let rows = (plan.nodes.iter().enumerate())
        .map(|(i, node)| (i, node.bindings, None))
        .chain(plan.looks.iter().map(|l| {
            let state = if l.state == 1 {
                "hover"
            } else {
                "focus-visible"
            };
            (l.node.0 as usize, l.bindings, Some(state))
        }));
    for (i, range, look) in rows {
        let site = &map["nodes"][i];
        for b in range.iter().map(|id| plan.binding(id)) {
            if b.kind != BindingKind::Style {
                continue;
            }
            // A row of the token's kind reads it (LLP 1121 D3): a colour
            // only a row that takes one (`text-align: center` names none),
            // another kind the rows the kernel admits it in (a shadow its own).
            let Some(row) = exact_kernel::StyleId::from_bit(b.id as u32).filter(|r| {
                kinds
                    .iter()
                    .any(|kind| match exact_kernel::style::theme::Kind::of(*kind) {
                        Some(exact_kernel::style::theme::Kind::Color) | None => colours(*r),
                        Some(k) => exact_kernel::style::theme::admits(k, *r),
                    })
            }) else {
                continue;
            };
            let rd = reads(plan, plan.code(b.expr));
            let found = named(&rd)
                .or_else(|| {
                    rd.slots
                        .iter()
                        .find_map(|s| holds.get(*s).cloned().flatten())
                })
                .or_else(|| {
                    rd.derives
                        .iter()
                        .find_map(|d| derived.get(*d).cloned().flatten())
                });
            let Some(via) = found else { continue };
            let mut at = json!({
                "site": i,
                "row": row.name(),
                "file": site["file"],
                "line": site["line"],
                "col": site["col"],
                "end_col": site["end_col"],
                "component": site["component"],
            });
            // A state part's row is read where it is written: `:active`'s
            // press paint (LLP 1121 D8), a look's row at its `:hover` or
            // `:focus-visible` line (LLP 1122's queue item (1)). The map
            // keys a part by (row, state): the row's `states`.
            let state = look.unwrap_or("active");
            let bound = site["bindings"].as_array().into_iter().flatten();
            if let Some(part) = bound
                .filter(|b| b["row"] == row.name())
                .find_map(|b| b["states"].as_array()?.iter().find(|p| p["state"] == state))
            {
                // (A part with no place, a conditional class's, keeps the
                // node's, in its state.)
                if part.get("at").is_some() {
                    for key in ["file", "line", "col", "end_col"] {
                        at[key] = part["at"][key].clone();
                    }
                }
                at["state"] = part["state"].clone();
            }
            if !via.is_null() {
                at["via"] = via;
            }
            // (A look's row the map gives no part keeps the node's place, in its state.)
            if let Some(state) = look {
                at["state"] = state.into();
            }
            out.push(at);
        }
    }
    out
}

/// Whether a row's value takes a colour: a colour, a paint, or a shadow,
/// filter, gradient, mask or palette that holds one.
fn colours(row: exact_kernel::StyleId) -> bool {
    use exact_kernel::StyleCodec as C;
    matches!(
        row.codec(),
        C::Rgba8
            | C::ColorValue
            | C::KeywordColor
            | C::Color2
            | C::Paint
            | C::Filter
            | C::BackgroundImage
            | C::BackdropFilter
            | C::BoxShadow
            | C::TextShadow
            | C::MaskImage
            | C::SymbolPalette
    )
}

/// The tokens whose values name `token`, directly or through another:
/// `names` gives each token's named tokens.
pub fn aliases<'a>(
    token: &str,
    names: impl Iterator<Item = (&'a str, &'a [String])> + Clone,
) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut frontier = vec![token.to_string()];
    while let Some(t) = frontier.pop() {
        for (name, named) in names.clone() {
            if named.contains(&t) && name != token && found.insert(name.to_string()) {
                frontier.push(name.to_string());
            }
        }
    }
    found
}

/// The sites a duration or easing token is read at (LLP 1121 D3): the build
/// writes it into its rows, so the plan holds no name to find. They are the
/// literals the loader wrote it into, in every source the app at `app`
/// loads (a package's too), and the component arguments whose literals name
/// it, which an expansion writes in: `row` is the row's name, or null for an
/// argument, whose `argument` is `{component, name}`. A site through a
/// token naming it (`aliases`) says so (`via: {token}`). `site` is null: the
/// plan node is the row's, not the name's. Err, the first refusal's message,
/// when the app's sources do not load.
pub fn motion_sites(
    app: &std::path::Path,
    token: &str,
    aliases: &[String],
) -> Result<Vec<Value>, String> {
    let first = |all: Vec<crate::CompileError>| {
        all.into_iter()
            .next()
            .map_or_else(|| "the app does not load".into(), |e| e.message)
    };
    let src = std::fs::read_to_string(app).map_err(|e| format!("{}: {e}", app.display()))?;
    let root = crate::app_root(app).map_err(first)?;
    let (_, sources) = crate::sources::load(app, &src, &root).map_err(first)?;
    let mut out = Vec::new();
    for site in &sources.motion {
        let via = if site.tokens.iter().any(|t| t == token) {
            None
        } else {
            match site.tokens.iter().find(|t| aliases.contains(t)) {
                Some(alias) => Some(json!({"token": alias})),
                None => continue,
            }
        };
        // The loader's path, which `motion_instances` joins against the
        // elements' (round 2, Astra 2): the finished reply names it
        // (`relative_files`), never this list.
        let shown = sources.path(site.span).to_string_lossy().into_owned();
        let mut v = json!({
            "site": Value::Null,
            "row": match site.component {
                None => Value::from(site.attr.replace('-', "_")),
                Some(_) => Value::Null,
            },
            "file": shown,
            "line": site.span.line,
            "col": site.span.col,
            "end_col": site.span.end_col,
        });
        if let Some(component) = &site.component {
            v["argument"] = json!({"component": component, "name": site.attr});
        }
        if let Some(via) = via {
            v["via"] = via;
        }
        out.push(v);
    }
    Ok(out)
}

/// The elements of the app at `app` (every source it loads, a package's
/// too), each with its component's identity, by walking each view: what
/// `uses` joins to the map's nodes by span (LLP 1121 stage 9 C2).
fn elements(
    app: &std::path::Path,
    map: &Value,
) -> Result<(contract_syntax::File, crate::sources::Sources), String> {
    let first = |all: Vec<crate::CompileError>| {
        all.into_iter()
            .next()
            .map_or_else(|| "the app does not load".into(), |e| e.message)
    };
    let src = std::fs::read_to_string(app).map_err(|e| format!("{}: {e}", app.display()))?;
    let root = crate::app_root(app).map_err(first)?;
    let (file, sources) = crate::sources::load(app, &src, &root).map_err(first)?;
    // The sources as the map's plan was compiled from them: a save between
    // the two reads would join one revision's elements to another's nodes.
    let compiled: std::collections::HashMap<&str, &str> = map["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["file"].as_str()?, s["sha256"].as_str()?)))
        .collect();
    for (path, digest) in sources.digested() {
        if compiled
            .get(path.to_string_lossy().as_ref())
            .is_some_and(|d| *d != digest)
        {
            return Err(format!(
                "{} changed while it was read: ask again",
                shown(path, app.parent().unwrap_or(app))
            ));
        }
    }
    Ok((file, sources))
}

/// `path` as an answer names it, never with this machine's directories
/// (the rule `scripts/tamber-paths.mjs` keeps for the JS server too):
/// - inside the app's directory `dir` (as given or canonical: a temporary
///   directory may be reached through a link), from it
///   (`node_modules/@acme/reading/index.contract`);
/// - else inside a package (the nearest directory above the file whose
///   `package.json` has a name), as D15 names a package's file
///   (`sources::home`): `<name>/<path inside it>` (`@acme/linked/index.contract`
///   for a package linked from outside the app);
/// - else `<outside>/<its name>`.
///
/// A relative path is the app's already and is kept.
pub fn shown(path: &std::path::Path, dir: &std::path::Path) -> String {
    let slashed = |p: &std::path::Path| p.to_string_lossy().replace('\\', "/");
    if path.is_relative() {
        return slashed(path);
    }
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    // Real through its nearest existing ancestor, the missing rest kept (a
    // package's `dist/theme.tamber` where there is no `dist/`).
    let real = {
        let mut at = path;
        let mut rest = Vec::new();
        while !at.exists() {
            match (at.parent(), at.file_name()) {
                (Some(up), Some(name)) => {
                    rest.push(name.to_owned());
                    at = up;
                }
                _ => break,
            }
        }
        let mut real = at.canonicalize().unwrap_or_else(|_| at.to_path_buf());
        real.extend(rest.iter().rev());
        real
    };
    for (p, d) in [
        (path, dir),
        (path, canonical.as_path()),
        (real.as_path(), canonical.as_path()),
    ] {
        if let Ok(rest) = p.strip_prefix(d) {
            return slashed(rest);
        }
    }
    for root in real.ancestors().skip(1) {
        let named = std::fs::read_to_string(root.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|m| m["name"].as_str().map(str::to_string));
        if let (Some(name), Ok(rest)) = (named, real.strip_prefix(root)) {
            return format!("{name}/{}", slashed(rest));
        }
    }
    format!(
        "<outside>/{}",
        real.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    )
}

/// Every `file` in an answer's `sites` (any depth) as [`shown`] names it
/// (tamber/1.15: an absolute path leaked the machine's directories).
pub fn relative_files(sites: &mut [Value], dir: &std::path::Path) {
    sites.iter_mut().for_each(|v| relative(v, dir));
}

/// [`relative_files`] of one value.
fn relative(v: &mut Value, dir: &std::path::Path) {
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                match (k.as_str(), &*x) {
                    ("file", Value::String(f)) => {
                        *x = Value::from(shown(std::path::Path::new(f), dir))
                    }
                    _ => relative(x, dir),
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| relative(x, dir)),
        _ => {}
    }
}

/// Each element of `nodes` (recursively), with `f`.
fn walk<'a>(nodes: &'a [contract_syntax::Node], f: &mut dyn FnMut(&'a contract_syntax::Node)) {
    use contract_syntax::Node;
    for n in nodes {
        f(n);
        match n {
            Node::Element { children, .. } | Node::Use { children, .. } => walk(children, f),
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, f);
                walk(otherwise, f);
            }
            Node::Each { body, .. } => walk(body, f),
            Node::Match { some, none, .. } => {
                walk(&some.1, f);
                walk(none, f);
            }
            Node::Children { .. } => {}
        }
    }
}

/// The strings an expression may take as an image's source: a literal, a
/// template with no `${}`, either arm of a condition or a `match`, a list's
/// items; never its condition or subject.
fn strings(e: &contract_syntax::Expr, out: &mut Vec<String>) {
    use contract_syntax::{Expr, TemplatePart};
    match e {
        Expr::Str(s, _) => out.push(s.clone()),
        Expr::Template(parts, _) if parts.iter().all(|p| matches!(p, TemplatePart::Text(_))) => out
            .push(
                parts
                    .iter()
                    .map(|p| match p {
                        TemplatePart::Text(t) => t.as_str(),
                        TemplatePart::Expr(_) => "",
                    })
                    .collect(),
            ),
        Expr::Ternary(_, a, b, _) => {
            strings(a, out);
            strings(b, out);
        }
        Expr::Match { some, none, .. } => {
            strings(some, out);
            strings(none, out);
        }
        Expr::List(items, _) => items.iter().for_each(|i| strings(i, out)),
        _ => {}
    }
}

/// The places of every string literal inside an expression (a motion
/// literal may sit in a condition's arm, a `+`'s right side, a `match`'s).
fn literal_spans(e: &contract_syntax::Expr, out: &mut Vec<contract_syntax::Span>) {
    use contract_syntax::{Expr, TemplatePart};
    match e {
        Expr::Str(_, s) | Expr::Template(_, s) => {
            out.push(*s);
            if let Expr::Template(parts, _) = e {
                for p in parts {
                    if let TemplatePart::Expr(x) = p {
                        literal_spans(x, out);
                    }
                }
            }
        }
        Expr::List(items, _) | Expr::Call(_, items, _) => {
            items.iter().for_each(|i| literal_spans(i, out))
        }
        Expr::Some(x, _)
        | Expr::Unary(_, x, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Member(x, _, _)
        | Expr::Typed(x, _, _) => literal_spans(x, out),
        Expr::Binary(_, a, b, _) => {
            literal_spans(a, out);
            literal_spans(b, out);
        }
        Expr::Ternary(c, a, b, _) => {
            literal_spans(c, out);
            literal_spans(a, out);
            literal_spans(b, out);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            literal_spans(subject, out);
            literal_spans(some, out);
            literal_spans(none, out);
        }
        Expr::Arrow { body, .. } => literal_spans(body, out),
        Expr::Let { value, body, .. } => {
            literal_spans(value, out);
            literal_spans(body, out);
        }
        _ => {}
    }
}

/// The map's nodes by where they are declared (`file`, `line`, `col`):
/// every instance of an element, indexed once.
fn by_span(map: &Value) -> std::collections::HashMap<(String, u64, u64), Vec<usize>> {
    let mut out: std::collections::HashMap<(String, u64, u64), Vec<usize>> = Default::default();
    for (i, n) in map["nodes"].as_array().into_iter().flatten().enumerate() {
        if let (Some(f), Some(l), Some(c)) =
            (n["file"].as_str(), n["line"].as_u64(), n["col"].as_u64())
        {
            out.entry((f.to_string(), l, c)).or_default().push(i);
        }
    }
    out
}

/// The map's nodes declared at `span` (every instance of that element).
fn nodes_at(
    index: &std::collections::HashMap<(String, u64, u64), Vec<usize>>,
    sources: &crate::sources::Sources,
    span: contract_syntax::Span,
) -> Vec<usize> {
    let file = sources.path(span).to_string_lossy().to_string();
    index
        .get(&(file, u64::from(span.line), u64::from(span.col)))
        .cloned()
        .unwrap_or_default()
}

/// A node's site, located as the map has it (`sitesAt` outlines by it).
fn site_of(site: usize, node: &Value, row: &str) -> Value {
    json!({"site": site, "row": row, "file": node["file"], "line": node["line"], "col": node["col"], "end_col": node["end_col"], "component": node["component"], "chain": node["chain"]})
}

/// Where the app draws an icon (LLP 1121 stage 9 C2): each plan node of
/// an `image` whose source ([`strings`]) is one of `reads`, each a source
/// (`symbol:icon.reply`) with the reference it reads through (`via`), or a
/// component's param, by identity, read as `param.<name>` or as the loader
/// rewrote it (`params`: each `(identity, name, token, via)`). A source
/// that is the thing asked about has no `via`. `row` is `symbol`: the
/// image's source, not a style row.
pub fn icon_sites(
    app: &std::path::Path,
    map: &Value,
    reads: &[(String, Option<String>)],
    params: &[(String, String, String, Option<String>)],
) -> Result<Vec<Value>, String> {
    let (file, sources) = elements(app, map)?;
    let index = by_span(map);
    let nodes = map["nodes"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for c in &file.components {
        let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
        walk(&c.view, &mut |n| {
            let contract_syntax::Node::Element {
                tag,
                positional,
                span,
                ..
            } = n
            else {
                return;
            };
            if tag != "image" {
                return;
            }
            let mut said = Vec::new();
            for e in positional {
                strings(e, &mut said);
            }
            let read = reads
                .iter()
                .find(|(source, _)| said.contains(source))
                .map(|(_, via)| via.clone())
                .or_else(|| {
                    params
                        .iter()
                        .find(|(id, name, token, _)| {
                            *id == identity
                                && said.iter().any(|s| {
                                    *s == format!("symbol:param.{name}")
                                        || *s == format!("symbol:{token}")
                                })
                        })
                        .map(|(.., via)| via.clone())
                });
            let Some(via) = read else {
                return;
            };
            for site in nodes_at(&index, &sources, *span) {
                let mut v = site_of(site, &nodes[site], "symbol");
                if let Some(via) = &via {
                    v["via"] = json!({"token": via});
                }
                out.push(v);
            }
        });
    }
    out.sort_by_key(|v| v["site"].as_u64());
    Ok(out)
}

/// A motion token's literal sites ([`motion_sites`]) by instance (LLP 1121
/// stage 9 C2): each plan node declared at the element whose attribute's
/// value holds the literal (in any part of it: a condition's arm, a `+`'s
/// side), or, for a literal on a `style`'s row, each node whose row the map
/// says is written at that attribute (D1's `at`), is a site, located as
/// the map has it, the literal kept as `literal`. A literal no node is
/// declared or written at (a component no one uses, a component
/// argument's, which its expansion writes into its component's row) keeps
/// `site: null`.
pub fn motion_instances(
    app: &std::path::Path,
    literals: Vec<Value>,
    map: &Value,
) -> Result<Vec<Value>, String> {
    let (file, sources) = elements(app, map)?;
    let index = by_span(map);
    let nodes = map["nodes"].as_array().cloned().unwrap_or_default();
    let dir = app.parent().unwrap_or(app);
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    // Each literal inside an element's attribute: its place, and the
    // element's span.
    // Each literal inside an element's attribute, by its file and line.
    let mut held: std::collections::HashMap<
        (String, u32),
        Vec<(contract_syntax::Span, contract_syntax::Span)>,
    > = Default::default();
    for c in &file.components {
        walk(&c.view, &mut |n| {
            if let contract_syntax::Node::Element { attrs, span, .. } = n {
                for a in attrs {
                    let mut spans = Vec::new();
                    literal_spans(&a.value, &mut spans);
                    for v in spans {
                        let key = (sources.path(v).to_string_lossy().to_string(), v.line);
                        held.entry(key).or_default().push((v, *span));
                    }
                }
            }
        });
    }
    let mut out = Vec::new();
    for lit in literals {
        let file = lit["file"].as_str().unwrap_or("");
        let paths = [
            dir.join(file).to_string_lossy().to_string(),
            canonical.join(file).to_string_lossy().to_string(),
            file.to_string(),
        ];
        let (line, col) = (
            lit["line"].as_u64().unwrap_or(0),
            lit["col"].as_u64().unwrap_or(0),
        );
        let element = paths.iter().find_map(|p| {
            held.get(&(p.clone(), line as u32))?
                .iter()
                .find(|(v, _)| u64::from(v.col) <= col && col < u64::from(v.end_col))
                .map(|(_, span)| *span)
        });
        // A node declared there that binds the row's kind (a button's text
        // child shares its span and binds none).
        let family = lit["row"]
            .as_str()
            .unwrap_or("")
            .split('_')
            .next()
            .unwrap_or("")
            .to_string();
        let binds = |site: &usize| {
            nodes[*site]["bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|b| {
                    b["row"]
                        .as_str()
                        .is_some_and(|r| r.split('_').next() == Some(family.as_str()))
                })
        };
        let mut found: Vec<usize> = Vec::new();
        if lit["row"].is_string() {
            if let Some(span) = element {
                found = nodes_at(&index, &sources, span)
                    .into_iter()
                    .filter(binds)
                    .collect();
            } else {
                // A style's row: the nodes whose binding is written at its
                // attribute, on the literal's line, before it, by its name.
                let attr = lit["row"].as_str().unwrap_or("").replace('_', "-");
                for (site, node) in nodes.iter().enumerate() {
                    let hit = node["bindings"].as_array().into_iter().flatten().any(|b| {
                        let at = &b["at"];
                        b["attr"].as_str() == Some(attr.as_str())
                            && at["line"].as_u64() == Some(line)
                            && at["col"].as_u64().is_some_and(|c| c <= col)
                            && at["file"]
                                .as_str()
                                .is_some_and(|f| paths.iter().any(|p| p == f))
                    });
                    if hit {
                        found.push(site);
                    }
                }
            }
        }
        if found.is_empty() {
            out.push(lit);
            continue;
        }
        let literal = json!({"file": lit["file"], "line": lit["line"], "col": lit["col"], "end_col": lit["end_col"]});
        for site in found {
            let mut v = site_of(site, &nodes[site], lit["row"].as_str().unwrap_or(""));
            v["literal"] = literal.clone();
            if lit.get("via").is_some() {
                v["via"] = lit["via"].clone();
            }
            out.push(v);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_matched_whole() {
        assert!(names(
            "linear-gradient(90deg, accent.fill, surface.card)",
            "accent.fill"
        ));
        assert!(names("accent.fill", "accent.fill"));
        assert!(!names("accent.fill-2", "accent.fill"));
        assert!(!names("my.accent.fill", "accent.fill"));
        assert!(names("drop-shadow(0 0 4px accent.soft)", "accent.soft"));
    }

    #[test]
    fn aliases_close_under_naming() {
        let table = [
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["c".to_string()]),
            ("d".to_string(), vec![]),
        ];
        let it = table.iter().map(|(n, v)| (n.as_str(), v.as_slice()));
        let found = aliases("c", it);
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["a", "b"]);
    }
}
