//! The JSON call an app makes (TypeScript's `native.call`, the web wasm's
//! `call`), routed to the client or, for the device's own operations, to the
//! host's [`Core`].
//!
//! - `{op:"open", path:"app:/data/<name>.sqlite", origin, viewer}` opens the
//!   kept partition: `{opened:true}`, or `{opened:false}` when it has never
//!   synced and the first `sync` round opens it on the server's backend.
//! - `read {name, args, now}`: a named query, answered by the device.
//! - `write {name, args, now, key?}`: admit a write and its prediction;
//!   `{id, state:"pending", newIds}` or `{id, state:"failed", why}`. With an
//!   idempotency `key` the id derives from it, and asking again answers what
//!   became of the first; `write_id {key}` names that id without writing.
//! - `persist`: save outcomes handed over with `hold`, without the network.
//! - `sync`, then `deliver {exchange, reply:{status, body}|{error}}` while the
//!   answer is `{fetch:{method, path, body?, exchange}}`; `{done:{ok, offline?,
//!   retry?, denied?}}` ends the round. `cancel {exchange}` abandons it. A
//!   reply or cancel naming another exchange is refused (`done.stale`).
//! - `changes {wait}` → `{fetch}`; `changed {exchange, reply}` → whether to sync.
//! - `refresh` → `{fetch}`; `refreshed {exchange, reply, now}` → the
//!   replacement session, or why not.
//!
//! `now` is milliseconds since the epoch, fractions allowed (floored); a
//! write requires it.
//! - `outcome {id}`, `status`, `close`.
//! - `refusals`: the writes the server refused, with their input, kept until
//!   `dismiss {ids?}` (no ids: all).
//! - `held` and `hold {held}`: the server's outcomes not yet saved, carried to
//!   a rebuilt device (the web's, after a failed save).
//!
//! Anything else is the device's own call (`query`, `state`, `meta`, …).

use crate::client::{Client, Config, Core, Reply};
use serde_json::{json, Value as Json};

fn text<'a>(request: &'a Json, key: &str) -> Result<&'a str, String> {
    request
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("Snapback4 {key} must be text"))
}

/// Answer `request`: `{ok}`, a device's `{denied}`, or the host's `Err`.
pub fn dispatch(
    client: &mut Option<Client>,
    core: &mut dyn Core,
    request: &Json,
) -> Result<Json, String> {
    let op = text(request, "op")?;
    if op == "open" {
        if request.get("backend").is_some_and(|b| !b.is_null()) {
            // A host that brings the backend itself installs it once.
            return core.call(request.clone());
        }
        // One client at a time, synced or not: a second open would leave the
        // first's caller driving the second's partition. Close first.
        if client.is_some() {
            return Err("close the current Snapback4 partition before opening another".into());
        }
        let mut opening = Client::new(Config {
            path: text(request, "path")?.into(),
            origin: text(request, "origin")?.into(),
            viewer: text(request, "viewer")?.into(),
        });
        let opened = opening.open(core)?;
        *client = Some(opening);
        return Ok(json!({"ok": {"opened": opened}}));
    }
    if op == "close" {
        *client = None;
        return core.call(request.clone());
    }
    let Some(client) = client.as_mut() else {
        return core.call(request.clone());
    };
    let now = |required: bool| -> Result<i64, String> {
        match request.get("now").and_then(Json::as_f64) {
            Some(now) if (0.0..9.007_199_254_740_991e15).contains(&now) => Ok(now.floor() as i64),
            None if !required => Ok(0),
            _ => Err("Snapback4 now must be milliseconds since the epoch".into()),
        }
    };
    let exchange = || text(request, "exchange");
    let args = || request.get("args").cloned().unwrap_or_else(|| json!({}));
    let reply = |key: &str| {
        Reply::from_json(
            request
                .get(key)
                .ok_or_else(|| format!("{op} needs {key}"))?,
        )
    };
    let answer = match op {
        "read" => client.read(core, text(request, "name")?, args(), now(false)?)?,
        "write" => {
            let key = request.get("key").and_then(Json::as_str);
            client.write(core, text(request, "name")?, args(), now(true)?, key)?
        }
        "write_id" => json!(client.write_id(core, text(request, "key")?)?),
        "persist" => client.persist(core),
        "outcome" => client.outcome(core, text(request, "id")?)?,
        "status" => client.status(core)?,
        "refusals" => client.refusals(core)?,
        "dismiss" => {
            let ids: Option<Vec<String>> = request.get("ids").and_then(Json::as_array).map(|ids| {
                ids.iter()
                    .filter_map(Json::as_str)
                    .map(str::to_owned)
                    .collect()
            });
            client.dismiss(core, ids.as_deref())?;
            Json::Null
        }
        "held" => client.held(),
        "hold" => {
            client.hold(request.get("held").unwrap_or(&Json::Null));
            Json::Null
        }
        "sync" => client.sync(core).to_json(),
        "deliver" => client.deliver(core, exchange()?, reply("reply")?).to_json(),
        "cancel" => json!(client.cancel(exchange()?)),
        "changes" => {
            let wait = request.get("wait").and_then(Json::as_u64).unwrap_or(25) as u32;
            json!({"fetch": client.changes(core, wait)?.to_json()})
        }
        "changed" => json!(client.changed(core, exchange()?, reply("reply")?)?),
        "refresh" => json!({"fetch": client.refresh()?.to_json()}),
        "refreshed" => client.refreshed(exchange()?, reply("reply")?, now(true)?),
        _ => return core.call(request.clone()),
    };
    Ok(json!({"ok": answer}))
}
