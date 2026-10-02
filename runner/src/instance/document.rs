//! The instance tree as a document's nodes, with no kernel (LLP 1048.004).
//!
//! A render whose runner mirrors no kernel (a detached one, whose updates
//! build no style or prop ops, [`Update::discard`]) writes its document from
//! here. Each node's rows and props are folded from the values its bindings
//! last took, as the ops a kernel would have applied leave them: a prop by
//! its declared kind, a style row through the kernel's own `set_dynamic`, a
//! value a row refuses (or `none`) unset. Its children are the tree's, in the
//! order the tree gives its kernel. What a kernel would have refused — a row
//! outside its domain, a child its parent can't hold, a tree too deep — is
//! refused here too, and so is what the fold doesn't cover (a virtualized
//! list, a relative length): the render then renders again with a kernel.

use super::collection::views::ListChild;
use super::*;
use exact_kernel::id::IdMap;
use exact_kernel::{NodeFacts, PropList, SortedMap, StyleId};
use exact_plan::EventKind;
use std::rc::Rc;

/// One node of a document written without a kernel.
#[derive(Debug)]
pub struct DocNode {
    /// Its view, as the kernel would have named it.
    pub id: ViewId,
    /// Its type.
    pub node_type: NodeType,
    /// Its parent, `None` for a root.
    pub parent: Option<ViewId>,
    /// Its children, in order.
    pub children: Vec<ViewId>,
    /// Its rows. Nodes whose style bindings read nothing share one, so a
    /// projection can compute a style's CSS once for all of them.
    pub style: Rc<StyleProps>,
    /// Its props.
    pub props: PropList,
    /// Whether it is a root.
    pub is_root: bool,
    /// The plan node it realizes; `None` for a virtualized list's spacer or
    /// row wrapper, which the list makes.
    pub site: Option<NodesId>,
}

/// A document's nodes, from the runner's instance tree ([`Tree::document`]).
#[derive(Debug, Default)]
pub struct DocTree {
    nodes: Vec<DocNode>,
    index: IdMap<ViewId, u32>,
    roots: Vec<ViewId>,
    /// The nodes each `id` prop names, in tree order.
    ids: std::collections::HashMap<String, Vec<ViewId>>,
}

/// Why a tree has no kernel-free document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocTreeError {
    /// The fold doesn't cover this; a kernel render does.
    Unsupported(&'static str),
    /// A kernel would have refused the tree: a kernel render reports it.
    Refused(String),
}

impl std::fmt::Display for DocTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocTreeError::Unsupported(what) => write!(f, "not written without a kernel: {what}"),
            DocTreeError::Refused(why) => write!(f, "a kernel would refuse the tree: {why}"),
        }
    }
}

impl DocTree {
    /// The roots, in order.
    pub fn roots(&self) -> &[ViewId] {
        &self.roots
    }

    /// Every node, parents before their children.
    pub fn nodes(&self) -> &[DocNode] {
        &self.nodes
    }

    /// The node `id`.
    pub fn node(&self, id: ViewId) -> Option<&DocNode> {
        self.index.get(&id).map(|i| &self.nodes[*i as usize])
    }

    /// The facts a projection reads of node `id`.
    pub fn facts(&self, id: ViewId) -> Option<NodeFacts<'_>> {
        let node = self.node(id)?;
        let inline_run = node.node_type == NodeType::Text
            && node
                .parent
                .and_then(|p| self.node(p))
                .is_some_and(|p| p.node_type == NodeType::Text);
        Some(NodeFacts {
            id: node.id,
            node_type: node.node_type,
            style: &node.style,
            props: &node.props,
            is_root: node.is_root,
            inline_run,
        })
    }

    /// The active head, as [`crate::Runner::head`] reads a kernel's tree.
    pub fn head(&self) -> crate::Head {
        crate::head::head_of(self.roots.clone(), |id| {
            self.node(id)
                .map(|n| (n.node_type, &n.props, n.children.clone()))
        })
    }

    /// Every node's listener declarations, as [`crate::Runner::handlers`]
    /// reads them for a kernel's live views.
    pub fn handlers(&self, plan: &Plan) -> SortedMap<ViewId, Vec<EventKind>> {
        self.nodes
            .iter()
            .filter_map(|n| Some((n.id, plan.node(n.site?).handlers)))
            .filter(|(_, handlers)| handlers.len > 0)
            .map(|(id, handlers)| (id, handlers.iter().map(|h| plan.handler(h).event).collect()))
            .collect()
    }

    /// The node an SVG reference `#id` from `from` names, by the kernel's
    /// rule (`Kernel::resolve_id`): the one node with that `id`, or of
    /// several the nearest to `from`.
    pub fn resolve_id(&self, from: ViewId, id: &str) -> Option<ViewId> {
        match self.ids.get(id)?.as_slice() {
            [] => None,
            [one] => Some(*one),
            several => exact_kernel::svg::refs::nearest(
                several,
                self.node(from)?.id,
                |node| self.node(node).and_then(|n| n.parent),
                |parent, child| {
                    self.node(parent)
                        .and_then(|p| p.children.iter().position(|c| *c == child))
                        .unwrap_or(usize::MAX)
                },
            ),
        }
    }
}

impl Tree {
    /// The document's nodes, from the values the tree's bindings last took.
    pub(crate) fn document(&self, plan: &Plan, sites: &SiteIndex) -> Result<DocTree, DocTreeError> {
        let mut build = Build {
            plan,
            sites,
            doc: DocTree::default(),
            shared: vec![None; plan.nodes.len()],
        };
        build.doc.roots = roots_of(&self.children);
        build.children(&self.children, None, 0)?;
        Ok(build.doc)
    }
}

struct Build<'p> {
    plan: &'p Plan,
    sites: &'p SiteIndex,
    doc: DocTree,
    /// Each plan node's rows when every style binding it has reads nothing:
    /// one allocation for all its instances.
    shared: Vec<Option<Rc<StyleProps>>>,
}

impl Build<'_> {
    fn children(
        &mut self,
        children: &[Child],
        parent: Option<(ViewId, NodeType)>,
        depth: u32,
    ) -> Result<(), DocTreeError> {
        for child in children {
            match child {
                Child::Node(n) => self.node(n, parent, depth)?,
                Child::Region(r) => match &r.active {
                    Active::Arm { roots, .. } => self.children(roots, parent, depth)?,
                    Active::Rows { rows } => {
                        for row in rows {
                            self.children(&row.roots, parent, depth)?;
                        }
                    }
                },
            }
        }
        Ok(())
    }

    fn node(
        &mut self,
        n: &NodeInst,
        parent: Option<(ViewId, NodeType)>,
        depth: u32,
    ) -> Result<(), DocTreeError> {
        let wire = self.plan.node(n.node).node_type;
        let node_type = NodeType::from_wire(wire)
            .ok_or_else(|| DocTreeError::Refused(format!("unknown node type {wire}")))?;
        // What the kernel's `SetChildren` checks (`txn::validate`).
        if let Some((parent_view, parent_type)) = parent {
            let refuse = |why: &str| {
                Err(DocTreeError::Refused(format!(
                    "view {parent_view} ({parent_type:?}) and child {} ({node_type:?}): {why}",
                    n.view
                )))
            };
            if !parent_type.can_hold_children() {
                return refuse("a leaf holds no children");
            }
            if parent_type == NodeType::Text && node_type != NodeType::Text {
                return refuse("a paragraph holds only inline runs");
            }
            if parent_type.is_svg_container() != node_type.is_svg_element() {
                return refuse("an svg holds SVG elements, and they live only there");
            }
        }
        if depth > exact_kernel::MAX_DEPTH {
            return Err(DocTreeError::Refused(format!(
                "view {} is deeper than {}",
                n.view,
                exact_kernel::MAX_DEPTH
            )));
        }
        let (style, props) = self.fold(n)?;
        let list = match &n.collection {
            Some(list) => Some(
                list.document_children(self.plan)
                    .map_err(|e| DocTreeError::Refused(format!("{e:?}")))?,
            ),
            None => None,
        };
        let children = match &list {
            Some(list) => list
                .iter()
                .map(|child| match child {
                    ListChild::Spacer(view, _) | ListChild::Row(view, ..) => *view,
                })
                .collect(),
            None => roots_of(&n.children),
        };
        self.push(DocNode {
            id: n.view,
            node_type,
            parent: parent.map(|(view, _)| view),
            children,
            style,
            props,
            is_root: parent.is_none(),
            site: Some(n.node),
        });
        let here = Some((n.view, node_type));
        let Some(list) = list else {
            return self.children(&n.children, here, depth + 1);
        };
        // A virtualized list: its spacers and its mounted rows' wrappers,
        // which the list makes, around each row's root.
        if depth + 1 > exact_kernel::MAX_DEPTH {
            return Err(DocTreeError::Refused(format!(
                "view {} is deeper than {}",
                n.view,
                exact_kernel::MAX_DEPTH
            )));
        }
        for child in list {
            let (view, style, props, roots) = match child {
                ListChild::Spacer(view, style) => {
                    let mut props = PropList::new();
                    props.set(
                        exact_kernel::PropId::AccessibilityElementsHidden,
                        exact_kernel::PropValue::Bool(true),
                    );
                    (view, style, props, None)
                }
                ListChild::Row(view, style, props, roots) => (view, style, props, Some(roots)),
            };
            self.push(DocNode {
                id: view,
                node_type: NodeType::View,
                parent: Some(n.view),
                children: roots.map(roots_of).unwrap_or_default(),
                style: Rc::new(style),
                props,
                is_root: false,
                site: None,
            });
            if let Some(roots) = roots {
                self.children(roots, Some((view, NodeType::View)), depth + 2)?;
            }
        }
        Ok(())
    }

    fn push(&mut self, node: DocNode) {
        if let Some(id) = node.props.str(exact_kernel::PropId::Id) {
            self.doc
                .ids
                .entry(id.to_string())
                .or_default()
                .push(node.id);
        }
        self.doc.index.insert(node.id, self.doc.nodes.len() as u32);
        self.doc.nodes.push(node);
    }

    /// The node's rows and props, as the kernel holds them after the ops its
    /// bindings' values made (`NodeInst::emit_bindings`).
    fn fold(&mut self, n: &NodeInst) -> Result<(Rc<StyleProps>, PropList), DocTreeError> {
        let plan = self.plan;
        let row = plan.node(n.node);
        let deps = &self.sites.deps;
        let constant = row.bindings.iter().all(|b| {
            plan.binding(b).kind != BindingKind::Style || deps.bindings[b.0 as usize].is_constant()
        });
        let shared = constant
            .then(|| self.shared[n.node.0 as usize].clone())
            .flatten();
        let mut props = PropList::new();
        let mut patch = StyleProps::default();
        for (i, b) in row.bindings.iter().enumerate() {
            let binding = plan.binding(b);
            let Some(value) = n.last[i].as_ref() else {
                continue;
            };
            match binding.kind {
                BindingKind::Prop => match bridge::prop_value(binding.id, value) {
                    Ok((prop, value)) => {
                        props.set(prop, value);
                    }
                    // Cleared, with a journal line on a kernel's commit.
                    Err(bridge::BridgeError::PropKind { prop, .. }) => {
                        props.remove(prop);
                    }
                    Err(e) => return Err(DocTreeError::Refused(format!("{e:?}"))),
                },
                BindingKind::Style if shared.is_some() => {}
                // `none` clears the row: unset in a fold from the default.
                BindingKind::Style if matches!(value, Value::Option(None)) => {}
                BindingKind::Style => {
                    match bridge::set_style(&mut patch, binding.id, value, plan.stacks.len()) {
                        Ok(StyleId::Animation) => {
                            self.sites.keyframes.resolve(&mut patch.animation);
                        }
                        Ok(StyleId::ExitAnimation) => {
                            self.sites.keyframes.resolve(&mut patch.exit_animation);
                        }
                        Ok(_) => {}
                        // A value its row refuses is unset, as on a kernel.
                        Err(
                            bridge::BridgeError::Style(_) | bridge::BridgeError::StyleKind { .. },
                        ) => {}
                        Err(e) => return Err(DocTreeError::Refused(format!("{e:?}"))),
                    }
                }
            }
        }
        if let Some(style) = shared {
            return Ok((style, props));
        }
        if !patch.relative.is_empty() {
            // The kernel resolves `rem` and `em` against inherited font
            // sizes; the fold doesn't.
            return Err(DocTreeError::Unsupported("a relative length (rem, em)"));
        }
        patch
            .validate_domain()
            .map_err(|e| DocTreeError::Refused(format!("view {}: {e:?}", n.view)))?;
        let style = Rc::new(patch);
        if constant {
            self.shared[n.node.0 as usize] = Some(Rc::clone(&style));
        }
        Ok((style, props))
    }
}
