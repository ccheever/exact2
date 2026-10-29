//! A module of devices a person plugs in (LLP 1133.037 §11): serial ports,
//! HID and USB devices under the declared substitute. The interface's
//! `devices` names the agent word that lists the devices (`prefer extension
//! serial.ports=[…]`), the handle kind, whether an open is exclusive, the
//! record's fields with what fills them, and the targets that have none.
//! Each name's `"device": {"role", …}` says what it does:
//!
//! - **`request`**: a `chooser` ticket listing every preferred device;
//!   `tap @t <id>` answers its record (a list of it, `many`) and remembers
//!   it, `cancel` answers `cancel`.
//! - **`remembered`**, **`attached`**: the devices chosen or opened and not
//!   forgotten, each with `connected`; the preferred devices plugged in.
//! - **`open`**: by the option `by`, a new connection (`<kind>-<n>`) on the
//!   device, which joins the remembered ones. An exclusive device open
//!   anywhere in the app fails `InvalidStateError` first; on Android a
//!   device whose `granted` is false holds a `permission` ticket.
//! - **`connection`**: `{open, connected, …}` of a connection.
//! - **`write`**: journaled in `state.extensions.<module>.writes` with its
//!   options, answered at once (`written`: `{status, bytesWritten}`); a USB
//!   claim, release or configuration is kept on the connection.
//! - **`keyed`** (LLP 1133.022 D19 item 2, LLP 1133.037 D19 item 4): the
//!   device fixture's value at `at`, a path whose `{option}` parts are the
//!   call's; a list answers its entries one per call, then repeats its last.
//! - **`stream`** (D19 item 2): a byte stream from the fixture's `events`,
//!   each delivered once when the agent clock passes its `at` from the open;
//!   every answer is every byte since the last, with the errors at their
//!   offsets, and `done` once the port is closed or unplugged.
//! - **`reports`** (LLP 1133.013 D5's batch): the device's input reports
//!   from its `events`, the first answer the latest, each later one every
//!   report since the reader's last, at most `history`.
//! - **`close`**, **`forget`**: end a connection; forget a device, ending
//!   every connection on it.
//!
//! A reader's cursor moves when the runner answers it: a stream answer
//! the runner then discards is not answered again (LLP 1133.022 D19 item 5
//! is not built).

use super::super::{DataSource, Runner};
use super::{CallOrigin, Decision, Json, ModulesRow, Waits};
use crate::agent::{base64, unbase64};
use crate::HoldAnswer;
use serde_json::{json, Map};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(in crate::runner) struct Devices {
    /// The devices each extension remembers (chosen, or opened from the
    /// attached list, and not forgotten), oldest first.
    remembered: BTreeMap<String, Vec<String>>,
    /// The connections this session opened, by handle.
    connections: BTreeMap<String, Connection>,
    next: u64,
    /// What each extension's writes sent, oldest first.
    writes: BTreeMap<String, Vec<Json>>,
    /// How far each keyed list has gone, by extension, then device and path.
    sequences: BTreeMap<String, BTreeMap<String, usize>>,
    /// The tickets held for an `open` on Android, by held token: the call.
    asking: BTreeMap<u64, Json>,
    /// The app's open exclusive devices, shared by its sessions, and the
    /// ones this session holds there.
    app: Option<super::AppState>,
    exclusive: BTreeSet<String>,
}

impl Drop for Devices {
    /// A session's end closes what it held open (D5).
    fn drop(&mut self) {
        if let Some(app) = &self.app {
            let mut state = app.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(open) = state.get_mut(OPEN).and_then(Json::as_object_mut) {
                for key in &self.exclusive {
                    open.remove(key);
                }
            }
        }
    }
}

/// The app-wide record of open exclusive devices, `<extension> <id>`.
const OPEN: &str = "devices.open";

struct Connection {
    extension: String,
    device: String,
    open: bool,
    /// The agent clock at the open, which the fixture's `at` counts from.
    opened: f64,
    options: Json,
    configuration: Option<Json>,
    claimed: BTreeSet<u64>,
    /// A byte stream's events delivered, its byte position, and whether its
    /// end was delivered (a later read fails).
    delivered: usize,
    position: usize,
    ended: bool,
    /// Each report reader's events delivered, by its arguments.
    readers: BTreeMap<String, usize>,
}

impl<D: DataSource> Runner<D> {
    /// The `device` role of `row`, where it is one of a device module's names.
    pub(in crate::runner) fn device_role(&self, row: &ModulesRow) -> Option<Json> {
        let sub = self.substitute(row);
        sub["device"]["role"]
            .is_string()
            .then(|| sub["device"].clone())
    }

    /// The preferred devices of `row`'s extension, each filled to a record
    /// and with its fixture.
    fn devices_of(&self, row: &ModulesRow) -> Vec<Json> {
        let sub = self.substitute(row);
        let word = sub["devices"]["word"].as_str().unwrap_or("devices");
        let key = format!("{}.{word}", self.plan.str(row.extension));
        (self.extensions.preferred.get(&key))
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default()
    }

    fn device_fixture(&self, row: &ModulesRow, id: &str) -> Option<Json> {
        (self.devices_of(row).into_iter()).find(|d| d["id"] == id)
    }

    /// A device as its record: the declared fields, filled, with whether
    /// this process may open it without a further prompt.
    fn device_record(&self, row: &ModulesRow, fixture: &Json) -> Json {
        let devices = &self.substitute(row)["devices"];
        let mut record = Map::new();
        for field in devices["record"].as_array().into_iter().flatten() {
            let Some(field) = field.as_str() else {
                continue;
            };
            let value = match &fixture[field] {
                Json::Null => devices["fill"][field].clone(),
                v => v.clone(),
            };
            if !value.is_null() {
                record.insert(field.to_owned(), value);
            }
        }
        Json::Object(record)
    }

    /// Whether the extension's module is absent on this target (D21).
    fn device_absent(&self, row: &ModulesRow) -> bool {
        let target = self.host_target();
        (self.substitute(row)["devices"]["absent"]
            .as_array()
            .into_iter())
        .flatten()
        .any(|t| t == target)
    }

    /// Every live source of `row`'s extension asked again: a device or a
    /// connection changed.
    pub(in crate::runner) fn devices_changed(&self, extension: &str) {
        for m in &self.plan.modules {
            if self.plan.str(m.extension) == extension && m.live && !m.send {
                self.announce(&self.key(m));
            }
        }
    }

    /// Whether `key` is a device module's stream or reports with a
    /// connection still reading: polled though no Contract resource reads it.
    pub(in crate::runner) fn device_polled(&self, key: &str) -> bool {
        let Some(row) = (self.plan.modules.iter()).find(|m| self.key(m) == key) else {
            return false;
        };
        let extension = self.plan.str(row.extension);
        let reading = matches!(
            self.substitute(row)["device"]["role"].as_str(),
            Some("stream" | "reports")
        );
        reading
            && (self.extensions.devices.connections.values())
                .any(|c| c.extension == extension && c.open && !c.ended)
    }

    /// `key` was preferred: where it is a device module's agent word, its
    /// sources are asked again.
    pub(in crate::runner) fn devices_preferred(&self, key: &str) {
        let Some((extension, word)) = key.split_once('.') else {
            return;
        };
        let lists = (self.plan.modules.iter()).any(|m| {
            self.plan.str(m.extension) == extension && self.substitute(m)["devices"]["word"] == word
        });
        if lists {
            self.devices_changed(extension);
        }
    }

    /// What a device module's name answers now, admitted: an answer, or a
    /// ticket. `None` for a name that is no device module's.
    pub(super) fn device_decide(
        &mut self,
        row: &ModulesRow,
        send: bool,
        args: &Json,
    ) -> Option<Decision> {
        let device = self.device_role(row)?;
        if self.device_absent(row) || self.unsupported_here(row) {
            return Some(Decision::Now(json!({ "error": "NotSupportedError" })));
        }
        if let Some(decision) = self.decide_permission(row, send, args) {
            return Some(decision);
        }
        Some(match device["role"].as_str().unwrap_or_default() {
            "request" => Decision::Hold(Waits::Device),
            "open" => self.device_open(row, &device, args),
            _ => Decision::Now(self.device_answer(row, &device, args)),
        })
    }

    /// A device module's send that answers nothing, substituted: `None`
    /// for a name that is no device module's.
    pub(in crate::runner) fn device_effect(
        &mut self,
        row: &ModulesRow,
        args: &Json,
    ) -> Option<Json> {
        let device = self.device_role(row)?;
        if self.device_absent(row) {
            return Some(json!({ "error": "NotSupportedError" }));
        }
        Some(self.device_answer(row, &device, args))
    }

    /// What a name answers now; it changes cursors, sequences, connections
    /// and the journal as it goes.
    fn device_answer(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Json {
        let extension = self.plan.str(row.extension).to_owned();
        match device["role"].as_str().unwrap_or_default() {
            "remembered" => {
                let ids = self.extensions.devices.remembered.get(&extension);
                let ids = ids.cloned().unwrap_or_default();
                let fixtures = self.devices_of(row);
                let list: Vec<Json> = (ids.iter())
                    .map(
                        |id| match fixtures.iter().find(|d| d["id"] == id.as_str()) {
                            Some(f) => self.device_record(row, f),
                            // A remembered device no longer plugged in.
                            None => json!({ "id": id, "connected": false }),
                        },
                    )
                    .map(|mut r| {
                        self.fill_record(row, &mut r);
                        r
                    })
                    .collect();
                json!(list)
            }
            "attached" => {
                let list: Vec<Json> = (self.devices_of(row).iter())
                    .filter(|d| d["connected"] != false)
                    .map(|d| self.device_record(row, d))
                    .collect();
                json!(list)
            }
            "connection" => self.connection_answer(row, args),
            "write" => self.device_write(row, device, args),
            "keyed" => self.device_keyed(row, device, args),
            "stream" => self.device_stream(row, device, args),
            "reports" => self.device_reports(row, device, args),
            "close" => self.device_close(args),
            "forget" => self.device_forget(row, args),
            other => json!({ "error": "NotSupportedError", "message": other }),
        }
    }

    /// A remembered record made whole where the device is gone: the
    /// declared fill, so it is the record's shape.
    fn fill_record(&self, row: &ModulesRow, record: &mut Json) {
        let devices = &self.substitute(row)["devices"];
        for (field, value) in devices["fill"].as_object().into_iter().flatten() {
            if record.get(field).is_none() {
                record[field] = value.clone();
            }
        }
        if let Some(fixture) = record["id"]
            .as_str()
            .and_then(|id| self.device_fixture(row, id))
        {
            record["connected"] = json!(fixture["connected"] != false);
        }
    }

    /// The connection a call names, open and plugged in, or why not.
    fn connection_of(&self, args: &Json) -> Result<(&Connection, Json), Json> {
        let handle = args["connection"].as_str().unwrap_or_default();
        let c = (self.extensions.devices.connections.get(handle)).ok_or_else(
            || json!({ "error": "InvalidStateError", "message": "no such connection" }),
        )?;
        if !c.open {
            return Err(json!({ "error": "InvalidStateError", "message": "closed" }));
        }
        let row = (self.plan.modules.iter())
            .find(|m| self.plan.str(m.extension) == c.extension)
            .cloned()
            .ok_or_else(|| json!({ "error": "InvalidStateError" }))?;
        let fixture = self.device_fixture(&row, &c.device).unwrap_or_default();
        if fixture.is_null() || fixture["connected"] == false {
            return Err(
                json!({ "error": "NetworkError", "message": "the device is not connected" }),
            );
        }
        Ok((c, fixture))
    }

    fn device_open(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Decision {
        let by = device["by"].as_str().unwrap_or("device");
        let id = args[by].as_str().unwrap_or_default().to_owned();
        let Some(fixture) = self.device_fixture(row, &id) else {
            return Decision::Now(json!({ "error": "NotFoundError" }));
        };
        if fixture["connected"] == false {
            return Decision::Now(json!({ "error": "NetworkError" }));
        }
        let extension = self.plan.str(row.extension).to_owned();
        // A port already open fails before any prompt (D17).
        if self.substitute(row)["devices"]["exclusive"] == true && self.held_open(&extension, &id) {
            return Decision::Now(
                json!({ "error": "InvalidStateError", "message": "already open" }),
            );
        }
        if self.host_target() == "android" && fixture["granted"] == false {
            return Decision::Hold(Waits::Device);
        }
        Decision::Now(self.opened(row, device, args))
    }

    /// Whether device `id` of `extension` is open in any session of the app.
    fn held_open(&mut self, extension: &str, id: &str) -> bool {
        let key = format!("{extension} {id}");
        let app = self.device_app();
        let state = app.lock().unwrap_or_else(|e| e.into_inner());
        state.get(OPEN).is_some_and(|open| open[&key] == true)
    }

    fn device_app(&mut self) -> super::AppState {
        if self.extensions.devices.app.is_none() {
            self.extensions.devices.app = Some(self.app_state());
        }
        self.extensions.devices.app.clone().unwrap_or_default()
    }

    /// A new connection on the device `args` names.
    fn opened(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Json {
        let by = device["by"].as_str().unwrap_or("device");
        let id = args[by].as_str().unwrap_or_default().to_owned();
        let extension = self.plan.str(row.extension).to_owned();
        let sub = self.substitute(row);
        let kind = sub["devices"]["kind"]
            .as_str()
            .unwrap_or("device")
            .to_owned();
        let devices = &mut self.extensions.devices;
        devices.next += 1;
        let handle = format!("{kind}-{}", devices.next);
        let mut options = device["options"].clone();
        if options.is_object() {
            for (k, v) in args.as_object().into_iter().flatten() {
                if k != by && !v.is_null() {
                    options[k] = v.clone();
                }
            }
        }
        if sub["devices"]["exclusive"] == true {
            let key = format!("{extension} {id}");
            let app = self.device_app();
            let mut state = app.lock().unwrap_or_else(|e| e.into_inner());
            if !state.get(OPEN).is_some_and(Json::is_object) {
                state.insert(OPEN.into(), json!({}));
            }
            state.get_mut(OPEN).expect("the open record")[&key] = json!(true);
            drop(state);
            self.extensions.devices.exclusive.insert(key);
        }
        let devices = &mut self.extensions.devices;
        devices.connections.insert(
            handle.clone(),
            Connection {
                extension: extension.clone(),
                device: id.clone(),
                open: true,
                opened: self.now_ms,
                options: options.clone(),
                configuration: None,
                claimed: BTreeSet::new(),
                delivered: 0,
                position: 0,
                ended: false,
                readers: BTreeMap::new(),
            },
        );
        self.remember(&extension, &id);
        self.log(format!("{extension}: opened {id} as {handle}"));
        self.devices_changed(&extension);
        let fixture = self.device_fixture(row, &id).unwrap_or_default();
        let mut record = self.device_record(row, &fixture);
        if self.host_target() == "android" {
            record["granted"] = json!(true);
        }
        let mut answer = json!({ "connection": handle });
        answer[by] = record;
        if options.is_object() {
            answer["options"] = options;
        }
        answer
    }

    fn remember(&mut self, extension: &str, id: &str) {
        let list = (self.extensions.devices.remembered)
            .entry(extension.to_owned())
            .or_default();
        if !list.iter().any(|d| d == id) {
            list.push(id.to_owned());
        }
    }

    /// `{open, connected}`, with the line settings where the module has them,
    /// and the further fields its `devices.connection` names (USB's
    /// configuration and claims).
    fn connection_answer(&self, row: &ModulesRow, args: &Json) -> Json {
        let extra = self.substitute(row)["devices"]["connection"].clone();
        let has = |field: &str| (extra.as_array().into_iter().flatten()).any(|f| f == field);
        let handle = args["connection"].as_str().unwrap_or_default();
        let c = self.extensions.devices.connections.get(handle);
        let fixture = c.and_then(|c| self.device_fixture(row, &c.device));
        let connected = fixture.is_some_and(|f| f["connected"] != false);
        let mut answer =
            json!({ "open": c.is_some_and(|c| c.open) && connected, "connected": connected });
        if let Some(options) = c.map(|c| &c.options).filter(|o| o.is_object()) {
            answer["options"] = options.clone();
        }
        if has("claimed") {
            answer["claimed"] = json!(c.map(|c| c.claimed.clone()).unwrap_or_default());
        }
        if let Some(configuration) = c
            .and_then(|c| c.configuration.clone())
            .filter(|_| has("configuration"))
        {
            answer["configuration"] = configuration;
        }
        answer
    }

    fn device_write(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Json {
        let (c, _) = match self.connection_of(args) {
            Ok(found) => found,
            Err(e) => return e,
        };
        let (extension, id) = (c.extension.clone(), c.device.clone());
        let claimed = c.claimed.clone();
        let handle = args["connection"].as_str().unwrap_or_default().to_owned();
        if let Err(e) = self.device_claim_rule(row, device, args, &claimed) {
            return e;
        }
        let declared = self.plan.str(row.declared).to_owned();
        let mut entry = json!({ "name": declared, "device": id, "at": self.now_ms });
        for (k, v) in args.as_object().into_iter().flatten() {
            if !v.is_null() {
                entry[k] = v.clone();
            }
        }
        (self
            .extensions
            .devices
            .writes
            .entry(extension.clone())
            .or_default())
        .push(entry);
        let c = self.extensions.devices.connections.get_mut(&handle);
        let c = c.expect("the connection just found");
        if let Some(field) = device["sets"].as_str() {
            c.configuration = Some(
                args[match field {
                    "configuration" => "configurationValue",
                    other => other,
                }]
                .clone(),
            );
        }
        let number = args["interfaceNumber"].as_u64();
        match (&device["claims"], number) {
            (Json::Bool(true), Some(n)) => {
                c.claimed.insert(n);
            }
            (Json::Bool(false), Some(n)) => {
                c.claimed.remove(&n);
            }
            _ => {}
        }
        self.log(format!("{extension}: {declared} to {id}"));
        self.devices_changed(&extension);
        match device["answer"].as_str() {
            Some("written") => {
                let n = args["data"]
                    .as_str()
                    .and_then(unbase64)
                    .map_or(0, |b| b.len());
                json!({ "status": "ok", "bytesWritten": n })
            }
            _ => Json::Null,
        }
    }

    /// WebUSB's claim rules: a claim held by another connection on the
    /// device fails `NetworkError`; an alternate setting, an interface's
    /// control transfer or a bulk transfer on an unclaimed interface fails
    /// `InvalidStateError`.
    fn device_claim_rule(
        &self,
        row: &ModulesRow,
        device: &Json,
        args: &Json,
        claimed: &BTreeSet<u64>,
    ) -> Result<(), Json> {
        let unclaimed =
            json!({ "error": "InvalidStateError", "message": "the interface is not claimed" });
        let handle = args["connection"].as_str().unwrap_or_default();
        if device["claims"] == true {
            let n = args["interfaceNumber"].as_u64();
            let c = &self.extensions.devices.connections[handle];
            let elsewhere = (self.extensions.devices.connections.iter()).any(|(h, other)| {
                h != handle
                    && other.open
                    && other.extension == c.extension
                    && other.device == c.device
                    && n.is_some_and(|n| other.claimed.contains(&n))
            });
            if elsewhere {
                return Err(
                    json!({ "error": "NetworkError", "message": "claimed by another connection" }),
                );
            }
        }
        if let Some(option) = device["claimed"].as_str() {
            if !args[option].as_u64().is_some_and(|n| claimed.contains(&n)) {
                return Err(unclaimed);
            }
        }
        if let Some(option) = device["recipient"].as_str() {
            let setup = &args[option];
            if setup["recipient"] == "interface"
                && !setup["index"]
                    .as_u64()
                    .is_some_and(|i| claimed.contains(&(i & 0xff)))
            {
                return Err(unclaimed);
            }
        }
        let declared = self.plan.str(row.declared);
        if args["endpointNumber"].is_number() && declared != "clearHalt" && claimed.is_empty() {
            return Err(unclaimed);
        }
        Ok(())
    }

    /// The fixture's value at the path `device.at` names, a list answering
    /// its entries in turn.
    fn device_keyed(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Json {
        let (c, fixture) = match self.connection_of(args) {
            Ok(found) => found,
            Err(e) => return e,
        };
        let (extension, id) = (c.extension.clone(), c.device.clone());
        let claimed = c.claimed.clone();
        if let Err(e) = self.device_claim_rule(row, device, args, &claimed) {
            return e;
        }
        let path = template(device["at"].as_str().unwrap_or_default(), args);
        let mut at = &fixture;
        for part in path.split('.') {
            at = &at[part];
        }
        let value = match at {
            Json::Array(list) if !list.is_empty() => {
                let seen = (self.extensions.devices.sequences.entry(extension))
                    .or_default()
                    .entry(format!("{id} {path}"))
                    .or_default();
                let entry = list[(*seen).min(list.len() - 1)].clone();
                *seen += 1;
                entry
            }
            Json::Null => return device["missing"].clone(),
            other => other.clone(),
        };
        match device["wrap"].as_str() {
            Some(field) if !value.is_object() => json!({ field: value }),
            _ => value,
        }
    }

    /// Every byte the device sent since the stream's last answer, with the
    /// errors at their offsets (D8, D9).
    fn device_stream(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Json {
        let handle = args["connection"].as_str().unwrap_or_default().to_owned();
        let Some(c) = self.extensions.devices.connections.get(&handle) else {
            return json!({ "error": "InvalidStateError", "message": "no such connection" });
        };
        if c.ended {
            return json!({ "error": "InvalidStateError", "message": "the stream has ended" });
        }
        let fixture = self.device_fixture(row, &c.device);
        let unplugged = fixture.as_ref().is_none_or(|f| f["connected"] == false);
        let events = (fixture.as_ref())
            .and_then(|f| {
                f[device["events"].as_str().unwrap_or("input")]
                    .as_array()
                    .cloned()
            })
            .unwrap_or_default();
        let since = self.now_ms - c.opened;
        let (mut delivered, mut position, open) = (c.delivered, c.position, c.open);
        let (mut bytes, mut errors) = (Vec::new(), Vec::new());
        while let Some(event) = events.get(delivered) {
            if event["at"].as_f64().unwrap_or(0.0) > since {
                break;
            }
            if let Some(data) = event["data"].as_str().and_then(unbase64) {
                position += data.len();
                bytes.extend(data);
            }
            if let Some(code) = event["error"].as_str() {
                errors.push(json!({ "at": position, "code": code }));
            }
            delivered += 1;
        }
        let done = !open || unplugged;
        if unplugged && open {
            errors.push(json!({ "at": position, "code": "NetworkError" }));
        }
        let c = self.extensions.devices.connections.get_mut(&handle);
        let c = c.expect("the connection just found");
        (c.delivered, c.position, c.ended) = (delivered, position, done);
        self.note_poll(row);
        json!({ "data": base64(&bytes), "errors": errors, "done": done, "position": position })
    }

    /// Input reports as LLP 1133.013 D5's batch: the latest first, then
    /// every report since this reader's last, at most `history`.
    fn device_reports(&mut self, row: &ModulesRow, device: &Json, args: &Json) -> Json {
        let handle = args["connection"].as_str().unwrap_or_default().to_owned();
        let Some(c) = self.extensions.devices.connections.get(&handle) else {
            return json!({ "error": "InvalidStateError", "message": "no such connection" });
        };
        let fixture = self.device_fixture(row, &c.device);
        let unplugged = fixture.as_ref().is_none_or(|f| f["connected"] == false);
        let since = self.now_ms - c.opened;
        let wanted = args["reportId"].as_f64();
        let arrived: Vec<Json> = (fixture.as_ref())
            .and_then(|f| {
                f[device["events"].as_str().unwrap_or("inputReports")]
                    .as_array()
                    .cloned()
            })
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e["at"].as_f64().unwrap_or(0.0) <= since)
            .filter(|e| wanted.is_none_or(|w| e["reportId"].as_f64() == Some(w)))
            .collect();
        let reader = super::samples::group(args);
        let history = (args["history"].as_u64())
            .or(device["history"].as_u64())
            .unwrap_or(16) as usize;
        let opened = c.opened;
        let first = !c.readers.contains_key(&reader);
        let from = c.readers.get(&reader).copied().unwrap_or(0);
        let (fresh, dropped) = if first {
            let n = arrived.len();
            (arrived[n.saturating_sub(1)..].to_vec(), 0)
        } else {
            let fresh = &arrived[from.min(arrived.len())..];
            let dropped = fresh.len().saturating_sub(history);
            (fresh[dropped..].to_vec(), dropped)
        };
        let c = self.extensions.devices.connections.get_mut(&handle);
        c.expect("the connection just found")
            .readers
            .insert(reader, arrived.len());
        self.note_poll(row);
        let reports: Vec<Json> = (fresh.iter())
            .map(|e| json!({ "reportId": e["reportId"].as_f64().unwrap_or(0.0), "data": e["data"].as_str().unwrap_or_default(), "at": opened + e["at"].as_f64().unwrap_or(0.0) }))
            .collect();
        json!({ "reports": reports, "dropped": dropped, "interrupted": unplugged })
    }

    fn device_close(&mut self, args: &Json) -> Json {
        let handle = args["connection"].as_str().unwrap_or_default().to_owned();
        let Some(c) = self.extensions.devices.connections.get_mut(&handle) else {
            return json!({ "error": "InvalidStateError", "message": "no such connection" });
        };
        if !c.open {
            return json!({ "error": "InvalidStateError", "message": "already closed" });
        }
        c.open = false;
        c.claimed.clear();
        let (extension, id) = (c.extension.clone(), c.device.clone());
        self.closed(&extension, &id);
        self.log(format!("{extension}: closed {handle}"));
        self.devices_changed(&extension);
        Json::Null
    }

    /// A connection on device `id` closed: the app may open it again once
    /// none is open.
    fn closed(&mut self, extension: &str, id: &str) {
        let still = (self.extensions.devices.connections.values())
            .any(|c| c.open && c.extension == extension && c.device == id);
        let key = format!("{extension} {id}");
        if !still && self.extensions.devices.exclusive.remove(&key) {
            let app = self.device_app();
            let mut state = app.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(open) = state.get_mut(OPEN).and_then(Json::as_object_mut) {
                open.remove(&key);
            }
        }
    }

    fn device_forget(&mut self, row: &ModulesRow, args: &Json) -> Json {
        let by = if args["port"].is_string() {
            "port"
        } else {
            "device"
        };
        let id = args[by].as_str().unwrap_or_default().to_owned();
        let extension = self.plan.str(row.extension).to_owned();
        if let Some(list) = self.extensions.devices.remembered.get_mut(&extension) {
            list.retain(|d| *d != id);
        }
        for c in self.extensions.devices.connections.values_mut() {
            if c.extension == extension && c.device == id {
                c.open = false;
                c.claimed.clear();
            }
        }
        self.closed(&extension, &id);
        self.log(format!("{extension}: forgot {id}"));
        self.devices_changed(&extension);
        Json::Null
    }

    /// The ticket a device name holds, its capability, summary and taps.
    pub(super) fn device_ticket(
        &self,
        row: &ModulesRow,
        origin: CallOrigin,
        call: &Json,
    ) -> (String, Json, Vec<String>) {
        let declared = self.plan.str(row.declared).to_owned();
        match self
            .device_role(row)
            .as_ref()
            .and_then(|d| d["role"].as_str())
        {
            Some("request") => {
                let ids: Vec<String> = (self.devices_of(row).iter())
                    .filter(|d| d["connected"] != false)
                    .filter_map(|d| d["id"].as_str().map(str::to_owned))
                    .collect();
                let args = json!({ "request": declared, "origin": origin.word(), "options": call, "devices": ids });
                ("chooser".into(), args, ids)
            }
            _ => {
                let args = json!({ "capability": self.plan.str(row.extension), "request": declared, "origin": origin.word(), "options": call });
                (
                    "permission".into(),
                    args,
                    vec!["allow".into(), "deny".into()],
                )
            }
        }
    }

    /// The agent answered a device module's ticket held under `token`.
    pub(super) fn device_answered(
        &mut self,
        row: &ModulesRow,
        token: u64,
        args: &Json,
        answer: &HoldAnswer,
    ) {
        let Some(device) = self.device_role(row) else {
            return;
        };
        let extension = self.plan.str(row.extension).to_owned();
        let choice = match answer {
            HoldAnswer::Choice(c) => c.as_str(),
            HoldAnswer::Value(_) => return,
        };
        let out = match (device["role"].as_str(), choice) {
            (Some("request"), "cancel") => device["cancel"].clone(),
            (Some("request"), id) => {
                let Some(fixture) = self.device_fixture(row, id) else {
                    return;
                };
                self.remember(&extension, id);
                self.devices_changed(&extension);
                let record = self.device_record(row, &fixture);
                match device["many"] == true {
                    true => json!([record]),
                    false => record,
                }
            }
            (Some("open"), "allow") => {
                let by = device["by"].as_str().unwrap_or("device");
                let id = args[by].as_str().unwrap_or_default().to_owned();
                // Granted until the device is unplugged (D17).
                let key = format!(
                    "{extension}.{}",
                    self.substitute(row)["devices"]["word"]
                        .as_str()
                        .unwrap_or("devices")
                );
                if let Some(list) = self
                    .extensions
                    .preferred
                    .get_mut(&key)
                    .and_then(Json::as_array_mut)
                {
                    for d in list.iter_mut().filter(|d| d["id"] == id.as_str()) {
                        d["granted"] = json!(true);
                    }
                }
                self.opened(row, &device, args)
            }
            (Some("open"), _) => json!({ "error": "NotAllowedError" }),
            _ => return,
        };
        self.extensions.devices.asking.remove(&token);
        self.release(token, out);
    }

    /// `state.extensions.<module>`'s device fields: the remembered devices,
    /// each connection with its state and cursors, the claims and the writes.
    pub(super) fn devices_json(&self, extension: &str, entry: &mut Json) {
        let d = &self.extensions.devices;
        if let Some(list) = d.remembered.get(extension) {
            entry["remembered"] = json!(list);
        }
        let connections: Vec<Json> = (d.connections.iter())
            .filter(|(_, c)| c.extension == extension)
            .map(|(handle, c)| {
                json!({
                    "handle": handle,
                    "device": c.device,
                    "open": c.open,
                    "claimed": c.claimed,
                    "delivered": c.delivered,
                    "position": c.position,
                    "readers": c.readers,
                })
            })
            .collect();
        if !connections.is_empty() {
            entry["connections"] = json!(connections);
        }
        if let Some(writes) = d.writes.get(extension) {
            entry["writes"] = json!(writes);
        }
        if let Some(sequences) = d.sequences.get(extension) {
            entry["sequences"] = json!(sequences);
        }
    }
}

/// `at` with each `{path}` replaced by the call's value there.
fn template(at: &str, args: &Json) -> String {
    let mut out = String::new();
    let mut rest = at;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let path = &rest[open + 1..open + close];
        let value = path.split('.').fold(args, |v, k| &v[k]);
        match value {
            Json::String(s) => out.push_str(s),
            Json::Number(n) => match n.as_f64() {
                Some(f) if f.fract() == 0.0 => out.push_str(&format!("{}", f as i64)),
                _ => out.push_str(&n.to_string()),
            },
            other => out.push_str(&other.to_string()),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}
