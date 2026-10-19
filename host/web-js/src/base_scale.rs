//! @ref LLP 1121 stage 11 B (B2) — the base scale on the JS target, which
//! keeps no kernel at run time. Its one look is the neutral one (the build
//! says so, `roles::set_column`), so its column is the kernel's in this
//! build (`metrics::column()`):
//! - a static row naming a base name is that column's length, as the kernel
//!   reads the literal while the sheet is projected (`theme::measure`);
//! - the sheet declares each base name the plan names, and the design does
//!   not define, once as a custom property on `:root`, `--exact-<name>`
//!   (`--exact-space-md`), from the kernel's table, never a copied number;
//! - a bound row's value naming one reads that property (`measures::map`).
//!
//! A design that defines a base name makes it its own token, which the
//! token path writes (`modes`, `measures`). The web has no iOS or Mac look
//! class to declare a second column under; one would add its rules here.

use exact_kernel::style::{metrics, scale};
use exact_plan::Plan;

/// The base names `plan` names (a string any of its rows or code may write)
/// and its design does not define, in the table's order.
pub fn named(plan: &Plan) -> Vec<&'static str> {
    scale::names()
        .filter(|n| plan.strings.iter().any(|s| s.trim() == *n))
        .filter(|n| !plan.theme.iter().any(|r| plan.str(r.name) == *n))
        .collect()
}

/// A base name's custom property: `--exact-space-md`.
pub fn property(name: &str) -> String {
    format!("--exact-{}", name.replace('.', "-"))
}

/// Base name `name`'s CSS in this build's column (the kernel's table).
pub fn value(name: &str) -> String {
    scale::css(scale::of(name, metrics::column()).expect("a base name"))
}

/// The sheet's declarations: `:root{--exact-space-md:16px;…}` for each base
/// name the plan names, nothing when it names none.
pub fn css(plan: &Plan) -> String {
    let names = named(plan);
    if names.is_empty() {
        return String::new();
    }
    let declarations: String = names
        .iter()
        .map(|n| format!("{}:{};", property(n), value(n)))
        .collect();
    format!(":root{{{declarations}}}")
}

/// A bound row's map entries for the base names the plan names: each its
/// property's `var()`, or its length where the row is an SVG attribute,
/// which reads no property.
pub fn pairs(plan: &Plan, attribute: bool) -> Vec<String> {
    named(plan)
        .into_iter()
        .map(|n| {
            let css = if attribute {
                value(n)
            } else {
                format!("var({})", property(n))
            };
            format!(
                "{}:{}",
                serde_json::to_string(n).unwrap(),
                serde_json::to_string(&css).unwrap()
            )
        })
        .collect()
}
