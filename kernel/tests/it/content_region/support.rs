#![allow(clippy::field_reassign_with_default)]
use exact_kernel::*;
use std::rc::Rc;
pub fn key(k: &Kernel, id: u32) -> NodeKey {
    k.node(id).unwrap().key
}
pub fn fixture() -> Kernel {
    fixture_with(Box::new(MonospaceMeasurer::default()))
}
pub fn fixture_with(measurer: Box<dyn TextMeasurer>) -> Kernel {
    let mut k = Kernel::new(measurer);
    let mut ops = Vec::new();
    for (id, t) in [
        (1, NodeType::View),
        (2, NodeType::View),
        (3, NodeType::View),
        (4, NodeType::Text),
        (5, NodeType::Text),
        (6, NodeType::Text),
    ] {
        ops.push(Op::CreateView { id, node_type: t });
    }
    let mut root = StyleProps::default();
    root.mask = mask(&[StyleId::Height, StyleId::TextAlign]);
    root.height = Dimension::Points(300.);
    root.text_align = TextAlign::Center;
    let mut owner = StyleProps::default();
    owner.mask = mask(&[
        StyleId::Width,
        StyleId::Height,
        StyleId::OverflowX,
        StyleId::OverflowY,
    ]);
    owner.width = Dimension::Percent(100.);
    owner.height = Dimension::Points(180.);
    owner.overflow_x = Overflow::Hidden;
    owner.overflow_y = Overflow::Hidden;
    let mut pending = StyleProps::default();
    pending.mask = mask(&[StyleId::PositionType]);
    pending.position_type = PositionType::Absolute;
    ops.extend([Op::SetStyle{id:1,patch:Box::new(root)},Op::SetStyle{id:2,patch:Box::new(owner)},Op::SetStyle{id:5,patch:Box::new(pending)},Op::SetProp{id:4,prop:PropId::Text,value:"word word word word word word word word word word word word word word word word word".into()},Op::SetProp{id:5,prop:PropId::Text,value:"Loading content".into()},Op::SetProp{id:6,prop:PropId::Text,value:"shell".into()},Op::SetChildren{id:3,children:vec![4]},Op::SetChildren{id:2,children:vec![3,5]},Op::SetChildren{id:1,children:vec![2,6]},Op::AttachRoot{id:1}]);
    k.apply(0, 0, &ops).unwrap();
    k
}
pub fn binding(k: &Kernel) -> ContentRegion {
    ContentRegion {
        owner: key(k, 2),
        content: key(k, 3),
        pending: key(k, 5),
    }
}
pub fn register(k: &mut Kernel) {
    let b = binding(k);
    k.set_content_region(Some(b)).unwrap();
}
pub fn pass(k: &mut Kernel, w: f32, catalog: u64) -> RegionLayoutReceipt {
    k.compute_region_layout(
        1,
        Offer::definite(w, 300.),
        RegionInputs {
            catalog,
            consumer_revision: 1,
        },
    )
    .unwrap()
}
pub fn ready(k: &mut Kernel, w: f32, catalog: u64) -> RegionLayoutReceipt {
    for _ in 0..64 {
        let r = pass(k, w, catalog);
        if r.current {
            return r;
        }
        let q = k.region_text_request().unwrap().clone();
        let m = q.with_request(|r| MonospaceMeasurer::default().measure(r));
        assert!(k.resolve_region_text(&q, m, Rc::new(())).unwrap());
    }
    panic!("bounded offer discovery")
}
pub fn handles(k: &Kernel) -> Vec<(NodeKey, String)> {
    k.arena()
        .iter_live()
        .map(|s| (k.arena().key(s), format!("{:?}", k.arena().taffy(s))))
        .collect()
}

pub fn mask(ids: &[StyleId]) -> StyleMask {
    let mut m = StyleMask::default();
    for id in ids {
        m.set(*id);
    }
    m
}
