//! Read-only text projection for virtual rows. Uses the same realization and
//! binding evaluation as rendering, but commits no kernel or host operations.
use super::*;
use crate::ListTextPosition;
use exact_kernel::{PropId, StyleId};

/// A virtualized list's rows as logical text sees them: every row, mounted
/// or not. Given the plan, also whether the list's text may be selected:
/// the `user-select` the list has or takes from above (`may_select`).
fn list_region<'a>(
    plan: Option<&Plan>,
    children: &'a [Child],
    view: ViewId,
    frames: &mut Vec<Frame>,
) -> Option<(&'a collection::Collection, bool)> {
    fn walk<'a>(
        plan: Option<&Plan>,
        children: &'a [Child],
        view: ViewId,
        frames: &mut Vec<Frame>,
        selectable: bool,
    ) -> Option<(&'a collection::Collection, bool)> {
        for child in children {
            match child {
                Child::Node(n) => {
                    let selectable = plan.map_or(selectable, |p| may_select(n, p, selectable));
                    if n.view == view {
                        return n.collection.as_deref().map(|c| (c, selectable));
                    }
                    // A nested list lives in a mounted row (LLP 1070 §4.7).
                    if let Some(c) = &n.collection {
                        for (_, row) in c.mounted_rows() {
                            frames.push(row.frame.clone());
                            if let Some(r) = walk(plan, &row.roots, view, frames, selectable) {
                                return Some(r);
                            }
                            frames.pop();
                        }
                    }
                    if let Some(r) = walk(plan, &n.children, view, frames, selectable) {
                        return Some(r);
                    }
                }
                Child::Region(r) => match &r.active {
                    Active::Arm { roots, frame, .. } => {
                        frames.push(frame.scope());
                        if let Some(r) = walk(plan, roots, view, frames, selectable) {
                            return Some(r);
                        }
                        frames.pop();
                    }
                    Active::Rows { rows } => {
                        for row in rows {
                            frames.push(row.frame.clone());
                            if let Some(r) = walk(plan, &row.roots, view, frames, selectable) {
                                return Some(r);
                            }
                            frames.pop();
                        }
                    }
                },
            }
        }
        None
    }
    walk(plan, children, view, frames, true)
}

fn hidden(n: &NodeInst, plan: &Plan) -> bool {
    plan.node(n.node).bindings.iter().enumerate().any(|(i, b)| {
        let b = plan.binding(b);
        b.kind == BindingKind::Style
            && b.id == StyleId::Display as u16
            && n.last[i].as_ref().and_then(Value::as_str) == Some("none")
    })
}

/// Whether a node's text may be selected, `inherited` being its parent's:
/// the nearest authored `user-select` decides, as in CSS and on the Mac
/// (`textSelectable`), so `none` here or above leaves the text out of a
/// copy, as the browser and the Mac's own copy of plain text leave it out.
fn may_select(n: &NodeInst, plan: &Plan, inherited: bool) -> bool {
    match n
        .bound_style(plan, StyleId::UserSelect)
        .and_then(Value::as_str)
    {
        Some("none") => false,
        None | Some("auto") => inherited,
        Some(_) => true,
    }
}

/// A row's paragraphs: each outer text, its inline texts joined in, and
/// whether it may be selected. Every text counts toward the ordinals, which
/// the hosts number the same way, selectable or not.
fn collect(
    u: &mut Update<'_>,
    children: &[Child],
    frames: &[Frame],
    inline: bool,
    selectable: bool,
    out: &mut Vec<(String, bool)>,
) -> Result<(), InstanceError> {
    for child in children {
        match child {
            Child::Node(n) if !hidden(n, u.env.plan) => {
                let selectable = may_select(n, u.env.plan, selectable);
                let text = u.env.plan.node(n.node).node_type == NodeType::Text as u8;
                if text && !inline {
                    out.push((String::new(), selectable));
                }
                if text {
                    if let Some(value) = n
                        .bound_prop(u.env.plan, PropId::Text)
                        .and_then(Value::as_str)
                    {
                        if let Some((last, _)) = out.last_mut() {
                            last.push_str(value);
                        }
                    }
                }
                collect(u, &n.children, frames, inline || text, selectable, out)?;
            }
            Child::Node(_) => {}
            Child::Region(r) => match &r.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.scope());
                    collect(u, roots, &inner, inline, selectable, out)?;
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        collect(u, &row.roots, &inner, inline, selectable, out)?;
                    }
                }
            },
        }
    }
    Ok(())
}

impl Tree {
    /// Resolve a row key without creating its views.
    pub fn list_index(&self, view: ViewId, key: &str) -> Option<usize> {
        list_region(None, &self.children, view, &mut Vec::new())?
            .0
            .logical_index(key)
    }

    /// Text for all rows, or two stable UTF-16 endpoints, without mutation.
    /// Text the reader cannot select (`user-select: none`) is left out; it
    /// still counts toward an endpoint's paragraph ordinal.
    pub fn list_text(
        &self,
        u: &mut Update<'_>,
        view: ViewId,
        range: Option<(ListTextPosition<'_>, ListTextPosition<'_>)>,
    ) -> Result<String, InstanceError> {
        let mut frames = Vec::new();
        let (list, selectable) = list_region(Some(u.env.plan), &self.children, view, &mut frames)
            .ok_or(InstanceError::List("unknown list"))?;
        let mut start = (0, 0, 0);
        let mut end = (list.logical_len().saturating_sub(1), usize::MAX, usize::MAX);
        if let Some((a, b)) = range {
            let position = |p: ListTextPosition<'_>| {
                list.logical_index(p.key)
                    .map(|i| (i, p.paragraph, p.offset))
                    .ok_or(InstanceError::List("selection row no longer exists"))
            };
            start = position(a)?;
            end = position(b)?;
            if start > end {
                std::mem::swap(&mut start, &mut end);
            }
        }
        let mounted: exact_kernel::SortedMap<usize, &Row> = list.mounted_rows().collect();
        let mut result = String::new();
        for i in start.0..list.logical_len().min(end.0.saturating_add(1)) {
            let temporary;
            let row = if let Some(row) = mounted.get(&i) {
                *row
            } else {
                temporary = list.logical_row(u, i, &frames)?;
                &temporary
            };
            let mut inner = frames.clone();
            inner.push(row.frame.clone());
            let mut paragraphs = Vec::new();
            collect(u, &row.roots, &inner, false, selectable, &mut paragraphs)?;
            for (p, (text, selectable)) in paragraphs.into_iter().enumerate() {
                if !selectable || (i, p) < (start.0, start.1) || (i, p) > (end.0, end.1) {
                    continue;
                }
                let units: Vec<_> = text.encode_utf16().collect();
                let lo = if (i, p) == (start.0, start.1) {
                    start.2.min(units.len())
                } else {
                    0
                };
                let hi = if (i, p) == (end.0, end.1) {
                    end.2.min(units.len())
                } else {
                    units.len()
                };
                if hi <= lo {
                    continue;
                }
                if !result.is_empty() {
                    result.push_str("\n\n");
                }
                result.push_str(&String::from_utf16_lossy(&units[lo..hi]));
            }
            u.ops.clear();
            u.surfaces.clear();
        }
        Ok(result)
    }
}
