//! What the CSS importers share once each declaration has its blocks: the
//! winner of each block (the last, as in CSS), each block's colour (a
//! literal, or a reference resolved where the block reads it), and the
//! tokens: a `light-dark()` of each theme's sides, `when` lines for themes.

use super::Mapped;
use std::collections::{BTreeMap, BTreeSet};

/// A block a declaration sets: its theme (`None` the default) and whether
/// it is the dark side.
pub type Target = (Option<String>, bool);

/// One declaration as read: its block (as the report names it), variable,
/// value and the blocks it sets.
pub type Decl = (String, String, String, Vec<Target>);

/// What became of a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fate {
    /// It wins a block and is a colour there.
    Mapped,
    /// Its variable would be the same token as this earlier one's.
    Collided(String),
    /// A later declaration wins every block it sets.
    Superseded,
    /// It wins a block but is a colour in none.
    NotColour,
    /// Its variable makes no token; it is read only through references.
    Aux,
    /// It is a colour, but its variable is no colour in this other block
    /// it is set in, so it makes no token (one side would wear the other's).
    Unresolved(Target),
}

/// Each variable's token name, first seen first; a later variable whose
/// name is taken collides with the first.
#[derive(Debug, Default)]
pub struct Names {
    /// Variable → token.
    pub names: BTreeMap<String, String>,
    /// The variables with tokens, in order.
    pub order: Vec<String>,
    /// Variable → the variable whose token name it would have.
    pub collided: BTreeMap<String, String>,
    /// Variables that make no token but may be read through a reference
    /// (Tailwind's backing variables): a reference to one is written as the
    /// colour it resolves to.
    pub aux: BTreeSet<String>,
}

/// Name every variable `decls` declare by `token`; `None` is a variable
/// with no token of its own.
pub fn name(decls: &[Decl], token: impl Fn(&str) -> Option<String>) -> Names {
    let mut out = Names::default();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    for (_, var, _, _) in decls {
        if out.names.contains_key(var) || out.collided.contains_key(var) || out.aux.contains(var) {
            continue;
        }
        let Some(t) = token(var) else {
            out.aux.insert(var.clone());
            continue;
        };
        match taken.get(&t) {
            Some(first) => {
                out.collided.insert(var.clone(), first.clone());
            }
            None => {
                taken.insert(t.clone(), var.clone());
                out.names.insert(var.clone(), t);
                out.order.push(var.clone());
            }
        }
    }
    out
}

/// A block as a report names it: `:root`, `.dark`, `.theme-x`,
/// `.dark .theme-x`.
pub fn block_name(t: &Target) -> String {
    match t {
        (None, false) => ":root".into(),
        (None, true) => ".dark".into(),
        (Some(theme), false) => format!(".theme-{theme}"),
        (Some(theme), true) => format!(".dark .theme-{theme}"),
    }
}

/// `var(--x)`'s name.
pub fn reference(value: &str) -> Option<&str> {
    value
        .trim()
        .strip_prefix("var(")?
        .strip_suffix(')')
        .map(str::trim)
        .filter(|n| n.starts_with("--") && !n.contains(','))
}

/// Where a block reads a variable it does not set, nearest first, as CSS
/// does: a dark theme block from the theme, then the default dark side.
fn chain(at: &Target) -> Vec<Target> {
    match at {
        (Some(t), true) => vec![
            (Some(t.clone()), true),
            (Some(t.clone()), false),
            (None, true),
            (None, false),
        ],
        (Some(t), false) => vec![(Some(t.clone()), false), (None, false)],
        (None, true) => vec![(None, true), (None, false)],
        (None, false) => vec![(None, false)],
    }
}

/// Whether `to` is reachable from `from` in `graph` by one step or more.
fn reaches(graph: &BTreeMap<String, BTreeSet<String>>, from: &str, to: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<&str> = graph
        .get(from)
        .map_or(Vec::new(), |n| n.iter().map(String::as_str).collect());
    while let Some(n) = stack.pop() {
        if n == to {
            return true;
        }
        if seen.insert(n) {
            stack.extend(graph.get(n).into_iter().flatten().map(String::as_str));
        }
    }
    false
}

/// The read of the declarations: who wins each block, and what each block
/// sees.
struct Read<'a> {
    decls: &'a [Decl],
    winners: BTreeMap<(Target, String), usize>,
    names: &'a Names,
    literal: &'a dyn Fn(&str) -> Option<String>,
}

impl Read<'_> {
    /// A colour as a block sees `var`: the winner nearest it, never through
    /// a cycle. As written (a reference a token's name) and resolved.
    fn seen(&self, at: &Target, var: &str, visiting: &mut Vec<String>) -> Option<(String, String)> {
        let k = chain(at)
            .iter()
            .find_map(|c| self.winners.get(&(c.clone(), var.to_string())))?;
        self.colour_of(&self.decls[*k].2, at, visiting)
    }

    fn colour_of(
        &self,
        value: &str,
        at: &Target,
        visiting: &mut Vec<String>,
    ) -> Option<(String, String)> {
        if let Some(c) = (self.literal)(value) {
            return Some((c.clone(), c));
        }
        let r = reference(value)?;
        let aux = self.names.aux.contains(r);
        if visiting.iter().any(|v| v == r) || !(aux || self.names.names.contains_key(r)) {
            return None;
        }
        visiting.push(r.to_string());
        let found = self.seen(at, r, visiting);
        visiting.pop();
        found.map(|(_, resolved)| match aux {
            true => (resolved.clone(), resolved),
            false => (self.names.names[r].clone(), resolved),
        })
    }
}

/// The tokens `decls` make, and each declaration's fate. `themes` are the
/// theme blocks' names, in order; `literal` reads a colour as written.
pub fn assemble(
    decls: &[Decl],
    names: &Names,
    themes: &[String],
    literal: &dyn Fn(&str) -> Option<String>,
) -> (Vec<Mapped>, Vec<Fate>) {
    let mut winners: BTreeMap<(Target, String), usize> = BTreeMap::new();
    for (k, (_, var, _, targets)) in decls.iter().enumerate() {
        if names.collided.contains_key(var) {
            continue;
        }
        for t in targets {
            winners.insert((t.clone(), var.clone()), k);
        }
    }
    let read = Read {
        decls,
        winners,
        names,
        literal,
    };
    // Each block's own colour for each variable, where its winner is one:
    // as written, resolved, and the variable a reference names.
    let mut sides: BTreeMap<(Target, String), (String, String, Option<String>)> = BTreeMap::new();
    for ((t, var), k) in &read.winners {
        let mut visiting = vec![var.clone()];
        let value = &decls[*k].2;
        if let Some((written, resolved)) = read.colour_of(value, t, &mut visiting) {
            let named = reference(value)
                .filter(|r| literal(value).is_none() && names.names.contains_key(*r))
                .map(str::to_string);
            sides.insert((t.clone(), var.clone()), (written, resolved, named));
        }
    }
    // A reference round a cycle of tokens (each side fine alone, the
    // tokens' `light-dark()`s each other's) is written as the colour it
    // resolves to. The tokens fold a missing side as CSS reads it (the
    // `when` lines below), so a reference elsewhere folds to what it read.
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ((_, var), (_, _, r)) in &sides {
        if let Some(r) = r {
            graph.entry(var.clone()).or_default().insert(r.clone());
        }
    }
    let cyclic: BTreeSet<String> = graph
        .keys()
        .filter(|v| reaches(&graph, v, v))
        .cloned()
        .collect();
    for ((_, var), entry) in sides.iter_mut() {
        if entry.2.is_some() && cyclic.contains(var) {
            *entry = (entry.1.clone(), entry.1.clone(), None);
        }
    }
    // A block with a winner that is no colour there: its variable makes no
    // token, rather than one side wearing another's colour.
    let mut failed: BTreeMap<String, Target> = BTreeMap::new();
    for (t, var) in read.winners.keys() {
        if !sides.contains_key(&(t.clone(), var.clone())) && names.names.contains_key(var) {
            failed.entry(var.clone()).or_insert_with(|| t.clone());
        }
    }
    // A value that names a dropped token makes no token either, to the end
    // of the chain.
    loop {
        let more: Vec<(String, Target)> = sides
            .iter()
            .filter(|((_, var), (_, _, r))| {
                !failed.contains_key(var) && r.as_ref().is_some_and(|r| failed.contains_key(r))
            })
            .map(|((t, var), _)| (var.clone(), t.clone()))
            .collect();
        if more.is_empty() {
            break;
        }
        for (var, t) in more {
            failed.entry(var).or_insert(t);
        }
    }
    let fates: Vec<Fate> = decls
        .iter()
        .enumerate()
        .map(|(k, (_, var, _, targets))| {
            if let Some(first) = names.collided.get(var) {
                return Fate::Collided(first.clone());
            }
            if names.aux.contains(var) {
                return Fate::Aux;
            }
            let won: Vec<&Target> = targets
                .iter()
                .filter(|t| read.winners.get(&((*t).clone(), var.clone())) == Some(&k))
                .collect();
            if won.is_empty() {
                Fate::Superseded
            } else if won
                .iter()
                .any(|t| sides.contains_key(&((*t).clone(), var.clone())))
            {
                match failed.get(var) {
                    Some(t) => Fate::Unresolved(t.clone()),
                    None => Fate::Mapped,
                }
            } else {
                Fate::NotColour
            }
        })
        .collect();
    let pair = |l: Option<&String>, d: Option<&String>| match (l, d) {
        (Some(l), Some(d)) if l == d => Some(l.clone()),
        (Some(l), Some(d)) => Some(format!("light-dark({l}, {d})")),
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (None, None) => None,
    };
    let mut tokens = Vec::new();
    for var in names.order.iter().filter(|v| !failed.contains_key(*v)) {
        let get = |theme: &Option<String>, dark: bool| {
            sides
                .get(&((theme.clone(), dark), var.clone()))
                .map(|(written, _, _)| written)
        };
        let mut whens = Vec::new();
        for t in themes {
            let theme = Some(t.clone());
            if get(&theme, false).is_none() && get(&theme, true).is_none() {
                continue;
            }
            // A theme's missing side reads as CSS does: its dark side the
            // theme's light, then the default's dark; its light the default's.
            let value = pair(
                get(&theme, false).or_else(|| get(&None, false)),
                get(&theme, true)
                    .or_else(|| get(&theme, false))
                    .or_else(|| get(&None, true)),
            );
            if let Some(v) = value {
                whens.push((vec![("theme".to_string(), t.clone())], v));
            }
        }
        // A variable only a theme sets takes that theme's value as its
        // default; a `when` equal to the default is none.
        let default = pair(get(&None, false), get(&None, true));
        let Some(value) = default.or_else(|| whens.first().map(|(_, v)| v.clone())) else {
            continue;
        };
        whens.retain(|(_, v)| *v != value);
        tokens.push(Mapped {
            name: names.names[var].clone(),
            value,
            whens,
            kind: None,
        });
    }
    (tokens, fates)
}

/// The selectors a nested rule's chain names, each in full: `&` is its
/// parent, and a rule without `&` is a descendant of it (CSS nesting).
/// Deduplicated level by level; a list past 1024 is none.
pub fn nested(rules: &[&String]) -> Vec<String> {
    let mut out = vec![String::new()];
    for prelude in rules {
        let alternatives = split_top(prelude);
        out = out
            .iter()
            .flat_map(|parent| {
                alternatives.iter().map(move |alt| {
                    let alt = alt.trim();
                    match (alt.contains('&'), parent.is_empty()) {
                        (true, _) => alt.replace('&', parent),
                        (false, true) => alt.to_string(),
                        (false, false) => format!("{parent} {alt}"),
                    }
                })
            })
            .collect();
        out.sort();
        out.dedup();
        if out.len() > 1024 {
            return Vec::new();
        }
    }
    out
}

/// A selector list's selectors: its commas outside parentheses and
/// brackets (`:is(.a, .b)` is one).
fn split_top(list: &str) -> Vec<&str> {
    let (mut out, mut start, mut depth) = (Vec::new(), 0, 0i32);
    for (i, c) in list.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&list[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&list[start..]);
    out
}

/// A compound selector's simple selectors: `.dark.theme-x` → `.dark`,
/// `.theme-x`; `[data-theme=x].dark` → `[data-theme=x]`, `.dark`.
pub fn split_compound(w: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut depth) = (0, 0);
    for (i, c) in w.char_indices() {
        match c {
            '[' | '(' => {
                if depth == 0 && c == '[' && i > start {
                    out.push(&w[start..i]);
                    start = i;
                }
                depth += 1;
            }
            ']' | ')' => depth -= 1,
            '.' | ':' if depth == 0 && i > start => {
                out.push(&w[start..i]);
                start = i;
            }
            _ => {}
        }
    }
    out.push(&w[start..]);
    out
}
