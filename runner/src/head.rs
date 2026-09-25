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
use exact_kernel::{Kernel, NodeRef, NodeType, PropId, PropValue, ViewId};

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

impl<D: DataSource> Runner<D> {
    /// The active head of the current tree: one walk from the roots, which
    /// skips the routes a navigation root has not selected.
    pub fn head(&self) -> Head {
        let kernel = self.kernel();
        let mut best: [Option<(usize, &str)>; 5] = [None; 5];
        let mut status: Option<(usize, u16)> = None;
        let mut stack: Vec<(ViewId, usize)> = self
            .roots()
            .into_iter()
            .rev()
            .map(|root| (root, 0))
            .collect();
        while let Some((id, depth)) = stack.pop() {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            if node.node_type == NodeType::Head {
                for (slot, prop) in best.iter_mut().zip(FIELDS) {
                    if let Some(value) = node.props.str(prop) {
                        if slot.is_none_or(|(at, _)| depth >= at) {
                            *slot = Some((depth, value));
                        }
                    }
                }
                if let Some(&PropValue::Int(code)) = node.props.get(PropId::HeadStatus) {
                    if status.is_none_or(|(at, _)| depth >= at) {
                        status = u16::try_from(code).ok().map(|code| (depth, code));
                    }
                }
                continue;
            }
            let selected = selected_route(kernel, &node);
            for child in node.children().into_iter().rev() {
                if !covered(kernel, selected, child) {
                    stack.push((child, depth + 1));
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
        }
    }

    /// Whether `id` lies under a route its navigation root has not selected:
    /// a screen a stack keeps mounted under its top, which every host hides
    /// and makes inert, so nothing there is pressed or read.
    pub fn inactive(&self, id: ViewId) -> bool {
        let kernel = self.kernel();
        let mut child = id;
        let mut parent = kernel.node(id).and_then(|node| node.parent);
        while let Some(node) = parent.and_then(|id| kernel.node(id)) {
            if covered(kernel, selected_route(kernel, &node), child) {
                return true;
            }
            child = node.id;
            parent = node.parent;
        }
        false
    }
}

/// A navigation root's selected route, when its key names one of its
/// routes; `None` for any other node — a key that names none leaves every
/// route as it is.
fn selected_route<'a>(kernel: &'a Kernel, node: &NodeRef<'a>) -> Option<&'a str> {
    let key = node
        .props
        .str(PropId::NavigationBack)
        .and(node.props.str(PropId::NavigationKey))?;
    node.children()
        .into_iter()
        .any(|c| {
            kernel
                .node(c)
                .is_some_and(|c| c.props.str(PropId::NavigationKey) == Some(key))
        })
        .then_some(key)
}

/// Whether `child`, under a root that selected `selected`, is a route that
/// root has not selected.
fn covered(kernel: &Kernel, selected: Option<&str>, child: ViewId) -> bool {
    selected.is_some_and(|key| {
        kernel
            .node(child)
            .and_then(|c| c.props.str(PropId::NavigationKey))
            .is_some_and(|route| route != key)
    })
}
