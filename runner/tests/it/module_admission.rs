//! A data-code send to a bound implementation passes the runner's admission
//! first (LLP 1133.006 D8 item 3): a path the source's `fs.read` does not
//! cover, or a malformed one, is refused before the implementation sees the
//! call; an admitted one reaches it as written, and the `$state` its answer
//! carries goes to `state.extensions`, not the caller.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{
    Answer, DataError, DataSource, Dispatch, Outcome, Reply, Request, Response, Runner, Store, Work,
};
use std::sync::{Arc, Mutex};

struct Source(&'static str);

impl DataSource for Source {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        self.0
    }
}

fn boot(grants: &'static str) -> (Runner<Source>, Arc<Mutex<Vec<String>>>) {
    let plan = contract::compile("component App\n  view\n    text \"x\"\n").unwrap();
    let mut r = Runner::boot(
        plan,
        Source(grants),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let saw = seen.clone();
    r.native_slot()
        .host(Some(Arc::new(move |body: Vec<u8>, reply: Reply| {
            saw.lock().unwrap().push(String::from_utf8(body).unwrap());
            reply.send(Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: br#"{"$state":{"last":{"types":["image/png"]}}}"#.to_vec(),
            }));
        })));
    r.bind_extensions(&[r#"tray {"person":["put"],"substitute":true}"#]);
    (r, seen)
}

/// The answer the call gets.
fn send(r: &mut Runner<Source>, args: &str) -> String {
    let body = format!(r#"{{"extension":"tray","send":"put","args":{args}}}"#);
    let work = match r.native_work(&Request::native(body.into_bytes())) {
        Dispatch::Run(Work::Now(now)) => return text(now()),
        Dispatch::Run(Work::Later(work)) => work,
        _ => panic!("a send is work"),
    };
    let (tx, rx) = std::sync::mpsc::channel();
    work(Reply::new(move |o| tx.send(o).unwrap()));
    text(rx.recv().unwrap())
}

fn text(outcome: Outcome) -> String {
    let Outcome::Response(Response { body, .. }) = outcome else {
        panic!("{outcome:?}")
    };
    String::from_utf8(body).unwrap()
}

#[test]
fn a_path_the_grant_does_not_cover_is_refused_before_the_implementation() {
    let (mut r, seen) = boot("fs.read app:/tmp/copy");
    let denied = send(&mut r, r#"{"item":{"$path":"app:/data/secret.png"}}"#);
    assert!(
        denied.contains("NotAllowedError") && denied.contains("fs.read app:/data/secret.png"),
        "{denied}"
    );
    let malformed = send(&mut r, r#"{"item":{"$path":"app:/tmp/copy/../../x"}}"#);
    assert!(malformed.contains("DataError"), "{malformed}");
    assert!(seen.lock().unwrap().is_empty());

    let answer = send(
        &mut r,
        r#"{"item":{"$path":"app:/tmp/copy/a.png"},"text":"hi"}"#,
    );
    assert_eq!(answer, "{}", "the $state is the runner's, not the caller's");
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(seen.lock().unwrap()[0].contains(r#""$path":"app:/tmp/copy/a.png""#));
    let state: serde_json::Value = serde_json::from_str(&r.extensions_json().unwrap()).unwrap();
    assert_eq!(state["tray"]["calls"]["put"], 3);
    assert_eq!(state["tray"]["last"]["types"][0], "image/png");
    assert_eq!(state["tray"]["bound"], true);
}

#[test]
fn a_send_with_no_path_needs_no_grant() {
    let (mut r, seen) = boot("");
    assert_eq!(send(&mut r, r#"{"text":"hi"}"#), "{}");
    assert_eq!(seen.lock().unwrap().len(), 1);
}
