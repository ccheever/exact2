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
    /// a screen a stack keeps mounted under its top, which every host hides
    /// and makes inert, so nothing there is pressed or read.
    pub fn inactive(&self, id: ViewId) -> bool {
        let kernel = self.kernel();
        let node = |id: ViewId| {
            kernel
                .node(id)
                .map(|n| (n.node_type, n.props, n.children()))
        };
        let mut child = id;
        let mut parent = kernel.node(id).and_then(|node| node.parent);
        while let Some(at) = parent.and_then(|id| kernel.node(id)) {
            let selected = node(at.id).and_then(|n| selected_route(&node, &n));
            if covered(&node, selected, child) {
                return true;
            }
            child = at.id;
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
