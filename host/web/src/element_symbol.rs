//! How a symbol image's path is filled, as its element's attributes say
//! (LLP 1121 D10, stage 14 C2): moved out of element.rs under its line cap.

use exact_kernel::SortedMap;

/// A filled path is a silhouette, drawn filled, not stroked: a role's
/// even-odd, an icon's paths each by its rule (`d`s joined by `;`): nonzero,
/// or even-odd where its SVG says so (`data-symbol-evenodd`; glue.js writes
/// each `<path>`'s `fill-rule` from it). A nonzero icon's attributes are as
/// before. `role`: the source after `symbol:`; `symbol`: its path.
pub(super) fn fill_marks(
    out: &mut SortedMap<String, String>,
    role: &str,
    symbol: Option<exact_kernel::SymbolPath<'_>>,
) {
    if symbol.is_some_and(|s| s.filled) {
        out.insert("data-symbol-fill".into(), String::new());
    }
    if let Some(s) = symbol.filter(|s| s.filled && !s.even_odd) {
        out.insert("data-symbol-nonzero".into(), String::new());
        out.insert("data-symbol-viewbox".into(), s.view_box.into());
    } else if let Some(s) =
        symbol.filter(|s| s.filled && s.even_odd && exact_kernel::Icon::parse(role).is_some())
    {
        out.insert("data-symbol-evenodd".into(), String::new());
        out.insert("data-symbol-viewbox".into(), s.view_box.into());
    }
}
