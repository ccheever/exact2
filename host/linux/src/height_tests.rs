use super::*;
use exact_kernel::{
    Dimension, MonospaceMeasurer, Op, StyleId, StyleProps, TextMeasureRequest, TextMetrics,
};
use exact_motion::{HoldEnd, Value};
use exact_runner::{DataError, Value as DataValue};
use std::{cell::Cell, rc::Rc};

struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}
fn boot(measurer: Box<dyn TextMeasurer>) -> Host<Empty> {
    Host::boot(&contract::compile("component App\n  view\n    box width=400 height=300\n      box testId=\"panel\" width=300 height=180 box-sizing=\"border-box\" transition=\"height -exact-spring(300,30,1), translate -exact-spring(300,30,1)\"\n        text \"words wrapping in the panel\"\n").unwrap().encode(), Empty, measurer, 400., 300.).unwrap().0
}
fn panel(h: &Host<Empty>) -> ViewId {
    h.kernel()
        .node_by_key(h.kernel().find_by_test_id("panel")[0])
        .unwrap()
        .id
}
fn style(h: &mut Host<Empty>, patch: StyleProps) -> Option<String> {
    let id = panel(h);
    let root = h.roots()[0];
    let receipt = h
        .runner
        .kernel_mut()
        .apply(
            root,
            0,
            &[Op::SetStyle {
                id,
                patch: Box::new(patch),
            }],
        )
        .unwrap();
    h.commit(
        &[Timed {
            at_ms: h.now_ms,
            receipt,
        }],
        None,
    )
}
#[test]
fn unsupported_height_retires_only_height_and_numeric_readoption_keeps_registration() {
    for height in [
        Dimension::Auto,
        Dimension::Percent(50.),
        Dimension::Env(exact_kernel::Edge::Top, 0.),
    ] {
        let mut h = boot(Box::new(MonospaceMeasurer::default()));
        let panel = panel(&h);
        h.set_height_owner(Some(panel)).unwrap();
        let owner = h.height_owner();
        let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
        let translate = h
            .hold_begin(panel, Property::Translate, 0.)
            .unwrap()
            .unwrap();
        h.hold_update(held.token, Value::scalar(250.), 0.).unwrap();
        let mut patch = StyleProps {
            height,
            ..Default::default()
        };
        patch.mask.set(StyleId::Height);
        assert!(style(&mut h, patch).is_none());
        assert_eq!(h.height_owner(), owner);
        assert!(!h.has_hold(held.token));
        assert!(h.has_hold(translate.token));
        assert!(h.height_projection.is_none());
        assert!(h
            .hold_begin(panel, Property::Height, f64::NAN)
            .unwrap()
            .is_none());
        assert!(!h.hold_end(held.token, HoldEnd::Cancel, f64::NAN).unwrap());
        let mut patch = StyleProps {
            height: Dimension::Points(100.),
            ..Default::default()
        };
        patch.mask.set(StyleId::Height);
        assert!(style(&mut h, patch).is_none());
        assert_eq!(h.kernel().node(panel).unwrap().frame.height, 100.);
        let new = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
        assert_ne!(new.token, held.token);
        assert_eq!(new.value, Value::scalar(100.));
    }
}

#[test]
fn transition_clear_while_held_preserves_presentation_then_snaps_latest_target() {
    let mut h = boot(Box::new(MonospaceMeasurer::default()));
    let panel = panel(&h);
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(250.), 0.).unwrap();
    let mut patch = StyleProps {
        height: Dimension::Points(100.),
        ..Default::default()
    };
    patch.mask.set(StyleId::Height);
    patch.mask.set(StyleId::Transition);
    assert!(style(&mut h, patch).is_none());
    assert_eq!(h.kernel().node(panel).unwrap().frame.height, 250.);
    h.hold_end(held.token, HoldEnd::Cancel, 0.).unwrap();
    assert_eq!(h.kernel().node(panel).unwrap().frame.height, 100.);
    assert!(h.height_projection.is_none());
    assert!(!h.motion());
}

#[test]
fn raw_nonfinite_height_is_rejected_before_nonnegative_clamp() {
    for raw in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, f64::MAX] {
        assert!(height_px(raw).is_err(), "{raw}");
    }
    assert_eq!(height_px(-50.).unwrap(), 0.);
    assert_eq!(height_px(-f64::MAX).unwrap(), 0.);
    assert_eq!(height_px(20.).unwrap(), 20.);
}

struct Fallible(Rc<Cell<bool>>);
impl TextMeasurer for Fallible {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        let mut metrics = MonospaceMeasurer::default().measure(r);
        if self.0.get() {
            metrics.height = f32::NAN;
        }
        metrics
    }
}
#[test]
fn failed_layout_preserves_publication_and_recovery_reuses_current_projection() {
    let fail = Rc::new(Cell::new(false));
    let mut h = boot(Box::new(Fallible(fail.clone())));
    let panel = panel(&h);
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(250.), 0.).unwrap();
    let frame = h.kernel().node(panel).unwrap().frame;
    fail.set(true);
    let mut patch = StyleProps {
        width: Dimension::Points(100.),
        ..Default::default()
    };
    patch.mask.set(StyleId::Width);
    assert!(style(&mut h, patch).is_some());
    assert_eq!(h.kernel().node(panel).unwrap().frame, frame);
    assert!(h.has_hold(held.token));
    fail.set(false);
    assert!(h.resize(400., 300.).is_none());
    assert_eq!(h.kernel().node(panel).unwrap().frame.height, 250.);
    assert_eq!(h.kernel().node(panel).unwrap().frame.width, 100.);
    h.set_height_owner(None).unwrap();
    assert_eq!(h.kernel().node(panel).unwrap().frame.height, 180.);
}

#[test]
fn failed_begin_does_not_leave_a_token_the_caller_never_received() {
    let fail = Rc::new(Cell::new(false));
    let mut h = boot(Box::new(Fallible(fail.clone())));
    let panel = panel(&h);
    h.set_height_owner(Some(panel)).unwrap();
    let key = h.height_owner().unwrap();
    fail.set(true);
    let mut patch = StyleProps {
        width: Dimension::Points(100.),
        ..Default::default()
    };
    patch.mask.set(StyleId::Width);
    assert!(style(&mut h, patch).is_some());
    let frame = h.kernel().node(panel).unwrap().frame;
    assert!(h.hold_begin(panel, Property::Height, 0.).is_err());
    assert!(!h.engine.is_held(motion_node(key), Property::Height));
    assert_eq!(h.kernel().node(panel).unwrap().frame, frame);
    fail.set(false);
    assert!(h.resize(400., 300.).is_none());
    assert!(h.hold_begin(panel, Property::Height, 0.).unwrap().is_some());
}

#[test]
fn unchanged_held_height_does_not_layout_during_translate_frames() {
    let mut h = boot(Box::new(MonospaceMeasurer::default()));
    let panel = panel(&h);
    h.set_height_owner(Some(panel)).unwrap();
    let held = h.hold_begin(panel, Property::Height, 0.).unwrap().unwrap();
    h.hold_update(held.token, Value::scalar(250.), 0.).unwrap();
    let calls = h.layout_calls;
    let translate = h
        .hold_begin(panel, Property::Translate, 0.)
        .unwrap()
        .unwrap();
    h.hold_update(translate.token, Value::new(-80., 0.), 0.)
        .unwrap();
    h.hold_end(translate.token, HoldEnd::Cancel, 0.).unwrap();
    assert!(h.motion());
    for i in 1..30 {
        h.tick(i as f64 * 10.);
        h.hold_update(held.token, Value::scalar(250.), i as f64 * 10.)
            .unwrap();
    }
    assert_eq!(
        h.layout_calls, calls,
        "cached text/frames alone do not prove no layout"
    );
    assert!(h.resize(400., 200.).is_none());
    assert_eq!(
        h.layout_calls,
        calls + 1,
        "resize must still run projected layout"
    );
}

#[test]
fn same_live_owner_is_noop_while_hidden_or_unsupported() {
    for hidden in [false, true] {
        let mut h = boot(Box::new(MonospaceMeasurer::default()));
        let panel = panel(&h);
        h.set_height_owner(Some(panel)).unwrap();
        let owner = h.height_owner();
        let mut patch = StyleProps::default();
        if hidden {
            patch.display = exact_kernel::Display::None;
            patch.mask.set(StyleId::Display);
        } else {
            patch.height = Dimension::Auto;
            patch.mask.set(StyleId::Height);
        }
        assert!(style(&mut h, patch).is_none());
        let before = h.kernel().export(None).unwrap();
        let calls = h.layout_calls;
        h.set_height_owner(Some(panel)).unwrap();
        assert_eq!(h.height_owner(), owner);
        assert_eq!(h.layout_calls, calls);
        assert_eq!(h.now(), 0.);
        assert_eq!(h.kernel().export(None).unwrap(), before);
    }
}
