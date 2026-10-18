//! `lower-round-button-label` (LLP 1116.011 D2.1): a label too long for
//! `exact:content`'s `RoundButton`. The part is the Clock's circle, an
//! 80-point button with 6 points of padding either side: 68 points, about
//! 7 characters of 17-point type (Start, Stop, Lap, Reset, Resume, Restart;
//! the one-screen pattern's "500 ml" is 6). trivia's
//! `RoundButton(label="Start quiz")` drew "Start q…" on a phone, the one
//! control a person needed. The width decides, not the number of words: a
//! short label of two words fits, and a long single word ("Continue", 8) does
//! not.
//!
//! The call is read where it is written, in the file's own components, not
//! in the expanded tree, where the part is a bare `button`. Only a literal
//! label is judged; a label the app computes is the benefit of the doubt. The
//! warning advises and the author decides: the part keeps its shape, and a
//! screen's main action of more words is the prominent `button`.
//!
//! `lower-empty-state-in-list` (LLP 1116.020 D5): an `EmptyState` inside a
//! `list`, which draws it as a row, a card among the sections, where UIKit's
//! content-unavailable view takes the screen's place. Five of round 18's
//! apps showed their empty state so. Read the same way: a part wrapped in a
//! component of the author's own is the benefit of the doubt.

use super::miswiring::arms;
use crate::LowerError;
use contract_syntax::{Expr, Node};
use contract_types::Checked;

/// The most characters the circle holds across its 68 points.
const HOLDS: usize = 7;

/// The part's props, in the order it declares them: a component of this
/// name that takes others is the author's own, and is not judged.
const PROPS: [&str; 5] = ["label", "tint", "press", "disabled", "testId"];

/// Every `RoundButton` call whose literal label has more characters than the
/// circle holds.
pub(super) fn round_button_label(checked: &Checked<'_>) -> Vec<LowerError> {
    let components = &checked.file.components;
    let is_part = components
        .iter()
        .any(|c| c.name == "RoundButton" && c.props.iter().map(|p| p.name.as_str()).eq(PROPS));
    let mut out = Vec::new();
    if !is_part {
        return out;
    }
    for component in components {
        let mut stack: Vec<&Node> = component.view.iter().rev().collect();
        while let Some(node) = stack.pop() {
            if let Node::Use { name, args, .. } = node {
                let label = args.iter().rev().find(|a| a.name == "label");
                if let (true, Some(Expr::Str(label, span))) =
                    (name == "RoundButton", label.map(|a| &a.value))
                {
                    let chars = label.trim().chars().count();
                    if chars > HOLDS {
                        out.push(LowerError {
                            id: "lower-round-button-label",
                            message: format!(
                                "`RoundButton` is the Clock's circle for a word (Start, Stop, Lap): \"{}\" is {chars} characters and the 80-point circle holds about {HOLDS}, so it is cut short. A screen's main action of more words is a `button` with `buttonStyle=\"filled\"` (docs/recipes/content-design.md)",
                                label.trim()
                            ),
                            span: *span,
                        });
                    }
                }
            }
            for arm in arms(node).into_iter().rev() {
                stack.extend(arm.iter().rev());
            }
        }
    }
    out
}

/// The `EmptyState` part's props, in its order.
const EMPTY_PROPS: [&str; 3] = ["symbol", "title", "message"];

/// Every `EmptyState` call written inside a `list` of the same component.
pub(super) fn empty_state_in_list(checked: &Checked<'_>) -> Vec<LowerError> {
    let components = &checked.file.components;
    let is_part = components
        .iter()
        .any(|c| c.name == "EmptyState" && c.props.iter().map(|p| p.name.as_str()).eq(EMPTY_PROPS));
    let mut out = Vec::new();
    if !is_part {
        return out;
    }
    for component in components {
        let mut stack: Vec<(&Node, bool)> =
            component.view.iter().rev().map(|n| (n, false)).collect();
        while let Some((node, in_list)) = stack.pop() {
            if let (Node::Use { name, span, .. }, true) = (node, in_list) {
                if name == "EmptyState" {
                    out.push(LowerError {
                        id: "lower-empty-state-in-list",
                        message: "`EmptyState` inside a `list` is drawn as one of its rows; it centres in the screen in place of the list: `if items.length == 0 and not pending(items)`, the `EmptyState`, `else` the `list` (pattern list-detail)".into(),
                        span: *span,
                    });
                }
            }
            let in_list = in_list || matches!(node, Node::Element { tag, .. } if tag == "list");
            for arm in arms(node).into_iter().rev() {
                stack.extend(arm.iter().rev().map(|n| (n, in_list)));
            }
        }
    }
    out
}

/// The `SummaryCard` part's props, in its order.
const SUMMARY_PROPS: [&str; 5] = ["label", "value", "detail", "progress", "symbol"];

/// @ref LLP 1116.021 D7 — `lower-summary-card-in-list`: a `SummaryCard`
/// written in a grouped list's section that has its card, which draws it as
/// a grey card inside a white cell, its edge on the cell's (round 19's
/// expense-tracker, habit-tracker). A card-less first section
/// (`background-color="transparent"`) is the place the one-screen pattern
/// gives it. Read as `empty_state_in_list` reads its part.
pub(super) fn summary_card_in_list(checked: &Checked<'_>) -> Vec<LowerError> {
    let components = &checked.file.components;
    let is_part = components.iter().any(|c| {
        c.name == "SummaryCard" && c.props.iter().map(|p| p.name.as_str()).eq(SUMMARY_PROPS)
    });
    let mut out = Vec::new();
    if !is_part {
        return out;
    }
    #[derive(Clone, Copy, PartialEq)]
    enum In {
        Page,
        Grouped,
        Card,
    }
    for component in components {
        let mut stack: Vec<(&Node, In)> =
            component.view.iter().rev().map(|n| (n, In::Page)).collect();
        while let Some((node, at)) = stack.pop() {
            if let (Node::Use { name, span, .. }, In::Card) = (node, at) {
                if name == "SummaryCard" {
                    out.push(LowerError {
                        id: "lower-summary-card-in-list",
                        message: "`SummaryCard` in a grouped list's section is a grey card inside its white cell: put it above the `list`, or in a first `section background-color=\"transparent\"`, which has no card (pattern one-screen)".into(),
                        span: *span,
                    });
                }
            }
            let at = match node {
                Node::Element { tag, attrs, .. } if tag == "list" => {
                    let grouped = attrs
                        .iter()
                        .rev()
                        .find(|a| a.name == "appearance")
                        .is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "auto"));
                    if grouped {
                        In::Grouped
                    } else {
                        In::Page
                    }
                }
                Node::Element { tag, attrs, .. } if tag == "section" && at == In::Grouped => {
                    let cardless = attrs
                        .iter()
                        .rev()
                        .find(|a| a.name == "background-color")
                        .is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "transparent"));
                    if cardless {
                        In::Page
                    } else {
                        In::Card
                    }
                }
                _ => at,
            };
            for arm in arms(node).into_iter().rev() {
                stack.extend(arm.iter().rev().map(|n| (n, at)));
            }
        }
    }
    out
}
