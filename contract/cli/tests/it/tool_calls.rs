//! LLP 1111 D2–D4a and LLP 1133.023 D7: a `tool` line, and a call of it as
//! an invocation of origin `tool` that owns the work its commit started,
//! identified by the invocation's id: the ask its mutation's refresh makes,
//! not a re-ask the device or the person makes, and not one a refused
//! commit let go of.

use exact_runner::{agent, CallHow, Runner, Value, BACKGROUND};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const APP: &str = r#"shape Ack
  ok: bool

shape Count
  n: number

component App
  state size = 1
  resource count = counter() as shape Count else empty()
  mutation logged as shape Ack queue refreshes count
  mutation changed as shape Ack queue refreshes count
  derive xs = range(size)
  tool bump writes closed "Add to the count." = bump
    returns count.n
    fails when match logged { case some(a) => not a.ok, case none => false }
  tool look reads "Look." = look
  action bump(by: number)
    send logged = add(by)
  action turn
    send changed = add(1)
  action recount
    refresh count
  action refreshThenFail(n: number)
    refresh count
    size = n
  action look
    focus("n")
  view
    column
      text testId="n" `${count.n}`
      text `${xs.length}`
"#;

/// Every source answers later: `counter` the count the test says, `add`
/// whether it took. While `background` is set, it has storage no answer
/// awaits, which the runner holds a ticket for.
struct Later(Arc<AtomicU64>);
impl exact_runner::DataSource for Later {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(source.into()))
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        // The count watches the device's `tick`, which asks it again.
        if source == "counter" {
            store.observe_topic("tick");
        }
        Ok(exact_runner::Answer::Later(exact_runner::Request::native(
            b"{}".to_vec(),
        )))
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        _: &[Value],
        outcome: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        let exact_runner::Outcome::Response(response) = outcome else {
            return Err(exact_runner::DataError::Unavailable("failed".into()));
        };
        let body = String::from_utf8(response.body).unwrap();
        Ok(exact_runner::Answer::Now(match source {
            "counter" => Value::record(vec![Value::Number(body.parse().unwrap())]),
            _ => Value::record(vec![Value::Bool(body == "ok")]),
        }))
    }
    fn grants(&self) -> &str {
        ""
    }
    fn background(&mut self, _: &exact_runner::Store) -> Option<exact_runner::Request> {
        (self.0.load(Ordering::Relaxed) > 0)
            .then(|| exact_runner::Request::continuation(BACKGROUND))
    }
}

fn boot() -> (Runner<Later>, Arc<AtomicU64>) {
    let background = Arc::new(AtomicU64::new(0));
    let plan = contract::compile(APP).unwrap_or_else(|e| panic!("{e:?}"));
    let mut r = Runner::boot(
        plan,
        Later(background.clone()),
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    answer(&mut r, "count", "3");
    (r, background)
}

fn by(r: &mut Runner<Later>, ms: f64) {
    let at = r.now_ms() + ms;
    r.advance(at).unwrap();
}

fn logs(r: &Runner<Later>) -> String {
    agent::handle(r, r#"{"op":"logs"}"#)
}

fn calls(r: &Runner<Later>) -> serde_json::Value {
    let state: serde_json::Value =
        serde_json::from_str(&agent::handle(r, r#"{"op":"state"}"#)).unwrap();
    state["intents"]["calls"].clone()
}

thread_local! {
    /// The requests taken from the runner and not yet answered.
    static OUT: std::cell::RefCell<Vec<exact_runner::RequestOut>> = Default::default();
}

/// The one request in flight for `target` (a resource or mutation),
/// answered with `body`.
fn answer(r: &mut Runner<Later>, target: &str, body: &str) {
    let ticket = OUT.with_borrow_mut(|out| {
        out.extend(r.take_requests());
        let mine: Vec<usize> = (0..out.len())
            .filter(|&i| out[i].target == target)
            .collect();
        assert_eq!(mine.len(), 1, "one {target} in flight: {}", logs(r));
        out.remove(mine[0]).ticket
    });
    let ok = exact_runner::Outcome::Response(exact_runner::Response {
        status: 200,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    });
    r.fulfill(ticket, ok).unwrap();
    by(r, 0.0);
}

fn call(r: &mut Runner<Later>, tool: &str, args: &str) {
    r.call_tool(
        tool,
        args,
        CallHow {
            door: "agent".into(),
            ..Default::default()
        },
    )
    .unwrap();
    by(r, 0.0);
}

#[test]
fn the_tool_line_is_the_plans_tools_row() {
    let plan = contract::compile(APP).unwrap();
    let rows: Vec<String> = (plan.tools.iter())
        .map(|t| {
            format!(
                "{} {} {:?} {} {} {}",
                plan.str(t.name),
                plan.str(t.consequence),
                [t.additive, t.idempotent, t.closed],
                plan.str(plan.action(t.action).name),
                t.returns.is_some(),
                t.fails.is_some()
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            "bump writes [false, false, true] bump true true",
            "look reads [false, false, false] look false false",
        ]
    );
    for (line, id) in [
        ("  tool x reads \"X.\" = nothing\n", "tool-action"),
        (
            "  tool x writes additive closed \"X.\" = look\n  tool x reads \"X.\" = look\n",
            "tool-duplicate",
        ),
        (
            "  tool x reads \"X.\" = look\n    available when 3\n",
            "tool-condition-type",
        ),
        (
            "  tool x writes \"X.\" = copy\n  action copy\n    copyText(\"x\")\n",
            "tool-command",
        ),
    ] {
        let text = APP.replacen("  action bump", &format!("{line}  action bump"), 1);
        let refused = format!("{:?}", contract::compile(&text).unwrap_err());
        assert!(refused.contains(id), "{id}: {refused}");
    }
}

/// D7: the call's commit, the `add` it sends and the `counter` re-ask its
/// refresh makes are its own; it ends when they have landed, with what
/// `returns` reads then.
#[test]
fn a_call_owns_its_send_and_the_refresh_it_makes() {
    let (mut r, _) = boot();
    call(&mut r, "bump", r#"{"by":2}"#);
    assert_eq!(calls(&r)[0]["state"], "running", "{}", logs(&r));
    // The door polls its call by id (ExactTools.swift).
    let id = calls(&r)[0]["id"].clone();
    let polled = |r: &mut Runner<Later>| -> serde_json::Value {
        let reply = r.call_agent("called", &format!(r#"{{"op":"called","id":{id}}}"#));
        serde_json::from_str(&reply.unwrap().unwrap()).unwrap()
    };
    assert_eq!(polled(&mut r)["state"], "running");
    answer(&mut r, "logged", "ok");
    assert_eq!(calls(&r)[0]["state"], "running", "{}", logs(&r));
    answer(&mut r, "count", "5");
    assert_eq!(polled(&mut r)["outcome"], "ok");
    let c = &calls(&r)[0];
    assert_eq!(
        (c["outcome"].as_str(), c["certainty"].as_str()),
        (Some("ok"), Some("applied")),
        "{c} {}",
        logs(&r)
    );
    assert_eq!(c["value"], 5.0);
    // A reply that says it did not take is `failed`.
    call(&mut r, "bump", r#"{"by":2}"#);
    answer(&mut r, "logged", "no");
    answer(&mut r, "count", "5");
    assert_eq!(calls(&r)[1]["outcome"], "failed", "{}", logs(&r));
}

/// D7: a re-ask the person's commit makes of a resource the call last
/// asked is not the call's; it replaces the call's ask, and the call is
/// `superseded`, its write `applied`.
#[test]
fn a_persons_refresh_supersedes_the_calls_ask() {
    let (mut r, _) = boot();
    call(&mut r, "bump", r#"{"by":2}"#);
    answer(&mut r, "logged", "ok");
    r.act("recount", Vec::new()).unwrap();
    by(&mut r, 0.0);
    let c = &calls(&r)[0];
    assert_eq!(
        (c["outcome"].as_str(), c["certainty"].as_str()),
        (Some("superseded"), Some("applied")),
        "{c} {}",
        logs(&r)
    );
}

/// D7: a commit that would let go of the call's ask and is then refused
/// lets go of nothing: the call runs on to its end.
#[test]
fn a_refused_commit_does_not_supersede() {
    let (mut r, _) = boot();
    call(&mut r, "bump", r#"{"by":2}"#);
    answer(&mut r, "logged", "ok");
    assert!(r.act("refreshThenFail", vec![Value::Number(2.5)]).is_err());
    by(&mut r, 0.0);
    assert_eq!(calls(&r)[0]["state"], "running", "{}", logs(&r));
    answer(&mut r, "count", "5");
    assert_eq!(calls(&r)[0]["outcome"], "ok", "{}", logs(&r));
}

/// D7: a re-ask the device makes of a resource a call last asked keeps the
/// call's origin and no invocation's id: a later call does not wait for
/// it. Nor for a re-ask the person made.
#[test]
fn a_device_reask_is_not_the_calls() {
    let (mut r, _) = boot();
    call(&mut r, "bump", r#"{"by":1}"#);
    answer(&mut r, "logged", "ok");
    answer(&mut r, "count", "4");
    assert_eq!(calls(&r)[0]["outcome"], "ok", "{}", logs(&r));
    r.changed("tick").unwrap();
    by(&mut r, 0.0);
    call(&mut r, "look", "{}");
    let c = &calls(&r)[1];
    assert_eq!(c["outcome"], "ok", "{c} {}", logs(&r));
    answer(&mut r, "count", "4");
    r.act("recount", Vec::new()).unwrap();
    call(&mut r, "look", "{}");
    assert_eq!(calls(&r)[2]["outcome"], "ok", "{}", logs(&r));
}

/// D4a: a command that moves the person's view is skipped, and listed.
#[test]
fn a_calls_commit_skips_a_command_that_moves_the_view() {
    let (mut r, _) = boot();
    call(&mut r, "look", "{}");
    let c = &calls(&r)[0];
    assert_eq!(c["skipped"], serde_json::json!(["focus"]), "{c}");
    assert!(
        logs(&r).contains("focus: skipped in a tool call"),
        "{}",
        logs(&r)
    );
}

/// D7: background storage still writing when the call's work has landed:
/// `failed`, `uncertain`.
#[test]
fn background_storage_at_the_end_is_uncertain() {
    let (mut r, background) = boot();
    call(&mut r, "bump", r#"{"by":1}"#);
    answer(&mut r, "logged", "ok");
    background.store(1, Ordering::Relaxed);
    answer(&mut r, "count", "4");
    let c = &calls(&r)[0];
    assert_eq!(
        (
            c["outcome"].as_str(),
            c["certainty"].as_str(),
            c["reason"].as_str()
        ),
        (Some("failed"), Some("uncertain"), Some("background")),
        "{c}"
    );
}

/// LLP 1111 D4: arguments are the action's, exactly; a refusal is `none`.
#[test]
fn arguments_that_are_not_the_actions_are_refused() {
    let (mut r, _) = boot();
    for args in [r#"{}"#, r#"{"by":"2"}"#, r#"{"by":2,"extra":1}"#] {
        call(&mut r, "bump", args);
    }
    for c in calls(&r).as_array().unwrap() {
        assert_eq!(
            (
                c["outcome"].as_str(),
                c["reason"].as_str(),
                c["certainty"].as_str()
            ),
            (Some("refused"), Some("arguments"), Some("none")),
            "{c}"
        );
    }
    assert!(r.call_tool("nothing", "{}", CallHow::default()).is_err());
}
