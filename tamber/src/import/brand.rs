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

/// The design `text` as the checker reads it alone: a design that checks
/// only with its packages (a line naming a package's token) without the
/// lines it can't read, so its own tokens can be measured.
fn readable(text: &str, vocab: &Vocab) -> String {
    let mut text = text.to_string();
    for _ in 0..16 {
        let Err(all) = crate::check(&text, vocab) else {
            break;
        };
        let lines: std::collections::BTreeSet<usize> =
            all.iter().map(|d| d.line as usize).collect();
        if lines.is_empty() || lines.contains(&0) {
            break;
        }
        text = text
            .lines()
            .enumerate()
            .filter(|(i, _)| !lines.contains(&(i + 1)))
            .map(|(_, l)| format!("{l}\n"))
            .collect();
    }
    text
}

/// The ink for text on `accent` with the operations `ops` applied to the
/// design `current`: `None` when it can't be measured (the accent isn't
/// there, or folds to no colour of its own), `Some(None)` when it varies by
/// an app axis (no one fill). A design that checks only with its packages is
/// measured without the lines the checker can't read alone.
pub fn measure(current: &str, ops: &[Op], accent: &str, vocab: &Vocab) -> Option<Option<BrandInk>> {
    let text = crate::apply_all(&readable(current, vocab), ops, vocab)
        .ok()?
        .text;
    let f = crate::check(&text, vocab).ok()?;
    let token = f.tokens.iter().find(|t| t.name == accent)?;
    let first = token.values.first()?;
    if token.values.iter().any(|v| v.css() != first.css()) {
        return Some(None);
    }
    Some(Some(ink_over(first.color()?)?))
}

/// Why to look before ticking a brand offer on `accent`: a scheme its ink
/// doesn't reach 4.5:1 in; none when it does in both.
pub fn note_for(accent: &str, ink: &BrandInk) -> Option<String> {
    (ink.below[0] || ink.below[1]).then(|| {
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
    })
}

/// A held brand offer measured again on the design `current` with the
/// operations `ops` it is held with (a partial accept's, an edit's, a
/// recompute's), its items `items` among them and every brand offer's
/// `brand_items` (left out of the measure): its ink, when the offer still
/// sets the fill `sets` to the accent `token` and its ink token to the ink
/// white or black would be on that accent now; none when it doesn't (its
/// fill or ink was edited, the accent changed under it, an operation it was
/// measured through is gone), so the offer goes.
pub fn recheck(
    current: &str,
    ops: &[Op],
    items: &[usize],
    brand_items: &[usize],
    sets: &str,
    token: &str,
    vocab: &Vocab,
) -> Option<BrandInk> {
    let mine: Vec<&Op> = items.iter().filter_map(|&i| ops.get(i)).collect();
    fn valued<'a>(op: &&'a Op) -> Option<(&'a str, &'a str)> {
        match op {
            Op::SetToken {
                name, value, when, ..
            } if when.is_empty() => Some((name.as_str(), value.trim())),
            _ => None,
        }
    }
    let fill = mine.iter().filter_map(valued).find(|(n, _)| *n == sets)?;
    let ink = mine.iter().filter_map(valued).find(|(n, _)| *n != sets)?;
    if fill.1 != token {
        return None;
    }
    // The accent as the rest of the operations leave it: the brand offers' own
    // set the packages' tokens, which this design alone doesn't hold.
    let rest: Vec<Op> = ops
        .iter()
        .enumerate()
        .filter(|(i, _)| !brand_items.contains(i))
        .map(|(_, op)| op.clone())
        .collect();
    measure(current, &rest, token, vocab)?.filter(|now| now.value == ink.1)
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
/// importer's accent, when the accent varies by an app axis or folds to no
/// colour of its own, for a pair the app already sets to the accent and that
/// ink, or for one whose own line is the only use of a dial.
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
    // The accent as the design the import proposes folds it: a value naming
    // another of its tokens (`var(--brand)`) reads that one. One that varies
    // by an app axis is no one fill (a role's own rule), so none is offered.
    let Some(Some(ink)) = measure(current, &import.ops, &accent, vocab) else {
        return Vec::new();
    };
    let note = note_for(&accent, &ink);
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
    // A brand's own line that reads a colour dial nothing but the brand lines
    // read isn't set over: the offers together would leave the dial unused
    // (`tamber-dial-inert`), refusing the whole import where they could only
    // be left unticked.
    let lines: Vec<&str> = pairs
        .iter()
        .flat_map(|(f, i)| [f.as_str(), i.as_str()])
        .collect();
    let orphans = |name: &str| own(name).is_some_and(|t| orphans_with(design, t, &lines));
    pairs
        .iter()
        .filter(|(fill, ink_token)| !(already(fill, &accent) && already(ink_token, &ink.value)))
        .filter(|(fill, ink_token)| !orphans(fill) && !orphans(ink_token))
        .map(|(fill, ink_token)| Brand {
            sets: fill.clone(),
            token: accent.clone(),
            ops: [set(fill, &accent), set(ink_token, &ink.value)].concat(),
            note: note.clone(),
            contrast: ink.contrast,
        })
        .collect()
}

/// Whether `t` reads a dial nothing reads but the tokens `lines` (the brand
/// lines the offers set over).
fn orphans_with(design: &Design, t: &crate::Token, lines: &[&str]) -> bool {
    let words = |t: &crate::Token| -> Vec<String> {
        std::iter::once(t.value.value.as_str())
            .chain(t.whens.iter().map(|w| w.value.value.as_str()))
            .flat_map(|v| {
                v.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '.'))
                    .filter(|w| !w.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let mine = words(t);
    design.dials.iter().any(|d| {
        let dial = d.name.value.as_str();
        mine.iter().any(|w| w == dial)
            && !design
                .tokens
                .iter()
                .filter(|o| !lines.contains(&o.name.value.as_str()))
                .any(|o| words(o).iter().any(|w| w == dial))
    })
}
