//! `lower-single-tab` (LLP 1116 D6).

use super::{computed, has, kids, literal};
use crate::LowerError;
use contract_syntax::Node;

/// `lower-single-tab`: a root tab bar (a `role="tablist"` whose tabs name
/// their panels with `aria-controls`) with fewer than two tabs (LLP 1116
/// D6): a bar of one tab under a single screen, which builders copied from
/// a tab skeleton (polls, chat-rooms).
pub(super) fn single_tab(tablist: &Node, out: &mut Vec<LowerError>) {
    let (mut tabs, mut controls, mut doubt) = (0, false, false);
    let mut stack: Vec<&Node> = kids(tablist).iter().collect();
    while let Some(node) = stack.pop() {
        match node {
            Node::Element {
                attrs, children, ..
            } => {
                if computed(attrs, "role") {
                    doubt = true;
                } else if literal(attrs, "role") == Some("tab") {
                    tabs += 1;
                    controls |= has(attrs, "aria-controls");
                } else {
                    stack.extend(children);
                }
            }
            _ => doubt = true,
        }
    }
    if controls && !doubt && tabs < 2 {
        out.push(LowerError {
            id: "lower-single-tab",
            message: format!(
                "this tab bar has {} tab{}: a tab bar is for moving between two or more sections, and one tab is a bar that does nothing. For a single screen, drop the `role=\"tablist\"` and the `tabpanel` around the routes, so the routes are the navigation root's children (one stack); add the other tabs if the app has them (docs/contract-for-agents.md, \"Tabs and stacks\")",
                tabs,
                if tabs == 1 { "" } else { "s" }
            ),
            span: tablist.span(),
        });
    }
}
