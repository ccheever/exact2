//! The native slot's one handler on a desktop loop (LLP 1133.022 D10 item
//! 5): a composed package's body reaches its instance on the loop's turn, a
//! shell's goes to the shell beside it, any other is refused; a call the
//! runner asks to admit reads the instance's own status first; the session's
//! end destroys each instance once.
use super::super::*;
use crate::package::{json, Context, Entry, Json, Package, Reply};
use std::cell::RefCell;

thread_local!(static HEARD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) });
thread_local!(static STATE: RefCell<&'static str> = const { RefCell::new("prompt") });

fn heard() -> Vec<String> {
    HEARD.with(|h| std::mem::take(&mut *h.borrow_mut()))
}

/// Answers its status from `prompt`'s state and echoes every call.
struct Probe {
    context: Context,
}

impl Package for Probe {
    fn new(context: Context) -> Self {
        HEARD.with(|h| {
            h.borrow_mut()
                .push(format!("made agent={}", context.agent()))
        });
        Probe { context }
    }
    fn later(&mut self, name: &str, args: &Json, _: Option<&Json>, reply: Reply) {
        let origin = self.context.origin();
        HEARD.with(|h| h.borrow_mut().push(format!("later {name} {args} {origin}")));
        match name {
            "status" => reply.send(json!({ "state": STATE.with(|s| *s.borrow()) })),
            _ => reply.send(json!({ "name": name })),
        }
    }
    fn release(&mut self, name: &str, args: Option<&Json>) {
        HEARD.with(|h| h.borrow_mut().push(format!("release {name} {args:?}")));
    }
    fn foreground(&mut self, shown: bool) {
        let said = self.context.foreground();
        HEARD.with(|h| h.borrow_mut().push(format!("foreground {shown} {said}")));
    }
    fn destroy(&mut self) {
        HEARD.with(|h| h.borrow_mut().push("destroy".into()));
    }
}

#[derive(Default)]
struct Nothing;
impl DataSource for Nothing {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(name.into()))
    }
}

fn boot() -> Presenter<Nothing> {
    let plan = crate::test_plan("component App\n  view\n    text \"Hi\"\n");
    let (p, _) = Presenter::boot_with(
        &plan,
        Nothing,
        (400., 800.),
        1.,
        std::path::PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    p
}

/// Hand `body` to the native slot as the runner does, run a turn, and
/// return what its reply carried.
fn call(p: &mut Presenter<Nothing>, body: Json) -> std::sync::mpsc::Receiver<Json> {
    let (tx, rx) = std::sync::mpsc::channel();
    let reply = exact_runner::Reply::new(move |outcome| {
        if let exact_runner::Outcome::Response(r) = outcome {
            let _ = tx.send(serde_json::from_slice(&r.body).unwrap_or_default());
        }
    });
    let handler = p
        .host()
        .runner()
        .native_slot()
        .handler()
        .expect("a handler");
    handler(body.to_string().into_bytes(), reply);
    p.run_commands(Nothing::default);
    rx
}

#[test]
fn bodies_reach_the_package_the_shell_or_a_refusal() {
    let mut p = boot();
    p.serve_packages(vec![Entry::of::<Probe>("probe")]);
    let answer = call(
        &mut p,
        json!({"extension": "probe", "send": "ping", "args": {"n": 1}, "origin": "tool"}),
    );
    assert_eq!(answer.recv().unwrap(), json!({ "name": "ping" }));
    let foreground = call(&mut p, json!({"extension": "probe", "foreground": false}));
    assert_eq!(foreground.recv().unwrap(), json!({}));
    call(
        &mut p,
        json!({"extension": "probe", "release": "status", "args": {"a": 1}}),
    );
    assert_eq!(
        heard(),
        [
            "made agent=false",
            "later ping {\"n\":1} tool",
            "foreground false false",
            "release status Some(Object {\"a\": Number(1)})"
        ]
    );
    // No shell: another extension is refused, not queued.
    let refused = call(
        &mut p,
        json!({"extension": "other", "send": "go", "args": {}}),
    );
    assert_eq!(refused.recv().unwrap()["error"], "NotSupportedError");
    assert_eq!(p.take_requests(), None);
    // A shell beside the packages: its extension's body is queued for it,
    // and the package's still reaches the package.
    p.serve_requests();
    let shells = call(
        &mut p,
        json!({"extension": "other", "send": "go", "args": {}}),
    );
    let queued = p.take_requests().expect("the shell's request");
    assert_eq!(queued[0], 11, "a module request, kind 11");
    let id = queued[1] as u32;
    assert!(p.answer_request(id, Some("{\"went\":true}")));
    assert_eq!(shells.recv().unwrap(), json!({ "went": true }));
    let still = call(
        &mut p,
        json!({"extension": "probe", "send": "ping", "args": {}}),
    );
    assert_eq!(still.recv().unwrap(), json!({ "name": "ping" }));
    heard();
    p.end_packages();
    p.end_packages();
    assert_eq!(heard(), ["destroy"], "destroyed once");
}

#[test]
fn an_admitted_call_reads_the_status_first() {
    let mut p = boot();
    p.serve_packages(vec![Entry::of::<Probe>("probe")]);
    let admit = json!({"status": "status", "field": "state", "denied": "NotAllowedError"});
    // The status reads `prompt`: refused before the call is made.
    let refused = call(
        &mut p,
        json!({"extension": "probe", "send": "ask", "args": {}, "origin": "tool", "admit": admit}),
    );
    let answer = refused.recv().unwrap();
    assert_eq!(
        (answer["error"].as_str(), answer["message"].as_str()),
        (Some("NotAllowedError"), Some("person-required"))
    );
    assert_eq!(heard(), ["made agent=false", "later status {} tool"]);
    // Granted: the call comes back through the inbox and is made on the next turn.
    STATE.with(|s| *s.borrow_mut() = "granted");
    let made = call(
        &mut p,
        json!({"extension": "probe", "send": "ask", "args": {}, "origin": "tool", "admit": admit}),
    );
    p.run_commands(Nothing::default);
    assert_eq!(made.recv().unwrap(), json!({ "name": "ask" }));
    assert_eq!(heard(), ["later status {} tool", "later ask {} tool"]);
}
