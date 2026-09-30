//! Springs on the web: a `spring()` in a node's `transition` row reaches the
//! page as frames the browser plays (LLP 1002 D2), released from wherever the
//! property is at that moment — the engine's value, the same rule natively.

use exact_motion::{Property, SpringConfig};
use exact_runner::{DataError, DataSource, Event, Value};
use exact_web::batch::Batch;
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot() -> (Host<NoData>, String) {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/spring.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    exact_web::link(exact_web_capabilities::ALL);
    Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap()
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

/// The one `animate` op in a batch: (id, property, delay, duration, values).
fn animate(batch: &str) -> (u32, String, f64, f64, Vec<f64>) {
    assert_eq!(batch.matches("\"op\":\"animate\"").count(), 1, "{batch}");
    let at = batch.find("\"op\":\"animate\"").unwrap();
    let op = &batch[at..];
    let op = &op[..op.find("]}").unwrap() + 2];
    let field = |name: &str| {
        let s = &op[op.find(&format!("\"{name}\":")).unwrap() + name.len() + 3..];
        s[..s.find([',', '}']).unwrap()].to_string()
    };
    let values = &op[op.find("\"values\":[").unwrap() + 10..op.len() - 2];
    let values: Vec<f64> = if values.is_empty() {
        Vec::new()
    } else {
        values.split(',').map(|v| v.parse().unwrap()).collect()
    };
    (
        field("id").parse().unwrap(),
        field("property").trim_matches('"').to_string(),
        field("delay").parse().unwrap(),
        field("duration").parse().unwrap(),
        values,
    )
}

#[test]
fn a_spring_reaches_the_page_as_frames_and_the_easing_beside_it_as_css() {
    let (mut host, first) = boot();
    assert!(!first.contains("animate"), "nothing moves at boot: {first}");
    assert!(
        first.contains("transition:opacity 0.2s linear 0s;"),
        "the easing is CSS, the spring is not: {first}"
    );
    assert!(first.contains("scale:1;"));
    let toggle = view(&host, "toggle");
    let hello = view(&host, "hello");

    let batch = host.dispatch_at(toggle, Event::Press, 0.0);
    assert!(
        batch.contains("scale:1.5;"),
        "the target is the style: {batch}"
    );
    let (id, property, delay, duration, values) = animate(&batch);
    assert_eq!((id, property.as_str(), delay), (hello, "scale", 0.0));
    assert!(duration > 100.0 && duration < 2000.0, "{duration}");
    assert_eq!(values[0], 1.0, "released from the value the page shows");
    assert_eq!(*values.last().unwrap(), 1.5, "the last frame is the target");
    assert_eq!(
        values.len(),
        (duration / 1000.0 * 240.0).round() as usize + 1,
        "one frame per 240 Hz grid point, plus the target"
    );
    let overshoot = values.iter().cloned().fold(f64::MIN, f64::max);
    assert!(
        overshoot > 1.5,
        "an underdamped spring overshoots: {overshoot}"
    );
}

#[test]
fn an_interrupted_spring_is_released_from_where_it_is_with_its_velocity() {
    let (mut host, _) = boot();
    let toggle = view(&host, "toggle");
    let hello = view(&host, "hello");
    host.dispatch_at(toggle, Event::Press, 0.0);
    // 100 ms in, the target goes back. The new spring starts from the
    // engine's presentation value at 100 ms — the closed form sampled once —
    // not from the target and not from the start.
    let batch = host.dispatch_at(toggle, Event::Press, 100.0);
    let (_, _, _, _, values) = animate(&batch);
    let config = SpringConfig {
        stiffness: 180.0,
        damping: 12.0,
        mass: 1.0,
    };
    let expected = 1.5 + config.sample(1.0 - 1.5, 0.0, 0.1).displacement;
    assert_eq!(
        values[0].to_bits(),
        expected.to_bits(),
        "{} vs {expected}",
        values[0]
    );
    assert!(values[0] > 1.0 && values[0] < 1.5);
    assert_eq!(*values.last().unwrap(), 1.0);
    let key = host.runner().kernel().node(hello).unwrap().key;
    let node = exact_kernel::motion::motion_node(key);
    assert_eq!(
        host.springs()
            .engine()
            .value(node, Property::Scale)
            .unwrap()
            .x
            .to_bits(),
        expected.to_bits(),
        "the page and the engine agree on where the property is"
    );
    // The velocity carried through: the first step continues toward 1.5
    // before turning back (the spring was still rising at 100 ms).
    assert!(values[1] > values[0], "{} then {}", values[0], values[1]);
}

#[test]
fn the_clock_alone_says_nothing_and_a_finished_spring_is_silent() {
    let (mut host, _) = boot();
    let toggle = view(&host, "toggle");
    host.dispatch_at(toggle, Event::Press, 0.0);
    let quiet = host.advance(50.0);
    assert!(
        !quiet.contains("animate"),
        "a running spring is not restated: {quiet}"
    );
    let done = host.advance(20_000.0);
    assert!(
        !done.contains("animate"),
        "a finished spring is not cancelled: {done}"
    );
    // And a new release after rest starts from the target it reached.
    let batch = host.dispatch_at(toggle, Event::Press, 20_000.0);
    let (_, _, _, _, values) = animate(&batch);
    assert_eq!(values[0], 1.5);
    assert_eq!(*values.last().unwrap(), 1.0);
}

#[test]
fn translate_frames_are_pairs() {
    let mut b = Batch::new();
    b.animate(
        7,
        "translate",
        0.0,
        250.0,
        &[(0.0, 0.0), (10.5, -2.0)],
        true,
    );
    b.animate(7, "opacity", 0.0, 0.0, &[], false);
    let s = b.finish(None, false, 0.0, None);
    assert!(s.contains(
        "{\"op\":\"animate\",\"id\":7,\"property\":\"translate\",\"delay\":0,\"duration\":250,\"values\":[[0,0],[10.5,-2]]}"
    ), "{s}");
    assert!(
        s.contains("\"property\":\"opacity\",\"delay\":0,\"duration\":0,\"values\":[]}"),
        "{s}"
    );
}

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..1_000).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}

#[test]
fn destroying_animated_virtual_rows_releases_bookkeeping_before_remount_or_reset() {
    let source = r#"component App
  resource rows = rows() as shape list<number>
  state shown = true
  state big = false
  action hide writes shown
    shown = not shown
  action toggle writes big
    big = not big
  view
    column
      button press=hide testId="hide"
        text "Hide"
      button press=toggle testId="toggle"
        text "Toggle"
      text "survivor" testId="survivor" scale=(big ? 1.5 : 1) transition="scale spring(180, 12, 1)"
      when shown
        list virtualized=true height=180
          each x in rows key=x
            column height=32
              text `${x}` scale=(big ? 1.5 : 1) opacity=(big ? 0.5 : 1) transition="scale spring(180, 12, 1), opacity spring(180, 12, 1)"
"#;
    let plan = contract::compile(source).unwrap().encode();
    exact_web::link(exact_web_capabilities::ALL);
    let (mut host, _) = Host::boot(&plan, Rows, Default::default(), "/").unwrap();
    let id = |host: &Host<Rows>, name| {
        let k = host.runner().kernel();
        k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
    };
    let hide = id(&host, "hide");
    let toggle = id(&host, "toggle");
    let mut previous = None;
    for cycle in 0..16 {
        let now = (cycle + 1) as f64 * 50.0;
        let snapshot = host.runner().collections().remove(0);
        let root = host
            .runner()
            .kernel()
            .node(snapshot.rows[0].root)
            .unwrap()
            .children()[0];
        let key = host.runner().kernel().node(root).unwrap().key;
        assert_ne!(Some(key), previous, "a remount has a new motion identity");
        let batch = host.dispatch_at(toggle, Event::Press, now);
        assert!(batch.contains("\"op\":\"animate\""));
        assert_eq!(host.springs().playing_count(), 2 * snapshot.rows.len() + 1);
        host.dispatch_at(hide, Event::Press, now + 1.0);
        assert!(host.runner().collections().is_empty());
        assert_eq!(
            host.springs().playing_count(),
            1,
            "only the survivor remains"
        );
        let node = exact_kernel::motion::motion_node(key);
        assert!(host
            .springs()
            .engine()
            .value(node, Property::Scale)
            .is_none());
        host.dispatch_at(hide, Event::Press, now + 2.0);
        assert_eq!(
            host.springs().playing_count(),
            1,
            "remount adopts its current style without animation"
        );
        previous = Some(key);
    }
    // Reload replaces the Host, including its engine and ownership map.
    exact_web::link(exact_web_capabilities::ALL);
    let (replacement, first) = Host::boot(&plan, Rows, Default::default(), "/").unwrap();
    assert_eq!(replacement.springs().playing_count(), 0);
    assert!(!first.contains("\"op\":\"animate\""));
}
