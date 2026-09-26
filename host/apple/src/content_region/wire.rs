//! Bounded immutable request and accepted frame wire; no live source at paint.
use super::*;
use crate::batch::quote;
use exact_kernel::{motion::motion_node, AxisOffer, FontStyle, NodeType, TextStyle};
use std::fmt::Write;
fn axis(value: AxisOffer) -> f32 {
    match value {
        AxisOffer::Definite(x) => x,
        AxisOffer::MinContent => -2.,
        AxisOffer::MaxContent => -1.,
    }
}
fn metric(style: TextStyle, out: &mut String) {
    let line = style.line_height.map_or("null".into(), |n| n.to_string());
    let _ = write!(out, "\"size\":{},\"weight\":{},\"family\":{},\"italic\":{},\"lineHeight\":{line},\"spacing\":{}", style.font_size, style.font_weight, style.font_family, style.font_style != FontStyle::Normal, style.letter_spacing);
}
fn colors(node: &exact_kernel::NodeRef<'_>, out: &mut String) {
    for (label, dark) in [("color", false), ("darkColor", true)] {
        let c = node.text_color().resolve(dark);
        let _ = write!(
            out,
            ",\"{label}\":[{},{},{},{}]",
            c.r(),
            c.g(),
            c.b(),
            c.a()
        );
    }
}
pub(super) fn request(kernel: &Kernel, p: &Pending, known: u64) -> Result<String, String> {
    let node = kernel
        .node_by_key(p.request.stamp().owner())
        .ok_or("removed paragraph")?;
    if node.paragraph_stamp().as_ref() != Some(p.request.stamp()) {
        return Err("stale source stamp".into());
    }
    let mut leaves = Vec::new();
    let mut stack = vec![node.id];
    while let Some(id) = stack.pop() {
        if leaves.len() + stack.len() >= exact_kernel::region::REGION_NODES {
            return Err("region run cap".into());
        }
        let n = kernel.node(id).ok_or("removed text run")?;
        if n.props.str(PropId::Text).is_some() {
            leaves.push(n);
        } else {
            stack.extend(n.children().into_iter().rev().filter(|id| {
                kernel
                    .node(*id)
                    .is_some_and(|n| n.node_type == NodeType::Text)
            }));
        }
    }
    let mut out = format!("{{\"request\":\"{}\",\"source\":\"{}\",\"ownerKey\":\"{}\",\"width\":{},\"height\":{},\"catalog\":\"{}\"", p.id, p.source, motion_node(node.key), axis(p.request.offer().width), axis(p.request.offer().height), p.request.catalog());
    p.request.with_request(|r| -> Result<(), String> {
        if r.runs.len() != leaves.len() {
            return Err("metric/paint run mismatch".into());
        }
        // Physical already (`Paragraph::from_style`); `start`/`end` never arrive.
        let align = match r.paragraph.text_align.physical(r.paragraph.direction) {
            exact_kernel::TextAlign::Left | exact_kernel::TextAlign::Start => 0,
            exact_kernel::TextAlign::Center => 1,
            exact_kernel::TextAlign::Right | exact_kernel::TextAlign::End => 2,
            exact_kernel::TextAlign::Justify => 3,
        };
        let wrap = match r.paragraph.overflow_wrap {
            exact_kernel::OverflowWrap::Normal => 0,
            exact_kernel::OverflowWrap::BreakWord => 1,
            exact_kernel::OverflowWrap::Anywhere => 2,
        };
        let _ = write!(
            out,
            ",\"align\":{align},\"lineClamp\":{},\"overflowWrap\":{wrap},\"strut\":{{",
            r.paragraph.line_clamp
        );
        metric(r.paragraph.strut, &mut out);
        out.push('}');
        if known != p.source {
            out.push_str(",\"runs\":[");
            for (i, (run, leaf)) in r.runs.iter().zip(&leaves).enumerate() {
                if i != 0 {
                    out.push(',');
                }
                out.push('{');
                metric(run.style, &mut out);
                out.push_str(",\"text\":");
                quote(run.text, &mut out);
                colors(leaf, &mut out);
                out.push_str(",\"decoration\":");
                quote(
                    match leaf.style.text_decoration_line {
                        exact_kernel::TextDecorationLine::None => "",
                        exact_kernel::TextDecorationLine::Underline => "underline",
                        exact_kernel::TextDecorationLine::LineThrough => "line-through",
                        exact_kernel::TextDecorationLine::UnderlineLineThrough => {
                            "underline line-through"
                        }
                    },
                    &mut out,
                );
                out.push_str(",\"href\":");
                quote(leaf.props.str(PropId::Href).unwrap_or(""), &mut out);
                let _ = write!(out, ",\"key\":\"{}\"}}", motion_node(leaf.key));
            }
            out.push(']');
        }
        Ok(())
    })?;
    out.push('}');
    Ok(out)
}
pub(super) fn state(region: &RegionState, kernel: &Kernel) -> String {
    if let Some(native) = &region.native {
        return native_state(region, kernel, native);
    }
    let id = |key| kernel.node_by_key(key).map_or(0, |n| n.id);
    let current = region.receipt.as_ref().is_some_and(|r| r.current);
    let mut out = format!("{{\"op\":\"region\",\"incarnation\":\"{}\",\"owner\":{},\"content\":{},\"pending\":{},\"ownerKey\":\"{}\",\"contentKey\":\"{}\",\"current\":{current},\"selection\":\"{}\",\"publication\":\"{}\",\"request\":\"{}\",\"source\":\"{}\"", region.incarnation, id(region.binding.owner), id(region.binding.content), id(region.binding.pending), motion_node(region.binding.owner), motion_node(region.binding.content), if region.publication.is_some() { "accepted" } else { "pending" }, region.publication.as_ref().map_or(0, |p| p.0), region.pending.as_ref().map_or(0, |p| p.id), region.pending.as_ref().map_or(0, |p| p.source));
    if let Some(receipt) = &region.receipt {
        let f = receipt.origin;
        let _ = write!(out, ",\"clip\":[{},{},{},{}]", f.x, f.y, f.width, f.height);
    }
    out.push_str(",\"refused\":");
    if let Some(why) = &region.refused {
        quote(why, &mut out);
    } else {
        out.push_str("null");
    }
    out.push_str(",\"members\":[");
    let mut stack = kernel
        .node_by_key(region.binding.content)
        .map(|n| vec![n.id])
        .unwrap_or_default();
    let mut first = true;
    while let Some(id) = stack.pop() {
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "{id}");
        if let Some(n) = kernel.node(id) {
            stack.extend(n.children());
        }
    }
    out.push(']');
    out.push_str(",\"frames\":");
    out.push_str(&region.frames_json);
    out.push('}');
    out
}
pub(super) fn frames(p: &RegionPublication, kernel: &Kernel) -> String {
    let mut out = String::from("[");
    for (i, row) in p.frames().iter().enumerate() {
        if i != 0 {
            out.push(',');
        }
        let f = row.frame;
        let artifact = p
            .paint_artifact(row.node)
            .and_then(|a| a.payload::<NativeArtifact>());
        let n = kernel.node_by_key(row.node);
        let kind = n.as_ref().map_or("removed", |n| match n.node_type {
            NodeType::ScrollView => "scroll",
            NodeType::Text => "text",
            _ => "view",
        });
        let id = n.as_ref().map_or(0, |n| n.id);
        let _ = write!(out, "{{\"key\":\"{}\",\"id\":{id},\"kind\":\"{kind}\",\"box\":[{},{},{},{}],\"extent\":[{},{}],\"artifact\":\"{}\"}}", motion_node(row.node), f.x, f.y, f.width, f.height, row.content.0, row.content.1, artifact.map_or(0, |a| a.id));
    }
    out.push(']');
    out
}

pub(super) fn native_collections(
    selected: &[exact_runner::CollectionSnapshot],
    outside: &[exact_runner::CollectionSnapshot],
    limits: NativeProjectionLimits,
) -> Result<String, String> {
    let count = selected
        .len()
        .checked_add(outside.len())
        .ok_or("native collection overflow")?;
    let rows = selected
        .iter()
        .chain(outside)
        .try_fold(0usize, |n, c| n.checked_add(c.rows.len()))
        .ok_or("native collection rows overflow")?;
    if count > limits.collections * 2 || rows > limits.collection_rows * 2 {
        return Err("selected/live collection union capacity".into());
    }
    let bound = count
        .checked_mul(1024)
        .and_then(|n| rows.checked_mul(1024).and_then(|r| n.checked_add(r)))
        .and_then(|n| n.checked_add(2))
        .ok_or("native collection wire overflow")?;
    if bound > limits.diff_wire_bound {
        return Err("native collection wire capacity".into());
    }
    let mut out = String::new();
    out.try_reserve_exact(bound)
        .map_err(|_| "native collection wire allocation")?;
    out.push('[');
    // Preserve ordinary view-ID order, without cloning A's rows into a third
    // snapshot. Both checked source arrays are independently sorted.
    let (mut a, mut b) = (0usize, 0usize);
    while a < selected.len() || b < outside.len() {
        if a + b > 0 {
            out.push(',');
        }
        let value =
            if a < selected.len() && (b == outside.len() || selected[a].view < outside[b].view) {
                let c = &selected[a];
                a += 1;
                c
            } else {
                if a < selected.len() && selected[a].view == outside[b].view {
                    return Err("native duplicate collection identity".into());
                }
                let c = &outside[b];
                b += 1;
                c
            };
        value.write_json(&mut out);
    }
    out.push(']');
    if out.len() > bound {
        return Err("native collection wire bound violated".into());
    }
    Ok(out)
}

fn native_state(region: &RegionState, kernel: &Kernel, native: &NativeProjection) -> String {
    let selected = native.selected.as_ref();
    let current = region.selected_native_current();
    let mut out = format!("{{\"op\":\"native-region\",\"incarnation\":\"{}\",\"current\":{current},\"publication\":\"{}\",\"request\":\"{}\",\"source\":\"{}\",\"revision\":\"{}\",\"selectedRevision\":\"{}\",\"origin\":", region.incarnation,
        selected.map_or(0, |a| a.serial), region.pending.as_ref().map_or(0, |p| p.id),
        region.pending.as_ref().map_or(0, |p| p.source), native.revision,
        selected.map_or(0, |a| a.identity.inputs.consumer_revision));
    if let Some(a) = selected {
        let f = a.origin;
        let _ = write!(out, "[{},{},{},{}]", f.x, f.y, f.width, f.height);
    } else {
        out.push_str("null");
    }
    out.push_str(",\"members\":[");
    if let Some(a) = selected {
        for (i, h) in a.headers.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"id\":{},\"key\":\"{}\"}}",
                h.id,
                motion_node(h.key)
            );
        }
    }
    out.push_str("],\"content\":");
    let id = selected
        .and_then(|a| a.headers.iter().find(|h| h.key == a.identity.content))
        .map(|h| h.id)
        .or_else(|| kernel.node_by_key(region.binding.content).map(|n| n.id))
        .unwrap_or(0);
    let _ = write!(out, "{id},\"refused\":");
    if let Some(refused) = &region.refused {
        quote(refused, &mut out);
    } else {
        out.push_str("null");
    }
    out.push('}');
    out
}
