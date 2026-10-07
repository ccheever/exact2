//! One round: open, sync to the head, send the outbox, sync again.
//!
//! A transcription of Snapback 4's TypeScript client (`local.ts` `syncRound`
//! and `flushOnce`, `local-receipts.ts`, 0.4.13) without its watches, media,
//! following, ephemeral reads, native jobs or timers: the device keeps the
//! protocol's state (`sync_state`), and this carries it between the device
//! and the wire. Where this differs it says so.

use crate::client::{
    open_request, refusal, Config, Fetch, Io, Refusal, Reply, Shared, Unpersisted, NEEDS_BACKEND,
};
use serde_json::{json, Value as Json};
use std::sync::{Arc, Mutex};

/// Rows a sync page may carry; the server's own bound cuts a page sooner.
const PAGE: u64 = 10_000;
/// More pages than any real partition; a server that never ends is refused.
const PAGES: usize = 100_000;

/// The device's terminal write history (`keep_write`'s; the device keeps
/// it under `client:write-history`, and its `meta` adds the prefix).
pub(crate) const HISTORY: &str = "write-history";

/// Record a refused write in the journal (once per id), before its entry —
/// and with it the input — is retired. The journal's layout is `journal.rs`'s.
async fn journal(io: &Io, entry: &Json, outcome: &Json) -> Result<()> {
    use crate::journal::{fits, index, index_text, segment, segment_key, stored, INDEX};
    let id = entry["id"].as_str().unwrap_or_default();
    let read = |key: String| io.device(json!({"op": "meta", "key": key}));
    let mut segments = index(&read(INDEX.into()).await.map_err(denied)?);
    let mut last = Vec::new();
    for n in &segments {
        last = segment(&read(segment_key(*n)).await.map_err(denied)?);
        if last.iter().any(|known| known["id"] == id) {
            return Ok(());
        }
    }
    let op = entry["op"].as_str().unwrap_or_default();
    let refused = stored(json!({"id": id, "op": op, "args": entry["args"],
        "input": crate::client::input_key(op, &entry["args"]), "why": outcome["why"], "at": entry["now"]}));
    let n = match segments.last() {
        Some(n) if fits(&last, &refused) => *n,
        _ => {
            last = Vec::new();
            segments.iter().max().map_or(0, |n| n + 1)
        }
    };
    last.push(refused);
    io.device(
        json!({"op": "set_meta", "key": segment_key(n), "value": Json::Array(last).to_string()}),
    )
    .await
    .map_err(denied)?;
    if !segments.contains(&n) {
        segments.push(n);
        io.device(json!({"op": "set_meta", "key": INDEX, "value": index_text(&segments)}))
            .await
            .map_err(denied)?;
    }
    Ok(())
}

/// Why a round stopped short.
enum Stop {
    /// The server was not reached, or answered as an outage.
    Offline(Refusal),
    /// The server asked to be asked again (a retryable refusal): nothing is
    /// settled and nothing is latched; the next round tries again.
    Retry(Refusal),
    /// The server or the device refused; a later round may still succeed.
    Denied(Refusal),
}

type Result<T> = std::result::Result<T, Stop>;

fn denied(why: Refusal) -> Stop {
    Stop::Denied(why)
}

pub(crate) async fn round(io: Io, config: Arc<Config>, shared: Arc<Mutex<Shared>>) -> Json {
    let outcome = run(&io, &config, &shared).await;
    let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
    match outcome {
        Ok(()) => {
            shared.online = Some(true);
            shared.denied = None;
            json!({"ok": true})
        }
        Err(Stop::Offline(why)) => {
            shared.online = Some(false);
            json!({"ok": false, "offline": true, "denied": why})
        }
        Err(Stop::Retry(why)) => {
            shared.online = Some(true);
            json!({"ok": false, "retry": true, "denied": why})
        }
        Err(Stop::Denied(why)) => {
            shared.online = Some(true);
            shared.denied = Some(why.clone());
            json!({"ok": false, "offline": false, "denied": why})
        }
    }
}

/// Save what the client holds (outcomes taken over from another device),
/// without the network: keep each receipt, journal each refusal, retire each
/// entry. It never fetches, so it runs to completion in one call.
pub(crate) async fn persist_round(io: Io, shared: Arc<Mutex<Shared>>) -> Json {
    match persist_pending(&io, &shared).await {
        Ok(()) => json!({"ok": true}),
        Err(Stop::Offline(why) | Stop::Retry(why) | Stop::Denied(why)) => {
            json!({"ok": false, "denied": why})
        }
    }
}

async fn run(io: &Io, config: &Config, shared: &Mutex<Shared>) -> Result<()> {
    open(io, config, shared).await?;
    // Receipts the server gave come first: before a page can retire or
    // replace their entries, and before anything is sent again.
    persist_pending(io, shared).await?;
    sync(io, shared).await?;
    if flush(io, shared).await? {
        // A settled send is observed by the next page; catch up to it now.
        sync(io, shared).await?;
    }
    Ok(())
}

fn bump(shared: &Mutex<Shared>) {
    shared.lock().unwrap_or_else(|p| p.into_inner()).revision += 1;
}

/// Open on the kept partition, or once on the server's backend
/// (`openLocalDevice`: a conditional first install, never a replacement).
async fn open(io: &Io, config: &Config, shared: &Mutex<Shared>) -> Result<()> {
    if shared.lock().unwrap_or_else(|p| p.into_inner()).opened {
        return Ok(());
    }
    let kept = io.raw(open_request(config, None)).await;
    match kept {
        Ok(answer) if answer.get("denied").is_none() => {}
        Err(error) if error == NEEDS_BACKEND => {
            let backend = load_backend(io).await?;
            let answer = io
                .raw(open_request(
                    config,
                    Some(json!({"initial_backend": backend})),
                ))
                .await;
            match answer {
                Ok(answer) if answer.get("denied").is_none() => {}
                Ok(answer) => return Err(denied(answer["denied"].clone())),
                Err(error) => return Err(denied(refusal("E_STORE", "store", error))),
            }
        }
        Ok(answer) => return Err(denied(answer["denied"].clone())),
        Err(error) => return Err(denied(refusal("E_STORE", "store", error))),
    }
    let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
    shared.opened = true;
    shared.revision += 1;
    Ok(())
}

/// A refusal as the server words one: nonempty `code`, `family` and
/// `message`, a boolean `retryable` if any (`http-response.ts` `refusal`).
fn refusal_shape(why: &Json) -> bool {
    ["code", "family", "message"]
        .iter()
        .all(|key| why[key].as_str().is_some_and(|text| !text.is_empty()))
        && (why.get("retryable").is_none() || why["retryable"].is_boolean())
}

/// What a reply says whether it came from the server or not
/// (`readResponse`): a body that is not a JSON object, a refusal that is not
/// one, or an error status without a refusal is `E_HTTP_RESPONSE`,
/// retryable: the outcome is unknown, never a refusal to settle.
fn read(reply: Reply) -> Result<(u16, Json, Option<Refusal>)> {
    let (status, body) = match reply {
        Reply::Failed(error) => {
            return Err(Stop::Offline(json!({"code": "E_OFFLINE", "family": "link",
                "message": format!("the request did not complete: {error}"), "retryable": true})))
        }
        Reply::Http { status, body } => (status, body),
    };
    let why = body.get("denied").or_else(|| body.get("why")).cloned();
    let valid = body.is_object()
        && why.as_ref().is_none_or(refusal_shape)
        && ((200..300).contains(&status) || why.is_some());
    if !valid {
        let invalid = json!({"code": "E_HTTP_RESPONSE", "family": "link", "retryable": true,
            "message": format!("the server answered HTTP {status} without a valid response; the outcome is unknown")});
        return Err(if status >= 500 {
            Stop::Offline(invalid)
        } else {
            Stop::Retry(invalid)
        });
    }
    if status >= 500
        && why
            .as_ref()
            .is_some_and(|why| why["code"] == "E_HTTP_RESPONSE")
    {
        return Err(Stop::Offline(why.unwrap()));
    }
    Ok((status, body, why))
}

/// Stop on a server refusal: retryable ones ask again next round.
fn refused(why: Refusal) -> Stop {
    if why["retryable"] == true {
        Stop::Retry(why)
    } else {
        Stop::Denied(why)
    }
}

/// `loadBackend`: `GET /schema`.
async fn load_backend(io: &Io) -> Result<Json> {
    let (_, body, why) = read(io.fetch(Fetch::get("/schema".into())).await)?;
    match why {
        Some(why) => Err(refused(why)),
        None => Ok(body),
    }
}

fn too_large(why: &Json) -> bool {
    why["code"] == "E_BOUND"
        && why["family"] == "bound"
        && why["message"].as_str().is_some_and(|m| {
            m.starts_with("the HTTP JSON response exceeds 16 MiB;") && m.contains("smaller page")
        })
}

fn uint(value: &Json) -> u64 {
    value.as_u64().unwrap_or(0)
}

/// `syncRound`: page from the device's watermark to the head, observing the
/// store's identity on every page and adopting a new backend when the
/// generation or the store changes.
async fn sync(io: &Io, shared: &Mutex<Shared>) -> Result<()> {
    let mut generation =
        io.device(json!({"op": "state"})).await.map_err(denied)?["generation"].clone();
    let mut after: Option<Json> = None;
    let mut catching_up = false;
    let mut publication_outstanding = false;
    let mut restarting_snapshot = false;
    let mut page_limit = PAGE;
    for _ in 0..PAGES {
        let captured = io
            .device(json!({"op": "sync_state"}))
            .await
            .map_err(denied)?;
        if let Some(why) = captured.get("restore_refusal").filter(|why| !why.is_null()) {
            return Err(denied(why.clone()));
        }
        let mut backend_required = captured["backend_required"].as_bool().unwrap_or(false);
        let requested_store = captured
            .get("transfer_store_id")
            .filter(|v| !v.is_null())
            .or_else(|| captured.get("store_id"))
            .cloned()
            .unwrap_or(Json::Null);
        let mut published_store = captured.get("store_id").cloned().unwrap_or(Json::Null);
        let snapshot_request = restarting_snapshot || after.is_some();
        let captured_watermark = uint(&captured["watermark"]);
        let watermark = if snapshot_request {
            0
        } else {
            captured
                .get("transfer_watermark")
                .and_then(Json::as_u64)
                .unwrap_or(captured_watermark)
        };
        catching_up = !snapshot_request
            && captured["snapshot_catchup"]
                .as_bool()
                .unwrap_or(catching_up);
        publication_outstanding |= watermark > captured_watermark;
        let mut body = json!({"generation": captured["generation"], "from": watermark, "limit": page_limit,
            "after": after.clone().unwrap_or(Json::Null),
            "stream": !snapshot_request && (catching_up || (after.is_none() && publication_outstanding)),
            "pending": captured["pending_ids"]});
        if requested_store.is_string() {
            body["store_id"] = requested_store;
        }
        let (_, mut page, _) = read(io.fetch(Fetch::post("/sync".into(), body)).await)?;
        for (field, value) in [
            ("send_revision", &captured["send_revision"]),
            ("pending_through", &captured["pending_through"]),
            ("pending_complete", &captured["pending_complete"]),
            ("pending_bound", &captured["pending_bound"]),
            ("captured_restore_watermark", &captured["restore_watermark"]),
            ("captured_restore_images_at", &captured["restore_images_at"]),
        ] {
            page[field] = value.clone();
        }
        page["requested_watermark"] = json!(watermark);
        page["captured_watermark"] = json!(captured_watermark);
        let refused = page.get("denied").filter(|why| why.is_object()).cloned();
        let generation_changed = refused
            .as_ref()
            .is_some_and(|why| why["code"] == "E_SYNC_GENERATION" && why["family"] == "generation");
        if generation_changed && page["watermark"].as_u64().is_none() {
            return Err(Stop::Offline(refused.unwrap()));
        }
        if let Some(why) = &refused {
            if !generation_changed && !too_large(why) {
                return Err(self::refused(why.clone()));
            }
        }
        // Refusals deliver no rows; an overflow watermark is only an event cut.
        let observed_page = match &refused {
            Some(_) => {
                let mut observed = page.clone();
                observed["snapshot"] = json!(false);
                observed["events"] = json!([]);
                observed["more"] = json!(true);
                if generation_changed {
                    observed["images_at"] = page["watermark"].clone();
                }
                observed
            }
            None => page.clone(),
        };
        let observation = io
            .device(json!({"op": "observe_store", "page": observed_page}))
            .await
            .map_err(denied)?;
        page["send_revision"] = observation["send_revision"].clone();
        let adoption_capture = observation
            .get("adoption_capture")
            .cloned()
            .unwrap_or(Json::Null);
        backend_required |= observation["backend_required"].as_bool().unwrap_or(false);
        let mut replaced_store = false;
        if observation["reset"].as_bool().unwrap_or(false) {
            after = None;
            catching_up = false;
            publication_outstanding = false;
            page["captured_watermark"] = json!(0);
            replaced_store = true;
            bump(shared);
            let state = io
                .device(json!({"op": "sync_state", "capture": false}))
                .await
                .map_err(denied)?;
            published_store = state.get("store_id").cloned().unwrap_or(Json::Null);
        }
        let page_generation = page
            .get("generation")
            .filter(|g| !g.is_null())
            .cloned()
            .unwrap_or(generation.clone());
        if generation_changed || page_generation != generation || replaced_store || backend_required
        {
            // A new store restarts generation numbers: reload even on a match.
            let backend = load_backend(io).await?;
            let mut adopt = json!({"op": "adopt", "backend": backend, "store_id": published_store,
                "send_revision": page["send_revision"]});
            if !adoption_capture.is_null() {
                adopt["adoption_capture"] = adoption_capture;
            }
            io.device(adopt).await.map_err(denied)?;
            bump(shared);
            generation = backend
                .get("generation")
                .cloned()
                .unwrap_or(page_generation);
            after = None;
            catching_up = false;
            publication_outstanding = false;
            continue;
        }
        if let Some(why) = refused {
            if page["snapshot"] == true && page_limit == 1 && (watermark > 0 || catching_up) {
                // Restart at the reduced limit; snapshots can split commits.
                after = None;
                catching_up = false;
                restarting_snapshot = true;
                continue;
            }
            if page_limit > 1 {
                page_limit = (page_limit / 2).max(1);
                continue;
            }
            return Err(self::refused(why));
        }
        let snapshot = page["snapshot"] == true;
        let more = page["more"] == true;
        let next = page.get("next").filter(|next| !next.is_null()).cloned();
        if uint(&page["watermark"]) < watermark
            || (catching_up && snapshot)
            || (snapshot && more && next.is_none())
        {
            return Err(denied(refusal(
                "E_SYNC",
                "link",
                "the server's page does not continue this partition",
            )));
        }
        page["stage_snapshot"] = json!(snapshot);
        page["snapshot_catchup"] = json!(catching_up);
        let first = !catching_up && after.is_none();
        let applied = io
            .device(json!({"op": "apply", "page": page, "first": first}))
            .await
            .map_err(denied)?;
        restarting_snapshot = false;
        if applied["touched"]
            .as_array()
            .is_none_or(|touched| !touched.is_empty())
        {
            bump(shared);
        }
        let published = applied["published"] != false;
        publication_outstanding = !published;
        if snapshot && more {
            after = next;
            continue;
        }
        after = None;
        if snapshot {
            catching_up = true;
            continue;
        }
        if !more && published {
            return Ok(());
        }
    }
    Err(denied(refusal(
        "E_SYNC",
        "link",
        "the server's pages did not end",
    )))
}

/// A receipt as kept (`writes.ts` `receipt`): execution confirmed, never the
/// historical result value.
fn receipt(id: &str, outcome: &Json) -> Json {
    if outcome["state"] == "sent" {
        json!({"state": "sent", "id": id, "seq": outcome["seq"], "replayed": true})
    } else {
        let why = &outcome["why"];
        let code = why["code"].as_str().unwrap_or("E_STORE");
        let message = if matches!(code, "E_STORE_REPLACED" | "E_STORE_RESTORE") {
            format!(
                "write refused: {code}; whether it executed is unknown; do not retry automatically"
            )
        } else {
            format!("write refused: {code}")
        };
        json!({"state": "failed", "id": id, "why": {"code": code, "family": why["family"], "message": message}})
    }
}

/// `settleReceipt`: hold the outcome, keep its receipt, then retire the
/// entry (a success keeps its rows until the stream confirms them; a refusal
/// withdraws them). The outcome is held in memory first, so a failed save
/// never forgets what the server said: the next round persists it before
/// anything else.
async fn settle(
    io: &Io,
    shared: &Mutex<Shared>,
    id: &str,
    outcome: Json,
    revalidated: Option<&Json>,
) -> Result<()> {
    {
        let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
        // The ring keeps the store binding too, for a host that rebuilds.
        let mut remembered = outcome.clone();
        if let Some(store) = revalidated {
            remembered["revalidated_store"] = store.clone();
        }
        shared.finish(id, remembered);
        shared.unpersisted.retain(|held| held.id != id);
        shared.unpersisted.push(Unpersisted {
            id: id.into(),
            outcome,
            revalidated: revalidated.cloned(),
            kept: false,
        });
    }
    persist(io, shared, id).await
}

/// Keep and settle one held outcome; drop it once both are durable.
async fn persist(io: &Io, shared: &Mutex<Shared>, id: &str) -> Result<()> {
    let Some(held) = shared
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .unpersisted
        .iter()
        .find(|held| held.id == id)
        .cloned()
    else {
        return Ok(());
    };
    let entry = queued_entry(io, id).await?;
    if held.outcome["state"] == "failed" {
        if let Some(entry) = &entry {
            journal(io, entry, &held.outcome).await?;
        }
    }
    if !held.kept {
        let mut kept = receipt(id, &held.outcome);
        if let Some(store) = &held.revalidated {
            kept["revalidated_store"] = store.clone();
        }
        // What it carried, so a key reused with other input is refused for as
        // long as the device keeps this receipt.
        if let Some(entry) = &entry {
            kept["input"] = json!(crate::client::input_key(
                entry["op"].as_str().unwrap_or_default(),
                &entry["args"]
            ));
        }
        io.device(json!({"op": "keep_write", "value": kept.to_string()}))
            .await
            .map_err(denied)?;
        if let Some(held) = shared
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .unpersisted
            .iter_mut()
            .find(|held| held.id == id)
        {
            held.kept = true;
        }
    }
    // Retire the entry only if it is still the device's to retire.
    if entry.is_some() {
        let mut request = json!({"op": "settle", "id": id});
        if held.outcome["state"] == "sent" {
            request["seq"] = held.outcome["seq"].clone();
        }
        io.device(request).await.map_err(denied)?;
    }
    let mut shared = shared.lock().unwrap_or_else(|p| p.into_inner());
    shared.unpersisted.retain(|held| held.id != id);
    shared.revision += 1;
    Ok(())
}

/// `persistPending`: every held outcome kept and settled, in order.
async fn persist_pending(io: &Io, shared: &Mutex<Shared>) -> Result<()> {
    let ids: Vec<String> = shared
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .unpersisted
        .iter()
        .map(|held| held.id.clone())
        .collect();
    for id in ids {
        persist(io, shared, &id).await?;
    }
    Ok(())
}

async fn queued_entry(io: &Io, id: &str) -> Result<Option<Json>> {
    let queued = io.device(json!({"op": "queued"})).await.map_err(denied)?;
    Ok(queued
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["id"] == id)
        .cloned())
}

/// `successSequence`: the server confirmed this entry's sequence.
pub(crate) fn success_sequence(entry: &Json) -> Option<u64> {
    let seq = entry["receipt_store"]["success_seq"].as_u64()?;
    (entry["sent_seq"].as_u64() == Some(seq) || entry["observed_seq"].as_u64() == Some(seq))
        .then_some(seq)
}

/// A terminal receipt for `id` in the device's kept history (`meta`'s
/// answer for [`HISTORY`]: a JSON list as text, or null).
/// The receipt as answered: without the `input` digest it keeps.
pub(crate) fn history_receipt(history: &Json, id: &str) -> Option<Json> {
    let mut receipt = history_entry(history, id)?;
    if let Some(fields) = receipt.as_object_mut() {
        fields.remove("input");
    }
    Some(receipt)
}

/// The `input` digest the device's kept receipt for `id` names, if any.
pub(crate) fn history_input(history: &Json, id: &str) -> Option<String> {
    history_entry(history, id)?["input"]
        .as_str()
        .map(str::to_owned)
}

fn history_entry(history: &Json, id: &str) -> Option<Json> {
    let list: Json = serde_json::from_str(history.as_str()?).ok()?;
    list.as_array()?
        .iter()
        .find(|write| {
            write["id"] == id && matches!(write["state"].as_str(), Some("sent" | "failed"))
        })
        .cloned()
}

/// `recoverReceipt`: what this client already knows the server said about a
/// queued write: an outcome held in memory, the device's evidence of
/// success, or a receipt it kept. Such a write is settled, never resent.
async fn recover(io: &Io, shared: &Mutex<Shared>, entry: &Json) -> Result<Option<Json>> {
    let id = entry["id"].as_str().unwrap_or_default();
    if let Some(held) = shared
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .unpersisted
        .iter()
        .find(|held| held.id == id)
    {
        return Ok(Some(held.outcome.clone()));
    }
    if let Some(seq) = success_sequence(entry) {
        return Ok(Some(
            json!({"state": "sent", "id": id, "seq": seq, "replayed": true}),
        ));
    }
    let history = io
        .device(json!({"op": "meta", "key": HISTORY}))
        .await
        .map_err(denied)?;
    Ok(history_receipt(&history, id))
}

/// `flushOnce`: send the outbox in order, one at a time, at most once each.
/// Returns whether anything settled. A transport failure, an answer that is
/// not the server's, or a retryable refusal stops the flush and leaves the
/// entry queued for the next round; only the server's own outcome settles.
async fn flush(io: &Io, shared: &Mutex<Shared>) -> Result<bool> {
    let entries = io.device(json!({"op": "queued"})).await.map_err(denied)?;
    let mut settled = false;
    for entry in entries.as_array().cloned().unwrap_or_default() {
        let id = entry["id"].as_str().unwrap_or_default().to_owned();
        let Some(entry) = queued_entry(io, &id).await? else {
            continue;
        };
        if let Some(known) = recover(io, shared, &entry).await? {
            settle(io, shared, &id, known, None).await?;
            settled = true;
            continue;
        }
        let state = io
            .device(json!({"op": "sync_state", "capture": false}))
            .await
            .map_err(denied)?;
        let binding = [
            &entry["receipt_store"]["id"],
            &entry["store_id"],
            &state["store_id"],
        ]
        .into_iter()
        .find(|id| id.is_string())
        .cloned();
        let Some(binding) = binding else {
            // No store identity yet: the next sync supplies it.
            break;
        };
        io.device(json!({"op": "begin_send", "id": id}))
            .await
            .map_err(denied)?;
        let body = json!({"id": id, "args": entry["args"], "newIds": entry["new_ids"], "store_id": binding});
        let op = entry["op"].as_str().unwrap_or_default();
        let (status, json, why) = read(io.fetch(Fetch::post(format!("/m/{op}"), body)).await)?;
        let sent = json["state"] == "sent";
        if let Some(why) = &why {
            // A retryable refusal leaves the write queued; so does a refused
            // session, which a fresh sign-in resolves (`refusedSession`).
            if why["retryable"] == true {
                return Err(Stop::Retry(why.clone()));
            }
            if why["code"] == "E_AUTH" {
                return Err(Stop::Denied(why.clone()));
            }
        } else if !sent {
            return Err(Stop::Retry(refusal(
                "E_HTTP_RESPONSE",
                "link",
                format!("/m/{op}: HTTP {status} without a state; the outcome is unknown"),
            )));
        }
        let outcome = if sent {
            let mut outcome =
                json!({"state": "sent", "id": id, "seq": json["seq"].as_u64().unwrap_or(0)});
            match json.get("result") {
                Some(result) if json["replayed"] != true => outcome["result"] = result.clone(),
                _ => outcome["replayed"] = json!(true),
            }
            outcome
        } else {
            json!({"state": "failed", "id": id, "why": why})
        };
        let revalidated =
            (json["replayed"] == true && json["store_id"] == binding).then_some(&binding);
        settle(io, shared, &id, outcome, revalidated).await?;
        settled = true;
    }
    Ok(settled)
}
