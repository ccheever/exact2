use super::*;
use taffy::prelude::{line, span};

#[test]
fn percent_converts_exactly_once() {
    let d = Dimension::Percent(50.0);
    let env = Env::default();
    assert_eq!(d.to_taffy(&env), percent(0.5_f32));
    assert_eq!(d.to_lpa(&env), percent(0.5_f32));
    assert_eq!(d.to_lp(&env), percent(0.5_f32));
}

#[test]
fn env_lengths_parse_by_the_css_grammar_and_resolve_against_the_environment() {
    assert_eq!(
        Dimension::parse_env("env(safe-area-inset-top)"),
        Some(Dimension::Env(Edge::Top, 0.0))
    );
    assert_eq!(
        Dimension::parse_env(" env( safe-area-inset-left ) "),
        Some(Dimension::Env(Edge::Left, 0.0))
    );
    assert_eq!(
        Dimension::parse_env("calc(env(safe-area-inset-bottom) + 12px)"),
        Some(Dimension::Env(Edge::Bottom, 12.0))
    );
    assert_eq!(
        Dimension::parse_env("calc(env(safe-area-inset-right)-2.5px)"),
        Some(Dimension::Env(Edge::Right, -2.5))
    );
    // Addition commutes: the length may come first (CSS's grammar).
    assert_eq!(
        Dimension::parse_env("calc(300px + env(safe-area-inset-bottom))"),
        Some(Dimension::Env(Edge::Bottom, 300.0))
    );
    assert_eq!(
        Dimension::parse_env("calc( 2.5px  +\tenv(safe-area-inset-left) )"),
        Some(Dimension::Env(Edge::Left, 2.5))
    );
    for bad in [
        "env(safe-area-inset-middle)",
        "env(keyboard-inset-height)",
        "calc(env(safe-area-inset-top) + 12)",
        "calc(env(safe-area-inset-top) * 2)",
        "calc(12px - env(safe-area-inset-top))",
        "calc(12px+env(safe-area-inset-top))",
        "calc(12px +env(safe-area-inset-top))",
        "calc(12 px + env(safe-area-inset-top))",
        "calc(12px + env(safe-area-inset-top) + 1px)",
        "calc(12 + env(safe-area-inset-top))",
        "env(safe-area-inset-top, 0px)",
        "12px",
        "auto",
    ] {
        assert_eq!(Dimension::parse_env(bad), None, "{bad}");
    }
    let env = Env::new(62.0, 0.0, 34.0, 0.0);
    assert_eq!(Dimension::Env(Edge::Top, 0.0).to_lp(&env), length(62.0_f32));
    assert_eq!(
        Dimension::Env(Edge::Bottom, 12.0).to_lpa(&env),
        length(46.0_f32)
    );
    assert_eq!(
        Dimension::Env(Edge::Left, 8.0).to_taffy(&env),
        length(8.0_f32)
    );
    // Environment and explicit pixel lengths share dimension decoding.
    let mut s = StyleProps::default();
    s.set_dynamic(
        StyleId::PaddingTop,
        &StyleValue::Text("env(safe-area-inset-top)".into()),
    )
    .unwrap();
    assert_eq!(s.padding_top, Dimension::Env(Edge::Top, 0.0));
    assert!(uses_env(&s));
    s.set_dynamic(StyleId::PaddingTop, &StyleValue::Text("12px".into()))
        .unwrap();
    assert_eq!(s.padding_top, Dimension::Points(12.0));
    assert!(!uses_env(&s));
    assert!(!uses_env(&StyleProps::default()));
}

#[test]
fn defaults_are_the_css_defaults() {
    let s = StyleProps::default().to_taffy(NodeType::View, &Env::default());
    assert_eq!(s.display, taffy::style::Display::Block);
    assert_eq!(s.box_sizing, taffy::style::BoxSizing::ContentBox);
    assert_eq!(s.flex_direction, taffy::style::FlexDirection::Row);
    assert_eq!(s.flex_shrink, 1.0);
    // `normal`, which each layout mode resolves (flex: stretch).
    assert_eq!(s.align_items, None);
    assert_eq!(s.justify_content, None);
    assert_eq!(s.align_content, None);
    assert_eq!(s.justify_items, None);
    assert_eq!(s.position, taffy::style::Position::Static);
    assert_eq!(s.overflow.y, taffy::style::Overflow::Visible);
}

#[test]
fn scroll_containers_scroll_on_the_block_axis_by_default() {
    let s = StyleProps::default().to_taffy(NodeType::ScrollView, &Env::default());
    assert_eq!(s.overflow.y, taffy::style::Overflow::Scroll);
    // CSS Overflow §3: a `visible` axis beside a non-visible one computes
    // to `auto` — `scroll` here — so a scroll container clips both axes.
    assert_eq!(s.overflow.x, taffy::style::Overflow::Scroll);
    let plain = StyleProps::default().to_taffy(NodeType::View, &Env::default());
    assert_eq!(plain.overflow.x, taffy::style::Overflow::Visible);
    // Symmetric: a hidden x makes an unset y scrollable, not hidden.
    let mut hidden_x = StyleProps::default();
    hidden_x.overflow_x = Overflow::Hidden;
    hidden_x.mask.set(StyleId::OverflowX);
    let t = hidden_x.to_taffy(NodeType::View, &Env::default());
    assert_eq!(
        (t.overflow.x, t.overflow.y),
        (
            taffy::style::Overflow::Hidden,
            taffy::style::Overflow::Scroll
        )
    );
    let mut explicit = StyleProps::default();
    explicit.overflow_y = Overflow::Hidden;
    explicit.mask.set(StyleId::OverflowY);
    assert_eq!(
        explicit
            .to_taffy(NodeType::ScrollView, &Env::default())
            .overflow
            .y,
        taffy::style::Overflow::Hidden
    );
}

#[test]
fn a_colour_parses_as_hex_or_as_css_rgb_notation() {
    let red = Some(Color::rgba(255, 0, 0, 255));
    assert_eq!(Color::parse(" #f00 "), red);
    assert_eq!(Color::parse("rgb(255, 0, 0)"), red);
    assert_eq!(Color::parse("rgba(255,0,0)"), red);
    assert_eq!(Color::parse("rgb(100%, 0%, 0%)"), red);
    assert_eq!(Color::parse("rgb(255 0 0)"), red);
    let clear = Some(Color::rgba(0, 0, 0, 0));
    assert_eq!(Color::parse("transparent"), clear);
    assert_eq!(Color::parse(" Transparent "), clear);
    assert_eq!(Color::parse("transparentt"), None);
    let half = Some(Color::rgba(255, 0, 0, 128));
    assert_eq!(Color::parse("rgba(255, 0, 0, 0.5)"), half);
    assert_eq!(Color::parse("rgba(255, 0, 0, 50%)"), half);
    assert_eq!(Color::parse("rgb(255 0 0 / 0.5)"), half);
    assert_eq!(Color::parse("rgb(255 0 0 / 50%)"), half);
    assert_eq!(Color::parse("rgb( 100% 0 0 / 50% )"), half);
    // Out-of-range values clamp, as on the web; fractions round.
    assert_eq!(
        Color::parse("rgb(300, -1, 127.5, 2)"),
        Some(Color::rgba(255, 0, 128, 255))
    );
    for text in [
        "rgb(255, 0)",
        "rgb(255, 0, 0, 1, 1)",
        "rgb(255 0 0 /)",
        "rgb(255, 0, 0 / 1)",
        "rgb(255, 0, 0,)",
        "rgb(a, b, c)",
        "rgb(nan, 0, 0)",
        "rgb(255, 0, 0",
        "hsl(0, 100%, 50%)",
        "red",
    ] {
        assert_eq!(Color::parse(text), None, "{text}");
    }
}

#[test]
fn a_colour_row_holds_a_light_dark_pair_and_the_host_resolves_it() {
    // CSS's spelling, and only it (LLP 1034 D1).
    let pair = ColorValue::parse_light_dark("light-dark(#ffffff, #000000)").unwrap();
    assert_eq!(
        pair,
        ColorValue::LightDark(
            Color::parse_hex("#ffffff").unwrap(),
            Color::parse_hex("#000000").unwrap()
        )
    );
    assert_eq!(pair.resolve(false), Color::parse_hex("#ffffff").unwrap());
    assert_eq!(pair.resolve(true), Color::parse_hex("#000000").unwrap());
    assert!(pair.is_scheme_aware());
    // Whitespace is free.
    assert_eq!(
        ColorValue::parse_light_dark("  light-dark( #fff , #000 )  "),
        Some(ColorValue::LightDark(
            Color::parse_hex("#fff").unwrap(),
            Color::parse_hex("#000").unwrap()
        ))
    );
    // Either colour may be `rgb()`; its commas are its own.
    assert_eq!(
        ColorValue::parse_light_dark("light-dark(rgb(255, 0, 0), rgba(0 0 255 / 50%))"),
        Some(ColorValue::LightDark(
            Color::rgba(255, 0, 0, 255),
            Color::rgba(0, 0, 255, 128)
        ))
    );
    // Anything that is not two colours is not this function.
    for text in [
        "#ffffff",
        "light-dark(#fff)",
        "light-dark(#fff, nope)",
        "dark-light(#fff, #000)",
        "light-dark(#fff, #000",
        "light-dark(rgb(1, 2, 3, 4, 5), #000)",
    ] {
        assert_eq!(ColorValue::parse_light_dark(text), None, "{text}");
    }
    // A fixed colour resolves to itself under either appearance.
    let one = ColorValue::Fixed(Color::parse_hex("#abcdef").unwrap());
    assert_eq!(one.resolve(false), one.resolve(true));
    assert!(!one.is_scheme_aware());
}

#[test]
fn a_colour_row_takes_a_pair_dynamically_as_a_dimension_takes_env() {
    let mut s = StyleProps::default();
    s.set_dynamic(
        StyleId::BackgroundColor,
        &StyleValue::Text("light-dark(#ffffff, #17181b)".into()),
    )
    .expect("a colour row takes CSS's own function");
    assert_eq!(
        s.background_color,
        ColorValue::LightDark(
            Color::parse_hex("#ffffff").unwrap(),
            Color::parse_hex("#17181b").unwrap()
        )
    );
    // And still takes a plain colour, which is the common case.
    s.set_dynamic(StyleId::TextColor, &StyleValue::Text("#112233".into()))
        .expect("a hex is still a colour");
    assert_eq!(
        s.text_color,
        ColorValue::Fixed(Color::parse_hex("#112233").unwrap())
    );
    // A text that is neither is refused, not silently taken.
    assert!(s
        .set_dynamic(
            StyleId::TextColor,
            &StyleValue::Text("light-dark(#fff)".into())
        )
        .is_err());
}

#[test]
fn color_channels() {
    let c = Color::rgba(0x12, 0x34, 0x56, 0x78);
    assert_eq!(c.0, 0x1234_5678);
    assert_eq!((c.r(), c.g(), c.b(), c.a()), (0x12, 0x34, 0x56, 0x78));
}

#[test]
fn grid_tracks_lower_to_engine_tracks() {
    let mut p = StyleProps::default();
    p.display = Display::Grid;
    p.grid_template_columns = GridTracks::from_tracks(vec![
        GridTrack::Fr(1.0),
        GridTrack::Points(40.0),
        GridTrack::Auto,
    ]);
    p.grid_row = GridPlacement::from_lines(GridLine::Line(1), GridLine::Span(2));
    let s = p.to_taffy(NodeType::View, &Env::default());
    assert_eq!(s.grid_template_columns.len(), 3);
    assert_eq!(s.grid_row.start, line(1));
    assert_eq!(s.grid_row.end, span(2));
}

/// Content regions check padding and border without the engine's style;
/// the answer is the one the engine's style gives.
#[test]
fn unpadded_reads_what_the_engine_style_would() {
    let env = Env::new(62.0, 0.0, 0.0, 0.0);
    let zero = |x: taffy::style::LengthPercentage| x == length(0.) || x == percent(0.);
    let engine = |p: &StyleProps| {
        let s = p.to_taffy(NodeType::View, &env);
        [s.padding, s.border]
            .iter()
            .all(|r| [r.left, r.right, r.top, r.bottom].into_iter().all(zero))
    };
    for d in [
        Dimension::Auto,
        Dimension::Points(0.0),
        Dimension::Points(-0.0),
        Dimension::Points(1.0),
        Dimension::Percent(0.0),
        Dimension::Percent(-0.0),
        Dimension::Percent(1e-45),
        Dimension::Percent(3.0),
        Dimension::Env(Edge::Right, 0.0),
        Dimension::Env(Edge::Right, -0.0),
        Dimension::Env(Edge::Top, 0.0),
        Dimension::Env(Edge::Top, -62.0),
    ] {
        let mut p = StyleProps::default();
        p.padding_bottom = d;
        assert_eq!(p.unpadded(&env), engine(&p), "{d:?}");
    }
    for (style, width) in [
        (BorderStyle::Solid, 0.0),
        (BorderStyle::Solid, -0.0),
        (BorderStyle::Solid, -2.0),
        (BorderStyle::Solid, 1.0),
        (BorderStyle::None, 3.0),
    ] {
        let mut p = StyleProps::default();
        p.border_style_left = style;
        p.border_width_left = width;
        assert_eq!(p.unpadded(&env), engine(&p), "{style:?} {width}");
    }
}

#[test]
fn calc_lengths_parse_one_percent_and_one_pixel_term_and_resolve_by_basis() {
    for (text, percent, points) in [
        ("calc(100% - 89px)", 100.0, -89.0),
        ("calc(50% + 12.5px)", 50.0, 12.5),
        ("calc(-10% + 4px)", -10.0, 4.0),
        ("calc(89px - 100%)", -100.0, 89.0),
        ("calc(4PX + 25%)", 25.0, 4.0),
        (" calc(\t1e2%\n-\t2E1px ) ", 100.0, -20.0),
    ] {
        assert_eq!(
            Dimension::parse_calc(text),
            Some(Dimension::Calc(percent, points)),
            "{text}"
        );
    }
    for bad in [
        "calc(100%-89px)",
        "calc(100% -89px)",
        "calc(100% - 89)",
        "calc(100% - 0)",
        "calc(100% * 2)",
        "calc(50% + 50%)",
        "calc(10px + 20px)",
        "calc(100%)",
        "calc(1e39% - 1px)",
        "calc(nan% - 1px)",
        "calc(env(safe-area-inset-top) + 50%)",
        "calc(100% - 89px",
        "100%",
        "89px",
    ] {
        assert_eq!(Dimension::parse_calc(bad), None, "{bad}");
    }
    let env = Env::default();
    let width = Dimension::Calc(100.0, -89.0).to_taffy(&env);
    assert_eq!(width, Dimension::Calc(100.0, -89.0).to_taffy(&env));
    assert_ne!(width, Dimension::Calc(100.0, -88.0).to_taffy(&env));
    let mut handle = None;
    if let taffy::style::ExpandedDimension::Calc(h) = width.expand() {
        handle = Some(h);
    }
    let handle = handle.expect("a calc() is a calc handle");
    assert_eq!(resolve_calc(handle, 400.0), 311.0);
    assert_eq!(resolve_calc(handle, 0.0), -89.0);
    let mut s = StyleProps::default();
    s.set_dynamic(
        StyleId::Width,
        &StyleValue::Text("calc(100% - 89px)".into()),
    )
    .unwrap();
    assert_eq!(s.width, Dimension::Calc(100.0, -89.0));
    assert!(!uses_env(&s));
    assert_eq!(
        s.set_dynamic(StyleId::Width, &StyleValue::Text("calc(1px + 2px)".into())),
        Err(StyleValueError::WrongKind {
            style: StyleId::Width,
            expected: "number, px, rem or em length, percent, auto, calc(<percent> ± <px>), env(safe-area-inset-*), or env(viewport-segment-* x y)",
        })
    );
}

/// LLP 1078 D3, D10: the viewport segment variables parse by CSS-ENV-1's
/// grammar, refuse by name, resolve against the grid, and travel the wire.
#[test]
fn segment_lengths_parse_resolve_and_refuse_by_name() {
    use env::EnvRefusal;
    env::link();
    let seg = |v, x, y, plus| Dimension::Segment(v, x, y, plus);
    assert_eq!(
        Dimension::parse_env("env(viewport-segment-width 0 0)"),
        Some(seg(SegmentVar::Width, 0, 0, 0.0))
    );
    assert_eq!(
        Dimension::parse_env(" env( viewport-segment-left  1   0 ) "),
        Some(seg(SegmentVar::Left, 1, 0, 0.0))
    );
    assert_eq!(
        Dimension::parse_env("calc(env(viewport-segment-height 0 1) - 12px)"),
        Some(seg(SegmentVar::Height, 0, 1, -12.0))
    );
    assert_eq!(
        Dimension::parse_env("calc(env(viewport-segment-right 15 15) + 2.5px)"),
        Some(seg(SegmentVar::Right, 15, 15, 2.5))
    );
    for (var, name) in SegmentVar::ALL
        .iter()
        .zip(["width", "height", "top", "left", "bottom", "right"])
    {
        assert_eq!(var.name(), name);
        assert_eq!(SegmentVar::from_name(name), Some(*var));
        assert_eq!(SegmentVar::from_index(*var as u8), Some(*var));
    }
    // Refused by name (D10), never silently zero.
    for (bad, why) in [
        ("env(viewport-segment-width 0)", EnvRefusal::IndexCount),
        ("env(viewport-segment-width)", EnvRefusal::IndexCount),
        ("env(viewport-segment-width 0 0 0)", EnvRefusal::IndexCount),
        ("env(viewport-segment-width -1 0)", EnvRefusal::IndexForm),
        ("env(viewport-segment-width 0.5 0)", EnvRefusal::IndexForm),
        ("env(viewport-segment-width 0 16)", EnvRefusal::IndexRange),
        (
            "env(viewport-segment-width 999999999999 0)",
            EnvRefusal::IndexRange,
        ),
        (
            "env(viewport-segment-width 0 0, 10px)",
            EnvRefusal::Fallback,
        ),
        ("env(safe-area-inset-top, 0px)", EnvRefusal::Fallback),
        (
            "env(viewport-segment-middle 0 0)",
            EnvRefusal::UnknownVariable,
        ),
        ("env(safe-area-inset-top 0 0)", EnvRefusal::UnknownVariable),
        ("env(keyboard-inset-height)", EnvRefusal::UnknownVariable),
        (
            "calc(env(viewport-segment-width 0) + 1px)",
            EnvRefusal::IndexCount,
        ),
    ] {
        assert_eq!(env::parse(bad), Err(why), "{bad}");
        assert_eq!(Dimension::parse_env(bad), None, "{bad}");
        let mut s = StyleProps::default();
        assert_eq!(
            s.set_dynamic(StyleId::Width, &StyleValue::Text(bad.into())),
            Err(StyleValueError::BadEnv {
                style: StyleId::Width,
                refusal: why
            }),
            "{bad}"
        );
    }
    // Not env() forms at all: other grammars get their turn.
    for not in [
        "calc(50% + 10px)",
        "12px",
        "auto",
        "calc(env(viewport-segment-width 0 0) * 2)",
    ] {
        assert_eq!(env::parse(not), Ok(None), "{not}");
    }
    // Two columns with a 40-point band between them (the Duo at 130°), in a
    // 951 × 669 viewport.
    let env = Env::new(0.0, 84.0, 34.0, 0.0).with_segments(
        2,
        1,
        vec![
            Rect::new(0.0, 0.0, 455.5, 669.0),
            Rect::new(495.5, 0.0, 455.5, 669.0),
        ],
    );
    assert!(env.segments_consistent() && env.is_finite());
    let at = |v, x, y, plus| seg(v, x, y, plus).resolve(&env);
    assert_eq!(at(SegmentVar::Width, 0, 0, 0.0), Dimension::Points(455.5));
    assert_eq!(at(SegmentVar::Left, 1, 0, 0.0), Dimension::Points(495.5));
    assert_eq!(at(SegmentVar::Right, 0, 0, 0.0), Dimension::Points(455.5));
    assert_eq!(at(SegmentVar::Right, 1, 0, -1.0), Dimension::Points(950.0));
    assert_eq!(at(SegmentVar::Top, 1, 0, 0.0), Dimension::Points(0.0));
    assert_eq!(at(SegmentVar::Bottom, 1, 0, 12.0), Dimension::Points(681.0));
    assert_eq!(at(SegmentVar::Height, 0, 0, 0.0), Dimension::Points(669.0));
    // Past the grid, or on a one-segment viewport: undefined.
    assert_eq!(at(SegmentVar::Width, 0, 1, 0.0), Dimension::Auto);
    assert_eq!(at(SegmentVar::Width, 2, 0, 0.0), Dimension::Auto);
    assert_eq!(
        seg(SegmentVar::Width, 0, 0, 0.0).resolve(&Env::default()),
        Dimension::Auto
    );
    // The insets still resolve beside the grid.
    assert_eq!(
        Dimension::Env(Edge::Right, 0.0).resolve(&env),
        Dimension::Points(84.0)
    );
    assert_eq!(
        env.with_insets(1.0, 2.0, 3.0, 4.0).segments,
        env.segments,
        "insets leave the grid"
    );
    // A malformed grid.
    assert!(!Env::default()
        .with_segments(2, 1, vec![])
        .segments_consistent());
    assert!(!Env::default()
        .with_segments(0, 1, vec![])
        .segments_consistent());
    assert!(!Env::default()
        .with_segments(1, 1, vec![Rect::new(0.0, 0.0, 1.0, 1.0)])
        .segments_consistent());
    // The style reads the environment.
    let mut s = StyleProps::default();
    s.set_dynamic(
        StyleId::Width,
        &StyleValue::Text("env(viewport-segment-width 1 0)".into()),
    )
    .unwrap();
    assert!(uses_env(&s));
    assert_eq!(
        s.to_taffy(NodeType::View, &env).size.width,
        length(455.5_f32)
    );
}

/// An undefined segment length is invalid at computed-value time: the row
/// takes its initial value — `auto` for a width, 0 for a margin, 0 for a
/// padding (the table's defaults), not `auto` everywhere.
#[test]
fn an_undefined_segment_length_takes_the_rows_initial_value() {
    env::link();
    let mut s = StyleProps::default();
    for (row, text) in [
        (StyleId::Width, "env(viewport-segment-width 0 0)"),
        (StyleId::MarginLeft, "env(viewport-segment-left 1 0)"),
        (
            StyleId::PaddingTop,
            "calc(env(viewport-segment-top 0 0) + 8px)",
        ),
        (StyleId::Left, "env(viewport-segment-left 1 0)"),
    ] {
        s.set_dynamic(row, &StyleValue::Text(text.into())).unwrap();
    }
    let one = Env::default();
    let t = s.to_taffy(NodeType::View, &one);
    assert_eq!(t.size.width, auto());
    assert_eq!(
        t.margin.left,
        length(0.0_f32),
        "margin's initial value is 0"
    );
    assert_eq!(t.padding.top, length(0.0_f32));
    assert_eq!(t.inset.left, auto());
    // The authored rows are untouched: the next environment resolves them.
    assert_eq!(s.width, Dimension::Segment(SegmentVar::Width, 0, 0, 0.0));
    let two = one.with_segments(
        2,
        1,
        vec![
            Rect::new(0.0, 0.0, 100.0, 300.0),
            Rect::new(140.0, 0.0, 100.0, 300.0),
        ],
    );
    let t = s.to_taffy(NodeType::View, &two);
    assert_eq!(t.size.width, length(100.0_f32));
    assert_eq!(t.margin.left, length(140.0_f32));
    assert_eq!(t.padding.top, length(8.0_f32));
    assert_eq!(t.inset.left, length(140.0_f32));
}

/// Wire kinds 8–13: a kind byte, the points, then the two indices.
#[test]
fn segment_lengths_round_trip_the_wire() {
    use crate::wire::codec::{Reader, Writer};
    env::link();
    for var in SegmentVar::ALL {
        for d in [
            Dimension::Segment(var, 0, 0, 0.0),
            Dimension::Segment(var, 1, 0, -12.5),
            Dimension::Segment(var, 15, 15, 3.0),
        ] {
            let mut w = Writer::new();
            w.dimension(d);
            let bytes = w.into_vec();
            assert_eq!(bytes.len(), 7, "{d:?}");
            assert_eq!(bytes[0], 8 + var as u8);
            let mut r = Reader::new(&bytes);
            assert_eq!(r.dimension(StyleId::Width, true).unwrap(), d);
        }
    }
    // An index past 15 on the wire is not a dimension.
    let mut r = Reader::new(&[8, 0, 0, 0, 0, 16, 0]);
    assert!(r.dimension(StyleId::Width, true).is_err());
}
