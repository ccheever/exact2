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
/// derive it loaded, and the slot it stored.
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
        let (mut pushed, mut loaded, mut stored) = (None, None, None);
        for operand in op.operands() {
            let ok = match operand {
                Operand::U8 | Operand::Enum(_) => r.u8().is_ok(),
                Operand::U16 => r.u16().is_ok(),
                Operand::U32 => r.u32().is_ok(),
                Operand::Idx(table) => r
                    .u32()
                    .map(|v| match op {
                        Opcode::LoadSlot | Opcode::LoadDerive => {
                            loaded = Some((*table == "slots", v as usize));
                        }
                        Opcode::StoreSlot => stored = Some(v as usize),
                        _ => {}
                    })
                    .is_ok(),
                Operand::F64 => r.f64().is_ok(),
                Operand::Str => r
                    .u32()
                    .map(|v| {
                        if op == Opcode::Str {
                            pushed = plan.strings.get(v as usize).map(String::as_str);
                        }
                    })
                    .is_ok(),
            };
            if !ok {
                return Reads::default();
            }
        }
        ops.push((op, pushed, loaded, stored));
    }
    // The stack: each value, the ops (strings, loads) it carries.
    let mut stack: Vec<Vec<usize>> = Vec::new();
    let mut compared = vec![false; ops.len()];
    // Each store: its slot and the run of ops whose value it stores.
    let mut runs: Vec<(usize, usize, usize)> = Vec::new();
    let mut start = 0;
    for (i, (op, pushed, loaded, stored)) in ops.iter().enumerate() {
        let pop = |stack: &mut Vec<Vec<usize>>| stack.pop().unwrap_or_default();
        use Opcode as O;
        match op {
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
            | O::LoadLocal
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
                pop(&mut stack);
                runs.push((stored.unwrap_or(usize::MAX), start, i));
                start = i + 1;
            }
            O::Pop | O::BindLocal | O::DropLocal => {
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
    let read = |range: std::ops::Range<usize>| {
        let mut out = Reads::default();
        for j in range {
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
    let mut out = read(0..ops.len());
    out.stores = runs
        .into_iter()
        .map(|(slot, a, b)| (slot, read(a..b)))
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
    let via_token = |s: &str| {
        if names(s, token) {
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
    let mut out = Vec::new();
    for (i, node) in plan.nodes.iter().enumerate() {
        let site = &map["nodes"][i];
        let range =
            node.bindings.start as usize..(node.bindings.start + node.bindings.len) as usize;
        for b in &plan.bindings[range] {
            if b.kind != BindingKind::Style {
                continue;
            }
            // A token is a colour (LLP 1121 until stage 6): only a row that
            // takes one reads it (`text-align: center` names no token).
            let Some(row) = exact_kernel::StyleId::from_bit(b.id as u32).filter(|r| colours(*r))
            else {
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
            if !via.is_null() {
                at["via"] = via;
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
