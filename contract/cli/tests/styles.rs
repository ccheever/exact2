//! LLP 1017 P6: named styles, proven on the kernel's rows after boot.

use exact_kernel::{Color, Dimension, Kernel};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn a_class_applies_its_style_and_the_nodes_own_attribute_wins() {
    let plan = contract::compile(&corpus("styles.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let style_of = |id: &str| {
        let key = k.find_by_test_id(id)[0];
        k.node_by_key(key).unwrap().style.clone()
    };
    let card = style_of("card");
    assert_eq!(card.padding_top, Dimension::Points(16.0));
    assert_eq!(card.padding_left, Dimension::Points(16.0));
    assert_eq!(card.border_radius_top_left, 16.0);
    assert_eq!(card.row_gap, 10.0);
    assert_eq!(
        card.background_color,
        Color::parse_hex("#ffffffd9").unwrap().into()
    );
    let tight = style_of("tight");
    assert_eq!(tight.padding_top, Dimension::Points(4.0));
    assert_eq!(tight.padding_bottom, Dimension::Points(4.0));
    assert_eq!(tight.border_radius_top_left, 16.0);
    assert_eq!(
        tight.background_color,
        Color::parse_hex("#000000").unwrap().into()
    );
}

#[test]
fn a_style_is_refused_with_the_css_name_for_an_old_spelling() {
    let src =
        "style Card\n  radius=16\ncomponent A\n  view\n    column class=Card\n      text \"a\"\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "lower-unknown-attr");
    assert!(e.message.contains("`border-radius`"), "{e}");
}

#[test]
fn attribute_typos_suggest_only_unambiguous_accepted_spellings() {
    for (typo, correct, value) in [
        ("widht", "width", "100"),
        ("align-item", "align-items", "\"center\""),
        ("paddng", "padding", "12"),
        ("opaccity", "opacity", "0.5"),
        ("backgrounf-color", "background-color", "\"#123456\""),
    ] {
        for style in [false, true] {
            let source = if style {
                format!(
                    "style Card\n  {typo}={value}\ncomponent App\n  view\n    view class=Card\n"
                )
            } else {
                format!("component App\n  view\n    view {typo}={value}\n")
            };
            let error = contract::compile(&source).unwrap_err();
            assert_eq!(error.id, "lower-unknown-attr");
            assert!(
                error
                    .message
                    .ends_with(&format!("; did you mean `{correct}`?")),
                "{error}"
            );
            assert_eq!(error.span.line, if style { 2 } else { 3 });
            contract::compile(&source.replace(typo, correct)).unwrap();
        }
    }
    for name in [
        "overflow-z",
        "unrelated-property",
        "éwidth",
        "xy",
        &"w".repeat(65),
    ] {
        let source = format!("component App\n  view\n    view {name}=1\n");
        let error = contract::compile(&source).unwrap_err();
        assert!(!error.message.contains("did you mean"), "{error}");
    }
    let error = contract::compile(
        "style Card\n  tesId=\"x\"\ncomponent App\n  view\n    view class=Card\n",
    )
    .unwrap_err();
    assert!(
        !error.message.contains("did you mean"),
        "style must not suggest props: {error}"
    );
    let error = contract::compile("component App\n  view\n    view tesId=\"x\"\n").unwrap_err();
    assert!(error.message.contains("did you mean `testId`"), "{error}");
}
