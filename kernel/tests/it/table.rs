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

/// A table's own `border-spacing` replaces the UA sheet's 2px, between its
/// cells and around them (CSS 2.1 §17.6.1): 0 sets them flush, as a ruled
/// table wants, and 6 spaces them as Chrome does.
#[test]
fn a_tables_border_spacing_replaces_the_ua_sheets() {
    for (spacing, table, second) in [
        (0.0, [0.0, 0.0, 44.0, 12.0], [22.0, 0.0, 22.0, 12.0]),
        (6.0, [0.0, 0.0, 62.0, 24.0], [34.0, 6.0, 22.0, 12.0]),
    ] {
        // 2 table, 3 tr, 4 td, 5 box, 6 td.
        let k = lay_out(N::Tag(
            "table",
            vec![(StyleId::BorderSpacing, spacing)],
            Vec::new(),
            vec![tag(
                "tr",
                vec![cell(&[], 20.0, 10.0), cell(&[], 20.0, 10.0)],
            )],
        ));
        assert_eq!(frame(&k, 2), table, "the table at {spacing}");
        assert_eq!(frame(&k, 6), second, "its second cell at {spacing}");
    }
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

/// The distance of the kernel's columns from Chrome's, which conformance
/// measured on the fixture `table-widths` (Chrome 156, 2026-10-30), to the
/// hundredth of a pixel: what the declared deviation is (LLP 1153 D7.3).
fn from_chrome(grid: [f32; 2], chrome: [f32; 2]) -> [f32; 2] {
    [0, 1].map(|i| ((grid[i] - chrome[i]) * 100.0).round() / 100.0)
}

/// The declared deviation: a spanning cell wider than the columns it spans
/// gives them its extra width equally, as a grid's `auto` tracks do, where
/// Chrome's table gives it in proportion to their widths.
#[test]
fn a_spans_extra_width_is_shared_equally() {
    // 2 table, 3 tbody, 4 tr, 5 th, 6 box (50), 7 th, 8 box (30), 9 tr, 10 td (colspan 2), 11 box (90).
    let th = |w: f64| N::Tag("th", Vec::new(), Vec::new(), vec![N::Box(w, 12.0)]);
    let k = lay_out(tag(
        "table",
        vec![tag(
            "tbody",
            vec![
                tag("tr", vec![th(50.0), th(30.0)]),
                tag("tr", vec![cell(&[(COLSPAN, 2)], 90.0, 12.0)]),
            ],
        )],
    ));
    let grid = [frame(&k, 5)[2], frame(&k, 7)[2]];
    assert_eq!(grid, [55.0, 35.0], "52 and 32, and 3 each");
    assert_eq!(from_chrome(grid, [55.70, 34.30]), [-0.70, 0.70]);
}

/// The declared deviation: a table narrower than its content gives each
/// column its least and shares the rest equally, as a grid's `auto` tracks
/// do, where Chrome's table shares it in proportion to what each column
/// could still grow.
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
    let grid = [frame(&k, 4)[2], frame(&k, 9)[2]];
    assert_eq!(from_chrome(grid, [50.0, 64.0]), [7.0, -7.0]);
}

/// The first `thead` is above the other row groups and the first `tfoot`
/// below them wherever each is written; a later one is a row group where it
/// stands (CSS 2.1 §17.2, as the browser lays them out).
#[test]
fn the_first_header_and_footer_go_first_and_last() {
    // 2 table; then each group, its row, its cell and its box: 3 tfoot,
    // 7 tbody, 11 thead, 15 tfoot, 19 thead (the cells 5, 9, 13, 17, 21).
    let group = |name: &'static str| tag(name, vec![tag("tr", vec![cell(&[], 20.0, 10.0)])]);
    let k = lay_out(tag(
        "table",
        ["tfoot", "tbody", "thead", "tfoot", "thead"]
            .map(group)
            .into(),
    ));
    let y = |id| frame(&k, id)[1];
    assert_eq!([13, 9, 17, 21, 5].map(y), [2.0, 16.0, 30.0, 44.0, 58.0]);
}

/// A `rowspan` of 0 is the rest of its row group (HTML §4.9.11), however
/// many rows that comes to: a row the group gains is spanned too.
#[test]
fn a_rowspan_of_0_reaches_the_end_of_its_row_group() {
    // 2 table, 3 tbody, 4 tr, 5 td (rowspan 0), 6 box, 7 td, 8 box, 9 tr,
    // 10 td, 11 box; then 12 tr, 13 td, 14 box.
    let mut k = lay_out(tag(
        "table",
        vec![tag(
            "tbody",
            vec![
                tag(
                    "tr",
                    vec![cell(&[(ROWSPAN, 0)], 20.0, 10.0), cell(&[], 20.0, 10.0)],
                ),
                tag("tr", vec![cell(&[], 20.0, 10.0)]),
            ],
        )],
    ));
    assert_eq!(frame(&k, 5), [2.0, 2.0, 22.0, 26.0], "two rows");
    assert_eq!(
        frame(&k, 10),
        [26.0, 16.0, 22.0, 12.0],
        "in the second column"
    );
    let (mut ops, mut next) = (Vec::new(), 12);
    let added = make(&tag("tr", vec![cell(&[], 20.0, 10.0)]), &mut next, &mut ops);
    ops.push(Op::SetChildren {
        id: 3,
        children: vec![4, 9, added],
    });
    k.apply(0, 2, &ops).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
    assert_eq!(frame(&k, 5), [2.0, 2.0, 22.0, 40.0], "three rows");
    assert_eq!(
        frame(&k, 13),
        [26.0, 30.0, 22.0, 12.0],
        "in the second column"
    );
}

/// A caption keeps what its author said: a width, and a margin, which is
/// from the table's edge as CSS's caption's is.
#[test]
fn a_caption_keeps_its_authors_width_and_margin() {
    // 2 table, 3 caption (30 wide, 12 below), 4 box, 5 tr, 6 td, 7 box.
    let k = lay_out(tag(
        "table",
        vec![
            N::Tag(
                "caption",
                vec![(StyleId::Width, 30.0), (StyleId::MarginBottom, 12.0)],
                Vec::new(),
                vec![N::Box(10.0, 10.0)],
            ),
            tag("tr", vec![cell(&[], 60.0, 10.0)]),
        ],
    ));
    assert_eq!(frame(&k, 3), [0.0, 0.0, 30.0, 10.0], "the caption");
    assert_eq!(
        frame(&k, 6),
        [2.0, 24.0, 62.0, 12.0],
        "12 below it, then the spacing"
    );
    assert_eq!(frame(&k, 2), [0.0, 0.0, 66.0, 38.0]);
}

/// A `dd` keeps its 40 px start margin beside an end margin its author said.
#[test]
fn a_dds_start_margin_stays_beside_its_end_margin() {
    // 2 dl, 3 dt, 4 box, 5 dd (12 at its end), 6 box.
    let k = lay_out(tag(
        "dl",
        vec![
            tag("dt", vec![N::Box(30.0, 10.0)]),
            N::Tag(
                "dd",
                vec![(StyleId::MarginRight, 12.0)],
                Vec::new(),
                vec![N::Box(30.0, 10.0)],
            ),
        ],
    ));
    assert_eq!(frame(&k, 5), [40.0, 26.0, 348.0, 10.0]);
}

/// Rows its author writes on node `id` after it is made, keyword rows as
/// text; then a layout.
fn restyle(k: &mut Kernel, epoch: u64, id: u32, rows: &[(StyleId, &str)]) {
    let mut s = StyleProps::default();
    for (row, value) in rows {
        let value = match value.parse::<f64>() {
            Ok(n) => StyleValue::Number(n),
            Err(_) => StyleValue::Text((*value).into()),
        };
        s.set_dynamic(*row, &value).unwrap();
        s.written.set(*row);
    }
    let patch = Box::new(s);
    k.apply(0, epoch, &[Op::SetStyle { id, patch }]).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
}

/// A table whose author says `display: block` is a block, as the browser
/// makes it, and its parts are an anonymous table inside it (CSS 2.2
/// §17.2.1): the same rows and cells, spaced and placed as a table's.
#[test]
fn a_table_whose_author_says_block_holds_an_anonymous_table() {
    // 2 table; 3 thead, 4 tr, 5/7 td; 9 tbody, 10 tr, 11/13 td.
    let two = || tag("tr", vec![cell(&[], 20.0, 10.0), cell(&[], 20.0, 10.0)]);
    let mut k = lay_out(tag(
        "table",
        vec![tag("thead", vec![two()]), tag("tbody", vec![two()])],
    ));
    let parts = |k: &Kernel| [3, 5, 7, 9, 11, 13].map(|id| frame(k, id));
    let table = parts(&k);
    assert_eq!(frame(&k, 2), [0.0, 0.0, 50.0, 30.0], "a table");
    restyle(&mut k, 2, 2, &[(StyleId::Display, "block")]);
    assert_eq!(frame(&k, 2), [0.0, 0.0, 400.0, 30.0], "a block");
    assert_eq!(parts(&k), table, "its parts where the table had them");
    restyle(&mut k, 3, 2, &[(StyleId::PaddingLeft, "10")]);
    let moved = table.map(|[x, y, w, h]| [x + 10.0, y, w, h]);
    assert_eq!(parts(&k), moved, "inside the block's own padding");
}

/// A table's height is its least (CSS 2.2 §17.5.3): taller content makes the
/// table taller, and the box after it follows; a taller height is shared
/// among its rows.
#[test]
fn a_tables_height_is_its_least() {
    // 2 view; 3 table (10 tall), 4 tr, 5 td, 6 box (40 tall); 7 the box after.
    let k = lay_out(tag(
        "view",
        vec![
            N::Tag(
                "table",
                vec![(StyleId::Height, 10.0)],
                Vec::new(),
                vec![tag("tr", vec![cell(&[], 20.0, 40.0)])],
            ),
            N::Box(20.0, 10.0),
        ],
    ));
    assert_eq!(frame(&k, 3), [0.0, 0.0, 26.0, 46.0], "as tall as its row");
    assert_eq!(frame(&k, 5), [2.0, 2.0, 22.0, 42.0]);
    assert_eq!(frame(&k, 7)[1], 46.0, "the box after it");
    // 2 table (60 tall), 3/6 tr, 4/7 td: two rows of 12 share 54.
    let k = lay_out(N::Tag(
        "table",
        vec![(StyleId::Height, 60.0)],
        Vec::new(),
        vec![
            tag("tr", vec![cell(&[], 20.0, 10.0)]),
            tag("tr", vec![cell(&[], 20.0, 10.0)]),
        ],
    ));
    assert_eq!(frame(&k, 2)[3], 60.0);
    assert_eq!(frame(&k, 4), [2.0, 2.0, 22.0, 27.0]);
    assert_eq!(frame(&k, 7), [2.0, 31.0, 22.0, 27.0]);
}

/// A row or row group moved relatively moves its cells with it, as the
/// browser moves a row's (CSS 2.2 §9.4.3).
#[test]
fn a_relatively_moved_row_or_group_moves_its_cells() {
    // 2 table, 3 tbody, 4 tr, 5 td, 6 box, 7 tr, 8 td, 9 box.
    let mut k = lay_out(tag(
        "table",
        vec![tag(
            "tbody",
            vec![
                tag("tr", vec![cell(&[], 20.0, 10.0)]),
                tag("tr", vec![cell(&[], 20.0, 10.0)]),
            ],
        )],
    ));
    let before = [4, 5, 6, 7, 8, 9].map(|id| frame(&k, id));
    let by = |f: [f32; 4], dx: f32, dy: f32| [f[0] + dx, f[1] + dy, f[2], f[3]];
    let relative = (StyleId::PositionType, "relative");
    restyle(&mut k, 2, 4, &[relative, (StyleId::Left, "10")]);
    assert_eq!(frame(&k, 4), by(before[0], 10.0, 0.0), "the row");
    assert_eq!(frame(&k, 5), by(before[1], 10.0, 0.0), "its cell");
    assert_eq!(
        frame(&k, 6),
        by(before[2], 10.0, 0.0),
        "what the cell holds"
    );
    assert_eq!(frame(&k, 8), before[4], "the other row's cell");
    restyle(&mut k, 3, 3, &[relative, (StyleId::Top, "5")]);
    assert_eq!(
        frame(&k, 5),
        by(before[1], 10.0, 5.0),
        "with the group's too"
    );
    assert_eq!(
        frame(&k, 8),
        by(before[4], 0.0, 5.0),
        "the group's other cell"
    );
}

/// A caption follows its table's padding as it changes: its margins and
/// width are the table's padding box's.
#[test]
fn a_caption_follows_its_tables_padding() {
    // 2 table, 3 caption, 4 box, 5 tr, 6 td, 7 box.
    let mut k = lay_out(tag(
        "table",
        vec![
            tag("caption", vec![N::Box(10.0, 10.0)]),
            tag("tr", vec![cell(&[], 60.0, 10.0)]),
        ],
    ));
    assert_eq!(frame(&k, 3), [0.0, 0.0, 66.0, 10.0]);
    let sides = [(StyleId::PaddingLeft, "20"), (StyleId::PaddingRight, "20")];
    restyle(&mut k, 2, 2, &sides);
    assert_eq!(frame(&k, 2)[2], 106.0, "20 + 2 + 62 + 2 + 20");
    assert_eq!(frame(&k, 3), [0.0, 0.0, 106.0, 10.0], "the padding box");
    assert_eq!(frame(&k, 6)[0], 22.0);
}

/// Right to left, with padding on one side only, the caption still spans
/// the table's padding box: its margins cancel the sides the padding is on.
#[test]
fn a_right_to_left_captions_margins_cancel_its_tables_own_sides() {
    // 2 table, 3 caption, 4 box, 5 tr, 6 td, 7 box.
    let mut k = lay_out(tag(
        "table",
        vec![
            tag("caption", vec![N::Box(10.0, 10.0)]),
            tag("tr", vec![cell(&[], 60.0, 10.0)]),
        ],
    ));
    let rows = [(StyleId::Direction, "rtl"), (StyleId::PaddingLeft, "10")];
    restyle(&mut k, 2, 2, &rows);
    assert_eq!(frame(&k, 2)[2], 76.0, "10 + 2 + 62 + 2");
    assert_eq!(frame(&k, 3), [0.0, 0.0, 76.0, 10.0], "the padding box");
    assert_eq!(frame(&k, 6)[0], 12.0);
}

/// A caption's percentage margin is of the table's width, as CSS's is of
/// the table wrapper's: 10% of 66 is 6.6.
#[test]
fn a_captions_percentage_margin_is_of_the_tables_width() {
    // 2 table, 3 caption, 4 box, 5 tr, 6 td, 7 box.
    let mut k = lay_out(tag(
        "table",
        vec![
            tag("caption", vec![N::Box(10.0, 10.0)]),
            tag("tr", vec![cell(&[], 60.0, 10.0)]),
        ],
    ));
    restyle(&mut k, 2, 3, &[(StyleId::MarginLeft, "10%")]);
    let [x, _, width, _] = frame(&k, 3);
    assert!(
        (x - 6.6).abs() < 1e-3 && (width - 59.4).abs() < 1e-3,
        "{x} {width}"
    );
}

/// A cell's width and its minimum: the greater is its floor.
#[test]
fn a_cells_width_and_min_width_take_the_greater() {
    // 2 table, 3 tr, 4 td (20 wide, at least 80), 5 box.
    let k = lay_out(tag(
        "table",
        vec![tag(
            "tr",
            vec![N::Tag(
                "td",
                vec![(StyleId::Width, 20.0), (StyleId::MinWidth, 80.0)],
                Vec::new(),
                vec![N::Box(10.0, 10.0)],
            )],
        )],
    ));
    assert_eq!(frame(&k, 4)[2], 82.0, "80 and its padding");
}

/// Right to left, a `dd`'s 40 px start margin is on its right, beside an
/// end margin its author said on its left.
#[test]
fn a_right_to_left_dds_start_margin_is_its_right() {
    // 2 dl, 3 dt, 4 box, 5 dd, 6 box.
    let mut k = lay_out(tag(
        "dl",
        vec![
            tag("dt", vec![N::Box(30.0, 10.0)]),
            tag("dd", vec![N::Box(30.0, 10.0)]),
        ],
    ));
    restyle(&mut k, 2, 2, &[(StyleId::Direction, "rtl")]);
    assert_eq!(frame(&k, 5)[0..3], [0.0, 26.0, 360.0], "the start margin");
    restyle(&mut k, 3, 5, &[(StyleId::MarginLeft, "12")]);
    assert_eq!(frame(&k, 5)[0..3], [12.0, 26.0, 348.0], "beside the end's");
}
