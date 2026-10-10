//! List rows laid out by replaying rows like them (Taffy patch 29) against
//! rows laid out fresh: over seeded random rows of a few shapes, rebound to
//! other texts and arms as a list rebinds them (LLP 1078), every frame and
//! scroll extent equals a rehydrated kernel's bit for bit, and most rows of a
//! shape and text sizes seen before are replays.
use crate::{Kernel, MonospaceMeasurer, NodeType, Offer, Op, PropId, StyleId, StyleProps};
use crate::{StyleValue, ViewId};

const OFFER: Offer = Offer {
    width: crate::AxisOffer::Definite(402.0),
    height: crate::AxisOffer::Definite(874.0),
};
const LIST: ViewId = 3;

fn random(seed: &mut u64, bound: u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed % bound
}
fn style(id: ViewId, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        patch.set_dynamic(*row, value).unwrap();
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
fn flex(direction: &str) -> [(StyleId, StyleValue); 2] {
    [
        (StyleId::Display, t("flex")),
        (StyleId::FlexDirection, t(direction)),
    ]
}

/// A mounted row: its wrapper, the views a rebind writes, and what shows.
struct Row {
    wrapper: ViewId,
    column: ViewId,
    head: ViewId,
    title: ViewId,
    body: ViewId,
    chip: ViewId,
    badge: ViewId,
    rule: ViewId,
    chipped: bool,
}

const TITLES: [&str; 3] = ["Message 104", "Message 9", "A much longer title that wraps"];
const BODIES: [&str; 4] = [
    "Yes! I found a place near the park.",
    "Can you send the file before lunch? I need it for the review at two.",
    "Short.",
    "Thanks for the update. The plan for the weekend looks good to me, and I can bring the \
     tickets if you still need them on Saturday.",
];

struct List {
    k: Kernel,
    next: ViewId,
    rows: Vec<Row>,
    seed: u64,
    batch: u64,
    /// Rows differ in their texts only, from a few (a feed of one kind).
    plain: bool,
}

impl List {
    fn new(seed: u64) -> List {
        let mut k = Kernel::with_monospace();
        let mut ops = Vec::new();
        for (id, node_type) in [
            (1, NodeType::View),
            (2, NodeType::Text),
            (LIST, NodeType::List),
            (4, NodeType::View),
            (5, NodeType::View),
        ] {
            ops.push(Op::CreateView { id, node_type });
        }
        ops.extend([
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "Messages".into(),
            },
            style(1, &flex("column")),
            style(
                1,
                &[
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::Height, StyleValue::Percent(100.0)),
                ],
            ),
            style(
                LIST,
                &[
                    (StyleId::FlexGrow, n(1.0)),
                    (StyleId::FlexBasis, StyleValue::Percent(0.0)),
                    (StyleId::MinHeight, n(0.0)),
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::OverflowX, t("hidden")),
                ],
            ),
            Op::SetChildren {
                id: 1,
                children: vec![2, LIST],
            },
            Op::AttachRoot { id: 1 },
        ]);
        k.apply(0, 1, &ops).unwrap();
        List {
            k,
            next: 100,
            rows: Vec::new(),
            seed,
            batch: 1,
            plain: false,
        }
    }

    fn pick(&mut self, bound: usize) -> usize {
        random(&mut self.seed, bound as u64) as usize
    }

    /// A row as an app writes one: an icon box, a column of a title line
    /// (the title flexes beside a fixed badge) and a wrapping body, an
    /// attachment chip some items show, an absolutely placed mark, and a
    /// rule whose margin a rebind changes. `block`: the padded column is a
    /// block, not a flex column.
    fn build(&mut self, ops: &mut Vec<Op>, block: bool) -> Row {
        let ids: [ViewId; 11] = std::array::from_fn(|i| self.next + i as ViewId);
        self.next += 11;
        let [wrapper, line, icon, column, head, title, badge, body, chip, mark, rule] = ids;
        for id in ids {
            let node_type = if [title, body, chip].contains(&id) {
                NodeType::Text
            } else {
                NodeType::View
            };
            ops.push(Op::CreateView { id, node_type });
        }
        ops.extend([
            style(wrapper, &flex("column")),
            style(
                wrapper,
                &[
                    (StyleId::FlexShrink, n(0.0)),
                    (StyleId::MinWidth, n(0.0)),
                    (StyleId::Width, StyleValue::Percent(100.0)),
                    (StyleId::BoxSizing, t("border-box")),
                    (StyleId::PositionType, t("relative")),
                ],
            ),
            style(line, &flex("row")),
            style(
                line,
                &[
                    (StyleId::AlignItems, t("center")),
                    (StyleId::ColumnGap, n(12.0)),
                    (StyleId::PaddingLeft, n(16.0)),
                    (StyleId::PaddingRight, n(16.0)),
                ],
            ),
            style(
                icon,
                &[
                    (StyleId::Width, n(28.0)),
                    (StyleId::Height, n(24.0)),
                    (StyleId::FlexShrink, n(0.0)),
                ],
            ),
            style(
                column,
                &[
                    (StyleId::FlexGrow, n(1.0)),
                    (StyleId::FlexBasis, StyleValue::Percent(0.0)),
                    (StyleId::MinWidth, n(0.0)),
                    (StyleId::PaddingTop, n(21.0)),
                    (StyleId::PaddingBottom, n(18.0)),
                    (StyleId::RowGap, n(10.0)),
                ],
            ),
            style(head, &flex("row")),
            style(
                head,
                &[
                    (StyleId::AlignItems, t("center")),
                    (StyleId::ColumnGap, n(8.0)),
                ],
            ),
            style(
                title,
                &[
                    (StyleId::FlexGrow, n(1.0)),
                    (StyleId::FlexBasis, StyleValue::Percent(0.0)),
                    (StyleId::MinWidth, n(0.0)),
                ],
            ),
            style(
                badge,
                &[
                    (StyleId::MinWidth, n(44.0)),
                    (StyleId::MinHeight, n(44.0)),
                    (StyleId::FlexShrink, n(0.0)),
                ],
            ),
            style(
                chip,
                &[
                    (StyleId::AlignSelf, t("flex-start")),
                    (StyleId::PaddingLeft, n(10.0)),
                    (StyleId::PaddingTop, n(8.0)),
                ],
            ),
            style(
                mark,
                &[
                    (StyleId::PositionType, t("absolute")),
                    (StyleId::Width, n(6.0)),
                    (StyleId::Height, n(6.0)),
                    (StyleId::Right, n(4.0)),
                ],
            ),
            style(
                rule,
                &[(StyleId::Height, n(1.0)), (StyleId::MarginLeft, n(56.0))],
            ),
            Op::SetProp {
                id: chip,
                prop: PropId::Text,
                value: "Weekend itinerary.pdf".into(),
            },
            Op::SetChildren {
                id: head,
                children: vec![title, badge],
            },
            Op::SetChildren {
                id: line,
                children: vec![icon, column],
            },
            Op::SetChildren {
                id: wrapper,
                children: vec![line, mark, rule],
            },
        ]);
        if !block {
            ops.push(style(column, &flex("column")));
        }
        Row {
            wrapper,
            column,
            head,
            title,
            body,
            chip,
            badge,
            rule,
            chipped: false,
        }
        .bound(self, ops, true)
    }

    /// Mount `count` rows, then rebind them for `steps` batches, comparing
    /// every batch with a fresh engine.
    fn run(&mut self, count: usize, steps: usize) {
        let mut ops = Vec::new();
        for i in 0..count {
            let row = self.build(&mut ops, !self.plain && i % 5 == 4);
            self.rows.push(row);
        }
        self.mount(&mut ops);
        self.apply(&ops);
        self.rebind(steps);
    }

    /// Rebind rows for `steps` batches, comparing each with a fresh engine.
    fn rebind(&mut self, steps: usize) {
        for _ in 0..steps {
            let mut ops = Vec::new();
            // A pass rebinds a row or two, as a window moving a row does:
            // the row leaves one end of the list and comes back at the other.
            for _ in 0..1 + self.pick(2) {
                let at = self.pick(self.rows.len());
                let row = self.rows.remove(at);
                let row = row.bound(self, &mut ops, false);
                if self.pick(2) == 0 {
                    self.rows.push(row);
                } else {
                    self.rows.insert(0, row);
                }
            }
            self.mount(&mut ops);
            self.apply(&ops);
        }
    }

    fn mount(&mut self, ops: &mut Vec<Op>) {
        let children = [4]
            .into_iter()
            .chain(self.rows.iter().map(|r| r.wrapper))
            .chain([5])
            .collect();
        ops.push(Op::SetChildren { id: LIST, children });
    }

    fn apply(&mut self, ops: &[Op]) {
        self.batch += 1;
        self.k.apply(0, self.batch, ops).unwrap();
        self.k.compute_layout(1, OFFER).unwrap();
        let mut fresh = self.k.rehydrate(Box::new(MonospaceMeasurer::default()));
        fresh.compute_layout(1, OFFER).unwrap();
        let (a, b) = (self.k.arena(), fresh.arena());
        for slot in a.iter_live() {
            let id = a.local_id(slot);
            let other = b.slot_of(id).unwrap();
            assert!(
                a.frame(slot).bits_eq(b.frame(other)),
                "frame of {id} at batch {}: {:?} against fresh {:?}",
                self.batch,
                a.frame(slot),
                b.frame(other)
            );
            let bits = |(w, h): (f32, f32)| (w.to_bits(), h.to_bits());
            assert_eq!(
                bits(a.content(slot)),
                bits(b.content(other)),
                "scroll extent of {id} at batch {}",
                self.batch
            );
        }
    }
}

impl Row {
    /// Bind the row to another item: its texts, whether its chip shows
    /// (a child added or taken), its badge hidden or not, its rule's margin.
    /// `new`: the row was just built.
    fn bound(mut self, list: &mut List, ops: &mut Vec<Op>, new: bool) -> Row {
        let title = match list.pick(8) {
            // A title no row had: no replay holds its sizes.
            0 if !list.plain => format!("Unseen {}", list.pick(100_000)),
            i => TITLES[i % TITLES.len()].to_string(),
        };
        let body = BODIES[list.pick(BODIES.len())];
        ops.extend([
            Op::SetProp {
                id: self.title,
                prop: PropId::Text,
                value: title.into(),
            },
            Op::SetProp {
                id: self.body,
                prop: PropId::Text,
                value: body.into(),
            },
        ]);
        let chipped = !list.plain && list.pick(3) == 0;
        if new || chipped != self.chipped {
            self.chipped = chipped;
            let mut children = vec![self.head, self.body];
            if chipped {
                children.push(self.chip);
            }
            ops.push(Op::SetChildren {
                id: self.column,
                children,
            });
        }
        ops.push(style(
            self.badge,
            &[(
                StyleId::Display,
                t(if !list.plain && list.pick(6) == 0 {
                    "none"
                } else {
                    "block"
                }),
            )],
        ));
        ops.push(style(
            self.rule,
            &[(
                StyleId::MarginLeft,
                n(if !list.plain && list.pick(4) == 0 {
                    94.0
                } else {
                    56.0
                }),
            )],
        ));
        self
    }
}

#[test]
fn rebound_rows_replayed_equal_rows_laid_out_fresh() {
    // One thread per case: each builds its own kernel.
    let (hits, records) = std::thread::scope(|scope| {
        let cases: Vec<_> = (0..4u64)
            .map(|case| {
                scope.spawn(move || {
                    let mut list = List::new(0x1078_0029_5eed ^ (case * 0x9E37_79B9));
                    list.run(6 + case as usize * 2, 300);
                    list.k.row_layout_memo_counts()
                })
            })
            .collect();
        cases
            .into_iter()
            .map(|case| case.join().unwrap())
            .fold((0, 0), |(h, r), (ch, cr)| (h + ch, r + cr))
    });
    // Rows of two kinds, three titles and unseen ones, four bodies, a chip,
    // a hidden badge and two margins: most rebinds still find a row like
    // theirs laid out before; every row with an unseen title does not.
    assert!(records > 100, "{records} rows were computed and recorded");
    assert!(hits > records, "{hits} replays, {records} recorded");
}

#[test]
fn a_feed_of_one_kind_is_all_replays_once_its_texts_were_seen() {
    let mut list = List::new(0x1078_0029_ea51);
    list.plain = true;
    list.run(10, 150);
    let (_, seen) = list.k.row_layout_memo_counts();
    list.rebind(200);
    let (hits, records) = list.k.row_layout_memo_counts();
    // Three titles and four bodies: twelve rows' worth of answers, held
    // after the first batches; none of the next 200 lays a row out.
    assert_eq!(records, seen, "rows computed after every text was seen");
    assert!(hits > 250, "{hits} replays");
}

#[test]
fn rows_lay_out_the_same_with_the_memo_off() {
    let mut list = List::new(0x1078_0029_00ff);
    list.k.set_row_layout_memo(false);
    list.run(8, 40);
    assert_eq!(list.k.row_layout_memo_counts(), (0, 0));
}
