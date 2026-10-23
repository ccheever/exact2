//! A tuple's colours, aliases followed: [`Resolve`] (moved out of `dtcg.rs`, whose line cap it shares).
use super::*;

/// A path's colour in one tuple: as written (a reference a token's name),
/// resolved, and the path it references.
pub(super) type Colour = (String, String, Option<String>);

/// A path's colour in one tuple, or why not.
pub(super) type Seen = Result<Colour, String>;

/// A tuple's resolution: its winners, the token names, and each path's
/// colour once found.
pub(super) struct Resolve<'a> {
    pub(super) defs: &'a [Def],
    pub(super) w: &'a BTreeMap<&'a str, usize>,
    pub(super) names: &'a BTreeMap<String, String>,
    pub(super) memo: HashMap<String, Seen>,
    /// Each path's typed definition, once found (`type_of`).
    pub(super) types: RefCell<HashMap<String, Option<usize>>>,
}

impl Resolve<'_> {
    /// A path's colour, aliases followed (a cycle is none).
    pub(super) fn seen(&mut self, path: &str, visiting: &mut BTreeSet<String>) -> Seen {
        if let Some(found) = self.memo.get(path) {
            return found.clone();
        }
        let Some(&i) = self.w.get(path) else {
            return Err(format!("`{{{path}}}` is no token in this mode"));
        };
        crate::budget::spend(1)?;
        let d = &self.defs[i];
        let found = if let Some(why) = &d.refused {
            Err(why.clone())
        } else {
            match alias(&d.value) {
                Some(Ok(target)) if visiting.contains(&target) => {
                    Err(format!("a cycle of aliases through `{{{target}}}`"))
                }
                Some(Ok(_)) if visiting.len() > 256 => {
                    Err("an alias chain longer than 256".to_string())
                }
                Some(Ok(target)) => {
                    visiting.insert(target.clone());
                    let inner = self.seen(&target, visiting);
                    visiting.remove(&target);
                    inner.map(|(_, resolved, _)| match self.names.get(&target) {
                        Some(n) => (n.clone(), resolved, Some(target)),
                        None => (resolved.clone(), resolved, None),
                    })
                }
                Some(Err(why)) => Err(why),
                None => colour(&d.value)
                    .map(|c| (c.clone(), c, None))
                    .ok_or_else(|| match d.value.as_str() {
                        Some(s) if exact_color::parse(s).is_some() => {
                            "a colour Tamber does not fold yet".to_string()
                        }
                        _ => "not a colour Tamber reads".to_string(),
                    }),
            }
        };
        // A colour found holds, its names (or why not) charged once returned, once kept; a failure may be a cycle in progress, kept only from the top.
        let size = match &found {
            Ok((a, b, c)) => a.len() + b.len() + c.as_ref().map_or(0, String::len),
            Err(why) => why.len(),
        };
        crate::budget::spend(2 * size as u64)?;
        if found.is_ok() || visiting.len() <= 1 {
            self.memo.insert(path.to_string(), found.clone());
        }
        found
    }
}
