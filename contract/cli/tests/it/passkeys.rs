//! `@exact/passkeys` under its declared substitute (LLP 1133.011 §9), the
//! package's own `exact.json`: a ceremony is data code's alone, admitted by
//! its RP ID's grant, held under a `credential` ticket whose args a software
//! authenticator signs over, one sheet at a time, its PRF output withheld,
//! and a registration dropped before data code had it disposed of (D11).

use exact_runner::{
    Answer, CallOrigin, DataError, DataSource, Dispatch, HoldAnswer, Outcome, Reply, Request,
    Response, Runner, Store, Value, Work,
};
use serde_json::{json, Value as Json};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

const APP: &str = r#"use getClientCapabilities, ClientCapabilities from "@exact/passkeys"

shape Result
  ok: bool
  message: string

component App
  resource caps = getClientCapabilities() as shape ClientCapabilities
  mutation passkey as shape Result
  action register
    send passkey = register()
  action idle
    send passkey = idle()
  view
    column
      text "passkeys"
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-passkeys-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/passkeys");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/passkeys");
        std::fs::create_dir_all(&package).unwrap();
        for file in ["package.json", "exact.json"] {
            std::fs::copy(source.join(file), package.join(file)).unwrap();
        }
        for platform in ["apple", "android", "web"] {
            std::fs::create_dir_all(package.join(platform)).unwrap();
            std::fs::copy(
                source.join(platform).join("exact.json"),
                package.join(platform).join("exact.json"),
            )
            .unwrap();
        }
        std::fs::write(path.join("app.contract"), contract).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The body data code's `credentials.create` sends for `rp_id`.
fn create(rp_id: Option<&str>) -> Json {
    let mut public_key = json!({ "rp": { "name": "Fixture" }, "challenge": "Y2hhbGxlbmdl",
        "user": { "id": "dXNlcg", "name": "fixture.test", "displayName": "fixture.test" },
        "pubKeyCredParams": [{ "type": "public-key", "alg": -7 }] });
    if let Some(rp) = rp_id {
        public_key["rp"]["id"] = json!(rp);
    }
    json!({ "extension": "passkeys", "send": "create", "args": { "publicKey": public_key } })
}

fn get(mediation: &str) -> Json {
    json!({ "extension": "passkeys", "send": "get", "args": {
        "publicKey": { "challenge": "Z2V0", "rpId": "auth-fixture.exact.test", "allowCredentials": [] },
        "mediation": mediation } })
}

/// A registration's response, as `authenticator.mjs` makes one, with a PRF output.
fn registration(id: &str) -> String {
    json!({ "id": id, "rawId": id, "type": "public-key",
        "clientExtensionResults": { "prf": { "enabled": true, "results": { "first": "c2VjcmV0" } } },
        "response": { "clientDataJSON": "e30", "attestationObject": "oA" } })
    .to_string()
}

/// The app's data: `register` calls `credentials.create` as data code
/// would, and keeps each reply it read.
#[derive(Clone, Default)]
struct Data(Arc<Mutex<Vec<Json>>>);
impl DataSource for Data {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "idle" => Ok(Value::record(vec![Value::Bool(true), Value::str("")])),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
    fn answer(&mut self, _: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        match source {
            "register" => Ok(Answer::Later(Request::native(
                create(Some("auth-fixture.exact.test"))
                    .to_string()
                    .into_bytes(),
            ))),
            _ => self.query(source, args).map(Answer::Now),
        }
    }
    fn parse(
        &mut self,
        _: &mut Store,
        _: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        let Outcome::Response(Response { body, .. }) = outcome else {
            return Err(DataError::Unavailable("no reply".into()));
        };
        let reply: Json = serde_json::from_slice(&body).unwrap();
        self.0.lock().unwrap().push(reply.clone());
        let ok = reply["error"].is_null();
        let message = reply["error"].as_str().unwrap_or_default().to_owned();
        Ok(Answer::Now(Value::record(vec![
            Value::Bool(ok),
            Value::str(&message),
        ])))
    }
    fn grants(&self) -> &str {
        "auth.passkey auth-fixture.exact.test\nauth.passkey localhost\n"
    }
}

fn boot(name: &str) -> (App, Runner<Data>, Data) {
    let app = App::new(name, APP);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let data = Data::default();
    let r = Runner::boot(
        plan,
        data.clone(),
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    (app, r, data)
}

/// This target with a credential manager, as a Mac with a profile would be.
fn available(r: &mut Runner<Data>) {
    let (_, out) = r
        .extension_agent(r#"{"op":"prefer","extension":{"passkeys.getClientCapabilities":{"available":true,"conditionalGet":true}}}"#)
        .unwrap();
    out.unwrap();
}

/// The host: the runner, and the requests whose replies a ticket's answer sends.
struct Host {
    r: Runner<Data>,
    waiting: Vec<(u64, mpsc::Receiver<Outcome>)>,
}

impl Host {
    /// Each request run, each reply that came fulfilled, until nothing is left.
    fn pump(&mut self) {
        for _ in 0..8 {
            let outs = self.r.take_requests();
            let mut moved = !outs.is_empty() | !self.r.apply_announced().0.is_empty();
            for out in outs {
                let (tx, rx) = mpsc::channel();
                match self.r.native_work(&out.request) {
                    Dispatch::Run(Work::Now(work)) => tx.send(work()).unwrap(),
                    Dispatch::Run(Work::Later(work)) => {
                        work(Reply::new(move |o| tx.send(o).unwrap()))
                    }
                    _ => continue,
                }
                self.waiting.push((out.ticket, rx));
            }
            for (ticket, rx) in std::mem::take(&mut self.waiting) {
                match rx.try_recv() {
                    Ok(outcome) => {
                        moved = true;
                        let _ = self.r.fulfill(ticket, outcome);
                    }
                    Err(_) => self.waiting.push((ticket, rx)),
                }
            }
            if !moved {
                return;
            }
        }
    }

    /// The `credential` tickets held, oldest first: each ticket and its args.
    fn tickets(&self) -> Vec<(u64, Json)> {
        (self.r.device_holds().iter())
            .filter(|h| h.capability == "credential")
            .map(|h| (h.ticket, serde_json::from_str(&h.args).unwrap()))
            .collect()
    }

    fn logs(&self) -> String {
        exact_runner::agent::handle(&self.r, r#"{"op":"logs"}"#)
    }
}

/// A data-code call of `body`: the receiver of its reply, now or once a
/// ticket's answer sends it.
fn call(r: &mut Runner<Data>, body: &Json) -> mpsc::Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    match r.native_work(&Request::native(body.to_string().into_bytes())) {
        Dispatch::Run(Work::Now(work)) => tx.send(work()).unwrap(),
        Dispatch::Run(Work::Later(work)) => work(Reply::new(move |o| tx.send(o).unwrap())),
        _ => panic!("a module call is work"),
    }
    rx
}

fn json_of(outcome: Outcome) -> Json {
    let Outcome::Response(Response { body, .. }) = outcome else {
        panic!("an answer");
    };
    serde_json::from_slice(&body).unwrap()
}

/// D12: Contract may not call a ceremony; the compiler says where to.
#[test]
fn a_contract_call_of_a_ceremony_is_refused() {
    let source = "use create from \"@exact/passkeys\"\n\ncomponent App\n  view\n    column\n      text \"x\"\n";
    let app = App::new("data-only", source);
    let errors = contract::compile_path_all(&app.0.join("app.contract"), false)
        .err()
        .map(|e| format!("{e:?}"))
        .unwrap_or_default();
    assert!(errors.contains("module-name-data-only"), "{errors}");
    assert!(errors.contains("app.ts"), "{errors}");
}

/// §9: a ceremony holds a `credential` ticket naming what an authenticator
/// signs over; a typed response answers it with its PRF output withheld.
#[test]
fn a_ceremony_is_a_ticket_and_its_answer_keeps_no_prf_output() {
    let (_app, r, data) = boot("ticket");
    let mut host = Host {
        r,
        waiting: Vec::new(),
    };
    available(&mut host.r);
    host.r.act("register", vec![]).unwrap();
    host.pump();
    let tickets = host.tickets();
    assert_eq!(tickets.len(), 1, "{}", host.logs());
    let (ticket, args) = &tickets[0];
    assert_eq!(args["ceremony"], "create");
    assert_eq!(args["mediation"], "optional");
    assert_eq!(args["rpId"], "auth-fixture.exact.test");
    assert_eq!(args["options"]["challenge"], "Y2hhbGxlbmdl");
    assert_eq!(args["origin"], "app");
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        assert_eq!(args["clientOrigin"], "https://auth-fixture.exact.test");
    }
    // Not a response: refused, and the ticket stays.
    let refused = host.r.answer_hold(*ticket, &HoldAnswer::Value("{}".into()));
    assert!(refused.is_err());
    let response = HoldAnswer::Value(registration("cred-1"));
    host.r.answer_hold(*ticket, &response).unwrap();
    host.pump();
    let replies = data.0.lock().unwrap().clone();
    assert_eq!(replies.len(), 1, "{}", host.logs());
    assert_eq!(replies[0]["id"], "cred-1");
    assert_eq!(
        replies[0]["clientExtensionResults"]["prf"],
        json!({ "enabled": true })
    );
    // The journal and the state name the ceremony and its outcome, never the response.
    let logs = host.logs();
    assert!(
        !logs.contains("c2VjcmV0") && !logs.contains("Y2hhbGxlbmdl"),
        "{logs}"
    );
    let extensions: Json = serde_json::from_str(&host.r.extensions_json().unwrap()).unwrap();
    let ceremonies = &extensions["passkeys"]["ceremonies"];
    assert_eq!(ceremonies[0]["name"], "create", "{extensions}");
    assert_eq!(ceremonies[0]["outcome"], "answered", "{extensions}");
    assert!(!extensions.to_string().contains("cred-1"), "{extensions}");
}

/// D6: the RP ID must be a granted line's target; natively an omitted one,
/// or `localhost`, has no association. Each fails before any ticket.
#[test]
fn an_rp_id_the_grants_do_not_name_fails_security_error_at_once() {
    let (_app, mut r, _) = boot("grant");
    available(&mut r);
    for rp in [Some("evil.example"), None, Some("localhost")] {
        let answer = json_of(call(&mut r, &create(rp)).recv().unwrap());
        assert_eq!(answer["error"], "SecurityError", "{rp:?}: {answer}");
    }
    assert!(r.device_holds().is_empty());
    let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
    assert!(logs.contains("needs `auth.passkey evil.example`"), "{logs}");
}

/// D8, drive 7: a ceremony a tool's call starts fails `person-required`
/// with no ticket, whatever its grants allow.
#[test]
fn a_tool_s_ceremony_is_refused_person_required() {
    let (_app, r, _) = boot("tool");
    let mut host = Host {
        r,
        waiting: Vec::new(),
    };
    available(&mut host.r);
    host.r
        .act_from(CallOrigin::Tool, "register", vec![])
        .unwrap();
    host.pump();
    assert!(host.tickets().is_empty(), "{}", host.logs());
    assert!(
        host.logs()
            .contains("passkeys.create: refused, person-required (tool origin)"),
        "{}",
        host.logs()
    );
}

/// D9: a modal ceremony ends a held conditional one with `AbortError`; one
/// that arrives while a modal one is held waits, with no ticket, until it ends.
#[test]
fn one_sheet_at_a_time() {
    let (_app, r, _) = boot("queue");
    let mut host = Host {
        r,
        waiting: Vec::new(),
    };
    available(&mut host.r);
    let conditional = call(&mut host.r, &get("conditional"));
    assert_eq!(host.tickets()[0].1["mediation"], "conditional");
    let modal = call(&mut host.r, &create(Some("auth-fixture.exact.test")));
    assert_eq!(json_of(conditional.recv().unwrap())["error"], "AbortError");
    let tickets = host.tickets();
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0].1["ceremony"], "create");
    let behind = call(&mut host.r, &get("optional"));
    assert_eq!(host.tickets().len(), 1, "the second waits with no ticket");
    let cancel = HoldAnswer::Choice("cancel".into());
    host.r.answer_hold(tickets[0].0, &cancel).unwrap();
    let cancelled = json_of(modal.recv().unwrap());
    assert_eq!(cancelled["error"], "NotAllowedError");
    assert_eq!(cancelled["apple"]["code"], "canceled");
    let next = host.tickets();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].1["ceremony"], "get");
    let none = HoldAnswer::Choice("none".into());
    host.r.answer_hold(next[0].0, &none).unwrap();
    assert_eq!(json_of(behind.recv().unwrap())["error"], "NotAllowedError");
}

/// D11: a registration answered after its mutation was superseded never
/// reaches data code's server; the journal records its disposal.
#[test]
fn a_dropped_registration_is_disposed_of() {
    let (_app, r, data) = boot("dispose");
    let mut host = Host {
        r,
        waiting: Vec::new(),
    };
    available(&mut host.r);
    host.r.act("register", vec![]).unwrap();
    host.pump();
    let (ticket, _) = host.tickets()[0].clone();
    host.r.act("idle", vec![]).unwrap();
    host.pump();
    let response = HoldAnswer::Value(registration("cred-2"));
    host.r.answer_hold(ticket, &response).unwrap();
    host.pump();
    let logs = host.logs();
    assert!(
        logs.contains("dispose passkeys cred-2: its reply was dropped"),
        "{logs}"
    );
    assert!(data.0.lock().unwrap().is_empty(), "no data code read it");
}

/// §9: a signal answers nothing, at once, and is listed without its arguments.
#[test]
fn a_signal_is_recorded() {
    let (_app, mut r, _) = boot("signal");
    let signal = |rp: &str| {
        json!({ "extension": "passkeys", "send": "signalUnknownCredential",
            "args": { "rpId": rp, "credentialId": "cred-3" } })
    };
    let answer = json_of(
        call(&mut r, &signal("auth-fixture.exact.test"))
            .recv()
            .unwrap(),
    );
    assert_eq!(answer, Json::Null);
    let ungranted = json_of(call(&mut r, &signal("evil.example")).recv().unwrap());
    assert_eq!(ungranted["error"], "SecurityError");
    let extensions = r.extensions_json().unwrap();
    assert!(
        extensions.contains("\"outcome\":\"recorded\""),
        "{extensions}"
    );
    assert!(!extensions.contains("cred-3"), "{extensions}");
}

/// §9, D6: a Mac build without a provisioning profile has no credential
/// manager an app can use: `available` reads false, and a ceremony fails
/// `NotSupportedError` with no ticket.
#[cfg(target_os = "macos")]
#[test]
fn a_mac_without_a_profile_has_no_passkeys() {
    let (_app, mut r, _) = boot("mac");
    let body = create(Some("auth-fixture.exact.test"));
    let answer = json_of(call(&mut r, &body).recv().unwrap());
    assert_eq!(answer["error"], "NotSupportedError", "{answer}");
    assert!(r.device_holds().is_empty());
}
