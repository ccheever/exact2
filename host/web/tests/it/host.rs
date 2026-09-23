//! The web host over the Caltrain app, natively: the first batch creates the
//! whole tree with CSS computed from the kernel's rows, and later batches
//! carry only what changed.

use exact_runner::Event;
use exact_web::css::{css_text, easing_css, transition_css};
use exact_web::Host;

fn boot() -> (Host<caltrain_data::Caltrain>, String) {
    let plan = caltrain::build().unwrap();
    Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_deferred_deep_launch_paints_before_its_source_activates() {
    struct Deferred(bool);
    impl exact_runner::DataSource for Deferred {
        fn ready(&self) -> bool {
            self.0
        }
        fn activate(&mut self) -> Result<(), exact_runner::DataError> {
            self.0 = true;
            Ok(())
        }
        fn query(
            &mut self,
            _: &str,
            args: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            assert!(self.0, "source ran before first pixel");
            Ok(args[0].clone())
        }
    }
    let plan = contract::bake(
        contract::compile(
            r#"
routes nav
  tab home "/"
    post "/post/:post"
      write "/post/:post/write"
component App
  resource page = loadPage(top(nav).url) as shape string
  view
    main navigationKey=`${top(nav).id}`
      text page testId="page"
"#,
        )
        .unwrap(),
        Deferred(true),
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        Deferred(false),
        Default::default(),
        "/post/5/write",
    )
    .unwrap();
    assert!(first.contains("\"text\":\"/\""), "{first}");
    assert!(first.contains("\"url\":\"/post/5/write\""), "{first}");
    assert_eq!(
        host.runner().resource_args("page"),
        Some([exact_runner::Value::str("/post/5/write")].as_slice())
    );
    let answered = host.data_ready();
    assert!(
        answered.contains("\"text\":\"/post/5/write\""),
        "{answered}"
    );
    assert!(answered.contains("\"error\":null"), "{answered}");
}

#[test]
fn symbol_roles_carry_host_paths_and_decorative_images() {
    let plan = contract::compile(
        r##"component App
  state source = "symbol:search"
  action change writes source
    source = "symbol:unknown"
  view
    column font-size=22
      button "Change" press=change testId="change"
      image source aria-label="Ignored decorative label" tint-color="light-dark(#007aff,#0a84ff)"
"##,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(first.contains("\"src\":\"symbol:search\""));
    assert!(first.contains("\"data-symbol-path\":\"M16 10"));
    assert!(first.contains("\"alt\":\"\""));
    assert!(!first.contains("Ignored decorative label"));
    assert!(first.contains("--exact-tint:light-dark("), "{first}");
    let changed = host.dispatch(view_with_test_id(&host, "change"), Event::Press);
    assert!(changed.contains("\"data-symbol-path\":\"\""), "{changed}");
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
    s.set_dynamic(
        StyleId::BorderStyleBottom,
        &StyleValue::Text("solid".into()),
    )
    .unwrap();
    s.set_dynamic(
        StyleId::BorderColorBottom,
        &StyleValue::Text("currentColor".into()),
    )
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
    s.set_dynamic(StyleId::LineClamp, &StyleValue::Number(2.0))
        .unwrap();
    let (css, skipped) = css_text(&s, &[]);
    for expected in [
        "display:-webkit-box;",
        "-webkit-box-orient:vertical;",
        "-webkit-line-clamp:2;",
        "width:100%;",
        "max-width:640px;",
        "height:auto;",
        "flex-grow:1;",
        "color:rgba(192,57,43,1);",
        "border-top-left-radius:16px;",
        "border-bottom-width:1px;",
        "border-bottom-style:solid;",
        "border-bottom-color:currentcolor;",
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
    // Chrome knows no `ui-*` family: bare, each rendered as Times.
    for (family, stack) in [
        ("ui-monospace", "ui-monospace,monospace"),
        ("ui-serif", "ui-serif,serif"),
        ("ui-sans-serif", "ui-sans-serif,system-ui,sans-serif"),
        ("ui-rounded", "ui-rounded,system-ui,sans-serif"),
        ("system-ui", "system-ui"),
    ] {
        let families = vec![String::new(), String::new(), family.into()];
        let (css, _) = css_text(&s, &families);
        assert!(css.contains(&format!("font-family:{stack};")), "{css}");
    }
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
    let (mut host, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
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
    let (_, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
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
    let (_, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
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
    let (host, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
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
    let len = bridge.boot(&encoded, NoData, 390.0, 844.0, "/");
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
    let (host, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
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
fn scroll_offsets_are_dom_properties_sent_only_when_their_bindings_change() {
    let plan = contract::compile(
        r#"component App
  state top = 0
  state note = ""
  action bottom writes top
    top = 1000000
  action edit(value) writes note
    note = value
  view
    column
      scroll scrollTop=top scrollLeft=top height=100
        text note
      input value=note change=edit testId="note"
      button press=bottom testId="bottom"
        text "Bottom"
"#,
    )
    .unwrap();
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(batch.contains("\"scrollTop\":\"0\""), "{batch}");
    assert!(batch.contains("\"scrollLeft\":\"0\""), "{batch}");
    let bottom = view_with_test_id(&host, "bottom");
    let batch = host.dispatch(bottom, Event::Press);
    assert!(batch.contains("\"scrollTop\":\"1000000\""), "{batch}");
    assert!(batch.contains("\"scrollLeft\":\"1000000\""), "{batch}");
    let note = view_with_test_id(&host, "note");
    let batch = host.dispatch(note, Event::Change("Reading history".into()));
    assert!(!batch.contains("scrollTop"), "{batch}");
    assert!(!batch.contains("scrollLeft"), "{batch}");
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
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
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
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
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

#[test]
fn inert_is_a_boolean_dom_attribute_without_removing_its_subtree() {
    let plan = contract::compile(
        r#"component App
  state blocked = true
  action flip writes blocked
    blocked = not blocked
  view
    column
      button press=flip testId="flip"
        text "Toggle"
      column inert=blocked testId="region"
        input value="Kept draft" testId="editor"
"#,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(first.contains("\"inert\":\"true\""), "{first}");
    assert!(!first.contains("data-inert"), "{first}");
    let region = view_with_test_id(&host, "region");
    let editor = view_with_test_id(&host, "editor");
    let flip = view_with_test_id(&host, "flip");
    for expected in [false, true] {
        let batch = host.dispatch(flip, Event::Press);
        assert!(
            batch.contains(&format!("\"inert\":\"{expected}\"")),
            "{batch}"
        );
        assert_eq!(view_with_test_id(&host, "region"), region);
        assert_eq!(view_with_test_id(&host, "editor"), editor);
        assert_eq!(
            host.runner()
                .kernel()
                .node(editor)
                .unwrap()
                .props
                .str(exact_kernel::PropId::Value),
            Some("Kept draft")
        );
    }
    assert!(contract::compile("component App\n  view\n    column inert=\"yes\"\n").is_err());
}

#[test]
fn declared_keyboard_shortcuts_are_standard_aria_attributes() {
    let plan = contract::compile(
        r#"component App
  state saved = false
  action save writes saved
    saved = true
  view
    button press=save disabled=saved aria-keyshortcuts="Meta+S Control+S" testId="save"
      text "Save"
"#,
    )
    .unwrap();
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        batch.contains("\"aria-keyshortcuts\":\"Meta+S Control+S\""),
        "{batch}"
    );
    let id = view_with_test_id(&host, "save");
    let batch = host.dispatch(id, Event::Press);
    assert!(batch.contains("\"disabled\":\"true\""), "{batch}");
}

#[test]
fn dialog_invokers_preserve_html_commands_and_live_action_labels() {
    let plan = contract::compile(
        r#"component App
  state count = 1
  action prepare writes count
    count = 2
  action confirm writes count
    count = 0
  view
    column
      button press=prepare commandfor="confirm" command="show-modal" testId="open"
        text "Delete"
      dialog id="confirm" closedby="any" role="alertdialog"
        button press=confirm commandfor="confirm" command="close"
          text `Delete ${count} messages`
        button commandfor="confirm" command="close"
          text "Cancel"
"#,
    )
    .unwrap();
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(batch.contains("\"tag\":\"dialog\""), "{batch}");
    assert!(batch.contains("\"closedby\":\"any\""), "{batch}");
    assert!(batch.contains("\"commandfor\":\"confirm\""), "{batch}");
    assert!(batch.contains("\"command\":\"show-modal\""), "{batch}");
    let id = view_with_test_id(&host, "open");
    let batch = host.dispatch(id, Event::Press);
    assert!(batch.contains("Delete 2 messages"), "{batch}");
}

/// A scheme-aware colour is handed to the browser, not resolved here.
///
/// This is the whole reason the kernel keeps `light-dark()` as a pair rather
/// than flattening it (LLP 1034 D2): handed the function, the browser
/// resolves it per element against the inherited `color-scheme`, with no
/// JavaScript, no repaint pass of ours, and no appearance reported to the
/// app. A host that flattened it would be making the browser's decision.
#[test]
fn a_light_dark_colour_reaches_the_browser_as_the_function() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut style = StyleProps::default();
    style
        .set_dynamic(
            StyleId::BackgroundColor,
            &StyleValue::Text("light-dark(#ffffff, #17181b)".into()),
        )
        .unwrap();
    let (css, _) = css_text(&style, &[]);
    assert!(
        css.contains("light-dark("),
        "the pair must survive to CSS, not be resolved here: {css}"
    );
    assert!(css.contains("255,255,255"), "the light half: {css}");
    assert!(css.contains("23,24,27"), "the dark half: {css}");

    // A fixed colour is unchanged — the common case pays nothing.
    let mut style = StyleProps::default();
    style
        .set_dynamic(StyleId::TextColor, &StyleValue::Text("#112233".into()))
        .unwrap();
    let (css, _) = css_text(&style, &[]);
    assert!(
        !css.contains("light-dark("),
        "a fixed colour is a colour: {css}"
    );
}

#[test]
fn a_multi_timer_advance_creates_children_before_attaching_them() {
    let plan = contract::compile(
        r#"component App
  state phase = 0
  action tick writes phase
    phase = phase + 1
  task clock mount
    every(100, tick)
  view
    column testId="parent"
      when phase == 1
        text "typing"
      when phase >= 2
        text "reply" testId="reply"
"#,
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    let advance = host.advance(200.0);
    let kernel = host.runner().kernel();
    let reply = kernel
        .node_by_key(kernel.find_by_test_id("reply")[0])
        .unwrap()
        .id;
    let parent = kernel
        .node_by_key(kernel.find_by_test_id("parent")[0])
        .unwrap()
        .id;
    let creation = advance
        .find(&format!("\"op\":\"create\",\"id\":{reply},"))
        .unwrap();
    let attachment = advance
        .find(&format!(
            "\"op\":\"children\",\"id\":{parent},\"ids\":[{reply}]"
        ))
        .unwrap();
    assert!(
        creation < attachment,
        "child attached before creation: {advance}"
    );
}

#[test]
fn editor_hints_and_picker_policy_reach_the_dom_and_update() {
    let plan = contract::compile(
        r#"component App
  state enabled = false
  action toggle writes enabled
    enabled = not enabled
  view
    column
      button "Toggle" press=toggle testId="toggle"
      input emojiPicker=enabled autocapitalize=(enabled ? "words" : "none") autocorrect=(enabled ? "on" : "off") spellcheck=(enabled ? "true" : "false")
      column contextTarget="source" contextMagnify=enabled
      textarea autocapitalize=(enabled ? "characters" : "off") autocorrect=(enabled ? "on" : "off") spellcheck=(enabled ? "true" : "false")
"#,
    )
    .unwrap();
    let (mut host, initial) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(initial.contains("\"autocapitalize\":\"none\""));
    assert!(initial.contains("\"autocapitalize\":\"off\""));
    assert_eq!(initial.matches("\"autocorrect\":\"off\"").count(), 2);
    assert!(!initial.contains("data-autocapitalize"));
    assert!(!initial.contains("data-autocorrect"));
    assert_eq!(initial.matches("\"spellcheck\":\"false\"").count(), 2);
    assert!(!initial.contains("data-spellcheck"));
    assert!(initial.contains("\"emojiPicker\":\"false\""));
    assert!(initial.contains("\"contextMagnify\":\"false\""));
    assert!(!initial.contains("data-emojipicker"));
    let id = view_with_test_id(&host, "toggle");
    let changed = host.dispatch(id, Event::Press);
    assert!(changed.contains("\"autocapitalize\":\"words\""));
    assert!(changed.contains("\"autocapitalize\":\"characters\""));
    assert_eq!(changed.matches("\"autocorrect\":\"on\"").count(), 2);
    assert_eq!(changed.matches("\"spellcheck\":\"true\"").count(), 2);
    assert!(changed.contains("\"emojiPicker\":\"true\""));
    assert!(changed.contains("\"contextMagnify\":\"true\""));
}

#[test]
fn caret_color_is_css_including_auto_transparency_and_scheme_pairs() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    for (authored, expected) in [
        ("auto", "caret-color:auto"),
        ("#ffffff", "caret-color:rgba(255,255,255,1)"),
        ("#00000000", "caret-color:rgba(0,0,0,0)"),
        (
            "light-dark(#ffffff, #112233)",
            "caret-color:light-dark(rgba(255,255,255,1), rgba(17,34,51,1))",
        ),
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::CaretColor, &StyleValue::Text(authored.into()))
            .unwrap();
        let (css, skipped) = css_text(&style, &[]);
        assert!(skipped.is_empty());
        assert!(css.contains(expected), "{authored}: {css}");
    }
}

#[test]
fn css_line_height_retains_ratio_length_normal_and_zero() {
    use exact_kernel::{StyleId, StyleProps, StyleValue};
    let mut style = StyleProps::default();
    for (value, css) in [
        (StyleValue::Number(1.5), "line-height:1.5;"),
        (StyleValue::Text("24px".into()), "line-height:24px;"),
        (StyleValue::Text("normal".into()), "line-height:normal;"),
        (StyleValue::Number(0.0), "line-height:0;"),
        (StyleValue::Text("0px".into()), "line-height:0px;"),
    ] {
        style.set_dynamic(StyleId::LineHeight, &value).unwrap();
        assert_eq!(exact_web::css::css_text(&style, &[]).0, css);
    }
}

// @ref LLP 1039 §5 — observe actual host batches across the breakpoint.
#[test]
fn viewport_boot_and_resize_use_one_batch_and_emit_aria_orientation() {
    struct NoData;
    impl exact_runner::DataSource for NoData {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            panic!("host fact reached {name}")
        }
    }
    let plan = contract::bake(
        contract::compile(include_str!(
            "../../../../contract/corpus/viewport.contract"
        ))
        .unwrap(),
        NoData,
    )
    .unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        exact_runner::Viewport {
            width: 1280.0,
            height: 900.0,
        },
        "/",
    )
    .unwrap();
    assert_eq!(host.runner().kernel().epoch(), 1);
    assert!(first.contains("aria-orientation") && first.contains("vertical"));
    assert!(!host.runner().kernel().find_by_test_id("wide").is_empty());
    let batch = host.resize(390.0, 844.0, 0.0);
    assert!(batch.contains("\"error\":null"), "{batch}");
    assert_eq!(host.runner().kernel().epoch(), 2);
    assert!(!host.runner().kernel().find_by_test_id("narrow").is_empty());
    host.resize(390.0, 844.0, 0.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
}

// @ref LLP 1039 D2 — a refused timer cannot swallow the browser's resize.
#[test]
fn viewport_resize_and_a_due_timer_refusal_share_one_batch() {
    let plan = contract::compile(
        r#"shape Viewport
  width: number
component App
  state count = 0
  resource viewport = exactViewport() as shape Viewport
  action refuse writes count
    count = 1 / 0
  task clock mount
    every(100, refuse)
  view
    text `${viewport.width}` testId="width"
"#,
    )
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    let batch = host.resize(1280.0, 900.0, 200.0);
    assert!(batch.contains("SlotType"), "{batch}");
    assert!(batch.contains("\"text\":\"1280\""), "{batch}");
    assert_eq!(host.runner().viewport().width, 1280.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
    assert_eq!(
        host.runner().slot("count"),
        Some(&exact_runner::Value::Number(0.0))
    );
}

// @ref LLP 1038 D5/D7 — boot sources see the launch, each batch drains once.
#[test]
fn router_batches_follow_launch_and_committed_actions() {
    #[derive(Default)]
    struct Questions {
        asked: Vec<Vec<exact_plan::Value>>,
    }
    impl exact_runner::DataSource for Questions {
        fn query(
            &mut self,
            name: &str,
            args: &[exact_plan::Value],
        ) -> Result<exact_plan::Value, exact_runner::DataError> {
            match name {
                "loadQuestions" => {
                    self.asked.push(args.to_vec());
                    Ok(args[0].clone())
                }
                "loadPosts" | "loadPeople" => Ok(exact_plan::Value::list(vec![])),
                _ => panic!("unexpected route source: {name}"),
            }
        }
    }
    let plan = contract::bake(
        contract::compile(include_str!("../../../../contract/corpus/routes.contract")).unwrap(),
        Questions::default(),
    )
    .unwrap();
    let (mut host, batch) = Host::boot(
        &plan.encode(),
        Questions::default(),
        Default::default(),
        "/prompt/5/write",
    )
    .unwrap();
    assert_eq!(host.location(), "/prompt/5/write");
    assert!(host
        .agent("{\"op\":\"logs\"}")
        .contains("query questions: loadQuestions"));
    assert_eq!(
        host.runner().data_ref().asked,
        vec![vec![exact_plan::Value::list(vec![
            exact_plan::Value::str("5"),
            exact_plan::Value::str("5")
        ])]]
    );
    assert_eq!(batch.matches("\"op\":\"router\"").count(), 1);
    assert!(batch.contains("\"url\":\"/prompt/5/write\",\"removed\":[]"));
    println!("launch batch: {batch}");
    let unchanged = host.resize(1280.0, 900.0, 0.0);
    assert!(!unchanged.contains("\"op\":\"router\""));
    let key = host.runner().kernel().find_by_test_id("select-home-6")[0];
    let id = host.runner().kernel().node_by_key(key).unwrap().id;
    let batch = host.dispatch_at(id, Event::Press, 0.0);
    assert_eq!(batch.matches("\"op\":\"router\"").count(), 1);
    assert!(batch.contains("\"url\":\"/\",\"removed\":[]"));
    assert_eq!(host.location(), "/");
    let (home, _) = Host::boot(
        &plan.encode(),
        Questions::default(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(home.runner().data_ref().asked.is_empty());
    assert!(!home.agent("{\"op\":\"logs\"}").contains("loadQuestions"));
}

#[test]
fn live_regions_and_autofocus_use_html_attributes() {
    let plan = contract::compile(include_str!(
        "../../../../contract/corpus/accessibility.contract"
    ))
    .unwrap();
    let (host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(batch.contains("\"aria-live\":\"polite\""), "{batch}");
    assert!(batch.contains("\"aria-live\":\"assertive\""), "{batch}");
    assert!(batch.contains("\"autofocus\":\"true\""), "{batch}");
    assert!(host
        .agent("{\"op\":\"tree\"}")
        .contains("accessibilityLive"));
}

/// A theme flip restyles every row: each touched row gets one style op and
/// nothing else. A receipt's `touched` never holds a created node (the
/// kernel's contract), so the host walks it once; searching it per key made
/// this quadratic — 163 ms for 10k rows in the browser against 2.4 ms in the
/// runner (review, 2026-09-22).
#[test]
fn a_theme_flip_restyles_each_touched_row_once() {
    const ROWS: usize = 10_000;
    struct Rows;
    impl exact_runner::DataSource for Rows {
        fn query(
            &mut self,
            _: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            let row =
                |i: usize| exact_runner::Value::record(vec![exact_runner::Value::Number(i as f64)]);
            Ok(exact_runner::Value::list((0..ROWS).map(row).collect()))
        }
    }
    let source = r##"shape Row
  id: number
component App
  state dark = false
  resource rows = rows() as shape list<Row>
  action flip writes dark
    dark = !dark
  view
    column
      button press=flip testId="flip"
        text "Flip"
      each r in rows key=r.id
        text `${r.id}` color=(dark ? "#ffffff" : "#000000")
"##;
    let plan = contract::bake(contract::compile(source).unwrap(), Rows).unwrap();
    let (mut host, _) = Host::boot(&plan.encode(), Rows, Default::default(), "/").unwrap();
    let flip = view_with_test_id_any(&host, "flip");
    let started = std::time::Instant::now();
    let batch = host.dispatch(flip, Event::Press);
    let ms = started.elapsed().as_secs_f64() * 1e3;
    eprintln!("theme flip over {ROWS} rows: {ms:.1} ms (dispatch, commit and batch)");
    assert_eq!(
        batch.matches("\"op\":\"style\"").count(),
        ROWS,
        "one style op per row"
    );
    assert_eq!(batch.matches("\"op\":\"create\"").count(), 0);
    assert_eq!(batch.matches("\"op\":\"props\"").count(), 0);
}

/// A same-origin link is followed in place only when a declared pattern
/// matches its location; notfound absorbs everything else and must not
/// capture a file's or another app's path (LLP 1038 §7).
#[test]
fn only_declared_routes_are_followed_in_place() {
    let plan = contract::compile(
        r#"routes nav
  tab home "/"
    post "/post/:post"
  notfound
component App
  view
    main navigationKey=`${top(nav).id}`
      text "Home"
"#,
    )
    .unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    for (location, declared) in [
        ("/", true),
        ("/post/42", true),
        ("/post/42?tab=replies", true),
        ("/post", false),
        ("/nowhere/file.txt", false),
        ("/assets/caltrain.png", false),
    ] {
        assert_eq!(host.route_matches(location), declared, "{location}");
    }
    let plain = contract::compile("component App\n  view\n    text \"x\"\n").unwrap();
    let (host, _) = Host::boot(
        &plain.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(
        !host.route_matches("/"),
        "an app without routes follows no link"
    );
}

/// A text block with a heading level is HTML's heading element — Chrome's
/// accessibility tree ignores `aria-level` on a role-less div (review,
/// 2026-09-22) — unless the author gave it another role. Past h6 it is a div
/// with `role="heading"`; an inline run stays a span.
#[test]
fn heading_levels_are_heading_elements() {
    let plan = contract::compile(
        r#"component App
  view
    column
      text "One" aria-level=1 testId="one"
      text "Six" aria-level=6 testId="six"
      text "Seven" aria-level=7 testId="seven"
      text "Item" aria-level=2 role="treeitem" testId="item"
      text "Plain" testId="plain"
      text testId="para"
        text "run" aria-level=3 testId="run"
"#,
    )
    .unwrap();
    let (host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    for (test_id, tag) in [
        ("one", "h1"),
        ("six", "h6"),
        ("seven", "div"),
        ("item", "div"),
        ("plain", "div"),
        ("para", "div"),
        ("run", "span"),
    ] {
        let id = view_with_test_id_any(&host, test_id);
        let create = format!("\"op\":\"create\",\"id\":{id},\"tag\":\"{tag}\"");
        assert!(batch.contains(&create), "{test_id} is not a {tag}: {batch}");
    }
    assert!(batch.contains("\"role\":\"heading\""), "{batch}");
    assert!(batch.contains("\"role\":\"treeitem\""), "{batch}");
    assert_eq!(batch.matches("\"role\":\"heading\"").count(), 1, "{batch}");
}
