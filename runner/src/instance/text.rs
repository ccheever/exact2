//! Read-only text projection for virtual rows. Uses the same realization and
//! binding evaluation as rendering, but commits no kernel or host operations.
use super::*;
use crate::{ListTextPolicy, ListTextPosition};
use exact_kernel::{PropId, StyleId};

/// A virtualized list's rows as logical text sees them: every row, mounted
/// or not. Given the plan, also whether the list's text may be selected:
/// the host policy and the `user-select` it has or takes from above.
fn list_region<'a>(
    plan: Option<(&Plan, &SiteIndex)>,
    children: &'a [Child],
    view: ViewId,
    frames: &mut Vec<Frame>,
    policy: ListTextPolicy,
) -> Option<(&'a collection::Collection, Selection)> {
    fn walk<'a>(
        plan: Option<(&Plan, &SiteIndex)>,
        children: &'a [Child],
        view: ViewId,
        frames: &mut Vec<Frame>,
        selectable: Selection,
        policy: ListTextPolicy,
    ) -> Option<(&'a collection::Collection, Selection)> {
        for child in children {
            match child {
                Child::Node(n) => {
                    let selectable = plan.map_or(selectable, |p| selectable.at(n, p, policy));
                    if n.view == view {
                        return n.collection().map(|c| (c, selectable));
                    }
                    // A nested list lives in a mounted row (LLP 1070 §4.7).
                    if let Some(c) = n.collection() {
                        for (_, row) in c.mounted_rows() {
                            frames.push(row.frame.clone());
                            if let Some(r) =
                                walk(plan, &row.roots, view, frames, selectable, policy)
                            {
                                return Some(r);
                            }
                            frames.pop();
                        }
                    }
                    if let Some(r) = walk(plan, &n.children, view, frames, selectable, policy) {
                        return Some(r);
                    }
                }
                Child::Region(r) => match &r.active {
                    Active::Arm { roots, frame, .. } => {
                        frames.push(frame.scope());
                        if let Some(r) = walk(plan, roots, view, frames, selectable, policy) {
                            return Some(r);
                        }
                        frames.pop();
                    }
                    Active::Rows { rows } => {
                        for row in rows {
                            frames.push(row.frame.clone());
                            if let Some(r) =
                                walk(plan, &row.roots, view, frames, selectable, policy)
                            {
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
    walk(plan, children, view, frames, Selection::default(), policy)
}

fn hidden(n: &NodeInst, plan: &Plan, sites: &SiteIndex) -> bool {
    plan.node(n.node).bindings.iter().enumerate().any(|(i, b)| {
        let b = plan.binding(b);
        b.kind == BindingKind::Style
            && b.id == StyleId::Display as u16
            && n.emitted(plan, sites, i).as_ref().and_then(Value::as_str) == Some("none")
    })
}

/// Top-down equivalent of AppKit's `textSelectable` walk. A nearer
/// authored value wins over a control; a nearer control wins over an
/// ancestor's value. Document/chrome defaults matter only without either.
/// Text the reader can neither see nor reach takes no part, as in Chrome
/// and AppKit's own selection: computed `visibility: hidden` (inherited, a
/// nearer `visible` shows again), and `inert` here or above.
#[derive(Clone, Copy, Default)]
struct Selection {
    explicit: Option<bool>,
    document: bool,
    chrome: bool,
    hidden: bool,
    inert: bool,
}
impl Selection {
    fn at(self, n: &NodeInst, (plan, sites): (&Plan, &SiteIndex), policy: ListTextPolicy) -> Self {
        let mut next = self;
        let node = plan.node(n.node);
        let prop = |id| {
            n.bound_prop(plan, sites, id)
                .as_ref()
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_default()
        };
        // A control's label is not selectable, AppKit's on the Mac and the
        // host stylesheet's on the web (`host/web/index.html`). A text's runs
        // join its paragraph, which `collect` judges at the text.
        let role = prop(PropId::AccessibilityRole);
        let role = role.as_str();
        let control = [NodeType::Pressable as u8, NodeType::Control as u8]
            .contains(&node.node_type)
            || node
                .handlers
                .iter()
                .any(|h| plan.handler(h).event == exact_plan::EventKind::Press)
            || [
                "button",
                "link",
                "tab",
                "menuitem",
                "menuitemcheckbox",
                "menuitemradio",
                "option",
                "checkbox",
                "radio",
                "switch",
                "treeitem",
            ]
            .contains(&role);
        if control {
            next.explicit = Some(false);
        }
        if policy == ListTextPolicy::MacOS {
            let tag = prop(PropId::SemanticTag);
            let tag = tag.as_str();
            next.document |= tag == "article" || ["article", "document"].contains(&role);
            next.chrome |= ["header", "nav", "footer"].contains(&tag)
                || ["toolbar", "tablist", "menubar", "menu", "listbox", "tree"].contains(&role);
        }
        match n
            .bound_style(plan, sites, StyleId::UserSelect)
            .as_ref()
            .and_then(Value::as_str)
        {
            Some("none") => next.explicit = Some(false),
            None | Some("auto") => {}
            Some(_) => next.explicit = Some(true),
        }
        if let Some(visibility) = n
            .bound_style(plan, sites, StyleId::Visibility)
            .as_ref()
            .and_then(Value::as_str)
        {
            next.hidden = visibility == "hidden";
        }
        next.inert |= n
            .bound_prop(plan, sites, PropId::Inert)
            .as_ref()
            .and_then(Value::as_bool)
            == Some(true);
        next
    }
    fn selectable(self) -> bool {
        !self.hidden && !self.inert && self.explicit.unwrap_or(self.document || !self.chrome)
    }
}

/// A row's paragraphs: each outer text, its inline texts joined in, and
/// whether it may be selected. Text without a host paragraph (an AppKit
/// native title) is omitted; every other text counts, selectable or not.
fn collect(
    u: &mut Update<'_>,
    children: &[Child],
    frames: &[Frame],
    inline: bool,
    selectable: Selection,
    policy: ListTextPolicy,
    out: &mut Vec<(String, bool)>,
) -> Result<(), InstanceError> {
    for child in children {
        match child {
            Child::Node(n) if !hidden(n, u.env.plan, u.sites) => {
                let selectable = selectable.at(n, (u.env.plan, u.sites), policy);
                if policy == ListTextPolicy::MacOS
                    && u.env.plan.node(n.node).node_type == NodeType::Control as u8
                    && n.bound_prop(u.env.plan, u.sites, PropId::Type)
                        .as_ref()
                        .and_then(Value::as_str)
                        == Some("button")
                {
                    continue;
                }
                let text = u.env.plan.node(n.node).node_type == NodeType::Text as u8;
                if text && !inline {
                    out.push((String::new(), selectable.selectable()));
                }
                // An image in a paragraph's text copies as its label (LLP 1011.002 D4).
                let image = inline && u.env.plan.node(n.node).node_type == NodeType::Image as u8;
                if text || image {
                    let prop = if image {
                        PropId::AccessibilityLabel
                    } else {
                        PropId::Text
                    };
                    if let Some(value) = n
                        .bound_prop(u.env.plan, u.sites, prop)
                        .as_ref()
                        .and_then(Value::as_str)
                    {
                        if let Some((last, _)) = out.last_mut() {
                            last.push_str(value);
                        }
                    }
                }
                collect(
                    u,
                    &n.children,
                    frames,
                    inline || text,
                    selectable,
                    policy,
                    out,
                )?;
            }
            Child::Node(_) => {}
            Child::Region(r) => match &r.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.scope());
                    collect(u, roots, &inner, inline, selectable, policy, out)?;
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        collect(u, &row.roots, &inner, inline, selectable, policy, out)?;
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
        list_region(
            None,
            &self.children,
            view,
            &mut Vec::new(),
            ListTextPolicy::Web,
        )?
        .0
        .logical_index(key)
    }

    /// Text for all rows, or two stable UTF-16 endpoints, without mutation.
    /// Text the reader cannot select (`user-select: none`, `visibility:
    /// hidden`, `inert`) is left out; it still counts toward an endpoint's
    /// paragraph ordinal.
    pub fn list_text(
        &self,
        u: &mut Update<'_>,
        view: ViewId,
        range: Option<(ListTextPosition<'_>, ListTextPosition<'_>)>,
        policy: ListTextPolicy,
    ) -> Result<String, InstanceError> {
        let mut frames = Vec::new();
        let (list, selectable) = list_region(
            Some((u.env.plan, u.sites)),
            &self.children,
            view,
            &mut frames,
            policy,
        )
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
            collect(
                u,
                &row.roots,
                &inner,
                false,
                selectable,
                policy,
                &mut paragraphs,
            )?;
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
