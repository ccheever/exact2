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
/// one (`accent.fill` is not in `accent.fill-2`).
pub fn names(text: &str, name: &str) -> bool {
    let part = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(part)
            && !text[i + name.len()..].chars().next().is_some_and(part)
    })
}

/// What a piece of code reads: the strings it pushes as values, and the
/// slots and derives it loads. A value compared (`mode == "brand"`) is no
/// read: one an `Eq`-like op takes next, or next but one after a single
/// push, is left out. Empty when the code does not walk whole.
#[derive(Default)]
struct Reads<'a> {
    strings: Vec<&'a str>,
    slots: Vec<usize>,
    derives: Vec<usize>,
}

/// One op of a walk: the string it pushed, and the slot (`true`) or derive
/// it loaded.
type Step<'a> = (Opcode, Option<&'a str>, Option<(bool, usize)>);

fn reads<'a>(plan: &'a Plan, code: &[u8]) -> Reads<'a> {
    let mut r = exact_plan::bytes::Reader::new(code);
    // Each op, with the string it pushed and the slot or derive it loaded.
    let mut ops: Vec<Step<'a>> = Vec::new();
    while !r.is_empty() {
        let Some(op) = r.u8().ok().and_then(Opcode::from_wire) else {
            return Reads::default();
        };
        let (mut pushed, mut loaded) = (None, None);
        for operand in op.operands() {
            let ok = match operand {
                Operand::U8 | Operand::Enum(_) => r.u8().is_ok(),
                Operand::U16 => r.u16().is_ok(),
                Operand::U32 => r.u32().is_ok(),
                Operand::Idx(table) => r
                    .u32()
                    .map(|v| {
                        if matches!(op, Opcode::LoadSlot | Opcode::LoadDerive) {
                            loaded = Some((*table == "slots", v as usize));
                        }
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
        ops.push((op, pushed, loaded));
    }
    let compares = |op: Option<&(Opcode, _, _)>| {
        op.is_some_and(|(o, _, _)| {
            matches!(
                o,
                Opcode::Eq | Opcode::Ne | Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge
            )
        })
    };
    let single = |o: &Opcode| {
        matches!(
            o,
            Opcode::Number
                | Opcode::Bool
                | Opcode::Str
                | Opcode::None
                | Opcode::LoadSlot
                | Opcode::LoadDerive
                | Opcode::LoadParam
                | Opcode::LoadItem
                | Opcode::LoadBound
                | Opcode::LoadLocal
        )
    };
    let mut out = Reads::default();
    for (i, (_, pushed, loaded)) in ops.iter().enumerate() {
        let compared = compares(ops.get(i + 1))
            || ops.get(i + 1).is_some_and(|(o, _, _)| single(o)) && compares(ops.get(i + 2));
        if let Some(s) = pushed.filter(|_| !compared) {
            out.strings.push(s);
        }
        match loaded.filter(|_| !compared) {
            Some((true, n)) => out.slots.push(n),
            Some((false, n)) => out.derives.push(n),
            None => {}
        }
    }
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
    // value or a write to it (an action's body) names it, a derive whose
    // body names it or loads one that may.
    let holds: Vec<Option<Value>> = plan
        .slots
        .iter()
        .enumerate()
        .map(|(n, slot)| {
            named(&reads(plan, plan.code(slot.init)))
                .or_else(|| {
                    plan.actions.iter().find_map(|a| {
                        let writes = &plan.writes
                            [a.writes.start as usize..(a.writes.start + a.writes.len) as usize];
                        writes
                            .iter()
                            .any(|w| w.slot.0 as usize == n)
                            .then(|| named(&reads(plan, plan.code(a.body))))
                            .flatten()
                    })
                })
                .map(|_| json!({"slot": plan.str(slot.name)}))
        })
        .collect();
    let derive_reads: Vec<Reads> = plan
        .derives
        .iter()
        .map(|d| reads(plan, plan.code(d.body)))
        .collect();
    let mut derived: Vec<Option<Value>> = vec![None; plan.derives.len()];
    loop {
        let mut moved = false;
        for (n, rd) in derive_reads.iter().enumerate() {
            if derived[n].is_some() {
                continue;
            }
            let found = named(rd)
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
            if found.is_some() {
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
