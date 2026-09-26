//! LLP 1057: `keyframes` declarations and `animation`, resolved at compile
//! time into the kernel's row — proven on the rows after boot, and on what
//! the compiler refuses.

use exact_kernel::{Animations, Kernel, StyleId};
use exact_motion::{Direction, Easing, FillMode, Property, Value};
use exact_plan::{Plan, Value as PlanValue};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[PlanValue]) -> Result<PlanValue, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn fixture() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/keyframes.contract");
    std::fs::read_to_string(path).unwrap()
}

fn animation(r: &Runner<NoData>, test_id: &str) -> (bool, Animations) {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap();
    (
        node.style.mask.has(StyleId::Animation),
        node.style.animation.clone(),
    )
}

#[test]
fn a_named_animation_reaches_the_row_with_its_keyframes_and_a_condition_can_stop_it() {
    let plan = contract::compile(&fixture()).unwrap();
    let mut r = Runner::boot(
        Plan::decode(&plan.encode()).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let (set, row) = animation(&r, "mark");
    assert!(set);
    let [a] = row.0.as_slice() else {
        panic!("{row:?}")
    };
    assert_eq!(a.keyframes.name, "breathe");
    assert_eq!(a.easing, Easing::EaseInOut);
    assert!(a.iterations.is_infinite());
    assert!((a.duration - 1.6).abs() < 1e-6);
    let offsets: Vec<f64> = a.keyframes.blocks.iter().map(|b| b.offset).collect();
    assert_eq!(offsets, [0.0, 0.5, 1.0]);
    assert_eq!(
        a.keyframes.blocks[0].values,
        [
            (Property::Opacity, Value::scalar(0.4)),
            (Property::Scale, Value::scalar(0.9)),
        ]
    );
    // From a `style`, through `class`: the keyframe's own easing and a
    // translate read by the kernel's row parser.
    let (_, fresh) = animation(&r, "added");
    let fresh = &fresh.0[0];
    assert_eq!(fresh.fill, FillMode::Both);
    assert_eq!(fresh.direction, Direction::Normal);
    assert_eq!(fresh.keyframes.blocks[0].easing, Some(Easing::EaseOut));
    assert_eq!(
        fresh.keyframes.blocks[0].values[1],
        (Property::Translate, Value::new(0.0, 8.0))
    );
    // `none` is CSS's: no animation, the row still authored.
    let k = r.kernel();
    let toggle = k.node_by_key(k.find_by_test_id("toggle")[0]).unwrap().id;
    r.dispatch(toggle, Event::Press).unwrap();
    assert_eq!(animation(&r, "mark").1, Animations::NONE);
}

#[test]
fn what_cannot_animate_or_resolve_is_refused_at_compile_time() {
    let app = |decls: &str, attr: &str| {
        format!("{decls}component App\n  state on = true\n  view\n    text \"a\" {attr}\n")
    };
    let breathe = "keyframes breathe\n  from\n    opacity=0\n";
    for (source, id, says) in [
        (
            app(breathe, "animation=\"pulse 1s\""),
            "lower-unknown-keyframes",
            "no `keyframes pulse` is declared; declared: `breathe`",
        ),
        (
            app(breathe, "animation=`breathe ${1}s`"),
            "lower-animation-literal",
            "resolved when the app compiles",
        ),
        (
            app(breathe, "animation=\"breathe 1s spring(100, 10, 1)\""),
            "lower-attr-value",
            "`spring()` is a `transition` extension",
        ),
        (
            app("keyframes k\n  from\n    height=10\n", ""),
            "lower-keyframes",
            "`height` cannot be in a keyframe",
        ),
        (
            app("keyframes k\n  to\n    opacity=\"lots\"\n", ""),
            "lower-attr-value",
            "`opacity` in a keyframe is not a valid `opacity`",
        ),
        (
            app("keyframes k\n  120%\n    opacity=1\n", ""),
            "lower-keyframes",
            "outside 0%–100%",
        ),
        (
            app(
                "keyframes k\n  to\n    animation-timing-function=\"spring(1, 2, 3)\"\n",
                "",
            ),
            "lower-keyframes",
            "is not a CSS easing",
        ),
        (
            app(
                "keyframes k\n  to\n    opacity=1\nkeyframes k\n  to\n    opacity=0\n",
                "",
            ),
            "syntax-duplicate-declaration",
            "keyframes `k` is declared twice",
        ),
        (
            app("keyframes k\n  to\n    opacity=1 opacity=0\n", ""),
            "syntax-duplicate-attr",
            "appears twice in `keyframes k`",
        ),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, id, "{source}\n{error}");
        assert!(error.message.contains(says), "{error}");
    }
    // A condition may choose between literals; `none` is one.
    contract::compile(&app(
        breathe,
        "animation=(on ? \"breathe 1s infinite\" : \"none\")",
    ))
    .unwrap();
}

#[test]
fn keyframes_format_and_keep_their_percentages_whole() {
    let source = "keyframes k\n  0%,   100%\n    opacity=0.4    scale=0.9\n  50%\n    opacity=1\ncomponent App\n  view\n    text \"a\" animation=\"k 1s\"\n";
    assert_eq!(
        contract_syntax::fmt::format(source).unwrap(),
        "keyframes k\n  0%, 100%\n    opacity=0.4 scale=0.9\n  50%\n    opacity=1\ncomponent App\n  view\n    text \"a\" animation=\"k 1s\"\n"
    );
}
