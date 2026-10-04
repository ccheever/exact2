//! Shared-element handoffs (LLP 1013.000 D3): a `sharedElement` name that
//! leaves one node and arrives at another in one commit.
//!
//! A name leaves with the node the commit destroys, directly or with an
//! ancestor; it arrives with a node the commit creates. A name that leaves
//! and arrives exactly once pairs. Two leavers or two arrivers pair nothing:
//! which would fly is not the kernel's guess. Names need not be unique in
//! the tree, since a pair is a change, not a lookup: two mounted routes can
//! hold the same name.

use crate::arena::NodeArena;
use crate::id::NodeKey;
use crate::PropId;
use exact_motion::{Property, Transition};

/// One name handed from a destroyed node to a created one.
#[derive(Debug, Clone, PartialEq)]
pub struct Handoff {
    /// The `sharedElement` name.
    pub name: String,
    /// The destroyed node; it resolves to nothing after the commit.
    pub from: NodeKey,
    /// The created node, live after the commit.
    pub to: NodeKey,
    /// The arriver's `layout-transition`, else the leaver's (D2): the curve
    /// the flight runs on. A handoff with neither is not reported.
    pub transition: Transition,
}

/// A node leaving with its name, recorded at its destroy.
#[derive(Debug)]
pub(crate) struct Leaver {
    name: String,
    key: NodeKey,
    transition: Option<Transition>,
}

/// The leaver a destroyed slot is, if it carries a name.
pub(crate) fn leaver(arena: &NodeArena, slot: u32) -> Option<Leaver> {
    let name = arena.props(slot).str(PropId::SharedElement)?;
    if name.is_empty() {
        return None;
    }
    Some(Leaver {
        name: name.to_owned(),
        key: arena.key(slot),
        transition: layout_curve(arena, slot),
    })
}

fn layout_curve(arena: &NodeArena, slot: u32) -> Option<Transition> {
    arena
        .style(slot)
        .layout_transition
        .matching(Property::Layout)
        .filter(|t| t.starts())
        .cloned()
}

/// Pair the commit's leavers with the named nodes it created.
pub(crate) fn pair(arena: &NodeArena, created: &[NodeKey], leavers: Vec<Leaver>) -> Vec<Handoff> {
    if leavers.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, leaver) in leavers.iter().enumerate() {
        if leavers
            .iter()
            .enumerate()
            .any(|(j, other)| i != j && other.name == leaver.name)
        {
            continue;
        }
        let mut arrivers = created.iter().filter_map(|key| {
            let slot = arena.resolve(*key)?;
            (arena.props(slot).str(PropId::SharedElement) == Some(leaver.name.as_str()))
                .then_some((*key, slot))
        });
        let (Some((to, slot)), None) = (arrivers.next(), arrivers.next()) else {
            continue;
        };
        let Some(transition) = layout_curve(arena, slot).or_else(|| leaver.transition.clone())
        else {
            continue;
        };
        out.push(Handoff {
            name: leaver.name.clone(),
            from: leaver.key,
            to,
            transition,
        });
    }
    out
}
