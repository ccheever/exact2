//! Anonymous health and funnel events, sent as OpenTelemetry logs (OTLP/HTTP
//! JSON) to Fleet's cloud, which forwards them to Axiom. Each event is a name
//! and a few scalar attributes: what happened and why, never message or
//! transcript text, session titles, machine names, ids or tokens. The install
//! id is random and kept on this phone only.

use serde_json::{json, Value as Json};

/// The app's version and build, as the archive's Info.plist has them
/// (`EXACT_BUILD_NUMBER` at build time; "dev" in a development build).
pub fn build() -> String {
    format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        option_env!("EXACT_BUILD_NUMBER").unwrap_or("dev")
    )
}

/// Where batches go: Fleet's cloud Worker (`cloud/src/telemetry.js`).
pub const ENDPOINT: &str = "https://fleet-service.eliot-4cd.workers.dev/v1/telemetry";

/// A batch is sent this often, or sooner when `FLUSH_AT` events wait.
const FLUSH_MS: f64 = 30_000.0;
const FLUSH_AT: usize = 20;
/// Events kept while sending fails; the oldest go first.
const KEEP: usize = 200;

/// An attribute's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    /// Text (short, never user content).
    Text(String),
    /// A whole number.
    Int(i64),
    /// A flag.
    Bool(bool),
}

impl From<&str> for Attr {
    fn from(s: &str) -> Self {
        Attr::Text(s.to_string())
    }
}
impl From<String> for Attr {
    fn from(s: String) -> Self {
        Attr::Text(s)
    }
}
impl From<i64> for Attr {
    fn from(n: i64) -> Self {
        Attr::Int(n)
    }
}
impl From<bool> for Attr {
    fn from(b: bool) -> Self {
        Attr::Bool(b)
    }
}

#[derive(Clone, Debug)]
struct Event {
    name: &'static str,
    warn: bool,
    at_ms: f64,
    attrs: Vec<(&'static str, Attr)>,
}

/// The queue and its sending.
#[derive(Default)]
pub struct Telemetry {
    /// This install's random id, once loaded or made.
    pub install: String,
    queue: Vec<Event>,
    /// Events out in the request now.
    sending: Vec<Event>,
    last_flush: f64,
}

impl Telemetry {
    /// Record an event at `now` (ms since the epoch).
    pub fn track(
        &mut self,
        now: f64,
        name: &'static str,
        warn: bool,
        attrs: Vec<(&'static str, Attr)>,
    ) {
        self.queue.push(Event {
            name,
            warn,
            at_ms: now,
            attrs,
        });
        if self.queue.len() > KEEP {
            let extra = self.queue.len() - KEEP;
            self.queue.drain(..extra);
        }
    }

    /// Whether a batch should go now.
    pub fn due(&self, now: f64) -> bool {
        self.sending.is_empty()
            && !self.install.is_empty()
            && !self.queue.is_empty()
            && (self.queue.len() >= FLUSH_AT || now - self.last_flush >= FLUSH_MS)
    }

    /// The batch to send, as an OTLP logs body; the events move out until
    /// `done`.
    pub fn take(&mut self, now: f64, version: &str) -> Option<String> {
        if self.queue.is_empty() || !self.sending.is_empty() {
            return None;
        }
        self.last_flush = now;
        self.sending = std::mem::take(&mut self.queue);
        let records: Vec<Json> = self.sending.iter().map(record).collect();
        Some(
            json!({ "resourceLogs": [{
                "resource": { "attributes": [
                    kv("service.name", &Attr::from("ocho-ios")),
                    kv("service.version", &Attr::from(version)),
                    kv("install.id", &Attr::Text(self.install.clone())),
                ]},
                "scopeLogs": [{ "scope": { "name": "ocho" }, "logRecords": records }],
            }]})
            .to_string(),
        )
    }

    /// The batch landed (dropped by a sink without a token counts too), or
    /// it comes back to the front of the queue.
    pub fn done(&mut self, ok: bool) {
        let sent = std::mem::take(&mut self.sending);
        if !ok {
            let later = std::mem::take(&mut self.queue);
            self.queue = sent;
            self.queue.extend(later);
            if self.queue.len() > KEEP {
                let extra = self.queue.len() - KEEP;
                self.queue.drain(..extra);
            }
        }
    }

    /// Events waiting, for tests.
    pub fn waiting(&self) -> usize {
        self.queue.len()
    }
}

/// A random-enough install id from the clock and a per-process counter:
/// unique per phone, meaningless outside telemetry.
pub fn new_install_id(now: f64, salt: u64) -> String {
    let mut x = (now as u64) ^ salt.rotate_left(29) ^ 0x9E37_79B9_7F4A_7C15;
    let mut out = String::new();
    for _ in 0..2 {
        // splitmix64
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        out.push_str(&format!("{:016x}", z ^ (z >> 31)));
    }
    out
}

fn kv(key: &str, value: &Attr) -> Json {
    let value = match value {
        Attr::Text(s) => json!({ "stringValue": s }),
        Attr::Int(n) => json!({ "intValue": n.to_string() }),
        Attr::Bool(b) => json!({ "boolValue": b }),
    };
    json!({ "key": key, "value": value })
}

fn record(e: &Event) -> Json {
    json!({
        "timeUnixNano": format!("{}", (e.at_ms.max(0.0) as u64).saturating_mul(1_000_000)),
        "severityText": if e.warn { "WARN" } else { "INFO" },
        "body": { "stringValue": e.name },
        "attributes": e.attrs.iter().map(|(k, v)| kv(k, v)).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_are_otlp_logs_and_failures_keep_their_events() {
        let mut t = Telemetry {
            install: "abc".into(),
            ..Telemetry::default()
        };
        t.track(
            1_000.0,
            "poll.failure",
            true,
            vec![("kind", "timeout".into()), ("status", 0i64.into())],
        );
        assert!(t.due(40_000.0));
        let body: Json = serde_json::from_str(&t.take(40_000.0, "0.1.0").unwrap()).unwrap();
        let r = &body["resourceLogs"][0];
        assert_eq!(
            r["resource"]["attributes"][0]["value"]["stringValue"],
            "ocho-ios"
        );
        let rec = &r["scopeLogs"][0]["logRecords"][0];
        assert_eq!(rec["body"]["stringValue"], "poll.failure");
        assert_eq!(rec["severityText"], "WARN");
        assert_eq!(rec["timeUnixNano"], "1000000000");
        assert_eq!(rec["attributes"][1]["value"]["intValue"], "0");
        // Sending fails: the event waits for the next batch, before newer ones.
        t.track(41_000.0, "app.launch", false, vec![]);
        t.done(false);
        assert_eq!(t.waiting(), 2);
        let again = t.take(80_000.0, "0.1.0").unwrap();
        assert!(again.find("poll.failure").unwrap() < again.find("app.launch").unwrap());
    }

    #[test]
    fn install_ids_differ() {
        assert_ne!(new_install_id(1.0, 1), new_install_id(1.0, 2));
        assert_eq!(new_install_id(5.0, 9).len(), 32);
    }
}
