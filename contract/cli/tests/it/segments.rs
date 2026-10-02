//! Viewport segments in Contract (LLP 1076 D1–D3, D9): the three fold facts
//! are `exactViewport` fields the bake answers (`continuous`, 1, 1) and a host
//! re-answers through `Runner::set_fold`; `env(viewport-segment-<var> <x> <y>)`
//! lengths in style attributes reach the kernel as segment rows the arena's
//! `Env` resolves from `Kernel::set_segments`. With one segment the fixture's
//! panes stack; with the Duo's two (the division at a book or half angle) the
//! list pane covers segment (0, 0) and the detail pane segment (1, 0), the
//! band between them belonging to neither; `-right` and `-bottom` are a
//! segment's edges from the viewport's left and top, as a DOMRect's. A grid
//! the fold's counts outrun (a host mid-switch, or an index past the grid)
//! resolves to the rows' initial values.

use exact_kernel::{Kernel, Offer, PropId, Rect};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Fold, Posture, Runner};
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

fn boot() -> Runner<NoData> {
    let plan = contract::compile(&corpus("segments.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn id_of(k: &Kernel, t: &str) -> u32 {
    k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
}

fn text_of(k: &Kernel, t: &str) -> String {
    k.node(id_of(k, t))
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap_or_default()
        .to_string()
}

/// The Duo's inner panel, open: `(x, y, w, h)` of the node with `t`.
fn frame(k: &mut Kernel, t: &str) -> (f32, f32, f32, f32) {
    let root = id_of(k, "root");
    k.compute_layout(root, Offer::definite(951.0, 669.0))
        .unwrap();
    let f = k.node(id_of(k, t)).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

/// The Duo at a book or half angle: the division (455.5, 0, 40, 669) splits
/// the inner panel into two columns.
fn duo_segments() -> Vec<Rect> {
    vec![
        Rect::new(0.0, 0.0, 455.5, 669.0),
        Rect::new(495.5, 0.0, 455.5, 669.0),
    ]
}

#[test]
fn one_segment_prints_the_bakes_facts_and_stacks_the_panes() {
    let mut r = boot();
    assert_eq!(text_of(r.kernel(), "fact-posture"), "continuous");
    assert_eq!(text_of(r.kernel(), "fact-h"), "1");
    assert_eq!(text_of(r.kernel(), "fact-v"), "1");
    let root = id_of(r.kernel(), "root");
    assert_eq!(
        r.kernel()
            .node(root)
            .unwrap()
            .props
            .str(PropId::ViewportFit),
        Some("cover")
    );
    assert!(r.kernel().find_by_test_id("corner").is_empty());
    let k = r.kernel_mut();
    let (lx, ly, lw, lh) = frame(k, "pane-list");
    let (dx, dy, dw, dh) = frame(k, "pane-detail");
    assert_eq!((lx, lw), (0.0, 951.0));
    assert_eq!((dx, dw), (0.0, 951.0));
    assert!(
        dy >= ly + lh,
        "the detail pane sits below the list: {dy} {ly} {lh}"
    );
    assert!((dy + dh - 669.0).abs() < 0.001, "{dy} {dh}");
}

#[test]
fn two_segments_put_the_panes_on_the_duos_division_and_one_again_stacks_them() {
    let mut r = boot();
    let folded = Fold {
        posture: Posture::Folded,
        cols: 2,
        rows: 1,
    };
    // The fold's facts switch the screen to its positioned panes; the grid
    // gives their lengths. The host sends both in one batch (D4).
    r.set_fold(folded).unwrap().unwrap();
    assert!(r.kernel_mut().set_segments(2, 1, duo_segments()).unwrap());
    assert_eq!(text_of(r.kernel(), "fact-posture"), "folded");
    assert_eq!(text_of(r.kernel(), "fact-h"), "2");
    assert_eq!(text_of(r.kernel(), "fact-v"), "1");
    let k = r.kernel_mut();
    assert_eq!(frame(k, "pane-list"), (0.0, 0.0, 455.5, 669.0));
    assert_eq!(frame(k, "pane-detail"), (495.5, 0.0, 455.5, 669.0));
    // `-right 0 0` is 455.5 and `-bottom 0 0` is 669: the marker is centred
    // on segment (0, 0)'s bottom-right corner through the `calc()` form.
    assert_eq!(frame(k, "corner"), (451.5, 665.0, 8.0, 8.0));
    // The fold still says two while the grid is one (a host mid-switch): the
    // segment lengths take their rows' initial values — `auto` for a width,
    // a height and an inset — so each absolutely positioned pane shrinks to
    // nothing at its static position, which in a flex column is the
    // container's start, as CSS has it.
    assert!(k.set_segments(1, 1, vec![]).unwrap());
    assert_eq!(
        frame(k, "pane-list"),
        (0.0, 0.0, 0.0, 0.0),
        "auto lengths and insets"
    );
    assert_eq!(
        frame(k, "pane-detail"),
        (0.0, 0.0, 0.0, 0.0),
        "auto lengths and insets"
    );
    // Flat again: one segment, the panes stack.
    r.set_fold(Fold {
        posture: Posture::Continuous,
        cols: 1,
        rows: 1,
    })
    .unwrap()
    .unwrap();
    assert_eq!(text_of(r.kernel(), "fact-posture"), "continuous");
    assert!(r.kernel().find_by_test_id("corner").is_empty());
    let k = r.kernel_mut();
    let (_, ly, lw, lh) = frame(k, "pane-list");
    let (_, dy, dw, dh) = frame(k, "pane-detail");
    assert_eq!((lw, dw), (951.0, 951.0));
    assert!(
        dy >= ly + lh && (dy + dh - 669.0).abs() < 0.001,
        "{ly} {lh} {dy} {dh}"
    );
}
