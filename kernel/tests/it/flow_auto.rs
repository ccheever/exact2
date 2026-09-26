//! @ref LLP 1043.000 §8 — auto-height paragraphs flow around exclusions where
//! the admission rule holds, measured around exactly what is published.
use exact_kernel::*;
use exact_textflow::{FlowOptions, Fragment, Options, Prepared};

fn patch(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut s = StyleProps::default();
    for (row, value) in rows {
        s.set_dynamic(*row, value).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(s),
    }
}
fn n(n: f64) -> StyleValue {
    StyleValue::Number(n)
}
fn t(s: &str) -> StyleValue {
    StyleValue::Text(s.into())
}
fn create(id: u32, node_type: NodeType) -> Op {
    Op::CreateView { id, node_type }
}
fn children(id: u32, children: &[u32]) -> Op {
    Op::SetChildren {
        id,
        children: children.to_vec(),
    }
}
fn text(id: u32, words: &str) -> Op {
    Op::SetProp {
        id,
        prop: PropId::Text,
        value: PropValue::Str(words.into()),
    }
}
/// An exclusion box at (x, y) in its parent's padding box.
fn exclusion(id: u32, x: f64, y: f64, w: f64, h: f64) -> Op {
    patch(
        id,
        &[
            (StyleId::PositionType, t("absolute")),
            (StyleId::WrapFlow, t("both")),
            (StyleId::ShapeOutside, t("inset(0)")),
            (StyleId::ShapeMargin, n(6.)),
            (StyleId::Left, n(x)),
            (StyleId::Top, n(y)),
            (StyleId::Width, n(w)),
            (StyleId::Height, n(h)),
        ],
    )
}

const PROSE: &str = "There is an hour when the garden belongs to neither day nor night. \
The visitors have gone, but the birds have not yet settled. Every leaf holds a different \
green, and the paths remember the weight of the afternoon. I used to think a garden was \
a collection of things. Now I think it is mostly a collection of spaces: the pause \
between two branches, the warmth beside a wall.";

// Monospace: 16px type advances 9.6 and a line is 19.2 tall.
const LINE: f32 = 19.2;

/// The drop cap: a padded column, an exclusion at the top-left of its
/// content box, an auto-height paragraph, and a box after it.
fn drop_cap(wrap: &str) -> Kernel {
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        0,
        &[
            create(1, NodeType::View),
            patch(
                1,
                &[
                    (StyleId::Width, n(400.)),
                    (StyleId::PaddingTop, n(20.)),
                    (StyleId::PaddingRight, n(20.)),
                    (StyleId::PaddingBottom, n(20.)),
                    (StyleId::PaddingLeft, n(20.)),
                ],
            ),
            create(2, NodeType::View),
            exclusion(2, 20., 20., 58., 58.),
            patch(2, &[(StyleId::WrapFlow, t(wrap))]),
            create(3, NodeType::Text),
            text(3, PROSE),
            create(4, NodeType::View),
            patch(4, &[(StyleId::Height, n(10.))]),
            children(1, &[2, 3, 4]),
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}
fn layout(k: &mut Kernel) -> LayoutReceipt {
    k.compute_layout(1, Offer::definite(800., 2000.)).unwrap()
}
fn frame(k: &Kernel, id: u32) -> Frame {
    k.node(id).unwrap().frame
}

/// What a painter draws for leaf `id`: the shared walker over monospace
/// advances, around the published shapes (the leaf has no padding here).
/// Advances accumulate per glyph as the measurer's do: the walker decides
/// fits exactly, so a product (48 × 9.6) instead of a sum moves a break.
fn painted(k: &Kernel, id: u32) -> (Vec<Fragment>, f32) {
    let node = k.node(id).unwrap();
    let words: String = node.text_runs().iter().map(|r| r.text.as_ref()).collect();
    let prepared = Prepared::new(&words, Options::default(), &mut |r: std::ops::Range<
        usize,
    >| {
        words[r].chars().map(|_| 9.6f32).sum::<f32>()
    });
    let mut out = Vec::new();
    let result = exact_textflow::flow(
        &prepared,
        node.flow_shapes(),
        &FlowOptions {
            direction: exact_textflow::Direction::Ltr,
            width: node.frame.width,
            line_height: LINE,
            min_fragment: 16. * exact_textflow::MIN_FRAGMENT_EM,
            max_lines: 0,
        },
        &mut out,
    );
    assert!(result.complete);
    (out, result.height)
}

#[test]
fn a_drop_cap_shortens_the_first_lines_and_grows_the_paragraph() {
    let mut plain = drop_cap("auto");
    layout(&mut plain);
    let mut k = drop_cap("both");
    let r = layout(&mut k);
    let leaf = k.node(3).unwrap().key;
    assert_eq!(r.flow_changed, vec![leaf]);
    assert!(r.flow_skipped.is_empty());
    assert_eq!(k.node(3).unwrap().flow_refusal(), None);
    // The inset(0) box, grown by its 6px margin, in the leaf's coordinates.
    assert_eq!(
        k.node(3).unwrap().flow_shapes(),
        &[FlowShape::RoundRect {
            x: -6.,
            y: -6.,
            width: 70.,
            height: 70.,
            radius: 6.
        }]
    );
    let (fragments, height) = painted(&k, 3);
    // Bands 0..=3 meet the box (its bottom is 64): four lines start beside it.
    for f in &fragments[..4] {
        assert_eq!(f.x, 64., "{fragments:?}");
    }
    assert_eq!(fragments[4].x, 0.);
    // Measured is painted: the frame is exactly the flowed paragraph.
    let p = frame(&k, 3);
    assert_eq!(p.height, height);
    let unobstructed = frame(&plain, 3).height;
    assert!(p.height > unobstructed, "{} vs {unobstructed}", p.height);
    let lines = fragments.last().unwrap().line + 1;
    assert_eq!((p.height / LINE).round() as u32, lines);
    assert_eq!((unobstructed / LINE).round() as u32 + 1, lines);
    // What follows moves with it; the column's height takes it in.
    assert_eq!(frame(&k, 4).y, p.y + p.height);
    assert_eq!(frame(&k, 1).height, p.height + 10. + 40.);
    // A still layout settles in one comparison.
    assert!(layout(&mut k).flow_changed.is_empty());
    assert!(frame(&k, 3).bits_eq(p));
}

#[test]
fn removing_or_disabling_the_exclusion_restores_ordinary_height() {
    let mut plain = drop_cap("auto");
    layout(&mut plain);
    let unobstructed = frame(&plain, 3);
    let mut k = drop_cap("both");
    layout(&mut k);
    assert!(frame(&k, 3).height > unobstructed.height);
    k.apply(0, 0, &[patch(2, &[(StyleId::WrapFlow, t("auto"))])])
        .unwrap();
    let r = layout(&mut k);
    assert_eq!(r.flow_changed, vec![k.node(3).unwrap().key]);
    assert!(frame(&k, 3).bits_eq(unobstructed));
    k.apply(0, 0, &[patch(2, &[(StyleId::WrapFlow, t("both"))])])
        .unwrap();
    layout(&mut k);
    assert!(frame(&k, 3).height > unobstructed.height);
    k.apply(0, 0, &[Op::DestroyView { id: 2 }]).unwrap();
    layout(&mut k);
    assert!(frame(&k, 3).bits_eq(unobstructed));
    assert!(k.node(3).unwrap().flow_shapes().is_empty());
}

/// Two auto-height paragraphs whose margins collapse, the second flowing
/// around a box it only reaches once the first has grown around the drop cap:
/// the case whose measure-time offset stage 0 found untrustworthy.
fn article(cap_x: f64) -> Vec<Op> {
    vec![
        create(1, NodeType::View),
        patch(1, &[(StyleId::Width, n(400.))]),
        create(2, NodeType::View),
        exclusion(2, cap_x, 0., 58., 58.),
        create(3, NodeType::Text),
        text(3, PROSE),
        patch(3, &[(StyleId::MarginBottom, n(40.))]),
        create(5, NodeType::View),
        patch(5, &[(StyleId::MarginTop, n(60.))]),
        create(6, NodeType::Text),
        text(6, PROSE),
        patch(6, &[(StyleId::MarginTop, n(10.))]),
        create(7, NodeType::View),
        exclusion(7, 250., 300., 150., 60.),
        children(5, &[6]),
        children(1, &[2, 3, 5, 7]),
        Op::AttachRoot { id: 1 },
    ]
}

#[test]
fn following_paragraphs_settle_in_document_order_through_collapsing_margins() {
    let mut k = Kernel::with_monospace();
    k.apply(0, 0, &article(0.)).unwrap();
    layout(&mut k);
    let p1 = frame(&k, 3);
    let p2 = frame(&k, 6);
    // 40 and 60 collapse between the siblings; 60 and 10 collapse through 5.
    assert_eq!(p2.y, p1.y + p1.height + 60.);
    assert_eq!(frame(&k, 5).y, p2.y);
    // The second paragraph now reaches the box at y 300 and flows around it.
    assert!(p2.y < 300. + 66. && p2.y + p2.height > 294., "{p2:?}");
    assert_eq!(k.node(6).unwrap().flow_shapes().len(), 1);
    for id in [3, 6] {
        assert_eq!(frame(&k, id).height, painted(&k, id).1, "#{id}");
    }
    let mut fresh = Kernel::with_monospace();
    fresh.apply(0, 0, &article(0.)).unwrap();
    layout(&mut fresh);
    for id in [1, 2, 3, 5, 6, 7] {
        assert!(frame(&fresh, id).bits_eq(frame(&k, id)), "#{id}");
        assert_eq!(
            fresh.node(id).unwrap().flow_shapes(),
            k.node(id).unwrap().flow_shapes()
        );
    }
}

#[test]
fn a_moving_exclusion_equals_fresh_replay_and_rehydration_every_step() {
    let mut log = vec![article(0.)];
    let mut k = Kernel::with_monospace();
    k.apply(0, 0, &log[0]).unwrap();
    layout(&mut k);
    for x in [40., 120., 300., 342., 500., 10.] {
        log.push(vec![patch(2, &[(StyleId::Left, n(x))])]);
        k.apply(0, 0, log.last().unwrap()).unwrap();
        layout(&mut k);
        let mut replay = Kernel::with_monospace();
        for batch in &log {
            replay.apply(0, 0, batch).unwrap();
        }
        layout(&mut replay);
        let mut rehydrated = k.rehydrate(Box::new(MonospaceMeasurer::default()));
        layout(&mut rehydrated);
        for other in [&replay, &rehydrated] {
            for id in [1, 2, 3, 5, 6, 7] {
                assert!(frame(other, id).bits_eq(frame(&k, id)), "x {x} #{id}");
                assert_eq!(
                    other.node(id).unwrap().flow_shapes(),
                    k.node(id).unwrap().flow_shapes(),
                    "x {x} #{id}"
                );
            }
        }
        for id in [3, 6] {
            assert_eq!(frame(&k, id).height, painted(&k, id).1, "x {x} #{id}");
        }
    }
}

#[test]
fn a_still_layout_measures_nothing_and_a_moved_shape_measures_only_its_context() {
    use std::{cell::Cell, rc::Rc};
    struct Count(Rc<Cell<usize>>);
    impl TextMeasurer for Count {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            self.0.set(self.0.get() + 1);
            MonospaceMeasurer::default().measure(r)
        }
    }
    let count = Rc::new(Cell::new(0));
    let mut k = Kernel::new(Box::new(Count(count.clone())));
    k.apply(0, 0, &article(0.)).unwrap();
    layout(&mut k);
    count.set(0);
    layout(&mut k);
    assert_eq!(count.get(), 0);
    k.apply(0, 0, &[patch(2, &[(StyleId::Left, n(30.))])])
        .unwrap();
    layout(&mut k);
    // Two leaves, each measured in the pass that changes its shapes and the
    // one that confirms them, and again for the one that follows it.
    assert!(count.get() <= 12, "{} measurements", count.get());
}

#[test]
fn refusals_keep_ordinary_layout_and_say_what_to_change() {
    let refused = |extra: &[Op]| {
        let mut k = drop_cap("both");
        k.apply(0, 0, extra).unwrap();
        let r = layout(&mut k);
        let node = k.node(3).unwrap();
        assert!(node.flow_shapes().is_empty());
        assert_eq!(r.flow_skipped, vec![node.key]);
        node.flow_refusal().unwrap()
    };
    assert_eq!(
        refused(&[patch(1, &[(StyleId::Display, t("flex"))])]),
        FlowRefusal::Context
    );
    assert_eq!(
        refused(&[patch(1, &[(StyleId::AlignContent, t("center"))])]),
        FlowRefusal::Context
    );
    // Bottom-anchored or percentage-placed: the box moves with the context.
    for rows in [
        vec![(StyleId::Top, StyleValue::Auto), (StyleId::Bottom, n(0.))],
        vec![(StyleId::Top, StyleValue::Percent(10.))],
        vec![
            (StyleId::Height, StyleValue::Auto),
            (StyleId::Bottom, n(0.)),
        ],
    ] {
        assert_eq!(refused(&[patch(2, &rows)]), FlowRefusal::Placement);
    }
    assert_eq!(
        refused(&[patch(3, &[(StyleId::PositionType, t("absolute"))])]),
        FlowRefusal::Chain
    );
    assert_eq!(
        refused(&[patch(3, &[(StyleId::Top, StyleValue::Percent(5.))])]),
        FlowRefusal::Chain
    );
    assert_eq!(
        refused(&[
            create(9, NodeType::View),
            patch(9, &[(StyleId::Display, t("flex"))]),
            children(9, &[3]),
            children(1, &[2, 9, 4]),
        ]),
        FlowRefusal::Chain
    );
    for r in [
        FlowRefusal::Context,
        FlowRefusal::Placement,
        FlowRefusal::Chain,
    ] {
        assert!(r.message().contains("give the text a height"), "{r:?}");
    }
    // A refused leaf is laid out exactly as if the shape were not there.
    let mut plain = drop_cap("auto");
    plain
        .apply(0, 0, &[patch(1, &[(StyleId::Display, t("flex"))])])
        .unwrap();
    layout(&mut plain);
    let mut k = drop_cap("both");
    k.apply(0, 0, &[patch(1, &[(StyleId::Display, t("flex"))])])
        .unwrap();
    layout(&mut k);
    assert!(frame(&k, 3).bits_eq(frame(&plain, 3)));
    // Pixel offsets are the leaf's own: still admitted.
    let mut k = drop_cap("both");
    k.apply(0, 0, &[patch(3, &[(StyleId::Top, n(4.))])])
        .unwrap();
    layout(&mut k);
    assert_eq!(k.node(3).unwrap().flow_refusal(), None);
    assert_eq!(k.node(3).unwrap().flow_shapes().len(), 1);
}

#[test]
fn padded_leaves_are_measured_in_content_space_and_line_clamp_holds() {
    let mut k = drop_cap("both");
    k.apply(
        0,
        0,
        &[patch(
            3,
            &[(StyleId::PaddingLeft, n(8.)), (StyleId::PaddingTop, n(5.))],
        )],
    )
    .unwrap();
    layout(&mut k);
    let node = k.node(3).unwrap();
    // Border-box shapes; the content box starts 8 right and 5 down.
    let shapes: Vec<_> = node
        .flow_shapes()
        .iter()
        .map(|s| s.translate(-8., -5.))
        .collect();
    let runs = [TextRun {
        text: PROSE.into(),
        style: node.text_style(),
    }];
    let metrics = MonospaceMeasurer::default().measure(&TextMeasureRequest {
        runs: &runs,
        paragraph: exact_kernel::text::Paragraph::from_style(node.style),
        width: AxisOffer::Definite(node.frame.width - 8.),
        height: AxisOffer::MaxContent,
        exclusions: &shapes,
    });
    assert_eq!(node.frame.height, metrics.height + 5.);
    k.apply(0, 0, &[patch(3, &[(StyleId::LineClamp, n(3.))])])
        .unwrap();
    layout(&mut k);
    assert_eq!(k.node(3).unwrap().frame.height, 3. * LINE + 5.);
}

/// Seeded articles: block contexts, wrappers with collapsing margins, flex
/// wrappers (refused), several exclusions, mutated each round. Every round
/// the incremental kernel equals a fresh replay and a rehydration bit for bit,
/// and every flowed leaf's frame is exactly the height its shapes flow to.
#[test]
fn seeded_articles_settle_to_the_fresh_fixed_point() {
    struct Rng(u64);
    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }
    let words: Vec<&str> = PROSE.split(' ').collect();
    let mut flowed = 0;
    for seed in 1..=120u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut ops = vec![
            create(1, NodeType::View),
            patch(
                1,
                &[
                    (StyleId::Width, n(300. + rng.below(200) as f64)),
                    (StyleId::PaddingTop, n(rng.below(2) as f64 * 12.)),
                ],
            ),
            Op::AttachRoot { id: 1 },
        ];
        let mut kids = vec![];
        let mut texts = vec![];
        let mut next = 2;
        for _ in 0..2 + rng.below(4) {
            let wrapped = rng.below(3) == 0;
            let text_id = next;
            next += 1;
            let len = 8 + rng.below(words.len() as u64 - 8) as usize;
            ops.extend([
                create(text_id, NodeType::Text),
                text(text_id, &words[..len].join(" ")),
                patch(
                    text_id,
                    &[
                        (StyleId::MarginTop, n([0., 10., 40.][rng.below(3) as usize])),
                        (
                            StyleId::MarginBottom,
                            n([0., 10., 40.][rng.below(3) as usize]),
                        ),
                    ],
                ),
            ]);
            texts.push(text_id);
            if wrapped {
                let wrap = next;
                next += 1;
                let display = if rng.below(4) == 0 { "flex" } else { "block" };
                ops.extend([
                    create(wrap, NodeType::View),
                    patch(
                        wrap,
                        &[
                            (StyleId::MarginTop, n(rng.below(3) as f64 * 30.)),
                            (StyleId::Display, t(display)),
                        ],
                    ),
                    children(wrap, &[text_id]),
                ]);
                kids.push(wrap);
            } else {
                kids.push(text_id);
            }
        }
        let mut shapes = vec![];
        for _ in 0..1 + rng.below(3) {
            let id = next;
            next += 1;
            let size = 40. + rng.below(80) as f64;
            ops.extend([
                create(id, NodeType::View),
                exclusion(id, rng.below(300) as f64, rng.below(400) as f64, size, size),
            ]);
            if rng.below(2) == 0 {
                ops.push(patch(id, &[(StyleId::ShapeOutside, t("circle()"))]));
            }
            kids.push(id);
            shapes.push(id);
        }
        ops.push(children(1, &kids));
        let mut log = vec![ops];
        let mut k = Kernel::with_monospace();
        k.apply(0, 0, &log[0]).unwrap();
        for round in 0..5 {
            let edit = match rng.below(3) {
                0 => patch(
                    shapes[rng.below(shapes.len() as u64) as usize],
                    &[
                        (StyleId::Left, n(rng.below(320) as f64)),
                        (StyleId::Top, n(rng.below(420) as f64)),
                    ],
                ),
                1 => patch(
                    texts[rng.below(texts.len() as u64) as usize],
                    &[(StyleId::MarginTop, n(rng.below(50) as f64))],
                ),
                _ => {
                    let id = texts[rng.below(texts.len() as u64) as usize];
                    text(
                        id,
                        &words[..8 + rng.below(words.len() as u64 - 8) as usize].join(" "),
                    )
                }
            };
            log.push(vec![edit]);
            k.apply(0, 0, log.last().unwrap()).unwrap();
            layout(&mut k);
            let mut replay = Kernel::with_monospace();
            for batch in &log {
                replay.apply(0, 0, batch).unwrap();
            }
            layout(&mut replay);
            let mut rehydrated = k.rehydrate(Box::new(MonospaceMeasurer::default()));
            layout(&mut rehydrated);
            for other in [&replay, &rehydrated] {
                for id in 1..next {
                    let (a, b) = (other.node(id).unwrap(), k.node(id).unwrap());
                    assert!(a.frame.bits_eq(b.frame), "seed {seed} round {round} #{id}");
                    assert_eq!(a.flow_shapes(), b.flow_shapes(), "seed {seed} #{id}");
                    assert_eq!(a.flow_refusal(), b.flow_refusal(), "seed {seed} #{id}");
                }
            }
            for &id in &texts {
                if !k.node(id).unwrap().flow_shapes().is_empty() {
                    flowed += 1;
                    assert_eq!(frame(&k, id).height, painted(&k, id).1, "seed {seed} #{id}");
                }
            }
        }
    }
    // A generator that never reaches an exclusion proves nothing.
    assert!(flowed > 300, "{flowed} flowed leaf layouts");
    println!("seeded articles: 120 seeds x 5 rounds, {flowed} flowed leaf layouts equal to fresh");
}
