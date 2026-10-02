//! Viewport segments in Contract (LLP 1076 D1–D3, D9): the three fold facts
//! are `exactViewport` fields the bake answers (`continuous`, 1, 1), and
//! `env(viewport-segment-<var> <x> <y>)` lengths in style attributes reach the
//! kernel as segment rows the arena's `Env` resolves. With one segment the
//! fixture's panes stack; the band between two segments belongs to neither.
//!
//! Written with the app half of LLP 1076 (2026-10-02), against the RFC's names,
//! before the core half landed: it fails until the compiler admits the six
//! `viewport-segment-*` variables and the bake the three fields. When
//! `Kernel::set_segments(cols, rows, rects)` (D3) lands, the two-segment case
//! belongs here too: `set_segments(2, 1, [(0, 0, 455.5, 669), (495.5, 0, 455.5,
//! 669)])` on a 951×669 offer puts `pane-list` at (0, 0, 455.5, 669) and
//! `pane-detail` at (495.5, 0, 455.5, 669) — the Duo's division at a book or
//! half angle, the facts reading `folded`, `2`, `1`.

use exact_kernel::{Kernel, Offer, PropId};
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
fn segment_lengths_compile_and_one_segment_stacks_the_panes() {
    let plan = contract::compile(&corpus("segments.contract")).unwrap();
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
    let text_of = |k: &Kernel, t: &str| {
        k.node(id_of(k, t))
            .unwrap()
            .props
            .str(PropId::Text)
            .map(str::to_owned)
    };
    // The bake's answers: a flat display, one segment.
    assert_eq!(
        text_of(r.kernel(), "fact-posture").as_deref(),
        Some("continuous")
    );
    assert_eq!(text_of(r.kernel(), "fact-h").as_deref(), Some("1"));
    assert_eq!(text_of(r.kernel(), "fact-v").as_deref(), Some("1"));
    let (root, list, detail) = (
        id_of(r.kernel(), "root"),
        id_of(r.kernel(), "pane-list"),
        id_of(r.kernel(), "pane-detail"),
    );
    assert_eq!(
        r.kernel()
            .node(root)
            .unwrap()
            .props
            .str(PropId::ViewportFit),
        Some("cover")
    );
    // The Duo's inner panel, open: the panes stack, each the viewport's width.
    let k = r.kernel_mut();
    k.compute_layout(root, Offer::definite(951.0, 669.0))
        .unwrap();
    let (l, d) = (k.node(list).unwrap().frame, k.node(detail).unwrap().frame);
    assert_eq!((l.x, l.width), (0.0, 951.0));
    assert_eq!((d.x, d.width), (0.0, 951.0));
    assert!(
        d.y >= l.y + l.height,
        "the detail pane sits below the list: {l:?} {d:?}"
    );
    assert!((d.y + d.height - 669.0).abs() < 0.001, "{d:?}");
}
