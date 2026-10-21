//! Native resize observations exclude host chrome. @ref LLP 1104 D5.
use exact_kernel::*;
use exact_plan::{asm::Asm, builder::PlanBuilder, EventKind, Value};
use exact_runner::{DataError, DataSource, ResizeRect, Runner};

struct Empty;
impl DataSource for Empty {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
}
struct Fields;
impl TextMeasurer for Fields {
    fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
        MonospaceMeasurer::default().measure(r)
    }
    fn field_chrome(&mut self, _: &FieldChromeRequest) -> FieldChrome {
        FieldChrome {
            top: 6.0,
            right: 8.0,
            bottom: 8.0,
            left: 10.0,
            ..Default::default()
        }
    }
}

#[test]
fn native_resize_reports_content_size_and_authored_padding_without_chrome() {
    let mut b = PlanBuilder::new(SCHEMA_DIGEST, 1);
    let code = b.code(Asm::new());
    let action = b.action("resized", &[], &[], code);
    let root = b.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    b.node(
        NodeType::TextInput as u8,
        Some(root),
        None,
        0,
        &[],
        &[(EventKind::Resize, action, &[])],
        None,
    );
    let mut r = Runner::boot(
        b.finish().unwrap(),
        Empty,
        Kernel::new(Box::new(Fields)),
        Default::default(),
        "/",
    )
    .unwrap();
    for padding in [false, true] {
        let mut s = StyleProps::default();
        for (row, value) in [
            (StyleId::Width, "100"),
            (StyleId::Height, "30"),
            (StyleId::PaddingLeft, if padding { "4" } else { "0" }),
            (StyleId::PaddingRight, if padding { "3" } else { "0" }),
            (StyleId::PaddingTop, if padding { "2" } else { "0" }),
            (StyleId::PaddingBottom, if padding { "5" } else { "0" }),
            (StyleId::BorderWidthLeft, "1"),
            (StyleId::BorderStyleLeft, "solid"),
            (StyleId::BorderWidthTop, "1"),
            (StyleId::BorderStyleTop, "solid"),
        ] {
            let value = value
                .parse()
                .map_or_else(|_| StyleValue::Text(value.into()), StyleValue::Number);
            s.set_dynamic(row, &value).unwrap();
        }
        r.kernel_mut()
            .apply(
                0,
                0,
                &[Op::SetStyle {
                    id: 2,
                    patch: Box::new(s),
                }],
            )
            .unwrap();
        r.kernel_mut()
            .compute_layout(1, Offer::definite(600.0, 400.0))
            .unwrap();
        let due = r.resize_due(0);
        assert_eq!(due.len(), 1);
        let (view, rect, _) = due[0];
        assert_eq!(
            rect,
            ResizeRect {
                x: if padding { 4.0 } else { 0.0 },
                y: if padding { 2.0 } else { 0.0 },
                width: 100.0,
                height: 30.0
            }
        );
        r.resize_delivered(view, rect);
        // Feeding the observed content size back must not grow the field.
        let mut feedback = StyleProps::default();
        feedback
            .set_dynamic(StyleId::Width, &StyleValue::Number(rect.width))
            .unwrap();
        feedback
            .set_dynamic(StyleId::Height, &StyleValue::Number(rect.height))
            .unwrap();
        r.kernel_mut()
            .apply(
                0,
                0,
                &[Op::SetStyle {
                    id: view,
                    patch: Box::new(feedback),
                }],
            )
            .unwrap();
        r.kernel_mut()
            .compute_layout(1, Offer::definite(600.0, 400.0))
            .unwrap();
        assert!(r.resize_due(0).is_empty());
    }
}
