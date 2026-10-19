//! Re-import's proposal judged whole (LLP 1121 stage 10b; theme2's 13:16Z
//! ruling, option B): the tokens' operations applied together and checked
//! once, and only when that is refused, each token the design refuses found
//! by the lines its diagnostics are on (split in two in source order where
//! one is on no line of theirs), with its own reason. A proposal
//! is applied as the whole edit it is: a token refused alone but taken with
//! the others lands with them, and one taken alone but refused with the
//! others is found here. The same input gives the same split and the same
//! report. The cost is one check when nothing is refused, one more for each
//! set of tokens a check refuses on their own lines, and about two a halving
//! where one is refused for another token's line.

use crate::edit::stepped_all;
use crate::{Diagnostic, Folding, Op, Vocab};

/// What a judgement made: the text the taken tokens' operations make on the
/// base, its folding, and each refused token (by its place in `members`)
/// with the design's diagnostics.
pub(crate) struct Judged {
    pub(crate) text: String,
    pub(crate) folding: Folding,
    pub(crate) refused: Vec<(usize, Vec<Diagnostic>)>,
}

/// `members`' operations (each a token's, in source order) on `base`, judged
/// whole; a refused set's tokens found by the lines the design refuses (each
/// diagnostic on one of their lines), and where a diagnostic is on no line of
/// theirs (another token's, or the operations' own refusal), halved, the first
/// half before the second, each on what the taken ones before it made. `Err`
/// when `base` itself does not check (nothing could be taken).
pub(crate) fn judged(
    base: &str,
    members: &[&[Op]],
    vocab: &Vocab,
) -> Result<Judged, Vec<Diagnostic>> {
    let folding = crate::check(base, vocab)?;
    let mut at = Judged {
        text: base.to_string(),
        folding,
        refused: Vec::new(),
    };
    let all: Vec<(usize, &[Op])> = members.iter().copied().enumerate().collect();
    take(&mut at, &all, vocab);
    at.refused.sort_by_key(|(k, _)| *k);
    Ok(at)
}

/// `members` (each its place in the whole list, and its operations) on what
/// `at` holds.
fn take(at: &mut Judged, members: &[(usize, &[Op])], vocab: &Vocab) {
    if members.is_empty() {
        return;
    }
    let ops: Vec<Op> = members
        .iter()
        .flat_map(|(_, m)| m.iter().cloned())
        .collect();
    let refused = match stepped_all(&at.text, &ops, vocab) {
        Ok(text) => match crate::check(&text, vocab) {
            Ok(folding) => {
                at.text = text;
                at.folding = folding;
                return;
            }
            Err(all) => match blamed(&text, &all, members) {
                Some(blamed) => Err(blamed),
                None => Ok(all),
            },
        },
        Err(all) => Ok(all),
    };
    match refused {
        // Each token a diagnostic is on refused with its own, the rest tried
        // again without them.
        Err(blamed) => {
            let rest: Vec<(usize, &[Op])> = members
                .iter()
                .filter(|(k, _)| !blamed.iter().any(|(b, _)| b == k))
                .copied()
                .collect();
            at.refused.extend(blamed);
            take(at, &rest, vocab);
        }
        Ok(all) if members.len() == 1 => at.refused.push((members[0].0, all)),
        Ok(_) => {
            let half = members.len() / 2;
            take(at, &members[..half], vocab);
            take(at, &members[half..], vocab);
        }
    }
}

/// The members `all` is on, each with its diagnostics in order, when every
/// diagnostic is on a line of a member's token or dial in `text`; else `None`.
fn blamed(
    text: &str,
    all: &[Diagnostic],
    members: &[(usize, &[Op])],
) -> Option<Vec<(usize, Vec<Diagnostic>)>> {
    let design = crate::parse(text).ok()?;
    let mut owner: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (k, ops) in members {
        for op in *ops {
            if let Op::SetToken { name, .. } | Op::SetDial { name, .. } = op {
                owner.entry(name.as_str()).or_insert(*k);
            }
        }
    }
    // Each line's token, by the tokens' first lines (in order in the text).
    let spans: Vec<(u32, u32, &str)> = design
        .tokens
        .iter()
        .map(|t| {
            (
                t.name.line,
                t.whens.last().map_or(t.name.line, |w| w.line),
                t.name.value.as_str(),
            )
        })
        .collect();
    let dials: std::collections::HashMap<u32, &str> = design
        .dials
        .iter()
        .map(|d| (d.name.line, d.name.value.as_str()))
        .collect();
    // A token's index by name, and the tokens its lines name.
    let index: std::collections::HashMap<&str, usize> = design
        .tokens
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.value.as_str(), i))
        .collect();
    let names = |i: usize| -> Vec<usize> {
        let t = &design.tokens[i];
        std::iter::once(&t.value.value)
            .chain(t.whens.iter().map(|w| &w.value.value))
            .flat_map(|v| {
                v.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
            })
            .filter_map(|w| index.get(w).copied())
            .collect()
    };
    // The member a diagnostic is on: its token's or its dial's; for a token
    // no member is, the one member among what it reads, through tokens no
    // member is (an unchanged token refused for what a change made of a token
    // it reads). None: on no member, or on more than one.
    let on = |line: u32| -> Option<usize> {
        let k = spans.partition_point(|(first, ..)| *first <= line);
        let name = match k.checked_sub(1).map(|k| spans[k]) {
            Some((_, last, name)) if line <= last => name,
            _ => return owner.get(dials.get(&line)?).copied(),
        };
        if let Some(&m) = owner.get(name) {
            return Some(m);
        }
        let (mut seen, mut queue, mut found) =
            (std::collections::HashSet::new(), vec![index[name]], None);
        while let Some(i) = queue.pop() {
            for j in names(i) {
                if !seen.insert(j) {
                    continue;
                }
                match owner.get(design.tokens[j].name.value.as_str()) {
                    Some(&m) if found.is_some_and(|f| f != m) => return None,
                    Some(&m) => found = Some(m),
                    None => queue.push(j),
                }
            }
        }
        found
    };
    let mut out: Vec<(usize, Vec<Diagnostic>)> = Vec::new();
    for d in all {
        let k = on(d.line)?;
        match out.iter_mut().find(|(b, _)| *b == k) {
            Some((_, ds)) => ds.push(d.clone()),
            None => out.push((k, vec![d.clone()])),
        }
    }
    (!out.is_empty()).then_some(out)
}

/// A token's plan (`export::reimport_against`), and what a round's folding
/// made of its lines.
pub(crate) enum Planned {
    /// Not imported, and why (its block, its reason).
    Report(String, String),
    /// Nothing the file changes.
    Unchanged,
    /// A token of another kind (stage 6 G), at its place in the design, and
    /// its lines.
    Kind(usize, Vec<Op>),
    /// A colour, at its place in the design: its lines, why not, and a dial's
    /// change if it is one (D14).
    Colour {
        index: usize,
        ops: Vec<Op>,
        why: Option<(String, String)>,
        dial: Option<crate::export_dials::Candidate>,
    },
    /// A round: the design has its values already, so it has no lines.
    Cleared,
    /// A round: every context as the file has it.
    Done,
    /// A round: lines that pin the contexts the file left alone.
    Pins(Vec<Op>),
}

/// What a round's folding says to one token: the folding, whether its lines
/// came out as they were, and how many rounds have pinned it.
pub(crate) struct Taken<'a> {
    pub(crate) folding: &'a Folding,
    pub(crate) unchanged: bool,
    pub(crate) rounds: u32,
}

/// The proposal judged whole, in at most four rounds: each pending token's
/// lines applied together on `current` and checked once ([`judged`]); a
/// refused token is reported with its own reason, and each other is asked
/// (`again`) what the folding makes of its lines, until no token adds a pin.
pub(crate) fn rounds(
    names: &[String],
    planned: &mut [Planned],
    cur: &Folding,
    current: &str,
    vocab: &Vocab,
    mut again: impl FnMut(&String, Taken) -> Planned,
) {
    let mut pinned = vec![0u32; planned.len()];
    let mut done = vec![false; planned.len()];
    let lines: Vec<&str> = current.split('\n').collect();
    for _ in 0..4 {
        // Every pending token's lines are the proposal's, whether or not its
        // values are settled; those whose values are not are asked.
        let members: Vec<usize> = (0..planned.len())
            .filter(|&i| match &planned[i] {
                Planned::Kind(_, ops) => !ops.is_empty(),
                Planned::Colour { ops, why: None, .. } => !ops.is_empty(),
                _ => false,
            })
            .collect();
        if members.iter().all(|&i| done[i]) {
            return;
        }
        let ops: Vec<&[Op]> = members
            .iter()
            .map(|&i| match &planned[i] {
                Planned::Kind(_, ops) | Planned::Colour { ops, .. } => ops.as_slice(),
                _ => &[],
            })
            .collect();
        let Ok(at) = judged(current, &ops, vocab) else {
            return;
        };
        for (k, all) in &at.refused {
            let first = all
                .first()
                .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
            let why = (
                String::new(),
                format!("the design would refuse it ({first})"),
            );
            let i = members[*k];
            match &mut planned[i] {
                Planned::Colour { why: w, .. } => *w = Some(why),
                p => *p = Planned::Report(why.0, why.1),
            }
            done[i] = true;
        }
        let made: Vec<&str> = at.text.split('\n').collect();
        let mut more = false;
        for &i in &members {
            if done[i] {
                continue;
            }
            let index = match &planned[i] {
                Planned::Kind(index, _) | Planned::Colour { index, .. } => *index,
                _ => continue,
            };
            let unchanged = block(&lines, &cur.design.tokens[index])
                == block(&made, &at.folding.design.tokens[index]);
            let taken = Taken {
                folding: &at.folding,
                unchanged,
                rounds: pinned[i],
            };
            match (again(&names[i], taken), &mut planned[i]) {
                (Planned::Done, _) => done[i] = true,
                (Planned::Cleared, Planned::Colour { ops, .. }) => {
                    ops.clear();
                    done[i] = true;
                }
                (Planned::Pins(pins), Planned::Colour { ops, .. }) => {
                    ops.extend(pins);
                    pinned[i] += 1;
                    more = true;
                }
                (Planned::Report(block, why), Planned::Colour { why: w, .. }) => {
                    *w = Some((block, why));
                    done[i] = true;
                }
                (answer, p) => {
                    *p = answer;
                    done[i] = true;
                }
            }
        }
        if !more {
            return;
        }
    }
}

/// A token's lines: its name's to its last `when`'s.
fn block<'a>(lines: &[&'a str], t: &crate::syntax::Token) -> Vec<&'a str> {
    let last = t.whens.last().map_or(t.name.line, |w| w.line);
    lines
        .get(t.name.line as usize - 1..last as usize)
        .map(<[&str]>::to_vec)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::judged;
    use crate::{Op, Vocab};

    fn set(name: &str, value: &str) -> Op {
        Op::SetToken {
            name: name.into(),
            when: Vec::new(),
            value: value.into(),
            at: None,
            comment: None,
        }
    }

    /// Each refused member's place and its first diagnostic's id.
    fn refused(base: &str, members: &[Vec<Op>]) -> (Vec<(usize, &'static str)>, String) {
        let all: Vec<&[Op]> = members.iter().map(Vec::as_slice).collect();
        let at = judged(base, &all, &Vocab::default()).unwrap();
        (
            at.refused.iter().map(|(k, d)| (*k, d[0].id)).collect(),
            at.text,
        )
    }

    #[test]
    fn a_token_refused_alone_lands_with_the_tokens_that_make_it_whole() {
        // `a` naming `b` as a colour is refused while `b` is a length; with
        // `b`'s change beside it the proposal checks, and both land.
        let base = "tamber t\n\ntheme\n  a = #000000\n  b = 4px\n";
        let (refused, text) = refused(
            base,
            &[
                vec![set("a", "light-dark(b, #000000)")],
                vec![set("b", "#ff0000")],
            ],
        );
        assert_eq!(refused, []);
        assert!(
            text.contains("a = light-dark(b, #000000)") && text.contains("b = #ff0000"),
            "{text}"
        );
    }

    #[test]
    fn a_token_taken_alone_but_refused_with_the_others_is_found_with_its_own_reason() {
        // Each alone checks; together `a` names a length as a colour. The
        // refusal is on `a`'s line, so `a` is refused and `b` lands.
        let base = "tamber t\n\ntheme\n  a = #000000\n  b = #ff0000\n";
        let (refused, text) = refused(
            base,
            &[
                vec![set("a", "light-dark(b, #000000)")],
                vec![set("b", "4px")],
            ],
        );
        assert_eq!(refused, [(0, "tamber-kind")]);
        assert!(
            text.contains("a = #000000") && text.contains("b = 4px"),
            "{text}"
        );
    }

    #[test]
    fn every_refused_token_is_found_in_source_order_with_its_own_reason() {
        let base = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  a = #000000\n  b = #111111\n  c = light-dark(d, #000000)\n  d = #ff0000\n  e = #222222\n  f = light-dark(g, h)\n  g = #333333\n  h = #444444\n";
        let grey = |n: &str| Op::SetToken {
            name: n.into(),
            when: vec![("dark-style".into(), "grey".into())],
            value: "#333333".into(),
            at: None,
            comment: None,
        };
        // `a` and `e` refused on their own lines; `d` made a length refuses
        // `c`, a token no member is, which reads only `d` of the members;
        // `g` made one refuses `f`, which reads `g` and `h`: found by halving.
        let members = [
            vec![grey("a")],
            vec![set("b", "#444444")],
            vec![set("d", "4px")],
            vec![grey("e")],
            vec![set("g", "4px")],
            vec![set("h", "#555555")],
        ];
        let first = refused(base, &members);
        assert_eq!(
            first.0,
            [
                (0, "tamber-unknown-axis"),
                (2, "tamber-kind"),
                (3, "tamber-unknown-axis"),
                (4, "tamber-kind")
            ]
        );
        assert!(
            first.1.contains("b = #444444") && first.1.contains("h = #555555"),
            "{}",
            first.1
        );
        // The same input, the same report.
        assert_eq!(refused(base, &members), first);
    }
}
