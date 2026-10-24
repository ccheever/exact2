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
/// `lower-symbol-tint` warns). A source that may be either at run time is
/// answered by what it is then (`startsWith(source, "symbol:")`). None for a
/// raster (a tint paints it as a silhouette), or a symbol that writes
/// neither. `rows` are the author's, its classes' included, without the
/// sheet's.
pub(crate) fn symbol_ink(tag: &str, positional: &[Expr], rows: &[Attr], face: bool) -> Vec<Attr> {
    let ink = |name: &str, said: &Attr, written: bool| Attr {
        name: name.into(),
        value: Expr::Str("currentcolor".into(), said.span),
        span: said.span,
        written,
    };
    let Some(source) = positional.first().filter(|_| tag == "image" && !face) else {
        return Vec::new();
    };
    // (Its author's still: said as the tint was, `currentcolor` in its place.)
    let base = rows
        .iter()
        .rev()
        .find(|a| a.written && a.name == "-exact-tint-color");
    if !symbol(source) {
        // A source that may be either at run time (a state's, a choice of a symbol and a file): a
        // symbol's tint is answered when it is one, a raster's kept. A literal file is a raster.
        let literal = matches!(source, Expr::Str(..))
            || matches!(source, Expr::Template(parts, _) if matches!(parts.first(), Some(contract_syntax::TemplatePart::Text(_))));
        return base
            .filter(|_| !literal)
            .map(|tint| {
                let at = tint.span;
                let is_symbol = Expr::Call(
                    "startsWith".into(),
                    vec![source.clone(), Expr::Str("symbol:".into(), at)],
                    at,
                );
                Attr {
                    value: Expr::Ternary(
                        Box::new(is_symbol),
                        Box::new(Expr::Str("currentcolor".into(), at)),
                        Box::new(tint.value.clone()),
                        at,
                    ),
                    ..tint.clone()
                }
            })
            .into_iter()
            .collect();
    }
    let mut out: Vec<Attr> = match base {
        Some(tint) => vec![ink("-exact-tint-color", tint, tint.written)],
        // A symbol whose `color` its author wrote, with no tint of anyone's: that colour.
        None if !rows.iter().any(|a| a.name == "-exact-tint-color") => rows
            .iter()
            .rev()
            .find(|a| a.name == "color")
            .map(|color| ink("-exact-tint-color", color, color.written))
            .into_iter()
            .collect(),
        None => Vec::new(),
    };
    // Each state's authored tint, answered the same.
    for a in rows
        .iter()
        .filter(|a| a.written && a.name != "-exact-tint-color" && tinted(&a.name))
    {
        if !out.iter().any(|o| o.name == a.name) {
            let said = rows.iter().rev().find(|r| r.name == a.name).expect("named");
            out.push(ink(&a.name, said, said.written));
        }
    }
    out
}
