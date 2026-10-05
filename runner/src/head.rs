//! The document's head: the active `head` elements' fields.
//!
//! @ref LLP 1048.003 D1
//!
//! A `head` goes anywhere in a view, and the innermost active one wins, field
//! by field: a deeper head wins over a shallower one, and at one depth the
//! later in document order wins. A head inside a route its navigation root
//! has not selected is inactive — the selection the web host's projection
//! hides and makes inert (`navigation.project`), so a covered route's title
//! is never the page's. Every host asks the runner, so the page's `<head>`,
//! a window's title and the agent's `state` agree. The agent's `tree` reads
//! the same selection ([`Runner::inactive`]): a testId on a covered screen
//! is flagged, and resolves only when no active screen carries it.

use crate::{DataSource, Runner};
use exact_kernel::{NodeType, PropId, PropList, PropValue, ViewId};

/// The active head's fields; `None` where no active head sets one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Head {
    /// The page or window title.
    pub title: Option<String>,
    /// A summary for search results and link previews.
    pub description: Option<String>,
    /// An image for link previews.
    pub image: Option<String>,
    /// The page's canonical URL.
    pub canonical: Option<String>,
    /// Directions to crawlers (`noindex`, `nofollow`, …).
    pub robots: Option<String>,
    /// The HTTP status the view declares (LLP 1048.000 D11): 404 or 410 for
    /// a not-found view, 503 for a view of failed data. Native hosts ignore
    /// it.
    pub status: Option<u16>,
    /// The document has unsaved changes (`head edited`, LLP 1069.010 D6):
    /// a Mac window's edited mark. A declared deviation; the web has none.
    pub edited: bool,
}

impl Head {
    /// Each text field with its `head` attribute name, in declaration
    /// order; `status` is the one number.
    pub fn fields(&self) -> [(&'static str, Option<&str>); 5] {
        [
            ("title", self.title.as_deref()),
            ("description", self.description.as_deref()),
            ("image", self.image.as_deref()),
            ("canonical", self.canonical.as_deref()),
            ("robots", self.robots.as_deref()),
        ]
    }
}

const FIELDS: [PropId; 5] = [
    PropId::HeadTitle,
    PropId::HeadDescription,
    PropId::HeadImage,
    PropId::HeadCanonical,
    PropId::HeadRobots,
];

/// A node as the head's walk reads it: its type, props and children.
pub(crate) type HeadNode<'a> = (NodeType, &'a PropList, Vec<ViewId>);

impl<D: DataSource> Runner<D> {
    /// The active head of the current tree: one walk from the roots, which
    /// skips the routes a navigation root has not selected.
    pub fn head(&self) -> Head {
        let kernel = self.kernel();
        head_of(self.roots(), |id| {
            kernel
                .node(id)
                .map(|n| (n.node_type, n.props, n.children()))
        })
    }

    /// Whether `id` lies under a route its navigation root has not selected:
    /// a screen a stack keeps mounted under its top, or any screen of a tab
    /// that is not the selected one (LLP 1075.003 §3.7; shop F16), which
    /// every host hides and makes inert, so nothing there is pressed or read.
    pub fn inactive(&self, id: ViewId) -> bool {
        let kernel = self.kernel();
        let node = |id: ViewId| {
            kernel
                .node(id)
                .map(|n| (n.node_type, n.props, n.children()))
        };
        let mut path = vec![id];
        let mut parent = kernel.node(id).and_then(|node| node.parent);
        while let Some(at) = parent.and_then(|id| kernel.node(id)) {
            if let Some(n) = node(at.id) {
                if unselected(&node, &n).iter().any(|off| path.contains(off)) {
                    return true;
                }
            }
            path.push(at.id);
            parent = at.parent;
        }
        false
    }
}

/// The active head of the tree under `roots`, as `node` reads it: the walk
/// [`Runner::head`] makes over the kernel, and a render host over the
/// document it writes without one (LLP 1048.004).
pub(crate) fn head_of<'a>(
    roots: Vec<ViewId>,
    node: impl Fn(ViewId) -> Option<HeadNode<'a>>,
) -> Head {
    let mut best: [Option<(usize, &str)>; 5] = [None; 5];
    let mut status: Option<(usize, u16)> = None;
    let mut edited: Option<(usize, bool)> = None;
    let mut stack: Vec<(ViewId, usize)> = roots.into_iter().rev().map(|root| (root, 0)).collect();
    while let Some((id, depth)) = stack.pop() {
        let Some(at) = node(id) else {
            continue;
        };
        let props: &'a PropList = at.1;
        if at.0 == NodeType::Head {
            for (slot, prop) in best.iter_mut().zip(FIELDS) {
                if let Some(value) = props.str(prop) {
                    if slot.is_none_or(|(at, _)| depth >= at) {
                        *slot = Some((depth, value));
                    }
                }
            }
            if let Some(&PropValue::Bool(on)) = props.get(PropId::HeadEdited) {
                if edited.is_none_or(|(at, _)| depth >= at) {
                    edited = Some((depth, on));
                }
            }
            if let Some(&PropValue::Int(code)) = props.get(PropId::HeadStatus) {
                if status.is_none_or(|(at, _)| depth >= at) {
                    status = u16::try_from(code).ok().map(|code| (depth, code));
                }
            }
            continue;
        }
        let selected = selected_route(&node, &at);
        for child in at.2.iter().rev() {
            if !covered(&node, selected, *child) {
                stack.push((*child, depth + 1));
            }
        }
    }
    let [title, description, image, canonical, robots] =
        best.map(|slot| slot.map(|(_, value)| value.to_owned()));
    Head {
        title,
        description,
        image,
        canonical,
        robots,
        status: status.map(|(_, code)| code),
        edited: edited.is_some_and(|(_, on)| on),
    }
}

/// A navigation root's selected route, when its key names one of its
/// routes; `None` for any other node — a key that names none leaves every
/// route as it is.
fn selected_route<'a>(
    node: &impl Fn(ViewId) -> Option<HeadNode<'a>>,
    at: &HeadNode<'a>,
) -> Option<&'a str> {
    let props: &'a PropList = at.1;
    let key = props
        .str(PropId::NavigationBack)
        .and(props.str(PropId::NavigationKey))?;
    at.2.iter()
        .any(|c| node(*c).is_some_and(|(_, c, _)| c.str(PropId::NavigationKey) == Some(key)))
        .then_some(key)
}

/// Whether `child`, under a root that selected `selected`, is a route that
/// root has not selected.
fn covered<'a>(
    node: &impl Fn(ViewId) -> Option<HeadNode<'a>>,
    selected: Option<&str>,
    child: ViewId,
) -> bool {
    selected.is_some_and(|key| {
        node(child)
            .and_then(|(_, c, _)| c.str(PropId::NavigationKey))
            .is_some_and(|route| route != key)
    })
}

/// What a navigation root leaves unselected, as every host's projection
/// hides it (LLP 1075.003 §3.7, host/web/navigation.js `project`): with
/// tabs — the tabpanels its own tablist's tabs name with `aria-controls` —
/// every panel but the one whose routes hold the selected key, and that
/// panel's other routes; without, its other direct routes. Empty for any
/// other node, and where the key names no route.
fn unselected<'a>(
    node: &impl Fn(ViewId) -> Option<HeadNode<'a>>,
    at: &HeadNode<'a>,
) -> Vec<ViewId> {
    let Some(key) =
        at.1.str(PropId::NavigationBack)
            .and(at.1.str(PropId::NavigationKey))
    else {
        return Vec::new();
    };
    let key_of = |id: ViewId| node(id).and_then(|(_, p, _)| p.str(PropId::NavigationKey));
    let routes = |ids: &[ViewId]| -> Vec<ViewId> {
        ids.iter()
            .copied()
            .filter(|r| key_of(*r).is_some())
            .collect()
    };
    let panels = tab_panels(node, &at.2);
    let stacks: Vec<Vec<ViewId>> = if panels.is_empty() {
        vec![routes(&at.2)]
    } else {
        panels
            .iter()
            .map(|p| node(*p).map(|(_, _, c)| routes(&c)).unwrap_or_default())
            .collect()
    };
    let Some(selected) = stacks
        .iter()
        .position(|rs| rs.iter().any(|r| key_of(*r) == Some(key)))
    else {
        return Vec::new();
    };
    let mut off: Vec<ViewId> = panels
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != selected)
        .map(|(_, p)| *p)
        .collect();
    off.extend(stacks[selected].iter().filter(|r| key_of(**r) != Some(key)));
    off
}

/// A navigation root's tabpanels, in tab order: those its own tablist's
/// tabs name with `aria-controls` (the browser's `panelsOf`; the Linux
/// host's `tab_panels`). A tablist inside a route is that route's.
fn tab_panels<'a>(
    node: &impl Fn(ViewId) -> Option<HeadNode<'a>>,
    children: &[ViewId],
) -> Vec<ViewId> {
    let (mut lists, mut panels, mut stack) = (Vec::new(), Vec::new(), children.to_vec());
    stack.reverse();
    while let Some(id) = stack.pop() {
        let Some((_, props, kids)) = node(id) else {
            continue;
        };
        let role = props.str(PropId::AccessibilityRole);
        match role {
            Some("tabpanel") => panels.push((id, props.str(PropId::Id))),
            Some("tablist") => lists.push(kids.clone()),
            _ => {}
        }
        if props.str(PropId::NavigationKey).is_none() && role != Some("tablist") {
            stack.extend(kids.iter().rev());
        }
    }
    lists
        .iter()
        .map(|tabs| -> Vec<ViewId> {
            tabs.iter()
                .filter_map(|t| node(*t))
                .filter(|(_, p, _)| p.str(PropId::AccessibilityRole) == Some("tab"))
                .filter_map(|(_, p, _)| p.str(PropId::AccessibilityControls))
                .filter_map(|name| {
                    panels
                        .iter()
                        .find(|(_, id)| *id == Some(name))
                        .map(|(p, _)| *p)
                })
                .collect()
        })
        .find(|named| !named.is_empty())
        .unwrap_or_default()
}
