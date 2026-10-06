//! A text field's user-agent sheet (LLP 1104): a bare `input` or `textarea`
//! is a visible field, as the browser's is — a border, padding, a corner
//! radius, a fill and the ink that reads on it. The rows go under the
//! author's, as a grouped list's sheet does (LLP 1084 D7), so a class or an
//! attribute replaces any one of them; a literal `appearance="none"` leaves
//! them all out, the bare box LLP 1064 D6 drew.

use crate::tags::{attr, AttrTarget};
use crate::values::four_sided;
use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Span};
use exact_kernel::StyleId;

/// The field's outline and fill, iOS 27's measured system colours (a
/// grouped cell's fill; `separator` opaque, as a field's border is).
const BORDER: &str = "light-dark(#c6c6c8, #48484a)";
const FILL: &str = "light-dark(#ffffff, #1c1c1e)";
const INK: &str = "light-dark(#000000, #ffffff)";

/// The `type`s that are a text field one types into (D2). `hidden` paints
/// nothing; `color`, `month` and `week` lower as text fields but are not.
const TYPES: &[&str] = &[
    "text", "email", "password", "search", "tel", "url", "number",
];

fn typed(value: &Expr) -> bool {
    match value {
        Expr::Str(t, _) => TYPES.iter().any(|k| t.eq_ignore_ascii_case(k)),
        Expr::Ternary(_, a, b, _) => typed(a) && typed(b),
        _ => false,
    }
}

/// Whether a node with these rows (its class's, then its own) is a text
/// field the sheet dresses: a `textarea` that is not the Markdown editor,
/// or an `input` whose `type` is a text field's or absent.
fn field<'a>(tag: &str, rows: impl Iterator<Item = &'a Attr> + Clone) -> bool {
    let last = |name: &str| rows.clone().filter(|a| a.name == name).last();
    match tag {
        "textarea" => {
            last("markup").is_none_or(|a| matches!(&a.value, Expr::Str(m, _) if m == "none"))
        }
        "input" => last("type").is_none_or(|a| typed(&a.value)),
        _ => false,
    }
}

/// The sheet's rows for a text field, pushed ahead of `sheet`'s, or none:
/// for any other node, or one whose `appearance` (its class's, then its
/// own) is the literal `"none"`. `"auto"`, the platform's own field, is not
/// admitted yet (D3).
pub(crate) fn sheet<'a>(
    tag: &str,
    rows: impl Iterator<Item = &'a Attr> + Clone,
    span: Span,
    sheet: &mut Vec<Attr>,
) -> Result<(), LowerError> {
    if !field(tag, rows.clone()) {
        return Ok(());
    }
    match rows.filter(|a| a.name == "appearance").last() {
        None => {}
        Some(Attr {
            value: Expr::Str(v, _),
            ..
        }) if v == "none" => return Ok(()),
        Some(a) => {
            let what = match &a.value {
                Expr::Str(v, _) if v == "auto" => {
                    "`\"auto\"`, the platform's own field, is not admitted yet"
                }
                _ => "it is a literal",
            };
            return err(
                "lower-field-appearance",
                format!("a text field's `appearance` is `\"none\"` (the bare box) or absent (a visible field, LLP 1104): {what}. To switch between them, write `when` with two fields"),
                a.span,
            );
        }
    }
    let s = |name: &str, value: &str| Attr {
        name: name.into(),
        value: Expr::Str(value.into(), span),
        span,
    };
    let n = |name: &str, value: f64| Attr {
        name: name.into(),
        value: Expr::Number(value, span),
        span,
    };
    let mut rows = Vec::new();
    for side in ["top", "right", "bottom", "left"] {
        rows.push(n(&format!("border-{side}-width"), 1.0));
        rows.push(s(&format!("border-{side}-style"), "solid"));
        rows.push(s(&format!("border-{side}-color"), BORDER));
    }
    for corner in ["top-left", "top-right", "bottom-right", "bottom-left"] {
        rows.push(n(&format!("border-{corner}-radius"), 6.0));
    }
    for (side, inset) in [("top", 6.0), ("right", 8.0), ("bottom", 6.0), ("left", 8.0)] {
        rows.push(n(&format!("padding-{side}"), inset));
    }
    rows.push(s("background-color", FILL));
    rows.push(s("color", INK));
    sheet.splice(0..0, rows);
    Ok(())
}

/// A conditional class's rows over a sheet (D3): where one side of
/// `class=(c ? A : B)` leaves a row unset, that side is what the row's style
/// rows resolve to on that side — another of that style's rows that covers
/// it (`border` for `border-color`), else the sheet's last row that does —
/// not `none`, which the runner clears to the kernel's default, past the
/// sheet. So switching classes keeps the sheet's look and the style's own.
pub(crate) fn over_sheet(rows: &mut [Attr], sheet: &[Attr]) {
    let class = rows.to_vec();
    for (k, row) in rows.iter_mut().enumerate() {
        let name = row.name.clone();
        let Expr::Ternary(_, yes, no, span) = &mut row.value else {
            continue;
        };
        for (on_yes, side) in [(true, yes), (false, no)] {
            if !matches!(**side, Expr::None(_)) {
                continue;
            }
            let resolved = |id| {
                let styled = class.iter().enumerate().rev().filter(|(j, _)| *j != k);
                styled
                    .filter_map(|(_, q)| Some((q, side_of(&q.value, on_yes)?)))
                    .find_map(|(q, v)| at(&q.name, v, id))
                    .or_else(|| sheet.iter().rev().find_map(|s| at(&s.name, &s.value, id)))
            };
            if let Some(v) = spell(&name, resolved, *span) {
                **side = v;
            }
        }
    }
}

/// A class row's value on one side of its choice; `None` where it is unset.
fn side_of(value: &Expr, on_yes: bool) -> Option<&Expr> {
    let v = match value {
        Expr::Ternary(_, yes, no, _) => {
            if on_yes {
                &**yes
            } else {
                &**no
            }
        }
        v => v,
    };
    (!matches!(v, Expr::None(_))).then_some(v)
}

/// The style rows an attribute name writes, in its value's component order.
fn ids(name: &str) -> Option<&'static [StyleId]> {
    match attr(name)? {
        AttrTarget::Styles(ids) => Some(ids),
        AttrTarget::Shorthand if name.starts_with("border") => Some(crate::shorthands::rows(name)),
        _ => None,
    }
}

/// What `name=value` writes to the row `id`: a box shorthand's side, a
/// `border`'s component, or the value itself.
fn at(name: &str, value: &Expr, id: StyleId) -> Option<Expr> {
    let ids = ids(name)?;
    let i = ids.iter().position(|&x| x == id)?;
    if four_sided(ids) {
        return Some(match crate::values::sides(name, value).ok()? {
            Some(sides) => sides[i].clone(),
            None => value.clone(),
        });
    }
    if matches!(attr(name), Some(AttrTarget::Shorthand)) {
        return crate::shorthands::component(value, name, i).ok();
    }
    Some(value.clone())
}

/// `name`'s value spelled from what each of its rows resolves to: the one
/// value, a box shorthand's four (`"6px 8px 6px 8px"`), or a `border`'s
/// width, style and colour when its sides agree. `None` where a row is
/// unresolved or not a literal.
fn spell(name: &str, resolved: impl Fn(StyleId) -> Option<Expr>, span: Span) -> Option<Expr> {
    let ids = ids(name)?;
    let mut values = ids
        .iter()
        .map(|&id| resolved(id))
        .collect::<Option<Vec<_>>>()?;
    if values.len() == 1 {
        return values.pop();
    }
    let word = |e: &Expr| match e {
        Expr::Number(n, _) => Some(format!("{n}px")),
        Expr::Str(v, _) => Some(v.clone()),
        _ => None,
    };
    let words = values.iter().map(word).collect::<Option<Vec<_>>>()?;
    if four_sided(ids) {
        return Some(Expr::Str(words.join(" "), span));
    }
    if matches!(attr(name), Some(AttrTarget::Shorthand)) {
        // `border` and `border-<side>`: width, style and colour, a side at a time.
        let first = &words[..3];
        return words
            .chunks(3)
            .all(|side| side == first)
            .then(|| Expr::Str(first.join(" "), span));
    }
    words
        .iter()
        .all(|w| *w == words[0])
        .then(|| values.swap_remove(0))
}
