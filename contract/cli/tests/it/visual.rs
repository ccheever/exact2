//! LLP 1077 D1–D3: `corner-shape`, `mask-image` and `text-shadow` reach the
//! kernel as CSS, in a literal, a `style` and a conditional, and what the
//! hosts do not draw is refused at compile time, by name.

use exact_kernel::corner::{Corner, CornerShape};
use exact_kernel::gradient::BackgroundImage;
use exact_kernel::style::TextShadow;
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn style_of(r: &Runner<NoData>, id: &str) -> exact_kernel::StyleProps {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .style
        .clone()
}

fn refused(attrs: &str) -> contract::CompileError {
    contract::compile(&format!("component App\n  view\n    view {attrs}\n")).unwrap_err()
}

#[test]
fn the_three_rows_take_css_in_a_literal_a_style_and_a_conditional() {
    let r = boot(
        "style Card\n  corner-shape=\"-apple-continuous\"\n  mask-image=\"linear-gradient(#000, transparent)\"\n\ncomponent App\n  state on = true\n  view\n    column\n      view class=Card testId=\"a\"\n      view corner-shape=(on ? \"squircle bevel\" : \"round\") testId=\"b\"\n      text \"x\" text-shadow=(on ? \"1px 2px 3px #000\" : \"none\") testId=\"c\"\n",
    );
    let a = style_of(&r, "a");
    assert!(a.corner_shape.is_apple_continuous());
    assert_eq!(
        a.mask_image,
        BackgroundImage::parse("linear-gradient(#000, transparent)").unwrap()
    );
    let b = style_of(&r, "b");
    assert_eq!(b.corner_shape.0[1], Corner::Superellipse(0.0));
    assert_eq!(
        b.corner_shape,
        CornerShape::check("squircle bevel").unwrap()
    );
    let c = style_of(&r, "c");
    assert_eq!(
        c.text_shadow,
        TextShadow::check("1px 2px 3px #000").unwrap()
    );
}

#[test]
fn what_no_host_draws_is_refused_by_name() {
    for (attr, says) in [
        ("corner-shape=\"circle\"", "squircle"),
        (
            "corner-shape=\"round round round round round\"",
            "one to four",
        ),
        ("mask-image=\"url(a.png)\"", "an image"),
        (
            "mask-image=\"linear-gradient(#000, #fff), linear-gradient(#fff, #000)\"",
            "several",
        ),
        ("text-shadow=\"1px 1px #000, 2px 2px #fff\"", "one shadow"),
        ("text-shadow=\"1px 1px 2px 3px #000\"", "no spread"),
    ] {
        let e = refused(attr);
        assert_eq!(e.id, "lower-attr-value", "{attr}: {e}");
        assert!(e.message.contains(says), "{attr}: {e}");
    }
}

/// LLP 1077 §5: the declared rows Apple draws, their refusals named, and
/// `haptic()` a host command.
#[test]
fn apples_affordances_are_declared_rows_and_haptic_is_a_command() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/fixtures/affordances.contract"
    ))
    .unwrap();
    let mut r = boot(&src);
    let palette = style_of(&r, "palette");
    assert_eq!(
        palette.symbol_rendering,
        exact_kernel::SymbolRendering::Palette
    );
    assert_eq!(palette.symbol_palette.0.len(), 2);
    assert_eq!(style_of(&r, "value").symbol_value, 0.4);
    assert_eq!(style_of(&r, "mono").symbol_value, -1.0);
    // A role now (LLP 1095 D2): the platform's colour, its pair the fallback.
    let secondary = style_of(&r, "secondary").text_color;
    assert!(matches!(secondary, exact_kernel::ColorValue::Role(_)));
    assert_eq!(
        secondary.fallback(),
        exact_kernel::ColorValue::parse_light_dark("light-dark(#3c3c4399, #ebebf599)").unwrap()
    );
    let k = r.kernel();
    let tap = k.node_by_key(k.find_by_test_id("tap")[0]).unwrap().id;
    r.dispatch(tap, exact_runner::Event::Press).unwrap();
    assert!(r.take_commands().iter().any(|c| c.name == "haptic"));
    for (attr, says) in [
        ("symbol-value=2", "0 to 1"),
        ("symbol-palette=\"#000 #111 #222 #333\"", "one to three"),
        ("press-haptic=\"buzz\"", "selection"),
    ] {
        let e = refused(attr);
        assert!(e.message.contains(says), "{attr}: {e}");
    }
}

#[test]
fn a_vendor_prefixed_name_is_an_attribute_before_its_equals_and_a_negation_elsewhere() {
    // LLP 1077 D7: glued to `=` or spaced from it, as any attribute may be.
    let r = boot(
        "component App\n  state w = 3\n  view\n    column\n      text \"a\" -webkit-text-stroke=\"2px #ff0000\" testId=\"a\"\n      text \"b\" -webkit-text-stroke-width = w testId=\"b\"\n      text `${10 -w}` testId=\"c\"\n",
    );
    let a = style_of(&r, "a");
    assert_eq!(a.text_stroke_width, 2.0);
    assert!(a.text_stroke_color.is_some());
    assert_eq!(style_of(&r, "b").text_stroke_width, 3.0);
}
