//! `keep-mounted` (@ref LLP 1010 §6.11): rows a list keeps mounted wherever
//! its window is, named by key. VoiceOver moves from a transcript's first
//! message to its last only if both are in the tree; a playing row keeps
//! its player.
use super::*;

/// The most keys a list keeps, `initial-item-count`'s bound: kept rows add
/// at most this many to any window. The set is the author's and does not
/// grow as the reader scrolls (§6.2's memory requirement).
pub(crate) const MOST: usize = 64;

/// The row identities `value` names: a list of at most [`MOST`] keys, each
/// a string, number or bool or an option of one (`none` names no row). The
/// first row of a repeated key is the one kept. `None` for any other value.
pub(crate) fn idents(value: &Value) -> Option<Vec<Rc<str>>> {
    let Value::List(items) = value else {
        return None;
    };
    if items.len() > MOST {
        return None;
    }
    let mut out: Vec<Rc<str>> = Vec::new();
    for item in items.iter() {
        let key = match item {
            Value::Option(None) => continue,
            Value::Option(Some(key)) => key,
            key => key,
        };
        let text: Rc<str> = super::super::key_text(key)?.into();
        if !out.contains(&text) {
            out.push(text);
        }
    }
    Some(out)
}

impl Collection {
    /// Take the list's `keep-mounted` value; whether the rows it names
    /// changed. A value it refuses keeps none (the bridge journals it).
    pub(super) fn keep_mounted(&mut self, value: Option<&Value>) -> bool {
        let keys = value.and_then(idents).unwrap_or_default();
        let changed = keys != self.keep;
        self.keep = keys;
        changed
    }

    /// The positions of the kept rows the list holds.
    pub(super) fn kept_rows(&self) -> Vec<usize> {
        self.keep
            .iter()
            .filter_map(|key| self.index.position(key))
            .collect()
    }
}
