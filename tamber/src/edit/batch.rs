//! Many `SetToken` and `SetDial` operations applied with one parse of the
//! design (re-import's transaction, LLP 1121 stage 10b): the text each would
//! make stepped one at a time ([`super::step`] parses the whole design for
//! each), for a fraction of the cost. Each such operation changes only its
//! own token's lines or its dial's line, and reads nothing else, so the
//! targets are taken bottom-up (an edit moves only the lines below it, which
//! are done) and one target's operations in their order, its new `when`
//! lines entered in the parse as the parse would read them. Anything else
//! (another operation, a refusal anywhere) is `None`: the caller steps them.

use super::{role_key, same_conds, valid, Doc, Op};
use crate::syntax::{At, When};
use crate::{Diagnostic, Vocab};
use std::collections::{HashMap, HashSet};

/// A design's tokens by exact name and by the role a name spells, each the
/// first (as [`super::token_index`] finds them: the exact name, else the role).
pub(super) struct Index {
    by_name: HashMap<String, usize>,
    by_role: HashMap<String, usize>,
}

impl Index {
    fn new(design: &crate::Design, roles: &[(String, bool)]) -> Index {
        let (mut by_name, mut by_role) = (HashMap::new(), HashMap::new());
        for (i, t) in design.tokens.iter().enumerate() {
            by_name.entry(t.name.value.clone()).or_insert(i);
            if let Some(k) = role_key(roles, &t.name.value) {
                by_role.entry(k).or_insert(i);
            }
        }
        Index { by_name, by_role }
    }

    pub(super) fn token(
        &self,
        roles: &[(String, bool)],
        name: &str,
    ) -> Result<usize, Vec<Diagnostic>> {
        self.by_name
            .get(name)
            .or_else(|| self.by_role.get(&role_key(roles, name)?))
            .copied()
            .ok_or_else(|| super::refuse(format!("no token `{name}`")))
    }
}

/// What an operation edits: a token (by index) or a dial (by name).
#[derive(PartialEq, Eq, Hash, Clone)]
enum Target {
    Token(usize),
    Dial(String),
}

/// Many `add-token`s with no `before` (an import's tokens) with one parse:
/// the first stepped as ever, each after it on the lines right after the one
/// before, where stepping it would land (after the last token, the one just
/// added). The text, and how many of `ops` it took: those before the first
/// that is no such operation, or that stepping would refuse for a field or a
/// name taken (the caller steps that one, with stepping's words). `None`
/// when fewer than two would be taken.
pub(super) fn added(src: &str, ops: &[Op], vocab: &Vocab) -> Option<(String, usize)> {
    let design = crate::parse(src).ok()?;
    let mut names: HashSet<&str> = design
        .tokens
        .iter()
        .map(|t| t.name.value.as_str())
        .collect();
    let taken = ops
        .iter()
        .take_while(|op| match op {
            Op::AddToken {
                name, before: None, ..
            } => valid(op).is_ok() && names.insert(name.as_str()),
            _ => false,
        })
        .count();
    if taken < 2 {
        return None;
    }
    let mut doc = Doc::new(src);
    doc.roles = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let indents = doc.indents(&design);
    doc.apply(&design, &ops[0]).ok()?;
    let mut at = doc.added_at?;
    for op in &ops[1..taken] {
        let Op::AddToken {
            name,
            kind,
            value,
            meta,
            whens,
            comment,
            ..
        } = op
        else {
            return None;
        };
        for l in super::added_lines(indents, name, kind, value, meta, whens, comment) {
            let l = doc.ended(l);
            doc.lines.insert(at, l);
            at += 1;
        }
    }
    Some((doc.text(), taken))
}

/// How many of `ops` (values and dials) lead with a target `src` has: the
/// first that names none is the one stepping refuses (round 3's re-check: a
/// held import recomputed on a design that lost a token stepped every op
/// before it, a parse each).
pub(super) fn present(src: &str, ops: &[Op], vocab: &Vocab) -> usize {
    let Ok(design) = crate::parse(src) else {
        return 0;
    };
    let roles: Vec<(String, bool)> = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let index = Index::new(&design, &roles);
    let dials: HashSet<&str> = design.dials.iter().map(|d| d.name.value.as_str()).collect();
    ops.iter()
        .take_while(|op| match op {
            Op::SetToken { name, .. } => index.token(&roles, name).is_ok(),
            Op::SetDial { name, .. } => dials.contains(name.as_str()),
            _ => false,
        })
        .count()
}

/// An import's values and new tokens interleaved (`set-token`s on the
/// tokens it has, `add-token`s with no `before` for those it lacks, in its
/// tokens' order): the new tokens with one parse ([`added`]), then the values
/// with one ([`stepped`]). The same text as stepping them in order: a value
/// edits only its own token's or dial's lines, all above where a new token
/// lands, and a new token lands after the last token's lines whatever a value
/// did to them. `None` when either kind is missing, when a value names a new
/// token (by its name or its role), or on any refusal: the caller steps them.
pub(super) fn mixed(src: &str, ops: &[Op], vocab: &Vocab) -> Option<String> {
    let (adds, sets): (Vec<Op>, Vec<Op>) = ops
        .iter()
        .cloned()
        .partition(|op| matches!(op, Op::AddToken { .. }));
    if adds.len() < 2 || sets.len() < 2 {
        return None;
    }
    let roles: Vec<(String, bool)> = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let key = |n: &str| role_key(&roles, n).unwrap_or_else(|| n.to_string());
    let new: HashSet<String> = adds
        .iter()
        .filter_map(|op| match op {
            Op::AddToken { name, .. } => Some(key(name)),
            _ => None,
        })
        .collect();
    let names_new = sets
        .iter()
        .any(|op| matches!(op, Op::SetToken { name, .. } if new.contains(&key(name))));
    if names_new {
        return None;
    }
    // A value giving a token ahead of the first one with `when` lines its
    // first can set the indent the new tokens' `when` lines take (Astra's
    // re-checks): stepped as written then, and only then.
    let add_whens = adds
        .iter()
        .any(|op| matches!(op, Op::AddToken { whens, .. } if !whens.is_empty()));
    if add_whens {
        let design = crate::parse(src).ok()?;
        let index = Index::new(&design, &roles);
        let first = design.tokens.iter().position(|t| !t.whens.is_empty());
        let moves = sets.iter().any(|op| match op {
            Op::SetToken { name, when, .. } if !when.is_empty() => index
                .token(&roles, name)
                .is_ok_and(|i| design.tokens[i].whens.is_empty() && first.is_none_or(|f| i < f)),
            _ => false,
        });
        if moves {
            return None;
        }
    }
    let (text, taken) = added(src, &adds, vocab)?;
    if taken != adds.len() {
        return None;
    }
    stepped(&text, &sets, vocab)
}

/// `ops` stepped on `src` with one parse, or `None` for the caller to step
/// them one at a time (an operation that is neither kind, or any refusal:
/// which one, and its words, are the stepping's).
pub(super) fn stepped(src: &str, ops: &[Op], vocab: &Vocab) -> Option<String> {
    if !ops
        .iter()
        .all(|op| matches!(op, Op::SetToken { .. } | Op::SetDial { .. }))
    {
        return None;
    }
    ops.iter().try_for_each(valid).ok()?;
    let mut design = crate::parse(src).ok()?;
    let mut doc = Doc::new(src);
    doc.roles = vocab
        .roles
        .iter()
        .map(|r| (r.name.clone(), r.system))
        .collect();
    let index = Index::new(&design, &doc.roles);
    // Each target's operations, in their order; the targets bottom-up.
    let mut groups: Vec<(Target, Vec<&Op>)> = Vec::new();
    let mut at: HashMap<Target, usize> = HashMap::new();
    for op in ops {
        let target = match op {
            Op::SetToken { name, .. } => Target::Token(index.token(&doc.roles, name).ok()?),
            Op::SetDial { name, .. } => Target::Dial(name.clone()),
            _ => return None,
        };
        let k = *at.entry(target.clone()).or_insert_with(|| {
            groups.push((target, Vec::new()));
            groups.len() - 1
        });
        groups[k].1.push(op);
    }
    let first_line = |t: &Target| match t {
        Target::Token(i) => Some(design.tokens[*i].name.line),
        Target::Dial(n) => design
            .dials
            .iter()
            .find(|d| d.name.value == *n)
            .map(|d| d.name.line),
    };
    let mut order: Vec<(u32, usize)> = Vec::with_capacity(groups.len());
    for (k, (t, _)) in groups.iter().enumerate() {
        order.push((first_line(t)?, k));
    }
    order.sort_by(|a, b| b.cmp(a));
    doc.index = Some(index);
    for (_, k) in order {
        let (target, group) = &groups[k];
        for op in group {
            doc.apply(&design, op).ok()?;
            // Stepping parses the whole text again before the next operation:
            // a line this one wrote that does not parse is refused there. The
            // lines it can write are its target's, which parse as they would
            // in the text when parsed alone in their block.
            let (head, from, to) = match target {
                Target::Dial(n) => {
                    let line = design.dials.iter().find(|d| d.name.value == *n)?.name.line;
                    ("dials", line, line)
                }
                Target::Token(i) => {
                    let t = &design.tokens[*i];
                    let last = t.whens.last().map_or(t.name.line, |w| w.line);
                    // (Its new `when` line is entered below, one past the last.)
                    let new = matches!(op, Op::SetToken { when, .. } if !when.is_empty() && !t.whens.iter().any(|w| same_conds(&w.conds, when)));
                    ("theme", t.name.line, last + u32::from(new))
                }
            };
            let alone = format!(
                "tamber t\n\n{head}\n{}\n",
                doc.lines[from as usize - 1..to as usize].join("\n")
            );
            crate::parse(&alone).ok()?;
            // A new `when` line, entered as the parse would read it: at its
            // place among the token's, the lines after it moved down one.
            if let (
                Target::Token(i),
                Op::SetToken {
                    when, value, at, ..
                },
            ) = (target, op)
            {
                let t = &mut design.tokens[*i];
                if !when.is_empty() && !t.whens.iter().any(|w| same_conds(&w.conds, when)) {
                    let index = at.unwrap_or(t.whens.len()).min(t.whens.len());
                    let after = match index {
                        0 => t.name.line,
                        k => t.whens[k - 1].line,
                    };
                    for w in &mut t.whens[index..] {
                        w.line += 1;
                    }
                    let line = after + 1;
                    let indent = doc.parts(line).0;
                    let value = At {
                        value: value.trim().to_string(),
                        line,
                        col: 1,
                    };
                    t.whens.insert(
                        index,
                        When {
                            conds: when.clone(),
                            value,
                            line,
                            col: indent as u32 + 1,
                        },
                    );
                }
            }
        }
    }
    Some(doc.text())
}

#[cfg(test)]
mod tests {
    use super::super::stepped as one_at_a_time;
    use crate::{Op, Role, Vocab};

    /// The schema's colour roles (their names and kinds are what a step reads).
    fn vocab() -> Vocab {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../kernel/tables/schema.json")).unwrap();
        let mut system = true;
        let roles = schema["colors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let name = row[0].as_str().unwrap().to_string();
                system &= name.starts_with(|c: char| c.is_ascii_uppercase());
                Role {
                    name,
                    system,
                    fallback: ["#000".into(), "#fff".into()],
                    accent: false,
                }
            })
            .collect();
        Vocab {
            roles,
            ..Vocab::default()
        }
    }

    /// A small generator, the same sequence every run.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 33) % n as u64) as usize
        }
    }

    #[test]
    fn the_batch_steps_as_one_at_a_time_does_over_fuzzed_operations() {
        let vocab = vocab();
        let designs = [
            include_str!("../../../apps/theme-fixture/app.tamber"),
            include_str!("../../tests/corpus/bluesky.tamber"),
        ];
        let (mut batched, mut fell_back) = (0, 0);
        for (d, src) in designs.iter().enumerate() {
            let design = crate::parse(src).unwrap();
            let names: Vec<String> = design.tokens.iter().map(|t| t.name.value.clone()).collect();
            let dials: Vec<String> = design.dials.iter().map(|d| d.name.value.clone()).collect();
            let axes: Vec<(String, Vec<String>)> = design
                .axes
                .iter()
                .map(|a| (a.name.value.clone(), a.values.clone()))
                .collect();
            let mut rng = Lcg(d as u64 + 7);
            for case in 0..300 {
                let n = 1 + rng.next(40);
                // One list in three has one operation the batch refuses or
                // does not take, somewhere in it.
                let odd = (rng.next(3) == 0).then(|| (rng.next(n), 1 + rng.next(3)));
                let mut ops = Vec::new();
                for k in 0..n {
                    let pick = match odd {
                        Some((at, kind)) if at == k => kind,
                        _ => 4 + rng.next(16) * usize::from(rng.next(16) != 0),
                    };
                    if pick == 4 && !dials.is_empty() {
                        ops.push(Op::SetDial {
                            name: dials[rng.next(dials.len())].clone(),
                            value: format!("{}", rng.next(9) + 1),
                        });
                        continue;
                    }
                    // Now and then a name no token has, a value no line takes,
                    // or an operation the batch does not take: the fallback.
                    let name = match pick {
                        1 => "no.such.token".to_string(),
                        _ => names[rng.next(names.len())].clone(),
                    };
                    if pick == 2 {
                        ops.push(Op::RemoveToken { name });
                        continue;
                    }
                    let mut when = Vec::new();
                    for (a, values) in &axes {
                        if rng.next(3) == 0 {
                            when.push((a.clone(), values[rng.next(values.len())].clone()));
                        }
                    }
                    let value = match pick {
                        3 => ["#fff // a comment", "x=y z"][rng.next(2)].to_string(),
                        _ => format!("#{:06x}", rng.next(0xff_ffff)),
                    };
                    let at = (rng.next(4) == 0).then(|| rng.next(3));
                    ops.push(Op::SetToken {
                        name,
                        when,
                        value,
                        at,
                        comment: None,
                    });
                }
                let want = one_at_a_time(src, &ops, &vocab);
                match super::stepped(src, &ops, &vocab) {
                    Some(text) => {
                        batched += 1;
                        assert_eq!(Ok(text), want, "design {d}, case {case}: {ops:?}");
                    }
                    None => fell_back += 1,
                }
                assert_eq!(
                    super::super::stepped_all(src, &ops, &vocab),
                    want,
                    "design {d}, case {case}"
                );
            }
        }
        // Both paths ran: most lists batch, the seeded refusals step.
        assert!(
            batched > 100 && fell_back > 100,
            "{batched} batched, {fell_back} stepped"
        );
    }

    #[test]
    fn a_line_that_would_not_parse_is_refused_even_when_a_later_operation_rewrites_it() {
        // `x=y z` is one line and no comment, so `valid` takes it; the dial
        // line it writes does not parse, which stepping finds as it parses
        // before the second operation, and the batch must not skip.
        let src = "tamber t\n\ndials\n  d: number = 1\n\ntheme\n  a = #000000\n";
        let ops = [
            Op::SetDial {
                name: "d".into(),
                value: "x=y z".into(),
            },
            Op::SetDial {
                name: "d".into(),
                value: "2".into(),
            },
        ];
        let vocab = Vocab::default();
        assert!(one_at_a_time(src, &ops, &vocab).is_err());
        assert_eq!(super::stepped(src, &ops, &vocab), None);
        assert_eq!(
            super::super::stepped_all(src, &ops, &vocab),
            one_at_a_time(src, &ops, &vocab)
        );
    }

    /// New tokens with one parse land where stepping each would put them:
    /// designs with and without a `theme` block, closing comments, blank
    /// lines, `when` lines, no final newline, and tokens with kinds,
    /// metadata, `when` lines and comments.
    #[test]
    fn new_tokens_with_one_parse_land_where_stepping_puts_them() {
        let designs = [
            "tamber t\n",
            "tamber t\n\ntheme\n",
            "tamber t\n\ntheme\n  ink = #111111\n",
            "tamber t\n\ntheme\n  ink = #111111",
            "tamber t\n\naxes\n  tone = a | b\n\ntheme\n    ink = #111111\n        when tone=b: #222222\n    // the end\n\nicons\n  icon.a = icon(sf \"house\")\n",
            "tamber t\n\ntheme\n  ink = #111111  // a note\n\n  // closing\n",
            "tamber t\r\n\r\ntheme\r\n  ink = #111111\r\n",
        ];
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for design in designs {
            for _ in 0..40 {
                let n = 2 + next(6);
                let ops: Vec<Op> = (0..n)
                    .map(|k| Op::AddToken {
                        name: format!("new.t{k}"),
                        kind: (next(3) == 0).then(|| "color".to_string()),
                        value: format!("#{:06x}", next(0xffffff)),
                        meta: match next(3) {
                            0 => vec!["label=\"A b\"".into()],
                            _ => Vec::new(),
                        },
                        whens: match (next(3), design.contains("tone")) {
                            (0, true) => {
                                vec![(vec![("tone".into(), "b".into())], "#000000".into(), None)]
                            }
                            _ => Vec::new(),
                        },
                        comment: (next(4) == 0).then(|| "// added".to_string()),
                        before: None,
                    })
                    .collect();
                assert_eq!(
                    super::added(design, &ops, &vocab()),
                    Some((one_at_a_time(design, &ops, &vocab()).unwrap(), ops.len())),
                    "{design:?} {ops:?}"
                );
            }
        }
    }

    /// A new token whose `when` line is the same text as the line it lands
    /// after keeps it (Astra's round 2: a suffix match put the next token
    /// between them).
    #[test]
    fn a_new_tokens_when_line_like_the_last_one_stays_its_own() {
        let design = "tamber t\n\naxes\n  tone = a | b\n\ntheme\n  old = #111111\n    when tone=b: #222222\n";
        let ops = vec![
            Op::AddToken {
                name: "first".into(),
                kind: None,
                value: "#111111".into(),
                meta: Vec::new(),
                whens: vec![(vec![("tone".into(), "b".into())], "#222222".into(), None)],
                comment: None,
                before: None,
            },
            Op::AddToken {
                name: "second".into(),
                kind: None,
                value: "#333333".into(),
                meta: Vec::new(),
                whens: Vec::new(),
                comment: None,
                before: None,
            },
        ];
        let stepped = one_at_a_time(design, &ops, &vocab()).unwrap();
        assert_eq!(
            super::added(design, &ops, &vocab()),
            Some((stepped.clone(), 2))
        );
        assert!(
            stepped.ends_with("  first = #111111\n    when tone=b: #222222\n  second = #333333\n"),
            "{stepped}"
        );
    }

    /// Values and new tokens interleaved (an import's, in its tokens' order)
    /// give stepping's text when the new tokens go first (Astra's round 3).
    #[test]
    fn interleaved_values_and_new_tokens_land_as_stepping_puts_them() {
        let designs = [
            "tamber t\n\ndials\n  d: number = 0.1  in 0..1\n\naxes\n  tone = a | b\n\ntheme\n  t0 = #111111\n  t1 = #111111\n    when tone=b: #222222\n  t2 = calc(d * 2)\n",
            "tamber t\n\naxes\n  tone = a | b\n\ntheme\n  t0 = #111111\n  t1 = #111111\n  t2 = #111111\n  // closing\n",
            "tamber t\n\naxes\n  tone = a | b\n\ntheme\n  t0 = #111111\n  t1 = #111111\n  t2 = #111111",
            // (Astra's re-check: `t1`'s six-space `when`, which a new token's
            // follows until `t0` gains one of four)
            "tamber t\n\naxes\n  tone = a | b\n\ntheme\n  t0 = #111111\n  t1 = #111111\n      when tone=b: #222222\n  t2 = #111111\n",
        ];
        let mut seed = 0x1234_5678_9abc_def1_u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let mut batched = 0;
        for design in designs {
            for _ in 0..200 {
                let mut ops = Vec::new();
                for k in 0..(4 + next(8)) {
                    ops.push(match next(3) {
                        0 => Op::AddToken {
                            name: format!("new.n{k}"),
                            kind: None,
                            value: format!("#{:06x}", next(0xffffff)),
                            meta: Vec::new(),
                            whens: match next(2) {
                                0 => vec![(
                                    vec![("tone".into(), "b".into())],
                                    "#000000".into(),
                                    None,
                                )],
                                _ => Vec::new(),
                            },
                            comment: None,
                            before: None,
                        },
                        1 => Op::SetToken {
                            name: format!("t{}", next(3)),
                            when: match next(2) {
                                0 => vec![("tone".into(), "b".into())],
                                _ => Vec::new(),
                            },
                            value: format!("#{:06x}", next(0xffffff)),
                            at: None,
                            comment: None,
                        },
                        _ => Op::SetDial {
                            name: "d".into(),
                            value: format!("0.{}", next(9)),
                        },
                    });
                }
                let stepped = one_at_a_time(design, &ops, &vocab()).ok();
                if let Some(text) = super::mixed(design, &ops, &vocab()) {
                    batched += 1;
                    assert_eq!(Some(text), stepped, "{design:?} {ops:?}");
                }
            }
        }
        assert!(batched >= 30, "{batched}");
    }
}
