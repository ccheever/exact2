//! Re-import's proposal judged whole (LLP 1121 stage 10b; theme2's 13:16Z
//! ruling, option B): the tokens' operations applied together and checked
//! once, and only when that is refused, each token the design refuses found
//! by the lines its diagnostics are on (split in two, in the members' order, where
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

/// `members`' operations (each a token's, in the order re-import lists names: the export's) on `base`, judged
/// whole; a refused set's tokens found by the lines the design refuses (each
/// diagnostic on one of their lines), and where a diagnostic is on no line of
/// theirs (another token's, or the operations' own refusal), halved, the first
/// half before the second, each on what the taken ones before it made. `Err`
/// when `base` itself does not check (nothing could be taken). Checks: the
/// base's, then one when nothing is refused.
pub(crate) fn judged(
    base: &str,
    members: &[&[Op]],
    vocab: &Vocab,
) -> Result<Judged, Vec<Diagnostic>> {
    Ok(judged_on(base, crate::check(base, vocab)?, members, vocab))
}

/// [`judged`] on a base whose folding is known.
fn judged_on(base: &str, folding: Folding, members: &[&[Op]], vocab: &Vocab) -> Judged {
    let mut at = Judged {
        text: base.to_string(),
        folding,
        refused: Vec::new(),
    };
    let all: Vec<(usize, &[Op])> = members.iter().copied().enumerate().collect();
    take(&mut at, &all, vocab);
    at.refused.sort_by_key(|(k, _)| *k);
    at
}

/// `members` (each its place in the whole list, and its operations) on what
/// `at` holds. The members a refusal blames are taken out and the rest tried
/// again in a loop, not a call apiece (Astra's round 2 on the checker bound:
/// a chain of edits each exposed by the last recursed once per edit); only
/// halving recurses, as deep as the members halve.
fn take(at: &mut Judged, members: &[(usize, &[Op])], vocab: &Vocab) {
    let mut members: Vec<(usize, &[Op])> = members.to_vec();
    let mut round = 0;
    loop {
        // Each round after the first a check of the whole design again,
        // charged to the re-import's budget (theme2's ruling on the batch's
        // time; the first is the judging any batch does): past it, nothing
        // more is judged, and the re-import is refused in the budget's
        // words, applying nothing (`reimport_against`).
        if members.is_empty() {
            return;
        }
        let spent = round > 0 && crate::budget::spend(at.text.len() as u64).is_err();
        if spent || crate::budget::spent().is_some() {
            return;
        }
        round += 1;
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
                Err(all) => match blamed(&text, &all, &members) {
                    Some(blamed) => Err(blamed),
                    None => Ok(all),
                },
            },
            Err(all) => Ok(all),
        };
        match refused {
            // Each token a diagnostic is on refused with its own, the rest
            // tried again without them.
            Err(blamed) => {
                let out: std::collections::HashSet<usize> =
                    blamed.iter().map(|(b, _)| *b).collect();
                members.retain(|(k, _)| !out.contains(k));
                at.refused.extend(blamed);
            }
            Ok(all) if members.len() == 1 => {
                at.refused.push((members[0].0, all));
                return;
            }
            Ok(_) => {
                // Halving is rounds too.
                if crate::budget::spend(at.text.len() as u64).is_err() {
                    return;
                }
                let half = members.len() / 2;
                take(at, &members[..half], vocab);
                take(at, &members[half..], vocab);
                return;
            }
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
    // The member a diagnostic is on: its token's or its dial's; on no
    // member's line (an unchanged token refused for what a change made of a
    // token it reads), none, and the set is halved.
    let on = |line: u32| -> Option<usize> {
        let k = spans.partition_point(|(first, ..)| *first <= line);
        let name = match k.checked_sub(1).map(|k| spans[k]) {
            Some((_, last, name)) if line <= last => name,
            _ => dials.get(&line)?,
        };
        owner.get(name).copied()
    };
    let mut out: Vec<(usize, Vec<Diagnostic>)> = Vec::new();
    let mut at: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for d in all {
        let k = on(d.line)?;
        match at.get(&k) {
            Some(&j) => out[j].1.push(d.clone()),
            None => {
                at.insert(k, out.len());
                out.push((k, vec![d.clone()]));
            }
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

/// The proposal judged whole, in rounds: each pending token's lines applied
/// together on `current` and checked once ([`judged`]); a refused token is
/// reported with its own reason, and each other is asked (`again`) what the
/// folding makes of its lines, until a round adds no pin, refuses nothing
/// and clears no token's lines (`settle`).
/// A token that may be a dial's change (D14) is not judged here: its literal
/// lines are only a fallback for when the dial cannot take it, which is known
/// after (`export_dials::dials`), and beside the others they would stand for
/// an edit the proposal may not make (its dial unused, say). Their indices
/// are returned, for [`settle_alone`] to judge one on its own if its dial
/// falls back (theme2's ruling on the counts' round 2: the dial decided
/// first, so N tokens a dial takes are no N checks of the design).
pub(crate) fn rounds(
    names: &[String],
    planned: &mut [Planned],
    cur: &Folding,
    current: &str,
    vocab: &Vocab,
    mut again: impl FnMut(&String, Taken) -> Planned,
) -> Vec<usize> {
    let pending = |p: &Planned| match p {
        Planned::Kind(_, ops) => !ops.is_empty(),
        Planned::Colour { ops, why: None, .. } => !ops.is_empty(),
        _ => false,
    };
    let (mut together, mut alone) = (Vec::new(), Vec::new());
    for (i, p) in planned.iter().enumerate() {
        match p {
            Planned::Colour { dial: Some(_), .. } if pending(p) => alone.push(i),
            p if pending(p) => together.push(i),
            _ => {}
        }
    }
    let lines: Vec<&str> = current.split('\n').collect();
    settle(
        &together, names, planned, cur, current, &lines, vocab, &mut again,
    );
    alone
}

/// One token [`rounds`] left alone, judged on its own now that its dial falls
/// back: one check of the design, charged to the re-import's budget (theme2's
/// condition (3)); past it nothing is judged and the re-import is refused.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_alone(
    i: usize,
    names: &[String],
    planned: &mut [Planned],
    cur: &Folding,
    current: &str,
    vocab: &Vocab,
    mut again: impl FnMut(&String, Taken) -> Planned,
) {
    if crate::budget::spend(current.len() as u64).is_err() {
        return;
    }
    let lines: Vec<&str> = current.split('\n').collect();
    settle(
        &[i],
        names,
        planned,
        cur,
        current,
        &lines,
        vocab,
        &mut again,
    );
}

/// [`rounds`] for the tokens `set` (in that order).
#[allow(clippy::too_many_arguments)]
fn settle(
    set: &[usize],
    names: &[String],
    planned: &mut [Planned],
    cur: &Folding,
    current: &str,
    lines: &[&str],
    vocab: &Vocab,
    again: &mut impl FnMut(&String, Taken) -> Planned,
) {
    let mut pinned = vec![0u32; set.len()];
    let mut refused = vec![false; set.len()];
    // Until a round changes nothing: each change is a pin (three a token, a
    // fourth asked is its refusal), a refusal, or lines that leave (cleared,
    // or a token of another kind that has its values already), so the rounds
    // end; settled only on a round that moves nothing, never by a count.
    loop {
        // Past the re-import's budget nothing more is settled (it is refused).
        if crate::budget::spent().is_some() {
            return;
        }
        // Every member's lines are the proposal's, and every member is asked
        // what this round's folding makes of them: one settled in a round
        // before is asked again, as a pin added or a token dropped since may
        // move what its lines read. A refused one leaves.
        let members: Vec<usize> = (0..set.len())
            .filter(|&k| !refused[k])
            .filter(|&k| match &planned[set[k]] {
                Planned::Kind(_, ops) => !ops.is_empty(),
                Planned::Colour { ops, why, .. } => why.is_none() && !ops.is_empty(),
                _ => false,
            })
            .collect();
        if members.is_empty() {
            return;
        }
        let ops: Vec<&[Op]> = members
            .iter()
            .map(|&k| match &planned[set[k]] {
                Planned::Kind(_, ops) | Planned::Colour { ops, .. } => ops.as_slice(),
                _ => &[],
            })
            .collect();
        let at = judged_on(current, cur.clone(), &ops, vocab);
        // (past the budget, no member is asked again: Astra's round 2)
        if crate::budget::spent().is_some() {
            return;
        }
        // Whether this round changed the proposal (then the next one asks
        // again; one that changes nothing settles every member).
        let mut moved = false;
        for (m, all) in &at.refused {
            let first = all
                .first()
                .map_or(String::new(), |d| format!("{}: {}", d.id, d.message));
            let why = (
                String::new(),
                format!("the design would refuse it ({first})"),
            );
            let k = members[*m];
            match &mut planned[set[k]] {
                Planned::Colour { why: w, .. } => *w = Some(why),
                p => *p = Planned::Report(why.0, why.1),
            }
            refused[k] = true;
            moved = true;
        }
        let made: Vec<&str> = at.text.split('\n').collect();
        for &k in &members {
            let i = set[k];
            if refused[k] {
                continue;
            }
            let index = match &planned[i] {
                Planned::Kind(index, _) | Planned::Colour { index, .. } => *index,
                _ => continue,
            };
            let unchanged = block(lines, &cur.design.tokens[index])
                == block(&made, &at.folding.design.tokens[index]);
            let taken = Taken {
                folding: &at.folding,
                unchanged,
                rounds: pinned[k],
            };
            match (again(&names[i], taken), &mut planned[i]) {
                (Planned::Done, _) => {}
                (Planned::Cleared, Planned::Colour { ops, .. }) => {
                    ops.clear();
                    moved = true;
                }
                (Planned::Pins(pins), Planned::Colour { ops, .. }) => {
                    ops.extend(pins);
                    pinned[k] += 1;
                    moved = true;
                }
                (Planned::Report(block, why), Planned::Colour { why: w, .. }) => {
                    *w = Some((block, why));
                    refused[k] = true;
                    moved = true;
                }
                (Planned::Kind(index, ops), Planned::Kind(_, was)) => {
                    moved |= ops != *was;
                    planned[i] = Planned::Kind(index, ops);
                }
                (answer, p) => {
                    *p = answer;
                    refused[k] = true;
                    moved = true;
                }
            }
        }
        if !moved {
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

/// The digest of the design's own export, built inside a re-import's
/// budget (each context's tree charged as it is built), so a design too
/// large for one re-import is refused with the budget's message instead of
/// exported whole first; `None` for a design that does not check or export.
pub fn own_digest(current: &str, vocab: &Vocab) -> Result<Option<String>, Vec<Diagnostic>> {
    let Ok(f) = crate::check(current, vocab) else {
        return Ok(None);
    };
    let _work = crate::budget::Work::begin(crate::budget::REIMPORT);
    match crate::export::dtcg(&f, vocab) {
        Ok(text) => Ok(crate::export::digest(&text)),
        Err(all) if all.iter().any(|d| d.message.contains("expands past")) => Err(all),
        Err(_) => Ok(None),
    }
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

    /// A chain of members each exposed only when the last is refused (Astra's
    /// round 2 on the checker bound: one recursion per edit) is taken out in
    /// the loop: 1,500 rounds on a 2 MiB stack, each member refused with
    /// its own reason.
    #[test]
    fn a_chain_of_refusals_each_exposing_the_next_is_a_loop_not_a_recursion() {
        let n = 1_500;
        let mut base = String::from("tamber t\n\ntheme\n");
        for k in 0..n {
            base += &format!("  t{k} = 4px\n");
        }
        // Each a colour naming the next; the last refused on its own line
        // (no such axis), which leaves it a length, so the one before names
        // a length as a colour, and so on down.
        let mut members: Vec<Vec<Op>> = (0..n - 1)
            .map(|k| {
                vec![set(
                    &format!("t{k}"),
                    &format!("light-dark(t{}, #000000)", k + 1),
                )]
            })
            .collect();
        members.push(vec![Op::SetToken {
            name: format!("t{}", n - 1),
            when: vec![("nope".into(), "x".into())],
            value: "#ffffff".into(),
            at: None,
            comment: None,
        }]);
        let refused = std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || refused(&base, &members).0.len())
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(refused, n);
    }

    /// A chain past the re-import's budget stops being judged, and the
    /// budget says so (theme2's ruling: a named refusal, nothing applied).
    #[test]
    fn a_chain_past_the_budget_stops_and_the_budget_says_so() {
        let (base, members) = chain(5_000);
        let began = std::time::Instant::now();
        let spent = std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || {
                let _work = crate::budget::Work::begin(crate::budget::REIMPORT);
                let _ = refused(&base, &members);
                crate::budget::spent()
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(spent.is_some_and(|w| w.contains("expands past")));
        assert!(
            began.elapsed() < std::time::Duration::from_secs(60),
            "{:?}",
            began.elapsed()
        );
    }

    /// 17,000 edits with a few refused on their own lines take two rounds,
    /// well within the budget.
    #[test]
    fn many_edits_with_a_few_refusals_pass_within_the_budget() {
        let n = 17_000;
        let mut base = String::from("tamber t\n\ntheme\n");
        for k in 0..n {
            base += &format!("  t{k} = #111111\n");
        }
        let members: Vec<Vec<Op>> = (0..n)
            .map(|k| match k % 6_000 {
                0 => vec![Op::SetToken {
                    name: format!("t{k}"),
                    when: vec![("nope".into(), "x".into())],
                    value: "#222222".into(),
                    at: None,
                    comment: None,
                }],
                _ => vec![set(&format!("t{k}"), "#222222")],
            })
            .collect();
        let began = std::time::Instant::now();
        let _work = crate::budget::Work::begin(crate::budget::REIMPORT);
        let (refused, text) = refused(&base, &members);
        assert!(crate::budget::spent().is_none());
        assert_eq!(refused.len(), 3);
        assert_eq!(text.matches("#222222").count(), n - 3);
        assert!(
            began.elapsed() < std::time::Duration::from_secs(60),
            "{:?}",
            began.elapsed()
        );
    }

    /// Below the budget, the answers are the unbounded loop's.
    #[test]
    fn below_the_budget_the_answers_are_the_unbounded_loops() {
        for n in [2, 5, 13, 40] {
            let (base, members) = chain(n);
            let free = refused(&base, &members);
            let _work = crate::budget::Work::begin(crate::budget::REIMPORT);
            assert_eq!(refused(&base, &members), free, "{n}");
            assert!(crate::budget::spent().is_none());
        }
    }

    /// A chain of `n` members, each exposed only when the one after it is
    /// refused (the last refused on its own line).
    fn chain(n: usize) -> (String, Vec<Vec<Op>>) {
        let mut base = String::from("tamber t\n\ntheme\n");
        for k in 0..n {
            base += &format!("  t{k} = 4px\n");
        }
        let mut members: Vec<Vec<Op>> = (0..n - 1)
            .map(|k| {
                vec![set(
                    &format!("t{k}"),
                    &format!("light-dark(t{}, #000000)", k + 1),
                )]
            })
            .collect();
        members.push(vec![Op::SetToken {
            name: format!("t{}", n - 1),
            when: vec![("nope".into(), "x".into())],
            value: "#ffffff".into(),
            at: None,
            comment: None,
        }]);
        (base, members)
    }
}
