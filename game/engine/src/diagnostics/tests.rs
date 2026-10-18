use super::*;
use crate::{Clock, Game, Input, Sim, Transform, World};
thread_local! { static TIME: std::cell::Cell<f64> = const { std::cell::Cell::new(0.) }; }
fn now() -> f64 {
    TIME.with(|t| t.get())
}
fn add(ms: f64) {
    TIME.with(|t| t.set(t.get() + ms));
}
struct Nested;
impl Game for Nested {
    const ID: &'static str = "spans";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn(Transform::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        add(1.);
        let outer = w.span("outer");
        add(2.);
        {
            let _inner = w.span("inner");
            add(3.);
        }
        add(2.);
        drop(outer);
        {
            let _again = w.span("inner");
            add(1.);
        }
        add(1.);
    }
}
#[allow(non_snake_case)]
#[derive(Default, crate::Data)]
struct Sample {
    name: String,
    at: String,
    p50: f64,
    p95: f64,
    perTick: f64,
    count: u64,
}
#[allow(non_snake_case)]
#[derive(Default, crate::Data)]
struct Report {
    tickMs: Distribution,
    spans: Vec<Sample>,
}
#[test]
fn spans_are_exclusive_aggregate_per_tick_and_never_change_saves_or_hashes() {
    let mut off = Sim::<Nested>::new(()).unwrap();
    let mut on = Sim::<Nested>::new(()).unwrap();
    on.measure_spans(now, false);
    for sim in [&mut off, &mut on] {
        sim.advance(0., Clock::Seekable);
        sim.advance(50., Clock::Seekable);
    }
    assert_eq!(off.world().hash(), on.world().hash());
    assert_eq!(off.save().unwrap(), on.save().unwrap());
    let report: Report = crate::json::from_str(&on.spans_json(None)).unwrap();
    let find = |name| report.spans.iter().find(|s| s.name == name).unwrap();
    assert_eq!(
        (find("outer").p50, find("inner").p50, find("tick.other").p50),
        (4., 4., 2.)
    );
    assert!(find("outer").at.contains("diagnostics/tests.rs:"));
    assert_eq!(find("outer").count, 3);
    assert_eq!(
        report.spans.iter().map(|s| s.perTick).sum::<f64>(),
        report.tickMs.mean
    );
    assert_eq!(report.tickMs.mean, 10.);
    let delta: Report = crate::json::from_str(&on.spans_json(Some(2))).unwrap();
    assert_eq!(delta.spans[0].count, 1);
    assert!(on.spans_json(Some(4)).contains("counters went back"));
    assert_eq!(find("inner").p95, 4.);
}

#[derive(Default, crate::Resource)]
struct Visited(u32);
struct Footgun;
impl Game for Footgun {
    const ID: &'static str = "span-query-scaling";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Visited::default());
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let _span = w.span("render-column-query");
        // Constructing the exclusive render-column query touches its pages even
        // when the loop writes nothing. The read-only control omits that term.
        let count = if w.resource::<Visited>().0 == 0 {
            w.query::<&mut Transform>().iter().count()
        } else {
            std::hint::black_box(1)
        };
        std::hint::black_box(count);
    }
}
#[test]
#[ignore = "wall-clock observation: run explicitly, never a timing gate"]
fn hundred_thousand_entity_query_span() {
    for count in [1000, 100_000] {
        for exclusive in [true, false] {
            let mut sim = Sim::<Footgun>::new(()).unwrap();
            for _ in 0..count {
                sim.world_mut().spawn(Transform::default());
            }
            sim.world().resource_mut::<Visited>().0 = u32::from(!exclusive);
            sim.measure_spans(clock, true);
            sim.advance(0., Clock::Live);
            for tick in 1..=40 {
                sim.advance(tick as f64 * 1000. / 60., Clock::Live);
            }
            let text = sim.spans_json(None);
            let report: Report = crate::json::from_str(&text).unwrap();
            assert!(report
                .spans
                .iter()
                .any(|s| s.name == "render-column-query" && s.count >= 39));
            println!("perf world: entities={count} exclusive={exclusive}: {text}");
        }
    }
}

#[test]
fn ten_seconds_of_tick_spans_are_joined_to_submitted_frames() {
    let mut sim = Sim::<Nested>::new(()).unwrap();
    sim.measure_spans(now, true);
    sim.advance(0., Clock::Seekable);
    for tick in 1..=600 {
        sim.advance(tick as f64 * 1000. / 60. + 0.001, Clock::Seekable);
        sim.trace_frame();
    }
    #[derive(Default, crate::Data)]
    struct Args {
        tick: u64,
        frame: u64,
    }
    #[derive(Default, crate::Data)]
    struct Interval {
        name: String,
        ts: f64,
        dur: f64,
        args: Args,
    }
    let events = sim.trace_events_json();
    let rows: Vec<Interval> = crate::json::from_str(&events).unwrap();
    let ticks: Vec<_> = rows.iter().filter(|e| e.name == "tick").collect();
    assert_eq!(ticks.len(), 600);
    for tick in ticks {
        let frame = rows
            .iter()
            .find(|e| e.name == "frame" && e.args.frame == tick.args.frame)
            .unwrap();
        assert!(frame.ts <= tick.ts && frame.ts + frame.dur >= tick.ts + tick.dur);
        assert_eq!(tick.args.tick, tick.args.frame);
    }
    if let Ok(path) = std::env::var("EXACT_DIAGNOSTIC_TRACE") {
        std::fs::write(path, format!("{{\"traceEvents\":{events}}}")).unwrap();
    }
    println!(
        "trace export: 600 ticks under 600 submitted frames; {} intervals",
        rows.len()
    );
}

#[test]
fn a_difference_from_zero_refuses_when_its_prefix_has_expired() {
    let mut sim = Sim::<Nested>::new(()).unwrap();
    sim.measure_spans(now, true);
    sim.advance(0., Clock::Seekable);
    sim.advance(300_000., Clock::Seekable);
    assert!(sim.spans_json(Some(0)).contains("window expired"));
    assert!(!sim.spans_json(None).contains("error"));
}
