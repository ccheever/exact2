//! Chrome 154.0.8037.92, 2026-10-18: literal HTML/CSS, measured with
//! getBoundingClientRect. The fixture holds name, containing-block CSS,
//! absolute-flex CSS, item CSS, item count and frames, separated by tabs.
//! Canvas and loaded images both have a natural size of 300×150.
use crate::browser_cases::{css_rows, lay_out_as, mismatches, props};
use exact_kernel::{Kernel, MonospaceMeasurer, NodeType, Offer, Op};

fn fixture(root: &str, container: &str, item: &str, count: u32, kind: NodeType) -> Kernel {
    let mut k = lay_out_as(
        props(&css_rows(root)),
        vec![(2, 1, css_rows(container))],
        &[],
        &[],
        NodeType::View,
    );
    let mut ops = Vec::new();
    for id in 3..3 + count {
        ops.push(Op::CreateView {
            id,
            node_type: kind,
        });
        ops.push(Op::SetStyle {
            id,
            patch: Box::new(props(&css_rows(item))),
        });
    }
    ops.push(Op::SetChildren {
        id: 2,
        children: (3..3 + count).collect(),
    });
    k.apply(0, 2, &ops).unwrap();
    if kind == NodeType::Image {
        for id in 3..3 + count {
            k.set_intrinsic_size(id, Some((300.0, 150.0))).unwrap();
        }
    }
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    k
}

#[test]
fn absolute_flex_sizes_match_chrome() {
    // Separate gaps reproduced on the starting code, recorded in QUEUE.md.
    // Keep Chrome's frames and require removing a gap when it starts matching.
    let owed = [
        "max-height:100px;flex-wrap:wrap-reverse",
        "padding:10px;margin-top:auto",
        "padding:10px;height:50%",
        "nearby flex-direction:column;min-height:400px grow 1",
    ];
    let mut failures = Vec::new();
    for line in include_str!("fixtures/browser_absolute_flex.tsv").lines() {
        let fields: Vec<_> = line.split('\t').collect();
        let [name, root, container, item, count, frames] = fields[..] else {
            panic!("invalid absolute-flex case: {line}");
        };
        let want: Vec<_> = frames
            .split(' ')
            .map(|frame| {
                let (id, rect) = frame.split_once('=').unwrap();
                let v: Vec<f32> = rect.split(',').map(|n| n.parse().unwrap()).collect();
                (id.parse().unwrap(), [v[0], v[1], v[2], v[3]])
            })
            .collect();
        for kind in [NodeType::Canvas, NodeType::Image] {
            let k = fixture(root, container, item, count.parse().unwrap(), kind);
            let found = mismatches(&format!("{name}; {kind:?}"), &k, &want);
            if owed.contains(&name) {
                if found.is_empty() {
                    failures.push(format!(
                        "{name}; {kind:?} now matches; remove its queue entry and owed marker"
                    ));
                }
            } else {
                failures.extend(found);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}

#[test]
fn switching_between_authored_and_auto_height_matches_a_fresh_layout() {
    let mut failures = Vec::new();
    for kind in [NodeType::Canvas, NodeType::Image] {
        let mut k = fixture(
            "position:relative;width:800px;height:600px",
            "position:absolute;display:flex;height:100px",
            "padding:10px",
            2,
            kind,
        );
        for (epoch, height, width) in [(3, "auto", 320.0), (4, "100px", 180.0), (5, "auto", 320.0)]
        {
            k.apply(
                0,
                epoch,
                &[Op::SetStyle {
                    id: 2,
                    patch: Box::new(props(&css_rows(&format!(
                        "height:{height};max-height:100px"
                    )))),
                }],
            )
            .unwrap();
            k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
            let want = [
                (2, [0.0, 0.0, width * 2.0, 100.0]),
                (3, [0.0, 0.0, width, 100.0]),
                (4, [width, 0.0, width, 100.0]),
            ];
            let name = format!("{kind:?}; height:{height}; epoch:{epoch}");
            failures.extend(mismatches(&name, &k, &want));
            let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
            fresh
                .compute_layout(1, Offer::definite(800.0, 600.0))
                .unwrap();
            failures.extend(mismatches(&format!("fresh {name}"), &fresh, &want));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n{} failures",
        failures.join("\n"),
        failures.len()
    );
}
