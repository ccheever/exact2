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
