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
    let sides = match checked.tokens.first()?.values.first()?.color()? {
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
/// fill and its ink's token that the app's packages declare. None without
/// an accent the import offers as `AccentColor` (with operations: one that
/// varies by an app axis is not offered, nor is it here), one whose value
/// isn't a colour of its own, or a fill the app already sets to it.
pub fn brands(
    import: &Import,
    design: &Design,
    vocab: &Vocab,
    pairs: &[(String, String)],
) -> Vec<Brand> {
    let Some(accent) = import
        .offers
        .iter()
        .find(|o| o.role == "AccentColor" && !o.ops.is_empty())
        .map(|o| o.token.clone())
    else {
        return Vec::new();
    };
    let value = import.ops.iter().rev().find_map(|op| match op {
        Op::AddToken {
            name, value, whens, ..
        } if *name == accent && whens.is_empty() => Some(value.clone()),
        Op::SetToken {
            name, value, when, ..
        } if *name == accent && when.is_empty() => Some(value.clone()),
        _ => None,
    });
    // The design's own value when the import leaves the token as it is.
    let value = value.or_else(|| {
        design
            .tokens
            .iter()
            .find(|t| t.name.value == accent && t.whens.is_empty())
            .map(|t| t.value.value.clone())
    });
    let Some(ink) = value.as_deref().and_then(|v| ink_on(v, vocab)) else {
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
    pairs
        .iter()
        .filter(|(fill, _)| {
            !design.tokens.iter().any(|t| {
                t.name.value == *fill && t.value.value.trim() == accent && t.whens.is_empty()
            })
        })
        .map(|(fill, ink_token)| Brand {
            sets: fill.clone(),
            token: accent.clone(),
            ops: vec![
                Op::SetToken {
                    name: fill.clone(),
                    when: Vec::new(),
                    value: accent.clone(),
                    at: None,
                    comment: None,
                },
                Op::SetToken {
                    name: ink_token.clone(),
                    when: Vec::new(),
                    value: ink.value.clone(),
                    at: None,
                    comment: None,
                },
            ],
            note: note.clone(),
            contrast: ink.contrast,
        })
        .collect()
}
