//! A token's `pair=` metadata (`Checker::pairs`), apart from the checker's
//! walks.

use super::*;

impl Checker<'_> {
    /// What `t` (token `i`) says it is drawn on (`pair=`, comma-separated,
    /// once or more): each a colour token or a role, and never `t` itself
    /// (`tamber-pair`). Every token is a colour today, so a token is one.
    pub(super) fn pairs(&mut self, i: usize, t: &Token) -> Vec<String> {
        let (mut out, mut seen) = (Vec::new(), HashSet::new());
        let named: Vec<&str> = t
            .meta
            .iter()
            .filter(|(k, _)| k == "pair")
            .flat_map(|(_, v)| v.split(','))
            .map(str::trim)
            .collect();
        if !named.is_empty() && self.kinds[i] != Kind::Color {
            self.at(
                &t.name,
                "tamber-pair",
                format!(
                    "`{}` is a {}: `pair=` says what a colour is drawn on",
                    t.name.value,
                    self.kinds[i].name()
                ),
            );
            return out;
        }
        // An empty list, or an empty name in one, names nothing.
        if named.iter().any(|n| n.is_empty()) {
            self.at(&t.name, "tamber-pair", format!("`{}` has an empty `pair=` name: `pair=` names what it is drawn on, `pair=surface.page` or several, comma-separated", t.name.value));
        }
        for name in named.into_iter().filter(|n| !n.is_empty()) {
            let token = self.names.get(name).copied().or_else(|| {
                let (_, row) = self.role_key(name)?;
                self.names.get(&row.to_ascii_lowercase()).copied()
            });
            let found = match token {
                Some(j) if j == i => {
                    self.at(&t.name, "tamber-pair", format!("`{}` says it is drawn on itself: `pair=` names what it is drawn on, another token or a role", t.name.value));
                    continue;
                }
                Some(j) if self.kinds[j] != Kind::Color => {
                    self.at(
                        &t.name,
                        "tamber-pair",
                        format!(
                            "`pair={name}` names a {}: `pair=` names the colour `{}` is drawn on",
                            self.kinds[j].name(),
                            t.name.value
                        ),
                    );
                    continue;
                }
                Some(j) => {
                    let name = &self.design.tokens[j].name.value;
                    self.role_key(name).map_or(name.clone(), |(_, row)| row)
                }
                None => match self.role_key(name) {
                    Some((_, row)) => row,
                    None => {
                        self.at(&t.name, "tamber-pair", format!("`pair={name}` names no colour token or role of this design: `pair=` names what `{}` is drawn on", t.name.value));
                        continue;
                    }
                },
            };
            if seen.insert(found.clone()) {
                out.push(found);
            }
        }
        out
    }
}
