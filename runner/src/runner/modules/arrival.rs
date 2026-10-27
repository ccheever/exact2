//! An arrival in the declared substitute (LLP 1133.035 D16 item 1, §10): an
//! agent word that stands for something arriving rather than a fact. The
//! interface's `"substitute": {"arrival": …}` names the words and the names
//! an arrival reaches: `field`, the word whose setting brings a fixture tag
//! into the field; `from`, the word listing the fixtures by `id`; the
//! status (its `readerUI`), the scan, the write and the lock, and the two
//! events.
//!
//! An arrival, within this session: a waiting write or lock settles
//! against the tag (the fixture takes the records, or is locked, or the
//! write fails as the fixture's `write` says) and nothing is read; else a
//! waiting scan raises `reading`, or `readingerror` for a tag with no NDEF
//! (on iOS only for a scan with `apple.polling`, D11), and each `once` scan
//! answers `read`. A tag outside a scan or write is the system's (D9).
//!
//! The record model is Web NFC's NDEF records, the one consumer's: a
//! fixture's `data` is a string for `text`, `url` and `absolute-url`, else
//! `{"base64": …}`, as bytes cross the seam. Several sessions sharing one
//! reader is the native registry's (D6), not this.

use super::super::{DataSource, Runner};
use super::{CallOrigin, Decision, Json, ModulesRow, Waits};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// What one interface's arrivals keep in this session.
#[derive(Default)]
pub(in crate::runner) struct Arrivals {
    /// The readings `ndefRecords` answers from, oldest first: each its id
    /// and the tag as it was read.
    readings: BTreeMap<String, VecDeque<(String, Json)>>,
    /// The last reading id given.
    last: u64,
    /// The scans suspended while the app is in the background, by token.
    suspended: BTreeSet<u64>,
}

/// A held claim of the arrival's names.
struct Claim {
    token: u64,
    hold: u64,
    declared: String,
    args: Json,
    origin: CallOrigin,
}

/// A record's type whose `data` Contract writes as a string (Web NFC's
/// "Data mapping").
fn textual(record_type: &str) -> bool {
    matches!(record_type, "text" | "url" | "absolute-url")
}

impl<D: DataSource> Runner<D> {
    /// The arrival `row`'s interface declares, with its extension.
    fn arrival_of(&self, row: &ModulesRow) -> Option<(String, Json)> {
        let arrival = self.substitute(row)["arrival"].clone();
        arrival
            .is_object()
            .then(|| (self.plan.str(row.extension).to_owned(), arrival))
    }

    /// Each interface's arrival, by its extension.
    fn arrivals(&self) -> Vec<(String, Json)> {
        let mut out: Vec<(String, Json)> = Vec::new();
        for row in &self.plan.modules {
            if let Some((extension, arrival)) = self.arrival_of(row) {
                if !out.iter().any(|(e, _)| *e == extension) {
                    out.push((extension, arrival));
                }
            }
        }
        out
    }

    /// The claims of `extension`'s names `declared` waiting on their ticket.
    fn claims(&self, extension: &str, declared: &[&str]) -> Vec<Claim> {
        (self.extensions.held.iter())
            .filter(|h| h.waits == Waits::Value)
            .filter_map(|h| {
                let row = self.module_row(&h.source);
                let name = self.plan.str(row.declared);
                (self.plan.str(row.extension) == extension && declared.contains(&name)).then(|| {
                    Claim {
                        token: h.token,
                        hold: h.hold,
                        declared: name.to_owned(),
                        args: h.args.clone(),
                        origin: h.origin,
                    }
                })
            })
            .collect()
    }

    /// The fixture tags the agent prefers for `extension`, as they are now.
    fn fixtures(&self, extension: &str, arrival: &Json) -> Vec<Json> {
        let key = format!("{extension}.{}", arrival["from"].as_str().unwrap_or("tags"));
        (self.extensions.preferred.get(&key))
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default()
    }

    /// A claim's reader settings (D6): the reader it needs and the
    /// platform's poll and flag words, a word left out being the default.
    fn reader_settings(args: &Json) -> Json {
        json!({
            "polling": args["apple"]["polling"],
            "technologies": args["android"]["technologies"],
            "skipNdefCheck": args["android"]["skipNdefCheck"] == true,
            "presenceCheckDelay": args["android"]["presenceCheckDelay"],
            "platformSounds": args["android"]["platformSounds"] != false,
        })
    }

    /// What the arrival's own names decide now (§10): a scan or a write
    /// whose settings another claim's exclude is `busy`; a write whose
    /// message cannot be encoded fails `TypeError`; `ndefRecords` answers a
    /// kept reading's records. `None` where the ticket decides.
    pub(super) fn arrival_decide(
        &self,
        row: &ModulesRow,
        sub: &Json,
        send: bool,
        args: &Json,
    ) -> Option<Decision> {
        let (extension, arrival) = self.arrival_of(row)?;
        let declared = self.plan.str(row.declared);
        let name = |k: &str| arrival[k].as_str().unwrap_or_default();
        if let Some(option) = sub["kept"].as_str() {
            let id = args[option].as_str().unwrap_or_default();
            let kept = (self.extensions.arrivals.readings.get(&extension))
                .and_then(|r| r.iter().find(|(r, _)| r == id))
                .map(|(_, tag)| tag.clone());
            return Some(Decision::Now(match kept {
                Some(tag) => Json::Array(
                    (tag["records"].as_array().into_iter().flatten())
                        .map(full_record)
                        .collect(),
                ),
                None => json!({ "error": "NotFoundError", "message": id }),
            }));
        }
        if !send || ![name("scan"), name("write"), name("lock")].contains(&declared) {
            return None;
        }
        if declared == name("write") {
            if let Err(why) = message_checked(&args["message"]) {
                return Some(Decision::Now(
                    json!({ "error": "TypeError", "message": why }),
                ));
            }
        }
        let names = [name("scan"), name("write"), name("lock")];
        let mine = Self::reader_settings(args);
        let writes = declared != name("scan");
        let busy = self.claims(&extension, &names).iter().any(|c| {
            let theirs = Self::reader_settings(&c.args);
            let hidden = (writes && theirs["skipNdefCheck"] == true)
                || (c.declared != name("scan") && mine["skipNdefCheck"] == true);
            theirs != mine || hidden
        });
        Some(match busy {
            true => Decision::Now(json!({ "error": "InvalidStateError", "message": "busy" })),
            false => Decision::Hold(Waits::Value),
        })
    }

    /// A claim of `row` was just held: a write or lock replaces any that
    /// waits, which fails `AbortError` (Web NFC: a write "replaces all
    /// previously configured write operations").
    pub(super) fn arrival_claimed(&mut self, row: &ModulesRow) {
        let Some((extension, arrival)) = self.arrival_of(row) else {
            return;
        };
        let name = |k: &str| arrival[k].as_str().unwrap_or_default().to_owned();
        let declared = self.plan.str(row.declared).to_owned();
        if declared != name("write") && declared != name("lock") {
            return;
        }
        let mut writes = self.claims(&extension, &[&name("write"), &name("lock")]);
        writes.pop();
        for c in writes {
            self.log(format!(
                "{extension}.{}: replaced by a newer write",
                c.declared
            ));
            self.retire_hold(c.hold);
            self.release(c.token, json!({ "error": "AbortError" }));
        }
        self.native_slot().changed("");
    }

    /// The ticket's further taps besides `cancel` (`taps`: a scan's
    /// `timeout`), and whether it takes a typed value: an arrival's names
    /// take none.
    pub(super) fn ticket_taps(sub: &Json) -> (Vec<String>, bool) {
        let taps = (sub["taps"].as_object().into_iter().flatten())
            .map(|(k, _)| k.clone())
            .collect();
        (taps, !sub["arrival"].is_object())
    }

    /// A send that stops the session's claims of the names it `stops`, each
    /// answering its `stopped` (`abortScan`, `abortWrite`). False for any
    /// other send.
    pub(super) fn apply_stops(&mut self, row: &ModulesRow) -> bool {
        let sub = self.substitute(row);
        let Some(stops) = sub["stops"].as_array() else {
            return false;
        };
        let extension = self.plan.str(row.extension).to_owned();
        let names: Vec<&str> = stops.iter().filter_map(Json::as_str).collect();
        for c in self.claims(&extension, &names) {
            let stopped = self
                .sibling(row, &c.declared)
                .map(|r| self.substitute(&r)["stopped"].clone())
                .unwrap_or(json!({ "error": "AbortError" }));
            self.log(format!(
                "{extension}.{}: stopped by {}",
                c.declared,
                self.plan.str(row.declared)
            ));
            self.extensions.arrivals.suspended.remove(&c.token);
            self.retire_hold(c.hold);
            self.release(c.token, stopped);
        }
        self.native_slot().changed("");
        true
    }

    /// Why `prefer extension key=value` is refused for an arrival's field
    /// word: an id no fixture has, among those the same prefer sets or
    /// those already set.
    pub(super) fn arrival_refusal(
        &self,
        key: &str,
        value: &Json,
        group: &serde_json::Map<String, Json>,
    ) -> Option<String> {
        let (extension, word) = key.split_once('.')?;
        let (_, arrival) = self.arrivals().into_iter().find(|(e, _)| e == extension)?;
        if arrival["field"] != word || value == "clear" {
            return None;
        }
        let id = value.as_str()?;
        let from = format!("{extension}.{}", arrival["from"].as_str().unwrap_or("tags"));
        let fixtures = match group.get(&from) {
            Some(tags) => tags.as_array().cloned().unwrap_or_default(),
            None => self.fixtures(extension, &arrival),
        };
        (!fixtures.iter().any(|t| t["id"] == id))
            .then(|| format!("prefer extension: no tag `{id}` in `{from}`"))
    }

    /// The agent set `key` from `before` to `after`: where it is an
    /// arrival's field word and names another tag, or the same one after a
    /// clear, that tag arrives.
    pub(super) fn arrival_field(&mut self, key: &str, before: Option<Json>, after: Option<Json>) {
        let Some((extension, word)) = key.split_once('.') else {
            return;
        };
        let Some((_, arrival)) = self.arrivals().into_iter().find(|(e, _)| e == extension) else {
            return;
        };
        if arrival["field"] != word || before == after {
            return;
        }
        let extension = extension.to_owned();
        match after.as_ref().and_then(Json::as_str) {
            Some(id) => self.tag_arrives(&extension, &arrival, id),
            None => self.log(format!("{key}: the tag left the field")),
        }
    }

    /// Tag `id` arrives in `extension`'s field.
    fn tag_arrives(&mut self, extension: &str, arrival: &Json, id: &str) {
        let name = |k: &str| arrival[k].as_str().unwrap_or_default().to_owned();
        let Some(tag) = (self.fixtures(extension, arrival).into_iter()).find(|t| t["id"] == id)
        else {
            return;
        };
        self.log(format!("{extension}: tag `{id}` arrived"));
        let mut writes = self.claims(extension, &[&name("write"), &name("lock")]);
        if let Some(write) = writes.pop() {
            for c in writes {
                self.retire_hold(c.hold);
                self.release(c.token, json!({ "error": "AbortError" }));
            }
            let lock = write.declared == name("lock");
            let answer = self.tag_written(extension, arrival, &tag, &write.args, lock);
            let outcome = super::error_code(&answer).unwrap_or("written").to_owned();
            self.log(format!("{extension}.{}: {outcome}", write.declared));
            self.retire_hold(write.hold);
            self.release(write.token, answer);
            self.native_slot().changed("");
            return;
        }
        let suspended = self.extensions.arrivals.suspended.clone();
        let scans: Vec<Claim> = (self.claims(extension, &[&name("scan")]).into_iter())
            .filter(|c| !suspended.contains(&c.token))
            .collect();
        if scans.is_empty() {
            self.log(format!("{extension}: tag `{id}` read by no scan"));
            return;
        }
        let plain = tag["ndef"] == false;
        if plain {
            // The NDEF reader session never reports a tag without NDEF (D11).
            let polled = scans
                .iter()
                .any(|c| (c.args["apple"]["polling"].as_array()).is_some_and(|p| !p.is_empty()));
            if self.host_target() == "ios" && !polled {
                self.log(format!(
                    "{extension}: tag `{id}` holds no NDEF, unseen by an NDEF reader session"
                ));
                return;
            }
            let args = json!([nfc_tag(&tag)]).to_string();
            if let Err(e) =
                self.raise_module_event(extension, &name("readingerror"), &args, CallOrigin::Device)
            {
                self.log(e);
            }
            return;
        }
        let arrivals = &mut self.extensions.arrivals;
        arrivals.last += 1;
        let reading_id = format!("r{}", arrivals.last);
        let hold = arrival["hold"].as_u64().unwrap_or(16) as usize;
        let kept = arrivals.readings.entry(extension.to_owned()).or_default();
        kept.push_back((reading_id.clone(), tag.clone()));
        while kept.len() > hold {
            kept.pop_front();
        }
        let mut reading = nfc_tag(&tag);
        reading["id"] = json!(reading_id);
        reading["records"] = Json::Array(
            (tag["records"].as_array().into_iter().flatten())
                .map(record_info)
                .collect(),
        );
        let args = json!([reading]).to_string();
        if let Err(e) =
            self.raise_module_event(extension, &name("reading"), &args, CallOrigin::Device)
        {
            self.log(e);
        }
        for c in scans.into_iter().filter(|c| c.args["once"] == true) {
            self.retire_hold(c.hold);
            self.release(c.token, json!({ "reason": "read", "reading": reading_id }));
        }
        self.native_slot().changed("");
    }

    /// The answer of a write (or, `lock`, a lock) to `tag`, which the
    /// fixture takes when it succeeds (D7's steps, by the fixture's `write`).
    fn tag_written(
        &mut self,
        extension: &str,
        arrival: &Json,
        tag: &Json,
        args: &Json,
        lock: bool,
    ) -> Json {
        let fail =
            |error: &str, message: Option<&str>| json!({ "error": error, "message": message });
        let records = tag["records"].as_array().cloned().unwrap_or_default();
        if !lock && args["overwrite"] == false && !records.is_empty() {
            return fail("NotAllowedError", Some("overwrite"));
        }
        match tag["write"].as_str().unwrap_or("ok") {
            _ if tag["ndef"] == false => return fail("NotSupportedError", None),
            "read-only" => return fail("NetworkError", Some("read-only")),
            "too-small" if !lock => return fail("NetworkError", Some("too-small")),
            "lost" => return fail("NetworkError", Some("lost")),
            "not-ndef" => return fail("NotSupportedError", None),
            _ => {}
        }
        let id = tag["id"].clone();
        let from = format!("{extension}.{}", arrival["from"].as_str().unwrap_or("tags"));
        if let Some(tags) = self
            .extensions
            .preferred
            .get_mut(&from)
            .and_then(Json::as_array_mut)
        {
            if let Some(t) = tags.iter_mut().find(|t| t["id"] == id) {
                if lock {
                    t["write"] = json!("read-only");
                } else {
                    t["records"] = Json::Array(
                        (args["message"]["records"].as_array().into_iter().flatten())
                            .map(fixture_record)
                            .collect(),
                    );
                }
            }
        }
        json!({ "serialNumber": tag["serialNumber"].as_str().unwrap_or("") })
    }

    /// The app left the foreground (`hidden`) or came back (D6's table): a
    /// waiting write fails `AbortError`; a scan behind the system's sheet
    /// (`readerUI` `system`) ends `interrupted`, and one the app draws for
    /// is suspended, and on return reads a tag still in the field.
    pub(in crate::runner) fn arrival_visibility(&mut self, hidden: bool) {
        for (extension, arrival) in self.arrivals() {
            let name = |k: &str| arrival[k].as_str().unwrap_or_default().to_owned();
            if hidden {
                for c in self.claims(&extension, &[&name("write"), &name("lock")]) {
                    self.log(format!(
                        "{extension}.{}: aborted, the app left the foreground",
                        c.declared
                    ));
                    self.retire_hold(c.hold);
                    self.release(
                        c.token,
                        json!({ "error": "AbortError", "message": "background" }),
                    );
                }
                let system = (self.plan.modules.iter())
                    .find(|m| {
                        self.plan.str(m.extension) == extension
                            && self.plan.str(m.declared) == name("status")
                    })
                    .cloned()
                    .and_then(|s| self.value_of(&s))
                    .is_some_and(|s| s["readerUI"] == "system");
                for c in self.claims(&extension, &[&name("scan")]) {
                    if system {
                        self.log(format!(
                            "{extension}.{}: interrupted, the app left the foreground",
                            c.declared
                        ));
                        self.retire_hold(c.hold);
                        self.release(
                            c.token,
                            json!({ "reason": "interrupted", "message": "background" }),
                        );
                    } else {
                        self.log(format!("{extension}.{}: suspended", c.declared));
                        self.extensions.arrivals.suspended.insert(c.token);
                    }
                }
            } else {
                let resumed = std::mem::take(&mut self.extensions.arrivals.suspended);
                if resumed.is_empty() {
                    continue;
                }
                self.log(format!("{extension}: scans resumed"));
                let key = format!("{extension}.{}", name("field"));
                if let Some(id) = (self.extensions.preferred.get(&key))
                    .and_then(Json::as_str)
                    .map(str::to_owned)
                {
                    self.tag_arrives(&extension, &arrival, &id);
                }
            }
            self.native_slot().changed("");
        }
    }

    /// `state.extensions.<extension>`'s arrival: the fixture tags as they
    /// are now, the field, each scan (its ticket, origin, `once`, and
    /// whether it is suspended), the waiting write, and the readings kept.
    pub(super) fn arrival_json(&self, extension: &str, entry: &mut Json) {
        let Some((_, arrival)) = self.arrivals().into_iter().find(|(e, _)| e == extension) else {
            return;
        };
        let name = |k: &str| arrival[k].as_str().unwrap_or_default().to_owned();
        let field = format!("{extension}.{}", name("field"));
        entry["tags"] = Json::Array(self.fixtures(extension, &arrival));
        entry["field"] = self
            .extensions
            .preferred
            .get(&field)
            .cloned()
            .unwrap_or(Json::Null);
        let suspended = &self.extensions.arrivals.suspended;
        entry["scans"] = Json::Array(
            (self.claims(extension, &[&name("scan")]).iter())
                .map(|c| {
                    json!({ "ticket": c.hold, "origin": c.origin.word(), "once": c.args["once"] == true,
                        "suspended": suspended.contains(&c.token) })
                })
                .collect(),
        );
        entry["write"] = (self
            .claims(extension, &[&name("write"), &name("lock")])
            .last())
        .map_or(
            Json::Null,
            |c| json!({ "ticket": c.hold, "name": c.declared, "origin": c.origin.word() }),
        );
        entry["readings"] = Json::Array(
            (self
                .extensions
                .arrivals
                .readings
                .get(extension)
                .into_iter()
                .flatten())
            .map(|(id, tag)| json!({ "id": id, "tag": tag["id"] }))
            .collect(),
        );
    }
}

/// A fixture tag as an `NFCTag`.
fn nfc_tag(tag: &Json) -> Json {
    json!({ "id": tag["id"], "serialNumber": tag["serialNumber"].as_str().unwrap_or("") })
}

/// A fixture record's bytes: a string's UTF-8, or its `{"base64": …}`.
fn record_bytes(record: &Json) -> Vec<u8> {
    match &record["data"] {
        Json::String(s) => s.as_bytes().to_vec(),
        data => data["base64"]
            .as_str()
            .map(base64_decode)
            .unwrap_or_default(),
    }
}

/// A fixture record as Contract reads it (`NDEFRecordInfo`): its `text`
/// decoded for the record types D2 names, its size in bytes.
fn record_info(record: &Json) -> Json {
    let record_type = record["recordType"].as_str().unwrap_or("empty");
    let text = match record_type {
        t if textual(t) => record["data"].as_str().map(str::to_owned),
        // A smart poster's text is the URL of its URI record.
        "smart-poster" => (record["records"].as_array().into_iter().flatten())
            .find(|r| r["recordType"] == "url")
            .and_then(|r| r["data"].as_str().map(str::to_owned)),
        _ => None,
    };
    let mut info =
        json!({ "recordType": record_type, "text": text, "size": record_bytes(record).len() });
    for field in ["mediaType", "id", "encoding", "lang"] {
        info[field] = record[field].clone();
    }
    if record_type == "text" && info["encoding"].is_null() {
        info["encoding"] = json!("utf-8");
    }
    info
}

/// A fixture record as data code reads it (`NDEFRecord`): the info, its
/// bytes, and its own records one level deep (`NDEFInnerRecord`: plan
/// values are finite trees).
fn full_record(record: &Json) -> Json {
    let bytes = |r: &Json| {
        let mut out = record_info(r);
        out["data"] = json!({ "base64": base64_encode(&record_bytes(r)) });
        out
    };
    let mut out = bytes(record);
    out["records"] = match record["records"].as_array() {
        Some(inner) => Json::Array(inner.iter().map(bytes).collect()),
        None => Json::Null,
    };
    out
}

/// A record the app wrote, as the fixture keeps it.
fn fixture_record(record: &Json) -> Json {
    let mut out = json!({ "recordType": record["recordType"], "data": record["data"] });
    for field in ["mediaType", "id", "encoding", "lang"] {
        if !record[field].is_null() {
            out[field] = record[field].clone();
        }
    }
    out
}

/// Why a write's message cannot be encoded (Web NFC's "create NDEF
/// record"): no record, a string's data for a type that takes bytes, or a
/// URL that is none.
fn message_checked(message: &Json) -> Result<(), String> {
    let records = message["records"].as_array().filter(|r| !r.is_empty());
    for record in records.ok_or("a message has a record")? {
        let record_type = record["recordType"].as_str().unwrap_or_default();
        match &record["data"] {
            Json::String(_) if !textual(record_type) => {
                return Err(format!("a `{record_type}` record's data is bytes"))
            }
            Json::String(url) if record_type != "text" && !url.contains(':') => {
                return Err(format!("`{url}` is not a URL"))
            }
            Json::Null if record_type != "empty" => {
                return Err(format!("a `{record_type}` record has data"))
            }
            _ => {}
        }
    }
    Ok(())
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            out.push(match i <= chunk.len() {
                true => B64[(n >> (18 - 6 * i) & 63) as usize] as char,
                false => '=',
            });
        }
    }
    out
}

fn base64_decode(text: &str) -> Vec<u8> {
    let (mut out, mut n, mut bits) = (Vec::new(), 0u32, 0);
    for c in text.bytes().filter(|c| *c != b'=') {
        let Some(v) = B64.iter().position(|b| *b == c) else {
            continue;
        };
        n = n << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits & 255) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for text in ["", "a", "ab", "abc", "abcd", "com.exact.shelf:book"] {
            assert_eq!(
                base64_decode(&base64_encode(text.as_bytes())),
                text.as_bytes()
            );
        }
        assert_eq!(base64_encode(b"ab"), "YWI=");
    }

    #[test]
    fn a_message_needs_records_of_their_kinds() {
        assert!(message_checked(&json!({ "records": [] })).is_err());
        assert!(message_checked(
            &json!({ "records": [{ "recordType": "url", "data": "https://x.org" }] })
        )
        .is_ok());
        assert!(
            message_checked(&json!({ "records": [{ "recordType": "url", "data": "nope" }] }))
                .is_err()
        );
        assert!(
            message_checked(&json!({ "records": [{ "recordType": "mime", "data": "text" }] }))
                .is_err()
        );
        assert!(message_checked(
            &json!({ "records": [{ "recordType": "x.org:a", "data": { "base64": "YQ==" } }] })
        )
        .is_ok());
    }
}
