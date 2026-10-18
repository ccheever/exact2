//! A node's state looks in `layout <node>` (LLP 1121 D8; stage 9 E): each
//! look's rows as a style row is, its value and the token it read.

use super::{quote, row, token_fields};
use crate::{DataSource, Runner};
use exact_kernel::{NodeRef, StyleId};

/// A node's looks (LLP 1121 D8) as `"looks":{"hover":{row:{value,token?}},
/// "focus_visible":{…}}`, the states it has; nothing when it has none.
pub(super) fn write<D: DataSource>(runner: &Runner<D>, node: &NodeRef<'_>, s: &mut String) {
    let looks = node.looks();
    if looks.is_empty() {
        return;
    }
    s.push_str(",\"looks\":{");
    for (i, (state, props)) in looks.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(
            if *state == exact_kernel::LOOK_HOVER {
                "hover"
            } else {
                "focus_visible"
            },
            s,
        );
        s.push_str(":{");
        let mut first = true;
        for row in StyleId::ALL.into_iter().filter(|r| props.mask.has(*r)) {
            if !first {
                s.push(',');
            }
            first = false;
            quote(row.name(), s);
            s.push_str(":{\"value\":");
            row::row_json(row, props.get(row), s);
            if let Some((names, text, modes, implied)) = runner.look_token(node.id, *state, row) {
                token_fields(&names, &text, &modes, implied, s);
            }
            s.push('}');
        }
        s.push('}');
    }
    s.push('}');
}
