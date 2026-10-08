//! The page's environment in Contract: `viewport-fit="cover"` on the root and
//! `env(safe-area-inset-*)` lengths in style attributes reach the kernel as
//! the `viewportFit` prop and `Dimension::Env` rows, and the insets the host
//! sets move the content (LLP 1001 §2, LLP 1006 §2).

use exact_kernel::{Dimension, Edge, Env, Kernel, Offer, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn viewport_fit_and_env_lengths_reach_the_kernel_and_follow_the_insets() {
    let plan = contract::compile(&corpus("insets.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id_of = |k: &Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let (root, content) = (id_of(r.kernel(), "root"), id_of(r.kernel(), "content"));
    {
        let k = r.kernel();
        let n = k.node(root).unwrap();
        assert_eq!(n.props.str(PropId::ViewportFit), Some("cover"));
        assert_eq!(n.style.padding_top, Dimension::Env(Edge::Top, 0.0));
        assert_eq!(n.style.padding_right, Dimension::Env(Edge::Right, 0.0));
        assert_eq!(n.style.padding_bottom, Dimension::Env(Edge::Bottom, 0.0));
        assert_eq!(n.style.padding_left, Dimension::Env(Edge::Left, 0.0));
    }
    let k = r.kernel_mut();
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    let f = k.node(content).unwrap().frame;
    assert_eq!((f.x, f.y, f.width, f.height), (0.0, 0.0, 402.0, 874.0));
    // The phone's insets (an iPhone 17 Pro, portrait): the content keeps out.
    assert!(k.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    let f = k.node(content).unwrap().frame;
    assert_eq!((f.x, f.y, f.width, f.height), (0.0, 62.0, 402.0, 778.0));
    // The input sits at the bottom of the content, above the home indicator.
    let note = k.node(id_of(k, "note")).unwrap().frame;
    // Flex placement accumulates f32 fractions as preceding rows change.
    let bottom = note.y + note.height;
    assert!((bottom - (874.0 - 34.0 - 16.0)).abs() < 0.001, "{bottom}");
}

#[test]
fn resizes_content_reaches_the_kernel_and_a_bottom_bar_follows_the_viewport() {
    let plan = contract::compile(&corpus("keyboard-bar.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id_of = |k: &Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let (root, bar) = (id_of(r.kernel(), "root"), id_of(r.kernel(), "bar"));
    let k = r.kernel_mut();
    assert_eq!(
        k.node(root).unwrap().props.str(PropId::InteractiveWidget),
        Some("resizes-content")
    );
    // The phone, keyboard down: the bar sits on the home indicator's inset.
    k.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap();
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    let f = k.node(bar).unwrap().frame;
    assert_eq!(f.y + f.height, 874.0 - 34.0);
    // Keyboard up (335): the host offers the viewport above it, the bottom
    // inset gone — the bar's bottom is the keyboard's top.
    k.set_env(Env::new(62.0, 0.0, 0.0, 0.0)).unwrap();
    k.compute_layout(root, Offer::definite(402.0, 874.0 - 335.0))
        .unwrap();
    let f = k.node(bar).unwrap().frame;
    assert_eq!(f.y + f.height, 539.0);
    assert_eq!(k.node(root).unwrap().frame.height, 539.0);
}

/// `blur()` is an ordinary command: the dismiss button's press reaches the
/// host by name with no argument, and focuses nothing else on the way.
#[test]
fn dismiss_reaches_the_host_as_a_blur_command() {
    let plan = contract::compile(&corpus("keyboard-bar.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let dismiss = r.kernel().find_by_test_id("dismiss")[0];
    let dismiss = r.kernel().node_by_key(dismiss).unwrap().id;
    r.dispatch(dismiss, exact_runner::Event::Press).unwrap();
    let commands: Vec<(String, usize)> = r
        .take_commands()
        .into_iter()
        .map(|c| (c.name, c.args.len()))
        .collect();
    assert_eq!(commands, [("blur".to_string(), 0)]);
}

/// LLP 1001 §2 (2026-10-07): `min()`, `max()` and `clamp()` in a length
/// attribute, a box shorthand's side among them, reach the kernel as one
/// length each and follow the insets the host sets.
#[test]
fn comparison_lengths_compile_and_follow_the_insets() {
    let src = "component A\n  view\n    column testId=\"root\" height=\"100%\"\n      column testId=\"bar\" height=49 padding=\"0 0 clamp(15px, env(safe-area-inset-bottom), 60px)\"\n      column testId=\"fab\" position=\"absolute\" height=56 bottom=\"calc(clamp(15px, env(safe-area-inset-bottom), 60px) + 15px + 44px)\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id_of = |k: &Kernel, t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id;
    let (root, bar, fab) = {
        let k = r.kernel();
        (id_of(k, "root"), id_of(k, "bar"), id_of(k, "fab"))
    };
    let k = r.kernel_mut();
    for (inset, padding) in [(0.0, 15.0), (34.0, 34.0), (80.0, 60.0)] {
        k.set_env(Env::new(0.0, 0.0, inset, 0.0)).unwrap();
        k.compute_layout(root, Offer::definite(402.0, 874.0))
            .unwrap();
        let (b, f) = (k.node(bar).unwrap().frame, k.node(fab).unwrap().frame);
        assert_eq!(b.height, 49.0 + padding, "{inset}");
        assert_eq!(f.y, 874.0 - (padding + 59.0) - 56.0, "{inset}");
    }
}

#[test]
fn a_comparison_outside_the_kernel_grammar_is_refused_at_compile_time() {
    for (value, says) in [
        (
            "clamp(15px, env(safe-area-inset-bottom))",
            "exactly three arguments",
        ),
        ("max(15px, 10%)", "percentage"),
        ("max(15, env(safe-area-inset-bottom))", "takes a unit"),
        (
            "max(15px, env(safe-area-inset-bottom)) + 44px",
            "inside calc()",
        ),
        ("max(15px, env(safe-area-inset-bottom, 0px))", "no fallback"),
    ] {
        let src = format!("component A\n  view\n    column padding-bottom=\"{value}\"\n");
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, "lower-attr-value", "{value}: {e}");
        assert!(e.message.contains(says), "{value}: {e}");
    }
}
