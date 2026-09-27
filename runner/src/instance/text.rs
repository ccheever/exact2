//! Read-only text projection for virtual rows. Uses the same realization and
//! binding evaluation as rendering, but commits no kernel or host operations.
use super::*;
use crate::ListTextPosition;
use exact_kernel::{PropId, StyleId};

/// A virtualized list's rows as logical text sees them: every row, mounted
/// or not.
fn list_region<'a>(
    children: &'a [Child],
    view: ViewId,
    frames: &mut Vec<Frame>,
) -> Option<&'a collection::Collection> {
    fn walk<'a>(
        children: &'a [Child],
        view: ViewId,
        frames: &mut Vec<Frame>,
    ) -> Option<&'a collection::Collection> {
        for child in children {
            match child {
                Child::Node(n) => {
                    if n.view == view {
                        return n.collection.as_deref();
                    }
                    if let Some(r) = walk(&n.children, view, frames) {
                        return Some(r);
                    }
                }
                Child::Region(r) => match &r.active {
                    Active::Arm { roots, frame, .. } => {
                        frames.push(frame.clone());
                        if let Some(r) = walk(roots, view, frames) {
                            return Some(r);
                        }
                        frames.pop();
                    }
                    Active::Rows { rows } => {
                        for row in rows {
                            frames.push(row.frame.clone());
                            if let Some(r) = walk(&row.roots, view, frames) {
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
    walk(children, view, frames)
}

fn hidden(n: &NodeInst, plan: &Plan) -> bool {
    plan.node(n.node).bindings.iter().enumerate().any(|(i, b)| {
        let b = plan.binding(b);
        b.kind == BindingKind::Style
            && b.id == StyleId::Display as u16
            && matches!(&n.last[i], Some(Value::Str(s)) if s.as_ref() == "none")
    })
}

fn collect(
    u: &mut Update<'_>,
    children: &[Child],
    frames: &[Frame],
    inline: bool,
    out: &mut Vec<String>,
) -> Result<(), InstanceError> {
    for child in children {
        match child {
            Child::Node(n) if !hidden(n, u.env.plan) => {
                let text = u.env.plan.node(n.node).node_type == NodeType::Text as u8;
                if text && !inline {
                    out.push(String::new());
                }
                if text {
                    if let Some(Value::Str(value)) = n.bound_prop(u.env.plan, PropId::Text) {
                        if let Some(last) = out.last_mut() {
                            last.push_str(value);
                        }
                    }
                }
                collect(u, &n.children, frames, inline || text, out)?;
            }
            Child::Node(_) => {}
            Child::Region(r) => match &r.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    collect(u, roots, &inner, inline, out)?;
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        collect(u, &row.roots, &inner, inline, out)?;
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
        list_region(&self.children, view, &mut Vec::new())?.logical_index(key)
    }

    /// Text for all rows, or two stable UTF-16 endpoints, without mutation.
    pub fn list_text(
        &self,
        u: &mut Update<'_>,
        view: ViewId,
        range: Option<(ListTextPosition<'_>, ListTextPosition<'_>)>,
    ) -> Result<String, InstanceError> {
        let mut frames = Vec::new();
        let list = list_region(&self.children, view, &mut frames)
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
            collect(u, &row.roots, &inner, false, &mut paragraphs)?;
            for (p, text) in paragraphs.into_iter().enumerate() {
                if (i, p) < (start.0, start.1) || (i, p) > (end.0, end.1) {
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
