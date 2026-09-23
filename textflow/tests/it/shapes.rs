use exact_textflow::{meets, FlowShape, ShapeOutside};
use std::sync::Arc;

fn resolve(css: &str, w: f32, h: f32) -> FlowShape {
    ShapeOutside::parse(css).unwrap().resolve(w, h)
}
fn rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> FlowShape {
    FlowShape::RoundRect {
        x,
        y,
        width: w,
        height: h,
        radius: r,
    }
}
#[test]
fn accepted_forms_round_trip() {
    for css in [
        "none",
        "NONE",
        "circle()",
        "circle(at center)",
        "circle(2)",
        "circle(2px)",
        "circle(20%)",
        "circle(closest-side)",
        "circle(farthest-side at 30% -5px)",
        "circle(.2e2 at center)",
        "ellipse()",
        "ellipse(at center)",
        "ellipse(10px 20%)",
        "ellipse(closest-side farthest-side at 10 30%)",
        "inset(1)",
        "inset(1 2)",
        "inset(1 2 3)",
        "inset(1 2 3 4 round 5%)",
        "inset(-2% round 3px)",
        "polygon(0 0)",
        "polygon(0 0, 1 1)",
        "polygon(0 0, 100% 0, 50% 100%)",
        "polygon(evenodd, 0 0, 100% 0, 50% 100%)",
        "polygon(nonzero, 0 0, 1px 0, 0 1px)",
    ] {
        let value = ShapeOutside::parse(css).unwrap_or_else(|| panic!("{css}"));
        assert_eq!(ShapeOutside::parse(&value.css()), Some(value), "{css}");
    }
    assert!(ShapeOutside::default().is_none());
    assert!(!ShapeOutside::parse("circle()").unwrap().is_none());
    let points = (0..64)
        .map(|i| format!("{i} {i}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(ShapeOutside::parse(&format!("polygon({points})")).is_some());
    assert!(ShapeOutside::parse(&format!("polygon({points}, 65 65)")).is_none());
}
#[test]
fn rejected_vocabulary_and_malformed_values() {
    for css in [
        "",
        "auto",
        "none none",
        "none()",
        "path('M 0 0')",
        "url(x)",
        "border-box",
        "circle() border-box",
        "circle (1)",
        "circle(1em)",
        "circle(-1)",
        "circle(NaN)",
        "circle(inf)",
        "circle(1e40)",
        "circle(at left)",
        "circle(at left top)",
        "circle(at 1)",
        "circle(1 2)",
        "circle(1 at center center)",
        "circle(1 at 1 2 3)",
        "circle(calc(1px))",
        "ellipse(1)",
        "ellipse(1 -2)",
        "ellipse(1 2 3)",
        "inset()",
        "inset(1 2 3 4 5)",
        "inset(1 round)",
        "inset(1 round -2)",
        "inset(1 round 2 3)",
        "inset(1 round 2 / 3)",
        "inset(1 / 2)",
        "polygon()",
        "polygon(evenodd)",
        "polygon(0, 1)",
        "polygon(0 0,)",
        "polygon(0 0 1 1)",
        "polygon(winding, 0 0)",
        "circle(1))",
        "circle(1) junk",
        "circle(1%px)",
    ] {
        assert!(ShapeOutside::parse(css).is_none(), "accepted {css}");
    }
}
#[test]
fn resolves_circle_rules_and_ellipse_axes() {
    assert_eq!(
        resolve("none", 200.0, 100.0),
        rect(0.0, 0.0, 200.0, 100.0, 0.0)
    );
    assert_eq!(
        resolve("circle()", 200.0, 100.0),
        FlowShape::Circle {
            cx: 100.0,
            cy: 50.0,
            r: 50.0
        }
    );
    assert_eq!(
        resolve("circle(closest-side at 20 30)", 200.0, 100.0),
        FlowShape::Circle {
            cx: 20.0,
            cy: 30.0,
            r: 20.0
        }
    );
    assert_eq!(
        resolve("circle(farthest-side at 20 30)", 200.0, 100.0),
        FlowShape::Circle {
            cx: 20.0,
            cy: 30.0,
            r: 180.0
        }
    );
    assert_eq!(
        resolve("circle(closest-side at -10 30)", 200.0, 100.0),
        FlowShape::Circle {
            cx: -10.0,
            cy: 30.0,
            r: 10.0
        }
    );
    let FlowShape::Circle { r, .. } = resolve("circle(50%)", 300.0, 400.0) else {
        panic!()
    };
    assert!((r - 250.0 / 2.0_f32.sqrt()).abs() < 0.0001);
    assert_eq!(
        resolve("ellipse()", 200.0, 100.0),
        FlowShape::Ellipse {
            cx: 100.0,
            cy: 50.0,
            rx: 100.0,
            ry: 50.0
        }
    );
    assert_eq!(
        resolve("ellipse(10% 20% at 25% 30%)", 200.0, 100.0),
        FlowShape::Ellipse {
            cx: 50.0,
            cy: 30.0,
            rx: 20.0,
            ry: 20.0
        }
    );
    assert_eq!(
        resolve(
            "ellipse(closest-side farthest-side at -10 30)",
            200.0,
            100.0
        ),
        FlowShape::Ellipse {
            cx: -10.0,
            cy: 30.0,
            rx: 10.0,
            ry: 70.0
        }
    );
}
#[test]
fn resolves_inset_shorthand_percent_overlap_and_radius() {
    for (css, expected) in [
        ("inset(10)", rect(10.0, 10.0, 180.0, 80.0, 0.0)),
        ("inset(10 20)", rect(20.0, 10.0, 160.0, 80.0, 0.0)),
        ("inset(10 20 30)", rect(20.0, 10.0, 160.0, 60.0, 0.0)),
        ("inset(10% 20% 30% 40%)", rect(80.0, 10.0, 80.0, 60.0, 0.0)),
        ("inset(10 round 100)", rect(10.0, 10.0, 180.0, 80.0, 40.0)),
        ("inset(10 round 10%)", rect(10.0, 10.0, 180.0, 80.0, 10.0)),
        ("inset(-10)", rect(-10.0, -10.0, 220.0, 120.0, 0.0)),
    ] {
        assert_eq!(resolve(css, 200.0, 100.0), expected, "{css}");
    }
    assert_eq!(
        resolve("inset(60%)", 200.0, 100.0).bounds(),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        resolve("none", f32::NAN, -1.0).bounds(),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        resolve("polygon(0 0, 100% 0, 50% 100%)", 200.0, 100.0),
        FlowShape::Polygon(Arc::from([(0.0, 0.0), (200.0, 0.0), (100.0, 100.0)]))
    );
}
#[test]
fn grow_translate_bounds_and_meets() {
    let circle = FlowShape::Circle {
        cx: 10.0,
        cy: 20.0,
        r: 5.0,
    };
    assert_eq!(
        circle.translate(-3.0, 4.0).grow(2.0).bounds(),
        (0.0, 17.0, 14.0, 31.0)
    );
    assert_eq!(circle.grow(-1.0), circle);
    assert_eq!(circle.grow(f32::NAN), circle);
    let ellipse = FlowShape::Ellipse {
        cx: 10.0,
        cy: 20.0,
        rx: 5.0,
        ry: 10.0,
    };
    assert_eq!(ellipse.grow(2.0).bounds(), (3.0, 8.0, 17.0, 32.0));
    assert_eq!(
        rect(10.0, 20.0, 30.0, 40.0, 5.0).grow(2.0),
        rect(8.0, 18.0, 34.0, 44.0, 7.0)
    );
    assert!(meets(std::slice::from_ref(&circle), 0.0, 0.0, 20.0, 20.0));
    assert!(!meets(std::slice::from_ref(&circle), 15.0, 0.0, 30.0, 30.0));
    assert!(!meets(&[circle], 0.0, 0.0, f32::NAN, 30.0));
    let spans = FlowShape::Spans {
        x: 10.0,
        y: 20.0,
        row_height: 2.0,
        rows: Arc::from([(0.0, 0.0), (2.0, 5.0), (1.0, 4.0), (0.0, 0.0)]),
    };
    assert_eq!(spans.bounds(), (11.0, 22.0, 15.0, 26.0));
    assert_eq!(
        spans.translate(3.0, -2.0).bounds(),
        (14.0, 20.0, 18.0, 24.0)
    );
    assert_eq!(spans.grow(2.0).bounds(), (9.0, 20.0, 17.0, 28.0));
    let triangle = resolve("polygon(0 0, 10 0, 5 10)", 10.0, 10.0);
    assert_eq!(triangle.grow(2.0).bounds(), (-2.0, -2.0, 12.0, 12.0));
    assert_eq!(
        triangle.translate(2.0, 3.0).bounds(),
        (2.0, 3.0, 12.0, 13.0)
    );
}
