//! The role table's switch (LLP 1115.000 D3.2), thrown: a binary of its
//! own, because the column is the process's and every other test of the
//! kernel reads a role's fallback beside it. One test, so nothing here
//! reads a fallback while the switch is moving.
use exact_kernel::gradient::color_css;
use exact_kernel::style::roles::{self, Column};
use exact_kernel::style::Color;
use exact_kernel::ColorValue;

/// A host that throws the switch shows the neutral column wherever a role
/// falls back: what the kernel resolves itself and the pair an Exact role
/// carries to the browser. A reported colour still wins, and a colour that
/// names no role is untouched.
#[test]
fn a_host_that_throws_the_switch_shows_the_neutral_column() {
    let role = |name: &str| ColorValue::Role(roles::role(name).unwrap());
    let css = |c: ColorValue| {
        let mut out = String::new();
        color_css(&mut out, c);
        out
    };
    let (separator, accent) = (role("-exact-separator"), role("AccentColor"));
    let authored = ColorValue::parse_light_dark("light-dark(#010203, #040506)").unwrap();

    assert_eq!(
        roles::column(),
        Column::Apple,
        "a process starts on Apple's"
    );
    assert_eq!(separator.resolve(false), Color(0x3c3c_431f));
    assert_eq!(separator.resolve(true), Color(0x5454_5880));
    assert_eq!(
        css(separator),
        "var(--exact-separator, light-dark(#3c3c431f, #54545880))"
    );

    roles::set_column(Column::Neutral);
    assert_eq!(roles::column(), Column::Neutral);
    assert_eq!(separator.resolve(false), Color(0x0000_001f));
    assert_eq!(separator.resolve(true), Color(0xffff_ff29));
    assert_eq!(
        separator.fallback(),
        ColorValue::LightDark(Color(0x0000_001f), Color(0xffff_ff29))
    );
    assert_eq!(accent.resolve(false), Color(0x0b66_d6ff));
    assert_eq!(accent.resolve(true), Color(0x68a5_feff));
    assert_eq!(
        css(separator),
        "var(--exact-separator, light-dark(#0000001f, #ffffff29))",
        "the browser's fallback for the role is the column's"
    );
    assert_eq!(
        css(role("CanvasText")),
        "CanvasText",
        "a CSS system colour is the browser's own, in either column"
    );
    assert_eq!(
        ColorValue::parse_light_dark(&css(separator)),
        Some(separator),
        "and it reads back as the role"
    );
    assert_eq!(authored.resolve(true), Color(0x0405_06ff), "no role");

    // What a host reports is the platform's, over either column.
    roles::set_reported([(separator, true, Color(0xff00_00ff))]);
    assert_eq!(separator.resolve(true), Color(0xff00_00ff));
    assert_eq!(separator.resolve(false), Color(0x0000_001f), "unreported");
    roles::set_reported([]);

    // The switch is the look's whole (LLP 1115.000 D17): the metrics follow
    // the colours. The kernel sizes a control nobody measured as the page's
    // sheet does, by its font size, and a sheet's metric reads its neutral
    // cell.
    use exact_kernel::style::{metrics, StyleValue};
    use exact_kernel::{ControlKind, Dimension, StyleId, StyleProps};
    assert_eq!(metrics::column(), metrics::Column::Neutral);
    assert_eq!(ControlKind::Checkbox.default_size(16.0), (20.0, 20.0));
    assert_eq!(ControlKind::Radio.default_size(20.0), (25.0, 25.0));
    assert_eq!(ControlKind::Switch.default_size(16.0), (36.0, 20.0));
    assert_eq!(ControlKind::Range.default_size(16.0), (129.0, 20.0));
    let read = |row, name: &str| {
        let mut style = StyleProps::default();
        let name = StyleValue::Text(format!("-exact-grouped-{name}").into());
        style.set_dynamic(row, &name).unwrap();
        style
    };
    assert_eq!(
        read(StyleId::BackgroundColor, "ground").background_color,
        Some(role("-exact-background"))
    );
    let radius = |style: StyleProps| style.border_radius_top_left;
    assert_eq!(
        radius(read(StyleId::BorderRadiusTopLeft, "card-radius")),
        Dimension::Points(12.0)
    );

    roles::set_column(Column::Apple);
    assert_eq!(separator.resolve(false), Color(0x3c3c_431f));
    assert_eq!(metrics::column(), metrics::Column::Web);
    assert_eq!(ControlKind::Checkbox.default_size(16.0), (13.0, 13.0));
    assert_eq!(ControlKind::Switch.default_size(16.0), (38.0, 22.0));
    assert_eq!(ControlKind::Range.default_size(16.0), (129.0, 16.0));
    assert_eq!(
        radius(read(StyleId::BorderRadiusTopLeft, "card-radius")),
        Dimension::Points(26.0)
    );
}
