//! Geometry reads (LLP 1051.000 D1, D3, D5): where layout put a node, and its
//! height at `auto`, without writing a frame, the epoch or a receipt.

use exact_kernel::{
    Kernel, MonospaceMeasurer, NodeType, Offer, Op, PresentedHeight, PropId, StyleId, StyleProps,
    StyleValue,
};

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

fn view(id: u32, node_type: NodeType) -> Op {
    Op::CreateView { id, node_type }
}

/// A 400×600 root holding a 180-high box with two lines of text, a 30-high
/// sibling, and an image with no natural size yet.
fn tree() -> Kernel {
    let mut k = Kernel::new(Box::new(MonospaceMeasurer::default()));
    k.apply(
        0,
        1,
        &[
            view(1, NodeType::View),
            view(2, NodeType::View),
            view(3, NodeType::View),
            view(4, NodeType::Text),
            view(5, NodeType::Image),
            style(
                1,
                &[
                    (StyleId::Width, StyleValue::Number(400.0)),
                    (StyleId::Height, StyleValue::Number(600.0)),
                ],
            ),
            style(
                2,
                &[
                    (StyleId::Height, StyleValue::Number(180.0)),
                    (StyleId::PaddingTop, StyleValue::Number(10.0)),
                    (StyleId::PaddingBottom, StyleValue::Number(10.0)),
                    (StyleId::BoxSizing, StyleValue::Text("border-box".into())),
                ],
            ),
            style(3, &[(StyleId::Height, StyleValue::Number(30.0))]),
            style(5, &[(StyleId::Width, StyleValue::Number(40.0))]),
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "one line\nand another".into(),
            },
            Op::SetChildren {
                id: 2,
                children: vec![4],
            },
            Op::SetChildren {
                id: 3,
                children: vec![5],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}

fn offer() -> Offer {
    Offer::definite(400.0, 600.0)
}

fn key(k: &Kernel, id: u32) -> exact_kernel::NodeKey {
    k.arena().key_of(id).unwrap()
}

#[test]
fn a_frame_is_the_published_box_once_laid_out_and_nothing_before() {
    let mut k = tree();
    assert_eq!(k.laid_out_frame(key(&k, 2)), None, "not laid out yet");
    k.compute_layout(1, offer()).unwrap();
    let (frame, provisional) = k.laid_out_frame(key(&k, 2)).unwrap();
    assert_eq!(
        (frame.x, frame.y, frame.width, frame.height),
        (0.0, 0.0, 400.0, 180.0)
    );
    assert!(!provisional);
    let (sibling, waiting) = k.laid_out_frame(key(&k, 3)).unwrap();
    assert_eq!(sibling.y, 180.0);
    assert!(
        waiting,
        "an image without its natural size makes its box provisional"
    );
    // A change since the last layout: the box last laid out until the next
    // layout (D1), and no what-if, which would see the change.
    k.apply(
        0,
        2,
        &[style(3, &[(StyleId::Height, StyleValue::Number(50.0))])],
    )
    .unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 3)).unwrap().0.height, 30.0);
    assert_eq!(k.measure_auto_height(key(&k, 2)), None);
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 3)).unwrap().0.height, 50.0);
    assert!(k.measure_auto_height(key(&k, 2)).is_some());
    // A node created since the last layout has no box yet.
    k.apply(
        0,
        3,
        &[
            view(6, NodeType::View),
            Op::SetChildren {
                id: 3,
                children: vec![5, 6],
            },
        ],
    )
    .unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 6)), None, "not laid out yet");
    k.compute_layout(1, offer()).unwrap();
    assert!(k.laid_out_frame(key(&k, 6)).is_some());
    // `display` answers as last laid out (D1): a box hidden since keeps its
    // box until the next layout, and one shown since has none until then.
    let display = |value: &str| style(2, &[(StyleId::Display, StyleValue::Text(value.into()))]);
    let shown = k.laid_out_frame(key(&k, 4)).unwrap().0;
    k.apply(0, 4, &[display("none")]).unwrap();
    assert_eq!(
        k.laid_out_frame(key(&k, 4)).unwrap().0,
        shown,
        "hidden since"
    );
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 4)), None, "under display: none");
    k.apply(0, 5, &[display("block")]).unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 4)), None, "shown since");
    k.compute_layout(1, offer()).unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 4)).unwrap().0, shown);
}

#[test]
fn a_measure_at_auto_is_what_committing_auto_would_publish_and_publishes_nothing() {
    let mut k = tree();
    let sample = |k: &Kernel| PresentedHeight {
        node: key(k, 2),
        epoch: k.epoch(),
        px: 250.0,
    };
    let presented = [sample(&k)];
    k.compute_layout_presented(1, offer(), &presented).unwrap();
    assert_eq!(
        k.node(2).unwrap().frame.height,
        250.0,
        "the presented height"
    );
    let before = k.export(None).unwrap();
    let (epoch, receipts) = (k.epoch(), k.receipts().count());

    let (measured, provisional) = k.measure_auto_height(key(&k, 2)).unwrap();
    assert!(!provisional);

    // The oracle: a fork that commits `height: auto` and lays out.
    let mut fork = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    fork.apply(0, 9, &[style(2, &[(StyleId::Height, StyleValue::Auto)])])
        .unwrap();
    fork.compute_layout(1, offer()).unwrap();
    let committed = fork.node(2).unwrap().frame;
    assert_eq!(measured.height.to_bits(), committed.height.to_bits());
    assert_eq!(measured.width.to_bits(), committed.width.to_bits());
    assert!(
        measured.height > 20.0 && measured.height < 180.0,
        "two lines and padding"
    );

    // Nothing moved: frames, the epoch, receipts; the presented height holds.
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!((k.epoch(), k.receipts().count()), (epoch, receipts));
    // The measure left the engine clean: the next layout finds nothing to do,
    // changes no frame and publishes no candidate. (Any pass, measured or not,
    // expires the previous publication's flags, so the export is not compared.)
    let frames = |k: &Kernel| {
        (1..=5)
            .map(|id| k.node(id).unwrap().frame)
            .collect::<Vec<_>>()
    };
    let laid_out = frames(&k);
    let presented = [sample(&k)];
    let receipt = k.compute_layout_presented(1, offer(), &presented).unwrap();
    assert!(receipt.changed.is_empty() && receipt.updated.is_empty());
    assert_eq!(frames(&k), laid_out, "the next layout is unchanged");
    assert_eq!(k.node(2).unwrap().frame.height, 250.0);
}

#[test]
fn a_kernel_that_does_not_lay_out_has_no_answer() {
    let mut k = Kernel::with_monospace_on_demand();
    k.apply(0, 1, &[view(1, NodeType::View), Op::AttachRoot { id: 1 }])
        .unwrap();
    assert_eq!(k.laid_out_frame(key(&k, 1)), None);
    assert_eq!(k.measure_auto_height(key(&k, 1)), None);
    assert_eq!(k.fit_content_height(key(&k, 1)), None);
}

#[test]
fn a_fit_content_height_is_the_content_laid_out_alone_and_publishes_nothing() {
    let mut k = tree();
    k.compute_layout(1, offer()).unwrap();
    let before = k.export(None).unwrap();
    let (epoch, receipts) = (k.epoch(), k.receipts().count());
    let auto = k.measure_auto_height(key(&k, 2)).unwrap().0.height;
    // Its own 180 points are not the content's: two lines and the padding,
    // as at `height: auto` where it stands.
    assert_eq!(k.fit_content_height(key(&k, 2)), Some(auto));
    // The root's 600 points neither: its two children (the image is 0 high
    // until it loads).
    assert_eq!(k.fit_content_height(key(&k, 1)), Some(180.0 + 30.0));
    assert_eq!(k.export(None).unwrap(), before);
    assert_eq!((k.epoch(), k.receipts().count()), (epoch, receipts));
    let receipt = k.compute_layout(1, offer()).unwrap();
    assert!(receipt.changed.is_empty() && receipt.updated.is_empty());
    // A height a transition presents is the presented one there too.
    let presented = [PresentedHeight {
        node: key(&k, 2),
        epoch: k.epoch(),
        px: 250.0,
    }];
    k.compute_layout_presented(1, offer(), &presented).unwrap();
    assert_eq!(k.fit_content_height(key(&k, 1)), Some(250.0 + 30.0));
    k.compute_layout_presented(1, offer(), &[]).unwrap();
    // Squeezed in a 100-point flex column, the two still ask for theirs.
    k.apply(
        0,
        2,
        &[style(
            1,
            &[
                (StyleId::Height, StyleValue::Number(100.0)),
                (StyleId::Display, StyleValue::Text("flex".into())),
                (StyleId::FlexDirection, StyleValue::Text("column".into())),
            ],
        )],
    )
    .unwrap();
    k.compute_layout(1, offer()).unwrap();
    assert!(k.node(2).unwrap().frame.height < 180.0, "shrunk in place");
    assert_eq!(k.fit_content_height(key(&k, 1)), Some(180.0 + 30.0));
}
