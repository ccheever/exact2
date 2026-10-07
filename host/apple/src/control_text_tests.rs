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
