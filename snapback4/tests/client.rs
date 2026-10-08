//! The client against a real Snapback4 server: `snapback4 dev` from the
//! pinned npm package (`bun install` at the checkout root), or
//! `SNAPBACK4_BIN`. Two devices, offline writes kept across a reopen, a
//! refusal withdrawn, and the change poll.

use exact_snapback4::Module;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

const SCHEMA: &str = r#"
use identity
table messages:
  author: principal
  body: text <=50
  at: time
  by byTime: at, id
  public 'messages are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  sync public last 100 by byTime

query inbox():
  return messages last 50 by byTime

mutation send(body: text <=50):
  require body != '' else EMPTY
  row = insert messages { author: viewer, body, at: now }
  return { id: row.id }

-- Only the server knows what is taken: a device cannot predict a claim,
-- and the server refuses a second one.
table taken:
  key: text <=20
  unique byKey: key
  read <- allow
  insert <- allow
  online only

mutation claim(key: text <=20):
  require taken[key = key] = null else TAKEN
  insert taken { key }

-- What the device can refuse itself: another's ticket (a row rule), and an
-- approval by a member it knows is not a lead (a require).
table tickets:
  title: text <=50
  owner: principal
  public 'tickets are shared'
  insert <- .owner = viewer
  update (old, next) <- old.owner = viewer and next.owner = viewer
  delete <- deny
  sync public last 100 by .id

mutation file(title: text <=50):
  return insert tickets { title, owner: viewer }

mutation retitle(t: tickets, title: text <=50):
  row = tickets[t] ! else NOT_FOUND
  update tickets[row.id] { title }
  return null

table members:
  person: principal
  role: enum('member', 'lead')
  unique person
  public 'the directory is public'
  insert <- .person = viewer
  update <- deny
  delete <- deny
  sync public last 100 by .id

mutation join(role: enum('member', 'lead')):
  return insert members { person: viewer, role }

mutation approve(body: text <=50):
  require exists members[person = viewer, role = 'lead'] else NOT_LEAD
  insert messages { author: viewer, body, at: now }
"#;

fn scratch(name: &str) -> PathBuf {
    let mut id = [0u8; 8];
    ibex2::stdlib::crypto::get_random_values(&mut id).unwrap();
    let id: String = id.iter().map(|b| format!("{b:02x}")).collect();
    let dir = std::env::temp_dir().join(format!("exact-snapback4-{name}-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Server {
    child: Child,
    port: u16,
    dir: PathBuf,
}

impl Server {
    fn start() -> Self {
        let binary = std::env::var_os("SNAPBACK4_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../node_modules/snapback4-darwin-arm64/snapback4")
            });
        assert!(
            binary.is_file(),
            "no snapback4 at {}: run `bun install` at the checkout root, or set SNAPBACK4_BIN",
            binary.display()
        );
        let dir = scratch("server");
        std::fs::create_dir_all(dir.join("snapback")).unwrap();
        std::fs::write(dir.join("snapback/schema.q"), SCHEMA).unwrap();
        let mut child = Command::new(binary)
            .args(["dev", "--port", "0", "--memory", "--no-watch"])
            .current_dir(&dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let port = loop {
            let line = lines
                .next()
                .expect("snapback4 dev stopped before it printed its URL")
                .unwrap();
            if let Some(port) = line.strip_prefix("http://127.0.0.1:") {
                break port.trim().parse().unwrap();
            }
        };
        // Keep draining stdout so the server never blocks on a full pipe.
        std::thread::spawn(move || for _ in lines {});
        Server { child, port, dir }
    }

    /// One exchange, as a driver performs a `Step::Fetch`.
    fn fetch(&self, persona: &str, fetch: &Value) -> Value {
        self.fetch_with(&format!("x-snapback-persona: {persona}"), fetch)
    }

    /// One exchange with this credential header (a persona or a bearer).
    fn fetch_with(&self, credential: &str, fetch: &Value) -> Value {
        let method = fetch["method"].as_str().unwrap();
        let path = fetch["path"].as_str().unwrap();
        let body = fetch.get("body").map(Value::to_string).unwrap_or_default();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(stream, "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\n{credential}\r\nconnection: close\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let text = String::from_utf8(raw).unwrap();
        let (head, mut rest) = text.split_once("\r\n\r\n").unwrap();
        let status: u16 = head.split(' ').nth(1).unwrap().parse().unwrap();
        let mut body = String::new();
        if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            while let Some((size, tail)) = rest.split_once("\r\n") {
                let size = usize::from_str_radix(size.trim(), 16).unwrap();
                if size == 0 {
                    break;
                }
                body.push_str(&tail[..size]);
                rest = &tail[size + 2..];
            }
        } else {
            body = rest.into();
        }
        json!({"status": status, "body": serde_json::from_str::<Value>(body.trim()).unwrap_or(Value::Null)})
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Device {
    module: Module,
    dir: PathBuf,
    persona: String,
}

const GRANTS: &str = "sqlite.open app:/data";

impl Device {
    fn new(persona: &str) -> Self {
        let dir = scratch(persona);
        let mut device = Device {
            module: Module::new("test.exact.snapback4", GRANTS).unwrap(),
            dir,
            persona: persona.into(),
        };
        device.configure();
        device
    }

    fn configure(&mut self) {
        self.module
            .configure_storage(
                self.dir.join("data"),
                self.dir.join("cache"),
                self.dir.join("tmp"),
            )
            .unwrap();
    }

    fn reopen(&mut self) {
        self.module = Module::new("test.exact.snapback4", GRANTS).unwrap();
        self.configure();
    }

    fn call(&mut self, request: Value) -> Value {
        let answer = self
            .module
            .call(&request)
            .unwrap_or_else(|e| panic!("{request}: {e}"));
        assert!(answer.get("ok").is_some(), "{request}: {answer}");
        answer["ok"].clone()
    }

    fn open(&mut self, origin: u16) -> bool {
        let viewer = format!("dev:{}", self.persona);
        self.call(json!({"op": "open", "path": "app:/data/inbox.sqlite",
            "origin": format!("http://127.0.0.1:{origin}"), "viewer": viewer}))["opened"]
            .as_bool()
            .unwrap()
    }

    /// Drive one round; `offline` fails every exchange as a lost link would.
    fn sync(&mut self, server: &Server, offline: bool) -> Value {
        self.sync_with(server, |_, real| {
            if offline {
                json!({"error": "offline"})
            } else {
                real()
            }
        })
    }

    /// Drive one round, each exchange's reply chosen by `answer(fetch, real)`;
    /// `real()` performs it against the server.
    fn sync_with(
        &mut self,
        server: &Server,
        mut answer: impl FnMut(&Value, &mut dyn FnMut() -> Value) -> Value,
    ) -> Value {
        let mut step = self.call(json!({"op": "sync"}));
        loop {
            if let Some(done) = step.get("done") {
                return done.clone();
            }
            let fetch = step["fetch"].clone();
            let persona = self.persona.clone();
            let reply = answer(&fetch, &mut || server.fetch(&persona, &fetch));
            step =
                self.call(json!({"op": "deliver", "exchange": fetch["exchange"], "reply": reply}));
        }
    }

    fn poll(&mut self, server: &Server) -> Value {
        let poll = self.call(json!({"op": "changes", "wait": 0}));
        let reply = server.fetch(&self.persona, &poll["fetch"]);
        self.call(json!({"op": "changed", "exchange": poll["fetch"]["exchange"], "reply": reply}))
    }

    fn queued(&mut self) -> Vec<Value> {
        self.call(json!({"op": "status"}))["queued"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    fn inbox(&mut self) -> Vec<Value> {
        let read = self.call(json!({"op": "read", "name": "inbox", "args": {}, "now": 1}));
        read["data"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| panic!("{read}"))
    }

    fn write(&mut self, body: &str, now: i64) -> Value {
        self.call(json!({"op": "write", "name": "send", "args": {"body": body}, "now": now}))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn bodies(rows: &[Value]) -> Vec<(String, bool)> {
    rows.iter()
        .map(|row| {
            (
                row["body"].as_str().unwrap().to_owned(),
                row["pending"] == true,
            )
        })
        .collect()
}

#[test]
fn two_devices_sync_send_offline_reopen_refuse_and_poll() {
    let server = Server::start();
    let mut alice = Device::new("alice");
    let mut bob = Device::new("bob");

    // Never synced: the first round opens on the server's backend.
    assert!(!alice.open(server.port));
    assert!(alice
        .module
        .call(&json!({"op": "write", "name": "send", "args": {"body": "x"}, "now": 1}))
        .is_err());
    assert_eq!(alice.sync(&server, false), json!({"ok": true}));
    assert!(alice.inbox().is_empty());

    // A write is predicted at once, then sent and confirmed by the stream.
    let written = alice.write("hello", now());
    assert_eq!(written["state"], "pending", "{written}");
    let id = written["id"].as_str().unwrap().to_owned();
    assert_eq!(id.len(), 26);
    assert_eq!(bodies(&alice.inbox()), [("hello".into(), true)]);
    assert_eq!(
        alice.call(json!({"op": "outcome", "id": id}))["state"],
        "pending"
    );
    assert_eq!(alice.sync(&server, false), json!({"ok": true}));
    assert_eq!(bodies(&alice.inbox()), [("hello".into(), false)]);
    let outcome = alice.call(json!({"op": "outcome", "id": id}));
    assert_eq!(outcome["state"], "sent", "{outcome}");
    assert_eq!(outcome["result"]["id"], written["newIds"][0], "{outcome}");
    assert_eq!(alice.inbox()[0]["id"], written["newIds"][0]);

    // Another device acquires the partition.
    assert!(!bob.open(server.port));
    assert_eq!(bob.sync(&server, false), json!({"ok": true}));
    assert_eq!(bodies(&bob.inbox()), [("hello".into(), false)]);

    // Offline: the write is kept, survives a reopen, and is sent later.
    let offline = bob.write("from the tunnel", now());
    let done = bob.sync(&server, true);
    assert_eq!(done["ok"], false);
    assert_eq!(done["offline"], true, "{done}");
    bob.reopen();
    assert!(
        bob.open(server.port),
        "a synced partition reopens with no network"
    );
    // Newest first: `last 50 by byTime`.
    assert_eq!(
        bodies(&bob.inbox()),
        [("from the tunnel".into(), true), ("hello".into(), false)]
    );
    assert_eq!(
        bob.call(json!({"op": "status"}))["queued"][0]["id"],
        offline["id"]
    );

    // Alice's poll is current until Bob's write lands.
    assert_eq!(alice.poll(&server), json!(false));
    assert_eq!(bob.sync(&server, false), json!({"ok": true}));
    assert!(bob.call(json!({"op": "status"}))["queued"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(alice.poll(&server), json!(true));
    assert_eq!(alice.sync(&server, false), json!({"ok": true}));
    assert_eq!(
        bodies(&alice.inbox()),
        [("from the tunnel".into(), false), ("hello".into(), false)]
    );

    // A write the server refuses is withdrawn and reported failed.
    let refused = alice.write("", now());
    if refused["state"] == "pending" {
        assert_eq!(alice.sync(&server, false), json!({"ok": true}));
        let outcome = alice.call(json!({"op": "outcome", "id": refused["id"]}));
        assert_eq!(outcome["state"], "failed", "{outcome}");
        assert_eq!(outcome["why"]["code"], "EMPTY", "{outcome}");
    } else {
        assert_eq!(refused["state"], "failed", "{refused}");
    }
    assert_eq!(alice.inbox().len(), 2);
    assert!(alice.call(json!({"op": "status"}))["queued"]
        .as_array()
        .unwrap()
        .is_empty());
    // The refusal is journaled with its input, survives a reopen, and stays
    // until dismissed.
    if refused["state"] == "pending" {
        alice.reopen();
        assert!(alice.open(server.port));
        let journal = alice.call(json!({"op": "refusals"}));
        assert_eq!(journal[0]["id"], refused["id"], "{journal}");
        assert_eq!(journal[0]["why"]["code"], "EMPTY");
        assert_eq!(journal[0]["args"], json!({"body": ""}));
        alice.call(json!({"op": "dismiss", "ids": [refused["id"]]}));
        assert_eq!(alice.call(json!({"op": "refusals"})), json!([]));
    }
}

/// A Rust source's way in: the typed client, each exchange as the runner
/// request a source hands its host (`Answer::Later`), the outcome back.
#[test]
fn a_rust_source_drives_the_typed_client_through_host_requests() {
    use exact_snapback4::{host_reply, host_request, Step};
    let server = Server::start();
    let mut erin = Device::new("erin");
    assert!(!erin.open(server.port));
    let origin = format!("http://127.0.0.1:{}", server.port);
    let headers = [("x-snapback-persona".to_owned(), "erin".to_owned())];
    // The host's side: run the runner request, answer with its outcome.
    let run = |request: exact_runner::Request| {
        let path = request.url.strip_prefix(&origin).unwrap().to_owned();
        let body: Option<Value> =
            (!request.body.is_empty()).then(|| serde_json::from_slice(&request.body).unwrap());
        assert!(request.headers.contains(&headers[0]));
        let mut fetch = json!({"method": request.method, "path": path});
        if let Some(body) = body {
            fetch["body"] = body;
        }
        let reply = server.fetch("erin", &fetch);
        exact_runner::Outcome::Response(exact_runner::Response {
            status: reply["status"].as_u64().unwrap() as u16,
            headers: Vec::new(),
            body: reply["body"].to_string().into_bytes(),
        })
    };
    let round = |device: &mut Device| {
        let (client, host) = device.module.parts();
        let client = client.expect("opened");
        let mut step = client.sync(host);
        loop {
            match step {
                Step::Fetch(fetch) => {
                    let outcome = run(host_request(&fetch, &origin, &headers));
                    step = client.deliver(host, &fetch.exchange, host_reply(&outcome));
                }
                Step::Done(done) => return done,
            }
        }
    };
    assert_eq!(round(&mut erin), json!({"ok": true}));
    let (client, host) = erin.module.parts();
    let written = client
        .unwrap()
        .write(host, "send", json!({"body": "typed"}), now(), None)
        .unwrap();
    assert_eq!(written["state"], "pending");
    assert_eq!(round(&mut erin), json!({"ok": true}));
    let (client, host) = erin.module.parts();
    let client = client.unwrap();
    assert_eq!(
        client
            .outcome(host, written["id"].as_str().unwrap())
            .unwrap()["state"],
        "sent"
    );
    let poll = client.changes(host, 0).unwrap();
    let outcome = run(host_request(&poll, &origin, &headers));
    assert!(!client
        .changed(host, &poll.exchange, host_reply(&outcome))
        .unwrap());
}

fn is_send(fetch: &Value) -> bool {
    fetch["path"]
        .as_str()
        .is_some_and(|path| path.starts_with("/m/"))
}

/// Only the server's own outcome settles a write: an answer that is not one
/// (a proxy's HTML, a gateway's 503, an empty body) or a retryable refusal
/// leaves it queued, latches nothing, and the next round sends it.
#[test]
fn replies_that_are_not_the_servers_outcome_never_settle_a_write() {
    let server = Server::start();
    let mut frank = Device::new("frank");
    frank.open(server.port);
    assert_eq!(frank.sync(&server, false), json!({"ok": true}));
    let written = frank.write("through a captive portal", now());
    let id = written["id"].clone();
    for (status, body, expect) in [
        (200, json!("<html>Sign in to Wi-Fi</html>"), "retry"),
        (200, Value::Null, "retry"),
        (401, json!({"error": "gateway"}), "retry"),
        (200, json!({"denied": {"code": "X"}}), "retry"),
        (
            503,
            json!({"denied": {"code": "E_HTTP_RESPONSE", "family": "link", "message": "gateway"}}),
            "offline",
        ),
        (
            429,
            json!({"denied": {"code": "E_RATE_LIMIT", "family": "bound", "message": "slow down", "retryable": true}}),
            "retry",
        ),
    ] {
        let done = frank.sync_with(&server, |fetch, real| {
            if is_send(fetch) {
                json!({"status": status, "body": body})
            } else {
                real()
            }
        });
        assert_eq!(done["ok"], false, "{status} {body}: {done}");
        assert_eq!(done[expect], true, "{status} {body}: {done}");
        assert_eq!(
            frank.queued()[0]["id"],
            id,
            "{status} {body}: the write stays queued"
        );
        assert_eq!(
            frank.call(json!({"op": "outcome", "id": id}))["state"],
            "pending"
        );
        assert!(
            frank.call(json!({"op": "status"}))["denied"].is_null(),
            "nothing latched"
        );
    }
    assert_eq!(frank.sync(&server, false), json!({"ok": true}));
    assert!(frank.queued().is_empty());
    assert_eq!(
        frank.call(json!({"op": "outcome", "id": id}))["state"],
        "sent"
    );
    assert_eq!(
        bodies(&frank.inbox()),
        [("through a captive portal".into(), false)]
    );
}

/// A reply belongs to its exchange: one for a cancelled round, or for a
/// client since closed and reopened, is refused and changes nothing.
#[test]
fn a_reply_is_delivered_only_to_the_exchange_that_asked() {
    let server = Server::start();
    let mut gina = Device::new("gina");
    gina.open(server.port);
    assert_eq!(gina.sync(&server, false), json!({"ok": true}));
    gina.write("first", now());
    // Round one reaches its send, and the driver goes away.
    let mut step = gina.call(json!({"op": "sync"}));
    while !is_send(&step["fetch"]) {
        let reply = server.fetch("gina", &step["fetch"]);
        step = gina
            .call(json!({"op": "deliver", "exchange": step["fetch"]["exchange"], "reply": reply}));
    }
    let first = step["fetch"].clone();
    assert_eq!(
        gina.call(json!({"op": "cancel", "exchange": "0.0"})),
        json!(false),
        "another exchange cannot cancel it"
    );
    assert_eq!(
        gina.call(json!({"op": "cancel", "exchange": first["exchange"]})),
        json!(true)
    );
    // The client is closed and reopened; round two reaches its own send.
    gina.call(json!({"op": "close"}));
    assert!(gina.open(server.port));
    gina.write("second", now());
    let mut step = gina.call(json!({"op": "sync"}));
    while !is_send(&step["fetch"]) {
        let reply = server.fetch("gina", &step["fetch"]);
        step = gina
            .call(json!({"op": "deliver", "exchange": step["fetch"]["exchange"], "reply": reply}));
    }
    // The first round's late success arrives: refused, nothing settles.
    let late =
        json!({"status": 200, "body": {"state": "sent", "id": first["body"]["id"], "seq": 99}});
    let stale = gina.call(json!({"op": "deliver", "exchange": first["exchange"], "reply": late}));
    assert_eq!(stale["done"]["stale"], true, "{stale}");
    assert_eq!(gina.queued().len(), 2);
    // Round two goes on with its own reply.
    let reply = server.fetch("gina", &step["fetch"]);
    let mut step =
        gina.call(json!({"op": "deliver", "exchange": step["fetch"]["exchange"], "reply": reply}));
    while let Some(fetch) = step.get("fetch").cloned() {
        let reply = server.fetch("gina", &fetch);
        step = gina.call(json!({"op": "deliver", "exchange": fetch["exchange"], "reply": reply}));
    }
    assert_eq!(step["done"], json!({"ok": true}));
    assert!(gina.queued().is_empty());
    let mut rows = bodies(&gina.inbox());
    rows.sort();
    assert_eq!(rows, [("first".into(), false), ("second".into(), false)]);
}

/// A receipt the device kept outranks its outbox: a write the server
/// answered, whose entry a crash left queued, is settled without resending.
#[test]
fn a_kept_receipt_settles_its_write_without_resending() {
    let server = Server::start();
    let mut hana = Device::new("hana");
    hana.open(server.port);
    assert_eq!(hana.sync(&server, false), json!({"ok": true}));
    let written = hana.write("sent once", now());
    let entry = hana.call(json!({"op": "queued"}))[0].clone();
    // The server executes it; the device keeps the receipt; then it dies
    // before settling.
    let state = hana.call(json!({"op": "sync_state", "capture": false}));
    let sent = server.fetch("hana", &json!({"method": "POST", "path": "/m/send",
        "body": {"id": entry["id"], "args": entry["args"], "newIds": entry["new_ids"], "store_id": state["store_id"]}}));
    assert_eq!(sent["body"]["state"], "sent", "{sent}");
    let receipt =
        json!({"state": "sent", "id": entry["id"], "seq": sent["body"]["seq"], "replayed": true});
    hana.call(json!({"op": "keep_write", "value": receipt.to_string()}));
    hana.reopen();
    assert!(hana.open(server.port));
    assert_eq!(
        hana.call(json!({"op": "outcome", "id": written["id"]}))["state"],
        "sent",
        "the receipt outranks the queue"
    );
    let mut sends = 0;
    let done = hana.sync_with(&server, |fetch, real| {
        sends += usize::from(is_send(fetch));
        real()
    });
    assert_eq!(done, json!({"ok": true}));
    assert_eq!(sends, 0, "a write with a kept receipt is never sent again");
    assert!(hana.queued().is_empty());
    assert_eq!(bodies(&hana.inbox()), [("sent once".into(), false)]);
}

/// Clocks may carry fractions; a write needs one. Every outbox entry names
/// the opened viewer, and the device's identity cannot be replaced.
#[test]
fn clocks_viewers_and_identities_are_checked() {
    let server = Server::start();
    let mut ida = Device::new("ida");
    ida.open(server.port);
    assert_eq!(ida.sync(&server, false), json!({"ok": true}));
    let at = now() as f64 + 0.625;
    let written =
        ida.call(json!({"op": "write", "name": "send", "args": {"body": "fractional"}, "now": at}));
    assert_eq!(written["state"], "pending");
    assert_eq!(ida.inbox()[0]["at"], json!(at.floor() as i64));
    assert!(ida
        .module
        .call(&json!({"op": "write", "name": "send", "args": {"body": "x"}}))
        .is_err());
    assert!(ida
        .module
        .call(
            &json!({"op": "enqueue", "entry": {"id": "x", "seq": 99, "op": "send",
        "args": {}, "viewer": "service:ai", "now": 1, "new_ids": [], "predicted": []}})
        )
        .is_err());
    assert!(ida
        .module
        .call(&json!({"op": "set_meta", "key": "exact:device", "value": "chosen"}))
        .is_err());
}

/// A change poll's reply counts once, and only for the newest poll.
#[test]
fn a_poll_reply_counts_once_and_only_for_the_newest_poll() {
    let server = Server::start();
    let mut kai = Device::new("kai");
    kai.open(server.port);
    assert_eq!(kai.sync(&server, false), json!({"ok": true}));
    let first = kai.call(json!({"op": "changes", "wait": 0}))["fetch"].clone();
    let second = kai.call(json!({"op": "changes", "wait": 0}))["fetch"].clone();
    assert_ne!(first["exchange"], second["exchange"]);
    let late = json!({"error": "the first poll's link dropped"});
    assert!(
        kai.module
            .call(&json!({"op": "changed", "exchange": first["exchange"], "reply": late}))
            .is_err(),
        "superseded"
    );
    let reply = server.fetch("kai", &second);
    assert_eq!(
        kai.call(json!({"op": "changed", "exchange": second["exchange"], "reply": reply.clone()})),
        json!(false)
    );
    assert!(
        kai.module
            .call(&json!({"op": "changed", "exchange": second["exchange"], "reply": reply}))
            .is_err(),
        "already answered"
    );
    assert_eq!(kai.call(json!({"op": "status"}))["online"], true);
}

/// A write only the server can refuse is journaled with its input, survives
/// a reopen, and stays until the app dismisses it.
#[test]
fn a_refused_write_is_journaled_with_its_input_until_dismissed() {
    let server = Server::start();
    let mut lena = Device::new("lena");
    lena.open(server.port);
    assert_eq!(lena.sync(&server, false), json!({"ok": true}));
    let claim = |device: &mut Device, key: &str| {
        device.call(json!({"op": "write", "name": "claim", "args": {"key": key}, "now": now()}))
    };
    let first = claim(&mut lena, "corner office");
    let second = claim(&mut lena, "corner office");
    assert_eq!(first["state"], "pending");
    assert_eq!(
        second["state"], "pending",
        "the device cannot know it is taken"
    );
    assert_eq!(lena.sync(&server, false), json!({"ok": true}));
    assert_eq!(
        lena.call(json!({"op": "outcome", "id": first["id"]}))["state"],
        "sent"
    );
    let outcome = lena.call(json!({"op": "outcome", "id": second["id"]}));
    assert_eq!(outcome["state"], "failed", "{outcome}");
    lena.reopen();
    assert!(lena.open(server.port));
    let journal = lena.call(json!({"op": "refusals"}));
    assert_eq!(journal.as_array().unwrap().len(), 1, "{journal}");
    assert_eq!(journal[0]["id"], second["id"]);
    assert_eq!(journal[0]["op"], "claim");
    assert_eq!(journal[0]["args"], json!({"key": "corner office"}));
    assert_eq!(journal[0]["why"]["code"], "TAKEN");
    lena.call(json!({"op": "dismiss", "ids": [second["id"]]}));
    assert_eq!(lena.call(json!({"op": "refusals"})), json!([]));
}

/// A write the device refuses (a row rule, a `require` over synced rows) is
/// failed at once, as Snapback's own client fails it: never sent, answered
/// by `outcome` and listed by `refusals` like a server's refusal, after a
/// reopen too, and its id is never given to another write. A keyed write is
/// not refused by a prediction: the server decides it.
#[test]
fn a_write_the_device_refuses_is_failed_and_journaled_never_sent() {
    let server = Server::start();
    let mut ned = Device::new("ned");
    ned.open(server.port);
    assert_eq!(ned.sync(&server, false), json!({"ok": true}));
    let filed = ned
        .call(json!({"op": "write", "name": "file", "args": {"title": "disk full"}, "now": now()}));
    assert_eq!(ned.sync(&server, false), json!({"ok": true}));
    let ticket = ned.call(json!({"op": "outcome", "id": filed["id"]}))["result"]["id"].clone();
    assert!(ticket.is_string());

    let mut pia = Device::new("pia");
    pia.open(server.port);
    assert_eq!(pia.sync(&server, false), json!({"ok": true}));
    pia.call(json!({"op": "write", "name": "join", "args": {"role": "member"}, "now": now()}));
    assert_eq!(pia.sync(&server, false), json!({"ok": true}));
    let retitled = pia.call(json!({"op": "write", "name": "retitle",
        "args": {"t": ticket, "title": "mine now"}, "now": now()}));
    assert_eq!(retitled["state"], "failed", "{retitled}");
    assert_eq!(retitled["why"]["code"], "E_RULE");
    let approved =
        pia.call(json!({"op": "write", "name": "approve", "args": {"body": "ok"}, "now": now()}));
    assert_eq!(approved["state"], "failed", "{approved}");
    assert_eq!(approved["why"]["code"], "NOT_LEAD");
    let next = pia.write("after two refusals", now());
    assert_eq!(next["state"], "pending");
    let ids = [&retitled["id"], &approved["id"], &next["id"]];
    assert!(
        ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2],
        "{ids:?}"
    );
    for (write, code) in [(&retitled, "E_RULE"), (&approved, "NOT_LEAD")] {
        let outcome = pia.call(json!({"op": "outcome", "id": write["id"]}));
        assert_eq!(outcome["state"], "failed", "{outcome}");
        assert_eq!(outcome["why"]["code"], code);
    }
    let mut sent = Vec::new();
    let done = pia.sync_with(&server, |fetch, real| {
        sent.push(fetch["path"].as_str().unwrap_or_default().to_owned());
        real()
    });
    assert_eq!(done, json!({"ok": true}));
    let sends: Vec<_> = sent.iter().filter(|path| path.starts_with("/m/")).collect();
    assert_eq!(sends, ["/m/send"], "the refused writes are never sent");

    pia.reopen();
    assert!(pia.open(server.port));
    let journal = pia.call(json!({"op": "refusals"}));
    let listed: Vec<_> = journal
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["op"].clone(), r["why"]["code"].clone()))
        .collect();
    assert_eq!(
        listed,
        [
            (json!("retitle"), json!("E_RULE")),
            (json!("approve"), json!("NOT_LEAD"))
        ],
        "{journal}"
    );
    assert_eq!(journal[0]["args"]["title"], "mine now");
    pia.call(json!({"op": "dismiss"}));
    assert_eq!(pia.call(json!({"op": "refusals"})), json!([]));
    let outcome = pia.call(json!({"op": "outcome", "id": approved["id"]}));
    assert_eq!(
        outcome["state"], "failed",
        "dismissed, still failed: {outcome}"
    );

    let keyed = pia.call(
        json!({"op": "write", "name": "approve", "args": {"body": "ok"},
        "now": now(), "key": "approve:1"}),
    );
    assert_eq!(keyed["state"], "pending", "{keyed}");
    assert_eq!(pia.sync(&server, false), json!({"ok": true}));
    let outcome = pia.call(json!({"op": "outcome", "id": keyed["id"]}));
    assert_eq!(outcome["state"], "failed", "{outcome}");
    assert_eq!(outcome["why"]["code"], "NOT_LEAD");
}

/// An idempotency key names one intent: writing it again admits nothing and
/// answers what became of the first, before and after it is sent.
#[test]
fn a_keyed_write_is_admitted_once_whatever_is_asked_again() {
    let server = Server::start();
    let mut mona = Device::new("mona");
    mona.open(server.port);
    assert_eq!(mona.sync(&server, false), json!({"ok": true}));
    let write = |device: &mut Device| {
        device.call(json!({"op": "write", "name": "send", "args": {"body": "once"}, "now": now(), "key": "draft:7"}))
    };
    let first = write(&mut mona);
    assert_eq!(first["state"], "pending");
    let id = mona.call(json!({"op": "write_id", "key": "draft:7"}));
    assert_eq!(id, first["id"]);
    let again = write(&mut mona);
    assert_eq!(again["id"], first["id"]);
    assert_eq!(again["state"], "pending");
    assert_eq!(mona.queued().len(), 1, "admitted once");
    // The same key with other input is a reuse, refused.
    let other = mona.call(json!({"op": "write", "name": "send", "args": {"body": "twice"}, "now": now(), "key": "draft:7"}));
    assert_eq!(other["state"], "failed");
    assert_eq!(other["why"]["code"], "E_WRITE_ID_REUSE");
    assert_eq!(mona.queued().len(), 1);
    // That refusal is the call's answer: the id still names the first write.
    assert_eq!(
        mona.call(json!({"op": "outcome", "id": first["id"]}))["state"],
        "pending"
    );
    assert_eq!(mona.call(json!({"op": "refusals"})), json!([]));
    assert_eq!(mona.sync(&server, false), json!({"ok": true}));
    let after = write(&mut mona);
    assert_eq!(after["state"], "sent", "{after}");
    assert!(mona.queued().is_empty());
    // Sent, its receipt still names its input: other input is still a reuse,
    // and the same input still answers the first, after a reopen too.
    mona.reopen();
    assert!(mona.open(server.port));
    let other = mona.call(json!({"op": "write", "name": "send", "args": {"body": "twice"}, "now": now(), "key": "draft:7"}));
    assert_eq!(other["why"]["code"], "E_WRITE_ID_REUSE", "{other}");
    assert_eq!(write(&mut mona)["state"], "sent");
    assert!(mona.queued().is_empty());
    let mine: Vec<_> = mona
        .inbox()
        .into_iter()
        .filter(|row| row["body"] == "once")
        .collect();
    assert_eq!(mine.len(), 1);
}

/// One client per module: opening another, even before the first has
/// synced, is refused until the first is closed.
#[test]
fn a_second_open_is_refused_until_the_first_client_is_closed() {
    let server = Server::start();
    let mut nia = Device::new("nia");
    assert!(!nia.open(server.port), "never synced");
    let other = json!({"op": "open", "path": "app:/data/other.sqlite",
        "origin": format!("http://127.0.0.1:{}", server.port), "viewer": "dev:nia"});
    assert!(nia.module.call(&other).is_err());
    nia.call(json!({"op": "close"}));
    assert_eq!(nia.call(other)["opened"], false);
}

/// The journal keeps every refusal until it is dismissed.
#[test]
fn the_refusal_journal_keeps_every_refusal_until_dismissed() {
    let server = Server::start();
    let mut ola = Device::new("ola");
    ola.open(server.port);
    assert_eq!(ola.sync(&server, false), json!({"ok": true}));
    for _ in 0..102 {
        ola.call(
            json!({"op": "write", "name": "claim", "args": {"key": "only one"}, "now": now()}),
        );
    }
    assert_eq!(ola.sync(&server, false), json!({"ok": true}));
    let journal = ola.call(json!({"op": "refusals"}));
    assert_eq!(journal.as_array().unwrap().len(), 101);
    // Kept in bounded segments: dismissing one leaves the other hundred.
    let second = journal[64]["id"].clone();
    ola.call(json!({"op": "dismiss", "ids": [second]}));
    let rest = ola.call(json!({"op": "refusals"}));
    assert_eq!(rest.as_array().unwrap().len(), 100);
    assert!(rest.as_array().unwrap().iter().all(|r| r["id"] != second));
    ola.reopen();
    assert!(ola.open(server.port));
    assert_eq!(
        ola.call(json!({"op": "refusals"}))
            .as_array()
            .unwrap()
            .len(),
        100
    );
    ola.call(json!({"op": "dismiss"}));
    assert_eq!(ola.call(json!({"op": "refusals"})), json!([]));
}

/// A keyed write the server refused stays known by its refusal after its
/// receipt is gone, and keeps its key bound to the input it carried; a
/// dismissal that matches nothing leaves the journal as it was.
#[test]
fn a_refusal_outlives_its_receipt() {
    let server = Server::start();
    let mut pia = Device::new("pia");
    pia.open(server.port);
    assert_eq!(pia.sync(&server, false), json!({"ok": true}));
    let claim = |device: &mut Device, key: &str, write: &str| {
        device.call(json!({"op": "write", "name": "claim", "args": {"key": key}, "now": now(), "key": write}))
    };
    assert_eq!(claim(&mut pia, "desk", "first")["state"], "pending");
    let refused = claim(&mut pia, "desk", "second");
    assert_eq!(pia.sync(&server, false), json!({"ok": true}));
    // The receipt window moves on; the journal still has the refusal.
    pia.call(json!({"op": "set_meta", "key": "write-history", "value": "[]"}));
    assert_eq!(
        pia.call(json!({"op": "meta", "key": "write-history"})),
        "[]"
    );
    pia.reopen();
    assert!(pia.open(server.port));
    let outcome = pia.call(json!({"op": "outcome", "id": refused["id"]}));
    assert_eq!(outcome["state"], "failed", "{outcome}");
    assert_eq!(outcome["why"]["code"], "TAKEN");
    let reused = claim(&mut pia, "window", "second");
    assert_eq!(reused["why"]["code"], "E_WRITE_ID_REUSE", "{reused}");
    assert_eq!(claim(&mut pia, "desk", "second")["state"], "failed");
    assert!(pia.queued().is_empty(), "nothing admitted");
    pia.call(json!({"op": "dismiss", "ids": []}));
    pia.call(json!({"op": "dismiss", "ids": ["00000000000000000000000000"]}));
    assert_eq!(
        pia.call(json!({"op": "refusals"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// A session refresh hands back a replacement for the bearer it was sent
/// with, retires the old one, and is refused when it names someone else.
#[test]
fn a_session_refreshes_once_and_only_as_its_own_principal() {
    let server = Server::start();
    let guest = server.fetch_with(
        "x-snapback-guest: 1",
        &json!({"method": "POST", "path": "/auth/guest"}),
    );
    let session = guest["body"]["session"].clone();
    let token = session["token"]
        .as_str()
        .unwrap_or_else(|| panic!("{guest}"))
        .to_owned();
    let principal = session["principal"].as_str().unwrap().to_owned();
    let mut rae = Device::new("rae");
    rae.call(json!({"op": "open", "path": "app:/data/inbox.sqlite",
        "origin": format!("http://127.0.0.1:{}", server.port), "viewer": principal}));
    let bearer = |token: &str| format!("authorization: Bearer {token}");
    let refresh = rae.call(json!({"op": "refresh"}))["fetch"].clone();
    assert_eq!(refresh["path"], "/auth/refresh");
    // One at a time: a second would present the token the first retires.
    let second = rae.module.call(&json!({"op": "refresh"}));
    assert!(
        second.as_ref().is_err_and(|e| e.starts_with("E_BUSY")),
        "{second:?}"
    );
    let reply = server.fetch_with(&bearer(&token), &refresh);
    let done = rae.call(
        json!({"op": "refreshed", "exchange": refresh["exchange"], "reply": reply, "now": now()}),
    );
    assert_eq!(done["ok"], true, "{done}");
    let fresh = done["session"]["token"].as_str().unwrap().to_owned();
    assert_ne!(fresh, token);
    assert_eq!(done["session"]["principal"], principal.as_str());
    let schema = json!({"method": "GET", "path": "/schema"});
    assert_eq!(
        server.fetch_with(&bearer(&token), &schema)["status"],
        401,
        "the old bearer is retired"
    );
    assert_eq!(server.fetch_with(&bearer(&fresh), &schema)["status"], 200);
    // The same reply twice is stale; a session for another principal is refused.
    let again = rae.call(
        json!({"op": "refreshed", "exchange": refresh["exchange"], "reply": reply, "now": now()}),
    );
    assert_eq!(again["denied"]["code"], "E_STALE");
    let mut sam = Device::new("sam");
    sam.open(server.port);
    let refresh = sam.call(json!({"op": "refresh"}))["fetch"].clone();
    let reply = server.fetch_with(&bearer(&fresh), &refresh);
    let other = sam.call(
        json!({"op": "refreshed", "exchange": refresh["exchange"], "reply": reply, "now": now()}),
    );
    assert_eq!(other["denied"]["code"], "E_AUTH", "{other}");
    // A retired bearer cannot refresh.
    let refresh = rae.call(json!({"op": "refresh"}))["fetch"].clone();
    let reply = server.fetch_with(&bearer(&token), &refresh);
    let dead = rae.call(
        json!({"op": "refreshed", "exchange": refresh["exchange"], "reply": reply, "now": now()}),
    );
    assert_eq!(dead["denied"]["code"], "E_AUTH", "{dead}");
    // A refresh whose reply never comes is cancelled, and the next may go.
    let lost = rae.call(json!({"op": "refresh"}))["fetch"].clone();
    assert_eq!(
        rae.call(json!({"op": "cancel", "exchange": lost["exchange"]})),
        true
    );
    assert!(rae.call(json!({"op": "refresh"}))["fetch"].is_object());
}
