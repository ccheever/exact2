//! A path's `d` under `transition` (LLP 1055.000 D15), held to Chrome: a
//! matched pair moves number by number, any other pair changes at once, and
//! a change back reverses from where the path is.

use exact_motion::{Engine, PathCommand, PathValue, Property, Transitions};

const NODE: u64 = 3;

fn path(cmds: &[(u8, &[f64])]) -> PathValue {
    PathValue(
        cmds.iter()
            .map(|(verb, args)| {
                let mut a = [0.0; 7];
                a[..args.len()].copy_from_slice(args);
                PathCommand {
                    verb: *verb,
                    args: a,
                }
            })
            .collect(),
    )
}

/// The issue's chevron, pointing down and up.
fn down() -> PathValue {
    path(&[
        (b'M', &[6.0, 9.0]),
        (b'L', &[12.0, 15.0]),
        (b'L', &[18.0, 9.0]),
    ])
}
fn up() -> PathValue {
    path(&[
        (b'M', &[6.0, 15.0]),
        (b'L', &[12.0, 9.0]),
        (b'L', &[18.0, 15.0]),
    ])
}

fn engine(transition: &str) -> Engine {
    let mut e = Engine::new();
    e.set_transitions(NODE, Transitions::parse(transition).unwrap())
        .unwrap();
    e.observe_path(NODE, Some(down()));
    e
}

fn shown(e: &Engine) -> Option<String> {
    e.presented_path(NODE).map(|p| p.to_d())
}

#[test]
fn a_matched_path_moves_under_d_and_all_and_settles_on_the_row() {
    for transition in ["d 400ms linear", "all 400ms linear"] {
        let mut e = engine(transition);
        assert_eq!(shown(&e), None, "first seen: no transition");
        e.observe_path(NODE, Some(up()));
        assert_eq!(shown(&e).as_deref(), Some("M6 9L12 15L18 9"));
        e.advance(0.1).unwrap();
        assert_eq!(shown(&e).as_deref(), Some("M6 10.5L12 13.5L18 10.5"));
        e.advance(0.2).unwrap();
        assert_eq!(
            shown(&e).as_deref(),
            Some("M6 12L12 12L18 12"),
            "flat half way"
        );
        assert!(e.frame().iter().any(|p| p.property == Property::D));
        assert_eq!(e.settle_time(), Some(0.4));
        e.advance(0.4).unwrap();
        assert_eq!(shown(&e), None, "settled: the row shows");
        assert!(e.quiescent());
    }
}

#[test]
fn an_unmatched_path_or_no_declaration_changes_at_once() {
    let curve = path(&[(b'M', &[6.0, 9.0]), (b'Q', &[12.0, 2.0, 18.0, 9.0])]);
    let mut e = engine("d 400ms linear");
    e.observe_path(NODE, Some(curve));
    assert_eq!(shown(&e), None);
    assert!(e.quiescent());

    let mut e = engine("opacity 400ms linear");
    e.observe_path(NODE, Some(up()));
    assert_eq!(shown(&e), None);
    assert!(e.quiescent());

    // A running one stops where a path it cannot reach arrives.
    let mut e = engine("d 400ms linear");
    e.observe_path(NODE, Some(up()));
    e.advance(0.1).unwrap();
    let line = path(&[(b'M', &[0.0, 0.0]), (b'L', &[1.0, 1.0])]);
    e.observe_path(NODE, Some(line));
    assert_eq!(shown(&e), None);
    assert!(e.quiescent());
}

#[test]
fn a_change_back_reverses_from_the_current_shape_in_the_shortened_time() {
    let mut e = engine("d 400ms linear");
    e.observe_path(NODE, Some(up()));
    e.advance(0.2).unwrap();
    e.observe_path(NODE, Some(down()));
    // CSS §3.2: half way through, the way back takes half the time.
    assert_eq!(shown(&e).as_deref(), Some("M6 12L12 12L18 12"));
    assert_eq!(e.settle_time(), Some(0.4));
    e.advance(0.3).unwrap();
    assert_eq!(shown(&e).as_deref(), Some("M6 10.5L12 13.5L18 10.5"));
    e.advance(0.4).unwrap();
    assert_eq!(shown(&e), None);
    assert!(e.quiescent());
}

#[test]
fn a_spring_on_d_plays_its_curve_and_a_removed_node_forgets_its_path() {
    let mut e = engine("d -exact-spring(420, 30, 1)");
    e.observe_path(NODE, Some(up()));
    let end = e.settle_time().expect("a spring's curve");
    assert!(end > 0.0);
    e.advance(end / 2.0).unwrap();
    assert!(shown(&e).is_some());
    e.remove(NODE);
    assert_eq!(shown(&e), None);
    assert!(e.quiescent());
}
