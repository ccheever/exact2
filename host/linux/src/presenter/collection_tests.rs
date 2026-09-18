use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, args: &[Value]) -> Result<Value, DataError> {
        if let [Value::Number(first)] = args {
            return Ok(Value::list(vec![
                Value::Number(*first),
                Value::Number(first + 1.),
            ]));
        }
        Ok(Value::list(
            (0..25_000).map(|n| Value::Number(n as f64)).collect(),
        ))
    }
}
fn boot(row: &str) -> Presenter<Rows> {
    let source = format!(
        r#"component App
  resource rows = rows() as shape list<number>
  view
    column width="100%" padding=20 box-sizing="border-box"
      text "nested port"
      list virtualized=true width="100%" height=180 padding-left=10 padding-right=10 border-width=2 border-style="solid" box-sizing="border-box" testId="port"
        each x in rows key=x
          {row}
"#
    );
    boot_source(&source)
}
fn boot_source(source: &str) -> Presenter<Rows> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Rows,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn settle<D: DataSource>(p: &mut Presenter<D>) {
    for _ in 0..16 {
        assert!(p.pump(p.host.now()).is_none());
        if p.dirty() {
            let _ = p.frame();
        }
    }
}
#[test]
fn collection_boot_measures_wrappers_in_nested_port_and_scroll_rewindows() {
    let mut p = boot("text `row ${x}` height=24");
    settle(&mut p);
    let before = p.host.runner().collections().pop().unwrap();
    assert!(
        before.rows.iter().all(|r| r.measured),
        "host never measured wrappers"
    );
    assert!(before.rows.len() < 40);
    let offered = p
        .host
        .kernel()
        .node(before.rows[0].view)
        .unwrap()
        .frame
        .width;
    assert_eq!(offered, 336.);
    p.wheel(before.view, 0., 24_000.).unwrap();
    settle(&mut p);
    let after = p.host.runner().collections().pop().unwrap();
    assert!(after.rows.iter().all(|r| r.index > 500));
    assert!(after.rows.iter().all(|r| r.measured));
    assert!(after.rows.len() < 40);
    assert!(p.scroll_of(after.view).1 > 10_000.);
    assert!(p.host.kernel().find_by_test_id("port").len() == 1);
}

#[test]
fn edge_membership_commits_continue_beyond_two_reports_without_scroll() {
    let mut p = boot_source(
        r#"component App
  state first = 0
  resource rows = rows(first) as shape list<number>
  action start writes first
    if first < 12
      first = first + 2
  view
    list virtualized=true height=180 width=320 reachstart=start
      each x in rows key=x
        text `${x}` height=1
"#,
    );
    // Drive only the presenter's normal pending-work loop, with no wheel or
    // manually supplied runner feedback to rescue membership after pass two.
    settle(&mut p);
    assert_eq!(p.host.runner().slot("first"), Some(&Value::Number(12.)));
    let snapshot = &p.host.collections()[0];
    assert!(snapshot.rows.iter().all(|row| row.measured));
    assert!(!p.collection.pending());
}

#[test]
fn activation_retries_a_refused_edge_with_unchanged_endpoints() {
    struct Deferred(bool);
    impl DataSource for Deferred {
        fn ready(&self) -> bool {
            self.0
        }
        fn activate(&mut self) -> Result<(), DataError> {
            self.0 = true;
            Ok(())
        }
        fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
            if source == "rows" {
                return Ok(Value::list(vec![Value::Number(0.), Value::Number(1.)]));
            }
            if self.0 {
                Ok(args[0].clone())
            } else {
                Err(DataError::Unavailable("executor not activated".into()))
            }
        }
    }
    let source = r#"component App
  state next = 0
  resource answer = answer(next) as shape number
  resource rows = rows() as shape list<number>
  action start writes next
    next = next + 1
  view
    column
      text `${answer}`
      list virtualized=true height=180 width=320 reachstart=start
        each x in rows key=x
          text `${x}` height=24
"#;
    let plan = contract::bake(contract::compile(source).unwrap(), Deferred(true)).unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        Deferred(false),
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    // Paint and measure before activation, exactly as the host can at boot.
    for _ in 0..4 {
        let _ = p.pump(p.host.now());
        if p.dirty() {
            let _ = p.frame();
        }
    }
    assert_eq!(p.host.runner().slot("next"), Some(&Value::Number(0.)));
    let before = p.host.collections()[0].count;
    p.first_pixel();
    settle(&mut p);
    assert_eq!(p.host.runner().slot("next"), Some(&Value::Number(1.)));
    assert_eq!(
        p.host.collections()[0].count,
        before,
        "activation did not change the supplied rows"
    );
}

#[test]
fn authored_collection_scroll_top_is_consumed_once_and_latest_reissues_it() {
    let mut p = boot_source(
        r#"component App
  state requested = 1000000
  resource rows = rows() as shape list<number>
  action latest writes requested
    requested = requested + 1000
  view
    column
      button press=latest testId="latest"
        text "Latest"
      list virtualized=true scrollFollowEnd=true scrollTop=requested height=180 width=400
        each x in rows key=x
          text `${x}` height=24
"#,
    );
    settle(&mut p);
    let c = p.host.collections().remove(0);
    assert_eq!(
        c.rows.last().unwrap().index,
        24_999,
        "initial authored offset"
    );
    p.wheel(c.view, 0., -10_000_000.).unwrap();
    settle(&mut p);
    assert_eq!(
        p.scroll_of(c.view).1,
        0.,
        "unchanged prop must not override reader"
    );
    let button = p.host.kernel().find_by_test_id("latest")[0];
    let button = p.host.kernel().node_by_key(button).unwrap().id;
    p.tap(button).unwrap();
    settle(&mut p);
    assert_eq!(
        p.host.collections()[0].rows.last().unwrap().index,
        24_999,
        "Latest offset request"
    );
}

#[test]
fn fractional_high_extent_end_follow_needs_one_wheel_without_false_origin_changes() {
    let mut p = boot_source(
        r#"component App
  resource rows = rows() as shape list<number>
  view
    column width="100%" padding=20 box-sizing="border-box"
      list virtualized=true scrollFollowEnd=true height=180 width="100%" padding-left=10 padding-right=10 border-width=2 border-style="solid" box-sizing="border-box"
        each x in rows key=x
          text `row ${x}` height=100.1
"#,
    );
    settle(&mut p);
    let before = p.host.collections().pop().unwrap();
    let node = p.host.kernel().node(before.view).unwrap();
    let [border_top, _, border_bottom, _] = node.style.border_widths();
    eprintln!(
        "bordered range: extent={} outer_height={} inner_height={} raw_content={} eager_limit={} virtual_limit={}",
        before.total_extent,
        node.frame.height,
        node.frame.height - border_top - border_bottom,
        content_size(&node, p.host.kernel()).1,
        content_size(&node, p.host.kernel()).1 - node.frame.height,
        p.collection_scroll_limits()[&before.view]
    );
    p.wheel(before.view, 0., 10_000_000.).unwrap();
    // Only pump/frame turns follow the single wheel. Measurements must not
    // invent an origin change from f32 wrapper positions minus f64 row tops.
    settle(&mut p);
    let after = p.host.collections().pop().unwrap();
    assert_eq!(
        after.scroll_sequence,
        before.scroll_sequence + 1,
        "measurement roundoff must not advance the host scroll sequence"
    );
    assert_eq!(
        after.rows.last().unwrap().index,
        24_999,
        "before={before:?}; after={after:?}; native_top={}",
        p.scroll_of(after.view).1
    );
    let port = p.host.kernel().node(after.view).unwrap();
    let [top, _, bottom, _] = port.style.border_widths();
    let height = port.frame.height - top - bottom;
    assert!(
        (p.scroll_of(after.view).1 as f64 - (after.total_extent - height as f64)).abs() < 0.5,
        "one wheel must consume the end correction"
    );
    assert!(!p.dirty(), "bounded refinement eventually becomes idle");
}

#[test]
fn collection_resize_remeasures_new_width_without_unbounded_frame_loop() {
    let mut p = boot("text `row ${x} with words to wrap when the port gets narrower` font-size=16");
    settle(&mut p);
    let before = p.host.runner().collections().pop().unwrap();
    assert!(p.resize(240., 500.).is_none());
    settle(&mut p);
    let after = p.host.runner().collections().pop().unwrap();
    assert!(after.rows.iter().all(|r| r.measured));
    assert_ne!(before.rows[0].epoch, after.rows[0].epoch);
    assert!(after.rows[0].height > before.rows[0].height);
    assert_eq!(
        p.host
            .kernel()
            .node(after.rows[0].view)
            .unwrap()
            .frame
            .width,
        176.
    );
    let epoch = p.host.kernel().epoch();
    settle(&mut p);
    assert_eq!(
        epoch,
        p.host.kernel().epoch(),
        "settled feedback must stop committing"
    );
    assert!(!p.dirty());
}

#[test]
fn focus_and_single_interaction_pin_survive_scroll_then_release() {
    let mut p = boot("input value=\"\" height=24");
    settle(&mut p);
    let initial = p.host.collections().pop().unwrap();
    let focus = initial.rows[0].root;
    let interaction = initial.rows[1].root;
    p.tap(focus).unwrap();
    p.set_collection_interaction(Some(interaction));
    p.wheel(initial.view, 0., 24_000.).unwrap();
    settle(&mut p);
    let now = p.host.collections().pop().unwrap();
    assert_eq!(p.focus(), Some(focus));
    assert!(now.rows.iter().any(|r| r.root == focus));
    assert!(now.rows.iter().any(|r| r.root == interaction));
    assert_eq!(now.rows.iter().filter(|r| r.index < 500).count(), 2);
    p.blur();
    p.set_collection_interaction(None);
    settle(&mut p);
    let now = p.host.collections().pop().unwrap();
    assert!(now.rows.iter().all(|r| r.index > 500));
    assert!(p.host.kernel().node(focus).is_none());
    assert!(p.host.kernel().node(interaction).is_none());
}

#[test]
fn ordinary_scroll_handler_runs_beside_collection_observation() {
    let mut p = boot_source(
        r#"component App
  state top = 0
  resource rows = rows() as shape list<number>
  action moved(x, y) writes top
    top = y
  view
    column
      text `${top}` testId="observed"
      list virtualized=true height=180 width="100%" scroll=moved testId="port"
        each x in rows key=x
          text `${x}` height=24
"#,
    );
    settle(&mut p);
    let list = p.host.collections()[0].view;
    p.wheel(list, 0., 8_000.).unwrap();
    settle(&mut p);
    let key = p.host.kernel().find_by_test_id("observed")[0];
    assert_eq!(
        p.host
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("8000")
    );
    assert!(p.host.collections()[0].rows.iter().all(|r| r.index > 100));
}

#[test]
fn feedback_budget_schedules_later_progress_and_stale_rows_do_not_commit() {
    let mut p = boot("text `${x}` height=24");
    p.scroll
        .insert(p.host.collections()[0].view, (0., 120_000.));
    p.queue_collections();
    let before = p.host.kernel().epoch();
    assert!(p.refine_collections().is_none());
    assert!(p.host.kernel().epoch() - before <= 2);
    // Further work remains scheduled rather than recursively exhausting it.
    assert!(p.collection.pending());
    settle(&mut p);
    let snapshot = p.host.collections().pop().unwrap();
    let epoch = p.host.kernel().epoch();
    let stale = exact_runner::CollectionFeedback {
        view: snapshot.view,
        revision: snapshot.revision,
        scroll_sequence: snapshot.scroll_sequence,
        scroll_top: 0.,
        port_width: 360.,
        port_height: 180.,
        row_width: 336.,
        measurements: vec![exact_runner::RowMeasurement {
            view: snapshot.rows[0].view,
            epoch: snapshot.rows[0].epoch + 1,
            height: 999.,
        }],
        focus_view: None,
        interaction_view: None,
    };
    assert!(!p.host.collection_feedback(stale).unwrap());
    assert_eq!(p.host.kernel().epoch(), epoch);
    assert!(!p.dirty());
}

#[test]
fn interaction_release_cancel_and_navigation_clear_the_pin() {
    let mut p = boot_source(
        r#"component App
  state selected = "list"
  resource rows = rows() as shape list<number>
  action away writes selected
    selected = "away"
  action back writes selected
    selected = "list"
  view
    main navigationKey=selected navigationBack="back"
      button press=away testId="away"
        text "away"
      button press=back testId="back"
        text "back"
      column navigationKey="list"
        list virtualized=true height=180 width="100%"
          each x in rows key=x
            button testId=`row-${x}` height=24
              text `${x}`
      column navigationKey="away"
        text "other route"
"#,
    );
    settle(&mut p);
    let view = p.host.collections()[0].rows[0].root;
    p.set_collection_interaction(Some(view));
    assert_eq!(p.collection_interaction(), Some(view));
    p.set_collection_interaction(None); // button release and Escape cancellation
    assert_eq!(p.collection_interaction(), None);
    p.set_collection_interaction(Some(view));
    for target in ["away", "back"] {
        let key = p.host.kernel().find_by_test_id(target)[0];
        let button = p.host.kernel().node_by_key(key).unwrap().id;
        assert!(p
            .host
            .dispatch_at(button, Event::Press, p.host.now())
            .is_none());
        assert!(p.after_commit().is_none());
        settle(&mut p);
        assert_eq!(
            p.collection_interaction(),
            None,
            "navigation must discard the interaction, not revive it on return"
        );
    }
    p.wheel(p.host.collections()[0].view, 0., 24_000.).unwrap();
    settle(&mut p);
    assert!(
        p.host.kernel().node(view).is_none(),
        "cleared pin must permit row retirement"
    );
}

#[test]
fn height_projection_refines_real_25k_port_through_hold_ticks_resize_and_typing() {
    use exact_motion::HoldEnd;
    let source = r#"component App
  resource rows = rows() as shape list<number>
  state draft = ""
  state target = 180
  action edit(value) writes draft
    draft = value
  action grow writes target
    target = 420
  view
    box width="100%" height="100%"
      input value=draft change=edit testId="input"
      button press=grow testId="grow"
        text "grow"
      column position="absolute" bottom=0 width="100%" height=target max-height="100%" padding=8 border-width=2 border-style="solid" box-sizing="border-box" transition="height spring(300,30,1)" testId="panel"
        list virtualized=true scrollFollowEnd=true flex=1 min-height=0 width="100%" testId="port"
          each x in rows key=x
            text `row ${x}` height=24
"#;
    let mut p = boot_source(source);
    settle(&mut p);
    let view = |p: &Presenter<Rows>, name| {
        p.host
            .kernel()
            .node_by_key(p.host.kernel().find_by_test_id(name)[0])
            .unwrap()
            .id
    };
    let panel = view(&p, "panel");
    let input = view(&p, "input");
    p.set_height_owner(Some(panel)).unwrap();
    let held = p.height_begin(0.).unwrap().unwrap();
    let port = view(&p, "port");
    p.wheel(port, 0., 10_000_000.).unwrap();
    settle(&mut p);
    for (i, px) in [256., 105., 420., 180.].into_iter().enumerate() {
        assert!(p.height_update(held.token, px, i as f64).unwrap());
        p.type_text(input, &format!("typed {i}")).unwrap();
        // Inspect the publication before any eventual settle/screenshot hides a gap.
        let snapshot = p.host.collections().pop().unwrap();
        let port_height = p.host.kernel().node(port).unwrap().frame.height;
        assert_eq!(p.host.kernel().node(panel).unwrap().frame.height, px as f32);
        assert_eq!(port_height, px as f32 - 20.);
        assert!(snapshot.rows.len() < 64);
        assert!(p.node_count() < 200);
        let offset = p.scroll_of(port).1 as f64;
        assert!(snapshot.rows.first().unwrap().top <= offset + 0.5);
        let last = snapshot.rows.last().unwrap();
        assert!(
            last.top + last.height
                >= (offset + port_height as f64).min(snapshot.total_extent) - 0.5
        );
        assert_eq!(last.index, 24_999);
        assert_eq!(
            p.host.kernel().node(panel).unwrap().style.height,
            exact_kernel::Dimension::Points(180.)
        );
    }
    p.tap(view(&p, "grow")).unwrap();
    assert_eq!(p.host.kernel().node(panel).unwrap().frame.height, 180.);
    p.height_end(held.token, HoldEnd::Cancel, 3.).unwrap();
    p.tick(80.);
    let _ = p.frame();
    assert!(p.host.kernel().node(panel).unwrap().frame.height > 180.);
    assert!(p.resize(400., 200.).is_none());
    p.tick(10_000.);
    let _ = p.frame();
    assert_eq!(p.host.kernel().node(panel).unwrap().frame.height, 200.);
    assert_eq!(p.host.kernel().node(port).unwrap().frame.height, 180.);
    settle(&mut p);
    assert!(!p.dirty());
    let old = p.height_begin(10_000.).unwrap().unwrap();
    p.reload(&contract::compile(source).unwrap().encode(), Rows)
        .unwrap();
    assert!(p.host.height_owner().is_none());
    assert!(!p.height_update(old.token, f64::NAN, f64::NAN).unwrap());
    assert!(!p.height_end(old.token, HoldEnd::Cancel, f64::NAN).unwrap());
}
