//! The web host over the Caltrain app, natively: the first batch creates the
//! whole tree with CSS computed from the kernel's rows, and later batches
//! carry only what changed.

use exact_runner::Event;
use exact_web::css::{css_text, easing_css, transition_css};
use exact_web::Host;

fn boot() -> (Host<caltrain_data::Caltrain>, String) {
    let plan = caltrain::build().unwrap();
    Host::boot(&plan.encode(), caltrain_data::Caltrain).unwrap()
}

fn view_with_test_id(host: &Host<caltrain_data::Caltrain>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn the_first_batch_creates_the_tree_with_css_from_the_rows() {
    let (host, batch) = boot();
    assert!(batch.starts_with("{\"ops\":["));
    assert!(
        batch.ends_with(",\"timers\":true,\"clock\":0,\"error\":null}"),
        "{}",
        &batch[batch.len() - 60..]
    );
    assert!(
        batch.contains("\"op\":\"create\",\"id\":1,\"tag\":\"main\""),
        "the root is a <main>"
    );
    assert!(batch.contains("\"data-testid\":\"caltrain-main\""));
    assert!(batch.contains("\"tag\":\"button\""));
    assert!(batch.contains("\"handlers\":[\"press\"]"));
    assert!(
        batch.contains("\"tag\":\"header\""),
        "semantic tags become elements"
    );
    assert!(
        batch.contains("font-size:24px;font-weight:700;"),
        "CSS from the rows"
    );
    assert!(batch.contains("display:flex;") && batch.contains("flex-direction:column;"));
    assert!(batch.contains("\"op\":\"roots\",\"ids\":[1]"));
    let creates = batch.matches("\"op\":\"create\"").count();
    assert_eq!(creates, host.runner().kernel().live_count());
    assert!(!batch.contains("\"op\":\"destroy\""));
}

#[test]
fn later_batches_carry_only_what_changed() {
    let (mut host, _) = boot();
    // The clock: a minute of ticks changes countdown texts only.
    let batch = host.advance(61_000.0);
    assert!(batch.contains("\"op\":\"props\""), "{batch}");
    assert!(batch.contains("\"text\":"));
    assert!(!batch.contains("\"op\":\"create\""));

    let change_station = view_with_test_id(&host, "change-station");
    let batch = host.dispatch(change_station, Event::Press);
    assert!(
        batch.contains("\"op\":\"destroy\""),
        "the home screen's arm is torn down"
    );
    assert!(batch.contains("\"data-testid\":\"stations-screen\""));
    assert!(batch.contains("\"tag\":\"input\""));
    assert!(batch.contains("\"handlers\":[\"change\",\"focus\",\"blur\",\"key\"]"));
    assert!(batch.contains("\"op\":\"children\",\"id\":"));
    assert!(
        !batch.contains("\"data-testid\":\"caltrain-main\""),
        "the unchanged root is not re-sent"
    );
    assert!(batch.contains("\"error\":null"));

    let search = view_with_test_id(&host, "station-search");
    let batch = host.dispatch(search, Event::Change("san".into()));
    assert!(
        batch.contains("\"op\":\"props\",\"id\":"),
        "the input's value prop changed"
    );
    assert!(batch.contains("\"value\":\"san\""));
    assert!(batch.contains("station-sf"));

    // A refusal reports in the batch and changes nothing.
    let batch = host.dispatch(999, Event::Press);
    assert!(batch.starts_with("{\"ops\":[],"));
    assert!(batch.contains("\"error\":\"UnknownView(999)\""));
}

#[test]
fn style_rows_lower_to_css_by_their_names() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut s = StyleProps::default();
    s.set_dynamic(StyleId::Width, &StyleValue::Percent(100.0))
        .unwrap();
    s.set_dynamic(StyleId::MaxWidth, &StyleValue::Number(640.0))
        .unwrap();
    s.set_dynamic(StyleId::Height, &StyleValue::Auto).unwrap();
    s.set_dynamic(StyleId::FlexGrow, &StyleValue::Number(1.0))
        .unwrap();
    s.set_dynamic(StyleId::TextColor, &StyleValue::Text("#c0392b".into()))
        .unwrap();
    s.set_dynamic(StyleId::BorderRadiusTopLeft, &StyleValue::Number(16.0))
        .unwrap();
    s.set_dynamic(StyleId::BorderWidthBottom, &StyleValue::Number(1.0))
        .unwrap();
    s.set_dynamic(StyleId::AlignSelf, &StyleValue::Text("center".into()))
        .unwrap();
    s.set_dynamic(StyleId::PositionType, &StyleValue::Text("absolute".into()))
        .unwrap();
    s.set_dynamic(StyleId::Rotate, &StyleValue::Number(45.0))
        .unwrap();
    s.set_dynamic(StyleId::Translate, &StyleValue::Vec2(10.0, -4.5))
        .unwrap();
    s.set_dynamic(StyleId::Opacity, &StyleValue::Number(0.5))
        .unwrap();
    s.set_dynamic(StyleId::LetterSpacing, &StyleValue::Number(1.2))
        .unwrap();
    s.set_dynamic(
        StyleId::PaddingTop,
        &StyleValue::Text("env(safe-area-inset-top)".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::PaddingBottom,
        &StyleValue::Text("calc(env(safe-area-inset-bottom) + 12px)".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::MarginLeft,
        &StyleValue::Text("calc(env(safe-area-inset-left) - 2px)".into()),
    )
    .unwrap();
    let (css, skipped) = css_text(&s, &[]);
    for expected in [
        "width:100%;",
        "max-width:640px;",
        "height:auto;",
        "flex-grow:1;",
        "color:rgba(192,57,43,1);",
        "border-top-left-radius:16px;",
        "border-bottom-width:1px;",
        "align-self:center;",
        "position:absolute;",
        "rotate:45deg;",
        "translate:10px -4.5px;",
        "opacity:0.5;",
        "letter-spacing:1.2px;",
        "padding-top:env(safe-area-inset-top);",
        "padding-bottom:calc(env(safe-area-inset-bottom) + 12px);",
        "margin-left:calc(env(safe-area-inset-left) - 2px);",
    ] {
        assert!(css.contains(expected), "{expected} in {css}");
    }
    assert!(skipped.is_empty(), "{skipped:?}");

    s.set_dynamic(StyleId::FontFamily, &StyleValue::Number(2.0))
        .unwrap();
    let families = vec![String::new(), String::new(), "sans-serif".into()];
    let (css, skipped) = css_text(&s, &families);
    assert!(css.contains("font-family:sans-serif;"), "{css}");
    assert!(!css.contains("font-family:\"sans-serif\";"), "{css}");
    assert!(skipped.is_empty(), "{skipped:?}");
}

#[test]
fn the_transition_row_lowers_to_css_transition_and_springs_are_named() {
    use exact_motion::{
        Easing, LinearStop, SpringConfig, StepPosition, TimingFunction, Transition,
        TransitionProperty, Transitions,
    };
    let rows = Transitions(vec![
        Transition {
            property: TransitionProperty::Property(exact_motion::Property::Opacity),
            duration: 0.25,
            delay: 0.0,
            timing: TimingFunction::Easing(Easing::EaseInOut),
        },
        Transition::new(
            TransitionProperty::All,
            0.5,
            TimingFunction::Easing(Easing::CubicBezier {
                x1: 0.4,
                y1: 0.0,
                x2: 0.2,
                y2: 1.0,
            }),
        ),
        Transition::new(
            TransitionProperty::Property(exact_motion::Property::Translate),
            0.0,
            TimingFunction::Spring(SpringConfig::default()),
        ),
    ]);
    let (css, spring) = transition_css(&rows);
    assert_eq!(
        css,
        "opacity 0.25s ease-in-out 0s,all 0.5s cubic-bezier(0.4,0,0.2,1) 0s"
    );
    assert!(spring, "the spring is reported, not silently dropped");
    assert_eq!(
        easing_css(&Easing::Steps {
            count: 4,
            position: StepPosition::JumpBoth
        }),
        "steps(4,jump-both)"
    );
    assert_eq!(
        easing_css(&Easing::PiecewiseLinear(vec![
            LinearStop {
                input: 0.0,
                output: 0.0
            },
            LinearStop {
                input: 0.5,
                output: 0.9
            },
            LinearStop {
                input: 1.0,
                output: 1.0
            },
        ])),
        "linear(0 0%,0.9 50%,1 100%)"
    );
    let mut s = exact_kernel::StyleProps {
        transition: rows,
        ..Default::default()
    };
    s.mask.set(exact_kernel::StyleId::Transition);
    let (css, skipped) = css_text(&s, &[]);
    assert!(css.starts_with("transition:opacity 0.25s"));
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].reason.contains("spring"));
}

#[test]
fn a_transition_authored_in_contract_reaches_the_page_as_css() {
    use exact_runner::{DataError, DataSource, Value};
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(s.into()))
        }
    }
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/transition.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (mut host, batch) = Host::boot(&plan.encode(), NoData).unwrap();
    assert!(batch.contains("opacity:1;"), "{batch}");
    assert!(
        batch.contains("transition:opacity 0.25s ease-in-out 0s;"),
        "the easing transition is CSS; the spring is the host's: {batch}"
    );
    let toggle = view_with_test_id_any(&host, "toggle");
    let batch = host.dispatch(toggle, Event::Press);
    assert!(batch.contains("opacity:0;"), "{batch}");
    assert!(batch.contains("\"op\":\"style\""));
}

fn view_with_test_id_any<D: exact_runner::DataSource>(host: &Host<D>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    let key = k.find_by_test_id(test_id)[0];
    k.node_by_key(key).unwrap().id
}

#[test]
fn an_image_is_an_img_with_its_source_and_object_fit() {
    let (host, batch) = boot();
    let logo = view_with_test_id(&host, "logo");
    let at = batch
        .find(&format!("\"id\":{logo},\"tag\":\"img\""))
        .unwrap();
    let create = &batch[at..at + 400];
    assert!(
        create.contains("\"src\":\"assets/caltrain.png\""),
        "{create}"
    );
    assert!(
        create.contains("\"alt\":\"A Caltrain train\"") && !create.contains("aria-label"),
        "{create}"
    );
    assert!(create.contains("object-fit:contain;"), "{create}");
    assert!(create.contains("width:96px;"), "{create}");
}

#[test]
fn an_input_uses_html_type_and_inputmode_attributes() {
    use exact_runner::{DataError, DataSource, Value};
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }
    let plan = contract::compile(
        "component App\n  view\n    input type=\"password\" inputmode=\"email\" testId=\"secret\"\n",
    )
    .unwrap();
    let (_, batch) = Host::boot(&plan.encode(), NoData).unwrap();
    let at = batch.find("\"tag\":\"input\"").unwrap();
    let create = &batch[at..];
    assert!(create.contains("\"type\":\"password\""), "{create}");
    assert!(create.contains("\"inputmode\":\"email\""), "{create}");
    assert!(!create.contains("\"data-type\"") && !create.contains("\"data-inputmode\""));
}

#[test]
fn only_text_under_text_is_an_inline_span() {
    use exact_runner::{DataError, DataSource, Value};
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }
    let plan = contract::compile(
        "component App\n  view\n    column\n      text \"block leaf\" testId=\"leaf\"\n      text testId=\"paragraph\"\n        text \"inline run\" testId=\"run\"\n",
    )
    .unwrap();
    let (_, batch) = Host::boot(&plan.encode(), NoData).unwrap();
    let create_for = |test_id: &str| {
        let prop = batch
            .find(&format!("\"data-testid\":\"{test_id}\""))
            .unwrap();
        let start = batch[..prop].rfind("{\"op\":\"create\"").unwrap();
        &batch[start..prop]
    };
    assert!(create_for("leaf").contains("\"tag\":\"div\""), "{batch}");
    assert!(
        create_for("paragraph").contains("\"tag\":\"div\""),
        "{batch}"
    );
    assert!(create_for("run").contains("\"tag\":\"span\""), "{batch}");
}

#[test]
fn declared_font_identity_reaches_the_readiness_barrier_and_css() {
    use exact_runner::{DataError, DataSource, Value};
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(s.into()))
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/fixtures/fonts/app.contract");
    let plan = contract::compile_path(&path).unwrap();
    let (host, batch) = Host::boot(&plan.encode(), NoData).unwrap();
    let catalog = host.font_catalog();
    assert_eq!(
        catalog,
        "[{\"family\":\"ExactPlanStack8\",\"source\":\"assets/DejaVuSans.ttf\",\"weight\":400,\"style\":\"normal\"},{\"family\":\"ExactPlanStack8\",\"source\":\"assets/DejaVuSans-Bold.ttf\",\"weight\":700,\"style\":\"normal\"}]"
    );
    assert!(
        !batch.contains("\"fonts\""),
        "font data leaked into the op batch: {batch}"
    );
    for test_id in ["font-400", "font-600", "font-700"] {
        let id = view_with_test_id_any(&host, test_id);
        let marker = format!("\"op\":\"create\",\"id\":{id},");
        let create = &batch[batch.find(&marker).unwrap()..];
        let create = &create[..create.find("\"handlers\":").unwrap()];
        assert!(
            create.contains("font-family:\\\"ExactPlanStack8\\\";"),
            "the opaque plan family reaches cssText: {create}"
        );
    }
    assert!(
        !batch.contains("Fixture Sans") && !catalog.contains("Fixture Sans"),
        "the Contract alias must never become a browser font lookup: {batch} {catalog}"
    );

    let encoded = plan.encode();
    let mut bridge: exact_web::abi::Bridge<NoData> = exact_web::abi::Bridge::new();
    let count = bridge.input_write(&encoded);
    let len = bridge.plan_fonts(count);
    assert_eq!(
        String::from_utf8_lossy(bridge.output_bytes(len as usize)),
        catalog
    );
    let len = bridge.fonts();
    assert_eq!(
        bridge.output_bytes(len as usize),
        b"[]",
        "font inspection must not start a host"
    );
    let len = bridge.boot(&encoded, NoData);
    let boot = String::from_utf8_lossy(bridge.output_bytes(len as usize));
    assert!(!boot.contains("\"fonts\""), "{boot}");
    let len = bridge.fonts();
    assert_eq!(
        String::from_utf8_lossy(bridge.output_bytes(len as usize)),
        catalog
    );
}

#[test]
fn an_iframe_is_the_element_with_html_props_and_handlers() {
    use exact_runner::{DataError, DataSource, Value};
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(s.into()))
        }
    }
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/iframe.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (host, batch) = Host::boot(&plan.encode(), NoData).unwrap();
    let key = host.runner().kernel().find_by_test_id("deck")[0];
    let deck = host.runner().kernel().node_by_key(key).unwrap().id;
    let at = batch
        .find(&format!("\"id\":{deck},\"tag\":\"iframe\""))
        .unwrap();
    let create = &batch[at..];
    assert!(create.contains("\"src\":\"/deck/index.html\""), "{create}");
    assert!(create.contains("\"sandbox\":\"allow-scripts\""), "{create}");
    assert!(
        create.contains("\"handlers\":[\"load\",\"message\"]"),
        "{create}"
    );
    assert!(create.contains("width:300px;height:150px;"), "{create}");
}

#[test]
fn textarea_preserves_multiline_values_through_the_change_seam() {
    let plan = contract::compile(
        r#"component App
  state note = ""
  action edit(value) writes note
    note = value
  view
    textarea value=note change=edit testId="note" height=200
"#,
    )
    .unwrap();
    let (mut host, batch) = Host::boot(&plan.encode(), caltrain_data::Caltrain).unwrap();
    assert!(batch.contains("\"tag\":\"textarea\""), "{batch}");
    assert!(batch.contains("white-space:pre-wrap"), "{batch}");
    let id = view_with_test_id(&host, "note");
    let batch = host.dispatch(id, Event::Change("First line\nSecond line\n".into()));
    assert!(batch.contains("First line\\nSecond line\\n"), "{batch}");
    assert_eq!(
        host.runner()
            .kernel()
            .node(id)
            .unwrap()
            .props
            .str(exact_kernel::PropId::Value),
        Some("First line\nSecond line\n")
    );
}

#[test]
fn readonly_uses_existing_editable_prop_and_reacts_to_changes() {
    let plan = contract::compile(
        r#"component App
  state locked = true
  action unlock writes locked
    locked = false
  view
    column
      textarea value="Copy this" readonly=locked testId="output"
      button press=unlock testId="unlock"
        text "Unlock"
"#,
    )
    .unwrap();
    let (mut host, batch) = Host::boot(&plan.encode(), caltrain_data::Caltrain).unwrap();
    assert!(batch.contains("\"readonly\":\"true\""), "{batch}");
    let id = view_with_test_id(&host, "output");
    assert_eq!(
        host.runner()
            .kernel()
            .node(id)
            .unwrap()
            .props
            .bool(exact_kernel::PropId::Editable),
        Some(false)
    );
    let button = view_with_test_id(&host, "unlock");
    let batch = host.dispatch(button, Event::Press);
    assert!(batch.contains("\"readonly\":\"false\""), "{batch}");
    assert_eq!(
        host.runner()
            .kernel()
            .node(id)
            .unwrap()
            .props
            .bool(exact_kernel::PropId::Editable),
        Some(true)
    );
    assert!(contract::compile("component App\n  view\n    textarea readonly=\"yes\"\n").is_err());
}
