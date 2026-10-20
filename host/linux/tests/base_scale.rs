//! The base scale on the Linux desktop and Android's reader (LLP 1121 stage
//! 11 B2), through each one's own column switch: the desktop paints the
//! neutral look (`show_neutral_look`, the role table's switch), the reader
//! Material (`show_material_look`, the metrics' column). A base name reads
//! that column's cell of the kernel's table. One test: the switches are the
//! process's.
use exact_kernel::style::{metrics, scale, theme};
use exact_kernel::{Kernel, StyleId};
use exact_runner::{DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        source: &str,
        _: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn padding(src: &str) -> String {
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let key = r.kernel().find_by_test_id("box")[0];
    format!(
        "{:?}",
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .computed(StyleId::PaddingTop)
    )
}

#[test]
fn the_desktop_reads_the_neutral_column_and_the_reader_the_material_one() {
    let src =
        "component App\n  view\n    column testId=\"box\" padding=\"space.xs\"\n      text \"a\"\n";
    let length = |k: theme::Kind, _| k == theme::Kind::Length;
    let every = |column: metrics::Column| {
        for name in scale::names() {
            let cell = scale::cell(scale::row(name).unwrap(), column);
            assert_eq!(
                theme::measure(name, length).map(|(_, v)| v.to_string()),
                Some(scale::css(cell.value)),
                "{name} in {column:?}"
            );
        }
    };
    exact_linux::app::show_neutral_look();
    assert_eq!(metrics::column(), metrics::Column::Neutral);
    every(metrics::Column::Neutral);
    let px = |column| match scale::of("space.xs", column) {
        Some(scale::ScaleValue::Px(n)) => format!("{n:?}"),
        other => panic!("{other:?}"),
    };
    let neutral = px(metrics::Column::Neutral);
    assert!(
        padding(src).contains(&format!("({neutral})")),
        "{} vs {neutral}",
        padding(src)
    );
    exact_linux::app::show_material_look();
    assert_eq!(metrics::column(), metrics::Column::Material);
    every(metrics::Column::Material);
    // Material's cells are Material 3's (the values may coincide with the neutral look's own steps).
    let xs = scale::row("space.xs").unwrap();
    assert_eq!(
        xs.material.source,
        scale::ScaleSource::Guideline("m3-spacing")
    );
    assert_eq!(
        xs.neutral.source,
        scale::ScaleSource::Guideline("neutral-steps")
    );
    let material = px(metrics::Column::Material);
    assert!(
        padding(src).contains(&format!("({material})")),
        "{} vs {material}",
        padding(src)
    );
}
