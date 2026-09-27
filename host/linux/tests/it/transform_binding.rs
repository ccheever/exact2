//! Host preflight and lifetime, separate from Presenter geometry admission.
use exact_kernel::{motion::motion_node, MonospaceMeasurer, NodeKey, PropId, TransformDragBinding};
use exact_linux::Host;
use exact_motion::{HoldEnd, Property, Value};
use exact_runner::{DataError, DataSource, Event};
struct Empty;
impl DataSource for Empty {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
const APP: &str = r#"component App
  state binding = "target"
  state x = 20
  state zoom = 2
  state transition = "translate spring(300,30,1), scale spring(300,30,1)"
  state count = 0
  action geometry(w: number, h: number, pw: number, ph: number) writes count
    count = count
  action release(x: number, y: number, s: number, vx: number, vy: number, vs: number) writes count
    count = count + 1
  action unbind writes binding, transition, x, zoom
    binding = ""
    transition = "none"
    x = 40
    zoom = 1
  view
    box width=400 height=500
      box width=200 height=150 overflow="hidden"
        box id="target" testId="target" width="100%" height="100%" box-sizing="border-box" translate=`${x}px 0px` scale=zoom transition=transition
          box height=30 testId="first" transformDragFor=binding transformgeometry=geometry transformrelease=release
          box height=30 testId="second" transformDragFor="target" transformgeometry=geometry transformrelease=release
      box width=200 height=150 overflow="hidden"
        box id="other" testId="other" width="100%" height="100%" box-sizing="border-box"
          box testId="other-handle" transformDragFor="other" transformgeometry=geometry transformrelease=release
      button testId="unbind" press=unbind
        text "unbind"
      text `${count}` testId="count"
"#;
fn boot(source: &str) -> Host<Empty> {
    let (h, e) = Host::boot(
        &contract::compile(source).unwrap().encode(),
        Empty,
        Box::new(MonospaceMeasurer::default()),
        400.,
        500.,
    )
    .unwrap();
    assert!(e.is_none(), "{e:?}");
    h
}
fn key(h: &Host<Empty>, name: &str) -> NodeKey {
    h.kernel().find_by_test_id(name)[0]
}
fn binding(h: &Host<Empty>, name: &str) -> TransformDragBinding {
    h.transform_drag_binding(key(h, name)).unwrap()
}
fn count(h: &Host<Empty>) -> &str {
    h.kernel()
        .node_by_key(key(h, "count"))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
}
#[test]
fn sole_owner_multiple_handles_and_prop_only_cannot_claim_owner() {
    let h = boot(APP);
    assert_eq!(binding(&h, "first").target, binding(&h, "second").target);
    assert!(h.transform_drag_binding(key(&h, "other-handle")).is_none());
    let h=boot(&APP.replace("transformgeometry=geometry transformrelease=release\n          box", "transformgeometry=geometry\n          box")
        .replace("testId=\"second\" transformDragFor=\"target\" transformgeometry=geometry transformrelease=release", "testId=\"second\" transformDragFor=\"target\" transformgeometry=geometry"));
    assert!(h.transform_drag_binding(key(&h, "first")).is_none());
    assert!(h.transform_drag_binding(key(&h, "second")).is_none());
    assert_eq!(binding(&h, "other-handle").target, key(&h, "other"));
}
#[test]
fn live_tuple_range_validation_is_atomic_and_stale_is_before_validation() {
    let mut h = boot(APP);
    let b = binding(&h, "first");
    let pair = h.transform_drag_begin(b, 10.).unwrap().unwrap();
    for values in [
        [
            Value {
                x: f64::MAX,
                y: 0.,
                ..Value::ZERO
            },
            Value::scalar(2.),
        ],
        [Value::ZERO, Value::scalar(0.)],
        [Value::ZERO, Value::scalar(-1.)],
        [Value::ZERO, Value::scalar(f64::MIN_POSITIVE)],
        [
            Value::ZERO,
            Value {
                x: 2.,
                y: 1.,
                ..Value::ZERO
            },
        ],
    ] {
        assert!(h.transform_drag_update(pair, b, values, 20.).is_err());
        assert_eq!(h.now(), 10.);
        assert!(h.transform_hold_live(pair, b));
        assert_eq!(
            h.engine().value(motion_node(b.target), Property::Translate),
            Some(Value {
                x: 20.,
                y: 0.,
                ..Value::ZERO
            })
        );
    }
    let values = [
        Value {
            x: 30.,
            y: -10.,
            ..Value::ZERO
        },
        Value::scalar(2.),
    ];
    assert!(h
        .dispatch_transform_held(pair, b, values, [Value::ZERO, Value::scalar(f64::NAN)], 20.)
        .is_err());
    assert_eq!(count(&h), "0");
    assert_eq!(h.now(), 10.);
    assert!(h
        .dispatch_transform_held(
            pair,
            b,
            values,
            [
                Value {
                    x: -200.,
                    y: 50.,
                    ..Value::ZERO
                },
                Value::scalar(-0.5)
            ],
            20.
        )
        .unwrap());
    assert_eq!(count(&h), "1");
    assert!(!h
        .dispatch_transform_held(
            pair,
            b,
            [Value::scalar(f64::NAN); 2],
            [Value::scalar(f64::NAN); 2],
            f64::NAN
        )
        .unwrap());
    h.transform_drag_end(pair, None, 20.).unwrap();
    assert!(!h
        .transform_drag_update(pair, b, [Value::scalar(f64::NAN); 2], f64::NAN)
        .unwrap());
}
#[test]
fn one_stale_token_prevents_action_but_cleanup_ends_survivor_not_successor() {
    let mut h = boot(APP);
    let b = binding(&h, "first");
    let pair = h.transform_drag_begin(b, 0.).unwrap().unwrap();
    let id = h.kernel().node_by_key(b.target).unwrap().id;
    let newer = h.hold_begin(id, Property::Scale, 10.).unwrap().unwrap();
    assert!(h.has_hold(pair.translate().token));
    assert!(!h.has_hold(pair.scale().token));
    assert!(!h
        .dispatch_transform_held(pair, b, [Value::ZERO; 2], [Value::ZERO; 2], f64::NAN)
        .unwrap());
    assert_eq!(h.now(), 10.);
    assert_eq!(count(&h), "0");
    h.transform_drag_end(pair, None, 10.).unwrap();
    assert!(!h.has_hold(pair.translate().token));
    assert!(h.has_hold(newer.token));
    h.hold_end(newer.token, HoldEnd::Cancel, 10.).unwrap();
}
#[test]
fn invalid_caught_scale_refuses_before_clock_or_replacing_existing_hold() {
    let mut h = boot(APP);
    let b = binding(&h, "first");
    let id = h.kernel().node_by_key(b.target).unwrap().id;
    let old = h.hold_begin(id, Property::Scale, 0.).unwrap().unwrap();
    h.hold_update(old.token, Value::scalar(0.), 0.).unwrap();
    assert!(h.transform_drag_begin(b, 50.).is_err());
    assert_eq!(h.now(), 0.);
    assert!(h.has_hold(old.token));
    assert!(!h
        .engine()
        .is_held(motion_node(b.target), Property::Translate));
}
#[test]
fn receipt_latest_none_transition_and_target_precede_invalid_binding_cancellation() {
    let mut h = boot(APP);
    let b = binding(&h, "first");
    let pair = h.transform_drag_begin(b, 100.).unwrap().unwrap();
    h.transform_drag_update(
        pair,
        b,
        [
            Value {
                x: 90.,
                y: 0.,
                ..Value::ZERO
            },
            Value::scalar(2.),
        ],
        110.,
    )
    .unwrap();
    let view = h.kernel().node_by_key(key(&h, "unbind")).unwrap().id;
    assert!(h.dispatch_at(view, Event::Press, 200.).is_none());
    assert_eq!(binding(&h, "second").target, b.target);
    assert!(!h.has_hold(pair.translate().token));
    assert!(!h.has_hold(pair.scale().token));
    assert_eq!(h.engine().now(), 0.2);
    assert_eq!(
        h.engine().value(motion_node(b.target), Property::Translate),
        Some(Value {
            x: 40.,
            y: 0.,
            ..Value::ZERO
        })
    );
    assert_eq!(
        h.engine().value(motion_node(b.target), Property::Scale),
        Some(Value::scalar(1.))
    );
    assert!(h.engine().quiescent());
    assert_eq!(count(&h), "0");
}

#[test]
fn pair_moves_also_publish_an_unrelated_running_height_projection() {
    let source = APP.replace("  state binding", "  state height = 180\n  action grow writes height\n    height = 300\n  state binding")
        .replace("      button testId=\"unbind\"", "      box testId=\"panel\" position=\"absolute\" bottom=0 width=40 height=height box-sizing=\"border-box\" transition=\"height spring(300,30,1)\"\n      button testId=\"grow\" press=grow\n        text \"grow\"\n      button testId=\"unbind\"");
    let mut h = boot(&source);
    let panel = h.kernel().node_by_key(key(&h, "panel")).unwrap().id;
    h.set_height_owner(Some(panel)).unwrap();
    let grow = h.kernel().node_by_key(key(&h, "grow")).unwrap().id;
    assert!(h.dispatch_at(grow, Event::Press, 0.).is_none());
    let b = binding(&h, "first");
    let pair = h.transform_drag_begin(b, 0.).unwrap().unwrap();
    h.transform_drag_update(pair, b, [Value::ZERO, Value::scalar(2.)], 100.)
        .unwrap();
    assert!(
        h.kernel().node(panel).unwrap().frame.height > 180.,
        "pair clock advancement must drain layout properties too"
    );
}
