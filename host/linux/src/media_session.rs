//! @ref LLP 1098 D9 — the media session on a host with no player: the
//! record, never a publication. `state.mediaSession` reads the claimants
//! (`audio`/`video` rows with `metadata=`) from their kernel props and the
//! runner's handlers; nothing plays, so the owner is the most recently
//! mounted claimant (D5's second rule), its playback state `"paused"`, and
//! no position. `tap … mediasession` dispatches one of the six to the owner
//! through the runner's media event path, never as a press. Windows's agent
//! mode is this presenter and does the same.
//!
//! Mount order is recorded as commits land ([`Mounts::commit`], from the
//! host's commit path), not when the agent asks: by then every claimant
//! mounted since the last request would look alike. It is keyed by
//! `NodeKey`, since a freed wire id is reused.

use crate::presenter::Presenter;
use exact_kernel::{CommitReceipt, Kernel, NodeKey, NodeRef, NodeType, PropId, PropValue, ViewId};
use exact_runner::{DataSource, Event};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The six actions, by `setActionHandler`'s names (LLP 1098 D2).
const ACTIONS: [&str; 6] = [
    "seekbackward",
    "seekforward",
    "seekto",
    "previoustrack",
    "nexttrack",
    "stop",
];

/// The commit each claimant mounted in, by key.
#[derive(Debug, Default)]
pub(crate) struct Mounts {
    commits: u64,
    first: BTreeMap<NodeKey, u64>,
}

impl Mounts {
    /// One commit: its new claimants take its number; its removed nodes go.
    pub(crate) fn commit(&mut self, kernel: &Kernel, receipt: &CommitReceipt) {
        self.commits += 1;
        for key in &receipt.destroyed {
            self.first.remove(key);
        }
        // A renewed node is a new mount too (LLP 1078): the key stays, the
        // mount does not.
        for key in receipt.created.iter().chain(receipt.renewed.iter()) {
            if kernel.node_by_key(*key).is_some_and(claims) {
                self.first.insert(*key, self.commits);
            }
        }
    }
}

/// Whether a node claims the session: a media node with `metadata=`.
fn claims(node: NodeRef<'_>) -> bool {
    node.node_type == NodeType::Video && node.props.contains(PropId::MediaTitle)
}

/// A seek's offset: the element's, else the default 10 s (D2).
fn offset(node: NodeRef<'_>, prop: PropId) -> f64 {
    match node.props.get(prop) {
        Some(&PropValue::Float(s)) if s.is_finite() && s > 0.0 => s,
        _ => 10.0,
    }
}

/// The claimants in document order and the owner: the latest mount, the
/// later in the tree of one commit.
fn owner<D: DataSource>(p: &Presenter<D>) -> (Vec<ViewId>, Option<ViewId>) {
    let kernel = p.host().kernel();
    let mounts = p.host().media_mounts();
    let rows = kernel.rows(None).unwrap_or_default();
    let claimants: Vec<_> = rows
        .iter()
        .filter(|r| kernel.node(r.id).is_some_and(claims))
        .map(|r| (r.id, mounts.first.get(&r.key).copied().unwrap_or(0)))
        .collect();
    let owner = claimants
        .iter()
        .enumerate()
        .max_by_key(|(order, (_, at))| (*at, *order))
        .map(|(_, (id, _))| *id);
    (claimants.into_iter().map(|(id, _)| id).collect(), owner)
}

/// The actions `id` handles, sorted.
fn handled<D: DataSource>(p: &Presenter<D>, id: ViewId) -> Vec<&'static str> {
    let kinds = p.host().runner().handlers_of(id);
    let mut names: Vec<_> = ACTIONS
        .into_iter()
        .filter(|a| kinds.iter().any(|k| k.name() == *a))
        .collect();
    names.sort_unstable();
    names
}

/// A node's testId, else its wire id: how a refusal names it.
fn named(kernel: &Kernel, id: ViewId) -> String {
    match kernel.node(id).and_then(|n| n.props.str(PropId::TestId)) {
        Some(t) => format!("\"{t}\""),
        None => format!("view {id}"),
    }
}

/// `state.mediaSession`, as every host reports it (D10).
pub(crate) fn state<D: DataSource>(p: &Presenter<D>) -> Value {
    let kernel = p.host().kernel();
    let (claimants, owner) = owner(p);
    let Some(node) = owner.and_then(|id| kernel.node(id)) else {
        return json!({"owner": null, "testId": null, "claimants": [], "metadata": null,
            "actions": [], "seekOffsets": null, "playbackState": "none", "position": null,
            "published": "none", "readback": null});
    };
    let text = |prop| node.props.str(prop).unwrap_or_default();
    json!({
        "owner": node.id,
        "testId": node.props.str(PropId::TestId),
        "claimants": claimants,
        "metadata": {"title": text(PropId::MediaTitle), "artist": text(PropId::MediaArtist),
            "album": text(PropId::MediaAlbum), "artwork": text(PropId::MediaArtwork)},
        // No `play` or `pause`: there is no player to act on.
        "actions": handled(p, node.id),
        "seekOffsets": {"seekbackward": offset(node, PropId::SeekbackwardOffset),
            "seekforward": offset(node, PropId::SeekforwardOffset)},
        "playbackState": "paused",
        "position": null,
        "published": "none",
        "readback": null,
    })
}

/// `tap <id> mediasession <action> [seconds]` (D9, D10): the action the
/// platform's handler would send, with the payload a player's host sends;
/// refused, with nothing dispatched, for a target that is not the owner or
/// an action it does not offer.
pub(crate) fn act<D: DataSource>(
    p: &mut Presenter<D>,
    id: ViewId,
    action: &str,
    seconds: Option<f64>,
) -> Result<String, String> {
    let (_, owner) = owner(p);
    let kernel = p.host().kernel();
    let Some(owner) = owner else {
        return Err(format!(
            "mediasession: there is no media session: no `audio` or `video` with `metadata=` is mounted (tapped {})",
            named(kernel, id)
        ));
    };
    if owner != id {
        return Err(format!(
            "mediasession: {} does not own the media session; {} does",
            named(kernel, id),
            named(kernel, owner)
        ));
    }
    if matches!(action, "play" | "pause") {
        return Err(format!("mediasession: {action} is unavailable: this host has no media player to act on (LLP 1098 D9)"));
    }
    if !handled(p, id).contains(&action) {
        return Err(format!(
            "mediasession: {} has no {action}",
            named(kernel, id)
        ));
    }
    let node = kernel.node(id).ok_or("mediasession: the owner is gone")?;
    let (offset, time) = match action {
        "seekbackward" => (
            seconds.unwrap_or(offset(node, PropId::SeekbackwardOffset)),
            0.0,
        ),
        "seekforward" => (
            seconds.unwrap_or(offset(node, PropId::SeekforwardOffset)),
            0.0,
        ),
        "seekto" => (
            0.0,
            seconds.ok_or("mediasession: seekto needs the seconds to seek to")?,
        ),
        _ => (0.0, 0.0),
    };
    if !(offset.is_finite() && offset >= 0.0 && time.is_finite() && time >= 0.0) {
        return Err("mediasession: seconds are a finite number of 0 or more".into());
    }
    let wire = format!("{action}\n{offset} {time} 0");
    let event = Event::media_payload(&wire).ok_or("mediasession: invalid media event")?;
    let now = p.host().now();
    if let Some(e) = p.host.dispatch_at(id, event, now) {
        return Err(e);
    }
    if let Some(e) = p.after_commit() {
        return Err(e);
    }
    Ok(
        json!({"tapped": id, "mediaSession": action, "seekOffset": offset, "seekTime": time,
        "delivery": "substituted"})
        .to_string(),
    )
}
