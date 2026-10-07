//! Native field environment and silent chrome settlement before publication.
use super::*;
use crate::Host;
use exact_kernel::{
    FieldChrome, FieldChromeRequest, MonospaceMeasurer, TextMeasureRequest, TextMeasurer,
    TextMetrics,
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
        let provisional = self.forever || self.seen.insert((kind, r.font.size.to_bits()));
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
