//! A text field's user-agent sheet (LLP 1104): a bare `input` or `textarea`
//! is a visible field, as the browser's is — a border, padding, a corner
//! radius, a fill and the ink that reads on it. The rows go under the
//! author's, as a grouped list's sheet does (LLP 1084 D7), so a class or an
//! attribute replaces any one of them; a literal `appearance="none"` leaves
//! them all out, the bare box LLP 1064 D6 drew.

use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Span};

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
/// `class=(c ? A : B)` leaves a row the sheet writes unset, that side is the
/// sheet's value, not `none` (which the runner clears to the kernel's
/// default, past the sheet), so switching classes keeps the sheet's look.
pub(crate) fn over_sheet(rows: &mut [Attr], sheet: &[Attr]) {
    for row in rows {
        if !matches!(row.value, Expr::Ternary(..)) {
            continue;
        }
        let Some(under) = under(&row.name, sheet) else {
            continue;
        };
        if let Expr::Ternary(_, yes, no, _) = &mut row.value {
            for side in [yes, no] {
                if matches!(**side, Expr::None(_)) {
                    **side = under.clone();
                }
            }
        }
    }
}

/// What the sheet writes for `name`: its own row, or a shorthand spelled
/// from the sheet's longhands when it writes every one — `padding` as
/// `"6px 8px 6px 8px"`, `border` as `"1px solid <colour>"` when its sides
/// agree.
fn under(name: &str, sheet: &[Attr]) -> Option<Expr> {
    use crate::tags::{attr, AttrTarget};
    if let Some(row) = sheet.iter().find(|s| s.name == name) {
        return Some(row.value.clone());
    }
    let ids = match attr(name)? {
        AttrTarget::Styles(ids) if crate::values::four_sided(ids) => ids,
        AttrTarget::Shorthand if name.starts_with("border") => crate::shorthands::rows(name),
        _ => return None,
    };
    let word = |id| {
        let row = sheet
            .iter()
            .find(|s| matches!(attr(&s.name), Some(AttrTarget::Styles([only])) if *only == id))?;
        match &row.value {
            Expr::Number(n, _) => Some(format!("{n}px")),
            Expr::Str(v, _) => Some(v.clone()),
            _ => None,
        }
    };
    let words = ids.iter().map(|&id| word(id)).collect::<Option<Vec<_>>>()?;
    let span = sheet.first()?.span;
    if crate::values::four_sided(ids) {
        return Some(Expr::Str(words.join(" "), span));
    }
    // `border` and `border-<side>`: width, style and colour, a side at a time.
    let first = &words[..3];
    words
        .chunks(3)
        .all(|side| side == first)
        .then(|| Expr::Str(first.join(" "), span))
}
