//! `@exact/sign-in` under its declared substitute (LLP 1133.014 §9): FedCM's
//! `identity` is a member of `@exact/passkeys`' `get` (D10 item 1), held to
//! the providers' grants and FedCM's rules before any ticket, and answered
//! through the one `credential` ticket, whose `args` name what
//! `packages/sign-in/test/idp.mjs` signs over. The tests run on this Mac,
//! whose cells are macOS's (D3): Google through the host's session, and
//! Apple only with a profile, which `prefer` stands in for.

use exact_runner::{
    Answer, CallOrigin, DataError, DataSource, Dispatch, HoldAnswer, Outcome, Reply, Request,
    Response, Runner, Store, Value, Work,
};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

const APP: &str = r#"use getClientCapabilities, ClientCapabilities from "@exact/passkeys"
use getIdentityCapabilities, IdentityCapabilities from "@exact/sign-in"

shape Result
  ok: bool
  message: string

component App
  resource passkeys = getClientCapabilities() as shape ClientCapabilities
  resource caps = getIdentityCapabilities() as shape IdentityCapabilities
  mutation signed as shape Result
  action google
    send signed = google()
  action idle
    send signed = idle()
  view
    column
      text "sign-in"
"#;

const GOOGLE: &str = "https://accounts.google.com/gsi/fedcm.json";
const APPLE: &str = "https://appleid.apple.com";
const GRANTS: &str = "auth.passkey auth-fixture.exact.test\nauth.google web.apps.googleusercontent.com\nauth.google ios.apps.googleusercontent.com\nauth.apple com.exact.authfixture.web\nnet.fetch http://127.0.0.1:4331\n";

struct App(PathBuf);
impl App {
    fn new(name: &str, contract: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("exact-sign-in-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let packages = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages");
        for package in ["passkeys", "sign-in"] {
            copy(
                &packages.join(package),
                &path.join("node_modules/@exact").join(package),
            );
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

/// A package's declarations: its `package.json` and each `exact.json`.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for file in ["package.json", "exact.json"] {
        std::fs::copy(from.join(file), to.join(file)).unwrap();
    }
    for platform in ["apple", "android", "web"] {
        let declared = from.join(platform).join("exact.json");
        if !declared.exists() {
            continue;
        }
        std::fs::create_dir_all(to.join(platform)).unwrap();
        std::fs::copy(declared, to.join(platform).join("exact.json")).unwrap();
    }
}

/// An active request for Google, as the recipe's action makes it on Apple.
fn google(client: &str) -> Json {
    json!({ "extension": "passkeys", "send": "get", "args": { "identity": { "mode": "active",
        "providers": [{ "configURL": GOOGLE, "clientId": client, "params": { "nonce": "n-1" },
            "apple": { "clientId": "ios.apps.googleusercontent.com" } }] } } })
}

/// A credential `idp.mjs` would make, for `config`.
fn credential(config: &str) -> String {
    json!({ "type": "identity", "id": "", "configURL": config, "isAutoSelected": false,
        "token": "eyJ.fixture.token", "handle": "h-1" })
    .to_string()
}

/// The app's data: `google` calls `credentials.get` as data code would, and
/// keeps each reply it read.
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
            "google" => Ok(Answer::Later(Request::native(
                google("web.apps.googleusercontent.com")
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
        Ok(Answer::Now(Value::record(vec![
            Value::Bool(ok),
            Value::str(reply["error"].as_str().unwrap_or_default()),
        ])))
    }
    fn grants(&self) -> &str {
        GRANTS
    }
    fn app_id(&self) -> &str {
        "com.exact.authfixture"
    }
}

fn boot(name: &str) -> (App, Host, Data) {
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
    let host = Host {
        r,
        waiting: Vec::new(),
    };
    (app, host, data)
}

/// The host: the runner, and the requests whose replies a ticket's answer sends.
struct Host {
    r: Runner<Data>,
    waiting: Vec<(u64, mpsc::Receiver<Outcome>)>,
}

impl Host {
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

    fn tickets(&self) -> Vec<(u64, Json)> {
        (self.r.device_holds().iter())
            .filter(|h| h.capability == "credential")
            .map(|h| (h.ticket, serde_json::from_str(&h.args).unwrap()))
            .collect()
    }

    fn logs(&self) -> String {
        exact_runner::agent::handle(&self.r, r#"{"op":"logs"}"#)
    }

    /// The capabilities laid over this target's default.
    fn prefer(&mut self, key: &str, value: Json) {
        let op = json!({ "op": "prefer", "extension": { key: value } }).to_string();
        let (_, out) = self.r.extension_agent(&op).unwrap();
        out.unwrap();
    }

    /// A data-code call of `body`: the receiver of its reply.
    fn call(&mut self, body: &Json) -> mpsc::Receiver<Outcome> {
        let (tx, rx) = mpsc::channel();
        match self
            .r
            .native_work(&Request::native(body.to_string().into_bytes()))
        {
            Dispatch::Run(Work::Now(work)) => tx.send(work()).unwrap(),
            Dispatch::Run(Work::Later(work)) => work(Reply::new(move |o| tx.send(o).unwrap())),
            _ => panic!("a module call is work"),
        }
        rx
    }

    fn answered(&mut self, body: &Json) -> Json {
        json_of(self.call(body).recv().unwrap())
    }
}

fn json_of(outcome: Outcome) -> Json {
    let Outcome::Response(Response { body, .. }) = outcome else {
        panic!("an answer");
    };
    serde_json::from_slice(&body).unwrap()
}

/// D10 item 1, §9: an active Google request is a member of passkeys' `get`,
/// held under the one `credential` ticket whose args name the provider, the
/// audience this target's token carries (the iOS client on Apple, D8) and
/// the nonce; a typed credential answers the data code, and neither the
/// journal nor `state` holds its token or handle.
#[cfg(target_os = "macos")]
#[test]
fn an_active_sign_in_is_a_member_of_get_and_a_credential_ticket() {
    let (_app, mut host, data) = boot("ticket");
    host.r.act("google", vec![]).unwrap();
    host.pump();
    let tickets = host.tickets();
    assert_eq!(tickets.len(), 1, "{}", host.logs());
    let (ticket, args) = &tickets[0];
    assert_eq!(args["ceremony"], "get");
    assert_eq!(args["options"], Json::Null, "no passkey asked");
    let provider = &args["identity"]["providers"][0];
    assert_eq!(args["identity"]["mode"], "active");
    assert_eq!(provider["provider"], "google");
    assert_eq!(provider["audience"], "ios.apps.googleusercontent.com");
    assert_eq!(provider["nonce"], "n-1");
    assert_eq!(provider["token"], "native");
    assert_eq!(provider["fields"], json!(["name", "email", "picture"]));
    // A passkey's response, or another provider's credential, is refused.
    let wrong = [
        json!({ "type": "public-key", "id": "x" }).to_string(),
        credential(APPLE),
    ];
    for text in wrong {
        assert!(host
            .r
            .answer_hold(*ticket, &HoldAnswer::Value(text))
            .is_err());
    }
    host.r
        .answer_hold(*ticket, &HoldAnswer::Value(credential(GOOGLE)))
        .unwrap();
    host.pump();
    let replies = data.0.lock().unwrap().clone();
    assert_eq!(replies.len(), 1, "{}", host.logs());
    assert_eq!(replies[0]["token"], "eyJ.fixture.token");
    let (logs, extensions) = (host.logs(), host.r.extensions_json().unwrap());
    for secret in ["eyJ.fixture.token", "h-1", "n-1"] {
        assert!(
            !logs.contains(secret) && !extensions.contains(secret),
            "{secret}: {logs}\n{extensions}"
        );
    }
    let listed: Json = serde_json::from_str(&extensions).unwrap();
    let ceremony = &listed["passkeys"]["ceremonies"][0];
    assert_eq!(
        ceremony["identity"],
        json!({ "mode": "active", "providers": ["google"] })
    );
    assert_eq!(ceremony["outcome"], "answered");
}

/// D1, D8: what admission refuses before any ticket, each with its name.
#[cfg(target_os = "macos")]
#[test]
fn admission_refuses_by_fedcm_s_rules_and_the_grants() {
    let (_app, mut host, _) = boot("refusals");
    let passkey = json!({ "challenge": "Z2V0", "rpId": "auth-fixture.exact.test" });
    let mut two = google("web.apps.googleusercontent.com");
    let first = two["args"]["identity"]["providers"][0].clone();
    two["args"]["identity"]["providers"] = json!([first.clone(), first.clone()]);
    let mut beside = google("web.apps.googleusercontent.com");
    beside["args"]["publicKey"] = passkey.clone();
    let mut silent = google("web.apps.googleusercontent.com");
    silent["args"]["mediation"] = json!("silent");
    let mut unknown = google("web.apps.googleusercontent.com");
    unknown["args"]["identity"]["providers"][0]["configURL"] = json!("https://idp.example");
    let mut no_ios = google("web.apps.googleusercontent.com");
    no_ios["args"]["identity"]["providers"][0]["apple"] = Json::Null;
    let mut apple = google("com.exact.authfixture.web");
    apple["args"]["identity"]["providers"][0]["configURL"] = json!(APPLE);
    let unregistered = json!({ "extension": "passkeys", "send": "get",
        "args": { "publicKey": passkey, "otp": { "transport": ["sms"] } } });
    let cases = [
        (
            google("evil.apps.googleusercontent.com"),
            "SecurityError",
            "needs `auth.google evil",
        ),
        (two, "NotSupportedError", "names one provider"),
        (beside, "NotSupportedError", "never beside publicKey"),
        (silent, "NotSupportedError", "silent"),
        (unknown, "NotSupportedError", "is no provider here"),
        (no_ios, "NotSupportedError", "needs apple.clientId"),
        // A Mac build without a profile offers no Apple (D7).
        (
            apple,
            "NotSupportedError",
            "apple offers no active request on macos",
        ),
        (
            unregistered,
            "NotSupportedError",
            "`otp` is no option of passkeys.get",
        ),
    ];
    for (body, code, why) in cases {
        let answer = host.answered(&body);
        assert_eq!(answer["error"], code, "{body}: {answer}");
        assert!(
            answer["message"].as_str().unwrap_or_default().contains(why),
            "{answer}"
        );
    }
    assert!(host.tickets().is_empty());
    // Data code calls the owner, never the member's row.
    let direct = json!({ "extension": "sign-in", "send": "identity", "args": {} });
    assert_eq!(host.answered(&direct)["error"], "NotSupportedError");
}

/// D3, §9: a passive Apple request beside a passkey is one sheet where the
/// target offers it (a Mac with a profile, by `prefer`); `cancel` then fails
/// as passkeys' does, and an identity request alone fails `NetworkError`.
#[cfg(target_os = "macos")]
#[test]
fn a_passkey_and_passive_apple_share_one_ticket() {
    let (_app, mut host, _) = boot("shared");
    host.prefer(
        "passkeys.getClientCapabilities",
        json!({ "available": true }),
    );
    host.prefer(
        "sign-in.getIdentityCapabilities",
        json!({ "providers": { "apple": { "active": true, "passive": true, "withPublicKey": true } } }),
    );
    let both = json!({ "extension": "passkeys", "send": "get", "args": {
        "publicKey": { "challenge": "Z2V0", "rpId": "auth-fixture.exact.test" },
        "identity": { "providers": [{ "configURL": APPLE, "clientId": "com.exact.authfixture.web",
            "params": { "nonce": "n-2" } }] } } });
    let reply = host.call(&both);
    let tickets = host.tickets();
    assert_eq!(tickets.len(), 1, "{}", host.logs());
    let args = &tickets[0].1;
    assert_eq!(args["options"]["challenge"], "Z2V0");
    assert_eq!(args["identity"]["mode"], "passive");
    // Apple's audience natively is the app's bundle ID (D8).
    assert_eq!(
        args["identity"]["providers"][0]["audience"],
        "com.exact.authfixture"
    );
    let cancel = HoldAnswer::Choice("cancel".into());
    host.r.answer_hold(tickets[0].0, &cancel).unwrap();
    assert_eq!(json_of(reply.recv().unwrap())["error"], "NotAllowedError");
    // Alone, a passive request the person dismisses fails `NetworkError`.
    let mut alone = both.clone();
    alone["args"]["publicKey"] = Json::Null;
    let reply = host.call(&alone);
    let (ticket, _) = host.tickets()[0].clone();
    host.r
        .answer_hold(ticket, &HoldAnswer::Choice("none".into()))
        .unwrap();
    assert_eq!(json_of(reply.recv().unwrap())["error"], "NetworkError");
    // Google cannot share a sheet on Apple (D3).
    let mut google_beside = both.clone();
    google_beside["args"]["identity"]["providers"][0]["configURL"] = json!(GOOGLE);
    google_beside["args"]["identity"]["providers"][0]["clientId"] =
        json!("web.apps.googleusercontent.com");
    google_beside["args"]["identity"]["providers"][0]["apple"] =
        json!({ "clientId": "ios.apps.googleusercontent.com" });
    let answer = host.answered(&google_beside);
    assert_eq!(answer["error"], "NotSupportedError", "{answer}");
    // A press's active request ends a held passive one with `AbortError` (D5).
    let passive = host.call(&alone);
    let active = host.call(&google("web.apps.googleusercontent.com"));
    assert_eq!(json_of(passive.recv().unwrap())["error"], "AbortError");
    let tickets = host.tickets();
    assert_eq!(tickets.len(), 1, "{}", host.logs());
    assert_eq!(tickets[0].1["identity"]["mode"], "active");
    // The runner answers the held call as it ends, so it ends before its receiver.
    drop(host);
    drop(active);
}

/// D9, drive 8's stand-in: a sign-in a tool's call starts fails
/// `person-required` with no ticket.
#[test]
fn a_tool_s_sign_in_is_refused_person_required() {
    let (_app, mut host, _) = boot("tool");
    host.r.act_from(CallOrigin::Tool, "google", vec![]).unwrap();
    host.pump();
    assert!(host.tickets().is_empty(), "{}", host.logs());
    assert!(
        host.logs()
            .contains("passkeys.get: refused, person-required (tool origin)"),
        "{}",
        host.logs()
    );
}

/// D12: a credential answered after its mutation was superseded never
/// reaches data code; its handle is disposed of by sign-in.
#[cfg(target_os = "macos")]
#[test]
fn a_dropped_sign_in_is_disposed_of_by_its_member() {
    let (_app, mut host, data) = boot("dispose");
    host.r.act("google", vec![]).unwrap();
    host.pump();
    let (ticket, _) = host.tickets()[0].clone();
    host.r.act("idle", vec![]).unwrap();
    host.pump();
    host.r
        .answer_hold(ticket, &HoldAnswer::Value(credential(GOOGLE)))
        .unwrap();
    host.pump();
    let logs = host.logs();
    assert!(
        logs.contains("dispose sign-in h-1: its reply was dropped"),
        "{logs}"
    );
    assert!(data.0.lock().unwrap().is_empty(), "no data code read it");
}

/// D10 item 1, D12: Contract may not name the member, and two packages may
/// not declare the same one.
#[test]
fn the_member_is_data_code_s_and_declared_once() {
    let source = "use identity from \"@exact/sign-in\"\n\ncomponent App\n  view\n    column\n      text \"x\"\n";
    let app = App::new("data-only", source);
    let errors = contract::compile_path_all(&app.0.join("app.contract"), false)
        .err()
        .map(|e| format!("{e:?}"))
        .unwrap_or_default();
    assert!(errors.contains("module-name-data-only"), "{errors}");
    let app = App::new("twice", APP);
    let twin = app.0.join("node_modules/@acme/sign-in");
    copy(&app.0.join("node_modules/@exact/sign-in"), &twin);
    let declared = std::fs::read_to_string(twin.join("exact.json")).unwrap();
    std::fs::write(
        twin.join("exact.json"),
        declared.replace("@exact/sign-in", "@acme/sign-in"),
    )
    .unwrap();
    let both = format!("use getIdentityCapabilities as other from \"@acme/sign-in\"\n{APP}");
    std::fs::write(app.0.join("app.contract"), both).unwrap();
    let errors = contract::compile_path_all(&app.0.join("app.contract"), false)
        .err()
        .map(|e| format!("{e:?}"))
        .unwrap_or_default();
    assert!(errors.contains("module-member-twice"), "{errors}");
}
