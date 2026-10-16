//! `-exact-hairline` over a display scale the host sets (LLP 1115.000 D10.3):
//! one device pixel, 1 px over the scale, resolved at the end of each commit
//! as `rem` is, so every reader sees pixels and layout follows the display.
//!
//! The expected boxes are Chrome's at each device scale factor (154.0.8037.98,
//! macOS, 2026-10-11, `--force-device-scale-factor`): a `div` 20 px tall with
//! a `var(--exact-hairline)` border on four sides is 22, 21 and 20.67 px tall
//! at 1x, 2x and 3x, and an `hr` with that top border 1, 0.5 and 0.33.

use exact_kernel::style::relative::{named_length, Unit};
use exact_kernel::{
    Kernel, KernelError, LayoutError, NodeType, Offer, Op, StyleId, StyleProps, StyleValue,
};

fn style(rows: &[(StyleId, &str)]) -> Box<StyleProps> {
    let mut s = StyleProps::default();
    for (id, text) in rows {
        let value = match text.parse::<f64>() {
            Ok(n) => StyleValue::Number(n),
            Err(_) => StyleValue::Text((*text).into()),
        };
        s.set_dynamic(*id, &value).unwrap();
    }
    Box::new(s)
}

/// 1: a 200 px block. 2: a box 20 px tall with a hairline border on four
/// sides. 3: a rule, a hairline top edge and no height. 4: a box whose
/// height is a hairline. 5: a box with an authored 0.5 px edge.
fn tree() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let solid = |side: StyleId| (side, "solid");
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: style(&[(StyleId::Width, "200")]),
        },
    ];
    for id in 2..=5 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::View,
        });
    }
    ops.extend([
        Op::SetStyle {
            id: 2,
            patch: style(&[
                (StyleId::Height, "20"),
                (StyleId::BorderWidthTop, "-exact-hairline"),
                (StyleId::BorderWidthRight, "-exact-hairline"),
                (StyleId::BorderWidthBottom, "-exact-hairline"),
                (StyleId::BorderWidthLeft, "-exact-hairline"),
                solid(StyleId::BorderStyleTop),
                solid(StyleId::BorderStyleRight),
                solid(StyleId::BorderStyleBottom),
                solid(StyleId::BorderStyleLeft),
            ]),
        },
        Op::SetStyle {
            id: 3,
            patch: style(&[
                (StyleId::BorderWidthTop, " -Exact-Hairline "),
                solid(StyleId::BorderStyleTop),
            ]),
        },
        Op::SetStyle {
            id: 4,
            patch: style(&[(StyleId::Height, "-exact-hairline")]),
        },
        Op::SetStyle {
            id: 5,
            patch: style(&[
                (StyleId::BorderWidthTop, "0.5"),
                solid(StyleId::BorderStyleTop),
            ]),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 3, 4, 5],
        },
        Op::AttachRoot { id: 1 },
    ]);
    kernel.apply(0, 1, &ops).unwrap();
    kernel
}

fn heights(kernel: &mut Kernel) -> [f32; 4] {
    kernel
        .compute_layout(1, Offer::definite(400.0, 400.0))
        .unwrap();
    [2, 3, 4, 5].map(|id| kernel.node(id).unwrap().frame.height)
}

#[test]
fn a_hairline_is_one_px_until_a_host_says_its_displays_scale() {
    let mut kernel = tree();
    assert_eq!(kernel.display_scale(), 1.0);
    assert!(kernel.has_hairlines());
    assert_eq!(heights(&mut kernel), [22.0, 1.0, 1.0, 0.5]);
    let rule = kernel.node(3).unwrap().style;
    assert_eq!(rule.border_widths(), [1.0, 0.0, 0.0, 0.0], "pixels");
    assert_eq!(
        rule.relative.get(StyleId::BorderWidthTop),
        Some((Unit::Hairline, 1.0)),
        "and what was written"
    );
}

#[test]
fn a_new_scale_moves_every_hairline_with_the_next_commit_and_nothing_else() {
    let mut kernel = tree();
    heights(&mut kernel);
    assert_eq!(kernel.set_display_scale(2.0), Ok(true));
    assert_eq!(kernel.set_display_scale(2.0), Ok(false), "the same scale");
    assert_eq!(kernel.display_scale(), 2.0);
    let receipt = kernel.apply(0, 2, &[]).unwrap();
    assert!(receipt.layout_invalidated);
    let touched = |kernel: &Kernel, id: u32| {
        let key = kernel.node(id).unwrap().key;
        receipt.touched.contains(&key)
    };
    assert!(touched(&kernel, 2) && touched(&kernel, 3) && touched(&kernel, 4));
    assert!(
        !touched(&kernel, 5),
        "an authored 0.5 is 0.5 on any display"
    );
    assert_eq!(heights(&mut kernel), [21.0, 0.5, 0.5, 0.5]);
    kernel.set_display_scale(3.0).unwrap();
    kernel.apply(0, 3, &[]).unwrap();
    let at3 = heights(&mut kernel);
    let third = 1.0 / 3.0;
    for (got, want) in at3.iter().zip([20.0 + 2.0 * third, third, third, 0.5]) {
        assert!((got - want).abs() < 1e-4, "{at3:?}");
    }
    // A hairline written after the scale was said resolves against it.
    let late = Op::SetStyle {
        id: 5,
        patch: style(&[(StyleId::BorderWidthTop, "-exact-hairline")]),
    };
    kernel.apply(0, 4, &[late]).unwrap();
    assert!((heights(&mut kernel)[3] - third).abs() < 1e-4);
}

#[test]
fn an_unusable_scale_is_refused_and_the_scale_is_kept_across_a_reset() {
    let mut kernel = tree();
    for bad in [0.0, -2.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            kernel.set_display_scale(bad),
            Err(KernelError::from(LayoutError::InvalidDisplayScale))
        );
    }
    kernel.set_display_scale(2.0).unwrap();
    kernel.apply(0, 2, &[]).unwrap();
    kernel.reset();
    assert_eq!(kernel.display_scale(), 2.0, "the host's, not the tree's");
    assert!(!kernel.has_hairlines());
}

#[test]
fn a_hairline_is_a_width_and_every_named_length_parses() {
    for name in exact_kernel::NAMED_LENGTHS {
        assert_eq!(
            named_length(&format!("-exact-{name}")),
            Some(Unit::Hairline),
            "{name}: the schema names it and the kernel resolves it"
        );
    }
    assert_eq!(named_length("hairline"), None, "written `-exact-hairline`");
    assert_eq!(named_length("-exact-hair"), None);
    let mut s = StyleProps::default();
    for row in [StyleId::FontSize, StyleId::LineHeight] {
        assert!(
            s.set_dynamic(row, &StyleValue::Text("-exact-hairline".into()))
                .is_err(),
            "{row:?}"
        );
    }
    for row in [
        StyleId::Width,
        StyleId::MarginTop,
        StyleId::RowGap,
        StyleId::BorderWidthLeft,
        StyleId::BorderRadiusTopLeft,
    ] {
        s.set_dynamic(row, &StyleValue::Text("-exact-hairline".into()))
            .unwrap_or_else(|e| panic!("{row:?}: {e:?}"));
    }
    assert!(
        s.set_dynamic(
            StyleId::Opacity,
            &StyleValue::Text("-exact-hairline".into())
        )
        .is_err(),
        "no length row"
    );
}
