//! A package's pictures through the image sync: asked once a showing box,
//! drawn when answered, emptied when revoked or refused, asked again after
//! a commit (a refusal) or an announcement (a change).
use super::super::*;
use exact_kernel::Op;

fn kernel(sources: &[&str]) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut ops = Vec::new();
    for (i, s) in sources.iter().enumerate() {
        let id = i as u32 + 1;
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Image,
        });
        ops.push(Op::SetProp {
            id,
            prop: PropId::ImageSource,
            value: (*s).into(),
        });
        ops.push(Op::AttachRoot { id });
    }
    k.apply(0, 1, &ops).unwrap();
    k
}

fn door() -> Images {
    let mut images = Images::with_assets(Assets::selected(
        PathBuf::new(),
        Arc::new(|name| panic!("a package picture opened as a file: {name}")),
    ));
    images.set_package_schemes(vec!["photos".into()]);
    images
}

/// A commit that changes something (one that changes nothing keeps the epoch).
fn commit(k: &mut Kernel) {
    let (epoch, id) = (k.epoch(), 100 + k.epoch() as u32);
    let node_type = NodeType::View;
    k.apply(0, epoch + 1, &[Op::CreateView { id, node_type }])
        .unwrap();
    assert_eq!(k.epoch(), epoch + 1);
}

fn pixels(w: u32, h: u32) -> tiny_skia::Pixmap {
    tiny_skia::Pixmap::new(w, h).unwrap()
}

#[test]
fn each_box_is_asked_once_and_drawn_when_answered() {
    let k = kernel(&["photos:agent-1", "photos:agent-2"]);
    let mut images = door();
    let roots = k.roots();
    assert!(images.sync(&k, &roots).is_empty());
    let asks = images.package_asks();
    assert_eq!(
        asks.iter().map(|a| a.source.as_str()).collect::<Vec<_>>(),
        ["photos:agent-1", "photos:agent-2"]
    );
    assert_eq!(asks[0].fit, "fill", "CSS's unsaid object-fit");
    // Asked and waiting: no second ask, and nothing for the workers to wait on.
    images.sync(&k, &roots);
    assert!(images.package_asks().is_empty());
    assert!(!images.pending());
    let drawn = images.package_answer(asks[1].ticket, Ok(pixels(30, 20)));
    assert_eq!(drawn, [(roots[1], Some((30., 20.)))]);
    assert!(images.bitmaps.contains_key(&roots[1]));
    assert!(
        images.stats().resident_bytes >= 30 * 20 * 4,
        "charged to the budget"
    );
    // An answer to no ask in flight changes nothing.
    assert!(images
        .package_answer(asks[1].ticket, Ok(pixels(1, 1)))
        .is_empty());
}

#[test]
fn a_revoked_picture_empties_its_box_and_drops_the_reply_in_flight() {
    let k = kernel(&["photos:agent-2"]);
    let mut images = door();
    let roots = k.roots();
    images.sync(&k, &roots);
    let first = images.package_asks().remove(0);
    images.package_answer(first.ticket, Ok(pixels(8, 8)));
    assert_eq!(
        images.package_changed("photos:agent-2", true),
        [(roots[0], None)]
    );
    assert!(!images.bitmaps.contains_key(&roots[0]));
    images.sync(&k, &roots);
    assert!(images.package_asks().is_empty(), "not asked again");
    // A changed one is asked again, at the next sync.
    let k = kernel(&["photos:agent-3"]);
    let roots = k.roots();
    let mut images = door();
    images.sync(&k, &roots);
    let ask = images.package_asks().remove(0);
    assert!(images.package_changed("photos:agent-3", false).is_empty());
    assert!(
        images
            .package_answer(ask.ticket, Ok(pixels(4, 4)))
            .is_empty(),
        "the old ask's reply is dropped"
    );
    images.sync(&k, &roots);
    assert_eq!(images.package_asks().len(), 1);
}

#[test]
fn a_refusal_empties_the_box_and_is_asked_again_after_a_commit() {
    let mut k = kernel(&["photos:agent-1"]);
    let mut images = door();
    let roots = k.roots();
    images.sync(&k, &roots);
    let ask = images.package_asks().remove(0);
    assert_eq!(
        images.package_answer(
            ask.ticket,
            Err((true, "permission photos is prompt".into()))
        ),
        [(roots[0], None)]
    );
    images.sync(&k, &roots);
    assert!(
        images.package_asks().is_empty(),
        "the same epoch asks nothing"
    );
    commit(&mut k);
    images.sync(&k, &roots);
    assert_eq!(
        images.package_asks().len(),
        1,
        "a commit later, admission is asked again"
    );
}

#[test]
fn an_implementation_failure_is_not_asked_again_until_the_source_changes() {
    let mut k = kernel(&["photos:agent-1"]);
    let mut images = door();
    let roots = k.roots();
    images.sync(&k, &roots);
    let ask = images.package_asks().remove(0);
    images.package_answer(ask.ticket, Err((false, "NotFoundError".into())));
    commit(&mut k);
    images.sync(&k, &roots);
    assert!(images.package_asks().is_empty());
    let epoch = k.epoch();
    k.apply(
        0,
        epoch + 1,
        &[Op::SetProp {
            id: 1,
            prop: PropId::ImageSource,
            value: "photos:agent-4".into(),
        }],
    )
    .unwrap();
    images.sync(&k, &roots);
    assert_eq!(images.package_asks()[0].source, "photos:agent-4");
}
