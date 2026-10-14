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
