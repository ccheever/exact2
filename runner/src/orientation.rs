//! `orientationlock` (LLP 1133.029): while an element that is mounted and
//! displayed says so, the session's window keeps to the orientations it
//! names, the Screen Orientation API's lock types and the shared word
//! `current`. Displayed is `wakelock`'s rule ([`crate::wakelock::displayed`]),
//! and a holder inside a `dialog` or popover never counts: whether a
//! top-layer box is open is the host's state, so a lock there is refused at
//! bake and, where the bake could not see it, journaled here (D3).
//!
//! Of the holders, three rules in order decide which count (D3): the
//! topmost sheet that holds wins over the routes beneath it; a holder with
//! a holder among its descendants is overridden by it; and the holders left
//! intersect. The runner intersects the types that need no device and finds
//! a conflict among them; the host resolves `natural` and `current`, bounds
//! the types by what its window can take, and reports what it applied
//! ([`Runner::set_orientation_applied`]), which the agent reads as
//! `state.orientationlock`.

use crate::{DataSource, Runner};
use exact_kernel::{Kernel, PropId, ViewId};
use std::fmt::Write as _;

/// Every word the attribute takes (D2), as lowering checks a literal.
pub const WORDS: [&str; 10] = [
    "portrait",
    "landscape",
    "portrait-primary",
    "portrait-secondary",
    "landscape-primary",
    "landscape-secondary",
    "natural",
    "any",
    "current",
    "none",
];

/// The four orientations as bits, the Screen Orientation API's
/// `OrientationType` in this order: what a lock's `types` holds.
pub const ORIENTATIONS: [&str; 4] = [
    "portrait-primary",
    "portrait-secondary",
    "landscape-primary",
    "landscape-secondary",
];

/// All four orientations: `any`, and a lock that names no type.
pub const ALL: u8 = 0b1111;

/// The orientations a type names; `None` for `natural` and `current`, which
/// only the host can resolve, and for `none`.
pub fn types(word: &str) -> Option<u8> {
    Some(match word {
        "portrait" => 0b0011,
        "landscape" => 0b1100,
        "any" => ALL,
        _ => 1 << ORIENTATIONS.iter().position(|o| *o == word)?,
    })
}

/// The session's lock as the host is told it (D3): the intersection of the
/// counted holders' types, and whether one says `natural` or `current`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    /// The orientations the types allow; [`ALL`] when none names one.
    pub types: u8,
    /// A holder says `natural`.
    pub natural: bool,
    /// A holder says `current`.
    pub current: bool,
    /// Each counted holder and its word, in tree order, which a conflict
    /// line the host writes names.
    pub by: Vec<(ViewId, String)>,
}

impl Lock {
    /// `{"types":…,"orientations":[…],"natural":…,"current":…,"by":[[view,"word"],…]}`:
    /// the batch field and the agent's `state.orientationlock.lock`.
    pub fn json(&self) -> String {
        let mut s = format!("{{\"types\":{},\"orientations\":[", self.types);
        let names = ORIENTATIONS.iter().enumerate();
        let names: Vec<String> = names
            .filter(|(i, _)| self.types & (1 << i) != 0)
            .map(|(_, o)| format!("\"{o}\""))
            .collect();
        s.push_str(&names.join(","));
        let _ = write!(
            s,
            "],\"natural\":{},\"current\":{},\"by\":",
            self.natural, self.current
        );
        holders_json(&self.by, &mut s);
        s.push('}');
        s
    }
}

/// What the session holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Session {
    /// No displayed element holds a lock: the manifest's orientation applies.
    None,
    /// The lock the host applies.
    Held(Lock),
    /// The counted holders' types share no orientation: none is held.
    Conflict(Vec<(ViewId, String)>),
}

impl Session {
    /// The lock, where one is held.
    pub fn lock(&self) -> Option<&Lock> {
        match self {
            Session::Held(lock) => Some(lock),
            _ => None,
        }
    }
}

/// Where a lock that cannot count was written: inside a `dialog` or popover.
fn in_top_layer(kernel: &Kernel, id: ViewId) -> bool {
    let mut at = Some(id);
    while let Some(node) = at.and_then(|id| kernel.node(id)) {
        let props = node.props;
        if props.str(PropId::Popover).is_some() || props.str(PropId::SemanticTag) == Some("dialog")
        {
            return true;
        }
        at = node.parent;
    }
    false
}

/// Every element that says a word other than `none`, with its word.
fn declared(kernel: &Kernel) -> Vec<(ViewId, String)> {
    if !kernel.has_prop(PropId::Orientationlock) {
        return Vec::new();
    }
    let ids = kernel.preorder_where(&kernel.roots(), |_, props| {
        props
            .str(PropId::Orientationlock)
            .is_some_and(|w| w != "none")
    });
    ids.into_iter()
        .filter_map(|id| {
            let word = kernel.node(id)?.props.str(PropId::Orientationlock)?;
            Some((id, word.to_owned()))
        })
        .collect()
}

/// The session's lock under D3's three rules.
pub fn session<D: DataSource>(runner: &Runner<D>) -> Session {
    let (kernel, open) = (runner.kernel(), runner.top_layer_open());
    let holders: Vec<(ViewId, String)> = declared(kernel)
        .into_iter()
        .filter(|(id, _)| {
            !in_top_layer(kernel, *id) && crate::wakelock::displayed(kernel, open, *id)
        })
        .collect();
    let by = counted(kernel, holders);
    if by.is_empty() {
        return Session::None;
    }
    let types = by
        .iter()
        .fold(ALL, |set, (_, word)| set & types(word).unwrap_or(ALL));
    if types == 0 {
        return Session::Conflict(by);
    }
    Session::Held(Lock {
        types,
        natural: by.iter().any(|(_, w)| w == "natural"),
        current: by.iter().any(|(_, w)| w == "current"),
        by,
    })
}

/// The holders that count, of those displayed (D3 rules 1 and 2).
fn counted(kernel: &Kernel, holders: Vec<(ViewId, String)>) -> Vec<(ViewId, String)> {
    // Rule 1: in each stack, a holder counts only in the topmost route that
    // has one, the page and the split's list beside it being one tier.
    let tiers: Vec<Vec<(ViewId, usize)>> =
        holders.iter().map(|(id, _)| tiers(kernel, *id)).collect();
    let top = |stack: ViewId| {
        tiers
            .iter()
            .flatten()
            .filter(|(s, _)| *s == stack)
            .map(|(_, tier)| *tier)
            .max()
    };
    let kept: Vec<(ViewId, String)> = holders
        .into_iter()
        .zip(&tiers)
        .filter(|(_, at)| at.iter().all(|(stack, tier)| top(*stack) == Some(*tier)))
        .map(|(holder, _)| holder)
        .collect();
    // Rule 2: a holder with a holder among its descendants is overridden.
    kept.iter()
        .filter(|(id, _)| {
            !kept
                .iter()
                .any(|(other, _)| other != id && above(kernel, *id, *other))
        })
        .cloned()
        .collect()
}

/// Whether `ancestor` is a proper ancestor of `id`.
fn above(kernel: &Kernel, ancestor: ViewId, id: ViewId) -> bool {
    let mut at = kernel.node(id).and_then(|n| n.parent);
    while let Some(parent) = at {
        if parent == ancestor {
            return true;
        }
        at = kernel.node(parent).and_then(|n| n.parent);
    }
    false
}

/// Each stack `id` lies in, by the node holding its routes, with the tier of
/// the route it lies under: its index, or the page's for the page and the
/// routes beneath it that show (a split's list), where the page is the last
/// route up to the selected one that is not a sheet.
fn tiers(kernel: &Kernel, id: ViewId) -> Vec<(ViewId, usize)> {
    let key = |id: ViewId| {
        kernel
            .node(id)
            .and_then(|n| n.props.str(PropId::NavigationKey))
    };
    let sheet = |id: ViewId| {
        kernel
            .node(id)
            .and_then(|n| n.props.str(PropId::NavigationPresentation))
            == Some("modal")
    };
    let mut out = Vec::new();
    let mut at = Some(id);
    while let Some(node) = at.and_then(|id| kernel.node(id)) {
        let parent = node.parent;
        if let (Some(_), Some(holder)) = (key(node.id), parent.and_then(|p| kernel.node(p))) {
            let stack: Vec<ViewId> = holder
                .children()
                .into_iter()
                .filter(|r| key(*r).is_some())
                .collect();
            // The navigation root: the holder, or the nearest ancestor of a
            // tab panel that names a key and a way back.
            let mut root = Some(holder.id);
            let mut selected = None;
            while let Some(r) = root.and_then(|r| kernel.node(r)) {
                let props = r.props;
                if let (Some(k), Some(_)) = (
                    props.str(PropId::NavigationKey),
                    props.str(PropId::NavigationBack),
                ) {
                    selected = stack.iter().position(|s| key(*s) == Some(k));
                    break;
                }
                root = r.parent;
            }
            let index = stack.iter().position(|r| *r == node.id);
            if let (Some(selected), Some(index)) = (selected, index) {
                let page = (0..=selected)
                    .rev()
                    .find(|i| !sheet(stack[*i]))
                    .unwrap_or(0);
                out.push((holder.id, index.max(page)));
            }
        }
        at = parent;
    }
    out
}

/// `[[view,"word"],…]`.
fn holders_json(by: &[(ViewId, String)], s: &mut String) {
    s.push('[');
    for (i, (id, word)) in by.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "[{id},\"{word}\"]");
    }
    s.push(']');
}

/// What the runner keeps of the lock between commits: the host's readback,
/// and what it has journaled.
#[derive(Debug, Default)]
pub struct Said {
    /// The host's readback, JSON (D5–D8); `None` until a host reports.
    applied: Option<String>,
    /// The host found a conflict only its resolution reveals.
    conflict: bool,
    /// The last conflict journaled.
    journaled: Option<Vec<(ViewId, String)>>,
    /// Locks inside a `dialog` or popover already journaled.
    top_layer: Vec<ViewId>,
}

impl<D: DataSource> Runner<D> {
    /// The session's lock, for a host to apply when it changes (D3), with
    /// the journal lines its changes earn: a conflict among the types, once
    /// per conflict, and a lock inside a `dialog` or popover, once per view.
    pub fn orientation_session(&mut self) -> Session {
        let session = session(self);
        let kernel = self.kernel();
        let boxed: Vec<ViewId> = declared(kernel)
            .into_iter()
            .map(|(id, _)| id)
            .filter(|id| in_top_layer(kernel, *id))
            .collect();
        let mut lines = Vec::new();
        for id in boxed {
            if !self.orientation.top_layer.contains(&id) {
                self.orientation.top_layer.push(id);
                lines.push(format!(
                    "orientationlock: view {id} is inside a dialog or popover; it holds nothing"
                ));
            }
        }
        let conflict = match &session {
            Session::Conflict(by) => Some(by.clone()),
            _ => None,
        };
        if conflict != self.orientation.journaled {
            if let Some(by) = &conflict {
                lines.push(conflict_line(by));
            }
            self.orientation.journaled = conflict;
        }
        for line in lines {
            self.log(line);
        }
        session
    }

    /// What the host applied (D5–D8), JSON, for `state.orientationlock.applied`;
    /// `conflict` when its resolution of `natural` or `current` left the
    /// counted holders sharing no orientation, which it journals itself.
    pub fn set_orientation_applied(&mut self, applied: String, conflict: bool) {
        self.orientation.applied = Some(applied);
        self.orientation.conflict = conflict;
    }
}

/// `orientationlock: conflict: view 12 "portrait", view 30 "landscape"; none held`:
/// the line either side journals.
pub fn conflict_line(by: &[(ViewId, String)]) -> String {
    let named: Vec<String> = by
        .iter()
        .map(|(id, word)| format!("view {id} \"{word}\""))
        .collect();
    format!("orientationlock: conflict: {}; none held", named.join(", "))
}

/// `,"orientationlock":{"held":…,"by":[…],"words":[…],"lock":…,"conflict":…,"applied":…}`,
/// the agent's `state.orientationlock` (§8).
pub(crate) fn state<D: DataSource>(runner: &Runner<D>, s: &mut String) {
    let session = session(runner);
    let said = &runner.orientation;
    let (by, lock, conflict): (&[(ViewId, String)], _, _) = match &session {
        Session::None => (&[], None, None),
        Session::Held(lock) => (&lock.by, Some(lock), said.conflict.then_some(&lock.by)),
        Session::Conflict(by) => (by, None, Some(by)),
    };
    let held = lock.is_some() && conflict.is_none();
    let _ = write!(s, ",\"orientationlock\":{{\"held\":{held},\"by\":[");
    let ids: Vec<String> = by.iter().map(|(id, _)| id.to_string()).collect();
    s.push_str(&ids.join(","));
    s.push_str("],\"words\":[");
    let words: Vec<String> = by.iter().map(|(_, w)| format!("\"{w}\"")).collect();
    s.push_str(&words.join(","));
    s.push_str("],\"lock\":");
    s.push_str(&lock.map_or("null".into(), Lock::json));
    s.push_str(",\"conflict\":");
    match conflict {
        Some(by) => holders_json(by, s),
        None => s.push_str("null"),
    }
    s.push_str(",\"applied\":");
    s.push_str(said.applied.as_deref().unwrap_or("\"unavailable\""));
    s.push('}');
}

#[cfg(test)]
#[path = "orientation_tests.rs"]
mod tests;
