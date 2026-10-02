use super::*;
use exact_runner::CollectionSnapshot;

struct GrowthRows;
impl DataSource for GrowthRows {
    fn query(&mut self, name: &str, args: &[Value]) -> Result<Value, DataError> {
        assert_eq!(name, "growthRows");
        let [Value::Number(count)] = args else {
            panic!("count argument required: {args:?}");
        };
        assert!(*count == 20. || *count == 200.);
        Ok(Value::list(
            (0..*count as usize)
                .map(|n| Value::Number(n as f64))
                .collect(),
        ))
    }
}

const SOURCE: &str = r#"component App
  state count = 20
  action grow
    count = 200
  resource rows = growthRows(count) as shape list<number>
  view
    column width=320 height=240
      box testId="grow" press=grow height=24
      list virtualized=true scrollFollowEnd=true width=320 height=180 testId="port"
        each x in rows key=x
          text `${x}` height=24
"#;

#[derive(Debug)]
struct Boundary {
    key: exact_kernel::NodeKey,
    snapshot: CollectionSnapshot,
    offset: f32,
    port_height: f32,
    live_max: f32,
    acknowledged_max: Option<f32>,
    acknowledged_clamp_of_correction: Option<f32>,
}

fn boundary(p: &Presenter<GrowthRows>, label: &str) -> Boundary {
    let snapshots = p.host.collections();
    assert_eq!(snapshots.len(), 1);
    let snapshot = snapshots.into_iter().next().unwrap();
    let node = p.host.kernel().node(snapshot.view).unwrap();
    let acknowledged = p.display.bounds(p.host.kernel(), snapshot.view);
    let out = Boundary {
        key: node.key,
        offset: p.scroll_of(snapshot.view).1,
        port_height: node.frame.height,
        live_max: p.collection_scroll_limits()[&snapshot.view],
        acknowledged_max: acknowledged.map(|b| b.max.1),
        acknowledged_clamp_of_correction: acknowledged.and_then(|b| {
            snapshot
                .correction
                .map(|c| b.clamp((0., c.offset as f32)).1)
        }),
        snapshot,
    };
    eprintln!("{label}: {out:?}");
    out
}

fn paint(p: &mut Presenter<GrowthRows>, displayed: bool) {
    if displayed {
        let frame = submit(p).expect("no earlier submission remains pending");
        assert!(complete(p, &frame));
    } else {
        p.frame();
        assert!(p.last_frame_succeeded);
    }
}

fn settle(p: &mut Presenter<GrowthRows>, displayed: bool) {
    for _ in 0..32 {
        assert!(p.pump(0.).is_none());
        if p.dirty() {
            paint(p, displayed);
        }
        if !p.dirty() && !p.collection.pending() {
            return;
        }
    }
    panic!(
        "bounded fixture did not settle: {:?}",
        boundary(p, "unsettled")
    );
}

fn wheel(p: &mut Presenter<GrowthRows>, delta: f32) {
    let port = p.host.collections()[0].view;
    p.wheel(port, 0., delta).unwrap();
}

fn at_end(b: &Boundary) -> bool {
    ((b.snapshot.total_extent - b.port_height as f64).max(0.) - b.offset as f64).abs() <= 0.5
}

fn boot_end(displayed: bool, follow: bool) -> (Presenter<GrowthRows>, Boundary) {
    boot_end_from(displayed, follow, SOURCE)
}

fn boot_end_from(
    displayed: bool,
    follow: bool,
    template: &str,
) -> (Presenter<GrowthRows>, Boundary) {
    let source = template.replace("scrollFollowEnd=true", &format!("scrollFollowEnd={follow}"));
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        GrowthRows,
        (320., 240.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // Attach before the first feedback cycle, exactly as display_frame does.
    paint(&mut p, displayed);
    settle(&mut p, displayed);
    for _ in 0..16 {
        wheel(&mut p, 1_000_000.);
        settle(&mut p, displayed);
        let end = boundary(&p, "A-end-challenge");
        if at_end(&end) {
            assert_eq!(end.snapshot.count, 20);
            assert_eq!(end.port_height, 180.);
            assert!(end.snapshot.rows.iter().all(|r| r.measured));
            if displayed {
                assert!((end.acknowledged_max.unwrap() - end.offset).abs() <= 0.5);
            }
            return (p, end);
        }
    }
    panic!("actual wheel/feedback never reached A index end");
}

fn grow_before_adapter(p: &mut Presenter<GrowthRows>, before: &Boundary) -> Boundary {
    let kernel = p.host.kernel();
    let grow = kernel
        .node_by_key(kernel.find_by_test_id("grow")[0])
        .unwrap()
        .id;
    // Existing viewport_action helper uses this same normal action boundary.
    assert!(p
        .host
        .dispatch_at(grow, Event::Press, p.host.now())
        .is_none());
    let issued = boundary(p, "actual-Runner-after-grow-before-Presenter");
    assert_eq!(issued.key, before.key, "same live List incarnation");
    assert_eq!(issued.snapshot.view, before.snapshot.view);
    assert_eq!(issued.snapshot.count, 200);
    assert!(issued.snapshot.revision > before.snapshot.revision);
    assert_eq!(
        issued.snapshot.scroll_sequence,
        before.snapshot.scroll_sequence
    );
    assert!(issued.live_max > before.live_max + 1000.);
    issued
}

fn finish_growth(p: &mut Presenter<GrowthRows>, displayed: bool) -> Boundary {
    // Actual adapter applies feedback and calls sync_commit/clamp_scroll.
    assert!(p.after_commit().is_none());
    boundary(p, "after-production-feedback-and-clamp");
    if displayed {
        let b = submit(p).unwrap();
        boundary(p, "B-submitted-A-still-acknowledged");
        assert!(complete(p, &b));
        boundary(p, "after-B-ACK");
    }
    settle(p, displayed);
    boundary(p, "settled-current-B-no-new-input")
}

fn follows_growth(displayed: bool) {
    let (mut p, before) = boot_end(displayed, true);
    let issued = grow_before_adapter(&mut p, &before);
    let correction = issued
        .snapshot
        .correction
        .expect("Runner must emit real end correction");
    assert_eq!(correction.scroll_sequence, before.snapshot.scroll_sequence);
    assert!((correction.offset - (issued.snapshot.total_extent - 180.)).abs() <= 0.5);
    assert!(correction.offset > before.offset as f64 + 1000.);
    if displayed {
        assert_eq!(issued.acknowledged_max, before.acknowledged_max);
        assert!((issued.acknowledged_clamp_of_correction.unwrap() - before.offset).abs() <= 0.5);
    }
    let after = finish_growth(&mut p, displayed);
    assert_eq!(after.key, before.key);
    assert!(
        at_end(&after),
        "emitted {correction:?}; current B lost end-follow: {after:?}"
    );
    assert!(after.offset > before.offset + 1000.);
}

#[test]
fn acknowledged_a_end_survives_same_list_growth_to_b() {
    follows_growth(true);
}

#[test]
fn headless_control_preserves_real_runner_end_correction() {
    follows_growth(false);
}

#[test]
fn away_reader_is_not_pulled_to_new_end() {
    for displayed in [false, true] {
        let (mut p, _) = boot_end(displayed, true);
        wheel(&mut p, -120.);
        settle(&mut p, displayed);
        let before = boundary(&p, "away-before-grow");
        assert!(!at_end(&before));
        let issued = grow_before_adapter(&mut p, &before);
        assert!(issued
            .snapshot
            .correction
            .is_none_or(|c| { (c.offset - before.offset as f64).abs() <= 0.5 }));
        let after = finish_growth(&mut p, displayed);
        assert!((after.offset - before.offset).abs() <= 0.5);
        assert!(!at_end(&after));
    }
}

#[test]
fn disabled_follow_keeps_old_end_offset() {
    for displayed in [false, true] {
        let (mut p, before) = boot_end(displayed, false);
        let issued = grow_before_adapter(&mut p, &before);
        assert!(issued
            .snapshot
            .correction
            .is_none_or(|c| { (c.offset - before.offset as f64).abs() <= 0.5 }));
        let after = finish_growth(&mut p, displayed);
        assert!((after.offset - before.offset).abs() <= 0.5);
        assert!(!at_end(&after));
    }
}

#[test]
fn newer_real_scroll_wins_over_issued_end_correction() {
    let (mut p, before) = boot_end(true, true);
    let issued = grow_before_adapter(&mut p, &before);
    let correction = issued.snapshot.correction.unwrap();
    assert!(correction.offset > before.offset as f64 + 1000.);
    wheel(&mut p, -40.);
    let moved = boundary(&p, "newer-wheel-before-old-correction-can-win");
    assert!(moved.snapshot.scroll_sequence > correction.scroll_sequence);
    assert!((moved.offset - (before.offset - 40.)).abs() <= 0.5);
    let after = finish_growth(&mut p, true);
    assert!((after.offset - moved.offset).abs() <= 0.5);
    assert!(!at_end(&after));
}

#[test]
fn future_b_paint_does_not_become_a_wheel_base_or_override_newer_input() {
    let (mut p, before) = boot_end(true, true);
    grow_before_adapter(&mut p, &before);
    assert!(p.after_commit().is_none());
    let b = submit(&mut p).unwrap();
    let b_top = b
        .picture
        .borrow()
        .as_ref()
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == before.snapshot.view)
        .unwrap()
        .scroll
        .unwrap()
        .1;
    assert!(
        b_top > before.offset + 1000.,
        "B must paint the real future correction"
    );
    let a_top = p
        .boxes
        .iter()
        .find(|b| b.id == before.snapshot.view)
        .unwrap()
        .scroll
        .unwrap()
        .1;
    assert_eq!(
        a_top, before.offset,
        "A is still the acknowledged painted scroll"
    );
    assert_eq!(p.scroll_of(before.snapshot.view).1, a_top);
    wheel(&mut p, -40.);
    assert_eq!(p.scroll_of(before.snapshot.view).1, a_top - 40.);
    assert!(complete(&mut p, &b));
    assert_eq!(
        p.scroll_of(before.snapshot.view).1,
        a_top - 40.,
        "old B cannot revive its correction"
    );
    settle(&mut p, true);
    let after = boundary(&p, "newer-A-wheel-survives-B-ACK");
    assert_eq!(after.offset, a_top - 40.);
    assert!(!at_end(&after));
}

#[test]
fn replacement_runtime_cannot_adopt_pending_old_model_scroll() {
    let (mut p, before) = boot_end(true, true);
    grow_before_adapter(&mut p, &before);
    assert!(p.after_commit().is_none());
    let b = submit(&mut p).unwrap();
    p.reload(&contract::compile(SOURCE).unwrap().encode(), GrowthRows)
        .unwrap();
    let replacement = boundary(&p, "replacement-before-old-B-ACK");
    assert_eq!(
        replacement.snapshot.count, 200,
        "reload carries compatible state"
    );
    assert_eq!(
        replacement.offset, 0.,
        "replacement owns a fresh scroll offset"
    );
    assert!(complete(&mut p, &b));
    assert_eq!(p.scroll_of(replacement.snapshot.view).1, replacement.offset);
    settle(&mut p, true);
    let after = boundary(&p, "replacement-after-new-ACK");
    assert_eq!(after.snapshot.count, 200);
    assert_eq!(
        after.offset, replacement.offset,
        "old B cannot restore its pending tail"
    );
}

#[test]
fn older_b_ack_preserves_newer_model_target_on_same_input_sequence() {
    let source = SOURCE.replace("count = 200", "count = (count == 20 ? 200 : 20)");
    let (mut p, before) = boot_end_from(true, true, &source);
    grow_before_adapter(&mut p, &before);
    assert!(p.after_commit().is_none());
    let b = submit(&mut p).unwrap();
    let b_top = b
        .picture
        .borrow()
        .as_ref()
        .unwrap()
        .boxes
        .iter()
        .find(|b| b.id == before.snapshot.view)
        .unwrap()
        .scroll
        .unwrap()
        .1;
    assert!(b_top > before.offset + 1000.);
    let grow = p
        .host
        .kernel()
        .node_by_key(p.host.kernel().find_by_test_id("grow")[0])
        .unwrap()
        .id;
    assert!(p
        .host
        .dispatch_at(grow, Event::Press, p.host.now())
        .is_none());
    let issued = boundary(&p, "C-shrink-before-B-ACK");
    assert_eq!(issued.snapshot.count, 20);
    assert_eq!(
        issued.snapshot.scroll_sequence,
        before.snapshot.scroll_sequence
    );
    assert_eq!(issued.snapshot.correction.unwrap().offset, 300.);
    assert!(p.after_commit().is_none());
    assert!(complete(&mut p, &b));
    assert_eq!(
        p.scroll_of(before.snapshot.view).1,
        b_top,
        "B ACK owns the interaction base"
    );
    settle(&mut p, true);
    let after = boundary(&p, "C-model-survives-older-B-ACK");
    assert_eq!(after.snapshot.count, 20);
    assert_eq!(after.offset, 300.);
    assert!(at_end(&after));
}

#[test]
fn acknowledged_a_wheel_stays_60_to_90_while_b_is_pending() {
    let ((mut p, _, _), _) = acknowledged_root_page();
    assert_eq!(p.page.1, 60.);
    assert!(p.resize(900., 900.).is_none());
    let b = submit(&mut p).unwrap();
    p.wheel_at(300., 200., 0., 30.);
    assert_eq!(
        p.page.1, 90.,
        "live B cannot replace A input bounds before ACK"
    );
    assert!(complete(&mut p, &b));
    assert_eq!(p.page.1, 0.);
}

#[test]
fn same_size_resize_before_b_ack_preserves_future_end() {
    let (mut p, before) = boot_end(true, true);
    grow_before_adapter(&mut p, &before);
    assert!(p.after_commit().is_none());
    let b = submit(&mut p).unwrap();
    let pending = boundary(&p, "future-B-before-identical-resize");
    assert!(p.resize(320., 240.).is_none());
    let resized = boundary(&p, "after-identical-resize-before-B-ACK");
    assert!(complete(&mut p, &b));
    settle(&mut p, true);
    let after = boundary(&p, "same-size-resize-settled-after-B-ACK");
    assert!(
        at_end(&after),
        "identical viewport lost end intent: {after:?}"
    );
    assert!(after.offset > before.offset + 1000.);
    assert_eq!(
        resized.snapshot.scroll_sequence,
        pending.snapshot.scroll_sequence
    );
}

#[test]
fn changed_size_resize_before_b_ack_supersedes_future_end() {
    let (mut p, before) = boot_end(true, true);
    grow_before_adapter(&mut p, &before);
    assert!(p.after_commit().is_none());
    let b = submit(&mut p).unwrap();
    let pending = boundary(&p, "future-B-before-changed-resize");
    assert!(p.resize(321., 240.).is_none());
    let resized = boundary(&p, "after-changed-resize-before-B-ACK");
    assert!(resized.snapshot.scroll_sequence > pending.snapshot.scroll_sequence);
    assert_eq!(resized.offset, before.offset);
    assert!(complete(&mut p, &b));
    settle(&mut p, true);
    let after = boundary(&p, "changed-size-resize-settled-after-B-ACK");
    assert_eq!(
        after.offset, before.offset,
        "old B must not restore its target"
    );
    assert!(!at_end(&after));
}
