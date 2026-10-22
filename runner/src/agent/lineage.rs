//! A view's logical ancestors with their plan sites, for `{"op":"ancestors"}` (tamber/1.16 `inspect` `select`).

use super::error;
use crate::{DataSource, Runner};
use std::fmt::Write;

/// `{"ancestors":[[view, site], …]}`: `id` and each logical ancestor to the root, innermost first, with its plan
/// site or `null` (tamber/1.16 `inspect` `select`); a stale `id` is an error. Bounded by the kernel's depth.
pub(super) fn ancestors<D: DataSource>(runner: &Runner<D>, id: u32) -> String {
    let kernel = runner.kernel();
    if kernel.node(id).is_none() {
        return error(&format!(
            "stale node #{id} (incarnation {})",
            kernel.incarnation()
        ));
    }
    let mut s = String::from("{\"ancestors\":[");
    let mut at = Some(id);
    let mut first = true;
    while let Some(view) = at {
        let Some(node) = kernel.node(view) else { break };
        if !first {
            s.push(',');
        }
        first = false;
        match runner.site_of(view) {
            Some((site, _)) => {
                let _ = write!(s, "[{view},{}]", site.0);
            }
            None => {
                let _ = write!(s, "[{view},null]");
            }
        }
        at = node.parent;
    }
    s.push_str("]}");
    s
}
