use super::*;
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::DataError;

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn source() -> String {
    r#"component App
  state count = 0
  state target = 0
  state showing = true
  action reply
    count = count + 1
    target = 180
  action hide
    showing = false
  action show
    showing = true
  view
    column width=400 height=500
      when showing
        box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" opacity=(target == 0 ? 1 : 0.25) transition="translate spring(300, 30, 1)"
          box testId="indicator" swipeIndicator=true opacity=0 scale=0 transition="opacity 100ms ease-out, scale 100ms ease-out" width=10 height=10
          text "swipe me" testId="label"
      button press=hide testId="hide"
        text "hide"
      button press=show testId="show"
        text "show"
      text `${count}` testId="count"
"#.into()
}
fn boot(src: &str) -> Presenter<NoData> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(src).unwrap().encode(),
        NoData,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id(p: &Presenter<NoData>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}
fn count(p: &Presenter<NoData>) -> &str {
    p.host
        .kernel()
        .node(id(p, "count"))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}
fn recognize(p: &mut Presenter<NoData>) {
    assert!(p.pointer_down(20., 40., 0.).unwrap());
    assert!(p.pointer_move(30., 40., 10.).unwrap());
}

#[test]
fn pointer_release_applies_final_sample_then_action_while_held_then_releases_pin() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    recognize(&mut p);
    assert_eq!(
        p.host.presented(row).translate.0,
        0.,
        "recognition starts at the sampled origin"
    );
    assert!(p.collection_interaction().is_some());
    // No intermediate move: release itself must apply the 70px displacement.
    assert!(p.pointer_up(100., 40., 50.).unwrap());
    assert_eq!(count(&p), "1");
    assert!((p.host.presented(row).translate.0 - 65.2).abs() < 0.001);
    assert!(!p.host.engine().is_held(
        exact_kernel::motion::motion_node(p.host.kernel().node(row).unwrap().key),
        Property::Translate
    ));
    assert_eq!(p.collection_interaction(), None);
    assert!(!p.pointer_up(100., 40., 60.).unwrap());
    p.tick(10_000.);
    assert_eq!(p.host.presented(row).translate.0, 0.);
    assert_eq!(p.host.presented(row).opacity, 0.25);
}

#[test]
fn pointer_catches_return_at_recognition_and_zero_delta_never_jumps() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    let first = p
        .host
        .hold_begin(row, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    p.host
        .hold_update(first.token, Value::new(100., 0.), 0.)
        .unwrap();
    p.host.hold_end(first.token, HoldEnd::Cancel, 0.).unwrap();
    p.tick(20.);
    // Hit the moving row, then advance the return before recognition.
    let x = p.host.presented(row).translate.0 + 20.;
    p.pointer_down(x, 40., 20.).unwrap();
    p.tick(40.);
    let caught = p.host.presented(row).translate.0;
    assert!(p.pointer_move(x + 10., 40., 40.).unwrap());
    assert!((p.host.presented(row).translate.0 - caught).abs() < 0.0001);
    assert!(
        !p.host.motion(),
        "holding alone must not schedule animation"
    );
    p.pointer_move(x + 10., 40., 60.).unwrap();
    assert!((p.host.presented(row).translate.0 - caught).abs() < 0.0001);
    p.pointer_cancel(60.).unwrap();
    assert_eq!(count(&p), "0");
}

#[test]
fn display_cancel_and_wheel_win_without_dispatching_reply() {
    let mut p = boot(&source());
    recognize(&mut p);
    p.pointer_move(150., 40., 30.).unwrap();
    p.pointer_cancel(40.).unwrap();
    assert_eq!(p.collection_interaction(), None);
    assert_eq!(count(&p), "0");
    p.tick(10_000.);
    p.pointer_down(20., 40., 10_000.).unwrap();
    p.pointer_move(30., 40., 10_010.).unwrap();
    p.pointer_move(150., 40., 10_030.).unwrap();
    p.wheel_at(20., 40., 0., 40.);
    assert!(!p.pointer_up(150., 40., 10_050.).unwrap());
    assert_eq!(p.collection_interaction(), None);
    assert_eq!(count(&p), "0");
}

#[test]
fn vertical_motion_and_auto_touch_action_never_take_over() {
    for src in [source(), source().replace("touch-action=\"pan-y\"", "")] {
        let mut p = boot(&src);
        let row = id(&p, "row");
        p.pointer_down(20., 40., 0.).unwrap();
        assert!(!p.pointer_move(21., 60., 10.).unwrap());
        assert!(!p.pointer_move(130., 60., 20.).unwrap());
        p.pointer_up(130., 60., 30.).unwrap();
        assert_eq!(p.host.presented(row).translate.0, 0.);
        assert_eq!(count(&p), "0");
    }
    let mut p = boot(&source().replace("touch-action=\"pan-y\"", ""));
    p.pointer_down(20., 40., 0.).unwrap();
    assert!(!p.pointer_move(140., 40., 10.).unwrap());
}

#[test]
fn deletion_remount_and_runtime_replacement_cannot_receive_old_release() {
    let src = source();
    let mut p = boot(&src);
    recognize(&mut p);
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    p.tap(id(&p, "hide")).unwrap();
    assert!(!p.host.has_hold(token));
    assert_eq!(p.collection_interaction(), None);
    p.tap(id(&p, "show")).unwrap();
    assert!(!p.pointer_up(150., 40., 100.).unwrap());
    assert_eq!(count(&p), "0");
    p.pointer_down(20., 40., 100.).unwrap();
    p.pointer_move(30., 40., 110.).unwrap();
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    p.reload(&contract::compile(&src).unwrap().encode(), NoData)
        .unwrap();
    assert!(!p.host.hold_update(token, Value::ZERO, f64::NAN).unwrap());
    assert!(!p.pointer_up(150., 40., 200.).unwrap());
    assert_eq!(count(&p), "0");
}

#[test]
fn action_deletion_and_ordinary_click_release_clean_up_once() {
    let mut p = boot(&source().replace("swiperight=reply", "swiperight=hide"));
    recognize(&mut p);
    p.pointer_up(150., 40., 50.).unwrap();
    assert!(p.host.kernel().find_by_test_id("row").is_empty());
    assert_eq!(p.collection_interaction(), None);
    assert!(!p.host.motion());
    let mut p = boot(&source().replace("swiperight=reply", "press=reply"));
    p.pointer_down(20., 40., 0.).unwrap();
    p.pointer_up(20., 40., 10.).unwrap();
    assert_eq!(count(&p), "1");
    assert_eq!(p.collection_interaction(), None);
}

#[test]
fn companion_holds_and_replaced_primary_are_bounded_and_do_not_release_successor() {
    let mut p = boot(&source());
    recognize(&mut p);
    let row = id(&p, "row");
    let indicator = id(&p, "indicator");
    p.pointer_move(110., 40., 30.).unwrap();
    assert_eq!(p.host.presented(indicator).opacity, 1.);
    let newer = p
        .host
        .hold_begin(row, Property::Translate, 30.)
        .unwrap()
        .unwrap();
    assert!(!p.pointer_up(110., 40., 40.).unwrap());
    assert!(p.host.has_hold(newer.token));
    assert_eq!(count(&p), "0");
    assert!(!p.host.engine().is_held(
        exact_kernel::motion::motion_node(p.host.kernel().node(indicator).unwrap().key),
        Property::Opacity
    ));
    assert_eq!(p.collection_interaction(), None);
}

#[test]
fn agent_contact_down_without_target_refuses_without_contact() {
    let mut p = boot(&source());
    let reply = crate::agent::handle(
        &mut p,
        "{\"op\":\"tap\",\"phase\":\"down\",\"x\":20,\"y\":40}",
    );
    assert!(reply.contains("error"));
    assert!(p.contact.is_none());
}

#[test]
fn leftward_catch_and_reverse_use_displayed_completion_threshold() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    let start = p
        .host
        .hold_begin(row, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    p.host
        .hold_update(start.token, Value::new(120., 0.), 0.)
        .unwrap();
    p.host.hold_end(start.token, HoldEnd::Cancel, 0.).unwrap();
    p.tick(10.);
    let x = p.host.presented(row).translate.0 + 100.;
    p.pointer_down(x, 40., 10.).unwrap();
    p.tick(20.);
    let caught = p.host.presented(row).translate.0;
    assert!(
        p.pointer_move(x - 10., 40., 20.).unwrap(),
        "catch directly leftward"
    );
    assert!((p.host.presented(row).translate.0 - caught).abs() < 0.0001);
    // Reverse far enough to close the caught displacement: no false reply.
    p.pointer_up(x - 410., 40., 60.).unwrap();
    assert_eq!(count(&p), "0");
    p.tick(10_000.);
    // Conversely, small displacement can complete when caught presentation
    // remains past the displayed threshold; no raw pointer-down distance gate.
    let start = p
        .host
        .hold_begin(row, Property::Translate, 10_000.)
        .unwrap()
        .unwrap();
    p.host
        .hold_update(start.token, Value::new(100., 0.), 10_000.)
        .unwrap();
    p.host
        .hold_end(start.token, HoldEnd::Cancel, 10_000.)
        .unwrap();
    p.pointer_down(150., 40., 10_000.).unwrap();
    assert!(p.pointer_move(140., 40., 10_010.).unwrap());
    p.pointer_up(135., 40., 10_020.).unwrap();
    assert_eq!(count(&p), "1");
}

#[test]
fn resize_keeps_live_hold_origin_pin_and_latest_target() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    recognize(&mut p);
    p.pointer_move(80., 40., 30.).unwrap();
    let before = p.host.presented(row).translate.0;
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    assert!(p.resize(600., 700.).is_none());
    assert!(p.host.has_hold(token));
    assert_eq!(p.collection_interaction(), Some(row));
    p.pointer_move(80., 40., 40.).unwrap();
    assert_eq!(p.host.presented(row).translate.0, before);
    p.pointer_up(110., 40., 60.).unwrap();
    assert_eq!(count(&p), "1");
    assert_eq!(p.collection_interaction(), None);
    p.tick(10_000.);
    assert_eq!(p.host.presented(row).translate.0, 0.);
    assert_eq!(p.host.presented(row).opacity, 0.25);
}

#[test]
fn navigation_cancels_a_held_view_without_waiting_for_the_next_pointer_event() {
    let src = r#"component App
  state selected = "a"
  state count = 0
  action away
    selected = "b"
  action back
    selected = "a"
  action reply
    count = count + 1
  view
    main navigationKey=selected navigationBack="back"
      column navigationKey="a"
        box testId="row" width=400 height=100 swiperight=reply touch-action="pan-y" transition="translate spring(300, 30, 1)"
          text "swipe me"
      column navigationKey="b"
        text "other route"
      button press=away testId="away"
        text "away"
      button press=back testId="back"
        text "back"
      text `${count}` testId="count"
"#;
    let mut p = boot(src);
    recognize(&mut p);
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    p.pointer_move(150., 40., 30.).unwrap();
    assert!(p
        .host
        .dispatch_at(id(&p, "away"), Event::Press, 30.)
        .is_none());
    assert!(p.after_commit().is_none());
    assert!(!p.host.has_hold(token));
    assert!(p.contact.is_none());
    assert_eq!(p.collection_interaction(), None);
    p.host.dispatch_at(id(&p, "back"), Event::Press, 30.);
    p.after_commit();
    assert!(!p.pointer_up(150., 40., 50.).unwrap());
    assert_eq!(count(&p), "0");
}

#[test]
fn displayed_velocity_and_companion_catch_are_continuous_past_resistance_knee() {
    let mut p = boot(&source());
    let row = id(&p, "row");
    let companion = id(&p, "indicator");
    for (view, property, value) in [
        (row, Property::Translate, Value::new(80., 0.)),
        (companion, Property::Opacity, Value::scalar(0.4)),
        (companion, Property::Scale, Value::scalar(0.4)),
    ] {
        let token = p
            .host
            .hold_begin(view, property, 0.)
            .unwrap()
            .unwrap()
            .token;
        p.host.hold_update(token, value, 0.).unwrap();
        p.host.hold_end(token, HoldEnd::Cancel, 0.).unwrap();
    }
    p.tick(10.);
    let x = p.host.presented(row).translate.0 + 20.;
    let before = p.host.presented(companion);
    p.pointer_down(x, 40., 10.).unwrap();
    p.pointer_move(x + 10., 40., 10.).unwrap();
    assert_eq!(p.host.presented(companion).opacity, before.opacity);
    assert_eq!(p.host.presented(companion).scale, before.scale);
    p.pointer_move(x + 30., 40., 30.).unwrap();
    let held = p.contact.as_ref().unwrap().hold.as_ref().unwrap();
    let velocity = held.velocity.estimate(0.03).x;
    assert!(
        (velocity - 200.).abs() < 0.01,
        "displayed velocity={velocity}, not raw1000px/s"
    );
    p.pointer_cancel(30.).unwrap();
}

#[test]
fn malformed_pointer_samples_preserve_contact_and_presentation() {
    let mut p = boot(&source());
    recognize(&mut p);
    let row = id(&p, "row");
    let token = p
        .contact
        .as_ref()
        .unwrap()
        .hold
        .as_ref()
        .unwrap()
        .primary
        .token;
    for (x, time) in [(f32::NAN, 100.), (80., 9.), (80., f64::INFINITY)] {
        assert!(p.pointer_move(x, 40., time).is_err());
        assert!(p.host.has_hold(token));
        assert_eq!(p.host.now(), 10.);
        assert_eq!(p.host.presented(row).translate.0, 0.);
    }
    p.pointer_cancel(10.).unwrap();
}

// SOURCE-ONLY baseline for the retained Messages action adapter. Uses the real
// region worker, CPU backend and submitted/acknowledged Presenter ownership.
pub(super) mod retained_actions {
    use super::*;
    use crate::content_region::ContentRegionRegistration;
    use crate::presenter::display_frame::SubmittedFrame;
    use exact_runner::Value as DataValue;
    use std::time::Instant;

    #[derive(Default)]
    pub(crate) struct Rows {
        pub extra_lengths: Vec<usize>,
    }
    impl DataSource for Rows {
        fn query(&mut self, source: &str, args: &[DataValue]) -> Result<DataValue, DataError> {
            if source == "extras" {
                return Ok(DataValue::list(
                    self.extra_lengths
                        .iter()
                        .enumerate()
                        .map(|(i, &len)| {
                            assert!(len >= 4);
                            DataValue::str(&format!("{i:04}{}", "x".repeat(len - 4)))
                        })
                        .collect(),
                ));
            }
            let [DataValue::Number(revision), DataValue::Bool(changed)] = args else {
                panic!("unexpected rows arguments {source}: {args:?}");
            };
            assert_eq!(source, "rows");
            Ok(DataValue::list((0..2).map(|i| DataValue::record(vec![
                DataValue::str(&format!("stable-{i}")),
                DataValue::str(&format!("{}-{i}", if *changed { "changed" } else { "message" })),
                DataValue::str("Sender"),
                DataValue::str(&format!("Revision {revision} row {i}: complete accepted paragraph with wrapping and decoration")),
                DataValue::Bool(i == 1), DataValue::str("now"),
            ])).collect()))
        }
    }
    pub(crate) fn source() -> String {
        let bubble = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/messages-stress/app.contract"
        ))
        .split_once("component MessageBubble\n")
        .unwrap()
        .1;
        let bubble = bubble.replace(
            "text message.body font-size=14",
            "text message.body testId=`body-${message.id}` font-size=14",
        );
        format!(
            r##"shape Message
  rowKey: string
  id: string
  sender: string
  body: string
  outgoing: bool
  meta: string
component App
  state revision = 0
  state changed = false
  state disabled = false
  state showing = true
  state replying = ""
  state draft = ""
  resource rows = rows(revision, changed) as shape list<Message>
  action body
    revision = revision + 1
  action curry
    changed = true
  action disable
    disabled = true
  action hide
    showing = false
  action replyTo(id: string)
    replying = id
  action outside
    replying = "outside"
  action edit(value)
    draft = value
  view
    column width=400 height=500 padding-left=5.3 padding-top=3.7 box-sizing="border-box"
      row height=24
        button press=body testId="body-change"
          text "Body"
        button press=curry testId="curry-change"
          text "Curry"
        button press=disable testId="disable"
          text "Disable"
        button press=hide testId="hide"
          text "Hide"
        button press=outside testId="outside"
          text "Outside"
      input value=draft input=edit testId="composer" height=32
      text replying testId="replying" height=20
      view id="owner" width=360.25 height=200.5 overflow-x="hidden" overflow-y="hidden"
        list id="content" testId="transcript" virtualized=true width=360.25 height=200.5 padding-left=1.3 border-width=0.7 border-style="solid" box-sizing="border-box"
          each m in rows key=m.rowKey
            column width="100%" padding=5.3 disabled=disabled display=(showing ? "flex" : "none")
              box testId=`reply-hit-${{m.id}}` swiperight=replyTo(m.id) touch-action="pan-y" retainFocus=true transition="translate spring(300, 30, 1)" width="100%" padding=0 border-width=0 background-color="#00000000" text-align="left"
                MessageBubble(message=m)
              button press=replyTo(m.id) testId=`reply-${{m.id}}` padding=4 border-width=0 background-color="#00000000" align-self="flex-end"
                text "Reply" font-size=11 color="#2668d8"
        text "Preparing" id="pending" position="absolute"
component MessageBubble
{bubble}"##
        )
    }
    pub(crate) fn id(p: &Presenter<Rows>, name: &str) -> ViewId {
        let keys = p.host.kernel().find_by_test_id(name);
        assert_eq!(keys.len(), 1, "{name}");
        p.host.kernel().node_by_key(keys[0]).unwrap().id
    }
    pub(crate) fn boot(source: &str, region: bool, data: Rows) -> Presenter<Rows> {
        let plan = contract::compile(source).unwrap().encode();
        let assets = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain"));
        let (mut p, error) = if region {
            Presenter::boot_with_content_region(
                &plan,
                data,
                (400., 500.),
                1.,
                assets,
                PainterChoice::Cpu,
                ContentRegionRegistration {
                    activate: None,
                    owner: "owner",
                    content: "content",
                    pending: "pending",
                },
            )
            .unwrap()
        } else {
            Presenter::boot_with(&plan, data, (400., 500.), 1., assets, PainterChoice::Cpu).unwrap()
        };
        assert!(error.is_none(), "{error:?}");
        ack(&mut p);
        if region {
            ready(&mut p);
        }
        // Same existing collection feedback loop for ordinary and retained A.
        for _ in 0..16 {
            assert!(p.pump(p.host.now()).is_none());
            if region {
                ready(&mut p);
            }
            if p.dirty() {
                ack(&mut p);
            }
        }
        assert!(!p.dirty());
        p
    }
    pub(crate) fn ready(p: &mut Presenter<Rows>) {
        let end = Instant::now() + Duration::from_secs(90);
        while !p.host.content_region().unwrap().receipt().unwrap().current {
            assert!(Instant::now() < end, "existing real worker watchdog");
            assert!(p.host.content_region().unwrap().refusal().is_none());
            std::thread::sleep(Duration::from_millis(1));
            let error = p.poll_content_region();
            assert!(error.is_none(), "{error:?}");
        }
    }
    pub(crate) fn ack(p: &mut Presenter<Rows>) -> SubmittedFrame {
        let frame = p.display_frame().expect("only one submission");
        assert!(p.last_frame_succeeded, "actual backend paint");
        assert!(p.display_complete(&frame));
        frame
    }
    pub(crate) fn point(p: &mut Presenter<Rows>, view: ViewId) -> (f32, f32) {
        let b = p.box_of(view).unwrap();
        let rect = b.clip.map_or(b.rect, |c| {
            let x = b.rect.0.max(c.0);
            let y = b.rect.1.max(c.1);
            (
                x,
                y,
                (b.rect.0 + b.rect.2).min(c.0 + c.2) - x,
                (b.rect.1 + b.rect.3).min(c.1 + c.3) - y,
            )
        });
        assert!(rect.2 > 4. && rect.3 > 4.);
        (rect.0 + rect.2 / 2., rect.1 + rect.3 / 2.)
    }
    pub(crate) fn action(p: &mut Presenter<Rows>, control: &str, at: f64) {
        assert!(p
            .host
            .dispatch_at(id(p, control), Event::Press, at)
            .is_none());
        assert!(p.after_commit().is_none());
    }
    pub(crate) fn selected(p: &Presenter<Rows>) -> DataValue {
        p.host.runner().slot("replying").unwrap().clone()
    }
    pub(crate) fn clocks(p: &Presenter<Rows>) -> (u64, u64, usize) {
        (
            p.host.now().to_bits(),
            p.host.runner().now_ms().to_bits(),
            p.host.runner().journal().count(),
        )
    }
    pub(crate) fn block_body(
        p: &mut Presenter<Rows>,
        at: f64,
    ) -> crate::content_region::test_hooks::Pause {
        let gate = crate::content_region::test_hooks::next_text();
        action(p, "body-change", at);
        gate.entered();
        assert_eq!(p.host.content_region().unwrap().work_counts().0, 1);
        gate
    }
    fn body_key(p: &Presenter<Rows>) -> exact_kernel::NodeKey {
        p.host.kernel().node(id(p, "body-message-0")).unwrap().key
    }

    #[test]
    fn retained_actions_swipe_moves_complete_a_pixels_and_hits_while_b_is_parked() {
        let _service = crate::content_region::test_service();
        let src = source();
        let mut p = boot(&src, true, Rows::default());
        let mut ordinary = boot(&src, false, Rows::default());
        let a = ack(&mut p);
        let oracle_a = ack(&mut ordinary);
        assert_eq!(a.pixels.data(), oracle_a.pixels.data());
        let row = id(&p, "reply-hit-message-0");
        let oracle_row = id(&ordinary, "reply-hit-message-0");
        let key = body_key(&p);
        let stamp = p
            .host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp()
            .clone();
        let start = point(&mut p, row);
        let _gate = block_body(&mut p, 1.);
        for q in [&mut p, &mut ordinary] {
            assert!(
                q.pointer_down(start.0, start.1, 20.).unwrap(),
                "actual retained DOWN must be admitted"
            );
            assert!(q.pointer_move(start.0 + 10., start.1, 30.).unwrap());
            assert!(q.pointer_move(start.0 + 80., start.1, 40.).unwrap());
            assert!(q.collection_interaction().is_some());
        }
        let held = ack(&mut p);
        let expected = ack(&mut ordinary);
        assert_ne!(
            held.pixels.data(),
            a.pixels.data(),
            "gesture must move pixels"
        );
        assert_eq!(
            held.pixels.data(),
            expected.pixels.data(),
            "full decorated A, ordinary transform order"
        );
        assert_eq!(
            p.box_of(row).unwrap().rect,
            ordinary.box_of(oracle_row).unwrap().rect
        );
        let hit = p.hit(start.0 + 70., start.1).unwrap();
        let hit = p.host.kernel().node(hit).unwrap().key;
        assert_eq!(
            p.swipe_candidate(hit),
            Some(p.host.kernel().node(row).unwrap().key)
        );
        assert_eq!(
            p.host
                .content_region()
                .unwrap()
                .text_snapshot(key)
                .unwrap()
                .request
                .stamp(),
            &stamp
        );
        let pending = p.display_frame().unwrap();
        let frozen = pending.pixels.data().to_vec();
        for (arm, q) in [("retained", &mut p), ("ordinary", &mut ordinary)] {
            assert!(
                q.pointer_up(start.0 + 80., start.1, 50.).unwrap(),
                "UP arm {arm}"
            );
            assert_eq!(selected(q), DataValue::str("message-0"));
            assert_eq!(q.collection_interaction(), None);
            assert!(!q.pointer_up(start.0 + 80., start.1, 60.).unwrap());
        }
        assert_eq!(pending.pixels.data(), frozen);
        assert!(p.display_complete(&pending));
        p.tick(5000.);
        ordinary.tick(5000.);
        assert_eq!(ack(&mut p).pixels.data(), ack(&mut ordinary).pixels.data());
        assert_eq!(p.host.presented(row).translate.0, 0.);
        assert_eq!(
            p.host
                .content_region()
                .unwrap()
                .text_snapshot(key)
                .unwrap()
                .request
                .stamp(),
            &stamp
        );
    }

    #[test]
    fn retained_actions_changed_curry_cancels_owned_hold_without_consuming_rejected_time() {
        let _service = crate::content_region::test_service();
        let mut p = boot(&source(), true, Rows::default());
        let row = id(&p, "reply-hit-message-0");
        let at = point(&mut p, row);
        assert!(p.pointer_down(at.0, at.1, 10.).unwrap());
        assert!(p.pointer_move(at.0 + 10., at.1, 20.).unwrap());
        let token = p
            .contact
            .as_ref()
            .unwrap()
            .hold
            .as_ref()
            .unwrap()
            .primary
            .token;
        let _gate = block_body(&mut p, 21.);
        action(&mut p, "curry-change", 22.);
        let before = clocks(&p);
        assert!(!p.pointer_up(at.0 + 90., at.1, 100.).unwrap());
        assert_eq!(
            clocks(&p),
            before,
            "invalid final sample must not advance Host or dispatch"
        );
        assert_eq!(selected(&p), DataValue::str(""));
        assert!(!p.host.has_hold(token));
        assert!(p.contact.is_none());
        assert_eq!(p.collection_interaction(), None);
    }

    fn budget_source() -> &'static str {
        r##"component App
  state replying = ""
  resource extras = extras() as shape list<string>
  action replyTo(id: string)
    replying = id
  view
    view id="owner" position="relative" width=400 height=500 overflow-x="hidden" overflow-y="hidden"
      scroll id="content" width=400 height=500
        column
          text "one complete paragraph"
          each value in extras key=value
            box width=1 height=1 press=replyTo(value)
      text "Preparing" id="pending"
"##
    }
    fn budget_frame(lengths: Vec<usize>) -> bool {
        crate::content_region::test_wait_idle();
        let plan = contract::compile(budget_source()).unwrap().encode();
        let (mut p, error) = Presenter::boot_with_content_region(
            &plan,
            Rows {
                extra_lengths: lengths,
            },
            (400., 500.),
            1.,
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
            PainterChoice::Cpu,
            ContentRegionRegistration {
                activate: None,
                owner: "owner",
                content: "content",
                pending: "pending",
            },
        )
        .unwrap();
        assert!(error.is_none());
        ack(&mut p);
        ready(&mut p);
        let frame = p.display_frame().unwrap();
        let success = p.last_frame_succeeded;
        assert!(p.display_complete(&frame));
        success
    }
    #[test]
    fn retained_actions_whole_picture_event_budget_is_256_not_selective_admission() {
        let _service = crate::content_region::test_service();
        assert!(budget_frame(vec![8; 256]));
        assert!(
            !budget_frame(vec![8; 257]),
            "overflow must refuse the WHOLE picture"
        );
    }
    #[test]
    fn retained_actions_whole_picture_utf8_budget_is_65536_not_per_binding_only() {
        let _service = crate::content_region::test_service();
        let mut at = vec![1009; 64];
        at.push(960);
        assert_eq!(at.iter().sum::<usize>(), 65536);
        assert!(budget_frame(at.clone()));
        *at.last_mut().unwrap() += 1;
        assert!(
            !budget_frame(at),
            "all65 args individually fit1024; aggregate must refuse"
        );
    }

    #[test]
    fn retained_actions_binding_utf8_accounting_counts_bytes_not_scalars_or_source() {
        let source = r#"component App
  state selected = ""
  action choose(id: string, ok: bool, n: number)
    selected = id
  action clear
    selected = ""
  view
    column
      button testId="mixed" press=choose("éλ🙂", true, 7)
        text "captured"
      button testId="empty" press=clear
        text "clear"
"#;
        let p = boot(source, false, Rows::default());
        for (name, expected) in [("mixed", 8), ("empty", 0)] {
            let key = p.host.kernel().node(id(&p, name)).unwrap().key;
            let binding = p
                .host
                .runner()
                .capture_action_binding(key, EventKind::Press)
                .unwrap();
            assert_eq!(binding.retained_utf8_bytes(), expected);
            assert_eq!(
                binding.retained_utf8_bytes(),
                expected,
                "read-only, repeatable"
            );
        }
    }

    #[test]
    fn retained_actions_unsupported_child_is_a_barrier_not_parent_fallthrough() {
        let _service = crate::content_region::test_service();
        let src = source()
            .replace(
                "  action outside",
                "  action unsupported\n    replying = replying\n  action outside",
            )
            .replace(
                "column width=\"100%\" padding=5.3",
                "column press=outside width=\"100%\" padding=5.3",
            )
            .replace("button press=replyTo(m.id)", "button press=unsupported");
        let mut p = boot(&src, true, Rows::default());
        let child = id(&p, "reply-message-0");
        let at = point(&mut p, child);
        let key = p.host.kernel().node(child).unwrap().key;
        assert!(matches!(
            p.host
                .runner()
                .capture_action_binding(key, EventKind::Press),
            Err(exact_runner::runner::ActionBindingRefusal::Unsupported)
        ));
        let _gate = block_body(&mut p, 1.);
        let before = clocks(&p);
        assert_eq!(p.press_at(at.0, at.1, 100.), None);
        assert_eq!(clocks(&p), before);
        assert_eq!(
            selected(&p),
            DataValue::str(""),
            "must not fall through to outside"
        );
    }

    #[test]
    fn retained_actions_owned_cancel_reload_refusal_and_foreign_ack_leave_no_contact() {
        let _service = crate::content_region::test_service();
        let src = source();
        let mut p = boot(&src, true, Rows::default());
        let row = id(&p, "reply-hit-message-0");
        let at = point(&mut p, row);
        assert!(p.pointer_down(at.0, at.1, 10.).unwrap());
        assert!(p.pointer_move(at.0 + 10., at.1, 20.).unwrap());
        let token = p
            .contact
            .as_ref()
            .unwrap()
            .hold
            .as_ref()
            .unwrap()
            .primary
            .token;
        let before = clocks(&p);
        // Existing region sessions explicitly refuse in-place reload; do not
        // invent a carrier policy or weaken that refusal for this adapter.
        assert!(p
            .reload(&contract::compile(&src).unwrap().encode(), Rows::default())
            .is_err());
        assert_eq!(clocks(&p), before);
        p.pointer_cancel(20.).unwrap();
        assert!(!p.host.has_hold(token));
        assert!(p.contact.is_none());
        assert_eq!(p.collection_interaction(), None);
        assert_eq!(selected(&p), DataValue::str(""));
        let old = p.display_frame().unwrap();
        let pixels = old.pixels.data().to_vec();
        drop(p);
        crate::content_region::test_wait_idle();
        let mut replacement = boot(&src, true, Rows::default());
        let before = clocks(&replacement);
        assert!(
            !replacement.display_complete(&old),
            "different Presenter/Runner origin"
        );
        assert_eq!(clocks(&replacement), before);
        assert!(!replacement.host.has_hold(token));
        assert!(!replacement.pointer_up(at.0, at.1, 100.).unwrap());
        assert_eq!(selected(&replacement), DataValue::str(""));
        assert_eq!(old.pixels.data(), pixels);
    }

    #[test]
    fn retained_actions_recycled_row_key_cannot_borrow_old_coordinates_or_binding() {
        let _service = crate::content_region::test_service();
        let src = source().replace("  action hide", "  action show\n    showing = true\n  action hide")
            .replace("      input value=draft", "      button press=show testId=\"show\" height=20\n        text \"Show\"\n      input value=draft")
            .replace("  resource rows = rows(revision, changed) as shape list<Message>", "  resource rows = rows(revision, changed) as shape list<Message>\n  resource emptyRows = extras() as shape list<Message>")
            .replace("each m in rows key=m.rowKey", "each m in (showing ? rows : emptyRows) key=m.rowKey");
        // Default Rows has an empty extras list. The typed conditional removes
        // actual keyed rows while keeping the required direct each and handlers.
        let mut p = boot(&src, true, Rows::default());
        let row = id(&p, "reply-message-0");
        let at = point(&mut p, row);
        let old = p.host.kernel().node(row).unwrap().key;
        let gate = block_body(&mut p, 1.);
        action(&mut p, "hide", 2.);
        assert!(p.host.kernel().node_by_key(old).is_none());
        action(&mut p, "show", 3.);
        let new = p.host.kernel().node(id(&p, "reply-message-0")).unwrap().key;
        assert_ne!(old, new);
        assert!(
            p.host
                .kernel()
                .node_by_key(p.host.kernel().arena().key(old.index))
                .is_some(),
            "the old arena slot was actually reused"
        );
        let before = clocks(&p);
        assert_eq!(p.press_at(at.0, at.1, 100.), None);
        assert_eq!(clocks(&p), before);
        assert_eq!(selected(&p), DataValue::str(""));
        drop(gate);
        ready(&mut p);
        ack(&mut p);
        let row = id(&p, "reply-message-0");
        let at = point(&mut p, row);
        assert_eq!(p.press_at(at.0, at.1, 101.), Some(row));
        assert_eq!(selected(&p), DataValue::str("message-0"));
    }

    #[test]
    fn retained_actions_review_distinct_ack_retires_held_a_without_another_turn() {
        let _service = crate::content_region::test_service();
        let mut p = boot(&source(), true, Rows::default());
        let row = id(&p, "reply-hit-message-0");
        let at = point(&mut p, row);
        let a = ack(&mut p);
        action(&mut p, "body-change", 1.);
        ready(&mut p);
        assert_eq!(p.host.presented(row).translate.0, 0.);
        let b = p.display_frame().unwrap();
        assert!(p.last_frame_succeeded);
        let b_pixels = b.pixels.data().to_vec();
        assert_ne!(b_pixels, a.pixels.data());
        assert!(p.pointer_down(at.0, at.1, 20.).unwrap());
        assert!(p.pointer_move(at.0 + 10., at.1, 30.).unwrap());
        assert!(p.pointer_move(at.0 + 80., at.1, 40.).unwrap());
        let token = p
            .contact
            .as_ref()
            .unwrap()
            .hold
            .as_ref()
            .unwrap()
            .primary
            .token;
        assert!(p.host.has_hold(token));
        assert!(p.host.presented(row).translate.0 > 0.);
        assert!(p.collection_interaction().is_some());
        let before = clocks(&p);

        assert!(p.display_complete(&b));
        // No input, pump, timer, frame or commit between ACK and these assertions.
        assert!(
            p.contact.is_none(),
            "distinct B ACK must retire A contact now"
        );
        assert!(
            !p.host.has_hold(token),
            "ACK cannot strand the owned A token"
        );
        assert_eq!(p.collection_interaction(), None);
        assert_eq!(clocks(&p), before, "ACK retirement uses accepted Host time");
        assert_eq!(selected(&p), DataValue::str(""));
        assert_eq!(b.pixels.data(), b_pixels);
    }

    #[test]
    fn retained_actions_review_nonzero_curry_cancel_paints_a_mid_return_and_shell() {
        let _service = crate::content_region::test_service();
        let src = source();
        let mut p = boot(&src, true, Rows::default());
        let mut ordinary = boot(&src, false, Rows::default());
        let row = id(&p, "reply-hit-message-0");
        let oracle_row = id(&ordinary, "reply-hit-message-0");
        let key = body_key(&p);
        let stamp = p
            .host
            .content_region()
            .unwrap()
            .text_snapshot(key)
            .unwrap()
            .request
            .stamp()
            .clone();
        let at = point(&mut p, row);
        let _gate = block_body(&mut p, 1.);
        for q in [&mut p, &mut ordinary] {
            assert!(q.pointer_down(at.0, at.1, 20.).unwrap());
            assert!(q.pointer_move(at.0 + 10., at.1, 30.).unwrap());
            assert!(q.pointer_move(at.0 + 80., at.1, 40.).unwrap());
        }
        let token = p
            .contact
            .as_ref()
            .unwrap()
            .hold
            .as_ref()
            .unwrap()
            .primary
            .token;
        let moved = ack(&mut p);
        assert_eq!(moved.pixels.data(), ack(&mut ordinary).pixels.data());
        let from = p.host.presented(row).translate.0;
        assert!(
            from > 0.,
            "actual nonzero paint, not recognition's zero sample"
        );
        assert!(p.host.has_hold(token));

        action(&mut p, "curry-change", 41.);
        action(&mut ordinary, "curry-change", 41.);
        // Ordinary dispatch has no retained binding policy: explicitly cancel
        // its owned gesture to get the same zero-velocity return geometry.
        ordinary.pointer_cancel(41.).unwrap();
        assert!(!p.host.has_hold(token));
        assert!(p.contact.is_none());
        assert_eq!(p.collection_interaction(), None);
        assert_eq!(selected(&p), DataValue::str(""));
        let before = clocks(&p);
        assert!(!p.pointer_up(at.0 + 80., at.1, 100.).unwrap());
        assert_eq!(
            clocks(&p),
            before,
            "invalid UP may not consume rejected time"
        );
        for q in [&mut p, &mut ordinary] {
            q.type_text(id(q, "composer"), "shell continues during return")
                .unwrap();
            q.tick(61.);
        }
        let mid = p.host.presented(row).translate.0;
        assert!(
            mid > 0. && mid < from,
            "sample must be inside the return curve"
        );
        assert_eq!(mid, ordinary.host.presented(oracle_row).translate.0);
        let expected = ack(&mut ordinary);
        let actual = p.display_frame().unwrap();
        assert!(
            p.last_frame_succeeded,
            "refused dispatch must not refuse A return painting"
        );
        assert!(p.display_complete(&actual));
        assert_eq!(
            actual.pixels.data(),
            expected.pixels.data(),
            "complete A plus live shell"
        );
        assert_ne!(actual.pixels.data(), moved.pixels.data());
        assert_eq!(
            p.box_of(row).unwrap().rect,
            ordinary.box_of(oracle_row).unwrap().rect
        );
        assert_eq!(
            p.host
                .content_region()
                .unwrap()
                .text_snapshot(key)
                .unwrap()
                .request
                .stamp(),
            &stamp
        );
        assert_eq!(p.host.content_region().unwrap().work_counts().0, 1);
        assert_eq!(selected(&p), DataValue::str(""));
    }

    #[test]
    fn retained_actions_review_returning_ack_does_not_freeze_b_or_end_a_replacement() {
        let _service = crate::content_region::test_service();
        for replacement in [false, true] {
            let mut p = boot(&source(), true, Rows::default());
            let row = id(&p, "reply-hit-message-0");
            let at = point(&mut p, row);
            ack(&mut p);
            action(&mut p, "body-change", 1.);
            ready(&mut p);
            let b = p.display_frame().unwrap();
            assert!(p.last_frame_succeeded);
            let b_pixels = b.pixels.data().to_vec();
            assert!(p.pointer_down(at.0, at.1, 20.).unwrap());
            assert!(p.pointer_move(at.0 + 10., at.1, 30.).unwrap());
            assert!(p.pointer_move(at.0 + 80., at.1, 40.).unwrap());
            let token = p
                .contact
                .as_ref()
                .unwrap()
                .hold
                .as_ref()
                .unwrap()
                .primary
                .token;
            p.pointer_cancel(41.).unwrap();
            assert!(p.contact.is_none());
            assert!(!p.host.has_hold(token));
            assert_eq!(p.collection_interaction(), None);
            assert!(p.host.presented(row).translate.0 > 0.);
            let intent = p
                .host
                .engine()
                .spring_descriptor(token.node(), Property::Translate)
                .unwrap();
            if replacement {
                // A different owner can produce the identical descriptor at the
                // same accepted clock. Numeric intent is not mutation authority.
                let foreign = p
                    .host
                    .hold_begin(row, Property::Translate, 41.)
                    .unwrap()
                    .unwrap();
                assert_ne!(foreign.token, token);
                assert!(p
                    .host
                    .hold_end(foreign.token, HoldEnd::Cancel, 41.)
                    .unwrap());
                assert_eq!(
                    p.host
                        .engine()
                        .spring_descriptor(token.node(), Property::Translate),
                    Some(intent)
                );
            }
            let before = clocks(&p);
            assert!(p.display_complete(&b));
            assert_eq!(clocks(&p), before);
            assert_eq!(b.pixels.data(), b_pixels);
            assert_eq!(selected(&p), DataValue::str(""));
            if replacement {
                assert_eq!(
                    p.host
                        .engine()
                        .spring_descriptor(token.node(), Property::Translate),
                    Some(intent),
                    "ACK has no authority to remove an identical replacement return"
                );
            } else {
                assert!(
                    !p.host.engine().is_active(token.node(), Property::Translate),
                    "ACK must finish its owned A return before B's next paint"
                );
                let next = p.display_frame().unwrap();
                assert!(
                    p.last_frame_succeeded,
                    "B must not freeze behind A's retired return"
                );
                assert!(p.display_complete(&next));
                assert_eq!(p.host.presented(row).translate.0, 0.);
            }
        }
    }
}
