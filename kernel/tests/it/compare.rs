//! CSS's `min()`, `max()` and `clamp()` on a dimension row (LLP 1001 §2,
//! 2026-10-07): over px, the safe-area insets and the viewport lengths, and
//! `calc()` sums of them, resolved where every `env()` length is — so a
//! change of the insets moves what reads them — and refused by name where a
//! form is outside that grammar.

use exact_kernel::error::StyleValueError;
use exact_kernel::{
    Dimension, Edge, Env, Kernel, NodeType, Offer, Op, StyleId, StyleProps, StyleValue,
};

fn text(row: StyleId, value: &str) -> Result<Dimension, StyleValueError> {
    let mut style = StyleProps::default();
    style.set_dynamic(row, &StyleValue::Text(value.into()))?;
    match style.get(row) {
        exact_kernel::RowValue::Dimension(d) => Ok(d),
        other => panic!("{value}: {other:?}"),
    }
}

fn css(d: Dimension) -> String {
    let Dimension::Compare(c) = d else {
        panic!("not a comparison: {d:?}");
    };
    let mut out = String::new();
    c.css(&mut out);
    out
}

/// Bluesky's bottom bar and compose button: a root the phone's height, a
/// bar padded by `clamp(15px, env(safe-area-inset-bottom), 60px)` at its
/// bottom, and a button positioned that far plus 15 and 44 above it.
fn bar_and_button() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::Height, &StyleValue::Percent(100.0))
        .unwrap();
    let mut bar = StyleProps::default();
    for (row, value) in [
        (StyleId::PositionType, "absolute"),
        (StyleId::Bottom, "0px"),
        (StyleId::Height, "49px"),
        (StyleId::BoxSizing, "content-box"),
        (
            StyleId::PaddingBottom,
            "clamp(15px, env(safe-area-inset-bottom), 60px)",
        ),
    ] {
        bar.set_dynamic(row, &StyleValue::Text(value.into()))
            .unwrap();
    }
    let mut button = StyleProps::default();
    for (row, value) in [
        (StyleId::PositionType, "absolute"),
        (StyleId::Height, "56px"),
        (
            StyleId::Bottom,
            "calc(clamp(15px, env(safe-area-inset-bottom), 60px) + 15px + 44px)",
        ),
    ] {
        button
            .set_dynamic(row, &StyleValue::Text(value.into()))
            .unwrap();
    }
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
    ];
    for (id, patch) in [(2, bar), (3, button)] {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
        ops.push(Op::SetStyle {
            id,
            patch: Box::new(patch),
        });
    }
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![2, 3],
    });
    ops.push(Op::AttachRoot { id: 1 });
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn frame(kernel: &Kernel, id: u32) -> (f32, f32, f32, f32) {
    let f = kernel.node(id).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn clamp_of_the_bottom_inset_follows_the_insets_within_its_bounds() {
    let mut kernel = bar_and_button();
    let offer = Offer::definite(402.0, 874.0);
    // No inset (a browser without `viewport-fit=cover`, a Mac): the floor.
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        frame(&kernel, 2),
        (0.0, 874.0 - 49.0 - 15.0, 0.0, 49.0 + 15.0)
    );
    assert_eq!(frame(&kernel, 3).1, 874.0 - (15.0 + 15.0 + 44.0) - 56.0);
    // The home indicator's 34: the inset itself.
    assert!(kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(frame(&kernel, 2).3, 49.0 + 34.0);
    assert_eq!(frame(&kernel, 3).1, 874.0 - (34.0 + 15.0 + 44.0) - 56.0);
    // Past the ceiling: 60.
    assert!(kernel.set_env(Env::new(62.0, 0.0, 80.0, 0.0)).unwrap());
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(frame(&kernel, 2).3, 49.0 + 60.0);
    assert_eq!(frame(&kernel, 3).1, 874.0 - (60.0 + 15.0 + 44.0) - 56.0);
}

#[test]
fn comparisons_resolve_as_css_says() {
    let at = |value: &str, env: &Env| text(StyleId::Width, value).unwrap().resolve(env);
    let phone = Env {
        viewport_width: 402.0,
        viewport_height: 874.0,
        ..Env::new(62.0, 0.0, 34.0, 0.0)
    };
    let mac = Env {
        viewport_width: 1200.0,
        viewport_height: 800.0,
        ..Env::default()
    };
    for (value, on_phone, on_mac) in [
        ("max(15px, env(safe-area-inset-bottom))", 34.0, 15.0),
        ("min(15px, env(safe-area-inset-bottom))", 15.0, 0.0),
        ("clamp(15px, env(safe-area-inset-bottom), 60px)", 34.0, 15.0),
        // `clamp(MIN, VAL, MAX)` is `max(MIN, min(VAL, MAX))`: MIN wins.
        ("clamp(40px, env(safe-area-inset-top), 20px)", 40.0, 40.0),
        // Sums inside the arguments, with or without `calc()`.
        (
            "max(env(safe-area-inset-top) - 20px, calc(env(safe-area-inset-bottom) + 2px))",
            42.0,
            2.0,
        ),
        // Nested in each other and in `calc()`.
        (
            "calc(min(max(10px, env(safe-area-inset-top)), 50px) + 1in)",
            50.0 + 96.0,
            10.0 + 96.0,
        ),
        (
            "calc(5px + (max(1px, env(safe-area-inset-bottom)) - 4px))",
            35.0,
            2.0,
        ),
        ("min(50vw, 300px)", 201.0, 300.0),
        ("max(10vh, env(safe-area-inset-top))", 87.4, 80.0),
    ] {
        assert_eq!(at(value, &phone), Dimension::Points(on_phone), "{value}");
        assert_eq!(at(value, &mac), Dimension::Points(on_mac), "{value}");
    }
}

#[test]
fn a_comparison_keeps_its_css_and_folds_what_reads_nothing() {
    for (value, written) in [
        (
            "clamp(15px,env(safe-area-inset-bottom),60px)",
            "clamp(15px, env(safe-area-inset-bottom), 60px)",
        ),
        (
            "calc(clamp(15px, env(safe-area-inset-bottom), 60px) + 15px + 44px)",
            "calc(clamp(15px, env(safe-area-inset-bottom), 60px) + 59px)",
        ),
        (
            "max(env(safe-area-inset-top) - 20px, 0)",
            "max(calc(env(safe-area-inset-top) - 20px), 0px)",
        ),
        ("min(50vw, 300px)", "min(50vw, 300px)"),
        (
            "calc(max(10px, 2px) + max(1px, 2vh))",
            "calc(max(1px, 2vh) + 10px)",
        ),
    ] {
        let d = text(StyleId::PaddingBottom, value).unwrap();
        assert_eq!(css(d), written, "{value}");
        // The written text is the same length, by the same handle.
        assert_eq!(text(StyleId::PaddingBottom, written), Ok(d), "{written}");
    }
    // Nothing reads the environment: points.
    assert_eq!(
        text(StyleId::Width, "max(10px, 2px)"),
        Ok(Dimension::Points(10.0))
    );
    assert_eq!(
        text(StyleId::Width, "calc(clamp(1px, 20px, 8px) + 2px)"),
        Ok(Dimension::Points(10.0))
    );
    // An inset plus a folded comparison is the inset's own length.
    assert_eq!(
        text(
            StyleId::Width,
            "calc(env(safe-area-inset-top) + min(4px, 6px))"
        ),
        Ok(Dimension::Env(Edge::Top, 4.0))
    );
    assert_eq!(
        format!(
            "{:?}",
            text(StyleId::Width, "min(1px, env(safe-area-inset-top))").unwrap()
        ),
        "Compare(Comparison(\"min(1px, env(safe-area-inset-top))\"))"
    );
}

/// CSS clamps a math function to the property's range: a radius that
/// would resolve below zero is zero.
#[test]
fn a_radius_comparison_never_resolves_below_zero() {
    let r = text(
        StyleId::BorderRadiusTopLeft,
        "min(-4px, env(safe-area-inset-top))",
    )
    .unwrap();
    assert_eq!(css(r), "max(0px, min(-4px, env(safe-area-inset-top)))");
    assert_eq!(
        r.resolve(&Env::new(62.0, 0.0, 0.0, 0.0)),
        Dimension::Points(0.0)
    );
    let kept = "max(0px, min(-4px, env(safe-area-inset-top)))";
    assert_eq!(
        text(StyleId::BorderRadiusTopLeft, kept),
        Ok(r),
        "not wrapped twice"
    );
    // Folded points: clamped too, where a literal `-4px` is refused.
    assert_eq!(
        text(StyleId::BorderRadiusTopLeft, "max(-8px, -2px)"),
        Ok(Dimension::Points(0.0))
    );
    assert!(text(StyleId::BorderRadiusTopLeft, "-4px").is_err());
    // Folded to an inset or a viewport length that can go negative: held at 0.
    let r = text(
        StyleId::BorderRadiusTopLeft,
        "calc(env(safe-area-inset-top) - max(8px, 12px))",
    )
    .unwrap();
    assert_eq!(css(r), "max(0px, calc(env(safe-area-inset-top) - 12px))");
    assert_eq!(r.resolve(&Env::default()), Dimension::Points(0.0));
    let r = text(StyleId::BorderRadiusTopLeft, "calc(-20vw + max(0px, 0px))").unwrap();
    assert_eq!(css(r), "max(0px, -20vw)");
    assert_eq!(
        text(
            StyleId::BorderRadiusTopLeft,
            "calc(env(safe-area-inset-top) + max(2px, 4px))"
        ),
        Ok(Dimension::Env(Edge::Top, 4.0))
    );
    // `minmax()` is no comparison: the other grammars refuse it as before.
    assert!(matches!(
        text(StyleId::Width, "minmax(0px, 1fr)"),
        Err(StyleValueError::WrongKind { .. })
    ));
    // A tree with no room left for the wrap is refused, never held negative.
    let deep = format!(
        "{}-4px, env(safe-area-inset-top){}",
        "min(".repeat(8),
        ")".repeat(8)
    );
    assert!(
        text(StyleId::MarginTop, &deep).is_ok(),
        "{:?}",
        text(StyleId::MarginTop, &deep)
    );
    match text(StyleId::BorderRadiusTopLeft, &deep) {
        Err(StyleValueError::BadComparison { reason, .. }) => assert!(reason.contains("7 deep")),
        other => panic!("{other:?}"),
    }
    // Another row keeps CSS's negative margin.
    let m = text(StyleId::MarginTop, "min(-4px, env(safe-area-inset-top))").unwrap();
    assert_eq!(
        m.resolve(&Env::new(62.0, 0.0, 0.0, 0.0)),
        Dimension::Points(-4.0)
    );
}

#[test]
fn forms_outside_the_grammar_are_refused_by_name() {
    let deep = format!(
        "{}env(safe-area-inset-top){}",
        "max(1px, ".repeat(9),
        ")".repeat(9)
    );
    for (value, says) in [
        (
            "clamp(1px, env(safe-area-inset-top))",
            "exactly three arguments",
        ),
        (
            "clamp(1px, env(safe-area-inset-top), 2px, 3px)",
            "exactly three arguments",
        ),
        ("min()", "one or more arguments"),
        ("max(10%, env(safe-area-inset-top))", "percentage"),
        (
            "max(calc(50% - 4px), env(safe-area-inset-top))",
            "percentage",
        ),
        ("max(15, env(safe-area-inset-bottom))", "takes a unit"),
        ("max(1rem, env(safe-area-inset-bottom))", "rem and em"),
        ("max(env(safe-area-inset-top, 0px), 1px)", "no fallback"),
        ("max(env(safe-area-inset-middle), 1px)", "env() names"),
        (
            "max(env(viewport-segment-width 0 0), 1px)",
            "viewport-segment",
        ),
        ("max(var(--inset), 1px)", "the functions are"),
        ("max(1px, auto)", "take px"),
        (
            "calc(env(safe-area-inset-top) + max(env(safe-area-inset-bottom), 1px))",
            "at most one",
        ),
        (
            "calc(60px - max(env(safe-area-inset-bottom), 1px))",
            "negates",
        ),
        (
            "calc(max(1px, env(safe-area-inset-bottom))+4px)",
            "white space",
        ),
        (
            "calc(max(1px, env(safe-area-inset-bottom)) * 2)",
            "`*` and `/`",
        ),
        (
            "max(1px, env(safe-area-inset-bottom)) + 4px",
            "inside calc()",
        ),
        (
            "max(1px env(safe-area-inset-bottom))",
            "separated by commas",
        ),
        (
            "max(1px, env(safe-area-inset-bottom)",
            "separated by commas",
        ),
        (deep.as_str(), "8 deep"),
        ("(max(1px, env(safe-area-inset-bottom)))", "inside calc()"),
        (
            "calc(calc(env(safe-area-inset-top) + 3e38px) + 3e38px + min(1px, 2px))",
            "finite",
        ),
    ] {
        match text(StyleId::PaddingBottom, value) {
            Err(StyleValueError::BadComparison { style, reason }) => {
                assert_eq!(style, StyleId::PaddingBottom);
                assert!(reason.contains(says), "{value}: {reason}");
            }
            other => panic!("{value}: {other:?}"),
        }
    }
}

#[test]
fn a_comparison_travels_the_wire_and_marks_its_node_as_reading_the_insets() {
    use exact_kernel::wire;
    let mut style = StyleProps::default();
    let value = "calc(clamp(env(safe-area-inset-bottom), 15px, min(60px, 10vh)) - 2px)";
    style
        .set_dynamic(StyleId::MarginBottom, &StyleValue::Text(value.into()))
        .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 7,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 7,
            patch: Box::new(style),
        },
        Op::AttachRoot { id: 7 },
    ];
    let mut other = Kernel::with_monospace();
    other.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
    let margin = other.node(7).unwrap().style.margin_bottom;
    assert_eq!(margin, text(StyleId::MarginBottom, value).unwrap());
    assert_eq!(css(margin), value);
    other
        .compute_layout(7, Offer::definite(402.0, 874.0))
        .unwrap();
    assert!(other.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
}
