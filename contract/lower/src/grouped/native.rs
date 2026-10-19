//! A native button as a grouped list's row (LLP 1069.011, LLP 1084 D7):
//! the platform's control at the row's inset, no wider than the card less
//! that inset.

use super::{m, metric, s, split};
use contract_syntax::{Attr, Expr, Span};

/// The sheet rows a native button takes (LLP 1069.011 D6), for a row whose
/// class made it one: its inset and height, and nothing it refuses.
/// `margins` says whether its author wrote a horizontal margin.
pub(crate) fn native_rows(sheet: &mut Vec<Attr>, margins: bool) {
    let Some(span) = sheet.first().map(|a| a.span) else {
        return;
    };
    // Its own inset, which knows a leading symbol, outside its box as a
    // native row's is, and its widest beside it; the rest is a button's.
    sheet.retain(|a| a.name == "padding-left" || a.name == "margin-left");
    for a in sheet.iter_mut() {
        a.name = "margin-left".into();
    }
    let widest = sheet.last().filter(|_| !margins).map(|inset| Attr {
        name: "max-width".into(),
        value: widest(&inset.value),
        ..inset.clone()
    });
    sheet.extend(widest);
    sheet.extend(split(&native_row(span, true)[2..]).0);
}

/// Whether an element's rows say a horizontal margin: then its author places
/// it in its row, and the sheet's widest for a native button gives way.
pub(crate) fn margins_said(attrs: &[Attr]) -> bool {
    attrs.iter().any(|a| {
        matches!(
            a.name.as_str(),
            "margin"
                | "margin-left"
                | "margin-right"
                | "margin-inline"
                | "margin-inline-start"
                | "margin-inline-end"
        )
    })
}

/// A native button row's widest for its leading inset `inset`: the card less
/// that inset (`grouped-button-max-width`), a symbol's too, so a row its
/// author makes as wide as the card stays in it.
fn widest(inset: &Expr) -> Expr {
    match inset {
        Expr::Ternary(c, a, b, span) => {
            Expr::Ternary(c.clone(), Box::new(widest(a)), Box::new(widest(b)), *span)
        }
        Expr::Str(v, span) => {
            // A destructive row's inset is its push button's padding on the
            // Mac (`grouped-button-padding-start`), and its widest that.
            let name = [
                ("row-inset", "button-max-width"),
                ("row-inset-symbol", "button-max-width-symbol"),
                ("button-padding-start", "button-max-width-pushed"),
                (
                    "button-padding-start-symbol",
                    "button-max-width-pushed-symbol",
                ),
            ]
            .into_iter()
            .find(|(inset, _)| v == &metric(inset))
            .map_or("button-max-width", |(_, widest)| widest);
            Expr::Str(metric(name), *span)
        }
        other => other.clone(),
    }
}

/// A native button row's sheet (LLP 1069.011): the platform's control, at
/// the row's inset, no wider than the card less its insets, and the height a
/// row's button has. Where a platform's
/// form shows a button at its own size (the Mac's, LLP 1115.002 D6), its
/// margins and its place are metrics too, and are no row elsewhere.
pub(super) fn native_row(span: Span, margins: bool) -> Vec<Attr> {
    // No wider than the card less its inset, so a native button its author
    // makes as wide as the row stays in the card (LLP 1084 D7); stretched
    // or at its own size, it is as wide as before. Where its author placed
    // it with a margin, the author's.
    let widest = if margins {
        s("max-width", "none", span)
    } else {
        m("max-width", "button-max-width", span)
    };
    vec![
        m("margin-left", "row-inset", span),
        widest,
        m("min-height", "button-min-height", span),
        m("margin-top", "button-margin-block", span),
        m("margin-bottom", "button-margin-block", span),
        m("align-self", "button-align", span),
    ]
}
