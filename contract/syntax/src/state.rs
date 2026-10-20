//! A state part's row as Contract carries it (LLP 1121 D8).

/// The states a part may name: the runner's (`disabled`, `checked`) and the
/// host's (`active`, `hover`, `focus-visible`).
pub const STATES: [&str; 5] = ["active", "hover", "focus-visible", "disabled", "checked"];

/// The attribute a state part's row is carried as: `:disabled
/// background-color` beside the node's own attributes, a name no author can
/// write as one.
pub fn state_attr(state: &str, property: &str) -> String {
    format!("{state} {property}")
}

/// A state part's attribute split into its state (without the `:`) and its
/// row; `None` for any other attribute (a part's `::placeholder color` too).
pub fn state_of(attr: &str) -> Option<(&str, &str)> {
    let rest = attr.strip_prefix(':').filter(|r| !r.starts_with(':'))?;
    let (state, row) = rest.split_once(' ')?;
    STATES.contains(&state).then_some((state, row))
}
