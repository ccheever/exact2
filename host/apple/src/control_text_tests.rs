//! Native field environment and silent chrome settlement before publication.
use crate::Host;
use exact_kernel::{
    ControlFont, ControlTextStyles, FieldChrome, FieldChromeRequest, FieldKind, FontStyle,
    MonospaceMeasurer, TextMeasureRequest, TextMeasurer, TextMetrics,
};
use std::collections::HashSet;

struct NoData;
impl exact_runner::DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, exact_runner::DataError> {
        Err(exact_runner::DataError::UnknownSource(name.into()))
    }
}
fn styles(size: f32) -> ControlTextStyles {
    let font = ControlFont {
        family: "UIKit body".into(),
        family_id: 42,
        size,
        weight: 400,
        style: FontStyle::Normal,
    };
    ControlTextStyles {
        field: font.clone(),
        textarea: font.clone(),
        button: font,
    }
}
#[derive(Default)]
struct CachedChrome {
    seen: HashSet<(u8, u32)>,
    forever: bool,
    failure: Option<std::rc::Rc<std::cell::Cell<bool>>>,
}
impl TextMeasurer for CachedChrome {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(request)
    }
    fn field_chrome(&mut self, r: &FieldChromeRequest) -> FieldChrome {
        let kind = match r.kind {
            FieldKind::Field => 0,
            FieldKind::SecureField => 1,
            FieldKind::SearchField => 2,
            FieldKind::Textarea => 3,
        };
        let provisional = self.forever
            || self.failure.as_ref().is_some_and(|f| f.get())
            || self.seen.insert((kind, r.font.size.to_bits()));
        FieldChrome {
            top: 6.0,
            right: 8.0,
            bottom: 6.0,
            left: 8.0,
            minimum_height: 34.0,
            provisional,
        }
    }
}
const SOURCE: &str = r#"component Fields
  view
    column font-size=22 font-style="italic" line-height=2 letter-spacing=2 color="red"
      input testId="field" padding="1em" value="hello"
      input testId="bare" appearance="none" value="hello"
      textarea testId="area" rows=2
"#;
fn field(host: &Host<NoData>, name: &str) -> u32 {
    let kernel = host.runner().kernel();
    kernel
        .node_by_key(kernel.find_by_test_id(name)[0])
        .unwrap()
        .id
}

fn boot(measurer: CachedChrome) -> Result<(Host<NoData>, String), crate::HostError> {
    let plan = contract::compile(SOURCE).unwrap().encode();
    Host::boot_stored_after_decode(
        crate::host::PlanBytes::Copied(&plan),
        NoData,
        Box::new(measurer),
        exact_runner::Viewport::sized(400.0, 800.0),
        None,
        Vec::new(),
        None,
        None,
        None,
        None,
        "/",
        None,
        crate::link::Links::ALL,
        |runner| {
            let mut env = runner.kernel().env();
            env.control_text_styles = Some(styles(17.0));
            runner.kernel_mut().set_env(env).unwrap();
            Ok(())
        },
    )
}
#[test]
fn control_fonts_and_chrome_are_final_before_the_first_batch_and_survive_insets() {
    let (mut host, batch) = boot(CachedChrome::default()).unwrap();
    let native = field(&host, "field");
    let bare = field(&host, "bare");
    let kernel = host.runner().kernel();
    let node = kernel.node(native).unwrap();
    let text = node.text_style();
    assert_eq!(text.font_size, 17.0);
    assert_eq!(text.font_family, 42);
    assert_eq!(text.font_style, FontStyle::Normal);
    assert_eq!(text.line_height, None);
    assert_eq!(text.letter_spacing, 0.0);
    assert_eq!(kernel.node(bare).unwrap().text_style().font_size, 22.0);
    assert_eq!(
        kernel.node(bare).unwrap().text_style().font_style,
        FontStyle::Italic
    );
    let content = node.field_content_rect().unwrap();
    assert_eq!(content.x, 8.0 + 17.0);
    assert_eq!(content.y, 6.0 + 17.0);
    assert!(batch.contains("\"op\":\"fieldContent\""));
    assert_eq!(kernel.provisional_layouts(), 1);
    let updated = host.set_insets(10.0, 0.0, 0.0, 0.0);
    assert!(
        serde_json::from_str::<serde_json::Value>(&updated).unwrap()["error"].is_null(),
        "{updated}"
    );
    assert_eq!(
        host.runner().kernel().env().control_text_styles,
        Some(styles(17.0))
    );
    let changed = host.set_control_text_styles(styles(28.0));
    assert!(
        serde_json::from_str::<serde_json::Value>(&changed).unwrap()["error"].is_null(),
        "{changed}"
    );
    assert_eq!(host.runner().kernel().env().top, 10.0);
    assert_eq!(
        host.runner()
            .kernel()
            .node(native)
            .unwrap()
            .text_style()
            .font_size,
        28.0
    );
    assert_eq!(
        host.runner()
            .kernel()
            .node(native)
            .unwrap()
            .field_content_rect()
            .unwrap()
            .x,
        8.0 + 28.0
    );
    assert!(host
        .agent("{\"op\":\"state\"}")
        .contains("\"provisionalLayouts\":2"));
}
#[test]
fn a_cache_that_never_settles_refuses_the_boot_in_three_passes() {
    let result = boot(CachedChrome {
        forever: true,
        ..Default::default()
    });
    assert!(
        matches!(result, Err(crate::HostError::Layout(ref reason)) if reason == "layout: field chrome remained provisional after three passes; batch refused before presentation")
    );
}

#[test]
fn running_session_chrome_failure_keeps_updates_but_withholds_staged_geometry() {
    let failure = std::rc::Rc::new(std::cell::Cell::new(false));
    let (mut host, _) = boot(CachedChrome {
        failure: Some(failure.clone()),
        ..Default::default()
    })
    .unwrap();
    let native = field(&host, "field");
    let mut batch = crate::batch::Batch::new();
    // Native-region promotion can have written geometry before layout fails.
    batch.frame(native, 10., 20., 30., 40.);
    batch.content(native, 300., 400.);
    batch.field_content(
        native,
        Some(exact_kernel::Frame {
            x: 1.,
            y: 2.,
            width: 3.,
            height: 4.,
        }),
    );
    batch.region("{\"op\":\"native-region\",\"origin\":[10,20]}");
    batch.props(native, &[("value", "kept".into())], &[]);
    let receipt = host
        .runner_mut()
        .kernel_mut()
        .apply(
            0,
            10,
            &[exact_kernel::Op::SetStyle {
                id: native,
                patch: Box::new({
                    let mut s = exact_kernel::StyleProps::default();
                    s.set_dynamic(
                        exact_kernel::StyleId::FontSize,
                        &exact_kernel::StyleValue::Number(31.),
                    )
                    .unwrap();
                    s
                }),
            }],
        )
        .unwrap();
    // This font misses forever, while the successfully booted session remains live.
    failure.set(true);
    let wire = host.commit_into(&[exact_runner::Timed { at_ms: 0., receipt }], None, batch);
    let json: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(json["layoutProvisional"], true, "{wire}");
    assert_eq!(json["error"], "layout: field chrome remained provisional after three passes; batch refused before presentation");
    assert!(json["ops"]
        .as_array()
        .unwrap()
        .iter()
        .any(|op| op["op"] == "props" && op["set"]["value"] == "kept"));
    assert_eq!(host.runner().kernel().provisional_layouts(), 4);
    for op in json["ops"].as_array().unwrap() {
        assert!(
            !["frame", "content", "fieldContent", "native-region"]
                .contains(&op["op"].as_str().unwrap()),
            "guessed geometry: {wire}"
        );
    }
    // Promotion may already have memoized this guessed editor rect. A
    // settled answer equal to that guess must still reach the presenter.
    let guessed = host
        .runner()
        .kernel()
        .node(native)
        .unwrap()
        .field_content_rect();
    host.mirror.get_mut(&native).unwrap().field_content = guessed;
    failure.set(false);
    let settled: serde_json::Value = serde_json::from_str(&host.resize(400., 800.)).unwrap();
    assert!(settled["error"].is_null(), "{settled}");
    assert!(
        settled["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "frame" && op["id"] == native),
        "withheld frames must be republished: {settled}"
    );
    assert!(
        settled["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "fieldContent" && op["id"] == native),
        "withheld editor rects must be republished: {settled}"
    );
}

#[test]
fn resolved_button_geometry_has_one_measurement_and_presentation_payload() {
    use exact_kernel::{ButtonMeasure, ButtonMeasureRequest, Dimension, StyleId};
    use std::{cell::RefCell, rc::Rc};
    struct Capture(Rc<RefCell<Vec<String>>>);
    #[allow(unsafe_code)] // Borrow the test-owned callback context and request synchronously.
    extern "C" fn measure(
        ctx: *mut std::ffi::c_void,
        r: *const crate::control_text::CButtonMeasureRequest,
    ) -> crate::control_text::CButtonMeasure {
        // This callback borrows the payload only for the synchronous call.
        let (out, r) = unsafe { (&*(ctx as *const RefCell<Vec<String>>), &*r) };
        let face = unsafe { std::slice::from_raw_parts(r.face, r.face_len) };
        out.borrow_mut()
            .push(String::from_utf8(face.to_vec()).unwrap());
        crate::control_text::CButtonMeasure {
            width: r.width,
            height: 60.,
            provisional: 0,
        }
    }
    impl TextMeasurer for Capture {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            MonospaceMeasurer::default().measure(r)
        }
        fn button_measure(&mut self, r: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
            Some(crate::control_text::button_measure(
                measure,
                Rc::as_ptr(&self.0) as *mut _,
                r,
            ))
        }
    }
    let source = r#"component Buttons
  view
    column width=400
      button testId="resolved" appearance="auto" width=200 padding-top="env(safe-area-inset-top)" padding-left="calc(10% + 2px)" padding-right="5%" border-radius="6px 12px 14px 16px"
        text "Measure"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let captured = Rc::new(RefCell::new(Vec::new()));
    let (mut host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(Capture(captured.clone())),
        400.,
        800.,
    )
    .unwrap();
    host.set_insets(31., 0., 0., 0.);
    let kernel = host.runner().kernel();
    let id = kernel
        .node_by_key(kernel.find_by_test_id("resolved")[0])
        .unwrap()
        .id;
    let rows = kernel.button_face_style(id).unwrap();
    assert_eq!(rows.button.padding_top, Dimension::Points(31.));
    assert_eq!(rows.button.padding_left, Dimension::Points(42.));
    assert_eq!(rows.button.padding_right, Dimension::Points(20.));
    assert!(rows.button.mask.has(StyleId::PaddingTop));
    let presented =
        crate::button::face_json(kernel.press_face(id).as_ref(), Some(&rows), "bordered");
    assert_eq!(captured.borrow().last().unwrap(), &presented);
    let json: serde_json::Value = serde_json::from_str(&presented).unwrap();
    assert_eq!(json["rows"]["button"]["padding_top"], 31.);
    assert_eq!(json["rows"]["button"]["border_radius_top_left"], 6.);
    assert_eq!(json["rows"]["button"]["border_radius_top_right"], 12.);
}

#[test]
fn control_size_fonts_feed_em_resolution_and_the_shared_face_payload() {
    extern "C" fn font(_: *mut std::ffi::c_void, kind: u8) -> crate::control_text::CControlFont {
        crate::control_text::CControlFont {
            family: std::ptr::null(),
            family_len: 0,
            family_id: 42,
            size: 17. + 10. * f32::from(kind),
            weight: 400,
            italic: 0,
        }
    }
    let fonts = crate::control_text::button_fonts(font, std::ptr::null_mut());
    let platform = crate::control_text::text_styles(font, std::ptr::null_mut());
    assert_eq!(
        platform
            .button_font(exact_kernel::ControlSize::Large, Some(&fonts))
            .size,
        57.
    );
    let plan = contract::compile(
        r#"component Buttons
  view
    column
      button appearance="auto" testId="em" -exact-control-size="large" font-size="1em"
        text "Large"
"#,
    )
    .unwrap()
    .encode();
    let (host, _) = Host::boot_stored_after_decode(
        crate::host::PlanBytes::Copied(&plan),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        exact_runner::Viewport::sized(400., 800.),
        None,
        Vec::new(),
        None,
        None,
        None,
        None,
        "/",
        None,
        crate::link::Links::ALL,
        |runner| {
            let mut env = runner.kernel().env();
            env.control_text_styles = Some(platform);
            env.button_fonts = Some(fonts);
            runner.kernel_mut().set_env(env).unwrap();
            Ok(())
        },
    )
    .unwrap();
    let kernel = host.runner().kernel();
    let id = kernel
        .node_by_key(kernel.find_by_test_id("em")[0])
        .unwrap()
        .id;
    let rows = kernel.button_face_style(id).unwrap();
    assert_eq!(rows.title.font_size, 57.);
    let json: serde_json::Value = serde_json::from_str(&crate::button::face_json(
        kernel.press_face(id).as_ref(),
        Some(&rows),
        "bordered",
    ))
    .unwrap();
    assert_eq!(json["rows"]["title"]["font_size"], 57.);
    assert_eq!(json["rows"]["title"]["font_size_resolved"], 1);
}

#[test]
fn trait_notification_remeasures_buttons_when_control_fonts_are_unchanged() {
    use std::cell::Cell;
    thread_local! {
        static SCALE: Cell<f32> = const { Cell::new(1.0) };
        static CALLS: Cell<usize> = const { Cell::new(0) };
    }
    extern "C" fn text(
        _: *mut std::ffi::c_void,
        _: *const crate::measure::CRequest,
    ) -> crate::measure::CMetrics {
        crate::measure::CMetrics {
            width: 50.0,
            height: 17.0,
            baseline: 14.0,
        }
    }
    extern "C" fn font(_: *mut std::ffi::c_void, _: u8) -> crate::control_text::CControlFont {
        crate::control_text::CControlFont {
            family: std::ptr::null(),
            family_len: 0,
            family_id: 0,
            size: 17.0,
            weight: 400,
            italic: 0,
        }
    }
    extern "C" fn button(
        _: *mut std::ffi::c_void,
        _: *const crate::control_text::CButtonMeasureRequest,
    ) -> crate::control_text::CButtonMeasure {
        CALLS.set(CALLS.get() + 1);
        let scale = SCALE.get();
        crate::control_text::CButtonMeasure {
            width: 100.0,
            height: (30.2 * scale).ceil() / scale,
            provisional: 0,
        }
    }
    let plan = contract::compile(
        "component Buttons\n  view\n    button \"Measure\" appearance=\"auto\" testId=\"button\"\n",
    )
    .unwrap()
    .encode();
    let hooks = crate::abi::Hooks {
        measure: Some(text),
        ..crate::abi::Hooks::none()
    };
    let mut bridge = crate::abi::Bridge::new();
    bridge.set_control_text(Some(font), None);
    bridge.set_button_measure(Some(button));
    SCALE.set(1.0);
    let n = bridge.boot(&plan, NoData, hooks, 400.0, 800.0);
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("\"error\":null"));
    assert!(CALLS.get() > 0, "the fixture must boot a native button");
    CALLS.set(0);
    let n = bridge.control_text_changed(hooks);
    assert!(
        CALLS.get() > 0,
        "a measuring-trait notification must reach the button cache even with identical fonts"
    );
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("\"error\":null"));
    CALLS.set(0);
    SCALE.set(2.0);
    let n = bridge.control_text_changed(hooks);
    let batch = String::from_utf8_lossy(bridge.output_bytes(n as usize));
    assert!(CALLS.get() > 0, "scale-only changes must measure again");
    assert!(batch.contains("\"h\":30.5"), "{batch}");
    CALLS.set(0);
    bridge.resize(400.0, 800.0);
    assert_eq!(CALLS.get(), 0, "ordinary layouts reuse the new revision");
}
