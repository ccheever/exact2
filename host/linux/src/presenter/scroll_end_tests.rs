use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

fn boot(scroll: bool) -> Presenter<NoData> {
    let source = format!(
        r#"component App
  state top = 0
  state visible = true
  state moves = 0
  state ends = 0
  state endedAt = 0
  state order = ""
  action moved(x: number, y: number)
    moves = moves + 1
    order = `${{order}}s`
  action ended
    ends = ends + 1
    endedAt = performanceNow()
    order = `${{order}}e`
  action jump
    top = top + 100
  action remove
    visible = false
  view
    column
      button "Jump" press=jump testId="jump"
      button "Remove" press=remove testId="remove"
      if visible
        scroll width=200 height=100 scrollTop=top testId="port" scrollend=ended {}
          box width=200 height=1000
"#,
        if scroll { "scroll=moved" } else { "" }
    );
    boot_source(&source)
}

fn boot_source(source: &str) -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    p
}

fn named<D: DataSource>(p: &Presenter<D>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn number<D: DataSource>(p: &Presenter<D>, name: &str) -> f64 {
    match p.host.runner().slot(name).unwrap() {
        Value::Number(n) => *n,
        value => panic!("{name}: {value:?}"),
    }
}

#[test]
fn discrete_notches_complete_at_their_sample_times() {
    for live in [false, true] {
        let mut p = boot(true);
        let port = named(&p, "port");
        for (i, at) in [0., 16., 32.].into_iter().enumerate() {
            assert_eq!(p.clock(at), (at, None));
            p.wheel(port, 0., 20.).unwrap();
            if live {
                assert_eq!(p.pump(at), None);
            } else {
                assert_eq!(p.clock(at), (at, None));
            }
            assert_eq!(number(&p, "ends"), (i + 1) as f64);
            assert_eq!(number(&p, "endedAt"), at);
        }
        assert_eq!(p.host.runner().slot("order"), Some(&Value::str("sesese")));
        assert_eq!(p.host.timer_due_ms(), None);
    }
}

fn text(p: &Presenter<NoData>, name: &str) -> String {
    p.host
        .runner()
        .slot(name)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}
fn press(p: &mut Presenter<NoData>, name: &str, at: f64) {
    let id = named(p, name);
    assert_eq!(p.host.dispatch_at(id, Event::Press, at), None);
    assert_eq!(p.after_commit(), None);
    p.run_commands(NoData::default);
}
fn reader(p: &mut Presenter<NoData>, at: f64, dy: f32) {
    assert_eq!(p.clock(at), (at, None));
    let port = named(p, "port");
    let b = p.box_of(port).unwrap();
    let words = p.reader_under(b.rect.0 + 10., b.rect.1 + 10., 1.);
    assert_eq!(words[3], port as i32);
    assert_eq!(
        p.reader_scrolled(port, (0., dy), words[5] as u32),
        Some((0., dy))
    );
}
fn settled(p: &mut Presenter<NoData>) -> serde_json::Value {
    serde_json::from_str(&crate::agent::handle(p, r#"{"op":"clock","settle":true}"#)).unwrap()
}

#[test]
fn reader_quiet_deadline_rearms_only_on_movement_and_wakes_the_shell() {
    let mut p = boot(true);
    for at in [1000., 1050., 1100., 1150.] {
        reader(&mut p, at, 20.);
        assert_eq!(p.timer_due_ms(), Some(at + 100.));
        assert_eq!(p.animation_frame(at + 49.), None);
        assert_eq!(number(&p, "ends"), 0.);
    }
    reader(&mut p, 1249., 0.);
    assert_eq!(p.timer_due_ms(), Some(1250.));
    assert_eq!(p.advance(1250.), None);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(number(&p, "endedAt"), 1250.);
    assert_eq!(text(&p, "order"), "sssse");
    assert_eq!(p.timer_due_ms(), None);
    assert_eq!(p.clock(2000.), (2000., None));
    assert_eq!(number(&p, "ends"), 1.);
}

#[test]
fn a_late_delivery_follows_main_catch_up_and_uses_the_landing_time() {
    for mode in 0..5 {
        let mut p = boot_source(
            r#"component App
  state order = ""
  state endedAt = -1
  action timer
    order = `${order}t`
  action ended
    order = `${order}e`
    endedAt = performanceNow()
  task once mount
    after(120, timer)
  view
    scroll testId="port" width=200 height=100 scrollend=ended
      box height=1000
"#,
        );
        reader(&mut p, 0., 20.);
        let error = match mode {
            0 => p.advance(150.),
            1 => p.clock(150.).1,
            2 => p.clock_until_request(150.).1,
            3 => p.animation_frame(150.),
            _ => {
                assert_eq!(p.host.advance(150.), None);
                p.pump(150.)
            }
        };
        assert_eq!(error, None);
        assert_eq!(text(&p, "order"), "te");
        assert_eq!(number(&p, "endedAt"), 150.);
    }
}

#[test]
fn jump_end_only_edges_and_removed_reader() {
    let mut p = boot(false);
    let port = named(&p, "port");
    p.wheel(port, 0., -20.).unwrap();
    assert_eq!(p.timer_due_ms(), None);
    press(&mut p, "jump", 5000.);
    assert_eq!(p.pump(5000.), None);
    assert_eq!(text(&p, "order"), "e");
    assert_eq!(number(&p, "endedAt"), 5000.);
    reader(&mut p, 5100., 20.);
    press(&mut p, "remove", 5150.);
    assert_eq!(p.timer_due_ms(), None);
    p.clock(6000.);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(number(&p, "moves"), 0.);
}

#[test]
fn settle_drains_a_reader_and_a_followup_command() {
    let mut p = boot_source(
        r#"component App
  state ends = 0
  state endedAt = -1
  action ended
    if ends == 0
      scrollBy("port", 0, 20)
    ends = ends + 1
    endedAt = performanceNow()
  view
    scroll id="port" testId="port" width=200 height=100 scrollend=ended
      box height=1000
"#,
    );
    reader(&mut p, 50., 20.);
    let result = settled(&mut p);
    assert_eq!(result["settled"], true, "{result}");
    assert_eq!(result["clock"], 150.);
    assert_eq!(number(&p, "ends"), 2.);
    assert_eq!(number(&p, "endedAt"), 150.);
    assert_eq!(p.scroll_of(named(&p, "port")).1, 40.);
}

#[test]
fn settle_drains_commands_mediated_by_focus_and_blur() {
    let mut p = boot_source(
        r#"component App
  state ends = 0
  action ended
    if ends == 0
      focus("field")
    ends = ends + 1
  action focused
    blur("field")
  action blurred
    scrollBy("port", 0, 20)
  view
    column
      input id="field" focus=focused blur=blurred height=30
      scroll id="port" testId="port" width=200 height=100 scrollend=ended
        box height=1000
"#,
    );
    reader(&mut p, 50., 20.);
    let result = settled(&mut p);
    assert_eq!(result["settled"], true, "{result}");
    assert_eq!(result["clock"], 150.);
    assert_eq!(number(&p, "ends"), 2.);
    assert_eq!(p.scroll_of(named(&p, "port")).1, 40.);
    assert!(p.commands.is_empty());
}

#[test]
fn batched_notches_each_keep_their_completion() {
    batched_moves(false);
}

#[test]
fn batched_commands_each_keep_their_completion() {
    batched_moves(true);
}

fn batched_moves(commands: bool) {
    let mut p = boot_source(
        r#"component App
  state order = ""
  action jump
    scrollBy("port", 0, 20)
    scrollBy("port", 0, 20)
  action moved(x: number, y: number)
    order = `${order}s`
  action ended
    order = `${order}e`
  view
    column
      button "Jump" testId="jump" press=jump
      scroll id="port" testId="port" width=200 height=100 scroll=moved scrollend=ended
        box height=1000
"#,
    );
    if commands {
        press(&mut p, "jump", 0.);
    } else {
        for _ in 0..2 {
            p.wheel(named(&p, "port"), 0., 20.).unwrap();
        }
    }
    assert_eq!(p.scroll_of(named(&p, "port")).1, 40.);
    assert_eq!(p.pump(0.), None);
    assert_eq!(text(&p, "order"), "ssee", "commands: {commands}");
    assert_eq!(p.timer_due_ms(), None);
}

#[test]
fn a_due_reader_is_preserved_when_the_next_sample_arrives_before_delivery() {
    let mut p = boot(true);
    reader(&mut p, 0., 20.);
    // The display shell samples input before its ordinary Presenter catch-up.
    assert_eq!(p.host.advance(150.), None);
    let port = named(&p, "port");
    assert_eq!(p.reader_scrolled(port, (0., 20.), 0), Some((0., 20.)));
    assert_eq!(p.pump(150.), None);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(p.timer_due_ms(), Some(250.));
    p.advance(250.);
    assert_eq!(text(&p, "order"), "ssee");
}

fn collection_source(extra: &str) -> String {
    format!(
        r#"component App
  state rows = [0, 1, 2, 3, 4]
  state height = 100
  state visible = false
  state ends = 0
  action show
    visible = true
  action shrink
    rows = [0, 1]
  action grow
    rows = [0, 1, 2, 3, 4, 5]
  action compact
    height = 50
  action ended
    ends = ends + 1
  view
    column
      button "Show" testId="show" press=show
      button "Shrink" testId="shrink" press=shrink
      button "Grow" testId="grow" press=grow
      button "Compact" testId="compact" press=compact
      if visible
        list virtualized=true testId="port" width=200 height=100 scrollend=ended {extra}
          for x in rows key=x
            box height=height
"#
    )
}

#[test]
fn collection_clamps_keep_the_reader_deadline() {
    for follow in [false, true] {
        let mut p = boot_source(&collection_source(&format!("scrollFollowEnd={follow}")));
        press(&mut p, "show", 0.);
        assert_eq!(p.settle_collections(), None);
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        reader(&mut p, 0., 400.);
        assert_eq!(p.scroll_of(named(&p, "port")).1, 400.);
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        press(&mut p, "compact", 50.);
        assert_eq!(p.settle_collections(), None);
        let frame = p.display_frame().unwrap();
        assert!(p.display_complete(&frame));
        let port = named(&p, "port");
        assert!(p.scroll_of(port).1 < 400.);
        assert_eq!(p.scroll_of(port).1, p.collection_scroll_limits()[&port]);
        assert_eq!(p.timer_due_ms(), Some(100.), "follow: {follow}");
        p.advance(99.);
        assert_eq!(number(&p, "ends"), 0.);
        p.advance(100.);
        assert_eq!(number(&p, "ends"), 1.);
    }
}

#[test]
fn a_collection_opened_at_the_end_after_boot_owes_no_completion() {
    for follow in [false, true] {
        let mut p = boot_source(&collection_source(&format!(
            "scroll-start=\"end\" scrollFollowEnd={follow}"
        )));
        press(&mut p, "show", 0.);
        assert_eq!(p.settle_collections(), None);
        assert_eq!(p.scroll_of(named(&p, "port")).1, 400.);
        assert_eq!(p.timer_due_ms(), None, "follow: {follow}");
        p.advance(100.);
        assert_eq!(number(&p, "ends"), 0.);
    }
}

#[test]
fn collection_end_follow_still_completes_when_content_arrives() {
    let mut p = boot_source(
        &collection_source("scrollFollowEnd=true").replace("box height=height", "box height=100"),
    );
    press(&mut p, "show", 0.);
    assert_eq!(p.settle_collections(), None);
    p.wheel(named(&p, "port"), 0., 400.).unwrap();
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 1.);
    press(&mut p, "grow", 50.);
    assert_eq!(p.settle_collections(), None);
    assert_eq!(p.scroll_of(named(&p, "port")).1, 500.);
    p.pump(50.);
    assert_eq!(number(&p, "ends"), 2.);
}

#[test]
fn keyboard_command_completes_after_its_scroll() {
    let mut p = boot_source(
        r#"component App
  state order = ""
  state endedAt = -1
  action move(key: string)
    scrollBy("port", 0, 120)
  action moved(x: number, y: number)
    order = `${order}s`
  action ended
    order = `${order}e`
    endedAt = performanceNow()
  view
    column
      button "Scroll" key=move testId="key"
      scroll id="port" testId="port" width=200 height=100 scroll=moved scrollend=ended
        box height=1000
"#,
    );
    p.tap(named(&p, "key")).unwrap();
    p.clock(5000.);
    p.hardware_key("ArrowDown", "ArrowDown", true, false);
    p.run_commands(NoData::default);
    p.pump(5000.);
    assert_eq!(text(&p, "order"), "se");
    assert_eq!(number(&p, "endedAt"), 5000.);
}

#[test]
fn hidden_self_or_parent_retires_debt_but_inert_keeps_it() {
    for parent in [false, true] {
        for inert in [false, true] {
            let prop = if inert {
                "inert=hidden"
            } else {
                "display=(hidden ? \"none\" : \"block\")"
            };
            let mut p = boot_source(&format!(
                r#"component App
  state hidden = false
  state ends = 0
  action hide
    hidden = true
  action show
    hidden = false
  action ended
    ends = ends + 1
  view
    column
      button "Hide" testId="hide" press=hide
      button "Show" testId="show" press=show
      box {}
        scroll testId="port" width=200 height=100 scrollend=ended {}
          box height=1000
"#,
                if parent { prop } else { "" },
                if parent { "" } else { prop }
            ));
            reader(&mut p, 1000., 20.);
            press(&mut p, "hide", 1050.);
            assert_eq!(p.timer_due_ms(), if inert { Some(1100.) } else { None });
            p.clock(1100.);
            assert_eq!(number(&p, "ends"), if inert { 1. } else { 0. });
            press(&mut p, "show", 1150.);
            p.clock(2000.);
            assert_eq!(number(&p, "ends"), if inert { 1. } else { 0. });
        }
    }
}

#[test]
fn modal_underlay_keeps_completion_and_push_cancels_it() {
    for modal in [false, true] {
        let mut p = boot_source(&format!(
            r#"routes nav
  home "/"
    detail "/detail"
component App
  state ends = 0
  action ended
    ends = ends + 1
  action open
    nav = push(nav, "/detail")
  action back
    nav = back(nav)
  view
    main navigationKey=`${{top(nav).id}}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${{e.id}}` navigationPresentation="{}" position="absolute" inset=0
          if e.name == "home"
            button "Open" testId="open" press=open
            scroll testId="port" width=200 height=100 scrollend=ended
              box height=2000
          if e.name == "detail"
            button "Back" testId="back" press=back
"#,
            if modal { "modal" } else { "push" }
        ));
        reader(&mut p, 1000., 20.);
        press(&mut p, "open", 1050.);
        assert_eq!(p.host.route_visibility(named(&p, "port")), (!modal, true));
        p.clock(1100.);
        assert_eq!(number(&p, "ends"), if modal { 1. } else { 0. });
        press(&mut p, "back", 1200.);
        p.clock(2000.);
        assert_eq!(number(&p, "ends"), if modal { 1. } else { 0. });
    }
}

#[test]
fn deferred_wheel_waits_for_its_scroll_handler() {
    let mut p = boot(true);
    p.set_deferred_collections(true);
    p.wheel(named(&p, "port"), 0., 20.).unwrap();
    p.wheel(named(&p, "port"), 0., 20.).unwrap();
    p.advance(50.);
    assert_eq!(text(&p, "order"), "");
    assert_eq!(p.timer_or_scroll_due_ms(), None);
    assert_eq!(p.timer_due_ms(), None);
    assert!(p.run_deferred_collections());
    assert_eq!(text(&p, "order"), "s");
    assert_eq!(p.timer_or_scroll_due_ms(), Some(0.));
    p.pump(50.);
    assert_eq!(text(&p, "order"), "se");
    assert_eq!(number(&p, "moves"), 1.);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(p.scroll_of(named(&p, "port")).1, 40.);
    assert_eq!(number(&p, "endedAt"), 50.);
}

#[test]
fn clamping_a_reader_does_not_restart_its_quiet_deadline() {
    let mut p = boot_source(
        r#"component App
  state height = 1000
  state ends = 0
  action shrink
    height = 150
  action ended
    ends = ends + 1
  view
    column
      button "Shrink" press=shrink testId="shrink"
      scroll testId="port" width=200 height=100 scrollend=ended
        box height=height
"#,
    );
    reader(&mut p, 0., 200.);
    press(&mut p, "shrink", 50.);
    assert_eq!(p.scroll_of(named(&p, "port")).1, 50.);
    assert_eq!(p.timer_due_ms(), Some(100.));
    p.clock(100.);
    assert_eq!(number(&p, "ends"), 1.);
    p.resize(400., 600.);
    p.clock(300.);
    assert_eq!(number(&p, "ends"), 1.);
}

#[test]
fn a_model_jump_completes_only_after_the_picture_applies_it() {
    let mut p = boot(true);
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    press(&mut p, "jump", 50.);
    assert_eq!(p.scroll_of(named(&p, "port")).1, 0.);
    p.pump(50.);
    assert_eq!(number(&p, "ends"), 0.);
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    assert_eq!(p.scroll_of(named(&p, "port")).1, 100.);
    p.advance(60.);
    assert_eq!(text(&p, "order"), "se");
    assert_eq!(number(&p, "endedAt"), 60.);
}

#[test]
fn smooth_into_view_uses_mains_instant_application() {
    let mut p = boot_source(
        r#"component App
  state ends = 0
  action jump
    scrollIntoView("target", behavior="smooth")
  action ended
    ends = ends + 1
  view
    column
      button "Jump" press=jump testId="jump"
      scroll testId="port" width=200 height=100 scrollend=ended
        box height=500
        box id="target" height=50
"#,
    );
    press(&mut p, "jump", 50.);
    assert!(p.scroll_of(named(&p, "port")).1 > 0.);
    p.pump(50.);
    assert_eq!(number(&p, "ends"), 1.);
}

#[test]
fn a_no_handler_app_creates_no_completion_or_wake() {
    let mut p = boot_source(
        r#"component App
  state top = 0
  action jump
    top = 100
  view
    column
      button "Jump" press=jump testId="jump"
      scroll testId="port" width=200 height=100 scrollTop=top
        box height=1000
"#,
    );
    reader(&mut p, 0., 20.);
    p.wheel(named(&p, "port"), 0., 20.).unwrap();
    press(&mut p, "jump", 50.);
    p.pump(50.);
    assert_eq!(p.timer_due_ms(), None);
    assert_eq!(p.scroll_end_due(), None);
}

#[test]
fn reader_agent_gesture_settles() {
    let mut p = boot(true);
    let port = named(&p, "port");
    let sample = format!(r#"{{"op":"tap","id":{port},"wheel":[0,20],"gesture":true}}"#);
    let reply = crate::agent::handle(&mut p, &sample);
    assert!(!reply.contains("error"), "{reply}");
    assert_eq!(number(&p, "ends"), 0.);
    assert_eq!(settled(&mut p)["clock"], 100.);
    assert_eq!(number(&p, "ends"), 1.);
}

#[test]
fn recursive_end_commands_stop_at_the_agent_bound() {
    let mut p = boot_source(
        r#"component App
  state ends = 0
  action ended
    scrollBy("port", 0, 1)
    ends = ends + 1
  view
    scroll id="port" testId="port" width=200 height=100 scrollend=ended
      box height=10000
"#,
    );
    p.wheel(named(&p, "port"), 0., 1.).unwrap();
    let result = settled(&mut p);
    assert_eq!(result["settled"], false, "{result}");
    assert!(number(&p, "ends") < 20.);
    assert!(p.scroll_end_due().is_some());
}

#[test]
fn row_recycling_retires_the_previous_items_completion() {
    struct Rows;
    impl DataSource for Rows {
        fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
            Ok(Value::list(
                (0..200).map(|i| Value::Number(i as f64)).collect(),
            ))
        }
    }
    let source = r#"component App
  resource rows = rows() as shape number[]
  state ends = 0
  action ended
    ends = ends + 1
  view
    list virtualized=true testId="list" width=200 height=300
      for x in rows key=x
        scroll testId=`row-${x}` width=180 height=50 scrollend=ended
          box height=500
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Rows,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    assert_eq!(p.settle_collections(), None);
    p.host.runner_mut().set_row_reuse(true);
    let mut old = Vec::new();
    for index in 0..6 {
        let id = named(&p, &format!("row-{index}"));
        let b = p.box_of(id).unwrap();
        let words = p.reader_under(b.rect.0 + 10., b.rect.1 + 10., 1.);
        assert_eq!(words[3], id as i32);
        assert_eq!(
            p.reader_scrolled(id, (0., 20.), words[5] as u32),
            Some((0., 20.))
        );
        old.push((
            id,
            p.host.kernel().node(id).unwrap().key,
            format!("row-{index}"),
        ));
    }
    let list = named(&p, "list");
    for top in [1000., 2000.] {
        p.scroll_to(list, (0., top)).unwrap();
        assert_eq!(p.settle_collections(), None);
    }
    assert!(
        old.iter().any(|(id, key, name)| p
            .host
            .kernel()
            .node(*id)
            .is_some_and(|n| n.key == *key && n.props.str(PropId::TestId) != Some(name.as_str()))),
        "at least one observed node must be rebound, not merely removed"
    );
    p.clock(200.);
    assert_eq!(number(&p, "ends"), 0.);
    assert_eq!(p.timer_due_ms(), None);
}

struct StagedPlan(Arc<[u8]>);
impl crate::delivery::Store for StagedPlan {
    fn prepare_selected(&mut self) -> Option<crate::delivery::Selection> {
        None
    }
    fn boot_started(&mut self) {}
    fn entry_refused(&mut self, _: &str, _: &str) {}
    fn selection_corrupt(&mut self, _: &str) {}
    fn boot_succeeded(&mut self) {}
    fn take_note(&mut self) -> Option<String> {
        None
    }
    #[cfg(unix)]
    fn fd(&self) -> std::os::unix::io::RawFd {
        -1
    }
    fn check(&self) -> bool {
        false
    }
    fn take_line(&mut self) -> Option<String> {
        None
    }
    fn prepare_activation(&self) -> Result<Option<crate::delivery::Selection>, String> {
        Ok(Some(crate::delivery::Selection {
            entry: Some("scrollend".into()),
            seq: 1,
            plan: self.0.clone(),
            assets: Arc::new(|_| Ok(None)),
        }))
    }
    fn commit_activation(&mut self, _: Option<String>, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn staged_stream_into(&self, _: &mut exact_runner::Delivery) {}
    fn status_into(&self, _: &mut exact_runner::Delivery) {}
}

#[test]
fn bundle_activation_cancels_old_debt_even_when_node_ids_repeat() {
    let mut p = boot(true);
    press(&mut p, "jump", 0.);
    p.clock(0.);
    assert_eq!(number(&p, "ends"), 1.);
    reader(&mut p, 50., 20.);
    let old = named(&p, "port");
    let plan = p.host.runner().plan().encode();
    p.set_updates(Some(Box::new(StagedPlan(plan.into()))));
    assert!(p.activate_update(NoData).unwrap());
    assert_eq!(named(&p, "port"), old);
    assert_eq!(
        p.scroll_of(old).1,
        100.,
        "carried binding restores without a new scroll"
    );
    assert_eq!(p.timer_due_ms(), None);
    p.clock(300.);
    assert_eq!(number(&p, "ends"), 1.);
}

#[test]
fn another_end_handlers_jump_delivers_its_pending_scroll_before_the_end() {
    let mut p = boot_source(
        r#"component App
  state top = 0
  state btop = 0
  state order = ""
  action jump
    top = 100
    btop = 100
  action a(x: number, y: number)
    order = `${order}a`
  action b(x: number, y: number)
    order = `${order}b`
  action ae
    order = `${order}A`
    btop = 200
  action be
    order = `${order}B`
  view
    column
      button "Jump" testId="jump" press=jump
      scroll width=200 height=100 scrollTop=top scroll=a scrollend=ae
        box height=1000
      scroll width=200 height=100 scrollTop=btop scroll=b scrollend=be
        box height=1000
"#,
    );
    press(&mut p, "jump", 0.);
    p.pump(0.);
    assert_eq!(text(&p, "order"), "abAbB");
    p.pump(0.);
    assert_eq!(text(&p, "order"), "abAbBB");
}

#[test]
fn the_reader_shells_existing_sample_stamp_does_not_advance_the_host() {
    let mut p = boot(false);
    reader(&mut p, 50., 20.);
    // CanvasHost stamps the accepted sample after reader_scrolled returns.
    p.scroll_end_reader_sampled(named(&p, "port"), 5000.);
    assert_eq!(p.host.now(), 50.);
    assert_eq!(p.host.runner().now_ms(), 50.);
    assert_eq!(p.timer_due_ms(), Some(5100.));
    p.advance(5099.);
    assert_eq!(number(&p, "ends"), 0.);
    p.advance(5100.);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(number(&p, "endedAt"), 5100.);
}

#[test]
fn a_late_reader_stamp_preserves_the_preceding_completion() {
    let mut p = boot(true);
    reader(&mut p, 0., 20.);
    let port = named(&p, "port");
    assert_eq!(p.reader_scrolled(port, (0., 20.), 0), Some((0., 20.)));
    p.scroll_end_reader_sampled(port, 150.);
    assert_eq!(p.host.now(), 0.);
    p.advance(150.);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(p.timer_due_ms(), Some(250.));
    p.advance(250.);
    assert_eq!(number(&p, "ends"), 2.);
    assert_eq!(text(&p, "order"), "ssee");
}

#[test]
fn a_wheel_delta_that_rounds_away_owes_no_completion() {
    let mut p = boot(true);
    let port = named(&p, "port");
    p.wheel(port, 0., 100.).unwrap();
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 1.);
    p.wheel(port, 0., f32::EPSILON).unwrap();
    assert_eq!(p.scroll_of(port).1, 100.);
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(p.timer_due_ms(), None);
}

#[test]
fn settle_includes_motion_started_by_a_completions_final_command() {
    let mut p = boot_source(
        r#"component App
  state faded = false
  action ended
    focus("field")
  action focused
    faded = true
  view
    column
      input id="field" focus=focused height=30
      box testId="fader" opacity=(faded ? 0 : 1) transition="opacity 200ms linear" width=20 height=20
      scroll testId="port" width=200 height=100 scrollend=ended
        box height=1000
"#,
    );
    reader(&mut p, 50., 20.);
    let result = settled(&mut p);
    assert_eq!(result["settled"], true, "{result}");
    assert_eq!(result["clock"], 350.);
    assert!(p.host.presented(named(&p, "fader")).opacity.abs() < f32::EPSILON);
}

#[test]
fn ordinary_follow_completes_growth_but_not_opening_resize_or_clamp() {
    for displayed in [false, true] {
        let mut p = boot_source(
            r#"component App
  state height = 500
  state port = 100
  state ends = 0
  action grow
    height = height + 200
  action resize
    port = 50
  action shrink
    height = 300
  action ended
    ends = ends + 1
  view
    column
      button "Grow" press=grow testId="grow"
      button "Resize" press=resize testId="resize"
      button "Shrink" press=shrink testId="shrink"
      scroll testId="port" scrollFollowEnd=true width=200 height=port scrollend=ended
        box width=200 height=height
"#,
        );
        let acknowledge = |p: &mut Presenter<NoData>| {
            if displayed {
                let frame = p.display_frame().unwrap();
                assert!(p.display_complete(&frame));
            }
        };
        acknowledge(&mut p);
        p.pump(0.);
        assert_eq!(number(&p, "ends"), 0.);
        press(&mut p, "grow", 0.);
        acknowledge(&mut p);
        assert_eq!(p.scroll_of(named(&p, "port")).1, 600.);
        p.pump(0.);
        assert_eq!(number(&p, "ends"), 1., "displayed: {displayed}");
        for action in ["resize", "shrink"] {
            press(&mut p, action, 0.);
            acknowledge(&mut p);
            p.pump(0.);
            assert_eq!(number(&p, "ends"), 1., "{action}, displayed: {displayed}");
        }
    }
}

#[test]
fn a_collection_seek_refines_to_one_completion() {
    let source = collection_source("")
        .replace(
            "state rows = [0, 1, 2, 3, 4]",
            "state rows = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]",
        )
        .replace(
            "    rows = [0, 1, 2, 3, 4, 5]",
            "    scrollIntoView(\"items\", target)\n    target = (target == 19 ? 0 : 19)",
        )
        .replace("state ends = 0", "state ends = 0\n  state target = 19")
        .replace(
            "list virtualized=true",
            "list id=\"items\" virtualized=true",
        );
    let mut p = boot_source(&source);
    press(&mut p, "show", 0.);
    p.settle_collections();
    press(&mut p, "grow", 0.);
    p.settle_collections();
    assert!(!p.host.collection(named(&p, "port")).unwrap().seeking);
    press(&mut p, "grow", 0.);
    p.settle_collections();
    assert_eq!(p.scroll_of(named(&p, "port")).1, 0.);
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 2.);
}

#[test]
fn an_acknowledged_collection_follow_wakes_for_one_completion() {
    let mut p = boot_source(&collection_source("scrollFollowEnd=true"));
    press(&mut p, "show", 0.);
    p.settle_collections();
    p.wheel(named(&p, "port"), 0., 400.).unwrap();
    p.pump(0.);
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    press(&mut p, "grow", 50.);
    p.settle_collections();
    assert_eq!(number(&p, "ends"), 1.);
    let frame = p.display_frame().unwrap();
    assert!(p.display_complete(&frame));
    assert_eq!(p.scroll_of(named(&p, "port")).1, 500.);
    assert_eq!(p.timer_due_ms(), Some(50.));
    p.pump(50.);
    assert_eq!(number(&p, "ends"), 2.);
}

#[test]
fn settle_includes_a_request_started_by_a_completions_final_command() {
    use exact_runner::{Answer, Dispatch, Outcome, Request, Store, Work};
    #[derive(Default)]
    struct Replies;
    impl DataSource for Replies {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
        fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
            Ok(Answer::Later(Request::continuation(1)))
        }
        fn dispatch(&mut self, _: u64, _: &Store) -> Dispatch {
            Dispatch::Run(Work::Later(Box::new(|reply| {
                reply.send(Dispatch::again_outcome())
            })))
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
    let source = r#"component App
  mutation result as shape number
  action ended
    focus("field")
  action focused
    send result = request()
  view
    column
      input id="field" focus=focused height=30
      scroll testId="port" width=200 height=100 scrollend=ended
        box height=1000
"#;
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(source).unwrap().encode(),
        Replies,
        (400., 500.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert_eq!(error, None);
    let port = named(&p, "port");
    let sample = format!(r#"{{"op":"tap","id":{port},"wheel":[0,20],"gesture":true}}"#);
    crate::agent::handle(&mut p, &sample);
    let result: serde_json::Value = serde_json::from_str(&crate::agent::handle(
        &mut p,
        r#"{"op":"clock","settle":true}"#,
    ))
    .unwrap();
    assert_eq!(result["settled"], true, "{result}");
    assert!(!p.pending());
    assert!(p.host.runner().in_flight().is_empty());
}

#[test]
fn safe_area_adjustments_preserve_scroll_completion() {
    for displayed in [false, true] {
        for reading in [false, true] {
            let mut p = boot_source(
                r#"component App
  state ends = 0
  action ended
    ends = ends + 1
  view
    main viewport-fit="cover"
      scroll testId="port" scrollFollowEnd=true width=200 height=100 box-sizing="border-box" padding-bottom="env(safe-area-inset-bottom)" scrollend=ended
        box height=500
"#,
            );
            let window = Window {
                size: (400., 500.),
                ..Window::default()
            };
            assert_eq!(p.set_window(window), None);
            let acknowledge = |p: &mut Presenter<NoData>| {
                if displayed {
                    let frame = p.display_frame().unwrap();
                    assert!(p.display_complete(&frame));
                }
            };
            acknowledge(&mut p);
            let port = named(&p, "port");
            assert_eq!(p.scroll_of(port).1, 400.);
            if reading {
                reader(&mut p, 0., -20.);
                acknowledge(&mut p);
                reader(&mut p, 50., 20.);
                acknowledge(&mut p);
            }
            let before = p.host.kernel().node(port).unwrap().frame;
            assert_eq!(
                p.set_window(Window {
                    bars: [0., 0., 30., 0.],
                    ..window
                }),
                None
            );
            acknowledge(&mut p);
            let after = p.host.kernel().node(port).unwrap().frame;
            assert_eq!((before.width, before.height), (after.width, after.height));
            assert_eq!(p.scroll_of(port).1, 430.);
            assert_eq!(
                p.timer_due_ms(),
                reading.then_some(150.),
                "displayed: {displayed}, reading: {reading}"
            );
            p.advance(149.);
            assert_eq!(number(&p, "ends"), 0.);
            p.advance(150.);
            assert_eq!(number(&p, "ends"), if reading { 1. } else { 0. });
        }
    }
}

#[test]
fn enabling_end_follow_completes_an_established_scroller() {
    for displayed in [false, true] {
        for binding in ["scrollTop=0", ""] {
            let mut p = boot_source(&format!(
                r#"component App
  state follow = false
  state order = ""
  action followEnd
    follow = true
  action moved(x: number, y: number)
    order = `${{order}}s`
  action ended
    order = `${{order}}e`
  view
    column
      button "Follow" testId="follow" press=followEnd
      scroll testId="port" {binding} scrollFollowEnd=follow width=200 height=100 scroll=moved scrollend=ended
        box height=500
"#,
            ));
            let acknowledge = |p: &mut Presenter<NoData>| {
                if displayed {
                    let frame = p.display_frame().unwrap();
                    assert!(p.display_complete(&frame));
                }
            };
            acknowledge(&mut p);
            let port = named(&p, "port");
            assert_eq!(p.scroll_of(port).1, 0.);
            assert_eq!(text(&p, "order"), "");
            press(&mut p, "follow", 50.);
            if displayed {
                assert_eq!(p.scroll_of(port).1, 0.);
                assert_eq!(p.timer_due_ms(), None);
            }
            acknowledge(&mut p);
            assert_eq!(p.scroll_of(port).1, 400.);
            p.pump(50.);
            assert_eq!(
                text(&p, "order"),
                "se",
                "displayed: {displayed}, binding: {binding}"
            );
            assert_eq!(p.timer_due_ms(), None);
        }
    }
}

#[test]
fn an_earlier_scroll_handler_cannot_reach_a_newly_hidden_route() {
    for modal in [false, true] {
        for handles_end in [false, true] {
            let mut p = boot_source(&format!(
                r#"routes nav
  home "/"
    detail "/detail"
component App
  state top = 0
  state order = ""
  action jump
    top = 100
  action a(x: number, y: number)
    order = `${{order}}a`
    nav = push(nav, "/detail")
  action b(x: number, y: number)
    order = `${{order}}b`
  action ended
    order = `${{order}}e`
  view
    main navigationKey=`${{top(nav).id}}` width=300 height=400
      button "Jump" testId="jump" press=jump
      for e in stack(nav) key=e.id
        column navigationKey=`${{e.id}}` navigationPresentation="{}" position="absolute" inset=0
          if e.name == "home"
            scroll testId="a" width=200 height=100 scrollTop=top scroll=a
              box height=1000
            scroll testId="b" width=200 height=100 scrollTop=top scroll=b {}
              box height=1000
          if e.name == "detail"
            text "Covered"
"#,
                if modal { "modal" } else { "push" },
                if handles_end { "scrollend=ended" } else { "" }
            ));
            press(&mut p, "jump", 0.);
            assert!(p.has_pending_scroll(named(&p, "a")));
            assert!(p.has_pending_scroll(named(&p, "b")));
            assert_eq!(p.pump(0.), None);
            assert_eq!(p.host.route_visibility(named(&p, "b")), (!modal, true));
            assert_eq!(
                text(&p, "order"),
                if modal && handles_end { "abe" } else { "a" }
            );
            assert_eq!(p.timer_due_ms(), None);
        }
    }
}

#[test]
fn a_preserved_reader_deadline_waits_through_a_pump_behind_the_shell() {
    let mut p = boot(true);
    reader(&mut p, 50., 20.);
    let port = named(&p, "port");
    p.scroll_end_reader_sampled(port, 5000.);
    assert_eq!(p.reader_scrolled(port, (0., 20.), 0), Some((0., 20.)));
    p.scroll_end_reader_sampled(port, 5100.);
    assert_eq!(p.pump(5100.), None);
    assert_eq!(p.host.now(), 50.);
    assert_eq!(number(&p, "ends"), 0.);
    assert_eq!(p.timer_or_scroll_due_ms(), Some(5100.));
    p.advance(5099.);
    assert_eq!(number(&p, "ends"), 0.);
    p.advance(5100.);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(number(&p, "endedAt"), 5100.);
    assert_eq!(p.timer_or_scroll_due_ms(), Some(5200.));
    p.advance(5200.);
    assert_eq!(number(&p, "ends"), 2.);
    assert_eq!(number(&p, "endedAt"), 5200.);
    assert_eq!(text(&p, "order"), "ssee");
}

#[test]
fn an_older_noop_picture_keeps_the_new_collection_seeks_completion() {
    let mut p = boot_source(
        r#"component App
  state order = ""
  state ends = 0
  action stay
    scrollIntoView("items", 0)
  action jump
    scrollIntoView("items", 4)
  action moved(x: number, y: number)
    order = `${order}s`
  action ended
    ends = ends + 1
    order = `${order}e`
  view
    column
      button "Stay" testId="stay" press=stay
      button "Jump" testId="jump" press=jump
      list id="items" virtualized=true testId="port" width=200 height=100 scroll=moved scrollend=ended
        for x in [0, 1, 2, 3, 4] key=x
          box height=100
"#,
    );
    assert_eq!(p.settle_collections(), None);
    let a = p.display_frame().unwrap();
    assert!(p.display_complete(&a));
    p.pump(0.);
    let port = named(&p, "port");
    press(&mut p, "stay", 0.);
    assert_eq!(p.settle_collections(), None);
    let previous = p.pending_model_scroll(port).unwrap();
    let b = p.display_frame().unwrap();
    press(&mut p, "jump", 0.);
    assert_eq!(p.settle_collections(), None);
    let newer = p.pending_model_scroll(port).unwrap();
    assert_eq!(previous.sequence, newer.sequence);
    assert!(previous != newer);
    assert!(p.display_complete(&b));
    assert_eq!(p.scroll_of(port).1, 0.);
    assert!(p.pending_model_scroll(port) == Some(newer));
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 0.);
    let c = p.display_frame().unwrap();
    assert!(p.display_complete(&c));
    assert_eq!(p.scroll_of(port).1, 400.);
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 1.);
    assert!(text(&p, "order").ends_with("se"));
    assert_eq!(p.timer_due_ms(), None);
}

#[test]
fn an_unacknowledged_jump_keeps_its_completion_when_layout_clamps_its_target() {
    let mut p = boot_source(
        r#"component App
  state height = 1000
  state top = 0
  state ends = 0
  action jump
    top = 200
  action shrink
    height = 150
  action ended
    ends = ends + 1
  view
    column
      button "Jump" testId="jump" press=jump
      button "Shrink" testId="shrink" press=shrink
      scroll testId="port" width=200 height=100 scrollTop=top scrollend=ended
        box height=height
"#,
    );
    let a = p.display_frame().unwrap();
    assert!(p.display_complete(&a));
    press(&mut p, "jump", 0.);
    press(&mut p, "shrink", 0.);
    let port = named(&p, "port");
    assert_eq!(p.scroll_of(port).1, 0.);
    let b = p.display_frame().unwrap();
    assert!(p.display_complete(&b));
    assert_eq!(p.scroll_of(port).1, 50.);
    p.pump(0.);
    assert_eq!(number(&p, "ends"), 1.);
}

#[test]
fn settle_waits_for_the_picture_of_a_completion_handlers_jump() {
    let mut p = boot_source(
        r#"component App
  state top = 0
  state ends = 0
  state order = ""
  action moved(x: number, y: number)
    order = `${order}s`
  action ended
    if ends == 0
      top = 100
    ends = ends + 1
    order = `${order}e`
  view
    scroll testId="port" width=200 height=100 scrollTop=top scroll=moved scrollend=ended
      box height=1000
"#,
    );
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    let port = named(&p, "port");
    p.wheel(port, 0., 20.).unwrap();
    let reply = settled(&mut p);
    assert_eq!(number(&p, "ends"), 1.);
    assert_eq!(p.scroll_of(port).1, 20.);
    assert!(p.pending_model_scroll(port).is_some());
    assert_eq!(reply["settled"], false, "{reply}");
    let next = p.display_frame().unwrap();
    assert!(p.display_complete(&next));
    assert_eq!(p.scroll_of(port).1, 100.);
    let reply = settled(&mut p);
    assert_eq!(reply["settled"], true, "{reply}");
    assert_eq!(number(&p, "ends"), 2.);
    assert_eq!(text(&p, "order"), "sese");
}

#[test]
fn settle_waits_for_a_frame_blocked_scroll() {
    let mut p = boot(true);
    let first = p.display_frame().unwrap();
    assert!(p.display_complete(&first));
    p.set_deferred_collections(true);
    p.wheel(named(&p, "port"), 0., 20.).unwrap();
    let reply = settled(&mut p);
    assert_eq!(reply["settled"], false, "{reply}");
    assert_eq!(text(&p, "order"), "");
    let next = p.display_frame().unwrap();
    assert!(p.display_complete(&next));
    assert!(p.run_deferred_collections());
    assert_eq!(settled(&mut p)["settled"], true);
    assert_eq!(text(&p, "order"), "se");
}

#[test]
fn focusout_returning_to_a_route_cannot_restore_its_retired_completion() {
    for modal in [false, true] {
        let mut p = boot_source(&format!(
            r#"routes nav
  home "/"
    detail "/detail"
component App
  state ends = 0
  state outs = 0
  action ended
    ends = ends + 1
  action open
    nav = push(nav, "/detail")
  action returned(e: FocusEvent)
    outs = outs + 1
    nav = back(nav)
  view
    main navigationKey=`${{top(nav).id}}` width="100%" height="100%"
      for e in stack(nav) key=e.id
        column navigationKey=`${{e.id}}` navigationPresentation="{}" position="absolute" inset=0
          if e.name == "home"
            button "Open" id="open" testId="open" press=open focusout=returned
            scroll testId="port" width=200 height=100 scrollend=ended
              box height=2000
          if e.name == "detail"
            text "Covered"
"#,
            if modal { "modal" } else { "push" }
        ));
        reader(&mut p, 1000., 20.);
        p.clock(1050.);
        p.tap(named(&p, "open")).unwrap();
        assert_eq!(number(&p, "outs"), 1.);
        assert_eq!(p.host.route_visibility(named(&p, "port")), (false, false));
        assert_eq!(p.timer_due_ms(), if modal { Some(1100.) } else { None });
        p.clock(1100.);
        assert_eq!(number(&p, "ends"), if modal { 1. } else { 0. });
    }
}
