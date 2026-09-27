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
//! compound shape's holes still fill by its `fill-rule`. A zero-length
//! subpath is a dot (`data-dot`), shown while the window holds its place.
//!
//! An author's `stroke-dasharray` needs the pieces' own dashes, so a dashed
//! stroke is the whole data dashed as SVG dashes it, masked by the pieces:
//! the trim reveals the dashes, it does not move them. Under
//! `vector-effect: non-scaling-stroke` the browser dashes in the box's
//! pixels, where `pathLength` no longer rescales, so the pieces drop it and
//! scale their numbers by `--k`, the view box's pixels per unit, read from
//! the node's size through container units.
//!
//! The markup is numbers the kernel produced and nothing an author wrote,
//! so the page may set it as `innerHTML` (`glue.js`, `pathMarkup`). It
//! depends on the node's own `vector-effect` and computed dashes, which the
//! kernel re-sends when they change, as it does every row.

use exact_kernel::vector::{commands_css, Command, PathData};
use exact_kernel::{NodeRef, PropId, RowValue, SortedMap, StyleId, VectorEffect};
use exact_num::Shortest32;
use std::fmt::Write as _;

/// The two registered properties, the dash every stroke piece takes, and a
/// dot's visibility. A zero-length dash would still paint a round or square
/// cap, so an empty piece fades to nothing instead; a full one runs a unit
/// past its end, so a browser that measures a hair longer leaves no gap.
/// Inside `<svg>` a `<style>` is not raw text to the HTML parser, so `<` is
/// a CSS escape here; and a selector's attribute name matches an SVG
/// element's case-sensitively, so the pieces are found by `data-stroke`, not
/// `pathLength`.
const SHEET: &str =
    "@property --exact-stroke-start{syntax:'\\3c number>';inherits:true;initial-value:0}\
@property --exact-stroke-end{syntax:'\\3c number>';inherits:true;initial-value:1}\
svg[data-exact-path],svg[data-exact-path] *{vector-effect:inherit}\
svg[data-exact-path] path[data-stroke]{\
--s:clamp(0,var(--exact-stroke-start)*var(--L) - var(--a),var(--l));\
--e:clamp(0,var(--exact-stroke-end)*var(--L) - var(--a),var(--l));\
stroke-dasharray:calc(max(0,var(--e) - var(--s) + clamp(0,(var(--e) - var(--l))*1e6 + 1,1))*var(--k)) calc((var(--l) + 2)*var(--k));\
stroke-dashoffset:calc(0 - var(--s)*var(--k));\
stroke-opacity:clamp(0,(var(--e) - var(--s))*1e6,1)}\
svg[data-exact-path] path:is([data-dot],[data-mark]){--v:calc(\
clamp(0,(var(--exact-stroke-end)*var(--L) - var(--a))*1e6 + 1,1)*\
clamp(0,(var(--a) - var(--exact-stroke-start)*var(--L))*1e6 + 1,1)*\
clamp(0,(var(--exact-stroke-end) - var(--exact-stroke-start))*1e12,1))}\
svg[data-exact-path] path[data-dot]{stroke-dasharray:none;stroke-opacity:var(--v)}\
svg[data-exact-path] path[data-mark]{opacity:var(--v)}";

/// The node's attributes: the markup in place of `d`, `viewBox` and
/// `preserveAspectRatio`, and a path is decorative (hidden from assistive
/// technology) unless it has an `aria-label`, when it is an image.
pub(super) fn props(node: &NodeRef<'_>, out: &mut SortedMap<String, String>) {
    out.insert("pathMarkup".into(), markup(node));
    if node.props.str(PropId::AccessibilityLabel).is_some() {
        out.get_or_insert_with("role".into(), || "img".into());
    } else {
        out.get_or_insert_with("aria-hidden".into(), || "true".into());
    }
}

/// Whether the node strokes in its box's pixels, so its `div` must be a size
/// container for `--k` (`element.rs`).
pub(super) fn non_scaling(node: &NodeRef<'_>) -> bool {
    node.style.vector_effect == VectorEffect::NonScalingStroke
}

fn markup(node: &NodeRef<'_>) -> String {
    let data = PathData::parse(node.props.str(PropId::PathData).unwrap_or(""));
    // A path in a path draws in the outermost one's coordinate system.
    let (view_box, aspect) = node.path_viewport();
    let dashed =
        matches!(node.computed(StyleId::StrokeDasharray), RowValue::DashArray(d) if d.dashes());
    let fixed = non_scaling(node);
    let mut out = String::from("<svg data-exact-path");
    if let Some([x, y, w, h]) = view_box {
        let _ = write!(
            out,
            " viewBox=\"{} {} {} {}\" preserveAspectRatio=\"{}\"",
            Shortest32(x),
            Shortest32(y),
            Shortest32(w),
            Shortest32(h),
            aspect.css()
        );
    }
    // `--k`: the box's pixels per view-box unit, for a stroke that dashes
    // in pixels. `none` stretches each axis apart; the smaller is taken.
    let k = match (fixed, view_box) {
        (true, Some([_, _, w, h])) => format!(
            "calc({}(100cqw / {}, 100cqh / {}) / 1px)",
            if aspect.slice && aspect.align.is_some() {
                "max"
            } else {
                "min"
            },
            Shortest32(w),
            Shortest32(h)
        ),
        _ => "1".into(),
    };
    let _ = write!(
        out,
        " aria-hidden=\"true\" style=\"position:absolute;inset:0;width:100%;height:100%;box-sizing:border-box;padding:inherit;overflow:hidden;pointer-events:none;--L:{};--k:{k}\"><style>{SHEET}</style><path d=\"{}\" stroke=\"none\"/>",
        Shortest32(data.length() as f32),
        data.css()
    );
    let id = node.id;
    if dashed {
        // The mask's region is the viewport many times over: a thin path's
        // bounding box would cut a wide stroke off.
        let _ = write!(
            out,
            "<mask id=\"exact-path-{id}\" maskUnits=\"userSpaceOnUse\" x=\"-1000%\" y=\"-1000%\" width=\"2100%\" height=\"2100%\">"
        );
    }
    let paint = if dashed { " stroke=\"#fff\"" } else { "" };
    let mut at = 0.0f64;
    for (commands, length) in data.subpaths().zip(data.lengths()) {
        let (a, l) = (Shortest32(at as f32), Shortest32(length as f32));
        match commands {
            [Command::Move(_)] => {}
            [Command::Move(p), ..] if length == 0.0 => {
                let dot = commands_css(&[Command::Move(*p), Command::Line(*p)]);
                let _ = write!(
                    out,
                    "<path data-dot d=\"{dot}\" fill=\"none\"{paint} style=\"--a:{a}\"/>"
                );
            }
            _ => {
                let _ = write!(out, "<path data-stroke d=\"{}\"", commands_css(commands));
                if !fixed {
                    let _ = write!(out, " pathLength=\"{l}\"");
                }
                let _ = write!(out, " fill=\"none\"{paint} style=\"--a:{a};--l:{l}\"/>");
            }
        }
        at += length;
    }
    if dashed {
        let _ = write!(
            out,
            "</mask><path d=\"{}\" fill=\"none\" mask=\"url(#exact-path-{id})\"/>",
            data.css()
        );
    }
    markers(node, &data, &mut out);
    out.push_str("</svg>");
    out
}

/// SVG's own markers (LLP 1065 D11), so `context-fill`/`context-stroke`
/// paint the path's moving `fill` and `stroke` as the browser has them. The
/// kernel says where they go: each vertex is a carrier — a stroke too thin to
/// paint, from the vertex along the direction `orient="auto"` takes there —
/// whose `marker-start` is the marker, `markerUnits="userSpaceOnUse"` at the
/// stroke's width worked in, and which shows while the trim holds its
/// place, as a dot does.
fn markers(node: &NodeRef<'_>, data: &PathData, out: &mut String) {
    use exact_kernel::vector::{Orient, Slot};
    let rows = [
        (Slot::Start, StyleId::MarkerStart),
        (Slot::Mid, StyleId::MarkerMid),
        (Slot::End, StyleId::MarkerEnd),
    ];
    let width = match node.computed(StyleId::StrokeWidth) {
        RowValue::Number(n) => n as f32,
        _ => 1.0,
    };
    let n = |v: f32| Shortest32(v).to_string();
    let id = node.id;
    let mut any = false;
    for (slot, row) in rows {
        let RowValue::Marker(marker) = node.computed(row) else {
            continue;
        };
        let Some(def) = marker.def() else { continue };
        any = true;
        let k = if def.stroke_units { width } else { 1.0 };
        let [w, h] = def.size;
        let view = def.view_box.unwrap_or([0.0, 0.0, w, h]).map(n).join(" ");
        let orient = match (def.orient, slot) {
            (Orient::AutoStartReverse, Slot::Start) => "auto-start-reverse".into(),
            (Orient::Auto | Orient::AutoStartReverse, _) => "auto".into(),
            (Orient::Angle(a), _) => n(a),
        };
        let aspect = match def.view_box {
            Some(_) => def.aspect.css(),
            None => "none".into(),
        };
        let _ = write!(
            out,
            "<marker id=\"exact-mark-{id}-{slot:?}\" markerUnits=\"userSpaceOnUse\" viewBox=\"{view}\" preserveAspectRatio=\"{aspect}\" refX=\"{}\" refY=\"{}\" markerWidth=\"{}\" markerHeight=\"{}\" orient=\"{orient}\" overflow=\"hidden\">",
            n(def.reference[0]),
            n(def.reference[1]),
            n(w * k),
            n(h * k),
        );
        for shape in &def.shapes {
            let _ = write!(
                out,
                "<path d=\"{}\" style=\"vector-effect:none;fill:{};stroke:{};stroke-width:{};stroke-linecap:{};stroke-linejoin:{};stroke-miterlimit:{};fill-rule:{};stroke-dasharray:none\"/>",
                shape.data.css(),
                shape.fill.css(),
                shape.stroke.css(),
                n(shape.width),
                shape.cap.name(),
                shape.join.name(),
                n(shape.miter),
                shape.rule.name()
            );
        }
        out.push_str("</marker>");
    }
    if !any {
        return;
    }
    // A carrier's length: small against the path, never lost to rounding.
    let reach = data
        .commands()
        .iter()
        .flat_map(|c| match *c {
            Command::Move(p) | Command::Line(p) => vec![p],
            Command::Cubic(a, b, p) => vec![a, b, p],
            Command::Close => Vec::new(),
        })
        .flatten()
        .fold(1.0f32, |m, v| m.max(v.abs()))
        * 1e-4;
    for vertex in data.vertices() {
        let row = rows
            .iter()
            .find(|(s, _)| *s == vertex.slot)
            .map(|(_, r)| *r);
        let has = row
            .is_some_and(|r| matches!(node.computed(r), RowValue::Marker(m) if m.def().is_some()));
        if !has {
            continue;
        }
        let (sin, cos) = vertex.angle.to_radians().sin_cos();
        let [x, y] = vertex.point;
        let _ = write!(
            out,
            "<path data-mark d=\"M {} {} L {} {}\" stroke-width=\"0\" marker-start=\"url(#exact-mark-{id}-{:?})\" style=\"--a:{}\"/>",
            n(x),
            n(y),
            n(x + reach * cos as f32),
            n(y + reach * sin as f32),
            vertex.slot,
            n(vertex.at as f32)
        );
    }
}
