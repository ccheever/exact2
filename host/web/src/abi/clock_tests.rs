//! The page's clock owns time until the agent's takes it (the timers RFC,
//! D1): a running runner and every one booted later take the choice.
use super::super::Bridge;
use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use std::cell::Cell;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const TICKER: &str = r#"component App
  state ticks = 0
  action tick
    ticks = ticks + 1
  task ticker mount
    every(1000, tick)
  view
    text "x"
"#;

thread_local! {
    // Each test's own page clock: the tests run in parallel
    static PAGE: Cell<f64> = const { Cell::new(0.) };
}

fn page() -> f64 {
    PAGE.get()
}

fn ticks(bridge: &Bridge<NoData>) -> f64 {
    let runner = bridge.host.as_ref().unwrap().runner();
    runner.slot("ticks").and_then(Value::as_number).unwrap()
}

#[test]
fn the_pages_clock_reaches_every_boot_until_the_agents_takes_time() {
    let plan = contract::compile(TICKER).unwrap().encode();
    let mut bridge = Bridge::<NoData>::new();
    PAGE.set(500.0);
    bridge.set_live_clock(Some(page));
    bridge.boot(&plan, NoData, 400., 400., "/");
    PAGE.set(3500.0);
    bridge.advance(3500., 0);
    assert_eq!(ticks(&bridge), 1.0, "three missed beats, one fire");
    // A dev restart, carrying the state and the timer, keeps the page's clock.
    let len = bridge.input_write(&plan);
    bridge.boot_plan(len, NoData, 400., 400., "/");
    PAGE.set(7500.0);
    bridge.advance(7500., 0);
    assert_eq!(ticks(&bridge), 2.0, "four missed beats, one fire");
    // The agent's clock: every beat fires again.
    bridge.set_live_clock(None);
    bridge.advance(10_500., 0);
    assert_eq!(ticks(&bridge), 5.0);
}

#[test]
fn a_fresh_boot_on_the_pages_clock_starts_where_it_reads() {
    let plan = contract::compile(TICKER).unwrap().encode();
    let mut bridge = Bridge::<NoData>::new();
    PAGE.set(60_000.0);
    bridge.set_live_clock(Some(page));
    bridge.boot(&plan, NoData, 400., 400., "/");
    let due = bridge.host.as_ref().unwrap().runner().timer_due_ms();
    assert_eq!(
        due,
        Some(61_000.0),
        "a beat after the boot, not 59 missed since 0"
    );
    bridge.advance(60_500., 0);
    assert_eq!(ticks(&bridge), 0.0);
    // A first dev generation, booted fresh through `boot_plan`, the same.
    let mut fresh = Bridge::<NoData>::new();
    fresh.set_live_clock(Some(page));
    let len = fresh.input_write(&plan);
    fresh.boot_plan(len, NoData, 400., 400., "/");
    let due = fresh.host.as_ref().unwrap().runner().timer_due_ms();
    assert_eq!(due, Some(61_000.0));
    // The agent's clock boots at zero.
    bridge.set_live_clock(None);
    bridge.boot(&plan, NoData, 400., 400., "/");
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().timer_due_ms(),
        Some(1000.0)
    );
}

#[test]
fn an_input_the_pages_clock_overtook_is_presented_at_the_inputs_time() {
    crate::link::link_for_tests();
    let source = r#"component App
  state on = false
  action go
    on = true
  view
    button "Go" press=go testId="go" opacity=(on ? 0 : 1) transition="opacity 1s linear"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let mut bridge = Bridge::<NoData>::new();
    PAGE.set(2600.0);
    bridge.set_live_clock(Some(page));
    bridge.boot(&plan, NoData, 400., 400., "/");
    let host = bridge.host.as_mut().unwrap();
    let k = host.runner().kernel();
    let go = k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id;
    host.dispatch_at(go, exact_runner::Event::Press, 2000.);
    assert_eq!(
        host.runner().now_ms(),
        2600.0,
        "the action observed the clock"
    );
    // The fade starts at the input's time: a release at 2000 can still end
    // a hold there.
    assert_eq!(host.motion_now(), 2.0);
}

#[test]
fn a_mixed_set_keeps_its_parked_request_pending_until_release_or_supersession() {
    use exact_data::Mixed;
    use exact_runner::{Answer, Dispatch, Placement, Store, Work};

    struct Numbers(Placement);
    impl DataSource for Numbers {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            Ok(args[0].clone())
        }
        fn placement(&self) -> Placement {
            self.0
        }
        fn grants(&self) -> &str {
            "secret.keep shared"
        }
    }
    let mut data = Mixed::new(
        Numbers(Placement::Worker),
        Numbers(Placement::Main),
        &[],
        &["number"],
    )
    .unwrap();
    let mut store = Store::new(data.grants(), []);
    let args = [Value::Number(99.)];
    let Answer::Later(reserved) = data.answer(&mut store, "number", &args).unwrap() else {
        panic!("the ordered call must reserve a turn")
    };
    let Dispatch::Run(Work::Now(work)) = data.dispatch(reserved.continuation.unwrap(), &store)
    else {
        panic!("the main child's turn must run inline")
    };
    // Keep this turn's reply outside the host. The runner's resource must
    // wait behind it in Mixed, with no browser operation to wait on.
    let outcome = work();
    let plan = contract::compile(
        "component App\n  state n = 1\n  resource value = number(n) as shape number\n  action next\n    n = n + 1\n  view\n    button \"Next\" press=next testId=\"next\"\n",
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    assert_eq!(bridge.requests_pending(), 0);
    let len = bridge.boot(&plan, data, 400., 400., "/");
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(!batch.contains("\"op\":\"request\""), "{batch}");
    assert!(!batch.contains("\"op\":\"continue\""), "{batch}");
    assert_eq!(bridge.requests_pending(), 1);
    let host = bridge.host.as_mut().unwrap();
    let ticket = host.runner().in_flight()[0].1;
    assert!(host.runner().held_requests().is_empty());
    let kernel = host.runner().kernel();
    let next = kernel
        .node_by_key(kernel.find_by_test_id("next")[0])
        .unwrap()
        .id;
    bridge.dispatch(next, 0, 0, 0.);
    assert_eq!(bridge.request_active(ticket as f64), 0);
    assert_eq!(bridge.requests_pending(), 1, "the replacement still waits");
    assert_ne!(
        bridge.host.as_ref().unwrap().runner().in_flight()[0].1,
        ticket
    );
    bridge
        .host
        .as_mut()
        .unwrap()
        .runner_mut()
        .data()
        .parse(&mut store, "number", &args, outcome)
        .unwrap();
    bridge.advance(0., 0);
    assert_eq!(bridge.requests_pending(), 0);
    assert_eq!(
        bridge.host.as_ref().unwrap().runner().resource("value"),
        Some(&Value::Number(2.))
    );
}

#[test]
fn pending_requests_exclude_declared_holds_and_streams_after_their_first_message() {
    use exact_runner::{Answer, Outcome, Request, Store};

    struct Network;
    impl DataSource for Network {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            unreachable!()
        }
        fn answer(
            &mut self,
            _: &mut Store,
            source: &str,
            _: &[Value],
        ) -> Result<Answer, DataError> {
            let request = Request::get("https://example.test/events");
            Ok(match source {
                "held" => Answer::Later(request.held()),
                "stream" => Answer::stream(request),
                _ => Answer::Later(request),
            })
        }
        fn parse(
            &mut self,
            _: &mut Store,
            _: &str,
            _: &[Value],
            _: Outcome,
        ) -> Result<Answer, DataError> {
            Ok(Answer::Now(Value::Number(1.)))
        }
    }
    for source in ["held", "stream", "device"] {
        let plan = contract::compile(&format!("component App\n  resource value = {source}() as shape number\n  view\n    text \"x\"\n")).unwrap().encode();
        let mut bridge = Bridge::new();
        bridge.boot(&plan, Network, 400., 400., "/");
        assert_eq!(bridge.requests_pending(), u32::from(source != "held"));
        let host = bridge.host.as_mut().unwrap();
        let ticket = host.runner().pending()[0].1;
        if source == "stream" {
            host.runner_mut()
                .fulfill(ticket, Outcome::Message(Default::default()))
                .unwrap();
        } else if source == "device" {
            host.runner_mut()
                .hold_request(ticket, "auth", "value", "{}");
        } else {
            assert_eq!(
                host.runner().held_requests(),
                vec![("value".into(), ticket)]
            );
        }
        assert_eq!(bridge.request_active(ticket as f64), 1);
        assert_eq!(bridge.requests_pending(), 0);
    }
}

/// A resource a turn over its budget refused is asked again at 250 ms
/// (exact-runner's `overrun.rs`): the agent's `settle` is that re-ask's time,
/// which the glue's `clock settle` seeks to, rather than stopping with the
/// resource pending.
#[test]
fn settle_is_a_reasks_time() {
    struct Busy(bool);
    impl DataSource for Busy {
        fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
            if args[0] == Value::Number(2.) && std::mem::take(&mut self.0) {
                return Err(DataError::Overrun(
                    "took 180.0 ms, over the 100 ms budget for one turn".into(),
                ));
            }
            Ok(args[0].clone())
        }
    }
    let plan = contract::compile(
        "component App\n  state n = 1\n  resource value = number(n) as shape number\n  action next\n    n = n + 1\n  view\n    text value.toString()\n",
    )
    .unwrap()
    .encode();
    let mut bridge = Bridge::new();
    bridge.boot(&plan, Busy(true), 400., 400., "/");
    let host = bridge.host.as_mut().unwrap();
    host.runner_mut().act("next", vec![]).unwrap();
    assert_eq!(host.runner().resource("value"), Some(&Value::Number(1.)));
    assert_eq!(host.agent("{\"op\":\"settle\"}"), "{\"settle\":250}");
}
