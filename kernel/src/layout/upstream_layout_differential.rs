use super::*;
use crate::{
    Kernel, MonospaceMeasurer, Op, PropId, PropValue, StyleId, StyleProps, StyleValue, TextMetrics,
};

fn random(seed: &mut u64, bound: u64) -> f64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed % bound) as f64
}
fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (id, value) in rows {
        patch.set_dynamic(*id, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn t(s: &str) -> StyleValue {
    StyleValue::Text(s.into())
}
fn n(x: f64) -> StyleValue {
    StyleValue::Number(x)
}
#[derive(Default)]
struct Metrics(Vec<[u32; 5]>);
impl TextMeasurer for Metrics {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        let m = MonospaceMeasurer::default().measure(r);
        let offer = |a| match a {
            AxisOffer::Definite(n) => n.to_bits(),
            AxisOffer::MinContent => u32::MAX,
            AxisOffer::MaxContent => u32::MAX - 1,
        };
        self.0.push([
            offer(r.width),
            offer(r.height),
            m.width.to_bits(),
            m.height.to_bits(),
            m.first_baseline.unwrap_or(-1.).to_bits(),
        ]);
        m
    }
}
#[test]
#[ignore = "async lane: 512 seeded trees, ~11 s warm; bun scripts/async.mjs runs it"]
fn seeded_512_trees_match_fresh_frames_content_and_baselines() {
    let mut seed = 0x1043_0007_d1ff_u64;
    let mut measured = 0;
    for case in 0..512 {
        let mut k = Kernel::with_monospace();
        let mut ops = Vec::new();
        for id in 1..=32 {
            let kind = if id <= 3 {
                NodeType::View
            } else if id == 5 || id == 7 || id % 5 == 0 {
                NodeType::Image
            } else {
                NodeType::Text
            };
            ops.push(Op::CreateView {
                id,
                node_type: kind,
            });
            if kind == NodeType::Text {
                ops.push(Op::SetProp {
                    id,
                    prop: PropId::Text,
                    value: PropValue::Str(
                        "alpha beta longerword ".repeat(2 + random(&mut seed, 16) as usize),
                    ),
                });
                ops.push(style(
                    id,
                    &[
                        (StyleId::FontSize, n(10. + random(&mut seed, 20))),
                        (StyleId::PaddingLeft, n(random(&mut seed, 40))),
                        (StyleId::PaddingTop, n(random(&mut seed, 20))),
                        (StyleId::BorderWidthRight, n(random(&mut seed, 8))),
                        (StyleId::BorderWidthBottom, n(random(&mut seed, 5))),
                        (
                            StyleId::BoxSizing,
                            t(if case % 2 == 0 {
                                "border-box"
                            } else {
                                "content-box"
                            }),
                        ),
                    ],
                ));
            }
        }
        ops.extend([
            style(
                1,
                &[
                    (StyleId::Width, n(320. + random(&mut seed, 480))),
                    (StyleId::Height, n(700.)),
                    (
                        StyleId::Display,
                        t(if case % 3 == 0 { "block" } else { "flex" }),
                    ),
                    (
                        StyleId::FlexDirection,
                        t(if case % 2 == 0 { "row" } else { "column" }),
                    ),
                    (StyleId::AlignItems, t("baseline")),
                ],
            ),
            style(
                2,
                &[
                    (StyleId::Width, StyleValue::Percent(80.)),
                    (StyleId::Display, t("flex")),
                    (StyleId::FlexDirection, t("column")),
                ],
            ),
            style(
                3,
                &[
                    (StyleId::Width, StyleValue::Percent(75.)),
                    (
                        StyleId::Display,
                        t(if case % 2 == 0 { "flex" } else { "block" }),
                    ),
                    (StyleId::AlignItems, t("baseline")),
                ],
            ),
            style(
                4,
                &[
                    (StyleId::Width, n(300.)),
                    (StyleId::PaddingLeft, n(50.)),
                    (StyleId::PaddingRight, n(50.)),
                    (StyleId::FlexShrink, n(1.)),
                ],
            ),
            style(
                5,
                &[
                    (StyleId::Width, n(100.)),
                    (StyleId::Height, n(0.)),
                    (StyleId::MarginTop, n(20. + random(&mut seed, 20))),
                    (StyleId::MarginBottom, n(30.)),
                ],
            ),
            style(
                6,
                &[(StyleId::Width, n(250.)), (StyleId::FlexShrink, n(1.))],
            ),
            style(
                7,
                &[
                    (StyleId::Width, n(40. + random(&mut seed, 90))),
                    (StyleId::AspectRatio, n(2.)),
                ],
            ),
            style(8, &[(StyleId::Width, StyleValue::Percent(90.))]),
            Op::SetChildren {
                id: 1,
                children: vec![2, 4, 5, 6, 7],
            },
            Op::SetChildren {
                id: 2,
                children: vec![3],
            },
            Op::SetChildren {
                id: 3,
                children: (8..=32).collect(),
            },
            Op::AttachRoot { id: 1 },
        ]);
        k.apply(0, 0, &ops).unwrap();
        for id in [5, 7, 10, 15, 20, 25, 30] {
            k.set_intrinsic_size(
                id,
                Some((
                    100. + random(&mut seed, 200) as f32,
                    40. + random(&mut seed, 200) as f32,
                )),
            )
            .unwrap();
        }
        let mut a = k.arena().clone();
        let mut b = a.clone();
        let mut incremental = LayoutTree::rebuild(&mut a);

        let (mut ma, mut mb) = (Metrics::default(), Metrics::default());
        for width in [320., 611.5, 480.] {
            let root = a.slot_of(1).unwrap();
            compute(
                &mut a,
                &mut incremental,
                &mut ma,
                root,
                Offer::definite(width, 900.),
            )
            .unwrap();
            let mut fresh = LayoutTree::rebuild(&mut b);
            mb.0.clear();
            ma.0.clear();
            compute(
                &mut b,
                &mut fresh,
                &mut mb,
                root,
                Offer::definite(width, 900.),
            )
            .unwrap();
            for slot in a.iter_live() {
                let x = incremental.layout(a.taffy(slot).unwrap());
                let y = fresh.layout(b.taffy(slot).unwrap());
                let bits = |v: taffy::tree::Layout| {
                    [
                        v.location.x,
                        v.location.y,
                        v.size.width,
                        v.size.height,
                        v.scrollable_overflow_rect.right,
                        v.scrollable_overflow_rect.bottom,
                        v.border.left,
                        v.border.top,
                        v.padding.left,
                        v.padding.top,
                    ]
                    .map(f32::to_bits)
                };
                assert_eq!(bits(x), bits(y), "tree {case}, node {slot}, offer {width}");
            }
            measured += mb.0.len();
        }
    }
    assert!(measured > 10_000); // an empty or bypassed measurer cannot pass
    println!("Upstream differential: 512 seeded trees x 3 offers, 49152 node layouts equal to fresh; {measured} fresh measurements");
}

#[test]
fn unpublished_layout_writes_are_bounded_by_live_nodes() {
    use taffy::prelude::TaffyMaxContent;
    let mut tree = LayoutTree::new();
    let node = tree.new_leaf(taffy::Style::default(), 0, false);
    for width in 1..=100 {
        let mut style = taffy::Style::default();
        style.size.width = taffy::Dimension::length(width as f32);
        tree.set_style(node, style);
        tree.taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
    }
    assert_eq!(tree.taffy.take_layout_changes(), vec![node]);
    assert!(tree.taffy.take_layout_changes().is_empty());
    tree.set_style(node, taffy::Style::default());
    tree.taffy.compute_layout(node, Size::MAX_CONTENT).unwrap();
    tree.remove(node);
    assert!(tree.taffy.take_layout_changes().is_empty());
}
