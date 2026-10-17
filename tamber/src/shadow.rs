//! Shadow tokens (LLP 1121 D3, stage 6 D): `box-shadow` and `text-shadow`,
//! each its row's own syntax, folded per tuple. A length is px (unitless
//! zero), a length token or dial; a colour is a literal, a colour token, a
//! role or a colour expression over tokens, folded as a colour token is. A
//! `rem` or `em` inside one waits on QUEUE's item.

use crate::css::Item;
use crate::fold::{self, fail, FoldError, Folded, Named, Names};
use crate::measure::{num, Length};

/// Why a relative length inside a shadow is refused.
const RELATIVE: &str = "the kernel takes only px in a shadow until QUEUE's item \"`rem`/`em` where the kernel still takes only `px`\"";

/// One folded shadow.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// Inside the padding box (`box-shadow` only).
    pub inset: bool,
    /// Offset x, offset y, blur, spread, in px: two to four of them.
    pub lengths: Vec<f64>,
    /// Its colour; `None` only in a `text-shadow` (`currentcolor`).
    pub color: Option<Folded>,
}

/// A folded shadow row: `none`, or its shadows (one for a `text-shadow`).
#[derive(Debug, Clone, PartialEq)]
pub struct Shadows {
    /// A `text-shadow`, rather than a `box-shadow`.
    pub text: bool,
    /// The shadows, the first on top; empty for `none`.
    pub list: Vec<Shadow>,
}

impl Shadows {
    /// The text the plan carries, which the row's codec reads.
    pub fn css(&self) -> String {
        if self.list.is_empty() {
            return "none".into();
        }
        let one = |s: &Shadow| {
            let mut out = String::new();
            if s.inset {
                out.push_str("inset ");
            }
            for l in &s.lengths {
                out.push_str(&num(*l));
                out.push_str("px ");
            }
            match &s.color {
                Some(c) => out.push_str(&c.css()),
                None => out.push_str("currentcolor"),
            }
            out
        };
        self.list.iter().map(one).collect::<Vec<_>>().join(", ")
    }
}

/// A value's shadows' texts: split at commas outside parentheses (the CSS
/// reader takes a comma only inside a function).
pub(crate) fn split(text: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, 0);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(text[start..].trim());
    out
}

/// Whether a value reads as a shadow (a list, two or more lengths, or
/// `inset`), so an unwritten kind is asked for rather than read as a colour.
pub(crate) fn looks(text: &str) -> bool {
    let parts = split(text);
    parts.len() > 1
        || crate::css::parse(parts[0]).is_ok_and(|items| {
            let lengths = items
                .iter()
                .filter(|i| matches!(i, Item::Dim(..)) || matches!(i, Item::Number(n) if *n == 0.0))
                .count();
            lengths >= 2
                || items
                    .iter()
                    .any(|i| matches!(i, Item::Ident(w) if w.eq_ignore_ascii_case("inset")))
        })
}

/// Fold a whole value's text as a shadow row: a `box-shadow` (`text`
/// false) or a `text-shadow`.
pub(crate) fn fold(value: &str, text: bool, names: &mut dyn Names) -> Result<Shadows, FoldError> {
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(Shadows {
            text,
            list: Vec::new(),
        });
    }
    let parts = split(value);
    if text && parts.len() > 1 {
        return fail("tamber-kind", "a text shadow is one shadow, not a list");
    }
    if parts.len() > 8 {
        return fail("tamber-kind", "at most 8 shadows in one box-shadow");
    }
    let mut list = Vec::new();
    for part in parts {
        let items = crate::css::parse(part).map_err(|m| FoldError {
            id: "tamber-syntax",
            message: m,
        })?;
        list.push(one(&items, text, names)?);
    }
    Ok(Shadows { text, list })
}

fn one(items: &[Item], text: bool, names: &mut dyn Names) -> Result<Shadow, FoldError> {
    let (mut lengths, mut color, mut inset, mut closed) = (Vec::new(), None, false, false);
    for item in items {
        match item {
            Item::Ident(w) if w.eq_ignore_ascii_case("inset") => {
                if text {
                    return fail("tamber-kind", "a text shadow is never `inset`");
                }
                if inset {
                    return fail("tamber-kind", "`inset` once per shadow");
                }
                inset = true;
                closed = !lengths.is_empty();
                continue;
            }
            _ => {}
        }
        if let Some(px) = length(item, names)? {
            if closed {
                return fail(
                    "tamber-kind",
                    "the lengths are written together: <offset-x> <offset-y> [<blur> [<spread>]]",
                );
            }
            lengths.push(px);
            continue;
        }
        if color.is_some() {
            return fail(
                "tamber-kind",
                "one colour, and lengths in px (unitless zero)",
            );
        }
        closed = !lengths.is_empty();
        color = Some(
            if text && matches!(item, Item::Ident(w) if w.eq_ignore_ascii_case("currentcolor")) {
                None
            } else {
                Some(fold::color_item(item, names)?)
            },
        );
    }
    match (lengths.len(), text) {
        (2 | 3, _) | (4, false) => {}
        (4, true) => {
            return fail(
                "tamber-kind",
                "a text shadow has no spread: <offset-x> <offset-y> [<blur>]",
            )
        }
        _ if text => {
            return fail(
                "tamber-kind",
                "two or three lengths: <offset-x> <offset-y> [<blur>]",
            )
        }
        _ => {
            return fail(
                "tamber-kind",
                "two to four lengths: <offset-x> <offset-y> [<blur> [<spread>]]",
            )
        }
    }
    if lengths.get(2).is_some_and(|b| *b < 0.0) {
        return fail("tamber-kind", "a blur radius is never negative");
    }
    let color = match color {
        Some(c) => c,
        None if text => None,
        None => {
            return fail(
                "tamber-kind",
                "write the colour: CSS's default, currentcolor, is not one a box shadow holds",
            )
        }
    };
    Ok(Shadow {
        inset,
        lengths,
        color,
    })
}

/// An item as a shadow's length in px, `None` when it is no length (a
/// colour, by elimination).
fn length(item: &Item, names: &mut dyn Names) -> Result<Option<f64>, FoldError> {
    // The kernel holds a length as an f32: one past it is no length.
    let fits = |n: f64| -> Result<Option<f64>, FoldError> {
        if n.abs() <= f64::from(f32::MAX) {
            Ok(Some(n))
        } else {
            fail(
                "tamber-kind",
                format!("`{}px` is past the lengths a shadow holds", num(n)),
            )
        }
    };
    Ok(match item {
        Item::Dim(n, unit) if unit == "px" => return fits(*n),
        Item::Dim(_, unit) if unit == "rem" || unit == "em" => {
            return fail("tamber-kind", format!("a shadow's length: {RELATIVE}"))
        }
        Item::Dim(n, unit) => {
            return fail(
                "tamber-kind",
                format!("`{}{unit}`: a shadow's length is px", num(*n)),
            )
        }
        Item::Number(n) if *n == 0.0 => Some(0.0),
        Item::Number(n) => {
            return fail(
                "tamber-kind",
                format!(
                    "`{}`: a shadow's length says px (unitless only zero)",
                    num(*n)
                ),
            )
        }
        Item::Ident(name) => match names.named(name)? {
            Named::Length(Length::Px(n)) => return fits(n),
            Named::Length(_) => {
                return fail("tamber-kind", format!("`{name}` in a shadow: {RELATIVE}"))
            }
            Named::Number(_) => {
                return fail(
                    "tamber-kind",
                    format!("`{name}` is a number, not a shadow's length"),
                )
            }
            // A colour by elimination: the colour fold reads it.
            _ => None,
        },
        _ => None,
    })
}
