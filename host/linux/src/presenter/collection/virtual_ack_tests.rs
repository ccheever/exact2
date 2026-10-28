//! A virtualized list's own positions under a reader's fling, through the collection's feedback and correction
//! path: rows prepended while the reader moves between paint and acknowledgement, the end
//! followed with and without a `scrollend` listener, and an app's `scrollLeft` refined by measurements.
use super::*;
use exact_runner::{DataError, Value};

/// `rows(first, count)`: `count` consecutive numbers from `first`, as an older page grows a chat's window.
struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        let [Value::Number(first), Value::Number(count)] = args else {
            return Ok(Value::list(
                (0..25_000).map(|n| Value::Number(n as f64)).collect(),
            ));
        };
        Ok(Value::list(
            (0..*count as u32)
                .map(|n| Value::Number(first + f64::from(n)))
                .collect(),
        ))
    }
}

fn boot(source: &str) -> Presenter<Rows> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(source),
        Rows,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    settle(&mut p);
    p
}

/// Commits, collection passes and frames painted and acknowledged until quiet, as a host with a display does.
fn settle(p: &mut Presenter<Rows>) {
    for _ in 0..16 {
        assert!(p.pump(p.host.now()).is_none());
        if let Some(frame) = p.display_frame() {
            assert!(p.display_complete(&frame));
        }
    }
}

fn named(p: &Presenter<Rows>, name: &str) -> ViewId {
    let kernel = p.host.kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

fn press(p: &mut Presenter<Rows>, name: &str) {
    let id = named(p, name);
    assert_eq!(p.host.dispatch_at(id, Event::Press, p.host.now()), None);
    assert_eq!(p.after_commit(), None);
}

fn revision(p: &mut Presenter<Rows>, port: ViewId) -> u32 {
    let words = p.reader_scrollers(&[], 1.).unwrap();
    let at = words
        .chunks(9)
        .position(|w| w[0] as ViewId == port)
        .unwrap();
    words[at * 9 + 2] as u32
}

fn held(p: &mut Presenter<Rows>, port: ViewId) -> u32 {
    let b = p.boxes().iter().find(|b| b.id == port).unwrap().rect;
    p.reader_under(b.0 + b.2 / 2., b.1 + b.3 / 2., 1.)[5] as u32
}

fn row_y(p: &mut Presenter<Rows>, name: &str) -> f32 {
    let row = named(p, name);
    p.boxes().iter().find(|b| b.id == row).unwrap().rect.1
}

#[test]
fn fifty_rows_prepended_to_a_virtualized_list_between_paint_and_ack_keep_the_fling() {
    let source = r#"component App
  state first = 1000
  state count = 200
  action prepend
    first = 950
    count = 250
  resource rows = rows(first, count) as shape number[]
  view
    column width="100%"
      button "Prepend" press=prepend testId="prepend"
      list virtualized=true estimated-item-height=20 width=400 height=300 testId="port"
        for x in rows key=x
          box testId=`r${x}` width=400 height=24
"#;
    let mut p = boot(source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 2400.)).unwrap();
    settle(&mut p);
    let rev = held(&mut p, port);
    assert_eq!(p.reader_scrolled(port, (0., -30.), rev), Some((0., -30.)));
    settle(&mut p);
    // A row in the middle of the port, whichever is there.
    let b = p.boxes().iter().find(|b| b.id == port).unwrap().rect;
    let middle = b.1 + b.3 / 2.;
    let under: Vec<ViewId> = p
        .boxes()
        .iter()
        .filter(|r| r.rect.1 <= middle && r.rect.1 + r.rect.3 > middle)
        .map(|r| r.id)
        .collect();
    let kernel = p.host.kernel();
    let name = under
        .iter()
        .find_map(|id| {
            let test_id = kernel.node(*id)?.props.str(PropId::TestId)?;
            (test_id.starts_with('r') && test_id != "port").then(|| test_id.to_owned())
        })
        .unwrap();
    let shown = row_y(&mut p, &name);
    press(&mut p, "prepend");
    assert!(p.pump(p.host.now()).is_none());
    let painted = p.display_frame().unwrap();
    // The reader moves on between the paint and its acknowledgement.
    assert!(p.reader_scrolled(port, (0., -30.), rev).is_some());
    assert!(p.display_complete(&painted));
    settle(&mut p);
    assert_eq!(
        row_y(&mut p, &name),
        shown + 30.,
        "{name} moved only as the reader moved it"
    );
    assert_eq!(
        revision(&mut p, port),
        rev,
        "rows prepended are no app scroll"
    );
    assert!(
        p.reader_scrolled(port, (0., -30.), rev).is_some(),
        "the fling's next move is taken"
    );
}

fn follow_source(listener: bool) -> String {
    format!(
        r#"component App
  state count = 40
  state ends = 0
  action grow
    count = count + 5
  action shrink
    count = count - 10
  action ended
    ends = ends + 1
  resource rows = rows(0, count) as shape number[]
  view
    column width="100%"
      button "Grow" press=grow testId="grow"
      button "Shrink" press=shrink testId="shrink"
      list virtualized=true scrollFollowEnd=true estimated-item-height=20 width=400 height=300 testId="port"{}
        for x in rows key=x
          box testId=`r${{x}}` width=400 height=20
"#,
        if listener { " scrollend=ended" } else { "" }
    )
}

#[test]
fn the_end_followed_moves_the_revision_on_with_or_without_a_scrollend_listener() {
    // The app's end follow (scrollFollowEnd) is the app's position whether or not a `scrollend` handler tracks
    // the end: adding a listener changes no revision.
    let mut outcome = Vec::new();
    for listener in [false, true] {
        let mut p = boot(&follow_source(listener));
        let port = named(&p, "port");
        p.scroll_to(port, (0., 1e6)).unwrap();
        settle(&mut p);
        let mut rev = held(&mut p, port);
        // Appends in a row, one before the last one's acknowledgement too.
        for (n, settled) in [(1, true), (2, false), (3, true)] {
            press(&mut p, "grow");
            if !settled {
                assert!(p.pump(p.host.now()).is_none());
                press(&mut p, "grow");
            }
            settle(&mut p);
            let max = p.collection_scroll_limits()[&port];
            assert_eq!(
                p.scroll_of(port).1,
                max,
                "listener={listener} append {n}: followed"
            );
            let now = revision(&mut p, port);
            outcome.push(now != rev);
            rev = now;
        }
    }
    assert!(
        outcome.iter().all(|b| *b),
        "every end followed moves it on: {outcome:?}"
    );
}

#[test]
fn the_end_followed_on_a_resize_pass_moves_the_revision_on() {
    // A port that shrinks (a composer growing under a chat) keeps a follow-end list at its end on the pass
    // that resizes it: still the app's position.
    let source = follow_source(false)
        .replace(
            "  state count = 40\n",
            "  state count = 40\n  state high = 300\n",
        )
        .replace(
            "  action grow\n",
            "  action squeeze\n    high = 240\n  action grow\n",
        )
        .replace(
            "      button \"Grow\"",
            "      button \"Squeeze\" press=squeeze testId=\"squeeze\"\n      button \"Grow\"",
        )
        .replace(
            "width=400 height=300 testId=\"port\"",
            "width=400 height=high testId=\"port\"",
        );
    let mut p = boot(&source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 1e6)).unwrap();
    settle(&mut p);
    let rev = held(&mut p, port);
    press(&mut p, "squeeze");
    settle(&mut p);
    let max = p.collection_scroll_limits()[&port];
    assert_eq!(
        p.scroll_of(port).1,
        max,
        "the end is followed as the port shrinks"
    );
    assert_ne!(revision(&mut p, port), rev);
}

#[test]
fn a_measurement_clamped_to_the_range_away_from_the_end_keeps_the_revision() {
    // Rows estimated at 40 and measured at 20 shrink the range under a reader who did not stand at the end: the
    // list's own position, from no row too, keeps the revision.
    let source = follow_source(false)
        .replace("  state count = 40\n", "  state count = 100\n")
        .replace("estimated-item-height=20", "estimated-item-height=40");
    let mut p = boot(&source);
    let port = named(&p, "port");
    let estimated_end = p.collection_scroll_limits()[&port];
    p.scroll_to(port, (0., estimated_end - 100.)).unwrap();
    let rev = held(&mut p, port);
    settle(&mut p);
    assert_eq!(
        revision(&mut p, port),
        rev,
        "the list's own measurements are no app scroll"
    );
}

#[test]
fn an_apps_scroll_left_refined_by_measurements_still_ends_the_fling() {
    // Cards estimated at 100 and measured at 94: the request's target is refined by measurement corrections
    // before it is painted, and stays the app's.
    let source = r#"component App
  state jump = 0
  action far
    jump = jump + 50000
  resource rows = rows() as shape number[]
  view
    column width="100%"
      button press=far testId="far"
        text "Far"
      list virtualized=true estimated-item-width=100 scrollLeft=jump display="flex" width=400 height=120 overflow-x="scroll" overflow-y="hidden" testId="strip"
        for x in rows key=x
          box testId=`c-${x}` width=90 height=80 margin-right=4
"#;
    let mut p = boot(source);
    let strip = named(&p, "strip");
    let rev = held(&mut p, strip);
    press(&mut p, "far");
    settle(&mut p);
    assert!(
        p.scroll_of(strip).0 > 40_000.,
        "the request landed: {:?}",
        p.scroll_of(strip)
    );
    assert_ne!(
        revision(&mut p, strip),
        rev,
        "an app's position, refined, still moves the revision on"
    );
}

#[test]
fn content_shrinking_under_a_list_at_its_end_is_the_lists_own() {
    // Parked at the end, rows removed: the clamp to the new end is the list's own.
    let mut p = boot(&follow_source(false));
    let port = named(&p, "port");
    p.scroll_to(port, (0., 1e6)).unwrap();
    settle(&mut p);
    let rev = held(&mut p, port);
    press(&mut p, "shrink");
    settle(&mut p);
    assert_eq!(p.scroll_of(port).1, p.collection_scroll_limits()[&port]);
    assert_eq!(
        revision(&mut p, port),
        rev,
        "a shrink's clamp is no app scroll"
    );
}

#[test]
fn a_fling_that_left_the_end_then_clamped_is_the_lists_own() {
    // The reader moved away from the end, then rows were removed under it so its offset clamps to the new end:
    // the list's own; the next sample is taken.
    let mut p = boot(&follow_source(false));
    let port = named(&p, "port");
    p.scroll_to(port, (0., 1e6)).unwrap();
    settle(&mut p);
    let rev = held(&mut p, port);
    assert!(p.reader_scrolled(port, (0., -100.), rev).is_some());
    settle(&mut p);
    press(&mut p, "shrink");
    press(&mut p, "shrink");
    settle(&mut p);
    assert_eq!(
        revision(&mut p, port),
        rev,
        "a clamp after leaving the end is no app scroll"
    );
    assert!(p.reader_scrolled(port, (0., -20.), rev).is_some());
}

#[test]
fn an_absolute_scroll_supersedes_a_pending_prepend() {
    // A request to 0 while a 50-row prepend's correction is pending: the request wins and the prepend's +1,000
    // is not applied over it.
    let source = r#"component App
  state first = 1000
  state count = 200
  action prepend
    first = 950
    count = 250
  resource rows = rows(first, count) as shape number[]
  view
    column width="100%"
      button "Prepend" press=prepend testId="prepend"
      list virtualized=true estimated-item-height=20 width=400 height=300 testId="port"
        for x in rows key=x
          box testId=`r${x}` width=400 height=24
"#;
    let mut p = boot(source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 2400.)).unwrap();
    settle(&mut p);
    press(&mut p, "prepend");
    assert!(p.pump(p.host.now()).is_none());
    assert_eq!(p.scroll_to(port, (0., 0.)), Some((0., 0.)));
    settle(&mut p);
    assert_eq!(p.scroll_of(port), (0., 0.));
}

#[test]
fn a_second_append_before_the_first_follow_is_painted_is_still_followed_as_the_apps() {
    // The first append's end follow is planned but not yet painted, and a settled pass raises the travel last
    // seen; the second append still finds the list standing at its end, by its pending position, not by the
    // unacknowledged offset. With and without a `scrollend` listener.
    for listener in [false, true] {
        let mut p = boot(&follow_source(listener));
        let port = named(&p, "port");
        p.scroll_to(port, (0., 1e6)).unwrap();
        settle(&mut p);
        press(&mut p, "grow");
        for _ in 0..4 {
            assert!(p.pump(p.host.now()).is_none());
        }
        let pending = p.collection.cursors[&port].model_scroll;
        assert_eq!(
            pending.map(|m| m.origin),
            Some(Origin::App),
            "listener={listener}: the first follow waits"
        );
        press(&mut p, "grow");
        for _ in 0..4 {
            assert!(p.pump(p.host.now()).is_none());
        }
        let pending = p.collection.cursors[&port].model_scroll.unwrap();
        assert_eq!(
            pending.origin,
            Origin::App,
            "listener={listener}: the second follow is the app's"
        );
        assert_eq!(
            pending.top,
            p.collection_scroll_limits()[&port],
            "listener={listener}: to the new end"
        );
    }
}

#[test]
fn a_clamp_under_a_carried_anchor_advances_its_base() {
    // A list at 1600 (travel 1700) loses 200 px of rows above it, so
    // the anchor's target is 1400 (travel 1500); that picture is submitted, a -10 sample carries the anchor
    // (target 1390, base 1590), the old picture's acknowledgement clamps the input offset to 1500, and another
    // -10 sample must land at 1380: the clamp already took 90 of the anchor's 200, not 1290.
    let source = r#"component App
  state first = 0
  state count = 100
  action remove
    first = 10
    count = 90
  resource rows = rows(first, count) as shape number[]
  view
    column width="100%"
      button "Remove" press=remove testId="remove"
      list virtualized=true estimated-item-height=20 width=400 height=300 testId="port"
        for x in rows key=x
          box testId=`r${x}` width=400 height=20
"#;
    let mut p = boot(source);
    let port = named(&p, "port");
    assert_eq!(p.collection_scroll_limits()[&port], 1700.);
    p.scroll_to(port, (0., 1600.)).unwrap();
    settle(&mut p);
    let rev = held(&mut p, port);
    press(&mut p, "remove");
    assert!(p.pump(p.host.now()).is_none());
    let painted = p.display_frame().unwrap();
    assert_eq!(p.reader_scrolled(port, (0., -10.), rev), Some((0., -10.)));
    assert!(p.display_complete(&painted));
    assert!(p.reader_scrolled(port, (0., -10.), rev).is_some());
    settle(&mut p);
    assert_eq!(p.scroll_of(port), (0., 1380.));
}

#[test]
fn a_wheel_during_a_finger_fling_ends_it_and_lands_where_the_wheel_put_it() {
    // theme2 10:25 (B): a wheel is a new input, as an app scroll is: it retires the list's own pending position
    // (no carry) and moves the reader's revision on, so a finger's glide ends and the list stays where the wheel
    // put it.
    let source = r#"component App
  state first = 1000
  state count = 200
  action prepend
    first = 950
    count = 250
  resource rows = rows(first, count) as shape number[]
  view
    column width="100%"
      button "Prepend" press=prepend testId="prepend"
      list virtualized=true estimated-item-height=20 width=400 height=300 testId="port"
        for x in rows key=x
          box testId=`r${x}` width=400 height=20
"#;
    let mut p = boot(source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 2400.)).unwrap();
    settle(&mut p);
    let rev = held(&mut p, port);
    assert!(p.reader_scrolled(port, (0., -30.), rev).is_some());
    settle(&mut p);
    press(&mut p, "prepend");
    assert!(p.pump(p.host.now()).is_none());
    let before = p.scroll_of(port);
    p.wheel(port, 0., -50.).unwrap();
    assert_eq!(
        p.scroll_of(port),
        (0., before.1 - 50.),
        "where the wheel put it"
    );
    settle(&mut p);
    assert_eq!(
        p.scroll_of(port),
        (0., before.1 - 50.),
        "and stays there: nothing carried over the wheel"
    );
    assert_ne!(revision(&mut p, port), rev, "the finger's glide ends");
    assert_eq!(
        p.reader_scrolled(port, (0., -10.), rev),
        None,
        "its next move is dropped"
    );
}

#[test]
fn a_wheel_on_deferred_collection_turns_rests_where_the_wheel_put_it() {
    // theme2 11:06 (X; Astra MAJOR, Grok BLOCKER): Android's loop defers the collection turn until after the
    // frame (pump, wheel, display_frame, display_complete, run_deferred_collections). A 50-row prepend is pending
    // (3370) when a wheel moves 2370 to 2320: the frame's acknowledgement must not write the prepend over the
    // wheel, so the list rests at 2320.
    let source = r#"component App
  state first = 1000
  state count = 200
  action prepend
    first = 950
    count = 250
  resource rows = rows(first, count) as shape number[]
  view
    column width="100%"
      button "Prepend" press=prepend testId="prepend"
      list virtualized=true estimated-item-height=20 width=400 height=300 testId="port"
        for x in rows key=x
          box testId=`r${x}` width=400 height=20
"#;
    let mut p = boot(source);
    let port = named(&p, "port");
    p.scroll_to(port, (0., 2370.)).unwrap();
    settle(&mut p);
    assert_eq!(p.scroll_of(port), (0., 2370.));
    p.set_deferred_collections(true);
    press(&mut p, "prepend");
    assert!(p.pump(p.host.now()).is_none());
    p.wheel(port, 0., -50.).unwrap();
    assert_eq!(p.scroll_of(port), (0., 2320.));
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    p.run_deferred_collections();
    for _ in 0..8 {
        assert!(p.pump(p.host.now()).is_none());
        if let Some(frame) = p.display_frame() {
            assert!(p.display_complete(&frame));
        }
        p.run_deferred_collections();
    }
    assert_eq!(p.scroll_of(port), (0., 2320.), "where the wheel put it");
}

#[test]
fn a_clamp_in_the_flip_gap_keeps_a_seeks_scrollend() {
    // theme2 11:06 (Y; Grok MAJOR), the reviewer's sequence: at 800 (travel 1000) a seek to 100 is painted and in
    // flight; the content shrinks (travel 400) and the clamp runs before the acknowledgement, advancing the
    // seek's base. The seek is matched by what it is (list, sequence, target), not by its base: its scrollend
    // fires.
    let source = r#"component App
  state top = 800
  state count = 65
  state ends = 0
  action seek
    top = 100
  action shrink
    count = 35
  action ended
    ends = ends + 1
  resource rows = rows(0, count) as shape number[]
  view
    column width="100%"
      button "Seek" press=seek testId="seek"
      button "Shrink" press=shrink testId="shrink"
      text `${ends}` testId="ends"
      scroll testId="port" width=400 height=300 scrollTop=top scrollend=ended
        for x in rows key=x
          box testId=`r${x}` width=400 height=20
"#;
    let mut p = boot(source);
    let port = named(&p, "port");
    assert_eq!(p.scroll_of(port), (0., 800.));
    let ends = |p: &Presenter<Rows>| {
        let id = named(p, "ends");
        p.host
            .kernel()
            .node(id)
            .and_then(|n| n.props.str(PropId::Text).map(str::to_owned))
            .unwrap_or_default()
    };
    let before = ends(&p);
    press(&mut p, "seek");
    assert!(p.pump(p.host.now()).is_none());
    let frame = p.display_frame().unwrap();
    press(&mut p, "shrink");
    // The clamp in the flip gap, as `clamp_scroll` makes it when a pump or an image report lands before the
    // flip completes: the input offset 800 to the new travel's 400, the pending seek's base advanced with it.
    p.scroll.insert(port, (0., 400.));
    p.rebase_clamped_anchor(port, (0., 800.), (0., 400.));
    assert!(p.display_complete(&frame));
    assert_eq!(p.scroll_of(port), (0., 100.), "the seek landed");
    for _ in 0..8 {
        p.advance(p.host.now() + 200.);
        if let Some(frame) = p.display_frame() {
            assert!(p.display_complete(&frame));
        }
    }
    assert_ne!(ends(&p), before, "the seek's scrollend fired");
}
