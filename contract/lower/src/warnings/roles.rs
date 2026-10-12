//! What a button or a role promises a person and VoiceOver, and the
//! element does not keep (LLP 1116.004 D4): a button with nothing to say,
//! and a dialog's role on a box; and a heading's level with no heading
//! (LLP 1115.000 D12.5).

use super::{computed, has, literal, BOXES};
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, Span};

/// Whether an element names itself: a written `aria-label` or
/// `aria-labelledby` that is not the empty string.
fn named(attrs: &[Attr]) -> bool {
    ["aria-label", "aria-labelledby"].iter().any(|name| {
        attrs
            .iter()
            .rev()
            .find(|a| a.name == *name)
            .is_some_and(|a| !matches!(&a.value, Expr::Str(v, _) if v.trim().is_empty()))
    })
}

/// What a button's content gives it to say.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Says {
    /// Nothing drawn and nothing read.
    Nothing,
    /// An image or `svg` with no name: drawn, and read as nothing.
    Unnamed,
    /// Words, a name, or what the compiler cannot see through.
    Words,
}

/// The most any of `nodes` says. A `when`, `each` or `match` says what its
/// most telling arm does; a use or a slot, and an element this walk does
/// not know, are given the benefit of the doubt.
fn says(nodes: &[Node], in_svg: bool) -> Says {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                if named(attrs) {
                    return Says::Words;
                }
                match tag.as_str() {
                    "text" | "tspan" => match positional.first() {
                        Some(Expr::Str(v, _)) if v.trim().is_empty() => says(children, in_svg),
                        Some(_) => Says::Words,
                        None => says(children, in_svg),
                    },
                    "image" => match attrs.iter().rev().find(|a| a.name == "alt") {
                        Some(a) if !matches!(&a.value, Expr::Str(v, _) if v.trim().is_empty()) => {
                            Says::Words
                        }
                        _ => Says::Unnamed,
                    },
                    // An `svg`'s shapes are drawn and read as nothing; its
                    // `text` is words.
                    "svg" => says(children, true).max(Says::Unnamed),
                    t if BOXES.contains(&t) => says(children, in_svg),
                    _ if in_svg => says(children, in_svg),
                    _ => Says::Words,
                }
            }
            Node::When {
                then, otherwise, ..
            } => says(then, in_svg).max(says(otherwise, in_svg)),
            Node::Each { body, .. } => says(body, in_svg),
            Node::Match { some, none, .. } => says(&some.1, in_svg).max(says(none, in_svg)),
            Node::Use { .. } | Node::Children { .. } => Says::Words,
        })
        .max()
        .unwrap_or(Says::Nothing)
}

/// `lower-button-unlabelled`: a `button` with no text anywhere in it, no
/// `aria-label` or `aria-labelledby`, and no image with an `alt` (LLP
/// 1116.004 D4). It is blank: water-tracker's `button press=drink(500)`
/// with no child drew an empty button, and VoiceOver reads one as
/// "button". A text of any expression, a symbol image with a name, and a
/// button hidden from assistive technology are not named.
pub(super) fn unlabelled_button(
    attrs: &[Attr],
    children: &[Node],
    span: Span,
) -> Option<LowerError> {
    let hidden = attrs
        .iter()
        .rev()
        .find(|a| a.name == "aria-hidden")
        .is_some_and(|a| match &a.value {
            Expr::Bool(b, _) => *b,
            Expr::Str(v, _) => v != "false",
            _ => true,
        });
    if named(attrs) || hidden {
        return None;
    }
    let message = match says(children, false) {
        Says::Words => return None,
        Says::Unnamed => "this `button` shows only an image with no name, so VoiceOver reads it as just \"button\". Name it: `aria-label=\"Add\"` on the button, beside its `image \"symbol:…\"` (docs/recipes/symbols.md)",
        Says::Nothing => "this `button` holds no text and no `aria-label`, so it is blank: an empty button to a person, just \"button\" to VoiceOver. Put its words in it (a `text \"Add 500 ml\"` child), or name a symbol-only button with `aria-label` beside its `image \"symbol:…\"` (docs/recipes/symbols.md)",
    };
    Some(LowerError {
        id: "lower-button-unlabelled",
        message: message.into(),
        span,
    })
}

/// `lower-dialog-role`: `role="dialog"` or `role="alertdialog"` on an
/// element that is not a `dialog` (LLP 1116.004 D4). The role is a label:
/// the box is drawn as written, with no Escape, no focus kept inside and
/// no iOS sheet (flashcards' editor, a `column role="dialog"
/// aria-modal=true`). A popover (light dismiss and Escape; with
/// `alertdialog`, the native confirmation) and a route the router
/// presents (`navigationPresentation`) are dialogs already; a computed
/// role is the benefit of the doubt.
pub(super) fn dialog_role(tag: &str, attrs: &[Attr], span: Span) -> Option<LowerError> {
    if tag == "dialog"
        || computed(attrs, "role")
        || has(attrs, "popover")
        || has(attrs, "navigationPresentation")
    {
        return None;
    }
    let role = literal(attrs, "role").filter(|r| matches!(*r, "dialog" | "alertdialog"))?;
    Some(LowerError {
        id: "lower-dialog-role",
        message: format!(
            "`role=\"{role}\"` on a `{tag}` names a dialog without making one: the box is drawn as written, with no Escape, no focus kept inside it (Tab reaches the page behind) and no iOS sheet. A modal you lay out (a form, an editor) is a sheet, a route with `navigationPresentation=\"modal\"`: UIKit's sheet on iOS, closed by Escape on the web, the page under it inert. A confirmation of text and buttons is `dialog role=\"alertdialog\"`, opened with `showModal(id)` (docs/recipes/navigation-stack.md)"
        ),
        span,
    })
}

/// `lower-level-without-heading`: an `aria-level` on an element that
/// writes no `role` (LLP 1115.000 D12.5). A level alone is not a heading:
/// lowering gives it no heading style (`heading_style` reads the role), and
/// the iOS navigation bar does not take it for the route's title, so a
/// header written `text "Inbox" aria-level=1` is body text under an
/// untitled bar. `treeitem`, `row` and `listitem` take a level of their
/// own, so any written role is left alone; a computed one is the benefit of
/// the doubt.
pub(super) fn level_without_heading(tag: &str, attrs: &[Attr], span: Span) -> Option<LowerError> {
    if !has(attrs, "aria-level") || has(attrs, "role") {
        return None;
    }
    Some(LowerError {
        id: "lower-level-without-heading",
        message: format!(
            "`aria-level` on a `{tag}` with no `role=\"heading\"` does not make a heading: it gets no heading style, and the iOS navigation bar does not take it for the screen's title. Write the role beside it: `text \"Results\" role=\"heading\" aria-level=2`"
        ),
        span,
    })
}
