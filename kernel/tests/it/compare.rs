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
    match d {
        Dimension::Compare(c) => {
            let mut out = String::new();
            c.css(&mut out);
            out
        }
        _ => panic!("not a calculation: {d:?}"),
    }
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
            "calc(59px + clamp(15px, env(safe-area-inset-bottom), 60px))",
        ),
        (
            "max(env(safe-area-inset-top) - 20px, 0px)",
            "max(-20px + env(safe-area-inset-top), 0px)",
        ),
        ("min(50vw, 300px)", "min(50vw, 300px)"),
        (
            "calc(max(10px, 2px) + max(1px, 2vh))",
            "calc(10px + max(1px, 2vh))",
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

/// CSS clamps a math function to the property's range (CSS Values 4
/// §10.12). On a row without negative lengths a comparison folded to points
/// is held at 0 or more, as the web writes it; one that resolves later keeps
/// its text and is clamped where it is used, as a `calc()` is — a size by
/// the engine, a radius by its painter. Margins and insets keep negatives.
#[test]
fn a_comparison_below_zero_is_clamped_where_it_is_used() {
    for row in [
        StyleId::Width,
        StyleId::MinHeight,
        StyleId::MaxWidth,
        StyleId::PaddingTop,
        StyleId::FlexBasis,
        StyleId::Rx,
        StyleId::ColumnWidth,
        StyleId::BorderRadiusTopLeft,
    ] {
        assert_eq!(
            text(row, "min(-8px, -2px)"),
            Ok(Dimension::Points(0.0)),
            "{row:?}"
        );
        let d = text(row, "min(-4px, env(safe-area-inset-top))").unwrap();
        assert_eq!(css(d), "min(-4px, env(safe-area-inset-top))", "{row:?}");
    }
    assert!(text(StyleId::BorderRadiusTopLeft, "-4px").is_err());
    for row in [StyleId::MarginLeft, StyleId::Top, StyleId::Cx] {
        assert_eq!(
            text(row, "min(-8px, -2px)"),
            Ok(Dimension::Points(-8.0)),
            "{row:?}"
        );
    }
    // A width that resolves to -4 lays out as 0
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::Width, &StyleValue::Number(400.0))
        .unwrap();
    let mut child = StyleProps::default();
    for (row, value) in [
        (StyleId::Width, "min(-4px, env(safe-area-inset-top))"),
        (StyleId::Height, "10px"),
        (StyleId::PaddingLeft, "min(-6px, 10% - 50px)"),
    ] {
        child
            .set_dynamic(row, &StyleValue::Text(value.into()))
            .unwrap();
    }
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(child),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
        .compute_layout(1, Offer::definite(400.0, 800.0))
        .unwrap();
    assert_eq!(
        frame(&kernel, 2).2,
        0.0,
        "a negative width and padding are 0"
    );
    // Its written CSS reads back to it: pixels before a subtracted function
    // and `-1 * ` before one alone, a sum in an argument without its own
    // `calc()`, a product inside one
    for value in [
        "calc(2 * min(10%, 20px))",
        "max(-20px + env(safe-area-inset-top), 0px)",
        "calc(50px - max(env(safe-area-inset-top), 4px))",
        "calc(-1 * max(env(safe-area-inset-top), 4px))",
        "clamp(none, 2 * env(safe-area-inset-bottom), 50%)",
    ] {
        let d = text(StyleId::MarginTop, value).unwrap();
        assert_eq!(css(d), value);
        assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d), "{value}");
    }
    // At the limits: 32 nested comparisons and 32 values are an author's,
    // whatever the row
    let deep = format!(
        "{}-4px, env(safe-area-inset-top){}",
        "min(".repeat(32),
        ")".repeat(32)
    );
    let r = text(StyleId::BorderRadiusTopLeft, &deep);
    assert!(r.is_ok(), "{r:?}");
    let wide = format!(
        "min({})",
        vec!["calc(env(safe-area-inset-top) - 1px)"; 16].join(", ")
    );
    assert!(text(StyleId::Width, &wide).is_ok());
}

#[test]
fn forms_outside_the_grammar_are_refused_by_name() {
    let deep = format!(
        "{}env(safe-area-inset-top){}",
        "max(1px, ".repeat(33),
        ")".repeat(33)
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
        ("max(none, env(safe-area-inset-top))", "first or third"),
        (
            "clamp(1px, none, env(safe-area-inset-top))",
            "first or third",
        ),
        ("max(15, env(safe-area-inset-bottom))", "takes a unit"),
        // Zero too: inside a math function it is a number, not a length.
        ("max(0, 10px)", "zero too"),
        ("max(env(safe-area-inset-top) - 20px, 0)", "zero too"),
        ("max(1rem, env(safe-area-inset-bottom))", "rem and em"),
        ("max(env(safe-area-inset-middle), 1px)", "env() names"),
        ("max(var(--inset), 1px)", "names a content owner's value"),
        ("max(attr(x), 1px)", "the functions are"),
        ("max(1px, auto)", "take px"),
        (
            "calc(max(1px, env(safe-area-inset-bottom))+4px)",
            "white space",
        ),
        (
            "calc(max(1px, env(safe-area-inset-bottom)) * 2px)",
            "a length times or over a length",
        ),
        (
            "calc(max(1px, env(safe-area-inset-bottom)) / 2px)",
            "a length times or over a length",
        ),
        (
            "max(1px, env(safe-area-inset-bottom)) + 4px",
            "inside calc()",
        ),
        (
            "max(1px env(safe-area-inset-bottom))",
            "separated by commas",
        ),
        (deep.as_str(), "32 deep"),
        ("(max(1px, env(safe-area-inset-bottom)))", "inside calc()"),
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
    assert_eq!(
        css(margin),
        "calc(-2px + clamp(env(safe-area-inset-bottom), 15px, min(60px, 10vh)))"
    );
    other
        .compute_layout(7, Offer::definite(402.0, 874.0))
        .unwrap();
    assert!(other.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
}

/// LLP 1001 §2: the comparisons take what `calc()` takes — a percentage,
/// segments, sums of several terms, subtraction and products by a number —
/// and `clamp()` takes `none`.
#[test]
fn the_comparisons_take_what_calc_takes() {
    exact_kernel::link_segments();
    assert_eq!(
        text(StyleId::Width, "clamp(100px, 20px, 50px)"),
        Ok(Dimension::Points(100.0))
    );
    assert_eq!(
        text(StyleId::Width, "clamp(none, 20px, 50px)"),
        Ok(Dimension::Points(20.0))
    );
    let at = |value: &str, env: &Env| text(StyleId::MarginTop, value).unwrap().resolve(env);
    let phone = Env::new(62.0, 0.0, 34.0, 0.0);
    assert_eq!(
        at("clamp(none, env(safe-area-inset-top), 50px)", &phone),
        Dimension::Points(50.0)
    );
    assert_eq!(
        at("clamp(20px, env(safe-area-inset-top), none)", &phone),
        Dimension::Points(62.0)
    );
    // Two variable terms, a negated comparison and a product by a number
    assert_eq!(
        at(
            "calc(env(safe-area-inset-top) + 2 * max(env(safe-area-inset-bottom), 1px) - min(1px, env(safe-area-inset-left)))",
            &phone
        ),
        Dimension::Points(62.0 + 68.0)
    );
    // An undefined segment in an argument the comparison would not select
    // still makes the whole value invalid at computed-value time: `auto`
    assert_eq!(
        at("max(10px, env(viewport-segment-width 1 0))", &phone),
        Dimension::Auto
    );
    assert_eq!(
        css(text(
            StyleId::MarginLeft,
            "min(50%, calc(100px - env(safe-area-inset-left)))"
        )
        .unwrap()),
        "min(50%, 100px - env(safe-area-inset-left))"
    );
}

/// A comparison with a percentage resolves against its basis at layout, and
/// under the environment of that layout: a change of the insets moves it.
#[test]
fn a_percentage_in_a_comparison_resolves_against_its_basis_at_layout() {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    root.set_dynamic(StyleId::Width, &StyleValue::Number(400.0))
        .unwrap();
    let mut child = StyleProps::default();
    child
        .set_dynamic(StyleId::Height, &StyleValue::Number(10.0))
        .unwrap();
    child
        .set_dynamic(
            StyleId::Width,
            &StyleValue::Text("min(50%, calc(env(safe-area-inset-top) + 10px))".into()),
        )
        .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(child),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    let offer = Offer::definite(402.0, 874.0);
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(frame(&kernel, 2).2, 10.0, "the inset term, at no inset");
    assert!(kernel.set_env(Env::new(62.0, 0.0, 34.0, 0.0)).unwrap());
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(frame(&kernel, 2).2, 72.0, "the inset term");
    assert!(kernel.set_env(Env::new(300.0, 0.0, 34.0, 0.0)).unwrap());
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(frame(&kernel, 2).2, 200.0, "half the 400-point parent");
}

/// What a host or the kernel reads where it sizes a box itself — a native
/// button's padding and radius, a painter's radius — resolves every length
/// against its basis, a comparison with a percentage too.
#[test]
fn a_length_resolves_against_a_basis_whatever_its_form() {
    let env = Env::new(62.0, 0.0, 34.0, 0.0);
    for (value, basis, points) in [
        ("min(10%, 20px)", 400.0, 20.0),
        ("min(10%, 20px)", 100.0, 10.0),
        ("calc(50% - env(safe-area-inset-top))", 200.0, 38.0),
        ("max(env(safe-area-inset-bottom), 5%)", 1000.0, 50.0),
        ("25%", 80.0, 20.0),
        ("calc(10% + 4px)", 100.0, 14.0),
    ] {
        let d = text(StyleId::PaddingTop, value).unwrap();
        assert_eq!(d.at_basis(&env, basis), points, "{value} at {basis}");
    }
    assert_eq!(Dimension::Auto.at_basis(&env, 100.0), 0.0);
}

/// A number times a sum of comparisons keeps the sum whole, as written:
/// spreading it would write more values than the author did, past the 32
/// CSS Values 4 §10.8 promises (Sol's review, stage 4 round 2).
#[test]
fn a_scaled_sum_is_written_whole_within_the_author_s_values() {
    let d = text(
        StyleId::MarginTop,
        "calc(2 * (min(1%, 1px) + min(2%, 2px)))",
    )
    .unwrap();
    assert_eq!(css(d), "calc(2 * (min(1%, 1px) + min(2%, 2px)))");
    let picks = (1..=15)
        .map(|i| format!("min({i}%, {i}px)"))
        .collect::<Vec<_>>()
        .join(" + ");
    let authored = format!("calc(2 * ({picks}))");
    let d = text(StyleId::MarginTop, &authored).unwrap();
    assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d), "{authored}");
    // One comparison takes the number as its coefficient
    let d = text(
        StyleId::MarginTop,
        "calc(3 * min(10%, env(safe-area-inset-top)))",
    )
    .unwrap();
    assert_eq!(css(d), "calc(3 * min(10%, env(safe-area-inset-top)))");
    // A sum of terms is kept whole too: distributed, 17 products would be
    // 34 values, past the budget the author's 18 fit
    exact_kernel::link_segments();
    let terms = (0..16)
        .map(|i| format!("env(viewport-segment-width {i} 0)"))
        .chain(["env(viewport-segment-height 0 0)".to_string()])
        .collect::<Vec<_>>()
        .join(" + ");
    let authored = format!("calc(2 * ({terms}))");
    let d = text(StyleId::MarginTop, &authored).unwrap();
    assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d), "{authored}");
    // A negation still takes the sign into each part, writing no number
    let d = text(
        StyleId::MarginTop,
        "calc(-1 * (env(safe-area-inset-top) + 4px))",
    )
    .unwrap();
    assert_eq!(css(d), "calc(-1 * (4px + env(safe-area-inset-top)))");
}

/// A sum keeps what CSS Values 4 §10.10.1 keeps: a zero-valued term is
/// combined only with values of its unit, never removed, and `env()`, a
/// non-math function, is never combined at all.
#[test]
fn a_sum_keeps_its_zero_terms_and_each_env() {
    for (value, written) in [
        ("calc(0vw + 10px)", "calc(10px + 0vw)"),
        ("calc(10vw - 10vw + 20px)", "calc(20px + 0vw)"),
        (
            "calc(env(safe-area-inset-top) + 0px)",
            "calc(0px + env(safe-area-inset-top))",
        ),
        (
            "calc(env(safe-area-inset-bottom) - env(safe-area-inset-bottom) + 2px)",
            "calc(2px + env(safe-area-inset-bottom) - env(safe-area-inset-bottom))",
        ),
    ] {
        let d = text(StyleId::MarginTop, value).unwrap();
        assert_eq!(css(d), written, "{value}");
        assert_eq!(text(StyleId::MarginTop, written), Ok(d), "{written}");
    }
    // A zero percentage beside pixels is the compact form that holds both
    assert_eq!(
        text(StyleId::Width, "calc(20px + 0%)"),
        Ok(Dimension::Calc(0.0, 20.0))
    );
}

/// An SVG radius below zero is SVG's error when written as a number, and a
/// calculation's is clamped to zero, even where it folds to a number, as a
/// corner radius's is (CSS Values 4 §10.12).
#[test]
fn a_negative_calculated_svg_radius_is_zero() {
    assert_eq!(
        text(StyleId::Rx, "calc(0px - 4px)"),
        Ok(Dimension::Points(0.0))
    );
    assert_eq!(text(StyleId::Ry, "calc(-10%)"), Ok(Dimension::Percent(0.0)));
    assert_eq!(text(StyleId::R, "-4px"), Ok(Dimension::Points(-4.0)));
}

/// A subtraction writes no number, so it spends none of the 32 values: 24
/// values in twelve comparisons, eleven of them subtracted, are admitted
/// and round-trip (CSS Values 4 §10.8).
#[test]
fn a_subtraction_spends_no_value() {
    let authored = format!(
        "calc(min(1%, 1px){})",
        (2..=12)
            .map(|i| format!(" - min({i}%, {i}px)"))
            .collect::<String>()
    );
    let d = text(StyleId::MarginTop, &authored).unwrap();
    assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d), "{authored}");
}

/// A sum of 32 values, `min(1%)` among them unwrapped to its one argument
/// (§10.10.1), reads back as itself.
#[test]
fn a_subtraction_that_opens_a_sum_is_written_after_its_pixels() {
    let authored = format!(
        "calc(1px - min(1%){})",
        (2..=16)
            .map(|i| format!(" - min({i}%, {i}px)"))
            .collect::<String>()
    );
    let d = text(StyleId::MarginTop, &authored).unwrap();
    assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d), "{authored}");
}

/// No number is past a calculation's range (CSS Values 4 §5, §9.1): an
/// infinity, from the keyword or a division by zero, and a NaN are values
/// inside a tree (§10.9.2), and only the top-level result is censored —
/// NaN to 0, an infinity to the largest length — then clamped to the row.
#[test]
fn infinities_and_nan_stay_inside_and_are_censored_at_the_top() {
    let env = Env::new(0.0, 0.0, 0.0, 0.0);
    for (value, basis, points) in [
        ("calc(infinity * 1px)", 100.0, f32::MAX),
        ("calc(-infinity * 1px)", 100.0, 0.0),
        ("calc(NaN * 1px)", 100.0, 0.0),
        ("min(1px / 0, 50%)", 100.0, 50.0),
        ("calc(1e38px - 1e38px + 20px)", 100.0, 20.0),
        ("calc(1e300 * 1e300 * 0 * 1px + 20px)", 100.0, 0.0),
        ("max(10%, 1e39px)", 100.0, f32::MAX),
        (
            "calc(2 * pi * 1px)",
            100.0,
            (2.0 * std::f64::consts::PI) as f32,
        ),
    ] {
        let d = text(StyleId::Width, value).unwrap_or_else(|e| panic!("{value}: {e:?}"));
        assert_eq!(d.at_basis(&env, basis), points, "{value}: {d:?}");
    }
    // Compact env + px forms must censor overflow just as trees do.
    let large = Env::new(f32::MAX, 0.0, 0.0, 0.0);
    let d = text(
        StyleId::Width,
        "calc(env(safe-area-inset-top) + 340282346638528859811704183484516925440px)",
    )
    .unwrap();
    assert_eq!(d.at_basis(&large, 100.0), f32::MAX);
    // The text of a tree holding one reads back as it
    let d = text(StyleId::MarginTop, "min(infinity * 1px, 10% + NaN * 1px)").unwrap();
    assert_eq!(css(d), "min(infinity * 1px, 10% + (NaN * 1px))");
    assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d));
}

/// A sum opening with a subtracted `env()` is written with a positive part
/// first, so the canonical text of an author's 32 values is 32 values too.
#[test]
fn a_subtracted_env_term_is_written_after_a_positive_part() {
    let authored = format!(
        "calc(1px - env(safe-area-inset-top){})",
        (1..=30)
            .map(|i| format!(" + min({i}%)"))
            .collect::<String>()
    );
    let d = text(StyleId::MarginTop, &authored).unwrap();
    assert!(!css(d).contains("-1 *"), "{}", css(d));
    assert_eq!(text(StyleId::MarginTop, &css(d)), Ok(d), "{authored}");
}

/// A minus goes only on a number: a subtracted function or group is never
/// written `-max()` or `-(…)`, whatever its arguments hold, and every sum
/// of literals, products, functions and groups in either sign reads back as
/// itself (CSS Values 4 §10.1) and crosses the wire.
#[test]
fn every_sum_is_written_as_css_and_reads_back_as_itself() {
    let d = text(
        StyleId::Width,
        "calc(100px - max(2 * env(safe-area-inset-top), 1%))",
    )
    .unwrap();
    assert_eq!(
        css(d),
        "calc(100px - max(2 * env(safe-area-inset-top), 1%))"
    );
    let d = text(StyleId::Width, "calc(-1 * (min(10%, 20px) + 1px))").unwrap();
    assert_eq!(css(d), "calc(-1 * (1px + min(10%, 20px)))");
    let d = text(StyleId::Width, "calc(-1 * (min(10%, 20px) + max(1%, 1px)))").unwrap();
    assert_eq!(css(d), "calc(-1 * (min(10%, 20px) + max(1%, 1px)))");
    let summands = [
        "4px",
        "10%",
        "20vw",
        "env(safe-area-inset-top)",
        "2 * env(safe-area-inset-bottom)",
        "min(10%, 20px)",
        "2 * max(1%, 2px)",
        "(min(1%, 1px) + max(3%, 3px))",
        "max(2 * env(safe-area-inset-left), 1%)",
        "2 * (min(2%, 2px) + 3px) / 2",
    ];
    let mut checked = 0;
    for a in summands {
        for b in summands {
            for c in summands {
                for signs in 0..8 {
                    let sign = |bit: usize| if signs & bit == 0 { "+" } else { "-" };
                    // `0px - a` opens with a subtracted `a`, the zero folding away
                    let authored =
                        format!("calc(0px {} {a} {} {b} {} {c})", sign(1), sign(2), sign(4));
                    let mut style = StyleProps::default();
                    if style
                        .set_dynamic(StyleId::Width, &StyleValue::Text(authored.as_str().into()))
                        .is_err()
                    {
                        continue;
                    }
                    let d = style.width;
                    if !matches!(d, Dimension::Compare(_)) {
                        continue;
                    }
                    let written = css(d);
                    let bytes = written.as_bytes();
                    for (i, _) in written.match_indices('-') {
                        let after_sign = i == 0 || matches!(bytes[i - 1], b'(' | b',' | b' ');
                        let spaced = bytes.get(i + 1) == Some(&b' ');
                        let number = bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit());
                        assert!(
                            !after_sign || spaced || number,
                            "{authored} is written {written}"
                        );
                    }
                    assert_eq!(
                        text(StyleId::Width, &written),
                        Ok(d),
                        "{authored}: {written}"
                    );
                    // The wire admits what the parser makes
                    let ops = [exact_kernel::Op::SetStyle {
                        id: 1,
                        patch: Box::new(style),
                    }];
                    let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
                    assert!(
                        exact_kernel::wire::frame::decode(&bytes).is_ok(),
                        "{authored}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 6000, "{checked}");
}

/// Large numbers that cancel resolve as a browser's doubles do: the kernel
/// folds and evaluates in `f64`, a host's knots agree, and a compact form
/// holds its numbers only when an `f32` holds them exactly.
#[test]
fn large_terms_cancel_before_narrowing_to_layout_precision() {
    let env = Env::new(1.0, 0.0, 1.0, 0.0);
    for value in [
        "calc(1e9 * min(1%, 1px) + 10px - 1e9 * min(1%, 1px))",
        "calc(1e9px - (1e9px - 10px))",
        "calc(999999990px - 999999980px)",
        "calc(999999990px - 999999980 * env(safe-area-inset-top))",
        "calc(1e9 * env(safe-area-inset-top) - 1e9 * env(safe-area-inset-bottom) + 10px)",
        "calc(1e7% - 19999990px)",
        "calc(1e7% + 1px - 19999991px)",
        "calc(1e9 * (min(0.1%, 1px) - 0.2px) + 10px)",
    ] {
        let d = text(StyleId::PaddingTop, value).unwrap_or_else(|e| panic!("{value}: {e:?}"));
        assert_eq!(d.at_basis(&env, 200.0), 10.0, "{value}: {d:?}");
    }
}

/// A group the arithmetic leaves at a coefficient of one or minus one is
/// merged into the sum around it, as its text reads back, so the kernel's
/// own bytes decode.
#[test]
fn a_product_keeps_its_number_and_its_bytes_decode() {
    for (value, css_text) in [
        (
            "calc(2 * (min(50%, 100px) + 1px) / 2)",
            "calc(1 * (1px + min(50%, 100px)))",
        ),
        (
            "calc(4px + 3 * (min(50%, 100px) - env(safe-area-inset-top)) / 3)",
            "calc(4px + (1 * (min(50%, 100px) - env(safe-area-inset-top))))",
        ),
    ] {
        let d = text(StyleId::Width, value).unwrap();
        assert_eq!(css(d), css_text, "{value}");
        assert_eq!(text(StyleId::Width, &css(d)), Ok(d), "{value}");
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::Width, &StyleValue::Text(value.into()))
            .unwrap();
        let ops = [exact_kernel::Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }];
        let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
        exact_kernel::wire::frame::decode(&bytes).unwrap_or_else(|e| panic!("{value}: {e:?}"));
    }
}

/// CSS Values 4 §10.10.1's simplification and §10.13's serialization, by
/// the spec's own examples and rules: like units combine and nothing else
/// does, a number spreads over a sum of values, `min()` and `max()` combine
/// the arguments of one comparable unit and unwrap one left, and the text
/// sorts the number, the percentage, the dimensions by unit, then the rest.
#[test]
fn simplification_and_serialization_are_css_values_4_s() {
    assert_eq!(
        text(StyleId::Width, "calc(20px + 30px)"),
        Ok(Dimension::Points(50.0))
    );
    assert_eq!(
        text(StyleId::Width, "calc(20px + 0%)"),
        Ok(Dimension::Calc(0.0, 20.0))
    );
    assert_eq!(
        text(StyleId::Width, "min(5vw)"),
        Ok(Dimension::Viewport(exact_kernel::ViewportUnit::Vw, 5.0))
    );
    for (value, written) in [
        ("calc(2 * (10px + 5vw))", "calc(20px + 10vw)"),
        ("calc((10px + 5vw) / 2)", "calc(5px + 2.5vw)"),
        ("min(10px, 20px, 5vw)", "min(10px, 5vw)"),
        ("max(10%, 5vw, 2vw, 1px)", "max(10%, 5vw, 1px)"),
        ("calc(50% + 10vw + 1px)", "calc(50% + 1px + 10vw)"),
        ("calc(min(10px, 5vw) + 2px)", "calc(2px + min(10px, 5vw))"),
        (
            "calc(-1 * min(1px, 5vw) - 3px)",
            "calc(-3px + (-1 * min(1px, 5vw)))",
        ),
        (
            "calc(1px - 2 * min(1px, 5vw))",
            "calc(1px - (2 * min(1px, 5vw)))",
        ),
        (
            "calc(10dvh + 10vw + 10svh + 10%)",
            "calc(10% + 10dvh + 10svh + 10vw)",
        ),
    ] {
        let d = text(StyleId::MarginTop, value).unwrap_or_else(|e| panic!("{value}: {e:?}"));
        assert_eq!(css(d), written, "{value}");
        assert_eq!(text(StyleId::MarginTop, written), Ok(d), "{written}");
    }
    // Names and keywords are ASCII case-insensitive, and comments stand
    // between tokens (CSS Syntax 3)
    for (value, points) in [
        ("CALC(1PX + 2px)", 3.0),
        ("Min(4px, 5px)", 4.0),
        ("calc(1px /* one */ + /* two */ 2px)", 3.0),
        ("calc(/* one */1px)", 1.0),
        ("clamp(NONE, 7px, none)", 7.0),
        ("calc(2 * PI * 1px)", (2.0 * std::f64::consts::PI) as f32),
        ("calc(E * 1px)", std::f64::consts::E as f32),
    ] {
        assert_eq!(
            text(StyleId::Width, value),
            Ok(Dimension::Points(points)),
            "{value}"
        );
    }
    assert_eq!(
        text(
            StyleId::Width,
            "calc(env(/* top */safe-area-inset-top) + 2px)"
        ),
        Ok(Dimension::Env(Edge::Top, 2.0))
    );
}

/// `env()` takes a fallback (CSS Environment Variables 1 §3), used where
/// the variable is undefined: a viewport segment on a viewport with one.
#[test]
fn env_s_fallback_stands_in_for_an_undefined_segment() {
    exact_kernel::link_segments();
    let d = text(
        StyleId::Width,
        "calc(env(viewport-segment-width 1 0, 50%) + 10px)",
    )
    .unwrap();
    let env = Env::new(0.0, 0.0, 0.0, 0.0);
    assert_eq!(d.at_basis(&env, 200.0), 110.0);
    // Held as written: substitution reads the text, not a tree of it
    assert_eq!(css(d), "calc(env(viewport-segment-width 1 0, 50%) + 10px)");
    assert_eq!(text(StyleId::Width, &css(d)), Ok(d));
    let inset = text(StyleId::Width, "env(safe-area-inset-top, 9px)").unwrap();
    assert_eq!(inset.at_basis(&Env::new(20.0, 0.0, 0.0, 0.0), 0.0), 20.0);
}

/// What round 10 of review found, held to the specifications: text that is
/// not ASCII is refused, never a panic; a `-0` as written is the unsigned
/// zero (CSS Values 4 §10.9.2); an `env()`'s fallback is a
/// `<declaration-value>`, checked only where it is used, and one unused
/// leaves the value needing no basis (CSS Environment Variables 1 §3); a
/// percentage or a viewport length is held compact only where an `f32`
/// holds it exactly; and a tree within §10.8's limits crosses the wire.
#[test]
fn specification_edges_hold() {
    assert!(text(StyleId::Width, "min(€€, 1px)").is_err());
    assert!(text(StyleId::Width, "mi€(1px)").is_err());
    let env = Env::new(0.0, 0.0, 0.0, 0.0);
    let d = text(StyleId::Width, "min(100px, 1px / -0)").unwrap();
    assert_eq!(d.at_basis(&env, 0.0), 100.0);
    assert_eq!(
        text(StyleId::Width, "calc(-0px)"),
        Ok(Dimension::Points(0.0))
    );
    // An unused fallback: a definite length, needing no basis
    let d = text(
        StyleId::Height,
        "calc(env(safe-area-inset-top, 50%) + 10px)",
    )
    .unwrap();
    assert_eq!(d.resolve(&env), Dimension::Points(10.0));
    // A fallback that is no length is kept as written, and is never used
    // where the variable is defined
    for value in [
        "max(16px, env(safe-area-inset-bottom, 0))",
        "calc(env(safe-area-inset-left, auto) + 1px)",
    ] {
        let d = text(StyleId::Width, value).unwrap_or_else(|e| panic!("{value}: {e:?}"));
        assert_eq!(text(StyleId::Width, &css(d)), Ok(d), "{value}");
    }
    exact_kernel::link_segments();
    let d = text(
        StyleId::Width,
        "calc(env(viewport-segment-width 1 0, auto) + 1px)",
    )
    .unwrap();
    assert_eq!(
        d.resolve(&env),
        Dimension::Auto,
        "used, the tokens are invalid"
    );
    // Exactly held, or a tree
    assert!(matches!(
        text(StyleId::Width, "calc(100% / 3)"),
        Ok(Dimension::Compare(_))
    ));
    assert_eq!(
        text(StyleId::Width, "calc(100% / 3)")
            .unwrap()
            .at_basis(&env, 3.0),
        1.0
    );
    assert_eq!(
        text(StyleId::Width, "calc(100% / 4)"),
        Ok(Dimension::Percent(25.0))
    );
    // Within the limits as written, whatever the text the kernel writes
    let f = |inner: &str, n: usize| {
        (0..n).fold(inner.to_string(), |e, _| format!("clamp(none, {e}, none)"))
    };
    for value in [
        format!("calc(1px + 2 * {})", f("1%", 31)),
        format!("min({})", vec![f("1%", 8); 32].join(", ")),
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::Width, &StyleValue::Text(value.as_str().into()))
            .unwrap_or_else(|e| panic!("{value}: {e:?}"));
        let ops = [exact_kernel::Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }];
        let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
        assert!(exact_kernel::wire::frame::decode(&bytes).is_ok(), "{value}");
    }
}

/// An `env()` fallback is substituted as its tokens, into the text, where
/// the variable is undefined, and only then checked (CSS Environment
/// Variables 1 §3); a zero arithmetic made negative crosses the wire; deep
/// nesting is refused, never exhausting the stack.
#[test]
fn fallbacks_substitute_as_tokens_and_deep_text_is_refused() {
    exact_kernel::link_segments();
    let env = Env::new(0.0, 0.0, 0.0, 0.0);
    // One segment: the variable is undefined and its tokens stand in place
    let d = text(
        StyleId::Width,
        "calc(2 * env(viewport-segment-width 1 0, 10px + 5px))",
    )
    .unwrap();
    assert_eq!(d.at_basis(&env, 0.0), 25.0, "2 * 10px + 5px");
    assert_eq!(
        css(d),
        "calc(2 * env(viewport-segment-width 1 0, 10px + 5px))"
    );
    let d = text(StyleId::Width, "env(viewport-segment-width 1 0, 0)").unwrap();
    assert_eq!(d.resolve(&env), Dimension::Points(0.0));
    // An empty fallback is one; a `;` or `!` at its top is not
    let d = text(StyleId::Width, "calc(env(safe-area-inset-top,) + 10px)").unwrap();
    assert_eq!(d.at_basis(&env, 0.0), 10.0);
    for bad in [
        "calc(env(safe-area-inset-top, auto; junk) + 10px)",
        "calc(env(safe-area-inset-top, 1px !important) + 10px)",
    ] {
        assert!(text(StyleId::Width, bad).is_err(), "{bad}");
    }
    // Values that cross the wire as the kernel writes them
    for value in [
        "calc(10vw + 0px * -1)",
        "calc(env(safe-area-inset-top, calc(calc(calc(1px)))) + 1%)",
        "calc(env(safe-area-inset-top,) + 1%)",
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::Width, &StyleValue::Text(value.into()))
            .unwrap_or_else(|e| panic!("{value}: {e:?}"));
        let ops = [exact_kernel::Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }];
        let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
        assert!(exact_kernel::wire::frame::decode(&bytes).is_ok(), "{value}");
    }
    assert_eq!(
        css(text(StyleId::MarginTop, "calc(10vw + 0px * -1)").unwrap()),
        "calc((-1 * 0px) + 10vw)"
    );
    // Deep nesting is refused by the depth limit, before any deep work
    let deep = |open: &str, close: &str| {
        format!("calc({}1px{})", open.repeat(100_000), close.repeat(100_000))
    };
    for value in [
        deep("(", ")"),
        deep("calc(", ")"),
        format!("{}1px{}", "(".repeat(100_000), ")".repeat(100_000)),
    ] {
        assert!(text(StyleId::Width, &value).is_err());
    }
    let fallback = format!(
        "env(safe-area-inset-top, {}1px{})",
        "(".repeat(100_000),
        ")".repeat(100_000)
    );
    assert!(text(StyleId::Width, &fallback).is_err());
}

/// A value whose `env()` has a fallback is substituted before its row's
/// grammar reads it (CSS Environment Variables 1 §3): the tokens group as
/// written in their place, type-check only once there, and the result is
/// the row's own value (`auto` included) or invalid, the row's initial; a
/// fallback's inset is read where it is used. A NaN is the one NaN.
#[test]
fn substitution_comes_before_the_row_reads_the_value() {
    exact_kernel::link_segments();
    let env = Env::new(62.0, 0.0, 0.0, 0.0);
    let d = text(
        StyleId::Width,
        "calc(env(viewport-segment-width 1 0, 10px + 5px) * 2)",
    )
    .unwrap();
    assert_eq!(d.at_basis(&env, 0.0), 20.0, "10px + 5px * 2");
    assert_eq!(
        css(d),
        "calc(env(viewport-segment-width 1 0, 10px + 5px) * 2)"
    );
    let d = text(
        StyleId::Width,
        "calc(10px * env(viewport-segment-width 1 0, 2))",
    )
    .unwrap();
    assert_eq!(d.at_basis(&env, 0.0), 20.0);
    // `auto` substituted is the row's `auto`; an invalid result is its initial
    let row = |id: StyleId, value: &str| {
        let mut style = StyleProps::default();
        style
            .set_dynamic(id, &StyleValue::Text(value.into()))
            .unwrap();
        let resolved = style.env_resolved(&env).into_owned();
        match resolved.get(id) {
            exact_kernel::RowValue::Dimension(d) => d.resolve(&env),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(
        row(StyleId::MarginLeft, "env(viewport-segment-left 1 0, auto)"),
        Dimension::Auto
    );
    assert_eq!(
        row(StyleId::MarginLeft, "env(viewport-segment-left 1 0, 5)"),
        Dimension::Points(0.0)
    );
    assert_eq!(
        row(StyleId::Width, "env(viewport-segment-width 1 0, -10px)"),
        Dimension::Auto
    );
    assert_eq!(
        row(
            StyleId::Width,
            "env(viewport-segment-width 1 0, calc(-10px))"
        ),
        Dimension::Points(0.0)
    );
    // The inset a used fallback reads
    let Ok(Dimension::Compare(c)) = text(
        StyleId::PaddingTop,
        "env(viewport-segment-top 1 0, env(safe-area-inset-top))",
    ) else {
        panic!("a substitution");
    };
    assert!(c.reads(Edge::Top, &env));
    assert_eq!(Dimension::Compare(c).resolve(&env), Dimension::Points(62.0));
    // A NaN crosses the wire as the one NaN
    let mut style = StyleProps::default();
    style
        .set_dynamic(
            StyleId::Width,
            &StyleValue::Text("calc(10vw - NaN * 1px)".into()),
        )
        .unwrap();
    let ops = [exact_kernel::Op::SetStyle {
        id: 1,
        patch: Box::new(style),
    }];
    let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
    assert!(exact_kernel::wire::frame::decode(&bytes).is_ok());
    // A substitution crosses the wire for its row
    let mut style = StyleProps::default();
    style
        .set_dynamic(
            StyleId::MarginLeft,
            &StyleValue::Text("calc(env(viewport-segment-left 1 0, 10px + 5px) * 2)".into()),
        )
        .unwrap();
    let ops = [exact_kernel::Op::SetStyle {
        id: 1,
        patch: Box::new(style),
    }];
    let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
    assert!(exact_kernel::wire::frame::decode(&bytes).is_ok());
}

/// What substitution reads is tokens, and the row reads them as its own
/// value: two tokens written together stay two (CSS Syntax 3 §9), a comment
/// is none, and a fallback's length converts as the row's own does (`ch`,
/// `in`), `rem` and `em` refused by name. A text the wire's string carries
/// crosses it whole, for its row. A row CSS gives no percentage
/// (`column-width`) refuses one inside a math function too, and on the wire.
#[test]
fn a_substitution_is_tokens_the_row_reads_as_its_own() {
    use StyleId::{ColumnWidth, MarginLeft, PaddingLeft, Width};
    exact_kernel::link_segments();
    let env = Env::new(0.0, 0.0, 0.0, 0.0);
    let row = |id: StyleId, value: &str| {
        let mut style = StyleProps::default();
        style
            .set_dynamic(id, &StyleValue::Text(value.into()))
            .unwrap();
        let resolved = style.env_resolved(&env).into_owned();
        match resolved.get(id) {
            exact_kernel::RowValue::Dimension(d) => d.resolve(&env),
            other => panic!("{other:?}"),
        }
    };
    let segment = |fallback: &str| format!("env(viewport-segment-width 1 0, {fallback})");
    // `1` then `0px`, not `10px`: invalid, so the initial value
    let joined = format!("calc({}0px)", segment("1"));
    assert_eq!(row(MarginLeft, &joined), Dimension::Points(0.0));
    assert_eq!(
        row(MarginLeft, &format!("calc({} 0px)", segment("1"))),
        Dimension::Points(0.0)
    );
    assert_eq!(
        row(MarginLeft, &format!("calc({} + 0px)", segment("1px"))),
        Dimension::Points(1.0)
    );
    assert_eq!(row(MarginLeft, &segment("/* c */ auto")), Dimension::Auto);
    assert_eq!(row(PaddingLeft, &segment("2ch")), Dimension::Points(16.0));
    assert_eq!(text(PaddingLeft, "2ch").unwrap(), Dimension::Points(16.0));
    assert_eq!(row(PaddingLeft, &segment("1in")), Dimension::Points(96.0));
    // Padding takes no `auto`: substituted, it is invalid
    assert_eq!(row(PaddingLeft, &segment("auto")), Dimension::Points(0.0));
    for relative in ["2rem", "calc(1em + 1px)"] {
        assert!(matches!(
            text(PaddingLeft, &segment(relative)),
            Err(StyleValueError::BadComparison { reason, .. }) if reason.contains("rem and em")
        ));
    }
    // The wire carries the whole text, and the row the decoder reads it for
    let round_trip = |id: StyleId, value: &str| {
        let mut style = StyleProps::default();
        style
            .set_dynamic(id, &StyleValue::Text(value.into()))
            .unwrap();
        let ops = vec![Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }];
        let bytes = exact_kernel::wire::frame::encode(1, 0, &ops);
        assert_eq!(
            exact_kernel::wire::frame::decode(&bytes).map(|f| f.ops),
            Ok(ops)
        );
    };
    round_trip(
        Width,
        &format!("calc({}env(safe-area-inset-top, 0px))", " ".repeat(65_536)),
    );
    round_trip(PaddingLeft, &segment("4px"));
    // No percentage where CSS gives none, inside a math function too
    for value in ["50%", "min(50%, 100px)", "calc(10px + 0%)", "max(1vw, 2%)"] {
        assert!(text(ColumnWidth, value).is_err(), "{value}");
    }
    assert_eq!(row(ColumnWidth, &segment("10%")), Dimension::Auto);
    assert_eq!(
        text(ColumnWidth, "min(50px, 100px)").unwrap(),
        Dimension::Points(50.0)
    );
    let mut style = StyleProps::default();
    style.rare.column_width = Dimension::Percent(50.0);
    style.mask.set(ColumnWidth);
    let bytes = exact_kernel::wire::frame::encode(
        1,
        0,
        &[Op::SetStyle {
            id: 1,
            patch: Box::new(style),
        }],
    );
    assert_eq!(
        exact_kernel::wire::frame::decode(&bytes),
        Err(exact_kernel::error::DecodeError::PercentNotAdmitted { style: ColumnWidth })
    );
}

#[test]
fn segment_edges_stay_wide_until_the_whole_calculation_resolves() {
    exact_kernel::link_segments();
    for (origin, size) in [(16_777_216., 1.), (f32::MAX, f32::MAX)] {
        let env = Env {
            cols: 2,
            rows: 1,
            segments: vec![
                exact_kernel::Rect::new(0., 0., 1., 1.),
                exact_kernel::Rect::new(origin, origin, size, size),
            ],
            ..Env::default()
        };
        for (end, start) in [("right", "left"), ("bottom", "top")] {
            let value = format!(
                "calc(env(viewport-segment-{end} 1 0) - env(viewport-segment-{start} 1 0))"
            );
            let d = text(StyleId::MarginLeft, &value).unwrap();
            assert_eq!(
                d.at_basis(&env, 0.),
                size,
                "{value}; origin={origin}, size={size}"
            );
        }
    }
}
