//! A table laid out as a grid (LLP 1153 D7.3): cells placed by HTML's
//! table-forming algorithm, `auto` columns, the table as wide as its
//! columns, and HTML's UA spacing (`border-spacing: 2px`, a cell's `padding:
//! 1px`). Each cell here holds a sized box, so no text is measured; the
//! expected boxes are what Chrome's table gives the same markup where the
//! grid and CSS 2.1 §17.5 agree, and the grid's own where they do not
//! (a row span's height, below).

use exact_kernel::{
    Frame, Kernel, NodeType, Offer, Op, PropId, PropValue, StyleId, StyleProps, StyleValue,
};

/// A node of the tree: its tag (`semanticTag`), its own rows and props, and
/// its children; `box W H` is a plain sized box.
enum N {
    Tag(
        &'static str,
        Vec<(StyleId, f64)>,
        Vec<(PropId, i64)>,
        Vec<N>,
    ),
    Box(f64, f64),
}

fn tag(name: &'static str, children: Vec<N>) -> N {
    N::Tag(name, Vec::new(), Vec::new(), children)
}

fn cell(props: &[(PropId, i64)], w: f64, h: f64) -> N {
    N::Tag("td", Vec::new(), props.to_vec(), vec![N::Box(w, h)])
}

fn rows(rows: &[(StyleId, f64)]) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    for (id, n) in rows {
        s.set_dynamic(*id, &StyleValue::Number(*n)).unwrap();
        s.written.set(*id);
    }
    Box::new(s)
}

/// Ops making `n` under a fresh id; returns it.
fn make(n: &N, next: &mut u32, ops: &mut Vec<Op>) -> u32 {
    let id = *next;
    *next += 1;
    ops.push(Op::CreateView {
        id,
        node_type: NodeType::View,
    });
    match n {
        N::Box(w, h) => ops.push(Op::SetStyle {
            id,
            patch: rows(&[(StyleId::Width, *w), (StyleId::Height, *h)]),
        }),
        N::Tag(name, styles, props, children) => {
            ops.push(Op::SetProp {
                id,
                prop: PropId::SemanticTag,
                value: (*name).into(),
            });
            if !styles.is_empty() {
                ops.push(Op::SetStyle {
                    id,
                    patch: rows(styles),
                });
            }
            for (prop, v) in props {
                ops.push(Op::SetProp {
                    id,
                    prop: *prop,
                    value: PropValue::Int(*v),
                });
            }
            let kids = children.iter().map(|c| make(c, next, ops)).collect();
            ops.push(Op::SetChildren { id, children: kids });
        }
    }
    id
}

/// A 400 px wide block (id 1) holding `n` (id 2), laid out.
fn lay_out(n: N) -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: rows(&[(StyleId::Width, 400.0)]),
        },
    ];
    let mut next = 2;
    let child = make(&n, &mut next, &mut ops);
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![child],
    });
    ops.push(Op::AttachRoot { id: 1 });
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 400.0))
        .unwrap();
    kernel
}

fn frame(kernel: &Kernel, id: u32) -> [f32; 4] {
    let Frame {
        x,
        y,
        width,
        height,
    } = kernel.node(id).unwrap().frame;
    [x, y, width, height]
}

const COLSPAN: PropId = PropId::ColSpan;
const ROWSPAN: PropId = PropId::RowSpan;

/// A `colspan=2` cell covers two columns, which share what it needs beyond
/// them (Chrome: 25 and 25, from 22 and 22, as its widths are equal); the
/// table is as wide as its columns and spacing (90 of 400), its rows as
/// wide as its columns.
#[test]
fn a_colspan_covers_two_columns_and_the_table_shrinks_to_them() {
    // 2 table, 3 tr, 4 td(colspan 2), 5 box, 6 td, 7 box, 8 tr, 9/11/13 td.
    let k = lay_out(tag(
        "table",
        vec![
            tag(
                "tr",
                vec![cell(&[(COLSPAN, 2)], 50.0, 10.0), cell(&[], 30.0, 10.0)],
            ),
            tag(
                "tr",
                vec![
                    cell(&[], 20.0, 10.0),
                    cell(&[], 20.0, 10.0),
                    cell(&[], 20.0, 10.0),
                ],
            ),
        ],
    ));
    assert_eq!(frame(&k, 2), [0.0, 0.0, 90.0, 30.0], "the table");
    assert_eq!(frame(&k, 3), [2.0, 2.0, 86.0, 12.0], "its first row");
    assert_eq!(frame(&k, 4), [2.0, 2.0, 52.0, 12.0], "the spanning cell");
    assert_eq!(
        frame(&k, 5),
        [3.0, 3.0, 50.0, 10.0],
        "inside its 1px padding"
    );
    assert_eq!(frame(&k, 6), [56.0, 2.0, 32.0, 12.0]);
    assert_eq!(frame(&k, 8), [2.0, 16.0, 86.0, 12.0], "the second row");
    assert_eq!(frame(&k, 9), [2.0, 16.0, 25.0, 12.0]);
    assert_eq!(frame(&k, 11), [29.0, 16.0, 25.0, 12.0]);
    assert_eq!(frame(&k, 13), [56.0, 16.0, 32.0, 12.0]);
}

/// A `rowspan=2` cell covers its column in the next row, so that row's
/// first cell lands in the second column, as HTML's algorithm puts it. The
/// rows share the height it needs beyond them equally, as a grid's tracks
/// do (32 over two 12 px rows: 15 and 15).
#[test]
fn a_rowspan_covers_the_next_rows_slot() {
    // 2 table, 3 tbody, 4 tr, 5 td(rowspan 2), 6 box, 7 td, 8 box, 9 tr, 10 td.
    let k = lay_out(tag(
        "table",
        vec![tag(
            "tbody",
            vec![
                tag(
                    "tr",
                    vec![cell(&[(ROWSPAN, 2)], 20.0, 30.0), cell(&[], 20.0, 10.0)],
                ),
                tag("tr", vec![cell(&[], 20.0, 10.0)]),
            ],
        )],
    ));
    assert_eq!(frame(&k, 2), [0.0, 0.0, 50.0, 36.0], "the table");
    assert_eq!(frame(&k, 3), [2.0, 2.0, 46.0, 32.0], "its row group");
    assert_eq!(frame(&k, 5), [2.0, 2.0, 22.0, 32.0], "two rows tall");
    assert_eq!(frame(&k, 7), [26.0, 2.0, 22.0, 15.0]);
    assert_eq!(frame(&k, 9), [2.0, 19.0, 46.0, 15.0], "the second row");
    assert_eq!(
        frame(&k, 10),
        [26.0, 19.0, 22.0, 15.0],
        "in the second column"
    );
}

/// A ragged row: a row with fewer cells than the table has columns leaves
/// the rest of its slots empty, and still spans every column.
#[test]
fn a_short_row_leaves_its_last_slots_empty() {
    // 2 table, 3 tr, 4/6/8 td, 10 tr, 11 td.
    let k = lay_out(tag(
        "table",
        vec![
            tag(
                "tr",
                vec![
                    cell(&[], 20.0, 10.0),
                    cell(&[], 20.0, 10.0),
                    cell(&[], 20.0, 10.0),
                ],
            ),
            tag("tr", vec![cell(&[], 40.0, 10.0)]),
        ],
    ));
    assert_eq!(
        frame(&k, 2),
        [0.0, 0.0, 94.0, 30.0],
        "42 + 22 + 22 and spacing"
    );
    assert_eq!(frame(&k, 4), [2.0, 2.0, 42.0, 12.0]);
    assert_eq!(frame(&k, 8), [70.0, 2.0, 22.0, 12.0]);
    assert_eq!(frame(&k, 10), [2.0, 16.0, 90.0, 12.0], "every column");
    assert_eq!(frame(&k, 11), [2.0, 16.0, 42.0, 12.0], "the first slot");
}

/// A table is as wide as its columns where a grid fills its container;
/// a caption is above the rows, as wide as the table, outside its spacing.
#[test]
fn a_table_shrinks_to_fit_and_its_caption_spans_it() {
    // 2 table, 3 caption, 4 box, 5 tr, 6 td, 7 box.
    let k = lay_out(tag(
        "table",
        vec![
            tag("caption", vec![N::Box(10.0, 20.0)]),
            tag("tr", vec![cell(&[], 60.0, 10.0)]),
        ],
    ));
    assert_eq!(frame(&k, 2), [0.0, 0.0, 66.0, 36.0], "not 400 wide");
    assert_eq!(frame(&k, 3), [0.0, 0.0, 66.0, 20.0], "the caption");
    assert_eq!(
        frame(&k, 6),
        [2.0, 22.0, 62.0, 12.0],
        "below it, after the spacing"
    );
    // A `section` holding the same box is a block: it fills the 400.
    let grid = lay_out(N::Tag(
        "section",
        vec![(StyleId::PaddingLeft, 0.0)],
        Vec::new(),
        vec![N::Box(60.0, 10.0)],
    ));
    assert_eq!(frame(&grid, 2)[2], 400.0);
}

/// A `dl` is HTML's block list: 1em above and below it, a `dd` indented
/// 40 px at its start, a `dt` a block.
#[test]
fn a_description_list_indents_its_descriptions() {
    // 2 dl, 3 dt, 4 box, 5 dd, 6 box.
    let k = lay_out(tag(
        "dl",
        vec![
            tag("dt", vec![N::Box(30.0, 10.0)]),
            tag("dd", vec![N::Box(30.0, 10.0)]),
        ],
    ));
    // The dl's margins collapse through the root's top: its box starts 16
    // down in a 400 px block, as `<div><dl>` does in Chrome.
    assert_eq!(frame(&k, 3), [0.0, 16.0, 400.0, 10.0], "a dt is a block");
    assert_eq!(frame(&k, 5), [40.0, 26.0, 360.0, 10.0], "a dd, indented");
}

/// The grid follows its table as it changes: a cell's span moves the cells
/// after it, and a row that arrives widens the column it reaches.
#[test]
fn a_changed_span_and_a_new_row_make_the_grid_again() {
    // 2 table, 3 tr, 4/6 td, 8 tr, 9/11 td; each td holds a 20 x 10 box.
    let row = || tag("tr", vec![cell(&[], 20.0, 10.0), cell(&[], 20.0, 10.0)]);
    let mut k = lay_out(tag("table", vec![row(), row()]));
    assert_eq!(frame(&k, 2)[2], 50.0);
    let set_span = Op::SetProp {
        id: 4,
        prop: COLSPAN,
        value: PropValue::Int(2),
    };
    k.apply(0, 2, &[set_span]).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(frame(&k, 2)[2], 74.0, "a third column");
    assert_eq!(frame(&k, 4), [2.0, 2.0, 46.0, 12.0], "two columns wide");
    assert_eq!(frame(&k, 6), [50.0, 2.0, 22.0, 12.0], "moved to the third");
    let (mut ops, mut next) = (Vec::new(), 13);
    let added = make(&tag("tr", vec![cell(&[], 60.0, 10.0)]), &mut next, &mut ops);
    ops.push(Op::SetChildren {
        id: 2,
        children: vec![3, 8, added],
    });
    k.apply(0, 3, &ops).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(
        frame(&k, 2),
        [0.0, 0.0, 114.0, 44.0],
        "a 62 px first column"
    );
    assert_eq!(frame(&k, 14), [2.0, 30.0, 62.0, 12.0], "the new row's cell");
    assert_eq!(frame(&k, 6), [90.0, 2.0, 22.0, 12.0]);
}

/// A row with `display: none` is no row of the grid, as CSS's table has no
/// box for it; shown again, its cells are placed again.
#[test]
fn a_hidden_row_leaves_the_grid_and_comes_back() {
    // 2 table, 3 tr, 4 td, 5 box (60 wide), 6 tr, 7 td, 8 box.
    let mut k = lay_out(tag(
        "table",
        vec![
            tag("tr", vec![cell(&[], 60.0, 10.0)]),
            tag("tr", vec![cell(&[], 20.0, 10.0)]),
        ],
    ));
    assert_eq!(frame(&k, 2), [0.0, 0.0, 66.0, 30.0]);
    let display = |value: &str| Op::SetStyle {
        id: 3,
        patch: {
            let mut s = StyleProps::default();
            s.set_dynamic(StyleId::Display, &StyleValue::Text(value.into()))
                .unwrap();
            Box::new(s)
        },
    };
    k.apply(0, 2, &[display("none")]).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(frame(&k, 2), [0.0, 0.0, 26.0, 16.0], "one 22 px row");
    assert_eq!(
        frame(&k, 7),
        [2.0, 2.0, 22.0, 12.0],
        "the second row, first"
    );
    k.apply(0, 3, &[display("block")]).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(frame(&k, 2), [0.0, 0.0, 66.0, 30.0], "both rows again");
    assert_eq!(frame(&k, 7), [2.0, 16.0, 62.0, 12.0]);
}

/// The declared deviation (LLP 1153 D7.3): a table narrower than its
/// content gives each column its least and shares the rest equally, as a
/// grid's `auto` tracks do, where Chrome's table shares it in proportion to
/// what each column could still grow (50 and 64 px here; the conformance
/// fixture `table-widths` measures it).
#[test]
fn a_narrow_tables_columns_share_the_rest_equally() {
    // 2 table (120 wide), 3 tr, 4 td, 5 a wrapping row of three 20 px boxes,
    // 9 td, 10 one of four: the columns are 22 to 62 and 22 to 82 px.
    let wrap = |n: usize| {
        let boxes = (0..n).map(|_| N::Box(20.0, 10.0)).collect();
        N::Tag(
            "td",
            Vec::new(),
            Vec::new(),
            vec![N::Tag("div", Vec::new(), Vec::new(), boxes)],
        )
    };
    let mut k = lay_out(N::Tag(
        "table",
        vec![(StyleId::Width, 120.0)],
        Vec::new(),
        vec![tag("tr", vec![wrap(3), wrap(4)])],
    ));
    let mut ops = Vec::new();
    for id in [5u32, 10] {
        let mut s = StyleProps::default();
        for (row, value) in [(StyleId::Display, "flex"), (StyleId::FlexWrap, "wrap")] {
            s.set_dynamic(row, &StyleValue::Text(value.into())).unwrap();
        }
        ops.push(Op::SetStyle {
            id,
            patch: Box::new(s),
        });
    }
    k.apply(0, 2, &ops).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(
        frame(&k, 2)[2],
        120.0,
        "border-box, as HTML's UA sheet has it"
    );
    assert_eq!(frame(&k, 4), [2.0, 2.0, 57.0, 22.0], "22 + 35");
    assert_eq!(frame(&k, 9), [61.0, 2.0, 57.0, 22.0], "22 + 35");
}
