//! The base scale in the terminal (LLP 1121 stage 11 B2): the host says its
//! column, a base spacing lands on the cell grid, and a base radius is 0.
use super::*;
use exact_kernel::style::{metrics, scale};
use exact_kernel::StyleId;

#[test]
fn a_base_spacing_is_cells_and_a_base_radius_is_none() {
    let plan = contract::compile_terminal(
        "component App\n  view\n    column testId=\"box\" padding=\"space.lg\" border-radius=\"radius.md\" gap=\"space.sm\"\n      text \"a\" testId=\"a\"\n      text \"b\" testId=\"b\" border-radius=\"radius.full\"\n",
    )
    .unwrap();
    let host = Host::boot(plan, (), Mode::Fullscreen, 80, 24).unwrap();
    assert_eq!(metrics::column(), metrics::Column::Terminal);
    // The kernel's table: space.lg is two cells, space.sm one; radii 0.
    assert_eq!(
        scale::of("space.lg", metrics::Column::Terminal),
        Some(scale::ScaleValue::Cells(2.0))
    );
    let k = host.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let computed = |id: &str, s: StyleId| format!("{:?}", node(id).computed(s));
    // Two cells: two columns (16 px) inline, a row's worth (16 px) on top, on the grid.
    assert!(
        computed("box", StyleId::PaddingLeft).contains("16.0"),
        "{}",
        computed("box", StyleId::PaddingLeft)
    );
    assert!(
        computed("box", StyleId::PaddingTop).contains("16.0"),
        "{}",
        computed("box", StyleId::PaddingTop)
    );
    for (id, row) in [
        ("box", StyleId::BorderRadiusTopLeft),
        ("b", StyleId::BorderRadiusTopLeft),
    ] {
        let r = computed(id, row);
        assert!(r.contains("0.0") && !r.contains("9999"), "{id}: {r}");
    }
}

/// LLP 1121 stage 11 (c): the kernel lays a terminal's spaces out in whole
/// cells, so px and the painter's rows agree. A list with gap space.sm (half
/// a row) puts item k on row k, where its px frame is; revealing item 30 in
/// a ten-row scroller brings it on screen.
#[test]
fn spaces_are_whole_cells_so_rows_follow_the_layout() {
    let mut src = String::from(
        "component App\n  view\n    column\n      scroll testId=\"list\" height=\"10lh\"\n        column gap=\"space.sm\" padding=\"space.sm\"\n",
    );
    for k in 0..40 {
        src += &format!("          text \"item {k}\" testId=\"i{k}\"\n");
    }
    let plan = contract::compile_terminal(&src).unwrap();
    let mut host = Host::boot(plan, (), Mode::Fullscreen, 40, 12).unwrap();
    let top = host.cells_of(host.by_test_id("list").unwrap()).unwrap().y;
    for k in [0, 1, 7, 30] {
        let id = host.by_test_id(&format!("i{k}")).unwrap();
        let frame = host.kernel().node(id).unwrap().frame;
        let cells = host.cells_of(id).unwrap();
        // Its row is the px layout's, and no blank row between items.
        assert_eq!(frame.y, (top as f32 + k as f32) * 16.0, "item {k}");
        assert_eq!((cells.y, cells.h), (top + k, 1), "item {k}");
    }
    let shown = |host: &mut Host<()>| -> Vec<String> {
        let text = host.frame().grid.text();
        text.lines()
            .map(|l| l.trim_end_matches(['│', '┃', ' ']).trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    };
    let rows: Vec<String> = (0..10).map(|k| format!("item {k}")).collect();
    assert_eq!(shown(&mut host), rows);
    // The least move that shows it: items 21 to 30, 30 on the last row.
    host.reveal_to(host.by_test_id("i30").unwrap(), "nearest");
    let rows: Vec<String> = (21..31).map(|k| format!("item {k}")).collect();
    assert_eq!(shown(&mut host), rows);
}
