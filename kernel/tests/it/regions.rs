//! The layout regions (LLP 1123 D10): `env(-exact-<region>-inset-<edge>)`
//! resolves against the insets the host reports for each region, and against
//! the safe area on a host that reports none; a change lays out again only
//! the nodes that read the environment.

use exact_kernel::style::env::{self, EnvRefusal, ENV_NAMES};
use exact_kernel::{
    Dimension, Edge, Env, Kernel, KernelError, LayoutError, NodeType, Offer, Op, Region, StyleId,
    StyleProps, StyleValue,
};

/// The regions in wire order as a host reports them: each region's
/// insets, top right bottom left, the value `10 × region + edge + 1`
/// so every one is distinct.
fn reported() -> [[f32; 4]; 5] {
    std::array::from_fn(|r| std::array::from_fn(|e| (10 * r + e + 1) as f32))
}

/// A root whose four paddings read four regions, and a child that fills
/// its content box.
fn tree() -> Kernel {
    let mut kernel = Kernel::with_monospace();
    let mut root = StyleProps::default();
    for (id, text) in [
        (
            StyleId::PaddingLeft,
            "env(-exact-layout-margins-inset-left)",
        ),
        (
            StyleId::PaddingTop,
            "env(-exact-safe-area-corners-inset-top)",
        ),
        (
            StyleId::PaddingRight,
            "calc(env(-exact-readable-content-inset-right) + 2px)",
        ),
        (
            StyleId::PaddingBottom,
            "max(16px, env(-exact-layout-margins-corners-inset-bottom))",
        ),
    ] {
        root.set_dynamic(id, &StyleValue::Text(text.into()))
            .unwrap();
    }
    root.set_dynamic(StyleId::BoxSizing, &StyleValue::Text("border-box".into()))
        .unwrap();
    root.set_dynamic(StyleId::Height, &StyleValue::Percent(100.0))
        .unwrap();
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
            patch: Box::new(root),
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
        .compute_layout(1, Offer::definite(951.0, 669.0))
        .unwrap();
    let f = kernel.node(2).unwrap().frame;
    (f.x, f.y, f.width, f.height)
}

#[test]
fn every_region_and_edge_is_a_length_by_its_name() {
    let edges = ["top", "right", "bottom", "left"];
    for region in Region::ALL {
        for (e, edge) in edges.iter().enumerate() {
            let text = format!("env({}{edge})", region.prefix());
            let want = Dimension::Region(region, Edge::from_index(e as u8).unwrap(), 0.0);
            assert_eq!(env::parse(&text), Ok(Some(want)), "{text}");
            assert_eq!(
                Dimension::parse_env(&format!(" env( {}{edge} ) ", region.prefix())),
                Some(want)
            );
            assert_eq!(
                Region::from_name(&format!("{}{edge}", region.prefix())),
                Some((region, want_edge(e)))
            );
        }
    }
    // Every family is a spelling Exact owns (LLP 1081 D8)
    for region in Region::ALL {
        assert!(ENV_NAMES
            .iter()
            .any(|(n, kind)| *n == region.prefix() && kind.starts_with("exact ")));
    }
}

fn want_edge(e: usize) -> Edge {
    Edge::from_index(e as u8).unwrap()
}

#[test]
fn a_misspelled_region_is_refused_by_name() {
    for text in [
        "env(-exact-layout-margins-inset-middle)",
        "env(-exact-layout-margins-inset-left 0)",
        "env(-exact-layout-margin-inset-left)",
        "env(-exact-safe-area-inset-left)",
    ] {
        assert_eq!(env::parse(text), Err(EnvRefusal::UnknownVariable), "{text}");
    }
}

#[test]
fn a_region_resolves_against_the_safe_area_until_the_host_reports_it() {
    let mut kernel = tree();
    // No host regions: each region is the safe area (0 until set)
    assert_eq!(child(&mut kernel), (0.0, 0.0, 951.0 - 2.0, 669.0 - 16.0));
    assert!(kernel.set_env(Env::new(5.0, 6.0, 30.0, 8.0)).unwrap());
    assert_eq!(
        child(&mut kernel),
        (8.0, 5.0, 951.0 - 8.0 - 8.0, 669.0 - 5.0 - 30.0)
    );
    // The host's regions: margins left 14, safe-area corners top 1, readable
    // right 32 (+ 2), margins corners bottom 23 (under max(16px, …))
    let regions = reported();
    assert!(kernel
        .set_env(kernel.env().with_regions(Some(regions)))
        .unwrap());
    assert_eq!(kernel.env().regions, Some(regions));
    assert_eq!(
        child(&mut kernel),
        (14.0, 1.0, 951.0 - 14.0 - 34.0, 669.0 - 1.0 - 23.0)
    );
    // Unchanged: nothing to lay out; none again: the safe area
    assert!(!kernel.set_env(kernel.env()).unwrap());
    assert!(kernel.set_env(kernel.env().with_regions(None)).unwrap());
    assert_eq!(
        child(&mut kernel),
        (8.0, 5.0, 951.0 - 8.0 - 8.0, 669.0 - 5.0 - 30.0)
    );
}

#[test]
fn a_region_reads_its_own_entry() {
    let mut env = Env::new(1.0, 2.0, 3.0, 4.0);
    for region in Region::ALL {
        for edge in Edge::ALL {
            assert_eq!(
                env.region_inset(region, edge),
                env.inset(edge),
                "the safe area without regions"
            );
        }
    }
    env = env.with_regions(Some(reported()));
    for (r, region) in Region::ALL.iter().enumerate() {
        for (e, edge) in Edge::ALL.iter().enumerate() {
            assert_eq!(env.region_inset(*region, *edge), (10 * r + e + 1) as f32);
            assert_eq!(
                Dimension::Region(*region, *edge, 0.5).resolve(&env),
                Dimension::Points((10 * r + e + 1) as f32 + 0.5)
            );
        }
    }
}

#[test]
fn a_non_finite_region_is_refused() {
    let mut kernel = tree();
    let mut regions = reported();
    regions[3][2] = f32::INFINITY;
    assert_eq!(
        kernel.set_env(Env::default().with_regions(Some(regions))),
        Err(KernelError::from(LayoutError::InvalidEnv))
    );
    assert_eq!(kernel.env().regions, None);
}

#[test]
fn region_lengths_travel_the_wire() {
    use exact_kernel::wire;
    let mut style = StyleProps::default();
    style
        .set_dynamic(
            StyleId::MarginLeft,
            &StyleValue::Text("calc(env(-exact-readable-content-corners-inset-left) - 2px)".into()),
        )
        .unwrap();
    style
        .set_dynamic(
            StyleId::MarginTop,
            &StyleValue::Text("min(env(-exact-layout-margins-inset-top), 4vh)".into()),
        )
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
    other.apply_frame(&wire::encode(0, 2, &ops)).unwrap();
    let node = other.node(7).unwrap();
    assert_eq!(
        node.style.margin_left,
        Dimension::Region(Region::ReadableContentCorners, Edge::Left, -2.0)
    );
    let env = Env::default().with_regions(Some(reported()));
    // The calculation tree keeps its region term: the margins' top is 11,
    // under 4vh of a 1000-high viewport
    let Dimension::Compare(c) = node.style.margin_top else {
        panic!("a calculation: {:?}", node.style.margin_top)
    };
    let mut text = String::new();
    c.css(&mut text);
    assert_eq!(text, "min(env(-exact-layout-margins-inset-top), 4vh)");
    let tall = Env {
        viewport_height: 1000.0,
        ..env
    };
    assert_eq!(
        Dimension::Compare(c).resolve(&tall),
        Dimension::Points(11.0)
    );
}

/// A browser gets the safe area's name for each region, an escaped name
/// included, and the rest of the text as written, a fallback too.
#[test]
fn a_browser_reads_each_region_as_its_safe_area() {
    for (text, want) in [
        (
            "max(16px, env(-exact-layout-margins-inset-left))",
            "max(16px, env(safe-area-inset-left))",
        ),
        (
            r"max(16px, env(-exact-\6c ayout-margins-inset-left, 8px))",
            "max(16px, env(safe-area-inset-left, 8px))",
        ),
        (
            "calc(env(-exact-readable-content-corners-inset-right, 30px) + 5px)",
            "calc(env(safe-area-inset-right, 30px) + 5px)",
        ),
        (
            "min(10vw, env(safe-area-inset-top))",
            "min(10vw, env(safe-area-inset-top))",
        ),
    ] {
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::PaddingLeft, &StyleValue::Text(text.into()))
            .unwrap();
        let Dimension::Compare(c) = style.padding_left else {
            panic!("a calculation: {text}: {:?}", style.padding_left);
        };
        let mut out = String::new();
        env::browser_css(c, &mut out);
        assert_eq!(out, want, "{text}");
    }
}
