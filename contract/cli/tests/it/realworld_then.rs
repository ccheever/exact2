//! LLP 1035.005.000 D7: RealWorld redirects after sign-in, publish and delete
//! from its mutations' `then` actions (LLP 1016.001), not from a polling
//! tick; the signed-out guard stays on the tick. Driven offline: every source
//! answers later, and the test lands the replies.
use exact_kernel::Kernel;
use exact_runner::{
    agent, Answer, DataError, DataSource, Outcome, Request, Response, Runner, Store, Value,
};
use serde_json::Value as Json;

const APP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../apps/realworld/app.contract"
);

/// RealWorld's API, offline: every read and write is a request; a reply
/// parses to what app.ts answers for a success.
struct Api;

fn change(kind: &str, slug: &str) -> Value {
    Value::record(vec![
        Value::Number(1.0),
        Value::str(kind),
        Value::Bool(true),
        Value::str(slug),
        Value::list(vec![]),
    ])
}

impl DataSource for Api {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::Unavailable(format!("{source} answers later")))
    }
    fn answer(&mut self, _: &mut Store, source: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(Request::get(&format!(
            "https://api.realworld.test/{source}"
        ))))
    }
    fn parse(
        &mut self,
        _: &mut Store,
        source: &str,
        _: &[Value],
        _: Outcome,
    ) -> Result<Answer, DataError> {
        Ok(Answer::Now(match source {
            "login" => Value::record(vec![
                Value::Number(1.0),
                Value::Bool(true),
                Value::list(vec![]),
            ]),
            "publish" => change("publish", "my-post"),
            "deleteArticle" => change("delete", ""),
            "currentUser" => Value::record(vec![
                Value::Bool(false),
                Value::str(""),
                Value::str(""),
                Value::str(""),
                Value::str(""),
                Value::str("/assets/default-avatar.svg"),
            ]),
            other => return Err(DataError::UnknownSource(other.into())),
        }))
    }
}

fn boot(launch: &str) -> Runner<Api> {
    let src = std::fs::read_to_string(APP).unwrap();
    let plan = contract::compile_path_source(std::path::Path::new(APP), &src).unwrap();
    let plan = contract::bake(plan, Api).unwrap();
    Runner::boot(
        plan,
        Api,
        Kernel::with_monospace(),
        Default::default(),
        launch,
    )
    .unwrap()
}

/// The top entry: (route name, url).
fn current(r: &Runner<Api>) -> (String, String) {
    let s: Json = serde_json::from_str(&agent::state(r)).unwrap();
    let c = &s["derives"]["current"];
    (
        c["name"].as_str().unwrap().into(),
        c["url"].as_str().unwrap().into(),
    )
}

/// Land the reply to the request `target` sent.
fn reply(r: &mut Runner<Api>, target: &str) {
    let ticket = r
        .take_requests()
        .into_iter()
        .find(|q| q.target == target)
        .unwrap_or_else(|| panic!("no request for {target}"))
        .ticket;
    let ok = Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: b"{}".to_vec(),
    });
    r.fulfill(ticket, ok).unwrap().unwrap();
}

#[test]
fn a_sign_in_answer_goes_home_at_the_next_advance() {
    let mut r = boot("/login");
    assert_eq!(current(&r).0, "login");
    r.act("login", vec![Value::str("a@b.c"), Value::str("pw")])
        .unwrap();
    reply(&mut r, "auth");
    // The answer's commit shows the answer; its `then` is due at once.
    assert_eq!(current(&r).0, "login");
    let now = r.now_ms();
    assert_eq!(r.timer_due_ms(), Some(now));
    r.advance(now).unwrap();
    assert_eq!(current(&r).0, "home");
}

#[test]
fn publish_opens_the_article_and_delete_goes_home() {
    let mut r = boot("/editor");
    assert_eq!(current(&r).0, "editor");
    r.act(
        "publish",
        vec![
            Value::str(""),
            Value::str("Title"),
            Value::str("About"),
            Value::str("Body"),
            Value::list(vec![]),
        ],
    )
    .unwrap();
    reply(&mut r, "change");
    let now = r.now_ms();
    r.advance(now).unwrap();
    assert_eq!(current(&r), ("article".into(), "/article/my-post".into()));
    r.act("deleteArticle", vec![Value::str("my-post")]).unwrap();
    reply(&mut r, "change");
    assert_eq!(current(&r).0, "article");
    let now = r.now_ms();
    r.advance(now).unwrap();
    assert_eq!(current(&r).0, "home");
}

#[test]
fn the_signed_out_guard_stays_on_the_tick() {
    // Your Feed, signed out: once `me` answers, the tick replaces the visit
    // with the global feed, then pushes sign-in.
    let mut r = boot("/?feed=following");
    r.advance(100.0).unwrap();
    assert_eq!(current(&r).1, "/?feed=following", "waits for `me`");
    reply(&mut r, "me");
    r.advance(200.0).unwrap();
    assert_eq!(current(&r).0, "home");
    r.advance(300.0).unwrap();
    assert_eq!(current(&r).0, "login");
}
