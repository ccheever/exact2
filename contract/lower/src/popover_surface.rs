//! A popover's default surface (LLP 1021; theme2 2026-10-21 18:45, 19:55):
//! an author who writes no surface on a `popover` gets the platform's, as
//! the web's UA style gives one a filled, bordered card and the Mac its
//! system popover. Only the compiler sees which rows the author wrote, so
//! it decides, and marks the node (`defaultSurface`); a host that paints the
//! popover itself draws the platform's surface under it (iOS's glass, from
//! the metrics' `popover-` rows), and inspect can say why.

use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Span};

/// Whether `name` is a row that draws a box's surface: its background, a
/// border's line or a shadow (a border's corner draws none), or a material,
/// in any state (`:hover background-color`, and `-exact-press-background-color`,
/// the press's own spelling, are the author's surface too).
fn surface(name: &str) -> bool {
    let name = contract_syntax::state::state_of(name).map_or(name, |(_, row)| row);
    let name = name.strip_prefix("-exact-press-").unwrap_or(name);
    name.starts_with("background") && !matches!(name, "background-clip" | "background-attachment")
        || name == "box-shadow"
        || name.starts_with("border") && !name.ends_with("radius")
}

/// The mark on a popover whose author wrote no surface on it or its class
/// (a row the compiler implied, a sheet's, is no author's). A menu
/// (`role="menu"`) is the platform's menu, never painted, so it has none.
pub(crate) fn implied(attrs: &[Attr], span: Span) -> Option<Attr> {
    let menu = attrs
        .iter()
        .any(|a| a.name == "role" && matches!(&a.value, Expr::Str(r, _) if r == "menu"));
    let popover = attrs.iter().any(|a| a.name == "popover") && !menu;
    let said = attrs.iter().any(|a| a.written && surface(&a.name));
    (popover && !said).then(|| Attr {
        name: "defaultSurface".into(),
        value: Expr::Bool(true, span),
        span,
        written: crate::IMPLIED,
    })
}

/// `defaultSurface` is the compiler's: an author's would claim the
/// platform's surface where they wrote their own, or none on a node no
/// popover is.
pub(crate) fn check_authored(attrs: &[Attr]) -> Result<(), LowerError> {
    match attrs.iter().find(|a| a.name == "defaultSurface" && a.written) {
        Some(a) => err(
            "lower-default-surface",
            "`defaultSurface` is the compiler's: a `popover` whose author writes no background, border or shadow gets the platform's surface. Write a surface of your own to replace it",
            a.span,
        ),
        None => Ok(()),
    }
}
