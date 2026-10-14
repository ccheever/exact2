//! A CSS part declaration as Contract carries it (LLP 1104 D3).

/// The attribute a part's declaration is carried as (LLP 1104 D3): CSS's
/// `::placeholder { color }` is `::placeholder color` beside the element's
/// own attributes, a name no author can write as one.
pub fn part_attr(part: &str, property: &str) -> String {
    format!("{part} {property}")
}

/// [`part_attr`]`("::placeholder", "color")`.
pub const PLACEHOLDER_COLOR: &str = "::placeholder color";
