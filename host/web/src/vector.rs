//! A `path` node on the web (LLP 1065 D5): an inline `<svg>` in the node's
//! element, built here from the kernel's normalized path data.
//!
//! SVG dashing restarts at every subpath, so one dash pattern cannot trim a
//! stroke by length along the whole path. The stroke is therefore one
//! `<path>` per subpath, each with `pathLength` set to the kernel's measure
//! of it and a dash computed in CSS from the node's `--exact-stroke-start`
//! and `--exact-stroke-end` — registered numbers, so the browser transitions
//! and keyframes them like any property — and the subpath's place in the
//! whole (`--a`, where it starts; `--l`, its length; `--L`, the total). The
//! pen order is exact whatever the browser's own length measure says. The
//! fill is one more `<path>` of the whole data under the strokes, so a
//! compound shape's holes still fill by the nonzero rule.
//!
//! The markup is numbers the kernel produced and nothing an author wrote,
//! so the page may set it as `innerHTML` (`glue.js`, `pathMarkup`).

use exact_kernel::vector::{commands_css, parse_view_box, PathData};
use exact_kernel::{NodeRef, PropId, SortedMap};
use exact_num::Shortest32;
use std::fmt::Write as _;

/// The two registered properties and the dash every stroke piece takes. A
/// zero-length dash would still paint a round or square cap, so an empty
/// piece fades to nothing instead. Inside `<svg>` a `<style>` is not raw
/// text to the HTML parser, so `<` is a CSS escape here; and a selector's
/// attribute name matches an SVG element's case-sensitively, so the pieces
/// are found by `data-stroke`, not `pathLength`.
const SHEET: &str =
    "@property --exact-stroke-start{syntax:'\\3c number>';inherits:true;initial-value:0}\
@property --exact-stroke-end{syntax:'\\3c number>';inherits:true;initial-value:1}\
svg[data-exact-path] path[data-stroke]{\
--s:clamp(0,var(--exact-stroke-start)*var(--L) - var(--a),var(--l));\
--e:clamp(0,var(--exact-stroke-end)*var(--L) - var(--a),var(--l));\
stroke-dasharray:max(0,var(--e) - var(--s)) calc(var(--l) + 1);\
stroke-dashoffset:calc(0 - var(--s));\
stroke-opacity:clamp(0,(var(--e) - var(--s))*1e6,1)}";

/// The node's attributes: the markup in place of `d` and `viewBox`, and a
/// path is decorative (hidden from assistive technology) unless it has an
/// `aria-label`, when it is an image.
pub(super) fn props(node: &NodeRef<'_>, out: &mut SortedMap<String, String>) {
    out.insert("pathMarkup".into(), markup(node));
    if node.props.str(PropId::AccessibilityLabel).is_some() {
        out.get_or_insert_with("role".into(), || "img".into());
    } else {
        out.get_or_insert_with("aria-hidden".into(), || "true".into());
    }
}

fn markup(node: &NodeRef<'_>) -> String {
    let data = PathData::parse(node.props.str(PropId::PathData).unwrap_or(""));
    let view_box = node.props.str(PropId::ViewBox).and_then(parse_view_box);
    let mut out = String::from("<svg data-exact-path");
    if let Some([x, y, w, h]) = view_box {
        let _ = write!(
            out,
            " viewBox=\"{} {} {} {}\"",
            Shortest32(x),
            Shortest32(y),
            Shortest32(w),
            Shortest32(h)
        );
    }
    let _ = write!(
        out,
        " aria-hidden=\"true\" style=\"position:absolute;inset:0;width:100%;height:100%;box-sizing:border-box;padding:inherit;overflow:hidden;pointer-events:none;--L:{}\"><style>{SHEET}</style><path d=\"{}\" stroke=\"none\"/>",
        Shortest32(data.length() as f32),
        data.css()
    );
    let mut at = 0.0f64;
    for (commands, length) in data.subpaths().zip(data.lengths()) {
        if length > 0.0 {
            let _ = write!(
                out,
                "<path data-stroke d=\"{}\" pathLength=\"{}\" fill=\"none\" style=\"--a:{};--l:{}\"/>",
                commands_css(commands),
                Shortest32(length as f32),
                Shortest32(at as f32),
                Shortest32(length as f32)
            );
        }
        at += length;
    }
    out.push_str("</svg>");
    out
}
