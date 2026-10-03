//! LLP 1064, LLP 1077 D4: `box-shadow` sets its one row; `text-transform` is
//! applied where the kernel produces runs, so what is measured is what a
//! host paints.

use exact_kernel::{Color, ColorValue, Kernel, Offer, StyleProps, Vec2};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
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

fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The first shadow as (colour, offset, blur, count of shadows).
fn shadow(s: &StyleProps) -> (ColorValue, Vec2, f32, f32) {
    let list = s.box_shadow.shadows();
    match list.first() {
        Some(f) => (f.color, f.offset, f.blur, list.len() as f32),
        None => (
            ColorValue::Fixed(Color::TRANSPARENT),
            Vec2 { x: 0.0, y: 0.0 },
            0.0,
            0.0,
        ),
    }
}

fn fixed(hex: &str) -> ColorValue {
    ColorValue::Fixed(Color::parse(hex).unwrap())
}

#[test]
fn box_shadow_sets_its_row_from_a_literal_a_style_a_choice_and_a_template() {
    let mut r = boot(concat!(
        "style Raised\n  box-shadow=\"0 2px 12px rgba(0, 0, 0, 0.2)\"\n",
        "style Flat\n  box-shadow=\"none\"\n",
        "component App\n  state lift = 3\n  state pressed = false\n",
        "  action press\n    pressed = not pressed\n",
        "  view\n    column\n",
        "      view box-shadow=\"#11223380 1px -2px\" testId=\"literal\"\n",
        "      view class=Raised testId=\"styled\"\n",
        "      button class=(pressed ? Flat : Raised) press=press testId=\"chosen\"\n        text \"x\"\n",
        "      view box-shadow=`0 ${lift}px 6px light-dark(#000000, #ffffff)` testId=\"template\"\n",
    ));
    let style = |r: &Runner<NoData>, t: &str| r.kernel().node(id(r, t)).unwrap().style.clone();
    let v = |x, y| Vec2 { x, y };
    assert_eq!(
        shadow(&style(&r, "literal")),
        (fixed("#11223380"), v(1.0, -2.0), 0.0, 1.0)
    );
    let raised = (fixed("rgba(0,0,0,0.2)"), v(0.0, 2.0), 12.0, 1.0);
    assert_eq!(shadow(&style(&r, "styled")), raised);
    assert_eq!(shadow(&style(&r, "chosen")), raised);
    let (color, offset, blur, opacity) = shadow(&style(&r, "template"));
    assert!(matches!(color, ColorValue::LightDark(..)), "{color:?}");
    assert_eq!((offset, blur, opacity), (v(0.0, 3.0), 6.0, 1.0));
    r.dispatch(id(&r, "chosen"), Event::Press).unwrap();
    let flat = shadow(&style(&r, "chosen"));
    assert_eq!(flat.3, 0.0, "`none` is no shadow: {flat:?}");
}

#[test]
fn box_shadow_refusals_name_what_exact2_does_not_draw() {
    // LLP 1077 D4: lists, `inset` and spread are drawn now.
    let r = boot("component App\n  view\n    view box-shadow=\"0 1px 2px #000, inset 0 2px 4px 3px #fff\" testId=\"a\"\n");
    let list = r
        .kernel()
        .node(id(&r, "a"))
        .unwrap()
        .style
        .box_shadow
        .clone();
    assert_eq!(
        (list.0.len(), list.0[1].inset, list.0[1].spread),
        (2, true, 3.0)
    );
    for (value, says) in [
        ("\"inset inset 0 1px 2px #000\"", "once"),
        ("\"0 1px 2px\"", "currentcolor"),
        ("\"0 1 2 #000\"", "length in px"),
        ("\"0 1px -2px #000\"", "negative"),
        ("4", "takes a string"),
    ] {
        for style in [false, true] {
            let source = if style {
                format!("style S\n  box-shadow={value}\ncomponent App\n  view\n    view class=S\n")
            } else {
                format!("component App\n  view\n    view box-shadow={value}\n")
            };
            let e = contract::compile(&source).unwrap_err();
            assert!(e.message.contains(says), "{value}: {e}");
            assert!(e.message.contains("box-shadow"), "{value}: {e}");
        }
    }
}

#[test]
fn text_transform_is_measured_and_shown_as_one_string() {
    let mut r = boot(concat!(
        "component App\n  view\n",
        "    column align-items=\"flex-start\" text-transform=\"uppercase\" testId=\"root\"\n",
        "      text \"straße\" testId=\"inherited\"\n",
        "      text \"straße\" text-transform=\"none\" testId=\"none\"\n",
        "      text \"MiXed\" text-transform=\"lowercase\" testId=\"lower\"\n",
        "      text text-transform=\"capitalize\" testId=\"para\"\n",
        "        text \"hel\" testId=\"a\"\n",
        "        text \"lo world\" testId=\"b\"\n",
        "      input value=\"abc\" field-sizing=\"content\" testId=\"field\"\n",
    ));
    let ids: Vec<u32> = ["root", "inherited", "none"].map(|t| id(&r, t)).into();
    let k = r.kernel();
    let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
    let shown = |t: &str| node(t).shown_text().map(|s| s.into_owned());
    assert_eq!(shown("inherited").as_deref(), Some("STRASSE"));
    assert_eq!(shown("none").as_deref(), Some("straße"));
    assert_eq!(shown("lower").as_deref(), Some("mixed"));
    // A word split across two runs is one word.
    assert_eq!(shown("a").as_deref(), Some("Hel"));
    assert_eq!(shown("b").as_deref(), Some("lo World"));
    let runs: Vec<String> = node("para")
        .text_runs()
        .iter()
        .map(|r| r.text.to_string())
        .collect();
    assert_eq!(runs, ["Hel", "lo World"]);
    // A field shows what was typed, as the web's form controls do.
    assert_eq!(node("field").text_runs()[0].text, "abc");
    // Measured as shown: 0.6 em a glyph at 16px, seven glyphs against six.
    let k = r.kernel_mut();
    k.compute_layout(ids[0], Offer::MAX_CONTENT).unwrap();
    let width = |id: u32| k.node(id).unwrap().frame.width;
    assert!(
        (width(ids[1]) - 7.0 * 9.6).abs() < 1e-3,
        "{}",
        width(ids[1])
    );
    assert!(
        (width(ids[2]) - 6.0 * 9.6).abs() < 1e-3,
        "{}",
        width(ids[2])
    );
}

#[test]
fn capitalize_keeps_word_context_across_empty_and_nested_runs() {
    for (parts, want) in [
        (["don", "'", "t stop"], ["Don", "'", "T Stop"]),
        (["foo", "", "bar"], ["Foo", "", "bar"]),
        (["don'", "", "t stop"], ["Don'", "", "T Stop"]),
        (["don", "", "'t stop"], ["Don", "", "'t Stop"]),
        (["foo ", "", "bar"], ["Foo ", "", "Bar"]),
        (["é", "’", "lan"], ["É", "’", "Lan"]),
    ] {
        let r = boot(&format!(
            "component App\n  view\n    text text-transform=\"capitalize\" testId=\"para\"\n      text {:?} testId=\"a\"\n      text\n        text {:?} testId=\"b\"\n        text {:?} testId=\"c\"\n",
            parts[0], parts[1], parts[2]
        ));
        let k = r.kernel();
        let node = |t: &str| k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
        let runs: Vec<String> = node("para")
            .text_runs()
            .iter()
            .map(|r| r.text.to_string())
            .collect();
        assert_eq!(runs, want, "{parts:?}");
        for (id, want) in ["a", "b", "c"].into_iter().zip(want) {
            assert_eq!(node(id).shown_text().as_deref(), Some(want), "{parts:?}");
        }
    }
}

#[test]
fn text_transform_is_refused_on_a_field_and_lists_its_values() {
    for tag in ["input", "textarea"] {
        for style in [false, true] {
            let source = if style {
                format!("style S\n  text-transform=\"uppercase\"\ncomponent App\n  view\n    {tag} class=S\n")
            } else {
                format!("component App\n  view\n    {tag} text-transform=\"uppercase\"\n")
            };
            let e = contract::compile(&source).unwrap_err();
            assert_eq!(e.id, "lower-attr-tag", "{e}");
            assert!(e.message.contains("shows what was typed"), "{e}");
        }
    }
    let e = contract::compile("component App\n  view\n    text \"a\" text-transform=\"caps\"\n")
        .unwrap_err();
    assert!(
        e.message
            .ends_with("expected one of \"none\", \"uppercase\", \"lowercase\", \"capitalize\""),
        "{e}"
    );
}
