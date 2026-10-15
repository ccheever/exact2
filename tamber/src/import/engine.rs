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
}

/// Name every variable `decls` declare by `token`.
pub fn name(decls: &[Decl], token: impl Fn(&str) -> String) -> Names {
    let mut out = Names::default();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    for (_, var, _, _) in decls {
        if out.names.contains_key(var) || out.collided.contains_key(var) {
            continue;
        }
        let t = token(var);
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
        if visiting.iter().any(|v| v == r) || !self.names.names.contains_key(r) {
            return None;
        }
        visiting.push(r.to_string());
        let found = self.seen(at, r, visiting);
        visiting.pop();
        found.map(|(_, resolved)| (self.names.names[r].clone(), resolved))
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
                .filter(|_| literal(value).is_none())
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
    let fates = decls
        .iter()
        .enumerate()
        .map(|(k, (_, var, _, targets))| {
            if let Some(first) = names.collided.get(var) {
                return Fate::Collided(first.clone());
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
                Fate::Mapped
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
    for var in &names.order {
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
        });
    }
    (tokens, fates)
}
