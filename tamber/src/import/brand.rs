//! Brand offers (theme2's brand-fill ruling, 2026-10-29): an import that
//! carries an accent offers it as the packages' brand fills too, a filled
//! control's colour the platform's accent doesn't reach (FollowButton's
//! `actions.follow-fill`, EmptyState's `cards.empty-button`), each with the
//! ink its label is read in. The ink is white or black for each scheme,
//! whichever reaches 4.5:1 on that scheme's fill, measured as the checker
//! measures a pair. A fill neither reaches it on is still offered, with the
//! ratio it reaches and why; every brand offer starts off, as a role
//! override does.

use super::Import;
use crate::edit::Op;
use crate::{ratio, Design, Folded, Vocab};

/// The ink a brand fill's label is read in.
#[derive(Debug, Clone, PartialEq)]
pub struct BrandInk {
    /// Its value, CSS text: one colour, or `light-dark(…)` when the light
    /// and dark fills want different inks.
    pub value: String,
    /// The ratio it reaches on the light fill and on the dark one, cut to
    /// two places as the checker's badges cut it (4.499 is 4.49); none on a
    /// fill that isn't opaque.
    pub contrast: [Option<f64>; 2],
    /// The schemes, light then dark, where neither ink reaches 4.5:1.
    pub below: [bool; 2],
}

/// The ink for text on a fill, `value` as a design writes a colour; none
/// when it folds to no colour of its own (the host's, a name not here).
pub fn ink_on(value: &str, vocab: &Vocab) -> Option<BrandInk> {
    let checked = crate::check(
        &format!("tamber app\n\ntheme\n  brand-fill = {value}\n"),
        vocab,
    )
    .ok()?;
    ink_over(checked.tokens.first()?.values.first()?.color()?)
}

/// The ink for text on a folded fill.
fn ink_over(fill: &Folded) -> Option<BrandInk> {
    let sides = match fill {
        Folded::One(h) => [h.clip(), h.clip()],
        Folded::Pair(l, d) => [l.clip(), d.clip()],
        Folded::Host(_) => return None,
    };
    let cut = |r: f64| (r * 100.0).floor() / 100.0;
    let mut inks = [""; 2];
    let mut contrast = [None; 2];
    for (i, fill) in sides.iter().enumerate() {
        // On an opaque fill one of the two always reaches it (their ratios
        // multiply to 21, so the higher is at least 4.58:1); a fill that
        // isn't opaque has no one ratio, and white is offered, marked.
        let (ink, r) = match (ratio("#ffffff", fill), ratio("#000000", fill)) {
            (Some(w), Some(b)) if b > w => ("#000000", Some(cut(b))),
            (Some(w), Some(_)) => ("#ffffff", Some(cut(w))),
            _ => ("#ffffff", None),
        };
        inks[i] = ink;
        contrast[i] = r;
    }
    let below = contrast.map(|r| r.is_none_or(|r| r < 4.5));
    let value = if inks[0] == inks[1] {
        inks[0].to_string()
    } else {
        format!("light-dark({}, {})", inks[0], inks[1])
    };
    Some(BrandInk {
        value,
        contrast,
        below,
    })
}

/// One brand offer: the package token it sets (`sets`), the import's
/// accent token it is set to (`token`), and its operations (the fill, then
/// the ink); a note when the ink falls short in a scheme.
#[derive(Debug, Clone, PartialEq)]
pub struct Brand {
    /// The package's brand fill it sets for the app.
    pub sets: String,
    /// The import's accent token.
    pub token: String,
    /// The fill set to the accent, then its ink.
    pub ops: Vec<Op>,
    /// Why to look before ticking it: a scheme neither ink reaches 4.5:1 in.
    pub note: Option<String>,
    /// The ink's ratio on the light and the dark fill (none: not opaque).
    pub contrast: [Option<f64>; 2],
}

/// The brand offers an import makes for `pairs`, each a package's brand
/// fill and its ink's token that the app's packages declare, planned on
/// `current` (the design's text, `design` parsed). None without the
/// importer's accent, when its `AccentColor` override is refused (it varies
/// by an app axis, keeps `when` lines, or would orphan a dial), when the
/// accent folds to no colour of its own on the design the import proposes,
/// or for a pair the app already sets to the accent and that ink.
pub fn brands(
    import: &Import,
    design: &Design,
    current: &str,
    vocab: &Vocab,
    pairs: &[(String, String)],
) -> Vec<Brand> {
    let Some(accent) = import.accent.clone() else {
        return Vec::new();
    };
    if import
        .offers
        .iter()
        .any(|o| o.role == "AccentColor" && o.ops.is_empty())
    {
        return Vec::new();
    }
    // The accent as the design the import proposes folds it: a value naming
    // another of its tokens (`var(--brand)`) reads that one.
    let proposed = crate::apply_all(current, &import.ops, vocab)
        .map(|a| a.text)
        .unwrap_or_else(|_| current.to_string());
    let Some(ink) = crate::check(&proposed, vocab).ok().and_then(|f| {
        let token = f.tokens.iter().find(|t| t.name == accent)?;
        ink_over(token.values.first()?.color()?)
    }) else {
        return Vec::new();
    };
    let note = (ink.below[0] || ink.below[1]).then(|| {
        let schemes: Vec<String> = [("light", 0), ("dark", 1)]
            .into_iter()
            .filter(|(_, i)| ink.below[*i])
            .map(|(s, i)| match ink.contrast[i] {
                Some(r) => format!("{s} {r}:1"),
                None => format!("{s}: not opaque, so no one ratio"),
            })
            .collect();
        format!(
            "its label's text doesn't reach 4.5:1 on `{accent}` ({}): it may be hard to read; look before choosing it",
            schemes.join(", ")
        )
    });
    // The app's own line for a token, if it has one.
    let own = |name: &str| design.tokens.iter().find(|t| t.name.value == name);
    let set = |name: &str, value: &str| -> Vec<Op> {
        // The value in every mode: the app's own `when` lines for it go, so
        // what the offer measured is what each mode shows.
        std::iter::once(Op::SetToken {
            name: name.into(),
            when: Vec::new(),
            value: value.into(),
            at: None,
            comment: None,
        })
        .chain(own(name).into_iter().flat_map(|t| {
            t.whens.iter().map(|w| Op::UnsetTokenWhen {
                name: name.into(),
                when: w.conds.clone(),
            })
        }))
        .collect()
    };
    let already = |name: &str, value: &str| {
        own(name).is_some_and(|t| t.value.value.trim() == value && t.whens.is_empty())
    };
    pairs
        .iter()
        .filter(|(fill, ink_token)| !(already(fill, &accent) && already(ink_token, &ink.value)))
        .map(|(fill, ink_token)| Brand {
            sets: fill.clone(),
            token: accent.clone(),
            ops: [set(fill, &accent), set(ink_token, &ink.value)].concat(),
            note: note.clone(),
            contrast: ink.contrast,
        })
        .collect()
}
