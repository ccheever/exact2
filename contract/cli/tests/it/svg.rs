//! LLP 1055: inline SVG and CSS animations, compiled and booted — the rows
//! and props on the kernel, the keyframes resolved onto the `animation` row,
//! the longhands composed, and every refusal by its id.

use exact_kernel::{Kernel, NodeType, PropId, StyleId};
use exact_motion::{Direction, Easing, FillMode, Property};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn refused(src: &str) -> String {
    match contract::compile(src) {
        Ok(_) => panic!("compiled: {src}"),
        Err(e) => {
            eprintln!("{e}");
            e.to_string()
        }
    }
}

const SPARK: &str = "\
keyframes draw
  from stroke-dashoffset=1
  to stroke-dashoffset=0
keyframes breathe
  from r=3 opacity=0.5
  to r=9 opacity=0
  50% animation-timing-function=\"linear\" r=5

component A
  view
    svg testId=\"chart\" width=96 height=32 viewBox=\"0 0 96 32\" overflow=\"visible\"
      polyline testId=\"line\" points=\"0,32 48,0 96,16\" fill=\"none\" stroke=\"#16a34a\" stroke-width=1.5 stroke-linejoin=\"round\" pathLength=1 stroke-dasharray=\"1\" animation=\"draw 600ms ease-out both\"
      g stroke=\"currentcolor\"
        circle testId=\"ring\" cx=96 cy=16 r=3 fill=\"none\" opacity=0 animation-name=\"breathe\" animation-duration=\"1200ms\" animation-timing-function=\"ease-out\" animation-delay=\"600ms\" animation-iteration-count=\"infinite\"
";

#[test]
fn the_sparkline_compiles_to_nodes_rows_and_resolved_animations() {
    let r = boot(SPARK);
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let chart = node("chart");
    assert_eq!(chart.node_type, NodeType::Svg);
    assert_eq!(chart.props.str(PropId::ViewBox), Some("0 0 96 32"));
    assert_eq!(
        chart.style.overflow_x,
        exact_kernel::Overflow::Visible,
        "authored over the tag's hidden"
    );
    let line = node("line");
    assert_eq!(line.node_type, NodeType::SvgPolyline);
    assert_eq!(
        line.props
            .get(PropId::PathLength)
            .and_then(|v| v.as_float()),
        Some(1.0)
    );
    assert_eq!(line.style.stroke_width, 1.5);
    assert_eq!(line.style.fill, exact_kernel::svg::Paint::None);
    let draw = &line.style.animation.0[0];
    assert_eq!(
        (draw.name.as_str(), draw.duration, draw.fill),
        ("draw", 0.6, FillMode::Both)
    );
    assert_eq!(draw.keyframes.0.len(), 2, "the rule rides with the row");
    let ring = node("ring");
    assert_eq!(ring.style.r, exact_kernel::Dimension::Points(3.0));
    let breathe = &ring.style.animation.0[0];
    assert!(breathe.iterations.is_infinite());
    assert_eq!(
        (breathe.delay, breathe.easing.clone(), breathe.direction),
        (0.6, Easing::EaseOut, Direction::Normal)
    );
    assert_eq!(breathe.keyframes.0[1].easing, Some(Easing::Linear));
    assert_eq!(
        breathe.keyframes.properties(),
        vec![Property::Opacity, Property::R]
    );
    // The ring's stroke is inherited from its `g`.
    let computed = ring.computed_style(exact_kernel::StyleMask::of(StyleId::Stroke));
    assert_eq!(computed.stroke, exact_kernel::svg::Paint::CurrentColor);
}

#[test]
fn a_computed_part_composes_one_animation() {
    let r = boot(
        "keyframes spin\n  to opacity=0\ncomponent A\n  state frozen = true\n  view\n    column testId=\"b\" animation=\"spin 1s linear infinite\" animation-play-state=(frozen ? \"paused\" : \"running\")\n",
    );
    let k = r.kernel();
    let s = &k.node_by_key(k.find_by_test_id("b")[0]).unwrap().style;
    assert!(s.animation.0[0].paused);
    assert!(s.animation.0[0].iterations.is_infinite());
}

#[test]
fn attribute_lengths_refuse_deferred_units_and_functions() {
    let attrs = [
        ("line", "x1"),
        ("line", "y1"),
        ("line", "x2"),
        ("line", "y2"),
        ("linearGradient", "x1"),
        ("linearGradient", "y1"),
        ("linearGradient", "x2"),
        ("linearGradient", "y2"),
        ("radialGradient", "fx"),
        ("radialGradient", "fy"),
        ("radialGradient", "fr"),
        ("marker", "markerWidth"),
        ("marker", "markerHeight"),
        ("marker", "refX"),
        ("marker", "refY"),
        ("text", "x"),
        ("text", "y"),
        ("text", "dx"),
        ("text", "dy"),
        ("tspan", "x"),
        ("tspan", "y"),
        ("tspan", "dx"),
        ("tspan", "dy"),
    ];
    let source = |tag: &str, attr: &str, value: &str| {
        let child = format!("{tag} {attr}={value}");
        let child = if tag == "tspan" {
            format!("text\n        {child}")
        } else {
            child
        };
        format!("component A\n  state n = 2\n  state on = true\n  view\n    svg\n      {child}\n")
    };
    for (tag, attr) in attrs {
        for unit in [
            "em", "rem", "ex", "ch", "vw", "vh", "vi", "vb", "vmin", "vmax", "svw", "svh", "svmin",
            "svmax", "lvw", "lvh", "lvmin", "lvmax", "dvw", "dvh", "dvmin", "dvmax", "lh", "rlh",
            "q", "REM",
        ] {
            let value = if matches!(tag, "text" | "tspan") {
                format!("\"10px, 20% 2{unit}\"")
            } else {
                format!("\"2{unit}\"")
            };
            let error = refused(&source(tag, attr, &value));
            for expected in [
                "lower-svg-length",
                "refused (LLP 1055.000 D4)",
                unit,
                attr,
                "px",
                "%",
            ] {
                assert!(
                    error.contains(expected),
                    "{tag} {attr}: missing {expected}: {error}"
                );
            }
        }
        for (value, name) in [
            ("\"calc(10px + 2%)\"", "calc()"),
            ("\"env(safe-area-inset-top)\"", "env()"),
            ("\"var(--position)\"", "var()"),
            ("`${n}em`", "em"),
            ("`10px, ${n}vh`", "vh"),
            ("`calc(${n}px + 1px)`", "calc()"),
            ("(on ? \"2rem\" : \"1px\")", "rem"),
        ] {
            let error = refused(&source(tag, attr, value));
            assert!(
                error.contains("lower-svg-length") && error.contains(name),
                "{error}"
            );
        }
        for value in [
            "2",
            "\"2\"",
            "\"2px\"",
            "\"2in\"",
            "\"2cm\"",
            "\"2mm\"",
            "\"2pt\"",
            "\"2pc\"",
            "\"20%\"",
            "\"1e2PX\"",
            "\"-1E-2pt\"",
            "n",
            "`${n}px`",
            "`${n}%`",
        ] {
            let src = source(tag, attr, value);
            contract::compile(&src).unwrap_or_else(|e| panic!("{src}: {e}"));
        }
        if matches!(tag, "text" | "tspan") {
            contract::compile(&source(tag, attr, "\"1px, -2.5% 3e2mm\"")).unwrap();
        }
    }
    let src = "component A\n  view\n    svg\n      rect x=\"2em\" y=\"3vh\" width=\"calc(10px + 2%)\"\n      marker orient=\"auto-start-reverse\"\n      linearGradient\n        stop offset=\"2em\"\n      filter\n        feOffset dx=\"2em\" dy=\"calc(1px + 2%)\"\n";
    contract::compile(src).unwrap();
}

#[test]
fn attribute_lengths_keep_split_tokens() {
    let mut failures = Vec::new();
    for (value, expected) in [
        ("`1e${n}px`", "1e2px"),
        ("`2e${n}`", "2e2"),
        ("`2E${n}`", "2E2"),
        ("`${n}e${n}px`", "2e2px"),
        ("`${n}p${suffix}`", "2px"),
        ("`${n}p${on ? \"x\" : \"x\"}`", "2px"),
        ("`${on ? \"2p\" : \"3p\"}x`", "2px"),
        ("`2p${\"x\"}`", "2px"),
        ("`${\"2p\"}x`", "2px"),
        ("`2p${suffix}`", "2px"),
        ("`${prefix}x`", "2px"),
        ("(\"2p\" + \"x\")", "2px"),
        ("(\"2\" + \"p\" + \"x\")", "2px"),
    ] {
        let src = format!("component A\n  state on = true\n  state n = 2\n  state prefix = \"2p\"\n  state suffix = \"x\"\n  view\n    svg\n      line testId=\"line\" x1={value}\n");
        if let Err(e) = contract::compile(&src) {
            failures.push(format!("{value}: {e}"));
            continue;
        }
        let r = boot(&src);
        let k = r.kernel();
        let line = k.node_by_key(k.find_by_test_id("line")[0]).unwrap();
        assert_eq!(line.props.str(PropId::X1), Some(expected));
        assert!(exact_kernel::svg::Length::parse(expected).is_some());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_check_match_arms() {
    for value in [
        "match choice { case some(v) => \"2em\", case none => \"0\" }",
        "match choice { case some(v) => \"0\", case none => \"2ex\" }",
    ] {
        let src = format!("component A\n  state choice: option<number> = some(1)\n  view\n    svg\n      line x1=({value})\n");
        assert!(refused(&src).contains("lower-svg-length"));
    }
}

#[test]
fn attribute_lengths_check_shared_derive_bodies() {
    let src = "component A\n  state suffix = \"px\"\n  view\n    svg\n      Child(suffix=suffix)\ncomponent Child\n  props\n    suffix: string\n  derive unit = suffix\n  derive twice = (unit == unit ? \"2em\" : \"1px\")\n  view\n    line x1=twice\n";
    assert!(refused(src).contains("lower-svg-length"));
}

#[test]
fn attribute_lengths_check_concatenated_literals() {
    let mut failures = Vec::new();
    for value in [
        "(\"2\" + \"em\")",
        "(\"2e\" + \"m\")",
        "(\"2\" + \"e\" + \"x\")",
        "(\"cal\" + \"c(1px)\")",
        "`2e${\"m\"}`",
        "`${\"2e\"}x`",
    ] {
        let src = format!("component A\n  view\n    svg\n      line x1={value}\n");
        match contract::compile(&src) {
            Ok(_) => failures.push(format!("compiled: {value}")),
            Err(e) => assert!(e.to_string().contains("lower-svg-length"), "{value}: {e}"),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_preserve_marker_keywords() {
    let mut failures = Vec::new();
    for (attr, words) in [
        ("refX", ["left", "center", "right"]),
        ("refY", ["top", "center", "bottom"]),
    ] {
        for word in words {
            for value in [
                format!("\"{word}\""),
                format!("(\"\" + (on ? \"{word}\" : \"{word}\"))"),
                format!("`${{on ? \"{word}\" : \"{word}\"}}`"),
            ] {
                let src = format!("component A\n  state on = true\n  view\n    svg\n      marker {attr}={value}\n");
                if let Err(e) = contract::compile(&src) {
                    failures.push(format!("{attr}={value}: {e}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_check_choices_with_literal_neighbors() {
    let mut failures = Vec::new();
    for (tag, attr, value, unit) in [
        ("line", "x1", "`${on ? \"2e\" : \"2c\"}m`", "em"),
        ("line", "x1", "`${on ? \"2em\" : \"1px\"} `", "em"),
        ("line", "x1", "((on ? \"2em\" : \"1px\") + \" \")", "em"),
        ("text", "x", "`${on ? \"2em\" : \"1px\"} 10px`", "em"),
        ("text", "x", "`${on ? \"1px\" : \"2em\"},10px`", "em"),
        (
            "line",
            "x1",
            "`${match choice { case some(v) => \"2e\", case none => \"2c\" }}m`",
            "em",
        ),
        ("line", "x1", "`2em${n}`", "em"),
        ("line", "x1", "`2em${n}px`", "em"),
        ("line", "x1", "`${on ? \"cal\" : \"cal\"}c(1px)`", "calc()"),
    ] {
        let src = format!("component A\n  state on = true\n  state n = \"\"\n  state choice: option<number> = some(1)\n  view\n    svg\n      {tag} {attr}={value}\n");
        match contract::compile(&src) {
            Ok(_) => failures.push(format!("compiled: {value}")),
            Err(e) if e.id == "lower-svg-length" && e.message.contains(&format!("`{unit}`")) => {}
            Err(e) => failures.push(format!("{value}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_keep_marker_keywords_with_empty_neighbors() {
    let mut failures = Vec::new();
    for (attr, a, b) in [("refX", "left", "right"), ("refY", "top", "bottom")] {
        for value in [
            format!("(on ? \"{a} \" : \"{b} \")"),
            format!("((on ? \"{a} \" : \"{b} \") + \"\")"),
            format!("(\"\" + (on ? \" {a}\" : \" {b}\"))"),
            format!("`${{on ? \"{a}\" : \"{b}\"}} `"),
        ] {
            let src = format!(
                "component A\n  state on = true\n  view\n    svg\n      marker {attr}={value}\n"
            );
            if let Err(e) = contract::compile(&src) {
                failures.push(format!("{attr}={value}: {e}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_check_shared_derive_values() {
    let src = "component A\n  view\n    svg\n      Child()\ncomponent Child\n  derive len = \"2em\"\n  derive pick = (len == len ? len : \"0\")\n  view\n    line x1=pick\n";
    assert!(refused(src).contains("lower-svg-length"));
}

#[test]
fn attribute_lengths_keep_shared_derive_context() {
    for (len, pick, value) in [
        ("2p", "(len == len ? len : \"3p\")", "`${pick}x`"),
        ("em", "(len == len ? \"2px\" : \"1px\")", "pick"),
    ] {
        let src = format!("component A\n  view\n    svg\n      Child()\ncomponent Child\n  derive len = \"{len}\"\n  derive pick = {pick}\n  view\n    line testId=\"line\" x1={value}\n");
        let r = boot(&src);
        let k = r.kernel();
        let line = k.node_by_key(k.find_by_test_id("line")[0]).unwrap();
        assert_eq!(line.props.str(PropId::X1), Some("2px"));
    }
}

#[test]
fn attribute_lengths_bound_shared_derive_work() {
    let mut failures = Vec::new();
    for choices in [true, false] {
        let mut source = String::from("component A\n  state on = true\n  view\n    svg\n      Child(on=on)\ncomponent Child\n  props\n    on: bool\n");
        source.push_str(if choices {
            "  derive d0 = \"1px\"\n"
        } else {
            "  derive d0 = \"\"\n"
        });
        for i in 1..=30 {
            let prev = i - 1;
            let value = if choices {
                format!("(on ? d{prev} : d{prev})")
            } else {
                format!("(d{prev} + d{prev})")
            };
            source.push_str(&format!("  derive d{i} = {value}\n"));
        }
        source.push_str("  view\n    line x1=d30\n");
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = contract::compile(&source)
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = send.send(result);
        });
        match receive.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            result => failures.push(format!("choices={choices}: {result:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_keep_marker_keywords_with_unknown_padding() {
    let mut failures = Vec::new();
    for (attr, a, b) in [("refX", "left", "right"), ("refY", "top", "bottom")] {
        for value in [
            format!("`${{on ? \"{a}\" : \"{b}\"}} ${{padding}}`"),
            format!("`${{padding}} ${{on ? \"{a}\" : \"{b}\"}}`"),
            format!("`${{padding}} ${{on ? \"{a}\" : \"{b}\"}} ${{padding}}`"),
        ] {
            let src = format!("component A\n  state on = true\n  state padding = \"\"\n  view\n    svg\n      marker {attr}={value}\n");
            if let Err(e) = contract::compile(&src) {
                failures.push(format!("{attr}={value}: {e}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn attribute_lengths_refuse_finished_bare_exponents() {
    let mut failures = Vec::new();
    for unit in ["e", "E"] {
        assert!(exact_kernel::svg::Length::parse(&format!("2{unit}")).is_none());
        for child in [
            format!("line x1=\"2{unit}\""),
            format!("text x=\"2{unit}, 4px\""),
        ] {
            let src = format!("component A\n  view\n    svg\n      {child}\n");
            match contract::compile(&src) {
                Ok(_) => failures.push(format!("compiled: {child}")),
                Err(e) => assert!(e.id == "lower-svg-length", "{child}: {e}"),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn refusals_are_named() {
    let svg = |body: &str| {
        format!("component A\n  state n = 0\n  action go\n    n = 1\n  view\n    svg width=10 height=10\n      {body}\n")
    };
    assert!(refused("component A\n  view\n    circle r=3\n").contains("lower-svg-content"));
    assert!(refused(&svg("column")).contains("lower-svg-content"));
    assert!(refused(&svg("text \"hi\" rotate=\"10 20\"")).contains("later stage"));
    // LLP 1055.000 §8 ruling 5: HTML inside, the web's; SVG shapes need an `svg`.
    assert!(
        refused(&svg("foreignObject width=10 height=10\n        circle r=3"))
            .contains("lower-svg-content")
    );
    assert!(refused(&svg("animate")).contains("SMIL is refused"));
    assert!(
        refused(&svg("defs press=go")).contains("lower-svg-attr"),
        "a definition handles no events"
    );
    assert!(refused(&svg("circle r=3 dblclick=go")).contains("later stage"));
    assert!(refused(&svg("circle r=3 padding=4")).contains("lower-svg-attr"));
    assert!(refused(&svg("rect points=\"0,0\"")).contains("lower-attr-tag"));
    assert!(refused("component A\n  view\n    column points=\"0,0\"\n").contains("lower-attr-tag"));
    assert!(refused(&svg("filter\n        feImage")).contains("later stage"));
    assert!(refused(&svg("hatch")).contains("Chrome does not implement"));
    assert!(refused(&svg("linearGradient\n        rect width=1")).contains("does not hold"));
    assert!(refused(&svg("circle r=3 animation=\"nope 1s\"")).contains("lower-animation-name"));
    assert!(
        refused("keyframes k\n  to width=3\ncomponent A\n  view\n    column\n")
            .contains("lower-keyframe-property")
    );
    assert!(refused("keyframes k\n  to r=2\ncomponent A\n  view\n    svg\n      rect width=1 animation=\"k 1s\"\n").contains("lower-animation-target"));
    assert!(
        refused("keyframes k\n  120% opacity=1\ncomponent A\n  view\n    column\n")
            .contains("syntax-keyframe-selector")
    );
}

// LLP 1055.000 stage 1: ellipse, nested viewports, geometry rows and
// lengths, transforms in both grammars, `class=` on SVG elements, and SVG
// inside a component.
const STAGE1: &str = "\
style Accent
  fill=\"#ff0000\"
  stroke=\"#000000\"

component A
  state w = 40
  view
    svg testId=\"root\" viewBox=\"0 0 100 50\"
      ellipse testId=\"e\" cx=50 cy=\"50%\" rx=10 class=Accent transform=\"rotate(30 50 25)\" transform-box=\"fill-box\" transform-origin=\"center\"
      rect testId=\"r\" x=\"10%\" y=2 width=w height=\"1in\" rx=2 rotate=15
      g testId=\"g\" transform=\"translate(10px, 5px) scale(2)\" visibility=\"hidden\"
        Tick(at=w)
      svg testId=\"inner\" x=5 y=5 width=\"50%\" height=20 viewBox=\"0 0 10 10\" overflow=\"visible\"
        circle cx=5 cy=5 r=\"50%\"

component Tick
  props
    at: number
  view
    line testId=\"tick\" x1=at x2=at y1=\"0\" y2=\"100%\" vector-effect=\"non-scaling-stroke\"
";

#[test]
fn stage_one_geometry_transforms_and_classes() {
    use exact_kernel::Dimension::{Percent, Points};
    let r = boot(STAGE1);
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let e = node("e");
    assert_eq!(e.node_type, NodeType::SvgEllipse);
    assert_eq!(
        (e.style.cx, e.style.cy, e.style.rx),
        (Points(50.0), Percent(50.0), Points(10.0))
    );
    assert_eq!(e.style.ry, exact_kernel::Dimension::Auto);
    assert_eq!(e.style.fill.css(), "#ff0000ff", "the class's rows apply");
    assert_eq!(
        e.style.transform.css(),
        "translate(50px, 25px) rotate(30deg) translate(-50px, -25px)"
    );
    assert_eq!(e.style.transform_box, exact_kernel::TransformBox::FillBox);
    let rect = node("r");
    assert_eq!((rect.style.x, rect.style.y), (Percent(10.0), Points(2.0)));
    assert_eq!(rect.style.height, Points(96.0), "1in is 96 user units");
    assert_eq!((rect.style.width, rect.style.rotate), (Points(40.0), 15.0));
    // A component's root inside a `g`; a computed number is a length's text.
    let tick = node("tick");
    assert_eq!(tick.node_type, NodeType::SvgLine);
    assert_eq!(tick.props.str(PropId::X1), Some("40"));
    assert_eq!(tick.props.str(PropId::Y2), Some("100%"));
    assert_eq!(
        tick.style.vector_effect,
        exact_kernel::VectorEffect::NonScalingStroke
    );
    assert_eq!(node("g").style.visibility, exact_kernel::Visibility::Hidden);
    let inner = node("inner");
    assert_eq!(inner.node_type, NodeType::SvgViewport);
    assert_eq!(inner.style.width, Percent(50.0));
}

// LLP 1055.000 stage 2: colour keyframes and transitions, transforms on SVG
// elements animated.
#[test]
fn stage_two_colour_and_transform_motion() {
    let r = boot(
        "keyframes flash\n  from color=\"#16a34a\"\n  to color=\"#000000\"\nkeyframes spin\n  to rotate=90\ncomponent A\n  view\n    column\n      text \"$1\" testId=\"p\" animation=\"flash 400ms ease\" transition=\"background-color 200ms\"\n      svg width=10 height=10\n        rect testId=\"r\" width=4 height=4 animation=\"spin 1s linear infinite\"\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let p = node("p");
    let flash = &p.style.animation.0[0];
    assert_eq!(flash.keyframes.properties(), vec![Property::Color]);
    let from = flash.keyframes.0[0].values[0].1;
    assert_eq!(from.to_rgba8(), [0x16, 0xa3, 0x4a, 255]);
    assert!(p.style.transition.0[0]
        .property
        .covers(Property::BackgroundColor));
    let spin = &node("r").style.animation.0[0];
    assert_eq!(spin.keyframes.properties(), vec![Property::Rotate]);
}

// LLP 1055.000 stage 3: definitions, gradients, use and symbol, paint order.
#[test]
fn stage_three_references_and_servers() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      defs\n        radialGradient id=\"g\" fx=\"30%\" fr=0.1 spreadMethod=\"reflect\"\n          stop testId=\"s\" offset=\"50%\" stop-color=\"#ff0000\" stop-opacity=0.5\n        symbol id=\"i\" viewBox=\"0 0 24 24\"\n          path d=\"M0 0L24 24\"\n      circle testId=\"c\" r=4 fill=\"url(#g) #00ff00\" paint-order=\"stroke\"\n      use testId=\"u\" href=\"#i\" x=2 width=6 height=6\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let c = node("c");
    assert_eq!(c.style.fill.css(), "url(#g) #00ff00ff");
    assert_eq!(c.style.paint_order.css(), "stroke fill markers");
    let s = node("s");
    assert_eq!(s.node_type, NodeType::SvgStop);
    assert_eq!(s.props.str(PropId::Offset), Some("50%"));
    assert_eq!(s.style.rare.stop_opacity, 0.5);
    let u = node("u");
    assert_eq!(u.node_type, NodeType::SvgUse);
    assert_eq!(u.props.str(PropId::Href), Some("#i"));
    assert_eq!(
        k.resolve_id(u.id, "i")
            .and_then(|t| k.node(t))
            .map(|n| n.node_type),
        Some(NodeType::SvgSymbol)
    );
}

// LLP 1055.000 stage 4: clipPath and clip-path on SVG elements.
#[test]
fn stage_four_clipping() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      clipPath id=\"c\" clipPathUnits=\"objectBoundingBox\"\n        circle cx=0.5 cy=0.5 r=0.5 clip-rule=\"evenodd\"\n      rect testId=\"r\" width=4 height=4 clip-path=\"url(#c)\"\n",
    );
    let k = r.kernel();
    let rect = k.node_by_key(k.find_by_test_id("r")[0]).unwrap();
    assert_eq!(rect.style.rare.clip_path.url(), Some("c"));
    assert!(
        refused("component A\n  view\n    column clip-path=\"url(#c)\"\n")
            .contains("clips SVG elements")
    );
    assert!(
        refused("component A\n  view\n    svg\n      clipPath\n        g\n")
            .contains("does not hold")
    );
}

// LLP 1055.000 stage 5: SVG text and its runs.
#[test]
fn stage_five_text() {
    let r = boot(
        "component A\n  view\n    column\n      text \"a box's text\" testId=\"box\"\n      svg width=100 height=40\n        text \"Q1\" testId=\"t\" x=50 y=20 text-anchor=\"middle\" dominant-baseline=\"central\" font-size=12\n        text testId=\"u\" x=4 y=36\n          tspan \"12\" testId=\"s\" dx=2 font-weight=700\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    assert_eq!(
        node("box").node_type,
        NodeType::Text,
        "outside an svg, text is a box"
    );
    let t = node("t");
    assert_eq!(t.node_type, NodeType::SvgText);
    assert_eq!(t.props.str(PropId::TextX), Some("50"));
    assert_eq!(t.props.str(PropId::Text), Some("Q1"));
    assert_eq!(t.style.text_anchor, exact_kernel::TextAnchor::Middle);
    let s = node("s");
    assert_eq!(s.node_type, NodeType::SvgTSpan);
    assert_eq!(s.props.str(PropId::TextDx), Some("2"));
}

// LLP 1055.000 stage 6: an element that renders takes events, and
// `pointer-events` decides what of it is hit.
#[test]
fn stage_six_events() {
    let r = boot(
        "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    svg width=10 height=10\n      g press=go\n        circle r=3 testId=\"c\" press=go pointer-events=\"stroke\"\n",
    );
    let k = r.kernel();
    let c = k.node_by_key(k.find_by_test_id("c")[0]).unwrap();
    assert_eq!(c.style.pointer_events, exact_kernel::PointerEvents::Stroke);
}

// LLP 1055.000 stage 7: `marker`, its attributes, and the marker rows.
#[test]
fn stage_seven_markers() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      marker id=\"m\" testId=\"m\" markerWidth=4 refX=2 orient=\"auto\"\n        circle r=2\n      polyline testId=\"p\" points=\"0,0 5,5\" marker=\"url(#m)\"\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let m = node("m");
    assert_eq!(m.node_type, NodeType::SvgMarker);
    assert_eq!(m.props.str(PropId::MarkerWidth), Some("4"));
    assert_eq!(m.style.overflow_x, exact_kernel::Overflow::Hidden);
    let p = node("p");
    for row in [
        &p.style.marker_start,
        &p.style.marker_mid,
        &p.style.marker_end,
    ] {
        assert_eq!(row.url(), Some("m"), "`marker` sets all three");
    }
    let svg =
        |body: &str| format!("component A\n  view\n    svg width=10 height=10\n      {body}\n");
    assert!(refused(&svg("rect markerWidth=3")).contains("lower-attr-tag"));
}

// LLP 1055.000 stage 8: `mask` and `pattern`, their attributes, and the
// `mask` and `mask-type` rows.
#[test]
fn stage_eight_masks_and_patterns() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      mask id=\"m\" testId=\"m\" maskUnits=\"userSpaceOnUse\" x=0 y=0 width=10 height=10 mask-type=\"alpha\"\n        rect width=5 height=5 fill=\"#ffffff\"\n      pattern id=\"p\" testId=\"p\" width=4 height=4 patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\" viewBox=\"0 0 2 2\"\n        circle cx=1 cy=1 r=1\n      rect testId=\"r\" width=10 height=10 fill=\"url(#p)\" mask=\"url(#m)\"\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    let m = node("m");
    assert_eq!(m.node_type, NodeType::SvgMask);
    assert_eq!(m.props.str(PropId::MaskUnits), Some("userSpaceOnUse"));
    assert_eq!(m.style.mask_type, exact_kernel::MaskType::Alpha);
    let p = node("p");
    assert_eq!(p.node_type, NodeType::SvgPattern);
    assert_eq!(p.props.str(PropId::PatternTransform), Some("rotate(45)"));
    assert_eq!(node("r").style.rare.svg_mask.url(), Some("m"));
    let svg =
        |body: &str| format!("component A\n  view\n    svg width=10 height=10\n      {body}\n");
    assert!(refused(&svg("rect maskUnits=\"userSpaceOnUse\"")).contains("lower-attr-tag"));
    let act = "component A\n  state n = 0\n  action go\n    n = 1\n  view\n    svg width=10 height=10\n      mask press=go\n";
    assert!(
        refused(act).contains("lower-svg-attr"),
        "a mask handles no events"
    );
}

// LLP 1055.000 stage 9: `filter`, its primitives as `SvgFe` nodes named by
// their `fe` prop, the renamed attributes, and the filter rows.
#[test]
fn stage_nine_filters() {
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      filter id=\"f\" testId=\"f\" filterUnits=\"userSpaceOnUse\" x=0 y=0 width=10 height=10\n        feFlood testId=\"fl\" flood-color=\"#0f172a\" flood-opacity=0.45 result=\"a\"\n        feOffset testId=\"o\" in=\"a\" dx=3 dy=4\n        feDiffuseLighting testId=\"l\" lighting-color=\"#ffffff\"\n          fePointLight testId=\"p\" x=1 y=2 z=3\n      rect testId=\"r\" width=10 height=10 filter=\"url(#f) blur(2px) drop-shadow(1px 2px 3px #000000)\"\n",
    );
    let k = r.kernel();
    let node = |id: &str| k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
    assert_eq!(node("f").node_type, NodeType::SvgFilter);
    let fl = node("fl");
    assert_eq!(fl.node_type, NodeType::SvgFe);
    assert_eq!(fl.props.str(PropId::Fe), Some("feFlood"));
    assert_eq!(fl.style.rare.flood_opacity, 0.45);
    let o = node("o");
    assert_eq!(o.props.str(PropId::In), Some("a"));
    assert_eq!(o.props.str(PropId::FeDx), Some("3"));
    assert_eq!(node("p").props.str(PropId::LightZ), Some("3"));
    assert_eq!(node("r").style.filter.0.len(), 3);
    let svg =
        |body: &str| format!("component A\n  view\n    svg width=10 height=10\n      {body}\n");
    assert!(refused(&svg("feFlood")).contains("lower-svg-content"));
    assert!(refused(&svg("rect filter=\"wobble(2)\"")).contains("lower-attr-value"));
}

// LLP 1055.000 stage 10: geometry keyframes and blending on SVG elements only.
#[test]
fn stage_ten_geometry_and_blend() {
    let r = boot(
        "keyframes slide\n  from cx=1\n  to cx=9\ncomponent A\n  view\n    svg width=10 height=10\n      circle testId=\"c\" r=1 animation=\"slide 1s\" mix-blend-mode=\"screen\" isolation=\"isolate\"\n",
    );
    let k = r.kernel();
    let c = k.node_by_key(k.find_by_test_id("c")[0]).unwrap();
    assert_eq!(c.style.mix_blend_mode, exact_kernel::MixBlendMode::Screen);
    assert!(c
        .style
        .animation
        .properties()
        .contains(&exact_motion::Property::Cx));
    assert!(
        refused("component A\n  view\n    column mix-blend-mode=\"multiply\"\n")
            .contains("lower-attr-tag")
    );
}

// LLP 1055.000 D15 (issue #123): a path's `d` is a transition property, as
// CSS names it; `all` covers it. No keyframe names it yet.
#[test]
fn d_transitions_and_all_covers_it() {
    use exact_motion::Property;
    let r = boot(
        "component A\n  view\n    svg width=10 height=10\n      path testId=\"p\" d=\"M0 0 L5 5\" transition=\"d 400ms linear\"\n      path testId=\"q\" d=\"M0 0 L5 5\" transition=\"all 400ms linear\"\n",
    );
    let k = r.kernel();
    for id in ["p", "q"] {
        let n = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
        let t = n.style.transition.matching(Property::D).expect("covers d");
        assert_eq!(t.duration, 0.4);
    }
    assert!(refused(
        "keyframes morph\n  from d=\"M0 0 L5 5\"\n  to d=\"M0 0 L9 9\"\ncomponent A\n  view\n    svg width=10 height=10\n      path d=\"M0 0 L5 5\" animation=\"morph 1s\"\n"
    )
    .contains("lower-"));
}

#[test]
fn box_transforms_are_refused_inline_and_through_classes() {
    let mut failures = Vec::new();
    for element in [
        "view",
        "box",
        "column",
        "row",
        "text",
        "image",
        "input",
        "textarea",
        "button aria-label=\"Move\"",
        "paint-surface",
    ] {
        for attr in [
            "transform=\"scale(0.8)\"",
            "transform=\"translate(0px, 25px)\"",
            "transform=matrix",
            "transform=(small ? \"scale(0.8)\" : \"none\")",
            "transform=\"none\"",
            "class=Scaled",
            "class=Shifted",
            "class=(small ? Scaled : Shifted)",
            "class=(small ? Scaled : Empty)",
        ] {
            let src = format!(
                r#"style Scaled
  transform="scale(0.8)"
style Shifted
  transform="translate(0px, 25px)"
style Empty
  opacity=1
component App
  state matrix = "scale(0.8)"
  state small = true
  view
    {element} {attr}
"#
            );
            match contract::compile(&src) {
                Ok(_) => failures.push(format!("compiled: {element} {attr}")),
                Err(e) => {
                    for expected in [
                        "lower-attr-tag",
                        "`transform`",
                        "deferred",
                        "LLP 1055.000 D5",
                        "`translate`",
                        "`rotate`",
                        "`scale`",
                        "translate=\"0px 25px\"",
                        "scale=0.8",
                    ] {
                        if !e.to_string().contains(expected) {
                            failures.push(format!("{element} {attr}: missing {expected}: {e}"));
                        }
                    }
                }
            }
        }
    }
    let html_in_svg = r#"component App
  view
    svg
      foreignObject width=100 height=100
        text "HTML text" transform="scale(0.8)"
"#;
    match contract::compile(html_in_svg) {
        Ok(_) => failures.push("compiled a box inside foreignObject".into()),
        Err(e) if e.id == "lower-attr-tag" && e.message.contains("deferred") => {}
        Err(e) => failures.push(e.to_string()),
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn box_transforms_report_independent_attribute_refusals() {
    let errors = |source: &str| {
        let Err(errors) = contract::compile_path_source_all(
            std::path::Path::new("box-transform-diagnostics.contract"),
            source,
            false,
        ) else {
            panic!("compiled an invalid transform fixture: {source}");
        };
        errors
    };
    let mut failures = Vec::new();
    let width = "transition=\"width 260ms ease\"";
    for tag in ["column", "text \"probe\"", "step-strip"] {
        for transform in [
            "transform=\"translateX(24px)\"",
            "transform=(small ? \"translateX(0px)\" : \"translateX(24px)\")",
        ] {
            let source =
                |body: &str| format!("component App\n  state small = true\n  view\n    {body}\n");
            let width_errors = errors(&source(&format!("{tag} {width}")));
            let transform_errors = errors(&source(&format!("{tag} {transform}")));
            if width_errors.len() != 1 || transform_errors.len() != 1 {
                failures.push(format!(
                    "{tag} {transform}: {width_errors:?} {transform_errors:?}"
                ));
                continue;
            }
            let width_error = &width_errors[0];
            let transform_error = &transform_errors[0];
            if width_error.id != "lower-attr-value" || transform_error.id != "lower-attr-tag" {
                failures.push(format!(
                    "{tag} {transform}: {width_error:?} {transform_error:?}"
                ));
            }
            let mut message = contract_lower::vocab::BOX_TRANSFORM_REFUSAL.to_string();
            if tag == "step-strip" {
                message.push_str("; on `step-strip` a name Contract knows binds to the box, not the module (LLP 1024 D1), so give the module prop another name");
            }
            if transform_error.message != message {
                failures.push(format!("{tag} {transform}: {}", transform_error.message));
            }
            for (body, expected) in [
                (
                    format!("{tag} {width} {transform}"),
                    [
                        ("transition", width_error, 4),
                        ("transform", transform_error, 4),
                    ],
                ),
                (
                    format!("{tag} {transform} {width}"),
                    [
                        ("transform", transform_error, 4),
                        ("transition", width_error, 4),
                    ],
                ),
                (
                    format!("column\n      {tag} {width}\n      {tag} {transform}"),
                    [
                        ("transition", width_error, 5),
                        ("transform", transform_error, 6),
                    ],
                ),
                (
                    format!("column\n      {tag} {transform}\n      {tag} {width}"),
                    [
                        ("transform", transform_error, 5),
                        ("transition", width_error, 6),
                    ],
                ),
            ] {
                let source = source(&body);
                let found = errors(&source);
                if found.len() != expected.len() {
                    failures.push(format!("{source}\n{found:?}"));
                    continue;
                }
                for (actual, (name, reference, line)) in found.iter().zip(expected) {
                    let text = source.lines().nth(line as usize - 1).unwrap();
                    let col = text.find(name).unwrap() as u32 + 1;
                    if actual.id != reference.id
                        || actual.message != reference.message
                        || actual.file != reference.file
                        || actual.span.source_id != reference.span.source_id
                        || actual.span.line != line
                        || actual.span.col != col
                        || actual.span.end_col != col + name.len() as u32
                        || !actual.related.is_empty()
                    {
                        failures.push(format!("{source}\n{name}: {actual:?}"));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn svg_transforms_keep_static_dynamic_and_class_rows() {
    let mut r = boot(
        r#"style Shifted
  transform="translate(0px, 25px)"
style Scaled
  transform="scale(0.8)"
component App
  state matrix = "scale(0.8)"
  state small = true
  action change
    matrix = "translate(0px, 25px)"
    small = false
  view
    svg testId="root" transform="scale(0.8)"
      svg testId="nested" class=Shifted
      g testId="group" class=(small ? Scaled : Shifted)
        rect testId="shape" transform=matrix width=10 height=10
        text testId="text" transform="translate(0px, 25px)"
          tspan "SVG text" testId="span" transform=matrix
      foreignObject testId="foreign" transform="scale(0.8)" width=10 height=10
        text "HTML text"
"#,
    );
    let matrix = |r: &Runner<NoData>, id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .transform
            .matrix()
    };
    let scaled = [0.8, 0.0, 0.0, 0.8, 0.0, 0.0];
    let shifted = [1.0, 0.0, 0.0, 1.0, 0.0, 25.0];
    for id in ["root", "group", "shape", "span", "foreign"] {
        assert_eq!(matrix(&r, id), scaled, "{id}");
    }
    for id in ["nested", "text"] {
        assert_eq!(matrix(&r, id), shifted, "{id}");
    }
    r.act("change", vec![]).unwrap();
    for id in ["group", "shape", "span"] {
        assert_eq!(matrix(&r, id), shifted, "{id}");
    }
    contract::compile(
        "component App\n  view\n    box translate=\"0px 25px\" rotate=15 scale=0.8\n",
    )
    .unwrap();
    let r =
        boot("component App\n  view\n    step-strip testId=\"strip\" fx=\"bloom\" offset=\"2\"\n");
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("strip")[0]).unwrap();
    assert_eq!(
        node.props.str(PropId::NativeViewProps),
        Some(r#"{"fx":"bloom","offset":"2"}"#)
    );
}
