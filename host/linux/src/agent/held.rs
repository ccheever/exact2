//! Requests the server holds (LLP 1016.000 D9) in this host's agent replies:
//! each clock reply that left one out names it, and the tree lists what is
//! still out, held asks marked, as Apple's does.
use super::*;

/// `reply` naming the requests the server holds, which no clock waited on
/// (LLP 1016.000 D9): `held` and `heldTargets`, as the web hosts' do. An
/// error reply is left as it is.
pub(super) fn held_named<D: DataSource>(p: &Presenter<D>, mut reply: String) -> String {
    let held = p.host().runner().held_requests();
    if held.is_empty() || reply.starts_with("{\"error\"") || !reply.ends_with('}') {
        return reply;
    }
    let targets: Vec<&str> = held.iter().map(|(name, _)| name.as_str()).collect();
    reply.pop();
    reply.push_str(&format!(
        ",\"held\":{},\"heldTargets\":{}}}",
        held.len(),
        serde_json::json!(targets)
    ));
    reply
}

/// The tree's `pending`: the requests still in flight and those the server
/// holds (`held: true`), for a missed target's note (agent-held.mjs).
pub(super) fn name_pending<D: DataSource>(p: &Presenter<D>, tree: &mut serde_json::Value) {
    if tree.is_object() {
        let runner = p.host().runner();
        let ask = |(name, ticket): (String, u64), held: bool| {
            let mut row = serde_json::json!({"name": name, "ticket": ticket});
            if held {
                row["held"] = true.into();
            }
            row
        };
        let mut pending: Vec<serde_json::Value> = runner
            .in_flight()
            .into_iter()
            .map(|r| ask(r, false))
            .collect();
        pending.extend(runner.held_requests().into_iter().map(|r| ask(r, true)));
        tree["pending"] = pending.into();
    }
}
