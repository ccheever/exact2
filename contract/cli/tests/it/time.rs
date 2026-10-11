//! @ref LLP 1027.000.000 — the date is a host fact: unknown (0) at bake and
//! boot, then one commit when the host says it; refused when implausible.
//! And tasks that wait for state (LLP 1092 D7–D11), below.
use exact_kernel::Kernel;
use exact_runner::{DataError, DataSource, Runner, RunnerError, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("host fact reached data: {source}")
    }
}

const APP: &str = "shape Time\n  epochAtZero: number\n  utcOffset: number\ncomponent App\n  resource time = exactTime() as shape Time\n  derive date = time.epochAtZero + performanceNow()\n  view\n    text `${date}/${time.utcOffset}` testId=\"date\"\n";

fn text(r: &Runner<NoData>) -> String {
    let key = r.kernel().find_by_test_id("date")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    node.props
        .str(exact_kernel::PropId::Text)
        .unwrap_or("")
        .to_string()
}

#[test]
fn the_date_arrives_as_one_commit_and_bad_facts_are_refused() {
    let plan = contract::bake(contract::compile(APP).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r), "0/0");
    assert!(r.set_time(1_790_000_000_000.0, 120.0).unwrap().is_some());
    assert_eq!(text(&r), "1790000000000/120");
    // Less than a second of drift is the same fact: no commit.
    assert!(r.set_time(1_790_000_000_400.0, 120.0).unwrap().is_none());
    assert!(matches!(
        r.set_time(f64::NAN, 0.0),
        Err(RunnerError::InvalidTime)
    ));
    assert!(matches!(
        r.set_time(1.0, 24.0 * 60.0),
        Err(RunnerError::InvalidTime)
    ));
    assert_eq!(text(&r), "1790000000000/120");
}

const PLACED: &str = "shape Time\n  epochAtZero: number\n  locale: string\n  timeZone: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text `${time.locale}|${time.timeZone}|${time.epochAtZero}` testId=\"date\"\n";

#[test]
fn the_locale_and_zone_arrive_beside_the_date_and_bad_ones_are_refused() {
    let plan = contract::bake(contract::compile(PLACED).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // Usable by Intl even before the host reports its place.
    assert_eq!(text(&r), "en-US|UTC|0");
    assert!(r
        .set_place("en-GB", "Europe/London", None)
        .unwrap()
        .is_some());
    assert_eq!(text(&r), "en-GB|Europe/London|0");
    assert!(r
        .set_place("en-GB", "Europe/London", None)
        .unwrap()
        .is_none());
    assert!(r.set_time(1_790_000_000_000.0, 60.0).unwrap().is_some());
    assert_eq!(text(&r), "en-GB|Europe/London|1790000000000");
    for (locale, zone) in [
        ("", "UTC"),
        ("en GB", "UTC"),
        ("en", ""),
        ("en", "Europe/London; rm"),
    ] {
        assert!(matches!(
            r.set_place(locale, zone, None),
            Err(RunnerError::InvalidPlace)
        ));
    }
    assert!(r
        .set_place("pt-BR", "America/Sao_Paulo", None)
        .unwrap()
        .is_some());
    assert_eq!(text(&r), "pt-BR|America/Sao_Paulo|1790000000000");
}

#[test]
fn the_launch_seed_arrives_as_a_host_fact_and_nothing_else_is_one() {
    const SEEDED: &str = "shape Time\n  seed: number\n  locale: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text `${time.seed}|${time.locale}` testId=\"date\"\n";
    let plan = contract::bake(contract::compile(SEEDED).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r), "0|en-US", "zero seed until the host says");
    let epoch = r.kernel().epoch();
    let receipt = r
        .set_place("fr-FR", "Europe/Paris", Some(123_456_789.0))
        .unwrap()
        .unwrap();
    assert_eq!(receipt.epoch, epoch + 1, "place and seed commit together");
    assert_eq!(text(&r), "123456789|fr-FR");
    assert!(r
        .set_place("en-GB", "Europe/London", None)
        .unwrap()
        .is_some());
    assert_eq!(text(&r), "123456789|en-GB", "a place keeps the seed");
    assert!(r
        .set_place("en-GB", "Europe/London", Some(123_456_789.0))
        .unwrap()
        .is_none());
    for bad in [-1.0, 0.5, f64::NAN, 9_007_199_254_740_992.0] {
        assert!(matches!(
            r.set_place("fr-FR", "Europe/Paris", Some(bad)),
            Err(RunnerError::InvalidPlace)
        ));
        assert_eq!(
            text(&r),
            "123456789|en-GB",
            "bad seed refuses the whole place"
        );
    }
}

// ---------------------------------------------------------------- gated tasks
// @ref LLP 1092 D7–D11 — a task that exists while its gate holds, restarted
// by a new key; nothing runs when the gate changes.

/// Answers `flag()` with its field, `items()` with one row, `later()` with a
/// request, and `calc(n)` with `{ n }` now.
#[derive(Default)]
struct Gated {
    flag: bool,
}
impl DataSource for Gated {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "flag" => Ok(Value::Bool(self.flag)),
            "items" => Ok(Value::list(vec![Value::str("a")])),
            "calc" => Ok(Value::record(vec![args[0].clone()])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
    fn answer(
        &mut self,
        _: &mut exact_runner::Store,
        source: &str,
        args: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        match source {
            "later" => Ok(exact_runner::Answer::Later(exact_runner::Request::get(
                "https://gated.test/later",
            ))),
            _ => self.query(source, args).map(exact_runner::Answer::Now),
        }
    }
    fn parse(
        &mut self,
        _: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
        _: exact_runner::Outcome,
    ) -> Result<exact_runner::Answer, DataError> {
        Ok(exact_runner::Answer::Now(Value::record(vec![
            Value::Number(1.0),
        ])))
    }
}

fn gated(src: &str, data: Gated) -> Runner<Gated> {
    let plan = contract::bake(contract::compile(src).unwrap(), Gated { flag: data.flag }).unwrap();
    Runner::boot(
        plan,
        data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn num(r: &Runner<Gated>, slot: &str) -> f64 {
    r.slot(slot).and_then(Value::as_number).unwrap()
}

const GATED: &str = "component App
  state on = false
  state n = 0
  state k = 0
  state other = 0
  action set(v: bool)
    on = v
  action rekey(v: number)
    k = v
  action bump
    other = other + 1
  action tick
    n = n + 1
  task t when on
    after(1000, tick)
  task r key=k
    after(1000, tick)
  view
    text `${n}`
";

#[test]
fn a_gate_on_then_off_before_its_due_time_fires_nothing_and_idle_reports_no_time() {
    let mut r = gated(
        &GATED.replace("  task r key=k\n    after(1000, tick)\n", ""),
        Gated::default(),
    );
    assert_eq!(
        r.timer_due_ms(),
        None,
        "an idle gated task keeps no host awake"
    );
    r.act("set", vec![Value::Bool(true)]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(1000.0));
    r.advance(500.0).unwrap();
    r.act("set", vec![Value::Bool(false)]).unwrap();
    assert_eq!(r.timer_due_ms(), None);
    r.advance(5000.0).unwrap();
    assert_eq!(num(&r, "n"), 0.0, "nothing ran");
    // On again: armed from now, not from the first arming.
    r.act("set", vec![Value::Bool(true)]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(6000.0));
}

#[test]
fn a_key_change_re_arms_a_nan_key_refuses_and_negative_zero_is_zero() {
    // A key computed past every number: NaN is no key (slots and derives
    // hold only finite numbers).
    let src = GATED
        .replace("  task t when on\n    after(1000, tick)\n", "")
        .replace("task r key=k", "task r key=k == 5 ? 0 / 0 : k");
    let mut r = gated(&src, Gated::default());
    assert_eq!(
        r.timer_due_ms(),
        Some(1000.0),
        "`key=` alone is `when true`"
    );
    r.advance(400.0).unwrap();
    r.act("rekey", vec![Value::Number(1.0)]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(1400.0));
    r.advance(600.0).unwrap();
    r.act("rekey", vec![Value::Number(1.0)]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(1400.0), "an unchanged key leaves it");
    assert!(matches!(
        r.act("rekey", vec![Value::Number(5.0)]),
        Err(RunnerError::TaskKey { task }) if task == "r"
    ));
    assert_eq!(num(&r, "k"), 1.0, "the refused commit wrote nothing");
    assert_eq!(r.timer_due_ms(), Some(1400.0), "and moved no timer");
    r.act("rekey", vec![Value::Number(0.0)]).unwrap();
    assert_eq!(r.timer_due_ms(), Some(1600.0));
    r.advance(700.0).unwrap();
    r.act("rekey", vec![Value::Number(-0.0)]).unwrap();
    assert_eq!(
        r.timer_due_ms(),
        Some(1600.0),
        "`-0` is `0`, as an `each` key"
    );
}

#[test]
fn a_spent_after_stays_spent_and_an_every_that_clears_its_gate_stops() {
    let mut r = gated(
        &GATED.replace("  task r key=k\n    after(1000, tick)\n", ""),
        Gated::default(),
    );
    r.act("set", vec![Value::Bool(true)]).unwrap();
    r.advance(1000.0).unwrap();
    assert_eq!(num(&r, "n"), 1.0);
    r.act("bump", vec![]).unwrap();
    assert_eq!(r.timer_due_ms(), None, "spent while the gate holds");
    let every = GATED
        .replace("  task r key=k\n    after(1000, tick)\n", "")
        .replace(
            "    n = n + 1\n",
            "    n = n + 1\n    if n >= 2\n      on = false\n",
        )
        .replace("after(1000, tick)", "every(100, tick)");
    let mut r = gated(&every, Gated::default());
    r.act("set", vec![Value::Bool(true)]).unwrap();
    r.advance(1000.0).unwrap();
    assert_eq!(num(&r, "n"), 3.0, "its own fire's commit dropped it");
    assert_eq!(r.timer_due_ms(), None);
}

#[test]
fn a_boot_gate_reads_a_derive_over_a_baked_answer() {
    let src = "component App
  state x = 1
  state n = 0
  resource flag = flag() as shape bool
  derive go = flag and x > 0
  action tick
    n = n + 1
  task t when go
    after(10, tick)
  view
    text `${n}`
";
    assert_eq!(gated(src, Gated { flag: true }).timer_due_ms(), Some(10.0));
    assert_eq!(gated(src, Gated { flag: false }).timer_due_ms(), None);
}

/// D8: an `after` fires at its deadline exactly, so an action that
/// re-tests it with a strict `>` does nothing; one that clears clears.
#[test]
fn an_after_fires_at_its_deadline_so_a_strict_retest_does_nothing() {
    let src = "component App
  state toast = \"\"
  state until = 0
  action show
    toast = \"hi\"
    until = performanceNow() + 5000
  action expire
    if performanceNow() > until
      toast = \"\"
  task hide when toast != \"\" key=until
    after(5000, expire)
  view
    text toast
";
    let mut r = gated(src, Gated::default());
    r.act("show", vec![]).unwrap();
    r.advance(60_000.0).unwrap();
    assert_eq!(r.slot("toast"), Some(&Value::str("hi")), "up forever");
    let mut r = gated(
        &src.replace(
            "    if performanceNow() > until\n      toast = \"\"\n",
            "    toast = \"\"\n",
        ),
        Gated::default(),
    );
    r.act("show", vec![]).unwrap();
    r.advance(5000.0).unwrap();
    assert_eq!(r.slot("toast"), Some(&Value::str("")));
    assert_eq!(r.timer_due_ms(), None);
}

#[test]
fn frames_skip_an_idle_frame_task_a_seek_never_fires_one_and_wants_frames_follows() {
    let src = "component App
  state flying = false
  state framed = 0
  action fly(v: bool)
    flying = v
  action step
    framed = framed + 1
  task f when flying
    every(frame, step)
  view
    text `${framed}`
";
    let mut r = gated(src, Gated::default());
    r.present_frames(true);
    assert!(!r.wants_frames());
    assert!(r.frame(16.0).receipts.is_empty());
    r.act("fly", vec![Value::Bool(true)]).unwrap();
    assert!(r.wants_frames());
    r.frame(32.0);
    r.frame(48.0);
    assert_eq!(num(&r, "framed"), 2.0);
    r.act("fly", vec![Value::Bool(false)]).unwrap();
    assert!(!r.wants_frames());
    r.frame(64.0);
    // A seek: an idle frame task's virtual frames never fire.
    r.present_frames(false);
    r.advance(2000.0).unwrap();
    assert_eq!(num(&r, "framed"), 2.0);
    // Armed while seeking: virtual frames from the commit's time.
    r.act("fly", vec![Value::Bool(true)]).unwrap();
    r.advance(3000.0).unwrap();
    assert_eq!(num(&r, "framed"), 62.0, "sixty virtual frames a second");
}

/// b6 review A3: a frame task armed by another frame task's commit in the
/// same frame waits for the next frame, as the web's `paint`, which fires
/// the tasks armed as the frame starts; one dropped there does not fire.
#[test]
fn a_frame_task_armed_mid_frame_waits_for_the_next_one() {
    let src = "component App
  state second = false
  state a = 0
  state b = 0
  action stepA
    a = a + 1
    second = not second
  action stepB
    b = b + 1
  task fa mount
    every(frame, stepA)
  task fb when second
    every(frame, stepB)
  view
    text `${a} ${b}`
";
    let mut r = gated(src, Gated::default());
    r.present_frames(true);
    r.frame(16.0);
    assert_eq!((num(&r, "a"), num(&r, "b")), (1.0, 0.0), "armed mid-frame");
    r.frame(32.0);
    assert_eq!(
        (num(&r, "a"), num(&r, "b")),
        (2.0, 0.0),
        "dropped by fa before its turn"
    );
    r.frame(48.0);
    assert_eq!((num(&r, "a"), num(&r, "b")), (3.0, 0.0));
}

#[test]
fn a_gate_armed_inside_an_advance_fires_within_it() {
    let src = GATED
        .replace("  task r key=k\n    after(1000, tick)\n", "")
        .replace("after(1000, tick)", "after(50, tick)")
        .replace(
            "  view\n",
            "  action arm\n    on = true\n  task a mount\n    after(100, arm)\n  view\n",
        );
    let mut r = gated(&src, Gated::default());
    r.advance(1000.0).unwrap();
    assert_eq!(num(&r, "n"), 1.0, "armed at 100, fired at 150");
}

/// D10's seek rule: one `advance(60_000)` equals sixty `advance(1_000)`s,
/// with a gate that turns on and off and a key that re-arms on the way.
#[test]
fn one_long_advance_equals_sixty_short_ones() {
    let src = "component App
  state on = true
  state n = 0
  state m = 0
  state k = 0
  action tick
    n = n + 1
    if n % 5 == 0
      on = false
      k = k + 1
  action wake
    m = m + 1
    on = true
  task beat when on
    every(700, tick)
  task again key=k
    after(1300, wake)
  view
    text `${n} ${m}`
";
    let mut long = gated(src, Gated::default());
    long.advance(60_000.0).unwrap();
    let mut short = gated(src, Gated::default());
    for s in 1..=60 {
        short.advance(f64::from(s) * 1000.0).unwrap();
    }
    for slot in ["on", "n", "m", "k"] {
        assert_eq!(long.slot(slot), short.slot(slot), "{slot}");
    }
    assert_eq!(long.timer_due_ms(), short.timer_due_ms());
    assert!(num(&long, "m") > 5.0, "the gate cycled");
}

/// The driver's case (D10): a gated tick sends; the reply's `then` clears
/// the gate. A jump stops at the commit that hands out the request and
/// lands its reply first, so `clock +60000` and sixty `clock +1000` stop
/// together.
#[test]
fn a_gated_tick_whose_then_clears_the_gate_stops_the_same_under_any_jump() {
    let src = "shape Ack
  n: number
component App
  state on = true
  state ticks = 0
  mutation m as shape Ack then done
  action tick
    ticks = ticks + 1
    send m = later()
  action done
    on = false
  task beat when on
    every(1000, tick)
  view
    text `${ticks}`
";
    let jump = |r: &mut Runner<Gated>, to: f64| loop {
        let a = r.advance_until_request(to);
        assert!(a.error.is_none());
        let requests = r.take_requests();
        for q in &requests {
            r.fulfill(
                q.ticket,
                exact_runner::Outcome::Failed {
                    kind: exact_runner::FailureKind::Network,
                    message: "offline".into(),
                },
            )
            .unwrap();
        }
        if requests.is_empty() && a.now_ms >= to {
            break;
        }
    };
    let mut long = gated(src, Gated::default());
    jump(&mut long, 60_000.0);
    let mut short = gated(src, Gated::default());
    for s in 1..=60 {
        jump(&mut short, f64::from(s) * 1000.0);
    }
    for r in [&long, &short] {
        assert_eq!(num(r, "ticks"), 1.0);
        assert_eq!(r.slot("on"), Some(&Value::Bool(false)));
        assert_eq!(r.timer_due_ms(), None);
    }
}

/// D8: the gate step is on the settlement path, inside its rollback. A
/// key that refuses a row action's commit leaves the row slot it wrote and
/// the request it sent unapplied.
#[test]
fn a_gate_refusal_rolls_back_row_slots_and_requests() {
    let src = "shape Ack
  n: number
component App
  state bad = false
  resource items = items() as shape list<string>
  mutation m as shape Ack
  action go
    send m = later()
    bad = true
  action tick
    bad = false
  task t key=bad ? 0 / 0 : 1
    after(1000, tick)
  view
    column
      each it in items key=it
        Row(id=it, go=go)
component Row
  props
    id: string
    go: action
  state n = 0
  action press
    n = n + 1
    go()
  view
    button press=press testId=`row-${id}`
      text `${n}` testId=`n-${id}`
";
    let mut r = gated(src, Gated::default());
    let row = |r: &Runner<Gated>| {
        let key = r.kernel().find_by_test_id("row-a")[0];
        r.kernel().node_by_key(key).unwrap().id
    };
    let label = |r: &Runner<Gated>| {
        let k = r.kernel();
        let key = k.find_by_test_id("n-a")[0];
        let node = k.node_by_key(key).unwrap();
        node.props
            .str(exact_kernel::PropId::Text)
            .unwrap_or("")
            .to_string()
    };
    assert_eq!(label(&r), "0");
    let e = r.dispatch(row(&r), exact_runner::Event::Press).unwrap_err();
    assert!(matches!(e, RunnerError::TaskKey { .. }), "{e:?}");
    assert!(r.take_requests().is_empty(), "no request went out");
    assert!(r.pending().is_empty());
    r.act("tick", vec![]).unwrap();
    assert_eq!(label(&r), "0", "the row slot was put back");
}

#[test]
fn a_task_gate_parses_in_three_forms_and_lowers_to_plan_code() {
    let plan = contract::compile(GATED).unwrap();
    let rows: Vec<_> = plan
        .timers
        .iter()
        .map(|t| (plan.str(t.name), t.gated, t.keyed))
        .collect();
    assert_eq!(rows, [("t", true, false), ("r", true, true)]);
    let both = GATED.replace("task t when on\n", "task t when on and n < 3 key=k\n");
    let plan = contract::compile(&both).unwrap();
    assert!(plan.timers[0].gated && plan.timers[0].keyed);
    // `mount` stays ungated, its gate the constant `true`.
    let plan = contract::compile(&GATED.replace("task t when on\n", "task t mount\n")).unwrap();
    assert!(!plan.timers[0].gated && !plan.timers[0].keyed);
    let e = contract::compile(&GATED.replace("task t when on\n", "task t\n")).unwrap_err();
    assert_eq!(e.id, "syntax-task-start");
}

/// D9's refusals, each message whole.
#[test]
fn a_gate_takes_a_bool_a_key_a_scalar_and_neither_reads_the_clock() {
    let src = |gate: &str| {
        format!("shape P\n  x: number\nfn late(t: number): bool = performanceNow() > t\ncomponent App\n  state toast = \"\"\n  state p = P(x=1)\n  state n = 0\n  derive clock = performanceNow() > 5\n  action tick\n    n = n + 1\n  task hide {gate}\n    after(5000, tick)\n  view\n    text toast\n")
    };
    let says = |gate: &str, id: &str, message: &str| {
        let e = contract::compile(&src(gate)).unwrap_err();
        assert_eq!((e.id.as_str(), e.message.as_str()), (id, message), "{gate}");
    };
    says(
        "when toast",
        "type-task-gate",
        "`when` takes a bool; `toast` is a string; write `toast != \"\"`",
    );
    says(
        "key=p",
        "type-task-key",
        "a task's `key=` is a string, number or bool, as an `each` key is; this one is P",
    );
    let clock = "A gate is read at commits, not as the clock moves, so an `after` gated on `performanceNow()` cannot be dropped before it fires, and an `every` stops up to an interval late. Gate on state (`toast != \"\"`) and let `after(5000, …)` measure the time";
    says(
        "when performanceNow() > 100",
        "analyze-task-gate-clock",
        &format!("`task hide`'s gate reads `performanceNow()`. {clock}"),
    );
    says(
        "when clock",
        "analyze-task-gate-clock",
        &format!("`task hide`'s gate reads `performanceNow()` through `clock`. {clock}"),
    );
    says(
        "when toast != \"\" key=late(3)",
        "analyze-task-gate-clock",
        &format!("`task hide`'s key reads `performanceNow()` through `fn late`. {clock}"),
    );
    contract::compile(&src("when toast != \"\" key=n")).unwrap();
}

/// @ref LLP 1109 D1 — `now()` is deleted, not deprecated: wherever an
/// expression may call it, it is refused at its own span with both repairs
/// in one sentence, and an app's `fn now` still shadows nothing away.
#[test]
fn now_is_refused_with_both_repairs_at_its_span() {
    let cases = [
        ("  derive d = 1 + now()\n  view\n    text toString(d)\n", 4, 18),
        ("  view\n    text `${now()}`\n", 5, 13),
        ("  state at = 0\n  action stamp\n    at = now() + 7 * 86400000\n  view\n    text toString(at)\n", 6, 10),
        ("  state at = 0\n  action set(t: number)\n    at = t\n  view\n    button \"s\" press=set(now())\n", 8, 26),
    ];
    for (body, line, col) in cases {
        let src = format!("shape Clock\n  epochAtZero: number\ncomponent App\n{body}");
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, "type-now-renamed", "{body}: {e}");
        assert_eq!(
            (e.span.line, e.span.col, e.span.end_col),
            (line, col, col + 3),
            "{body}: {e}"
        );
        for repair in [
            "`performanceNow()` for durations and timers",
            "`time.epochAtZero + performanceNow()` for the date",
            "`resource time = exactTime() as shape Clock`",
        ] {
            assert!(e.message.contains(repair), "{body}: {}", e.message);
        }
        assert_eq!(
            e.message.matches(". ").count(),
            0,
            "one sentence: {}",
            e.message
        );
    }
    let e = contract::compile(
        "fn f(): number = now()\ncomponent App\n  view\n    text toString(f())\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-now-renamed", "{e}");
    // An app's own `fn now` shadows the refusal as it would a roster name.
    contract::compile("fn now(): number = 7\ncomponent App\n  view\n    text toString(now())\n")
        .unwrap();
    // The web's spellings meet the same repairs.
    for web in ["Date.now()", "performance.now()"] {
        let e = contract::compile(&format!(
            "component App\n  view\n    text toString({web})\n"
        ))
        .unwrap_err();
        assert!(
            e.message
                .contains("`performanceNow()` for durations and timers"),
            "{web}: {e}"
        );
    }
}
