//! @ref LLP 1121 D3, D7 (stage 6 B) — length and number tokens on the JS
//! target. A token that varies by an app axis, and in a development build
//! every one, is a custom property as a colour is (`modes.rs`): its value per
//! tuple is the CSS the target writes for the literal (a `rem` stays `rem`,
//! a text style is its `rem`, the hairline the sheet's property), and a row
//! naming it writes `var()`, found through a sentinel the plain projection
//! never writes. A bound row's value naming one is mapped at run time.

use exact_kernel::style::relative::{self, Unit};
use exact_kernel::style::theme::Kind;
use exact_kernel::StyleId;
use exact_plan::{BindingKind, Plan};

/// A theme row's kind, when this build knows it.
pub fn kind(row: &exact_plan::ThemeRow) -> Option<Kind> {
    Kind::of(row.kind)
}

/// The CSS a length or number token's `value` is, as the target writes the
/// literal on a row: a length's text, a text style as its `rem`, the
/// hairline as the sheet's property; a number's text.
pub fn css(kind: Kind, value: &str) -> String {
    // A shadow is its row's CSS, as css.rs writes the literal's.
    match kind {
        Kind::BoxShadow => {
            return exact_kernel::style::BoxShadows::parse(value)
                .map_or_else(|| value.trim().to_string(), |s| s.css())
        }
        Kind::TextShadow => {
            return exact_kernel::style::TextShadow::parse(value)
                .map_or_else(|| value.trim().to_string(), |s| s.css())
        }
        Kind::Length => {}
        _ => return value.trim().to_string(),
    }
    match relative::parse(value) {
        Some((Unit::TextStyle(id), n)) => {
            let medium = relative::MEDIUM;
            format!(
                "{}rem",
                exact_web::css::num(n * Unit::TextStyle(id).basis(medium, medium, 1.0) / medium)
            )
        }
        Some((Unit::Hairline, _)) => exact_web::css::HAIRLINE.to_string(),
        _ => value.trim().to_string(),
    }
}

/// Row `id`'s part of type token `value`, or family token `value`'s stack,
/// as the bound row's own map reads it; `None` where it writes none. A
/// family is its plan stack's CSS, as a literal's (a bundled font's
/// `ExactPlanStack<n>` alias with its fallbacks); a text style stays its
/// name, which the size's map turns into its `rem` and the weight and
/// leading the style gives (`rows.rs`).
fn font_part(plan: &Plan, kind: Kind, id: StyleId, value: &str) -> Option<String> {
    use exact_kernel::style::theme::typeface;
    let family = |text: &str| {
        let stack = exact_runner::bridge::plan_stack(plan, text)?;
        exact_web::css::font_family_names(plan)
            .get(usize::from(stack))
            .cloned()
    };
    if kind == Kind::Family {
        return family(value);
    }
    let p = typeface::parts(value)?;
    Some(match id {
        StyleId::FontStyle => p.style?.to_string(),
        StyleId::FontWeight => p.weight?.to_string(),
        StyleId::FontSize => p.size.to_string(),
        StyleId::LineHeight => p.leading?.to_string(),
        StyleId::FontFamily => family(p.family?)?,
        _ => return None,
    })
}

/// Whether a row may read a length, number or shadow token: those the kernel
/// resolves one in (`style::theme::admits`).
pub fn row(id: StyleId) -> bool {
    use exact_kernel::style::theme::admits;
    [
        Kind::Length,
        Kind::Number,
        Kind::BoxShadow,
        Kind::TextShadow,
        Kind::Type,
        Kind::Family,
    ]
    .into_iter()
    .any(|k| admits(k, id))
}

/// The tokens the page must write inline, each with what writes it: a
/// number a `font-weight` names (no sentinel fraction is a weight), and a
/// length or number an SVG attribute row names (`x`, `width`, `r` …: an
/// attribute reads no property). In a development build a change to one
/// builds (the design says `inline`); one that varies by an app axis is
/// refused ([`refuse_inline`]).
pub fn inline_tokens(plan: &Plan) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for node in &plan.nodes {
        let attributes = exact_kernel::NodeType::from_wire(node.node_type)
            .map_or(&[][..], exact_web::host::template::attribute_rows);
        for b in node.bindings.iter() {
            let b = plan.binding(b);
            if b.kind != BindingKind::Style {
                continue;
            }
            let writer = match attributes.iter().find(|(r, _)| *r as u16 == b.id) {
                Some((_, attribute)) => format!("an SVG `{attribute}` attribute"),
                None if b.id == StyleId::FontWeight as u16 => "`font-weight`".to_string(),
                None => continue,
            };
            // A static row's: the sentinel projection writes it. A bound
            // value maps through the row's map instead ([`map`]).
            let Some(literal) = crate::style::literal(plan, plan.code(b.expr)) else {
                continue;
            };
            let Some(text) = literal.as_str().map(str::trim) else {
                continue;
            };
            for r in plan
                .theme
                .iter()
                .filter(|r| matches!(kind(r), Some(Kind::Length | Kind::Number)))
            {
                let name = plan.str(r.name);
                if text == name && !out.iter().any(|(n, _)| n == name) {
                    out.push((name.to_string(), writer.clone()));
                }
            }
        }
    }
    out
}

/// A token the page writes inline that varies by an app axis: no tuple's
/// rule can reach it on the JS target, so the build is refused, naming it.
pub fn refuse_inline(plan: &Plan) -> Result<(), String> {
    for (name, writer) in inline_tokens(plan) {
        let varies = plan
            .theme
            .iter()
            .any(|r| plan.str(r.name) == name && r.slices.len > 0);
        if varies {
            return Err(format!("`{name}` varies by an app axis, and {writer} reads no design property on the JS target (LLP 1121 D7): give it one value, or write the row from state"));
        }
    }
    Ok(())
}

/// The rows a type token may write, each with its CSS property and the
/// suffix of the property a page declares it in (`--x-type-body-size`).
pub const TYPE_ROWS: [(StyleId, &str, &str); 5] = [
    (StyleId::FontStyle, "font-style", "style"),
    (StyleId::FontWeight, "font-weight", "weight"),
    (StyleId::FontSize, "font-size", "size"),
    (StyleId::LineHeight, "line-height", "leading"),
    (StyleId::FontFamily, "font-family", "family"),
];

/// Theme row `row`'s text in app tuple `tuple`: its slice where it varies.
pub fn value_at<'a>(plan: &'a Plan, row: &exact_plan::ThemeRow, tuple: u32) -> &'a str {
    match row.slices.len {
        0 => plan.str(row.value),
        len => plan.str(plan.theme_slices[(row.slices.start + tuple.min(len - 1)) as usize].value),
    }
}

/// Row `id`'s part of type or family token text `value` as a page's rule
/// declares it (LLP 1121 stage 6 C, as properties): what the template writes
/// for the literal's part, a size or a line height that is a length as its
/// CSS (a text style its `rem`), a family its plan stack's.
pub fn part_css(plan: &Plan, kind: Kind, id: StyleId, value: &str) -> Option<String> {
    let part = font_part(plan, kind, id, value)?;
    Some(match id {
        StyleId::FontSize => css(Kind::Length, &part),
        StyleId::LineHeight if part.parse::<f64>().is_err() => css(Kind::Length, &part),
        _ => part,
    })
}

/// Each type or family token the page declares as properties, one a row it
/// writes (LLP 1121 stage 6 C's follow-up): a token that varies by an app
/// axis, and in a development build every one; `(name, row, property)`. A
/// family token's one row is its name's property, `--x-<name>`; a type
/// token's `--x-<name>-<part>`. Every tuple writes the same rows
/// (`tamber-type-longhands`), so the default tuple's say which.
pub fn type_properties(plan: &Plan, every: bool) -> Vec<(String, StyleId, String)> {
    let mut out = Vec::new();
    for r in plan.theme.iter() {
        let Some(kind @ (Kind::Type | Kind::Family)) = kind(r) else {
            continue;
        };
        if r.slices.len == 0 && !every {
            continue;
        }
        let name = plan.str(r.name);
        let base: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        for (id, _, part) in TYPE_ROWS {
            if kind == Kind::Family && id != StyleId::FontFamily
                || font_part(plan, kind, id, value_at(plan, r, 0)).is_none()
            {
                continue;
            }
            let prop = match kind {
                Kind::Family => format!("--x-{base}"),
                _ => format!("--x-{base}-{part}"),
            };
            out.push((name.to_string(), id, prop));
        }
    }
    out
}

/// Token `i`'s sentinel of `kind` at offset `k`: the value text a parse
/// reads first, then the CSS texts the projection may write for it, the
/// longer first (a length is unitless in an SVG stroke's CSS). Every one is
/// as long as every other of its kind, so none is another's substring.
pub fn sentinel(kind: Kind, k: u32, i: usize) -> Vec<String> {
    let n = k as usize * 64 + i;
    match kind {
        // A px length no app writes: six digits and a quarter.
        Kind::Length => {
            let px = format!("7{n:05}.25");
            vec![format!("{px}px"), format!("{px}px"), px]
        }
        // A shadow whose spread (or a text shadow's offset) is such a length:
        // its marker, which `modes` finds in the declaration it writes.
        Kind::BoxShadow | Kind::TextShadow => {
            let px = format!("7{n:05}.25px");
            let value = match kind {
                Kind::BoxShadow => format!("0px 0px 0px {px} #000000ff"),
                _ => format!("{px} 0px #000000ff"),
            };
            vec![value, px]
        }
        // A fraction in (0, 1) an opacity, a flex factor and a press scale
        // take, its last digit a 9, so no zero is dropped.
        _ => {
            let text = format!("0.0007{n:04}9");
            let css = text
                .parse::<f32>()
                .map_or_else(|_| text.clone(), exact_web::css::num);
            vec![text, css]
        }
    }
}

/// The name of row `id`'s bound map ([`map`]): one per mix of the kinds it
/// takes, and an attribute row's of its own.
pub fn map_name(id: StyleId, attribute: bool) -> String {
    use exact_kernel::style::theme::admits;
    format!(
        "$t{}{}{}{}{}{}",
        if admits(Kind::Length, id) { "l" } else { "" },
        if admits(Kind::Number, id) { "n" } else { "" },
        if admits(Kind::BoxShadow, id) { "b" } else { "" },
        if admits(Kind::TextShadow, id) {
            "t"
        } else {
            ""
        },
        // A type token's part is the row's own (stage 6 C).
        if admits(Kind::Type, id) {
            format!("y{}", id as u16)
        } else {
            String::new()
        },
        if attribute { "a" } else { "" }
    )
}

/// Row `id`'s bound map: each length or number token of a kind the row
/// takes (`style::theme::admits`; a name of another kind stands, for the
/// browser to refuse, as the kernel leaves it), as its CSS, or its
/// property's `var()` where the page declares one (`props`, name to
/// property) and the row is no SVG attribute. `None` when the plan keeps
/// no such token.
pub fn map(
    plan: &Plan,
    props: &[(String, String)],
    id: StyleId,
    attribute: bool,
) -> Option<String> {
    let types = type_properties(plan, crate::modes::every_token());
    let pairs: Vec<String> = plan
        .theme
        .iter()
        .filter_map(|r| {
            let kind = kind(r)
                .filter(|k| *k != Kind::Color && exact_kernel::style::theme::admits(*k, id))?;
            // An SVG attribute reads no property, and no tuple's rule reaches
            // it: a varying token's name stands there (the JS target does not
            // read one computed into an attribute). Nor does an inherited row
            // read one, as the kernel reads none (LLP 1121 D7).
            let name = plan.str(r.name);
            // A type or family token's part the page declares: its property
            // (a text with no element children reads it, stage 6 C).
            if matches!(kind, Kind::Type | Kind::Family) && !attribute {
                if let Some((.., prop)) = types.iter().find(|(n, row, _)| n == name && *row == id) {
                    let var = serde_json::to_string(&format!("var({prop})")).unwrap();
                    return Some(format!("{}:{var}", serde_json::to_string(name).unwrap()));
                }
            }
            if (attribute || id.inherited()) && r.slices.len > 0 {
                return None;
            }
            // A type token's part for this row, a family's stack, as the
            // template writes the literal's (stage 6 C); a part it does not
            // write stands, for the browser to refuse, as the kernel leaves it.
            if matches!(kind, Kind::Type | Kind::Family) {
                let css = font_part(plan, kind, id, plan.str(r.value))?;
                return Some(format!(
                    "{}:{}",
                    serde_json::to_string(name).unwrap(),
                    serde_json::to_string(&css).unwrap()
                ));
            }
            let value = match props.iter().find(|(n, _)| n == name && !attribute) {
                Some((_, prop)) => serde_json::to_string(&format!("var({prop})")).unwrap(),
                None if kind == Kind::Number => css(kind, plan.str(r.value)),
                // A shadow's text is a string, whatever it holds.
                None => serde_json::to_string(&css(kind, plan.str(r.value))).unwrap(),
            };
            Some(format!("{}:{value}", serde_json::to_string(name).unwrap()))
        })
        .collect();
    // A text shadow is a colour channel a mode boundary writes again: a row
    // that read a token says so, as the colour map's does (`$th`, modes.js).
    let read = if id == StyleId::TextShadow && crate::modes::active(plan) {
        "((globalThis.exact??={}).$th=1,M[v.trim()])"
    } else {
        "M[v.trim()]"
    };
    (!pairs.is_empty()).then(|| {
        format!(
            "(M=>v=>typeof v===\"string\"&&Object.hasOwn(M,v.trim())?{read}:v)({{{}}})",
            pairs.join(",")
        )
    })
}
