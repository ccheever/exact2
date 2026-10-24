//! `@exact/local-auth`'s `authenticate` under its declared substitute (LLP
//! 1133.010 §7, D12): Face ID's permission first while it is `prompt`, then
//! the person's check as a `presence` ticket; a denial that the passcode
//! still answers; a device with nothing to check; and presence never
//! delegated: a tool's call fails `person-required` before anything is
//! asked, in every state. The interface is the package's own `exact.json`.

use exact_runner::{CallOrigin, Dispatch, HoldAnswer, Outcome, Reply, Runner, Work};
use std::path::PathBuf;

const APP: &str = r#"use authenticate, localAuthStatus, LocalAuthentication, LocalAuthStatus from "@exact/local-auth"

component App
  resource status = localAuthStatus() as shape LocalAuthStatus
  mutation checked as shape LocalAuthentication
  action check
    send checked = authenticate(reason="reason.unlock")
  action checkBiometric
    send checked = authenticate(reason="reason.unlock", biometricOnly=true)
  view
    column
      text status.state
"#;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "exact-local-auth-{name}-{}-{n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let package = path.join("node_modules/@exact/local-auth");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/local-auth");
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
        std::fs::write(path.join("app.contract"), APP).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Data;
impl exact_runner::DataSource for Data {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "device.local-auth purpose.lock\n"
    }
}

fn boot(name: &str) -> (App, Runner<Data>) {
    let app = App::new(name);
    let (plan, _) = contract::compile_path_all(&app.0.join("app.contract"), false)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let r = Runner::boot(
        plan,
        Data,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    (app, r)
}

/// The host's part: each request dispatched (a held one parked), released
/// work run, each outcome fulfilled, until nothing is left to do.
fn pump(r: &mut Runner<Data>, parked: &mut Vec<(u64, u64)>) {
    for _ in 0..8 {
        let outs = r.take_requests();
        let released = r.release_work();
        if outs.is_empty() && released.is_empty() && r.apply_announced().0.is_empty() {
            return;
        }
        for out in outs {
            let dispatch = match out.request.continuation {
                Some(token) => {
                    parked.push((token, out.ticket));
                    r.dispatch_work(token)
                }
                None => r.native_work(&out.request),
            };
            run(r, out.ticket, dispatch);
        }
        for (token, dispatch) in released {
            let ticket = parked.iter().find(|(t, _)| *t == token).expect("parked").1;
            run(r, ticket, dispatch);
        }
    }
}

fn run(r: &mut Runner<Data>, ticket: u64, dispatch: Dispatch) {
    let outcome: Outcome = match dispatch {
        Dispatch::Run(Work::Now(work)) => work(),
        Dispatch::Run(Work::Later(work)) => {
            let (tx, rx) = std::sync::mpsc::channel();
            work(Reply::new(move |o| tx.send(o).unwrap()));
            rx.recv().unwrap()
        }
        _ => return,
    };
    let _ = r.fulfill(ticket, outcome);
}

fn prefer(r: &mut Runner<Data>, request: &str) {
    let (_, reply) = r.extension_agent(request).expect("a prefer");
    reply.unwrap();
}

/// The one hold of `capability`, its ticket.
fn held(r: &Runner<Data>, capability: &str) -> u64 {
    let holds: Vec<_> = r
        .device_holds()
        .iter()
        .filter(|h| h.capability == capability)
        .collect();
    assert_eq!(holds.len(), 1, "{:?}", r.device_holds());
    holds[0].ticket
}

fn tap(r: &mut Runner<Data>, parked: &mut Vec<(u64, u64)>, capability: &str, choice: &str) {
    let ticket = held(r, capability);
    r.answer_hold(ticket, &HoldAnswer::Choice(choice.into()))
        .unwrap();
    pump(r, parked);
}

#[test]
fn a_check_asks_face_id_s_permission_then_the_person() {
    let (_app, mut r) = boot("asks");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("check", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    tap(&mut r, &mut parked, "permission", "allow");
    tap(&mut r, &mut parked, "presence", "allow");
    let checked = format!("{:?}", r.slot("checked"));
    assert!(checked.contains("Some"), "{checked}");
    // Granted, a check is the person's alone; each failure is its code.
    for (choice, code) in [
        ("fail", "AUTHENTICATION_FAILED"),
        ("lockout", "LOCKOUT"),
        ("cancel", "USER_CANCEL"),
    ] {
        r.act("check", Vec::new()).unwrap();
        pump(&mut r, &mut parked);
        tap(&mut r, &mut parked, "presence", choice);
        let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
        assert!(
            logs.contains(&format!("local-auth.authenticate: {code}")),
            "{choice}: {logs}"
        );
    }
}

#[test]
fn a_denial_leaves_the_passcode_and_fails_a_biometric_check_alone() {
    let (_app, mut r) = boot("denied");
    let mut parked = Vec::new();
    pump(&mut r, &mut parked);
    r.act("check", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    tap(&mut r, &mut parked, "permission", "deny");
    // Face ID refused, the passcode still answers.
    held(&r, "presence");
    tap(&mut r, &mut parked, "presence", "allow");
    assert!(format!("{:?}", r.slot("checked")).contains("Some"));
    r.act("checkBiometric", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty(), "{:?}", r.device_holds());
    let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
    assert!(
        logs.contains("local-auth.authenticate: PERMISSION_DENIED"),
        "{logs}"
    );
    // A device with nothing to check: no ticket.
    prefer(
        &mut r,
        r#"{"op":"prefer","extension":{"local-auth.localAuthStatus":{"available":false,"biometry":"none","deviceCredential":false,"presenceSecrets":false}}}"#,
    );
    r.act("check", Vec::new()).unwrap();
    pump(&mut r, &mut parked);
    assert!(r.device_holds().is_empty());
    let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
    assert!(
        logs.contains("local-auth.authenticate: NOT_AVAILABLE"),
        "{logs}"
    );
}

#[test]
fn presence_is_never_delegated_to_a_tool() {
    for state in ["prompt", "granted", "denied"] {
        let (_app, mut r) = boot(&format!("tool-{state}"));
        let mut parked = Vec::new();
        pump(&mut r, &mut parked);
        prefer(
            &mut r,
            &format!(r#"{{"op":"prefer","permission":{{"local-auth":"{state}"}}}}"#),
        );
        r.act_from(CallOrigin::Tool, "check", Vec::new()).unwrap();
        pump(&mut r, &mut parked);
        assert!(
            r.device_holds().is_empty(),
            "{state}: {:?}",
            r.device_holds()
        );
        let logs = exact_runner::agent::handle(&r, r#"{"op":"logs"}"#);
        assert!(
            logs.contains("local-auth.authenticate: refused, person-required (tool origin)"),
            "{state}: {logs}"
        );
        assert!(
            logs.contains("PERMISSION_DENIED: person-required"),
            "{state}: {logs}"
        );
    }
}
