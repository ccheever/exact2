//! `lower-colour-one-scheme`'s question (LLP 1121 D3): which of the app's
//! tokens are one opaque colour in both schemes (moved from `theme.rs`).

use super::{inline_design, packages, stand_in_profiles, vocab};
use contract_syntax::File;
use exact_tamber as tamber;
use std::collections::{HashMap, HashSet};

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
    // Found from the fixed ones outwards, each name once (linear: the
    // checker bound's sweep), names compared as `same_name` does.
    let pairs: Vec<_> = folding.tokens.iter().zip(&folding.design.tokens).collect();
    let key = |n: &str| {
        super::role_named(n)
            .map(|(x, _)| x)
            .ok_or_else(|| n.to_string())
    };
    let mut naming: HashMap<_, Vec<usize>> = HashMap::new();
    for (i, (_, declared)) in pairs.iter().enumerate() {
        if declared.whens.is_empty() {
            naming
                .entry(key(declared.value.value.trim()))
                .or_default()
                .push(i);
        }
    }
    let mut queue: Vec<usize> = (0..pairs.len()).filter(|&i| pairs[i].0.fixed).collect();
    let (mut fixed, mut keys) = (HashSet::new(), HashSet::new());
    while let Some(i) = queue.pop() {
        let (t, declared) = pairs[i];
        if !fixed.insert(declared.name.value.clone()) {
            continue;
        }
        fixed.insert(t.name.clone());
        for n in [&declared.name.value, &t.name] {
            if keys.insert(key(n)) {
                queue.extend(naming.get(&key(n)).into_iter().flatten());
            }
        }
    }
    let judged: HashSet<&str> = folding.design.tokens[..app.min(folding.design.tokens.len())]
        .iter()
        .map(|t| t.name.value.as_str())
        .collect();
    pairs
        .into_iter()
        .filter(|(_, declared)| {
            !fixed.contains(&declared.name.value) && judged.contains(declared.name.value.as_str())
        })
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
