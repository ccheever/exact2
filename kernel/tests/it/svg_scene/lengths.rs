use super::*;

const UNDEFINED: &[&str] = &[
    "env(viewport-segment-width 1 0)",
    "calc(env(viewport-segment-width 1 0) + 10%)",
    "min(env(viewport-segment-width 1 0), 50%)",
    "max(5px, env(viewport-segment-width 1 0))",
    "clamp(1px, env(viewport-segment-width 1 0), 100px)",
];
const FALLBACK: &str = "env(viewport-segment-width 1 0, 25%)";

fn viewport_size(target: Option<NodeType>, rows: &[(StyleId, &str)]) -> (f32, f32) {
    exact_kernel::link_segments();
    let mut d = Doc::new();
    let svg = d.node(
        NodeType::Svg,
        &[(StyleId::Width, "200"), (StyleId::Height, "400")],
        &[],
    );
    let viewport = if let Some(kind) = target {
        let defs = d.node(NodeType::SvgDefs, &[], &[]);
        let source = d.node(kind, &[], &[(PropId::Id, "source")]);
        let instance = d.node(NodeType::SvgUse, rows, &[(PropId::Href, "#source")]);
        d.children(defs, &[source]);
        d.children(svg, &[defs, instance]);
        instance
    } else {
        let nested = d.node(NodeType::SvgViewport, rows, &[]);
        d.children(svg, &[nested]);
        nested
    };
    let k = d.kernel(&[svg], 800.0);
    let s = resolve(&k, svg);
    let Kind::Viewport { rect, .. } = &find(&s.items, viewport).unwrap().kind else {
        panic!("a viewport");
    };
    (rect.2, rect.3)
}

fn check_viewport_sizes(target: Option<NodeType>) {
    assert_eq!(viewport_size(target, &[]), (200.0, 400.0));
    for value in UNDEFINED {
        assert_eq!(
            viewport_size(target, &[(StyleId::Width, value), (StyleId::Height, value)]),
            (200.0, 400.0),
            "{target:?}: {value}"
        );
    }
    assert_eq!(
        viewport_size(target, &[(StyleId::Width, "0"), (StyleId::Height, "0")]),
        (0.0, 0.0)
    );
    assert_eq!(
        viewport_size(
            target,
            &[(StyleId::Width, FALLBACK), (StyleId::Height, FALLBACK)]
        ),
        (50.0, 100.0)
    );
}

#[test]
fn nested_svg_undefined_sizes_take_the_parent_viewport() {
    check_viewport_sizes(None);
}

#[test]
fn use_undefined_sizes_take_the_parent_viewport() {
    check_viewport_sizes(Some(NodeType::SvgSymbol));
    check_viewport_sizes(Some(NodeType::SvgViewport));
}

#[test]
fn undefined_radii_copy_the_other_radius() {
    use exact_kernel::svg::{geometry, Viewport};
    exact_kernel::link_segments();
    let env = exact_kernel::Env::default();
    let vp = Viewport {
        env: &env,
        width: 200.0,
        height: 400.0,
    };
    for kind in [NodeType::SvgRect, NodeType::SvgEllipse] {
        for (missing, other) in [(StyleId::Rx, StyleId::Ry), (StyleId::Ry, StyleId::Rx)] {
            let mut base = StyleProps::default();
            for (id, value) in [
                (StyleId::Width, 100.0),
                (StyleId::Height, 100.0),
                (other, 10.0),
            ] {
                base.set_dynamic(id, &StyleValue::Number(value)).unwrap();
            }
            let draw = |style: &StyleProps| geometry(kind, &Default::default(), style, vp);
            let initial = draw(&base);
            assert!(initial.is_some());
            for value in UNDEFINED {
                let mut style = base.clone();
                style
                    .set_dynamic(missing, &StyleValue::Text((*value).into()))
                    .unwrap();
                assert_eq!(draw(&style), initial, "{kind:?} {missing:?}: {value}");
            }
            for (value, expected) in [("0px", "0px"), (FALLBACK, "25%")] {
                let mut style = base.clone();
                style
                    .set_dynamic(missing, &StyleValue::Text(value.into()))
                    .unwrap();
                let mut control = base.clone();
                control
                    .set_dynamic(missing, &StyleValue::Text(expected.into()))
                    .unwrap();
                assert_eq!(draw(&style), draw(&control));
                assert_ne!(draw(&style), initial);
            }
        }
    }
}

#[test]
fn calculated_radii_clamp_at_zero_as_the_window_resizes() {
    use exact_kernel::svg::{geometry, Viewport};
    exact_kernel::link_segments();
    for kind in [NodeType::SvgRect, NodeType::SvgEllipse] {
        for (radius, other) in [(StyleId::Rx, StyleId::Ry), (StyleId::Ry, StyleId::Rx)] {
            for value in [
                "calc(10vw - 90px)",
                "min(20px, calc(10vw - 90px))",
                "max(-20px, calc(10vw - 90px))",
                "clamp(-20px, calc(10vw - 90px), 20px)",
            ] {
                let mut style = StyleProps::default();
                for (id, value) in [
                    (StyleId::Width, "100px"),
                    (StyleId::Height, "100px"),
                    (other, "10px"),
                    (radius, value),
                ] {
                    style
                        .set_dynamic(id, &StyleValue::Text(value.into()))
                        .unwrap();
                }
                for width in [800.0, 900.0, 1000.0, 800.0] {
                    let env = exact_kernel::Env::default().with_screen(Some((width, 600.0)));
                    let vp = Viewport {
                        env: &env,
                        width: 200.0,
                        height: 400.0,
                    };
                    let draw = |s: &StyleProps| geometry(kind, &Default::default(), s, vp);
                    let mut control = style.clone();
                    control
                        .set_dynamic(
                            radius,
                            &StyleValue::Number((width * 0.1 - 90.0).max(0.0).into()),
                        )
                        .unwrap();
                    assert_eq!(
                        draw(&style),
                        draw(&control),
                        "{kind:?} {radius:?} {value} at {width}"
                    );
                    for literal in [
                        "-10px",
                        "-10%",
                        "-10vw",
                        "env(viewport-segment-width 1 0, -10px)",
                    ] {
                        control
                            .set_dynamic(radius, &StyleValue::Text(literal.into()))
                            .unwrap();
                        let negative = draw(&control);
                        control.set_dynamic(radius, &StyleValue::Auto).unwrap();
                        assert_eq!(
                            negative,
                            draw(&control),
                            "a negative literal remains invalid: {literal}"
                        );
                    }
                }
            }
        }
    }
}

fn subregion(units: &str, rows: &[(StyleId, &str)]) -> (f32, f32, f32, f32) {
    exact_kernel::link_segments();
    let mut d = Doc::new();
    let svg = d.node(
        NodeType::Svg,
        &[(StyleId::Width, "200"), (StyleId::Height, "400")],
        &[],
    );
    let filter = d.node(
        NodeType::SvgFilter,
        &[],
        &[(PropId::Id, "filter"), (PropId::PrimitiveUnits, units)],
    );
    let primitive = d.node(NodeType::SvgFe, rows, &[(PropId::Fe, "feFlood")]);
    d.children(filter, &[primitive]);
    let rect = d.node(
        NodeType::SvgRect,
        &[
            (StyleId::Width, "100"),
            (StyleId::Height, "100"),
            (StyleId::Filter, "url(#filter)"),
        ],
        &[],
    );
    d.children(svg, &[filter, rect]);
    let k = d.kernel(&[svg], 800.0);
    let s = resolve(&k, svg);
    find(&s.items, rect)
        .unwrap()
        .filter
        .as_ref()
        .unwrap()
        .primitives[0]
        .subregion
}

#[test]
fn undefined_primitive_lengths_keep_the_filter_region() {
    for units in ["userSpaceOnUse", "objectBoundingBox"] {
        let initial = subregion(units, &[]);
        assert_eq!((initial.0, initial.1), (-10.0, -10.0));
        assert!((initial.2 - 120.0).abs() < 0.001 && (initial.3 - 120.0).abs() < 0.001);
        for id in [StyleId::X, StyleId::Y, StyleId::Width, StyleId::Height] {
            for value in UNDEFINED {
                assert_eq!(
                    subregion(units, &[(id, value)]),
                    initial,
                    "{units} {id:?}: {value}"
                );
            }
            assert_ne!(
                subregion(units, &[(id, "0")]),
                initial,
                "{units} {id:?}: zero"
            );
            assert_eq!(
                subregion(units, &[(id, FALLBACK)]),
                subregion(units, &[(id, "25%")]),
                "{units} {id:?}: fallback"
            );
        }
    }
}
