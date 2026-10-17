//! @ref LLP 1121 D3, D7 (stage 6 B) — length and number tokens on the JS
//! target. A token that varies by an app axis, and in a development build
//! every one, is a custom property as a colour is (`modes.rs`): its value per
//! tuple is the CSS the target writes for the literal (a `rem` stays `rem`,
//! a text style is its `rem`, the hairline the sheet's property), and a row
//! naming it writes `var()`, found through a sentinel the plain projection
//! never writes. A bound row's value naming one is mapped at run time.

use exact_kernel::style::relative::{self, Unit};
use exact_kernel::style::theme::Kind;
use exact_kernel::{StyleCodec, StyleId};
use exact_plan::{BindingKind, Plan};

/// The plan's kind code for `kind` ([`Kind::of`]'s inverse).
fn code(kind: Kind) -> u8 {
    match kind {
        Kind::Color => 0,
        Kind::Length => 1,
        Kind::Number => 2,
    }
}

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

/// Whether a row may read a length or number token (the rows the compiler
/// admits one in: a length, a line height, a number).
pub fn row(id: StyleId) -> bool {
    matches!(
        id.codec(),
        StyleCodec::Dimension | StyleCodec::LineHeight | StyleCodec::F32 | StyleCodec::U16
    )
}

/// Whether the plan keeps a length or number token.
pub fn any(plan: &Plan) -> bool {
    plan.theme
        .iter()
        .any(|r| kind(r).is_some_and(|k| k != Kind::Color))
}

/// The number tokens a literal names in a row that takes an integer
/// (`font-weight`): no sentinel fraction stands in for them, so they stay
/// inline, and a live edit of one waits for the rebuild.
pub fn integer_tokens(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for node in &plan.nodes {
        for b in node.bindings.iter() {
            let b = plan.binding(b);
            if b.kind != BindingKind::Style || b.id != StyleId::FontWeight as u16 {
                continue;
            }
            for r in plan.theme.iter().filter(|r| r.kind == code(Kind::Number)) {
                let name = plan.str(r.name);
                if crate::style::can_be(plan, plan.code(b.expr), &|s| s.trim() == name)
                    && !out.iter().any(|n| n == name)
                {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

/// Token `i`'s sentinel of `kind` at offset `k`: the value text a parse
/// reads, and the CSS the projection writes for it.
pub fn sentinel(kind: Kind, k: u32, i: usize) -> [String; 2] {
    match kind {
        // A px length no app writes: thousands, and a quarter.
        Kind::Length => {
            let px = 7000.0 + f64::from(k) * 64.0 + i as f64 + 0.25;
            [
                format!("{px}px"),
                format!("{}px", exact_web::css::num(px as f32)),
            ]
        }
        // A fraction in (0, 1): an opacity, a flex factor and a press scale
        // take it.
        _ => {
            let n = (0.000_1 + f64::from(k) * 0.000_01 + i as f64 * 0.000_000_1) as f32;
            [format!("{n}"), exact_web::css::num(n)]
        }
    }
}

/// The bound rows' map: each length or number token's CSS, or its
/// property's `var()` where the page declares one (`props`, name to
/// property). `None` when the plan keeps none.
pub fn map(plan: &Plan, props: &[(String, String)]) -> Option<String> {
    let pairs: Vec<String> = plan
        .theme
        .iter()
        .filter_map(|r| {
            let kind = kind(r).filter(|k| *k != Kind::Color)?;
            let name = plan.str(r.name);
            let value = match props.iter().find(|(n, _)| n == name) {
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
