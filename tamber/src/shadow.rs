//! Shadow tokens (LLP 1121 D3, stage 6 D): `box-shadow` and `text-shadow`,
//! each its row's own syntax, folded per tuple. A length is px (unitless
//! zero), a length token or dial, or over base names the design leaves
//! unset (a bare one, a token over some, or a length's function over one),
//! kept as a [`Residual`] each host folds in its column (stage 12 B3); a
//! colour is a literal, a colour token, a role or a colour expression over
//! tokens, folded as a colour token is. A `rem` or `em` inside one waits on
//! QUEUE's item.

use crate::css::Item;
use crate::fold::{self, fail, FoldError, Folded, Named, Names};
use crate::measure::{num, Length};
use crate::residual::Residual;

/// Why a relative length inside a shadow is refused.
const RELATIVE: &str = "the kernel takes only px in a shadow until QUEUE's item \"`rem`/`em` where the kernel still takes only `px`\"";

/// One folded shadow.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// Inside the padding box (`box-shadow` only).
    pub inset: bool,
    /// Offset x, offset y, blur, spread: two to four of them.
    pub lengths: Vec<ShadowLength>,
    /// Its colour; `None` only in a `text-shadow` (`currentcolor`).
    pub color: Option<Folded>,
}

/// One of a shadow's lengths: px, or over base names the design leaves
/// unset, each host's in its column (stage 12 B3), as a length token's
/// [`Length::Base`] is.
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowLength {
    /// Pixels.
    Px(f64),
    /// Over base names: a bare one (`space.md`) or an expression.
    Base(Residual),
}

impl ShadowLength {
    /// The text the plan carries: `4px`, `space.md`, `calc(space.md * 0.5)`.
    pub fn css(&self) -> String {
        match self {
            ShadowLength::Px(n) => format!("{}px", num(*n)),
            ShadowLength::Base(r) => r.css(),
        }
    }

    /// Its px, each base name's px `base` gives (a host's column, or outside
    /// Exact the neutral one's); `None` where [`Residual::fold`] gives none.
    pub fn px(&self, base: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
        match self {
            ShadowLength::Px(n) => Some(*n),
            ShadowLength::Base(r) => r.fold(base),
        }
    }
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
                out.push_str(&l.css());
                out.push(' ');
            }
            match &s.color {
                Some(c) => out.push_str(&c.css()),
                None => out.push_str("currentcolor"),
            }
            out
        };
        self.list.iter().map(one).collect::<Vec<_>>().join(", ")
    }

    /// The base names its lengths read, each once, in order (stage 12 B3).
    pub fn bases(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for s in &self.list {
            for l in &s.lengths {
                if let ShadowLength::Base(r) = l {
                    for b in r.bases() {
                        if !out.contains(&b) {
                            out.push(b);
                        }
                    }
                }
            }
        }
        out
    }

    /// Itself with every length px, each base name's px `base` gives:
    /// what is written outside Exact (`tamber gen`'s CSS), the neutral
    /// column's, as a length token's expression is. `None` where one
    /// doesn't fold.
    pub fn folded(&self, base: &dyn Fn(&str) -> Option<f64>) -> Option<Shadows> {
        let mut out = self.clone();
        for s in &mut out.list {
            for l in &mut s.lengths {
                *l = ShadowLength::Px(l.px(base)?);
            }
        }
        Some(out)
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

/// Whether a value reads as a shadow (a list, two or more lengths, a
/// length token's or dial's name or a length's function among them, or
/// `inset`), so an unwritten kind is asked for rather than read as a
/// colour. `length` says a name is a length token's or dial's; `named` a
/// named length's or an unset base name's, which only a function's
/// arguments are asked of.
pub(crate) fn looks(
    text: &str,
    length: &mut dyn FnMut(&str) -> bool,
    named: &dyn Fn(&str) -> bool,
) -> bool {
    // A function over a dimension or a length's name, at any depth: one
    // over plain numbers is a number's (stage 12 B3, round 3, Grok).
    fn measures(
        args: &[Vec<Item>],
        length: &mut dyn FnMut(&str) -> bool,
        named: &dyn Fn(&str) -> bool,
    ) -> bool {
        args.iter().flatten().any(|i| match i {
            Item::Dim(..) => true,
            Item::Ident(n) => named(n) || length(n),
            Item::Func(_, args, _) => measures(args, length, named),
            _ => false,
        })
    }
    let parts = split(text);
    parts.len() > 1
        || crate::css::parse(parts[0]).is_ok_and(|items| {
            let lengths = items
                .iter()
                .filter(|i| match i {
                    Item::Dim(..) => true,
                    Item::Number(n) => *n == 0.0,
                    Item::Ident(name) => length(name),
                    // A length's function (stage 12 B3, round 2, Grok 1).
                    Item::Func(f, args, _) => {
                        matches!(f.as_str(), "calc" | "min" | "max" | "clamp")
                            && measures(args, length, named)
                    }
                    _ => false,
                })
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
    // A whole value naming a shadow token of its kind is an alias of it.
    if let Ok([Item::Ident(name)]) = crate::css::parse(value).as_deref() {
        if let Named::Shadow(s) = names.named(name)? {
            return match s.text == text {
                true => Ok(s),
                false => fail("tamber-kind", format!("`{name}` is the other row's shadow")),
            };
        }
    }
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
            id: crate::css::refusal_id(part, &m),
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
        if let Some(l) = length(item, names)? {
            if closed {
                return fail(
                    "tamber-kind",
                    "the lengths are written together: <offset-x> <offset-y> [<blur> [<spread>]]",
                );
            }
            lengths.push(l);
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
    // A blur over base names is checked in each column, by the build
    // (stage 12 B3).
    if matches!(lengths.get(2), Some(ShadowLength::Px(b)) if *b < 0.0) {
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

/// An item as a shadow's length, `None` when it is no length (a colour, by
/// elimination). One over base names the design leaves unset is kept, as a
/// length token's is (stage 12 B3): a bare one, or a token aliasing one or
/// over some (its references inlined). A length's `calc()`, `min()`,
/// `max()` or `clamp()` folds by the length's own code, whose refusals
/// stand: px, or over base names.
fn length(item: &Item, names: &mut dyn Names) -> Result<Option<ShadowLength>, FoldError> {
    // The kernel holds a length as an f32: one past it is no length.
    let fits = |n: f64| -> Result<Option<ShadowLength>, FoldError> {
        if n.abs() <= f64::from(f32::MAX) {
            Ok(Some(ShadowLength::Px(n)))
        } else {
            fail(
                "tamber-kind",
                format!("`{}px` is past the lengths a shadow holds", num(n)),
            )
        }
    };
    let based = |r: Residual| Ok(Some(ShadowLength::Base(r)));
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
        Item::Number(n) if *n == 0.0 => Some(ShadowLength::Px(0.0)),
        Item::Number(n) => {
            return fail(
                "tamber-kind",
                format!(
                    "`{}`: a shadow's length says px (unitless only zero)",
                    num(*n)
                ),
            )
        }
        // A base name the design leaves unset, bare: each host's.
        Item::Ident(name) if names.base(name) => return based(Residual::base(name)),
        Item::Ident(name) => match names.named(name)? {
            Named::Length(Length::Px(n)) => return fits(n),
            // A token aliasing a base name the design leaves unset, or over
            // some: inlined, as a length's `calc()` inlines it.
            Named::Length(Length::Named(n)) if names.base(&n) => return based(Residual::base(&n)),
            Named::Length(Length::Base(r)) => {
                // Its base names are read here too, whatever folding leaves.
                for b in r.bases() {
                    names.base(b);
                }
                return based(r);
            }
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
        // A length's function (stage 12 B3): the length's own fold, as a
        // length token's value folds, so px and length tokens give px, a
        // base the design sets its px, and one it leaves unset a residual
        // (`calc(space.md)` is `space.md`); setting the base never turns
        // the same text into a refusal.
        Item::Func(f, ..) if matches!(f.as_str(), "calc" | "min" | "max" | "clamp") => {
            return match crate::measure::length(std::slice::from_ref(item), names, &|_| false)? {
                Length::Px(n) => fits(n),
                Length::Named(n) => based(Residual::base(&n)),
                Length::Base(r) => based(r),
                // A length's function folds to px or over base names.
                other => fail(
                    "tamber-kind",
                    format!("`{}` in a shadow: {RELATIVE}", other.css()),
                ),
            };
        }
        _ => None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    // Stage 12 B3: a length over base names is written as its residual,
    // reads its bases, and folds with each base's px outside Exact.
    #[test]
    fn a_length_over_base_names_is_written_read_and_folded() {
        let md = Residual::base("space.md");
        let s = Shadows {
            text: false,
            list: vec![Shadow {
                inset: false,
                lengths: vec![
                    ShadowLength::Px(0.0),
                    ShadowLength::Base(md.clone()),
                    ShadowLength::Base(md.times(2.0).plus(Residual::base("space.sm"), 1.0)),
                ],
                color: None,
            }],
        };
        assert_eq!(
            s.css(),
            "0px space.md calc(space.md * 2 + space.sm) currentcolor"
        );
        assert_eq!(s.bases(), ["space.md", "space.sm"]);
        let px = |n: &str| match n {
            "space.md" => Some(16.0),
            "space.sm" => Some(8.0),
            _ => None,
        };
        assert_eq!(
            s.folded(&px).map(|s| s.css()).as_deref(),
            Some("0px 16px 40px currentcolor")
        );
        assert_eq!(s.folded(&|_| None), None);
    }
}
