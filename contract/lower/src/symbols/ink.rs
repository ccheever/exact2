//! A symbol's ink as the hosts draw it: its `color` (LLP 1081 stage 2).

use super::tint::{symbol, tinted};
use contract_syntax::{Attr, Expr};

/// @ref LLP 1116.019 D6 — a `color` the author wrote on a symbol `image`
/// (or its class) is the symbol's colour, as an inline SVG icon drawn in
/// `currentcolor` follows `color` on the web: the row this returns,
/// `-exact-tint-color: currentcolor`, said as the `color` was. A symbol is
/// drawn in its `color` on every host (LLP 1081 stage 2), so an authored
/// `-exact-tint-color` on one that is no native button's `face` (which keeps
/// its tint, LLP 1115 D4) is answered the same, after it, each state's row
/// too: Apple already ignores it, and the web now does (theme2 2026-10-23;
/// `lower-symbol-tint` warns). None for a raster (a tint paints it as a
/// silhouette), or a symbol that writes neither. `rows` are the author's,
/// its classes' included, without the sheet's.
pub(crate) fn symbol_ink(tag: &str, positional: &[Expr], rows: &[Attr], face: bool) -> Vec<Attr> {
    let ink = |name: &str, said: &Attr, written: bool| Attr {
        name: name.into(),
        value: Expr::Str("currentcolor".into(), said.span),
        span: said.span,
        written,
    };
    if tag != "image" || !positional.first().is_some_and(symbol) {
        return Vec::new();
    }
    if rows.iter().any(|a| tinted(&a.name)) {
        if face {
            return Vec::new();
        }
        let mut names: Vec<&str> = Vec::new();
        for a in rows.iter().filter(|a| a.written && tinted(&a.name)) {
            if !names.contains(&a.name.as_str()) {
                names.push(&a.name);
            }
        }
        return names
            .into_iter()
            .map(|name| {
                ink(
                    name,
                    rows.iter().rev().find(|a| a.name == name).expect("named"),
                    false,
                )
            })
            .collect();
    }
    rows.iter()
        .rev()
        .find(|a| a.name == "color")
        .map(|color| ink("-exact-tint-color", color, color.written))
        .into_iter()
        .collect()
}
