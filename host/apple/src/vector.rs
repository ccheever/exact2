//! A `path` node's props for the presenter (LLP 1065 D6): its path data
//! normalized by the kernel (absolute `M`, `L`, `C`, `Z`, which Core
//! Graphics draws as is) and its view box validated, so Swift parses no SVG.

use exact_kernel::vector::{parse_view_box, PathData};
use exact_kernel::{NodeRef, PropId};
use std::collections::BTreeMap;

/// Replace `pathData` and `viewBox` with what the presenter draws.
pub(crate) fn props(node: &NodeRef<'_>, out: &mut BTreeMap<String, String>) {
    let data = PathData::parse(node.props.str(PropId::PathData).unwrap_or(""));
    out.insert(PropId::PathData.name().into(), data.css());
    match node.props.str(PropId::ViewBox).and_then(parse_view_box) {
        Some([x, y, w, h]) => {
            let n = |v: f32| crate::style::num(v);
            out.insert(
                PropId::ViewBox.name().into(),
                format!("{} {} {} {}", n(x), n(y), n(w), n(h)),
            );
        }
        None => {
            out.remove(PropId::ViewBox.name());
        }
    }
}
