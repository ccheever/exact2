//! The root's declarations by name, built once for the lowerer: a scan of
//! them for each use is quadratic in a large root.
use std::collections::HashMap;

/// Each name to its first index, as `position` finds it.
pub(crate) fn first<'a>(names: impl Iterator<Item = &'a str>) -> HashMap<&'a str, usize> {
    let mut index = HashMap::new();
    for (i, name) in names.enumerate() {
        index.entry(name).or_insert(i);
    }
    index
}

/// The root's states (`Ok`) and mutations (`Err`) by name, to the first
/// declaration's index; a state wins over a mutation of the same name.
pub(crate) fn slots(root: &contract_syntax::Component) -> HashMap<&str, Result<usize, usize>> {
    let mut index: HashMap<&str, Result<usize, usize>> =
        first(root.mutations.iter().map(|m| m.name.as_str()))
            .into_iter()
            .map(|(name, i)| (name, Err(i)))
            .collect();
    for (name, i) in first(root.states.iter().map(|s| s.name.as_str())) {
        index.insert(name, Ok(i));
    }
    index
}
