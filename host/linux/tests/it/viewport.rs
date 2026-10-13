use exact_runner::{DataError, Value};

// @ref LLP 1039 §5 — native layout and the resource share one viewport.
#[test]
fn viewport_resize_reanswers_before_native_layout() {
    struct FactsOnly;
    impl exact_runner::DataSource for FactsOnly {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("host fact reached {name}")
        }
    }
    let plan = contract::bake(
        contract::compile(include_str!(
            "../../../../contract/corpus/viewport.contract"
        ))
        .unwrap(),
        FactsOnly,
    )
    .unwrap();
    let (mut host, _) = exact_linux::Host::boot(
        &plan.encode(),
        FactsOnly,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        1280.0,
        900.0,
    )
    .unwrap();
    assert_eq!(host.runner().kernel().epoch(), 1);
    assert!(!host.runner().kernel().find_by_test_id("wide").is_empty());
    host.resize(390.0, 844.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
    assert!(!host.runner().kernel().find_by_test_id("narrow").is_empty());
    assert_eq!(
        host.runner().viewport(),
        exact_runner::Viewport::sized(390.0, 844.0)
    );
    host.resize(390.0, 844.0);
    assert_eq!(host.runner().kernel().epoch(), 2);
    host.resize(f32::NAN, 0.0);
    assert_eq!(host.runner().viewport().width, 390.0);
    host.resize(1280.0, 900.0);
    assert_eq!(host.runner().kernel().epoch(), 3);
}

#[test]
fn resolved_language_switches_native_paragraph_direction() {
    struct NoData;
    impl exact_runner::DataSource for NoData {
        fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
            panic!("unexpected source {name}")
        }
    }
    let mut plan = contract::compile("component App\n  state language = \"en\"\n  view\n    column width=300\n      text \"123\" testId=\"text\"\n").unwrap();
    plan.locale = Some(exact_plan::SlotsId(0));
    for rtl in [false, true] {
        let name = exact_plan::StrId(plan.strings.len() as u32);
        plan.strings.push(if rtl { "ar" } else { "en" }.into());
        plan.locales.push(exact_plan::LocalesRow {
            name,
            rtl,
            texts: Default::default(),
        });
    }
    let plan = contract::bake(plan, NoData).unwrap();
    let (mut host, _) = exact_linux::Host::boot(
        &plan.encode(),
        NoData,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        400.0,
        300.0,
    )
    .unwrap();
    for (locale, lang, direction) in [
        ("ar-EG", "ar", exact_kernel::Direction::Rtl),
        ("de", "en", exact_kernel::Direction::Ltr),
    ] {
        assert!(host
            .set_place(&exact_runner::time::Place {
                locale: locale.into(),
                ..Default::default()
            })
            .is_none());
        assert_eq!(host.runner().resolved_locale(), lang);
        let kernel = host.runner().kernel();
        let node = kernel
            .node_by_key(kernel.find_by_test_id("text")[0])
            .unwrap();
        assert_eq!(
            node.computed_style(exact_kernel::StyleMask::INHERITED)
                .direction,
            direction
        );
    }
}

/// A route a bar covers at its bottom scrolls its last child clear of the
/// bar: its extent's end padding is the engine's, the cover included (LLP
/// 1075.003 §3.5), though the CSS padding published leaves the cover out.
#[test]
fn a_covered_scrollers_extent_keeps_the_cover() {
    use exact_kernel::{HostCover, Kernel, NodeType, Offer, Op, StyleId, StyleProps, StyleValue};
    let style = |rows: &[(StyleId, StyleValue)]| {
        let mut s = StyleProps::default();
        for (id, value) in rows {
            s.set_dynamic(*id, value).unwrap();
        }
        Box::new(s)
    };
    let mut kernel = Kernel::with_monospace();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: style(&[
                (StyleId::Width, StyleValue::Number(400.0)),
                (StyleId::Height, StyleValue::Number(300.0)),
                (StyleId::OverflowY, StyleValue::Text("auto".into())),
                (StyleId::BoxSizing, StyleValue::Text("border-box".into())),
            ]),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: style(&[
                (StyleId::PositionType, StyleValue::Text("absolute".into())),
                (StyleId::Top, StyleValue::Number(400.0)),
                (StyleId::Width, StyleValue::Number(100.0)),
                (StyleId::Height, StyleValue::Number(100.0)),
            ]),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .set_host_cover(1, Some(HostCover::Edges([0.0, 0.0, 80.0, 0.0])))
        .unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 300.0))
        .unwrap();
    let node = kernel.node(1).unwrap();
    assert_eq!(exact_linux::paint::content_size(&node, &kernel).1, 580.0);
}
