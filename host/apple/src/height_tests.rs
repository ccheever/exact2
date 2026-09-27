//! Explicit single-panel numeric CSS height goes through real host layout.
use super::{Batch, HeightOwnerDisposition, HeightOwnerError, Host};
use exact_kernel::motion::motion_node;
use exact_kernel::{MonospaceMeasurer, NodeKey};
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::{DataError, DataSource, Event};

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        if name == "rows" {
            return Ok(exact_runner::Value::List(
                (0..100)
                    .map(|n| exact_runner::Value::Number(n as f64))
                    .collect::<Vec<_>>()
                    .into(),
            ));
        }
        Err(DataError::UnknownSource(name.into()))
    }
}
fn fixture(extra: &str, _unsupported: &str) -> Host<NoData> {
    fixture_measurer(extra, Box::new(MonospaceMeasurer::default()))
}
fn fixture_measurer(extra: &str, measurer: Box<dyn exact_kernel::TextMeasurer>) -> Host<NoData> {
    let source = format!(
        r#"component App
  state size = 120
  state eligible = true
  state hidden = false
  state counter = 0
  action grow writes size
    size = 300
  action unsupported writes eligible
    eligible = false
  action hide writes hidden
    hidden = true
  action show writes hidden
    hidden = false
  action unrelated writes counter
    counter = counter + 1
  view
    column testId="root" width="100%" height="100%"
      button testId="grow" press=grow
        text "grow"
      button testId="unsupported" press=unsupported
        text "unsupported"
      button testId="hide" press=hide
        text "hide"
      button testId="show" press=show
        text "show"
      button testId="unrelated" press=unrelated
        text `${{counter}}`
      column testId="ancestor" display=(hidden ? "none" : "flex")
        column testId="panel" box-sizing="border-box" height=size {extra} transition="height 1000ms linear, opacity 1000ms linear, translate 1000ms linear"
          scroll testId="port" flex=1 min-height=0 width="100%"
            text "child" height=600
      box testId="other" box-sizing="border-box" height=60
      box testId="contentbox" height=40
      image "fixture.png" testId="image"
      box height=1 padding-top="env(safe-area-inset-top)"
"#
    );
    let plan = contract::compile(&source).unwrap().encode();
    Host::boot(&plan, NoData, measurer, 400., 800.).unwrap().0
}
fn key(h: &Host<NoData>, id: &str) -> NodeKey {
    h.runner().kernel().find_by_test_id(id)[0]
}
fn view(h: &Host<NoData>, id: &str) -> u32 {
    h.runner().kernel().node_by_key(key(h, id)).unwrap().id
}
fn height(h: &Host<NoData>, id: &str) -> f32 {
    h.runner()
        .kernel()
        .node_by_key(key(h, id))
        .unwrap()
        .frame
        .height
}
fn press(h: &mut Host<NoData>, id: &str) -> String {
    h.dispatch_at(view(h, id), Event::Press, h.engine().now() * 1000.)
}
fn token(batch: &str) -> u64 {
    batch
        .split("\"token\":\"")
        .nth(1)
        .expect(batch)
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn good(batch: &str) {
    assert!(batch.contains("\"error\":null"), "{batch}");
}

#[test]
fn explicit_owner_adopts_only_one_height_and_registration_is_idempotent() {
    let mut h = fixture("", "\"auto\"");
    let panel = view(&h, "panel");
    let node = key(&h, "panel");
    assert_eq!(h.engine().target(motion_node(node), Property::Height), None);
    let result = h.set_height_owner(Some(panel)).unwrap();
    good(&result.batch);
    assert_eq!(
        result.disposition,
        HeightOwnerDisposition::Registered { node }
    );
    assert_eq!(h.height_owner(), Some(node));
    assert_eq!(
        h.engine().target(motion_node(node), Property::Height),
        Some(Value::scalar(120.))
    );
    assert_eq!(
        h.engine()
            .target(motion_node(key(&h, "other")), Property::Height),
        None
    );
    assert_eq!(
        h.set_height_owner(Some(panel)).unwrap().disposition,
        HeightOwnerDisposition::Unchanged
    );
}

#[test]
fn target_sync_precedes_layout_and_height_never_emits_compositor_present() {
    let mut h = fixture("", "\"auto\"");
    h.set_height_owner(Some(view(&h, "panel"))).unwrap();
    let batch = press(&mut h, "grow");
    good(&batch);
    assert_eq!(height(&h, "panel"), 120.);
    assert!(!batch.contains("\"property\":\"height\""));
    let epoch = h.runner().kernel().epoch();
    good(&h.tick(500.));
    assert_eq!(height(&h, "panel"), 210.);
    assert_eq!(height(&h, "port"), 210.);
    assert_eq!(h.runner().kernel().epoch(), epoch);
    good(&h.tick(1000.));
    assert_eq!(height(&h, "panel"), 300.);
}

#[test]
fn held_projection_survives_unrelated_commit_resize_intrinsic_and_environment() {
    let mut h = fixture("", "\"auto\"");
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    good(&h.hold_update(hold, Value::scalar(180.), 0.));
    good(&press(&mut h, "grow"));
    assert_eq!(height(&h, "panel"), 180.);
    good(&press(&mut h, "unrelated"));
    assert_eq!(height(&h, "panel"), 180.);
    good(&h.resize(500., 900.));
    assert_eq!(height(&h, "panel"), 180.);
    good(&h.set_intrinsic(view(&h, "image"), Some((80., 20.))));
    assert_eq!(height(&h, "panel"), 180.);
    good(&h.set_insets(12., 0., 0., 0.));
    assert_eq!(height(&h, "panel"), 180.);
    assert!(h.engine().quiescent());
    assert!(h.has_hold(hold));
    good(&h.hold_end(hold, HoldEnd::Cancel, 0.));
    good(&h.tick(1000.));
    assert_eq!(height(&h, "panel"), 300.);
}

#[test]
fn constrained_catch_uses_published_border_box_css_pixels() {
    let mut h = fixture(
        "max-height=80 padding=10 border-width=2 border-style=\"solid\"",
        "\"auto\"",
    );
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    assert_eq!(height(&h, "panel"), 80.);
    let begin = h.hold_begin(panel, Property::Height, 0.);
    assert!(begin.contains("\"x\":80"), "{begin}");
    let hold = token(&begin);
    good(&h.hold_update(hold, Value::scalar(80.), 0.));
    assert_eq!(height(&h, "panel"), 80.);
    assert_eq!(
        h.engine()
            .target(motion_node(key(&h, "panel")), Property::Height),
        Some(Value::scalar(120.))
    );
}

#[test]
fn invalid_candidate_is_atomic_and_switch_or_clear_retires_height_only() {
    let mut h = fixture("", "\"auto\"");
    let panel = view(&h, "panel");
    let node = key(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    let opacity = token(&h.hold_begin(panel, Property::Opacity, 0.));
    assert!(matches!(
        h.set_height_owner(Some(u32::MAX)),
        Err(HeightOwnerError::UnknownView(_))
    ));
    assert!(matches!(
        h.set_height_owner(Some(view(&h, "contentbox"))),
        Err(HeightOwnerError::UnsupportedBoxSizing(_))
    ));
    assert_eq!(h.height_owner(), Some(node));
    assert!(h.has_hold(hold));
    assert!(h.has_hold(opacity));
    h.set_height_owner(Some(view(&h, "other"))).unwrap();
    assert!(!h.has_hold(hold));
    assert!(h.has_hold(opacity));
    good(&h.hold_update(hold, Value::scalar(f64::NAN), f64::NAN));
    assert_eq!(h.engine().now(), 0.);
    h.set_height_owner(None).unwrap();
    assert_eq!(h.height_owner(), None);
    assert!(h.has_hold(opacity));
}

#[test]
fn unsupported_and_ancestor_hidden_retire_held_height_without_retiring_opacity() {
    use exact_kernel::{Op, StyleId, StyleValue};
    for unsupported in [
        StyleValue::Auto,
        StyleValue::Percent(50.),
        StyleValue::Text("env(safe-area-inset-top)".into()),
    ] {
        let mut h = fixture("", "");
        let panel = view(&h, "panel");
        h.set_height_owner(Some(panel)).unwrap();
        let hold = token(&h.hold_begin(panel, Property::Height, 0.));
        let opacity = token(&h.hold_begin(panel, Property::Opacity, 0.));
        let mut patch = exact_kernel::StyleProps::default();
        patch.set_dynamic(StyleId::Height, &unsupported).unwrap();
        h.runner
            .kernel_mut()
            .apply(
                0,
                99,
                &[Op::SetStyle {
                    id: panel,
                    patch: Box::new(patch),
                }],
            )
            .unwrap();
        h.layout(&mut Batch::new()).unwrap();
        assert_eq!(h.height_owner(), Some(key(&h, "panel")));
        assert!(!h.has_hold(hold));
        assert!(h.has_hold(opacity));
        assert_eq!(
            h.engine()
                .target(motion_node(key(&h, "panel")), Property::Height),
            None
        );
    }
    let mut h = fixture("", "");
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    let opacity = token(&h.hold_begin(panel, Property::Opacity, 0.));
    good(&press(&mut h, "hide"));
    assert_eq!(h.height_owner(), Some(key(&h, "panel")));
    assert!(!h.has_hold(hold));
    assert!(h.has_hold(opacity));
    good(&press(&mut h, "show"));
    assert_eq!(
        h.engine()
            .target(motion_node(key(&h, "panel")), Property::Height),
        Some(Value::scalar(120.))
    );
    assert!(!h.has_hold(hold));
    assert!(h.has_hold(opacity));
}

#[test]
fn unregistered_height_hold_does_not_adopt_a_slot() {
    let mut h = fixture("", "\"auto\"");
    let batch = h.hold_begin(view(&h, "panel"), Property::Height, 0.);
    good(&batch);
    assert!(!batch.contains("\"token\""));
    assert_eq!(h.height_owner(), None);
}

#[test]
fn compositor_ticks_and_unchanged_held_height_skip_layout() {
    let mut h = fixture("", "");
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    h.hold_update(hold, Value::scalar(180.), 0.);
    let translation = token(&h.hold_begin(panel, Property::Translate, 0.));
    h.hold_update(translation, Value::new(50., 0.), 0.);
    h.hold_end(translation, HoldEnd::Cancel, 0.);
    let calls = h.layout_calls;
    for now in [100., 200., 500., 1000.] {
        good(&h.tick(now));
    }
    assert_eq!(h.layout_calls, calls);
    assert_eq!(height(&h, "panel"), 180.);
}

#[test]
fn height_hold_invalid_input_and_stale_token_do_not_advance_clock() {
    let mut h = fixture("", "");
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    for value in [
        Value::scalar(-1.),
        Value::scalar(-f64::MIN_POSITIVE),
        Value::scalar(f64::NEG_INFINITY),
        Value::scalar(f64::NAN),
        Value::scalar(f64::MAX),
        Value::new(10., 1.),
        Value::new(10., f64::NAN),
    ] {
        assert!(!h.hold_update(hold, value, 100.).contains("\"error\":null"));
        assert_eq!(h.engine().now(), 0.);
        assert_eq!(height(&h, "panel"), 120.);
        assert!(h.has_hold(hold));
    }
    h.set_height_owner(None).unwrap();
    good(&h.hold_update(hold, Value::scalar(f64::NAN), f64::NAN));
    assert_eq!(h.engine().now(), 0.);
}

#[test]
fn multiple_roots_refuse_registration_and_destroyed_generation_is_not_readopted() {
    use exact_kernel::{NodeType, Op};
    let mut h = fixture("", "");
    let panel = view(&h, "panel");
    let old = key(&h, "panel");
    h.runner
        .kernel_mut()
        .apply(
            0,
            99,
            &[
                Op::CreateView {
                    id: 999,
                    node_type: NodeType::View,
                },
                Op::AttachRoot { id: 999 },
            ],
        )
        .unwrap();
    assert!(matches!(
        h.set_height_owner(Some(panel)),
        Err(HeightOwnerError::RequiresSingleRoot { roots: 2 })
    ));
    assert_eq!(h.height_owner(), None);
    let mut h = fixture("", "");
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    h.runner
        .kernel_mut()
        .apply(0, 99, &[Op::DestroyView { id: panel }])
        .unwrap();
    h.layout(&mut Batch::new()).unwrap();
    assert_eq!(h.height_owner(), None);
    assert_eq!(h.engine().value(motion_node(old), Property::Height), None);
}

#[test]
fn failed_projected_layout_keeps_published_frames_and_recovery_remeasures() {
    use exact_kernel::{TextMeasureRequest, TextMeasurer, TextMetrics};
    use std::{cell::Cell, rc::Rc};
    struct Fallible(Rc<Cell<bool>>);
    impl TextMeasurer for Fallible {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            if self.0.get() {
                TextMetrics {
                    width: f32::NAN,
                    height: 0.,
                    first_baseline: None,
                }
            } else {
                MonospaceMeasurer::default().measure(request)
            }
        }
    }
    let failing = Rc::new(Cell::new(false));
    let mut h = fixture_measurer("", Box::new(Fallible(failing.clone())));
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    h.hold_update(hold, Value::scalar(180.), 0.);
    let before = h.mirror[&panel].frame;
    failing.set(true);
    let rejected = h.resize(500., 900.);
    assert!(rejected.contains("InvalidTextMetrics"), "{rejected}");
    assert!(!rejected.contains("\"op\":\"frame\""));
    assert_eq!(h.mirror[&panel].frame, before);
    assert_eq!(height(&h, "panel"), 180.);
    failing.set(false);
    good(&h.resize(500., 900.));
    assert_eq!(height(&h, "panel"), 180.);
    assert!(h.has_hold(hold));
}

#[test]
fn nested_collection_feedback_relayouts_current_height_at_new_epoch() {
    use exact_runner::instance::collection::{CollectionFeedback, RowMeasurement};
    let source = r#"component App
  resource rows = rows() as shape list<number>
  state h = 120
  action grow writes h
    h = 300
  view
    column height="100%" width="100%"
      button testId="grow" press=grow
        text "grow"
      column testId="panel" height=h box-sizing="border-box" transition="height 1000ms linear"
        list virtualized=true testId="port" flex=1 min-height=0 width="100%"
          each n in rows key=n
            text `${n}` height=24
"#;
    let plan = contract::compile(source).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        800.,
    )
    .unwrap()
    .0;
    let panel = view(&h, "panel");
    h.set_height_owner(Some(panel)).unwrap();
    press(&mut h, "grow");
    h.tick(500.);
    assert_eq!(height(&h, "port"), 210.);
    let before = h.runner().kernel().epoch();
    let snapshot = h.runner().collections().remove(0);
    let feedback = CollectionFeedback {
        view: snapshot.view,
        revision: snapshot.revision,
        scroll_sequence: 1,
        offset: 0.,
        port_cross: 400.,
        port_main: height(&h, "port") as f64,
        cross: 400.,
        measurements: snapshot
            .rows
            .iter()
            .map(|row| RowMeasurement {
                view: row.view,
                epoch: row.epoch,
                size: h.runner().kernel().node(row.view).unwrap().frame.height as f64,
            })
            .collect(),
        focus_view: None,
        interaction_view: None,
    };
    good(&h.collection_feedback(&feedback.encode().unwrap(), 500.));
    assert!(h.runner().kernel().epoch() > before);
    assert_eq!(height(&h, "panel"), 210.);
    assert_eq!(height(&h, "port"), 210.);
    assert!(h.runner().collections()[0].rows.len() < 100);
}

#[test]
fn negative_held_height_refuses_atomically_and_zero_catch_uses_constrained_layout() {
    let mut h = fixture("min-height=12 padding=4 border-width=2", "");
    let panel = view(&h, "panel");
    let node = motion_node(key(&h, "panel"));
    h.set_height_owner(Some(panel)).unwrap();
    let hold = token(&h.hold_begin(panel, Property::Height, 0.));
    let translation = token(&h.hold_begin(panel, Property::Translate, 0.));
    good(&h.hold_update(translation, Value::new(-30., -10.), 0.));
    let refused = h.hold_update(hold, Value::scalar(-50.), 10.);
    assert!(!refused.contains("\"error\":null"));
    assert!(refused.contains("\"ops\":[]"));
    assert_eq!(h.engine().now(), 0.);
    assert_eq!(h.now_ms, 0.);
    assert_eq!(height(&h, "panel"), 120.);
    assert!(h.has_hold(hold));
    assert!(h.has_hold(translation));
    assert_eq!(
        h.engine().value(node, Property::Height),
        Some(Value::scalar(120.))
    );
    assert_eq!(
        h.engine().value(node, Property::Translate),
        Some(Value::new(-30., -10.))
    );
    good(&h.hold_update(hold, Value::scalar(0.), 10.));
    assert_eq!(
        h.engine().value(node, Property::Height),
        Some(Value::scalar(0.))
    );
    assert_eq!(
        h.engine().target(node, Property::Height),
        Some(Value::scalar(120.))
    );
    assert_eq!(height(&h, "panel"), 12.);
    let caught = token(&h.hold_begin(panel, Property::Height, 10.));
    assert_eq!(
        h.engine().value(node, Property::Height),
        Some(Value::scalar(12.))
    );
    assert!(!h.has_hold(hold));
    assert!(h.has_hold(caught));
    good(&h.hold_update(hold, Value::scalar(f64::NAN), f64::NAN));
    good(&h.hold_end(hold, HoldEnd::Cancel, f64::NAN));
    assert_eq!(h.engine().now(), 0.01);
}

#[test]
fn negative_spring_lobe_clips_only_height_presentation_and_settles() {
    use exact_kernel::{Op, StyleId, StyleValue};
    for target in [0., 20.] {
        let mut h = fixture("", "");
        let panel = view(&h, "panel");
        let node = motion_node(key(&h, "panel"));
        let mut patch = exact_kernel::StyleProps::default();
        patch
            .set_dynamic(StyleId::Height, &StyleValue::Number(target))
            .unwrap();
        patch
            .set_dynamic(
                StyleId::Transition,
                &StyleValue::Text("height spring(180, 12, 1), translate spring(180, 12, 1)".into()),
            )
            .unwrap();
        h.runner
            .kernel_mut()
            .apply(
                0,
                99,
                &[Op::SetStyle {
                    id: panel,
                    patch: Box::new(patch),
                }],
            )
            .unwrap();
        h.layout(&mut Batch::new()).unwrap();
        good(&h.set_height_owner(Some(panel)).unwrap().batch);
        let hold = token(&h.hold_begin(panel, Property::Height, 0.));
        let translate = token(&h.hold_begin(panel, Property::Translate, 0.));
        good(&h.hold_update(translate, Value::new(-30., -10.), 0.));
        good(&h.hold_end(
            hold,
            HoldEnd::Release {
                velocity: Value::scalar(-2000.),
            },
            0.,
        ));
        assert!(!h.has_hold(hold));
        good(&h.tick(50.));
        let raw = h.engine().value(node, Property::Height).unwrap().x;
        assert!(raw < 0., "raw={raw}");
        assert!((raw - (target - 69.7162387436)).abs() < 0.000001);
        assert_eq!(height(&h, "panel"), 0.);
        assert_eq!(
            h.engine().target(node, Property::Height),
            Some(Value::scalar(target))
        );
        assert_eq!(h.height_catch(key(&h, "panel")), Some(Value::scalar(0.)));
        assert_eq!(
            h.engine().value(node, Property::Translate),
            Some(Value::new(-30., -10.))
        );
        good(&h.hold_end(hold, HoldEnd::Cancel, f64::NAN));
        assert_eq!(h.engine().now(), 0.05);
        good(&h.tick(10000.));
        assert_eq!(height(&h, "panel"), target as f32);
        assert_eq!(
            h.engine().value(node, Property::Height),
            Some(Value::scalar(target))
        );
        assert!(h.has_hold(translate));
    }
}

fn accordions(measurer: Box<dyn exact_kernel::TextMeasurer>) -> Host<NoData> {
    let source = r#"component App
  state first = false
  state second = false
  state allowed = true
  state hidden = false
  state mounted = true
  state contentHeight = 120
  state counter = 0
  action toggleFirst writes first
    first = not first
  action toggleSecond writes second
    second = not second
  action optOut writes allowed
    allowed = false
  action hide writes hidden
    hidden = true
  action show writes hidden
    hidden = false
  action remove writes mounted
    mounted = false
  action growContent writes contentHeight, counter
    contentHeight = 240
    counter = counter + 1
  view
    column width="100%" height="100%" interpolate-size=(allowed ? "allow-keywords" : "numeric-only")
      button testId="first-toggle" press=toggleFirst
        text "First"
      button testId="second-toggle" press=toggleSecond
        text "Second"
      button testId="opt-out" press=optOut
        text "Opt out"
      button testId="hide" press=hide
        text "Hide"
      button testId="show" press=show
        text "Show"
      button testId="remove" press=remove
        text "Remove"
      button testId="grow-content" press=growContent
        text `${counter}`
      column testId="sections" display=(hidden ? "none" : "flex")
        when mounted
          column testId="first-body" height=(first ? "auto" : "0px") box-sizing="border-box" overflow="hidden" inert=(not first) transition="height 1000ms linear"
            box height=contentHeight
          column testId="second-body" height=(second ? "auto" : "0px") box-sizing="border-box" overflow="hidden" inert=(not second) transition="height 1000ms linear"
            box height=80
        box testId="following" height=20
"#;
    Host::boot(
        &contract::compile(source).unwrap().encode(),
        NoData,
        measurer,
        400.,
        800.,
    )
    .unwrap()
    .0
}

#[test]
fn accordion_transitions_overlap_reverse_and_return_to_authored_layout() {
    let mut h = accordions(Box::new(MonospaceMeasurer::default()));
    assert_eq!(
        (height(&h, "first-body"), height(&h, "second-body")),
        (0., 0.)
    );
    assert!(h.engine().quiescent());
    let following = h
        .runner()
        .kernel()
        .node(view(&h, "following"))
        .unwrap()
        .frame
        .y;
    good(&press(&mut h, "first-toggle"));
    assert_eq!(height(&h, "first-body"), 0., "no expanded-frame flash");
    good(&h.tick(250.));
    assert_eq!(height(&h, "first-body"), 30.);
    good(&press(&mut h, "second-toggle"));
    let measured = h.height_target_passes;
    for time in 251..=500 {
        good(&h.tick(time as f64));
    }
    assert_eq!(
        h.height_target_passes, measured,
        "motion ticks must not measure authored targets"
    );
    assert_eq!(
        (height(&h, "first-body"), height(&h, "second-body")),
        (60., 20.)
    );
    let next = h
        .runner()
        .kernel()
        .node(view(&h, "following"))
        .unwrap()
        .frame
        .y;
    assert!((next - following - 80.).abs() < 0.001);
    good(&press(&mut h, "first-toggle"));
    assert_eq!(
        height(&h, "first-body"),
        60.,
        "reversal begins at current presentation"
    );
    good(&h.tick(1000.));
    assert_eq!(
        (height(&h, "first-body"), height(&h, "second-body")),
        (0., 60.)
    );
    good(&h.tick(1250.));
    assert_eq!(
        (height(&h, "first-body"), height(&h, "second-body")),
        (0., 80.)
    );
    assert!(h.height_projection.is_empty());
    let layouts = h.layout_calls;
    good(&h.tick(1300.));
    assert_eq!(h.layout_calls, layouts);
}

#[test]
fn accordion_settled_content_changes_snap_but_active_content_retargets_continuously() {
    let mut h = accordions(Box::new(MonospaceMeasurer::default()));
    good(&press(&mut h, "first-toggle"));
    good(&h.tick(250.));
    good(&press(&mut h, "grow-content"));
    assert_eq!(height(&h, "first-body"), 30.);
    good(&h.tick(1250.));
    assert_eq!(height(&h, "first-body"), 240.);
    assert!(h.engine().quiescent());
    // Same auto value, changed content, after settling: ordinary layout.
    let mut h = accordions(Box::new(MonospaceMeasurer::default()));
    good(&press(&mut h, "first-toggle"));
    good(&h.tick(1000.));
    good(&press(&mut h, "grow-content"));
    assert_eq!(height(&h, "first-body"), 240.);
    assert!(h.engine().quiescent());
}

#[test]
fn accordion_opt_out_does_not_cancel_inflight_motion_but_prevents_new_keyword_transition() {
    let mut h = accordions(Box::new(MonospaceMeasurer::default()));
    good(&press(&mut h, "first-toggle"));
    good(&h.tick(250.));
    good(&press(&mut h, "opt-out"));
    assert_eq!(height(&h, "first-body"), 30.);
    good(&h.tick(500.));
    assert_eq!(height(&h, "first-body"), 60.);
    good(&h.tick(1000.));
    assert_eq!(height(&h, "first-body"), 120.);
    good(&press(&mut h, "first-toggle"));
    assert_eq!(height(&h, "first-body"), 0.);
    assert!(h.engine().quiescent());
}

#[test]
fn accordion_hide_show_and_removal_retire_only_live_height_owners() {
    let mut h = accordions(Box::new(MonospaceMeasurer::default()));
    good(&press(&mut h, "first-toggle"));
    good(&h.tick(250.));
    good(&press(&mut h, "hide"));
    assert_eq!(height(&h, "first-body"), 0.);
    assert!(h.engine().quiescent());
    good(&press(&mut h, "show"));
    assert_eq!(height(&h, "first-body"), 120.);
    assert!(h.engine().quiescent());
    good(&press(&mut h, "second-toggle"));
    good(&h.tick(500.));
    good(&press(&mut h, "remove"));
    assert!(h.height_transitions.is_empty());
    assert!(h.height_projection.is_empty());
    assert!(h.engine().quiescent());
}

#[test]
fn accordion_measurement_refusal_preserves_frames_and_retries_without_panicking() {
    use exact_kernel::{TextMeasureRequest, TextMeasurer, TextMetrics};
    use std::{cell::Cell, rc::Rc};
    struct Fallible(Rc<Cell<bool>>);
    impl TextMeasurer for Fallible {
        fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
            if self.0.get() {
                TextMetrics {
                    width: f32::NAN,
                    height: 0.,
                    first_baseline: None,
                }
            } else {
                MonospaceMeasurer::default().measure(request)
            }
        }
    }
    let failing = Rc::new(Cell::new(false));
    let mut h = accordions(Box::new(Fallible(failing.clone())));
    good(&press(&mut h, "first-toggle"));
    good(&h.tick(250.));
    let before = height(&h, "first-body");
    failing.set(true);
    let error = press(&mut h, "grow-content");
    assert!(error.contains("InvalidTextMetrics"), "{error}");
    assert_eq!(height(&h, "first-body"), before);
    assert!(!error.contains("\"op\":\"frame\""));
    failing.set(false);
    good(&h.tick(250.));
    assert_eq!(height(&h, "first-body"), before);
    good(&h.tick(1250.));
    assert_eq!(height(&h, "first-body"), 240.);
}
