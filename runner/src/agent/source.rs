//! Where a row's value came from, for `node` (LLP 1035.002 D1).

use super::token_json;
use crate::{DataSource, Runner};
use exact_kernel::{NodeRef, StyleId};
use std::fmt::Write;

/// Row `row`'s source on `node`, its own (`own`) row bound by an
/// expression (`dynamic`) or not, or another's: `dynamic`, `heading` (a
/// heading's implied size or weight, LLP 1115.000 D12.2), `authored`, `style`
/// with the node whose platform text style gives it (a weight or a leading,
/// D12.2), `inherited` with the ancestor whose own row won (its view, its plan
/// site where it has one, and the token it read), or `initial`; the row's
/// object closed.
pub(super) fn write<D: DataSource>(
    runner: &Runner<D>,
    node: &NodeRef,
    row: StyleId,
    own: bool,
    dynamic: bool,
    s: &mut String,
) {
    if dynamic {
        s.push_str(",\"source\":\"dynamic\"}");
    } else if own && node.heading_implies(row) {
        s.push_str(",\"source\":\"heading\"}");
    } else if own {
        s.push_str(",\"source\":\"authored\"}");
    } else if let Some(from) = node.style_source_of(row) {
        let _ = write!(s, ",\"source\":\"style\",\"from\":{from}}}");
    } else {
        match node.source_of(row) {
            Some(from) => {
                // And the token the ancestor's own row read, its link; and that ancestor's plan site, which the
                // design server names by its source (tamber/1.16 `fromAt`).
                let _ = write!(s, ",\"source\":\"inherited\",\"from\":{from}");
                if let Some((site, _)) = runner.site_of(from) {
                    let _ = write!(s, ",\"fromSite\":{}", site.0);
                }
                token_json(runner, from, row, s);
                s.push('}');
            }
            None => s.push_str(",\"source\":\"initial\"}"),
        }
    }
}
