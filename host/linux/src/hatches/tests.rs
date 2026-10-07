//! The hatches' store, queue and crash breadcrumb against their bounds (LLP
//! 1075.003.000.001 §2.2.1, §2.5, §3.2, §4.4). The presenter's half, driven
//! through a booted plan, is `presenter/hatch_tests.rs`.
use super::breadcrumb::{Breadcrumb, DEPTH_MAX, SLOTS};
use super::diagnostics::{Store, NAMES, OPEN_SPANS, SAMPLES, SNAPSHOTS};
use super::session::{ACTS, ACT_BYTES, INPUT_BYTES, OVERLAY_BYTES, OVERLAY_OPS, SNAPSHOT};
use super::{ActKind, Element, Node, Rect, Shared};
use serde_json::{json, Map, Value};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn state(store: &Store) -> Value {
    Value::Object(store.state(Map::new()))
}

fn perf(store: &Store) -> Value {
    store.perf(Map::new(), 0)
}

#[test]
fn diagnostics_are_bounded_refused_past_their_bounds_and_scoped() {
    let mut d = Store::new(true);
    // Counters: 64 names a module, shared by every scope; a 65th is refused.
    for i in 0..NAMES {
        d.count("module", &format!("c{i}"), 1);
    }
    d.count("module", "c0", 4);
    d.count("element badge", "one-too-many", 1);
    let s = state(&d);
    assert_eq!(s["scopes"]["module"]["counters"]["c0"], 5);
    assert_eq!(s["rejected"], 1, "the 65th counter name is refused");
    assert!(s["words"].get("badge").is_none(), "{s}");
    // A bad name is refused and counted; the first 8 distinct are journaled.
    let before = d.lines.len();
    for i in 0..10 {
        d.count("module", &format!("Bad Name {i}"), 1);
    }
    d.count("module", "Bad Name 0", 1);
    d.count("module", "", 1);
    d.count("module", &"x".repeat(65), 1);
    assert_eq!(state(&d)["rejected"], 1 + 13);
    assert_eq!(d.lines.len() - before, 8, "{:?}", d.lines);
    assert!(d.lines[before].starts_with("module: diagnostics refused the name \"Bad Name 0\""));

    // Timings: a cumulative count, sum and max, and a ring of the last 256.
    let mut d = Store::new(true);
    for i in 0..SAMPLES + 10 {
        d.sample("element dot", "draw", i as f64, true);
    }
    d.sample("element dot", "draw", -1.0, true);
    d.sample("element dot", "draw", f64::NAN, true);
    let t = &perf(&d)["timings"]["element dot"]["draw"];
    assert_eq!(t["count"], SAMPLES as u64 + 10);
    assert_eq!(t["samples"], SAMPLES as u64);
    assert_eq!(
        t["dropped"], 10,
        "the ring overwrites and counts what it lost"
    );
    assert_eq!(t["max"], (SAMPLES + 9) as f64);
    assert_eq!(t["measured"], true);
    assert_eq!(perf(&d)["rejected"], 2, "a negative or non-finite sample");
    for i in 0..NAMES {
        d.sample("module", &format!("t{i}"), 1.0, true);
    }
    assert_eq!(perf(&d)["rejected"], 3, "a 65th timing name");

    // Spans: on the session clock, 32 open at once; a node's end abandons its own.
    let mut d = Store::new(true);
    let span = d.begin("element badge", "shown", 7, 100.0);
    d.end(span, 350.0);
    d.end(span, 900.0);
    let t = &perf(&d)["timings"]["element badge"]["shown"];
    assert_eq!(
        (t["count"].as_u64(), t["sum"].as_f64()),
        (Some(1), Some(250.0))
    );
    assert!(
        t.get("measured").is_none(),
        "a span is on the session clock"
    );
    let open: Vec<u64> = (0..OPEN_SPANS)
        .map(|i| d.begin("element badge", "held", 7 + (i as u32 % 2), 0.0))
        .collect();
    assert!(open.iter().all(|id| *id != 0));
    assert_eq!(d.begin("module", "one-more", 0, 0.0), 0, "an inert span");
    assert_eq!(state(&d)["rejected"], 1);
    d.ended(7);
    assert_eq!(state(&d)["abandoned"], OPEN_SPANS as u64 / 2);
    d.end(open[0], 50.0);
    assert!(perf(&d)["timings"]["element badge"].get("held").is_none());

    // Snapshots: 16 names, 4 KB of JSON each, the latest kept, refused whole.
    let mut d = Store::new(true);
    d.publish("module", "last", r#"{"n":1}"#);
    d.publish("module", "last", r#"{"n":2}"#);
    d.publish("module", "last", &format!("\"{}\"", "x".repeat(4096)));
    d.publish("module", "last", "{not json");
    assert_eq!(
        state(&d)["scopes"]["module"]["published"]["last"],
        json!({"n": 2})
    );
    assert_eq!(state(&d)["rejected"], 2);
    for i in 0..SNAPSHOTS {
        d.publish("element badge", &format!("s{i}"), "1");
    }
    assert_eq!(state(&d)["rejected"], 3, "a 17th snapshot name");
    assert_eq!(state(&d)["words"]["badge"]["published"]["s0"], 1);

    // Lines: 256 bytes, 20 a second of session clock a scope, then one for the rest.
    let mut d = Store::new(true);
    d.log("module", &"é".repeat(200), 0.0);
    assert_eq!(d.lines[0].len(), "module: ".len() + 255);
    assert!(d.lines[0].ends_with('…'));
    for i in 0..30 {
        d.log("element dot", &format!("line {i}"), 10.0);
    }
    d.log("module", "another scope has its own budget", 10.0);
    assert_eq!(d.lines.len(), 1 + 20 + 1);
    assert_eq!(state(&d)["limited"], 10);
    d.log("element dot", "the next second", 1010.0);
    assert_eq!(d.lines[d.lines.len() - 2], "element dot: … 10 more");
    assert_eq!(d.lines.last().unwrap(), "element dot: the next second");

    // A production build keeps nothing: each call returns on one flag.
    let mut d = Store::new(false);
    d.count("module", "n", 1);
    d.log("module", "said", 0.0);
    d.publish("module", "s", "1");
    d.sample("module", "t", 1.0, true);
    assert_eq!(d.begin("module", "span", 0, 0.0), 0);
    let started = d.begin_call();
    d.end_call(started, "app", None, "built", true);
    assert!(d.lines.is_empty());
    assert_eq!(
        state(&d),
        json!({"words": {}, "scopes": {}, "measuring": false, "rejected": 0, "abandoned": 0, "limited": 0})
    );
    assert_eq!(perf(&d)["calls"], json!([]));

    // A reply is at most 64 KB: the longest list is cut, and it says so.
    let mut d = Store::new(true);
    for i in 0..SNAPSHOTS {
        let scope = if i % 2 == 0 {
            "module"
        } else {
            "element badge"
        };
        d.publish(
            scope,
            &format!("s{i}"),
            &format!("\"{}\"", "y".repeat(4094)),
        );
    }
    let big = Value::Object(super::diagnostics::fit(
        d.state(Map::new()),
        &["words", "scopes"],
    ));
    assert_eq!(big["rejected"], 0, "each fits its 4 KB");
    assert!(big.to_string().len() <= 65536 && big["truncated"] == true);
}

#[test]
fn a_call_is_timed_by_site_and_moment_and_a_nested_call_is_charged_to_itself() {
    let mut d = Store::new(true);
    let outer = d.begin_call();
    std::thread::sleep(std::time::Duration::from_millis(4));
    let inner = d.begin_call();
    std::thread::sleep(std::time::Duration::from_millis(12));
    d.end_call(inner, "element dot", Some(9), "changed", false);
    d.end_call(outer, "app", None, "built", true);
    let p = perf(&d);
    let (app, dot) = (&p["hatches"]["app"], &p["hatches"]["element dot"]);
    assert_eq!(
        (app["calls"].as_u64(), dot["calls"].as_u64()),
        (Some(1), Some(1))
    );
    assert!(dot["ms"].as_f64().unwrap() >= 12.0, "{p}");
    assert!(
        app["ms"].as_f64().unwrap() < dot["ms"].as_f64().unwrap(),
        "the outer call's time is its own: {p}"
    );
    assert_eq!(dot["worst"], dot["ms"]);
    let row = p["calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["hatch"] == "element dot");
    assert_eq!(row.unwrap()["site"], 9);
    assert_eq!(row.unwrap()["moment"], "changed");
    assert_eq!(d.site(9).map(|(calls, _)| calls), Some(1));
    assert_eq!(d.site(4), None);
    // A scope's calls are counted by moment; a node's are its word's.
    assert_eq!(state(&d)["scopes"], json!({"app": {"calls": {"built": 1}}}));
    // A read changes nothing.
    assert_eq!(perf(&d), p);
    d.reset();
    assert_eq!(perf(&d)["calls"], json!([]));
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("exact-crumb-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_run_that_ended_inside_a_hatch_is_read_by_the_next_once() {
    let dir = scratch("death");
    let mut crumbs = Breadcrumb::open(&dir, 4127).unwrap();
    let slot = crumbs.take("").unwrap();
    for _ in 0..3 {
        crumbs.push(slot, "app", "built", 1);
        crumbs.pop(slot);
    }
    crumbs.push(slot, "frames", "tick", 1);
    crumbs.push(slot, "element avatar", "changed", 2);
    // A live process's file is not read: that run has not ended.
    assert!(Breadcrumb::read(&dir).is_empty());
    assert!(crumbs.path().exists());
    // The process dies here: nothing pops, nothing is deleted.
    crumbs.close(false);
    assert_eq!(
        Breadcrumb::read(&dir),
        ["the last run ended while inside hatch element avatar (changed, call 5); if it repeats, launch with EXACT_HATCHES=off"]
    );
    assert!(
        Breadcrumb::read(&dir).is_empty(),
        "a breadcrumb is read once"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clean_run_leaves_nothing_and_nesting_past_eight_is_counted() {
    let dir = scratch("clean");
    let mut crumbs = Breadcrumb::open(&dir, 1).unwrap();
    let slot = crumbs.take("main").unwrap();
    crumbs.push(slot, "element dot", "built", 1);
    crumbs.pop(slot);
    crumbs.close(false);
    assert!(Breadcrumb::read(&dir).is_empty(), "every call returned");
    // A run that exits cleanly deletes its own file.
    let crumbs = Breadcrumb::open(&dir, 2).unwrap();
    let path = crumbs.path().to_owned();
    crumbs.close(true);
    assert!(!path.exists());
    // Eleven calls deep: eight records, three counted; pops undo them in order.
    let mut crumbs = Breadcrumb::open(&dir, 3).unwrap();
    let slot = crumbs.take("main").unwrap();
    for depth in 0..DEPTH_MAX + 3 {
        crumbs.push(slot, &format!("element n{depth}"), "changed", 1);
    }
    crumbs.pop(slot);
    crumbs.close(false);
    assert_eq!(
        Breadcrumb::read(&dir),
        ["the last run ended while inside hatch element n7 (changed, call 8) (+2 nested), session main; if it repeats, launch with EXACT_HATCHES=off"]
    );
    // A name is cut at a character, to the record's 46 bytes.
    let mut crumbs = Breadcrumb::open(&dir, 4).unwrap();
    let slot = crumbs.take("").unwrap();
    crumbs.push(slot, &format!("element {}", "é".repeat(40)), "ended", 1);
    crumbs.close(false);
    let line = &Breadcrumb::read(&dir)[0];
    assert!(
        line.contains(&format!("hatch element {} (ended, call 1)", "é".repeat(19))),
        "{line}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_sessions_slot_is_its_own_and_a_ninth_runs_without_one() {
    let dir = scratch("slots");
    let mut crumbs = Breadcrumb::open(&dir, 9).unwrap();
    let slots: Vec<usize> = (0..SLOTS)
        .map(|i| crumbs.take(&format!("s{i}")).unwrap())
        .collect();
    assert_eq!(crumbs.take("ninth"), None);
    crumbs.push(slots[2], "window", "built", 1);
    crumbs.push(slots[5], "element dot", "ended", 1);
    // One session ends cleanly: its slot is cleared, and no other's.
    crumbs.free(slots[5]);
    assert_eq!(crumbs.take("again"), Some(slots[5]));
    crumbs.close(false);
    assert_eq!(
        Breadcrumb::read(&dir),
        ["the last run ended while inside hatch window (built, call 1), session s2; if it repeats, launch with EXACT_HATCHES=off"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_torn_record_is_passed_over_for_the_last_whole_one() {
    let dir = scratch("torn");
    let mut crumbs = Breadcrumb::open(&dir, 5).unwrap();
    let slot = crumbs.take("").unwrap();
    crumbs.push(slot, "element feed", "built", 1);
    crumbs.push(slot, "element feed", "changed", 1);
    let path = crumbs.path().to_owned();
    crumbs.close(false);
    // The death came inside the second push: its record is half written
    // under a depth that already admits it.
    let mut bytes = std::fs::read(&path).unwrap();
    let second = 32 + 60;
    bytes[second + 20] ^= 0xff;
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        Breadcrumb::read(&dir),
        ["the last run ended while inside hatch element feed (built, call 1); if it repeats, launch with EXACT_HATCHES=off"]
    );
    // A file of another size, or a depth past the stack, says nothing.
    let mut crumbs = Breadcrumb::open(&dir, 6).unwrap();
    let slot = crumbs.take("").unwrap();
    crumbs.push(slot, "app", "built", 1);
    let path = crumbs.path().to_owned();
    crumbs.close(false);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[4] = 200;
    std::fs::write(&path, &bytes).unwrap();
    std::fs::write(dir.join("hatch-breadcrumb-7"), b"short").unwrap();
    assert!(Breadcrumb::read(&dir).is_empty());
    assert!(
        std::fs::read_dir(&dir).unwrap().next().is_none(),
        "both are deleted"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn node(shared: &Rc<Shared>, id: u32) -> Rc<Node> {
    Rc::new(Node {
        id,
        word: "probe".into(),
        html_id: String::new(),
        scope: Rc::from("element probe"),
        shared: shared.clone(),
        live: Cell::new(true),
        new: Cell::new(true),
        frame: Cell::new(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        }),
        data: RefCell::default(),
    })
}

fn lines(shared: &Shared) -> Vec<String> {
    std::mem::take(&mut shared.store().lines)
}

#[test]
fn the_act_queue_is_one_bounded_fifo_drained_a_snapshot_at_a_time() {
    let shared = Rc::new(Shared::new(true));
    let probe = Element {
        node: node(&shared, 7),
    };
    // 256 acts; one past it is refused by name and counted.
    (0..ACTS + 3).for_each(|_| probe.click());
    assert_eq!(shared.in_flight(), ACTS);
    let refused = lines(&shared);
    assert_eq!(refused.len(), 3);
    assert_eq!(
        refused[0],
        "element probe #7: click refused: the act queue is full"
    );
    // A drain runs the acts queued when it starts, at most 64, in order.
    assert_eq!(shared.snapshot().len(), SNAPSHOT);
    probe.focus();
    let next = shared.snapshot();
    assert!(next.len() == SNAPSHOT && next.iter().all(|act| act.kind == ActKind::Click));
    while !shared.snapshot().is_empty() {}
    assert_eq!(shared.in_flight(), 0);
    // An input's text: 64 KB each, 1 MB queued; past either it is refused.
    probe.input(&"x".repeat(INPUT_BYTES + 1));
    assert_eq!(
        lines(&shared),
        ["element probe #7: input refused: 65537 bytes is over 64 KB"]
    );
    let text = "x".repeat(INPUT_BYTES);
    (0..ACT_BYTES / INPUT_BYTES + 1).for_each(|_| probe.input(&text));
    assert_eq!(shared.in_flight(), ACT_BYTES / INPUT_BYTES);
    assert_eq!(
        lines(&shared),
        ["element probe #7: input refused: the act queue is full"]
    );
    // What a drain takes frees its bytes.
    assert_eq!(shared.snapshot().len(), ACT_BYTES / INPUT_BYTES);
    probe.input(&text);
    assert_eq!(shared.in_flight(), 1);
    // An act from an ended handle does nothing, and is journaled.
    probe.node.live.set(false);
    probe.blur();
    assert_eq!(shared.in_flight(), 1);
    assert_eq!(
        lines(&shared),
        ["element probe #7: blur() after its end does nothing"]
    );
}

#[test]
fn a_recording_is_at_most_4096_ops_and_256_kb_and_the_latest_published_wins() {
    use exact_canvas::list::{Op, Writer};
    let shared = Rc::new(Shared::new(true));
    let probe = node(&shared, 3);
    let frame = probe.frame.get();
    let ops = |n: usize| {
        let mut list = Writer::new();
        (0..n).for_each(|_| list.op(Op::Save, &[]));
        list.finish()
    };
    // The bound is on the whole recording, however many lists it seals into.
    assert!(shared.publish(&probe, vec![ops(OVERLAY_OPS)], frame));
    assert!(shared.publish(
        &probe,
        vec![ops(OVERLAY_OPS / 2), ops(OVERLAY_OPS / 2)],
        frame
    ));
    assert!(!shared.publish(
        &probe,
        vec![ops(OVERLAY_OPS / 2), ops(OVERLAY_OPS / 2 + 1)],
        frame
    ));
    assert_eq!(
        lines(&shared),
        ["element probe #3: overlay refused: 4097 ops is over 4,096"]
    );
    let dashes = |n: usize| {
        let mut list = Writer::new();
        (0..n).for_each(|_| list.op(Op::LineDash, &[1.0; 8190]));
        list.finish()
    };
    assert!(dashes(4).len() <= OVERLAY_BYTES && dashes(5).len() > OVERLAY_BYTES);
    assert!(shared.publish(&probe, vec![dashes(4)], frame));
    assert!(!shared.publish(&probe, vec![dashes(5)], frame));
    assert!(lines(&shared)[0].ends_with("bytes is over 256 KB"));
    // Bytes that are not a list are refused whole.
    assert!(!shared.publish(&probe, vec![b"EC2Dgarbage".to_vec()], frame));
    assert!(lines(&shared)[0].starts_with("element probe #3: overlay refused: "));
    // One recording is pending at a time: the latest wins, and a clear is one.
    assert!(shared.publish(&probe, vec![ops(1)], frame));
    assert!(shared.publish(&probe, Vec::new(), frame));
    let taken = shared.take_overlays();
    assert!(taken.len() == 1 && taken[&3].lists.is_empty() && taken[&3].size == (10.0, 10.0));
    assert!(shared.take_overlays().is_empty());
    // A ticket, an `after` and an observer are live until stopped.
    let (ticket, after, observer) = (shared.frames(), shared.after(5.0), shared.observe(&probe));
    assert!(shared.live(ticket) && shared.live(after) && shared.live(observer));
    assert_eq!(shared.observers_on(3), [observer]);
    shared.stop(observer);
    shared.stop(after);
    assert!(shared.live(ticket) && !shared.live(after) && !shared.live(observer));
    assert!(
        shared.due_afters(100.0).is_empty(),
        "a stopped after never fires"
    );
}
