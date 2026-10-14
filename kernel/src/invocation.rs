//! What pressing a button does to an element it names, as HTML's button
//! activation resolves it, for the commands Exact runs: the popover
//! commands, `show-modal` and `close`.
//!
//! A `commandfor` that names an element governs and nothing falls back, even
//! when its `command` cannot act on that element; otherwise `popovertarget`
//! acts by `popovertargetaction`. Keywords match ASCII case-insensitively and
//! untrimmed; an invalid `popovertargetaction` toggles. (HTML: the button
//! element's activation behaviour, and the popover target attribute
//! activation behaviour.)
//!
//! The kernel's header rule (`bars.rs`) and the hosts that read the kernel's
//! tree resolve through here; the Apple hosts resolve their own view mirror
//! by the same rule (`Invocation.swift`). Both read one table of cases,
//! `kernel/tests/it/fixtures/invocation.tsv`.

use crate::generated::PropId;
use crate::{ControlKind, Kernel, NodeKey, NodeRef, NodeType};

/// What a button does to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `show-popover`, or `popovertargetaction="show"`.
    Show,
    /// `hide-popover`, or `popovertargetaction="hide"`.
    Hide,
    /// `toggle-popover`, or a `popovertarget` with any other action.
    Toggle,
    /// `show-modal` on a `dialog`.
    ShowModal,
    /// `close` on a `dialog`.
    Close,
}

/// A button's target and what activating it does there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invocation {
    /// The element named, held by key: presentation acts on it, never on
    /// its name read again.
    pub target: NodeKey,
    /// What activating the button does to it.
    pub action: Action,
}

impl Invocation {
    /// What `source` invokes now; `None` when it is not a button or invokes
    /// nothing.
    pub fn of(kernel: &Kernel, source: &NodeRef<'_>) -> Option<Invocation> {
        let button = source.node_type == NodeType::Pressable
            || ControlKind::of(source.node_type, source.props) == Some(ControlKind::Button);
        if !button {
            return None;
        }
        let named = |prop| source.props.str(prop).filter(|name| !name.is_empty());
        if let Some(element) = named(PropId::Commandfor).and_then(|name| element(kernel, name)) {
            let Named::View(commanded) = element else {
                return None;
            };
            let popover = commanded.props.get(PropId::Popover).is_some();
            let dialog = commanded.props.str(PropId::SemanticTag) == Some("dialog");
            let command = source.props.str(PropId::Command)?;
            let action = if popover && command.eq_ignore_ascii_case("show-popover") {
                Action::Show
            } else if popover && command.eq_ignore_ascii_case("hide-popover") {
                Action::Hide
            } else if popover && command.eq_ignore_ascii_case("toggle-popover") {
                Action::Toggle
            } else if dialog && command.eq_ignore_ascii_case("show-modal") {
                Action::ShowModal
            } else if dialog && command.eq_ignore_ascii_case("close") {
                Action::Close
            } else {
                return None;
            };
            return Some(Invocation {
                target: commanded.key,
                action,
            });
        }
        let Named::View(popover) = element(kernel, named(PropId::Popovertarget)?)? else {
            return None;
        };
        popover.props.get(PropId::Popover)?;
        let action = match source.props.str(PropId::Popovertargetaction) {
            Some(a) if a.eq_ignore_ascii_case("show") => Action::Show,
            Some(a) if a.eq_ignore_ascii_case("hide") => Action::Hide,
            _ => Action::Toggle,
        };
        Some(Invocation {
            target: popover.key,
            action,
        })
    }

    /// Whether it opens a popover, as a header's menu button does:
    /// `show-modal` on a `dialog` that is also a popover is no menu.
    pub fn opens_popover(&self) -> bool {
        matches!(self.action, Action::Show | Action::Toggle)
    }
}

/// What an `id` names: a view, or a run of a paragraph, an element no
/// command acts on (the Apple hosts fold runs into their paragraph's text).
enum Named<'a> {
    View(NodeRef<'a>),
    Run,
}

/// The element an `id` names: among several, a view before an inline run
/// (a `text` inside a paragraph), then the lowest node id, as the Apple hosts
/// resolve it. HTML takes the first in tree order; the two differ only for
/// duplicate ids, which HTML forbids.
fn element<'a>(kernel: &'a Kernel, name: &str) -> Option<Named<'a>> {
    let run = |node: &NodeRef<'_>| {
        node.node_type == NodeType::Text
            && node
                .parent
                .and_then(|parent| kernel.node(parent))
                .is_some_and(|p| p.node_type == NodeType::Text)
    };
    let (run, node) = kernel
        .find_by_id(name)
        .into_iter()
        .filter_map(|key| kernel.node_by_key(key))
        .map(|node| (run(&node), node))
        .min_by_key(|(run, node)| (*run, node.id))?;
    Some(if run { Named::Run } else { Named::View(node) })
}
