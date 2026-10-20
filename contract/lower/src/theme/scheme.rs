//! `lower-colour-one-scheme`'s question (LLP 1121 D3): which of the app's
//! tokens are one opaque colour in both schemes (moved from `theme.rs`).

use super::{inline_design, packages, same_name, stand_in_profiles, vocab};
use contract_syntax::File;
use exact_tamber as tamber;

/// The app's tokens that are one opaque colour in light and dark in some app
/// tuple and not marked `scheme=fixed` (LLP 1121 D3), each by its name as
/// declared with that colour's text and whether it is one colour in every
/// tuple (only then may it be marked): what `lower-colour-one-scheme` reports
/// at a token's declaration. Folded by the crate, so a `light-dark()` over a
/// one-colour token is two colours and an expression is its result. A
/// profile's colour (`color(--…)`) is the plan's, not judged here.
pub(crate) fn one_scheme_tokens(file: &File) -> Vec<(String, String, bool)> {
    let design = match (&file.design, &file.theme) {
        (None, Some(theme)) => inline_design(theme),
        (Some(source), _) => match tamber::parse(&source.text) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        },
        (None, None) => return Vec::new(),
    };
    // Folded with its packages' tokens, which an app token may name (D11);
    // only the app's own are judged.
    let app = design.tokens.len();
    let Some(mut design) = packages::merge(Some(design), &file.package_designs).0 else {
        return Vec::new();
    };
    // As the build stands one in (`profiles`): nothing to judge.
    stand_in_profiles(&mut design);
    let Ok(folding) = tamber::check_design(design, &vocab()) else {
        return Vec::new();
    };
    // A token fixed, or an alias of one (`ink = -exact-label` of a fixed
    // `label`), by every name it answers to.
    let pairs: Vec<_> = folding.tokens.iter().zip(&folding.design.tokens).collect();
    let mut fixed: Vec<String> = Vec::new();
    let judged = |name: &str| {
        folding.design.tokens[..app.min(folding.design.tokens.len())]
            .iter()
            .any(|t| t.name.value == name)
    };
    loop {
        let before = fixed.len();
        for (t, declared) in &pairs {
            let value = declared.value.value.trim();
            // Only a whole alias with no `when` of its own is the mark's.
            let alias = declared.whens.is_empty() && fixed.iter().any(|f| same_name(f, value));
            if (t.fixed || alias) && !fixed.contains(&declared.name.value) {
                fixed.push(declared.name.value.clone());
                fixed.push(t.name.clone());
            }
        }
        if fixed.len() == before {
            break;
        }
    }
    pairs
        .into_iter()
        .filter(|(_, declared)| !fixed.contains(&declared.name.value) && judged(&declared.name.value))
        .filter_map(|(t, declared)| {
            // Opaque only: a translucent one is an overlay, which reads over
            // either scheme's ground, and a transparent one paints nothing.
            let one = t.values.iter().find(|v| {
                matches!(v, tamber::Value::Color(tamber::Folded::One(h)) if h.wide().alpha >= 1.0 - 1e-9)
            })?;
            let every = t
                .values
                .iter()
                .all(|v| matches!(v, tamber::Value::Color(tamber::Folded::One(_))));
            Some((declared.name.value.clone(), one.css(), every))
        })
        .collect()
}
