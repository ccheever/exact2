//! A call whose request the runner forgot is let go (LLP 1016 D5). Newer
//! arguments replace a request in flight; a refused commit puts back the
//! request it replaced. Either way the executor keeps parked only the calls
//! whose requests are still in flight.

#![cfg(exact_js_engine)]

use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_runner::{Event, Outcome, Response, Runner};

const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/castle.hbc"));
const APP: &str = "xyz.castle.test";
const GRANTS: &str = "net.fetch https://api.castle.xyz\nsecret.keep castle.session\n";

const SRC: &str = r#"
shape Session
  ok: bool
  username: string
  error: string

component App
  state who = ""
  mutation session as shape Session
  mutation probe as shape Session

  action typed(v)
    who = v
    send session = login(v, "pw")
  action refusing
    send session = login("zed", "pw")
    send probe = nowhere()

  view
    column
      input value=who change=typed testId="who"
      button press=refusing aria-label="Refuse" testId="refuse"
        text "Refuse"
      match session
        case some(s)
          text s.username testId="user"
        case none
          text "none" testId="none"
"#;

fn boot() -> Runner<Module> {
    let mut module = Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap();
    module.set_budget_ms(f64::INFINITY);
    Runner::boot(
        contract::compile(SRC).expect("the fixture's Contract compiles"),
        module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view_of(r: &Runner<Module>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn text_of(r: &Runner<Module>, test_id: &str) -> Option<String> {
    let k = r.kernel();
    let key = k.find_by_test_id(test_id).into_iter().next()?;
    k.node_by_key(key)?
        .props
        .str(PropId::Text)
        .map(str::to_string)
}

fn login(username: &str) -> Outcome {
    let body = format!(r#"{{"data":{{"loginV2":{{"token":"t0k","username":"{username}"}}}}}}"#);
    Outcome::Response(Response {
        status: 200,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.into_bytes(),
    })
}

#[test]
fn a_request_replaced_by_newer_arguments_lets_its_call_go() {
    let mut r = boot();
    for v in ["a", "ab", "abc"] {
        r.dispatch(view_of(&r, "who"), Event::Change(v.into()))
            .unwrap();
    }
    let requests = r.take_requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(r.pending().len(), 1, "the runner holds the newest");
    assert_eq!(r.data().in_flight(), 1, "the executor parks only its call");
    // The newest reply settles; the older replies are dropped.
    r.fulfill(requests[2].ticket, login("abc")).unwrap();
    assert_eq!(text_of(&r, "user").as_deref(), Some("abc"));
    for q in &requests[..2] {
        assert!(r.fulfill(q.ticket, login("old")).unwrap().is_none());
    }
    assert_eq!(text_of(&r, "user").as_deref(), Some("abc"));
    assert_eq!(r.data().in_flight(), 0);
}

#[test]
fn a_refused_commit_puts_back_the_request_it_replaced_and_lets_its_own_call_go() {
    let mut r = boot();
    r.dispatch(view_of(&r, "who"), Event::Change("ada".into()))
        .unwrap();
    let ada = r.take_requests();
    assert_eq!(ada.len(), 1);
    // `refusing` asks `session` again, with other arguments, and then a
    // source the module does not answer, which refuses (a source's answer
    // that fails would end that send unsent instead, LLP 1027.000 D3): the
    // commit is refused, and ada's request is put back in flight.
    assert!(r.dispatch(view_of(&r, "refuse"), Event::Press).is_err());
    assert!(r.take_requests().is_empty());
    assert_eq!(r.pending(), [("session".to_string(), ada[0].ticket)]);
    assert_eq!(r.data().in_flight(), 1, "only ada's call is parked");
    // Her reply still finds her call.
    r.fulfill(ada[0].ticket, login("ada")).unwrap();
    assert_eq!(text_of(&r, "user").as_deref(), Some("ada"));
    assert_eq!(r.data().in_flight(), 0);
}
