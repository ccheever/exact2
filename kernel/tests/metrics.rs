//! The metric table's switch (LLP 1115.002 D6), thrown: a binary of its
//! own, because the column is the process's and every other test of the
//! kernel reads a metric in the web's. One test, so nothing here reads a
//! metric while the switch is moving.
use exact_kernel::style::metrics::{self, Column};
use exact_kernel::style::{roles, Color, StyleValue};
use exact_kernel::{Appearance, ColorValue, Dimension, StyleId, StyleProps};

/// One sheet, read on each platform: a row given a metric's name holds the
/// value of the column its host said, and a metric with no row on a
/// platform leaves the row unset there.
#[test]
fn a_host_that_says_its_platform_reads_that_platforms_column() {
    let sheet = [
        (StyleId::MinHeight, "row-min-height"),
        (StyleId::PaddingTop, "row-padding-block"),
        (StyleId::MaxWidth, "section-max-width"),
        (StyleId::BackgroundColor, "card-fill"),
        (StyleId::BorderColorBottom, "separator"),
        (StyleId::TextColor, "destructive-color"),
        (StyleId::Appearance, "button-appearance"),
        (StyleId::TextAlign, "field-align"),
    ];
    let read = || {
        let mut style = StyleProps::default();
        for (row, name) in sheet {
            let name = StyleValue::Text(format!("-exact-grouped-{name}").into());
            style.set_dynamic(row, &name).unwrap();
        }
        style
    };
    let role = |name: &str| ColorValue::Role(roles::role(name).unwrap());
    let pair = |light, dark| ColorValue::LightDark(Color(light), Color(dark));

    assert_eq!(
        metrics::column(),
        Column::Web,
        "a process starts on the web's"
    );
    let web = read();
    assert_eq!(web.min_height, Dimension::Points(52.0));
    assert!(!web.mask.has(StyleId::PaddingTop), "no row on the web");
    assert!(!web.mask.has(StyleId::MaxWidth));
    assert!(!web.mask.has(StyleId::TextAlign));
    assert_eq!(web.background_color, Some(pair(0xffff_ffff, 0x1c1c_1eff)));
    assert_eq!(web.text_color, role("-exact-system-red"));
    assert_eq!(web.appearance, Appearance::None);

    metrics::set_column(Column::Mac);
    assert_eq!(metrics::column(), Column::Mac);
    let mac = read();
    assert_eq!(mac.min_height, Dimension::Points(16.0));
    assert_eq!(mac.padding_top, Dimension::Points(10.0));
    assert_eq!(mac.max_width, Dimension::Points(744.0));
    assert_eq!(mac.background_color, Some(pair(0xf9f9_f9ff, 0x2727_27ff)));
    assert_eq!(mac.border_color_bottom, pair(0xefef_efff, 0x3333_33ff));
    assert_eq!(mac.text_color, role("-exact-label"));
    assert_eq!(mac.appearance, Appearance::Auto);
    assert!(mac.mask.has(StyleId::TextAlign));
    assert!(
        mac.relative.is_empty(),
        "each is a value, read once: nothing to resolve again"
    );

    let card = roles::role("-exact-secondary-grouped-background").unwrap();
    let separator = roles::role("-exact-separator").unwrap();
    let card_theme = pair(0xffe0_b0ff, 0x4422_00ff);
    let separator_theme = pair(0xff00_00ff, 0xff55_55ff);
    roles::set_overrides(vec![(card, card_theme), (separator, separator_theme)]);
    let mut themed = read();
    assert_eq!(
        (
            themed.background_color.map(ColorValue::themed),
            themed.border_color_bottom.themed()
        ),
        (Some(card_theme), separator_theme),
        "the Mac's compiler-owned surfaces follow the replaced roles"
    );
    themed
        .set_dynamic(
            StyleId::BackgroundColor,
            &StyleValue::Text("#123456".into()),
        )
        .unwrap();
    assert_eq!(
        themed.background_color,
        Some(ColorValue::Fixed(Color(0x1234_56ff)))
    );
    roles::set_overrides(Vec::new());
    let restored = read();
    assert_eq!(restored.background_color, mac.background_color);
    assert_eq!(restored.border_color_bottom, mac.border_color_bottom);

    // iOS's is the web's, cell for cell (neither moved when the sheet's
    // numbers became rows), so a sheet read there is the one read above.
    metrics::set_column(Column::Ios);
    let ios = read();
    assert_eq!(ios.min_height, web.min_height);
    assert_eq!(ios.background_color, web.background_color);
    assert_eq!(ios.text_color, web.text_color);
    assert_eq!(ios.appearance, web.appearance);
    assert_eq!(ios.mask, web.mask);

    metrics::set_column(Column::Web);
}
