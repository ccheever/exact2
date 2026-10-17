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
    if kind != Kind::Length {
        return value.trim().to_string();
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

/// Whether a row may read a length or number token: those the kernel
/// resolves one in (`style::theme::admits`).
pub fn row(id: StyleId) -> bool {
    use exact_kernel::style::theme::admits;
    admits(Kind::Length, id) || admits(Kind::Number, id)
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
            for r in plan
                .theme
                .iter()
                .filter(|r| matches!(kind(r), Some(Kind::Length | Kind::Number)))
            {
                let name = plan.str(r.name);
                if crate::style::can_be(plan, plan.code(b.expr), &|s| s.trim() == name)
                    && !out.iter().any(|(n, _)| n == name)
                {
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
        "$t{}{}{}",
        if admits(Kind::Length, id) { "l" } else { "" },
        if admits(Kind::Number, id) { "n" } else { "" },
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
    let pairs: Vec<String> = plan
        .theme
        .iter()
        .filter_map(|r| {
            let kind = kind(r)
                .filter(|k| *k != Kind::Color && exact_kernel::style::theme::admits(*k, id))?;
            // An SVG attribute reads no property, and no tuple's rule reaches
            // it: a varying token's name stands there (the JS target does not
            // read one computed into an attribute).
            if attribute && r.slices.len > 0 {
                return None;
            }
            let name = plan.str(r.name);
            let value = match props.iter().find(|(n, _)| n == name && !attribute) {
                Some((_, prop)) => serde_json::to_string(&format!("var({prop})")).unwrap(),
                None if kind == Kind::Number => css(kind, plan.str(r.value)),
                None => serde_json::to_string(&css(kind, plan.str(r.value))).unwrap(),
            };
            Some(format!("{}:{value}", serde_json::to_string(name).unwrap()))
        })
        .collect();
    (!pairs.is_empty()).then(|| {
        format!(
            "(M=>v=>typeof v===\"string\"&&Object.hasOwn(M,v.trim())?M[v.trim()]:v)({{{}}})",
            pairs.join(",")
        )
    })
}
