//! The title-bar area (Window Controls Overlay): `env(titlebar-area-*)` is
//! undefined, so its fallback applies, until the host reports an area; all
//! zero while the area is hidden; a change lays out again.

use exact_kernel::style::env::{self, ENV_NAMES};
use exact_kernel::{
    Dimension, Env, EnvRefusal, Kernel, NodeType, Offer, Op, Rect, StyleId, StyleProps, StyleValue,
    TitleBarVar,
};

/// A bar whose rows read the area: its left padding the area's x (no
/// fallback, so 0, the row's initial value, while undefined), its width the
/// area's (no fallback: `auto`), its height the area's or 40px, and a child
/// that fills its content box.
fn tree() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut bar = StyleProps::default();
    for (id, text) in [
        (StyleId::PaddingLeft, "env(titlebar-area-x)"),
        (StyleId::MarginTop, "calc(env(titlebar-area-y) + 1px)"),
        (StyleId::Width, "env(titlebar-area-width)"),
        (StyleId::Height, "env(titlebar-area-height, 40px)"),
    ] {
        bar.set_dynamic(id, &StyleValue::Text(text.into())).unwrap();
    }
    let mut fill = StyleProps::default();
    fill.set_dynamic(StyleId::Height, &StyleValue::Percent(100.0))
        .unwrap();
    let ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(bar),
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(fill),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::AttachRoot { id: 1 },
    ];
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

/// The child's frame: x, y, width, height.
fn child(kernel: &mut Kernel) -> (f32, f32, f32, f32) {
    kernel
        .compute_layout(1, Offer::definite(900.0, 600.0))
        .unwrap();
    let f = kernel.node(2).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn each_variable_is_a_calculation_by_its_name() {
    for var in TitleBarVar::ALL {
        let name = format!("titlebar-area-{}", var.name());
        assert_eq!(TitleBarVar::from_name(&name), Some(var));
        let Ok(Some(Dimension::Compare(c))) = env::parse(&format!(" env( {name} ) ")) else {
            panic!("{name}: a calculation");
        };
        let mut text = String::new();
        c.css(&mut text);
        assert_eq!(text, format!("env({name})"));
        // A browser resolves it itself, a fallback as written
        let mut style = StyleProps::default();
        let with = format!("calc(env({name}, 12px) + 3px)");
        style
            .set_dynamic(StyleId::PaddingLeft, &StyleValue::Text(with.clone().into()))
            .unwrap();
        let Dimension::Compare(c) = style.padding_left else {
            panic!("{with}: a calculation");
        };
        let mut out = String::new();
        env::browser_css(c, &mut out);
        assert_eq!(out, with);
    }
    for text in [
        "env(titlebar-area-z)",
        "env(titlebar-area-x 0)",
        "env(titlebar-area)",
        "env(-exact-titlebar-area-x)",
    ] {
        assert_eq!(env::parse(text), Err(EnvRefusal::UnknownVariable), "{text}");
    }
    assert!(ENV_NAMES
        .iter()
        .any(|(n, kind)| *n == TitleBarVar::PREFIX && kind.starts_with("css ")));
}

#[test]
fn the_area_is_undefined_until_the_host_reports_it_and_zero_while_hidden() {
    let mut kernel = tree();
    // Undefined: padding 0 and width auto (initial values), height 40px
    // (the fallback), margin 0 (the calculation reads an undefined term)
    assert_eq!(kernel.env().titlebar_area, None);
    assert_eq!(child(&mut kernel), (0.0, 0.0, 900.0, 40.0));
    // AppKit's area beside the traffic lights in a 900-point window
    let area = Rect::new(78.0, 0.0, 822.0, 32.0);
    assert!(kernel
        .set_env(kernel.env().with_titlebar_area(Some(area)))
        .unwrap());
    assert_eq!(child(&mut kernel), (78.0, 1.0, 822.0, 32.0));
    assert!(!kernel.set_env(kernel.env()).unwrap(), "unchanged");
    // Insets set later keep the area
    assert!(kernel
        .set_env(kernel.env().with_insets(32.0, 0.0, 0.0, 0.0))
        .unwrap());
    assert_eq!(kernel.env().titlebar_area, Some(area));
    // Hidden (full screen, a toolbar): every variable is zero
    let hidden = Rect::default();
    assert!(kernel
        .set_env(kernel.env().with_titlebar_area(Some(hidden)))
        .unwrap());
    assert_eq!(child(&mut kernel), (0.0, 1.0, 0.0, 0.0));
    // Gone again: the fallbacks
    assert!(kernel
        .set_env(kernel.env().with_titlebar_area(None))
        .unwrap());
    assert_eq!(child(&mut kernel), (0.0, 0.0, 900.0, 40.0));
    // A non-finite area is refused
    assert!(kernel
        .set_env(Env::default().with_titlebar_area(Some(Rect::new(f32::NAN, 0.0, 1.0, 1.0))))
        .is_err());
}

#[test]
fn titlebar_lengths_travel_the_wire() {
    use exact_kernel::wire;
    // Each as written, and as CSS Values 4 §10.13 serializes it
    let texts = [
        (
            StyleId::PaddingLeft,
            "env(titlebar-area-x)",
            "env(titlebar-area-x)",
        ),
        (
            StyleId::MarginLeft,
            "calc(env(titlebar-area-x) - 2px)",
            "calc(-2px + env(titlebar-area-x))",
        ),
        (
            StyleId::Width,
            "min(env(titlebar-area-width), 50%)",
            "min(env(titlebar-area-width), 50%)",
        ),
        (
            StyleId::Height,
            "env(titlebar-area-height, 40px)",
            "env(titlebar-area-height, 40px)",
        ),
    ];
    let mut style = StyleProps::default();
    for (id, text, _) in texts {
        style
            .set_dynamic(id, &StyleValue::Text(text.into()))
            .unwrap();
    }
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
    other.apply_frame(&wire::encode(0, 2, &ops)).unwrap();
    let node = other.node(7).unwrap();
    let rows = [
        node.style.padding_left,
        node.style.margin_left,
        node.style.width,
        node.style.height,
    ];
    let area = Env::default().with_titlebar_area(Some(Rect::new(78.0, 0.0, 822.0, 32.0)));
    for ((_, text, css), (row, want)) in
        texts.iter().zip(rows.iter().zip([78.0, 76.0, 822.0, 32.0]))
    {
        let Dimension::Compare(c) = row else {
            panic!("{text}: a calculation, {row:?}");
        };
        let mut out = String::new();
        c.css(&mut out);
        assert_eq!(out, *css);
        let resolved = match row.resolve(&area) {
            Dimension::Points(v) => v,
            // A percentage stays for layout: min(822, 50% of 2000)
            Dimension::Compare(c) => c.value(&area, Some(2000.0)).unwrap(),
            other => panic!("{text}: {other:?}"),
        };
        assert_eq!(resolved, want, "{text}");
    }
}
