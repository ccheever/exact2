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
    let mut r = Runner::boot(plan, NoData, Kernel::with_monospace()).unwrap();
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
    assert_eq!(note.y + note.height, 874.0 - 34.0 - 16.0);
}
