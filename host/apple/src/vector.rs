//! A `path` node's props for the presenter (LLP 1065 D6): its path data
//! normalized by the kernel (absolute `M`, `L`, `C`, `Z`, which Core
//! Graphics draws as is), the view box and `preserveAspectRatio` of the
//! coordinate system it draws in — the outermost path's, for a path in a
//! path (D12) — and its markers placed (D11), so Swift parses no SVG.

use exact_kernel::style::ColorValue;
use exact_kernel::vector::{PathData, ShapePaint};
use exact_kernel::{NodeRef, PropId};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Replace `pathData` and `viewBox` with what the presenter draws.
pub(crate) fn props(node: &NodeRef<'_>, out: &mut BTreeMap<String, String>) {
    let n = |v: f32| crate::style::num(v);
    let data = PathData::parse(node.props.str(PropId::PathData).unwrap_or(""));
    out.insert(PropId::PathData.name().into(), data.css());
    let (view_box, aspect) = node.path_viewport();
    match view_box {
        Some(v) => {
            out.insert(PropId::ViewBox.name().into(), v.map(n).join(" "));
        }
        None => {
            out.remove(PropId::ViewBox.name());
        }
    }
    out.insert(PropId::PreserveAspectRatio.name().into(), aspect.css());
    match markers(node, &data) {
        Some(text) => out.insert("markers".into(), text),
        None => out.remove("markers"),
    };
}

/// A marker shape's paint for the presenter: a keyword, or channels 0–255
/// (`r,g,b,a`, a `light-dark()` pair as two joined by `/`).
fn paint(p: ShapePaint) -> String {
    let channels = |c: exact_kernel::Color| format!("{},{},{},{}", c.r(), c.g(), c.b(), c.a());
    match p {
        ShapePaint::Color(ColorValue::Fixed(c)) => channels(c),
        ShapePaint::Color(ColorValue::LightDark(l, d)) => {
            format!("{}/{}", channels(l), channels(d))
        }
        keyword => keyword.css(),
    }
}

/// The path's markers as lines: `L total`, then per marker used `D index`
/// and its shapes (`S d|fill|stroke|width|cap|join|miterlimit|rule`), then
/// each instance `I index at a b c d e f x y w h sx sy dx dy` — its
/// viewport's transform into path units, the viewport, and the content's
/// fit into it (`exact_kernel::vector::Placed`).
fn markers(node: &NodeRef<'_>, data: &PathData) -> Option<String> {
    let placed = node.path_markers(data);
    if placed.is_empty() {
        return None;
    }
    let n = |v: f32| crate::style::num(v);
    let mut defs: Vec<&exact_kernel::vector::MarkerDef> = Vec::new();
    let mut out = format!("L {}", n(data.length() as f32));
    for (def, p) in &placed {
        let index = match defs.iter().position(|d| *d == *def) {
            Some(i) => i,
            None => {
                defs.push(*def);
                let _ = write!(out, "\nD {}", defs.len() - 1);
                for s in &def.shapes {
                    let _ = write!(
                        out,
                        "\nS {}|{}|{}|{}|{}|{}|{}|{}",
                        s.data.css(),
                        paint(s.fill),
                        paint(s.stroke),
                        n(s.width),
                        s.cap.name(),
                        s.join.name(),
                        n(s.miter),
                        s.rule.name()
                    );
                }
                defs.len() - 1
            }
        };
        let numbers: Vec<String> = p
            .outer
            .iter()
            .chain(&p.clip)
            .chain(&p.fit)
            .map(|v| n(*v))
            .collect();
        let _ = write!(out, "\nI {index} {} {}", n(p.at as f32), numbers.join(" "));
    }
    Some(out)
}
