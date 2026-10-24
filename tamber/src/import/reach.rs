//! The importers' role analysis over names (the checker bound's round 3):
//! what a name reaches through the values that name others, found for every
//! name at once, iteratively and linear in the design and the import, so a
//! long chain of names is neither a recursion apiece nor a scan apiece.

use super::Mapped;
use crate::{Design, Token};
use std::collections::{HashMap, HashSet};

/// The names a value may hold.
pub(super) fn words(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
}

/// A design's tokens by name, as `edit::token_index` finds one: the first
/// of that name, else the first spelling of the same role.
pub(super) struct Index<'a> {
    roles: &'a [(String, bool)],
    exact: HashMap<&'a str, usize>,
    by_role: HashMap<String, usize>,
}

impl<'a> Index<'a> {
    pub(super) fn new(design: &'a Design, roles: &'a [(String, bool)]) -> Self {
        let mut exact = HashMap::new();
        let mut by_role = HashMap::new();
        for (i, t) in design.tokens.iter().enumerate() {
            exact.entry(t.name.value.as_str()).or_insert(i);
            if let Some(k) = crate::edit::role_key(roles, &t.name.value) {
                by_role.entry(k).or_insert(i);
            }
        }
        Index {
            roles,
            exact,
            by_role,
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<usize> {
        self.exact.get(name).copied().or_else(|| {
            let k = crate::edit::role_key(self.roles, name)?;
            self.by_role.get(&k).copied()
        })
    }
}

/// Which nodes reach one of `seeds` along `edges`, themselves included.
pub(super) fn reaching(edges: &[Vec<usize>], seeds: impl IntoIterator<Item = usize>) -> Vec<bool> {
    let mut back = vec![Vec::new(); edges.len()];
    for (u, out) in edges.iter().enumerate() {
        for &v in out {
            back[v].push(u);
        }
    }
    let mut on = vec![false; edges.len()];
    let mut todo = Vec::new();
    for s in seeds {
        if !std::mem::replace(&mut on[s], true) {
            todo.push(s);
        }
    }
    while let Some(v) = todo.pop() {
        for &u in &back[v] {
            if !std::mem::replace(&mut on[u], true) {
                todo.push(u);
            }
        }
    }
    on
}

/// The names that reach one `holds` says yes to: a name is the import's
/// token of that name, else the design's token it names (by [`Index`]), and
/// that token names the words of the import token's value, else the design
/// token's default. Every name the import or the design holds, and every
/// word of the design's values, is a node; so is each token a name resolves
/// to, once, so that its words are read once however many spellings name it
/// (Astra's round 3 re-check).
pub(super) fn names_reaching<'a>(
    design: &'a Design,
    roles: &'a [(String, bool)],
    tokens: &'a [Mapped],
    holds: &dyn Fn(Option<&Mapped>, Option<&Token>) -> bool,
) -> HashSet<&'a str> {
    let index = Index::new(design, roles);
    let mut mapped: HashMap<&'a str, usize> = HashMap::new();
    for (i, m) in tokens.iter().enumerate() {
        mapped.entry(m.name.as_str()).or_insert(i);
    }
    // Nodes: names (`names`, by `ids`), then each resolved pair of the
    // import's token and the design's (`defs`), by `edges`' index.
    let mut edges: Vec<Vec<usize>> = Vec::new();
    let mut ids: HashMap<&'a str, usize> = HashMap::new();
    let mut names: Vec<(&'a str, usize)> = Vec::new();
    let mut intern =
        |n: &'a str, names: &mut Vec<(&'a str, usize)>, edges: &mut Vec<Vec<usize>>| {
            *ids.entry(n).or_insert_with(|| {
                edges.push(Vec::new());
                names.push((n, edges.len() - 1));
                edges.len() - 1
            })
        };
    let roots = tokens
        .iter()
        .map(|m| m.name.as_str())
        .chain(design.tokens.iter().flat_map(|t| {
            std::iter::once(t.name.value.as_str()).chain(
                std::iter::once(&t.value.value)
                    .chain(t.whens.iter().map(|w| &w.value.value))
                    .flat_map(|v| words(v)),
            )
        }));
    for n in roots {
        intern(n, &mut names, &mut edges);
    }
    let mut defs: HashMap<(Option<usize>, Option<usize>), usize> = HashMap::new();
    let mut seeds = Vec::new();
    let mut k = 0;
    while k < names.len() {
        let (n, node) = names[k];
        k += 1;
        let key = (mapped.get(n).copied(), index.get(n));
        if key == (None, None) {
            continue;
        }
        let def = match defs.get(&key) {
            Some(&d) => d,
            None => {
                edges.push(Vec::new());
                let d = edges.len() - 1;
                defs.insert(key, d);
                let m = key.0.map(|i| &tokens[i]);
                let have = key.1.map(|i| &design.tokens[i]);
                if holds(m, have) {
                    seeds.push(d);
                }
                let value: &'a str = match (m, have) {
                    (Some(m), _) => &m.value,
                    (None, Some(t)) => &t.value.value,
                    (None, None) => "",
                };
                for w in words(value) {
                    let to = intern(w, &mut names, &mut edges);
                    edges[d].push(to);
                }
                d
            }
        };
        edges[node].push(def);
    }
    let on = reaching(&edges, seeds);
    names
        .into_iter()
        .filter_map(|(n, node)| on[node].then_some(n))
        .collect()
}
