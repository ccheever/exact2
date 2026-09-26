//! Keyframe animations on Linux (LLP 1057 D4): the host's engine samples the
//! `animation` row under its clock, as it does a transition, and the painter
//! draws the presented values.
use exact_kernel::MonospaceMeasurer;
use exact_linux::Host;
use exact_runner::{DataError, DataSource, Event};

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

fn view(h: &Host<NoData>, name: &str) -> u32 {
    let k = h.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-5, "{actual} vs {expected}");
}

#[test]
fn an_animation_plays_under_the_clock_and_stops_with_its_row() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/keyframes.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let (mark, added) = (view(&h, "mark"), view(&h, "added"));
    assert!(h.motion(), "an endless animation keeps frames coming");
    close(h.presented(mark).opacity, 0.4);
    close(h.presented(mark).scale, 0.9);
    close(h.presented(added).translate.1, 8.0);
    h.tick(400.);
    close(h.presented(mark).opacity, 0.7);
    close(h.presented(added).opacity, 1.0);
    assert_eq!(h.agent("{\"op\":\"settle\"}"), "{\"settle\":null}");
    h.dispatch_at(view(&h, "toggle"), Event::Press, 500.);
    close(h.presented(mark).opacity, 1.0);
    assert!(!h.motion());
}
