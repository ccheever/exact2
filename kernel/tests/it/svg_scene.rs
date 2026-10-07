//! LLP 1055.000 D1, D4, D5: the resolved scene — transforms composed as
//! Chrome composes them (pinned to Chrome 154's `getCTM()`), lengths against
//! the nearest viewport, inheritance carried down, `display` and
//! `visibility`, and an `svg`'s natural ratio from its view box.
use exact_kernel::svg::scene::{self, Item, Kind};
use exact_kernel::svg::transform::{self as tf, Affine};
use exact_kernel::{
    Kernel, NodeType, Offer, Op, PropId, PropValue, StyleId, StyleProps, StyleValue,
};

fn style(id: u32, rows: &[(StyleId, &str)]) -> Op {
    let mut patch = StyleProps::default();
    for (row, value) in rows {
        let v = match (
            value.parse::<f64>(),
            value.strip_suffix('%').map(str::parse::<f64>),
        ) {
            (Ok(n), _) => StyleValue::Number(n),
            (_, Some(Ok(p))) => StyleValue::Percent(p),
            _ => StyleValue::Text((*value).into()),
        };
        patch
            .set_dynamic(*row, &v)
            .unwrap_or_else(|e| panic!("{row:?} = {value}: {e:?}"));
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

fn prop(id: u32, prop: PropId, value: &str) -> Op {
    Op::SetProp {
        id,
        prop,
        value: PropValue::Str(value.into()),
    }
}

struct Doc {
    ops: Vec<Op>,
    next: u32,
}

impl Doc {
    fn new() -> Doc {
        Doc {
            ops: vec![Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            }],
            next: 2,
        }
    }

    fn node(&mut self, t: NodeType, rows: &[(StyleId, &str)], props: &[(PropId, &str)]) -> u32 {
        let id = self.next;
        self.next += 1;
        self.ops.push(Op::CreateView { id, node_type: t });
        if !rows.is_empty() {
            self.ops.push(style(id, rows));
        }
        for (p, v) in props {
            self.ops.push(prop(id, *p, v));
        }
        id
    }

    fn children(&mut self, id: u32, children: &[u32]) {
        self.ops.push(Op::SetChildren {
            id,
            children: children.to_vec(),
        });
    }

    fn kernel(mut self, root_children: &[u32], width: f32) -> Kernel {
        self.children(1, root_children);
        self.ops.push(Op::AttachRoot { id: 1 });
        let mut k = Kernel::with_monospace();
        k.apply(0, 1, &self.ops).unwrap();
        k.compute_layout(1, Offer::definite(width, 800.0)).unwrap();
        k
    }
}

fn resolve(k: &Kernel, svg: u32) -> scene::Scene {
    let node = k.node(svg).unwrap();
    let content = scene::content_box(&node);
    scene::resolve(k, &node, content, &|_, _| None)
}

fn find(items: &[Item], id: u32) -> Option<&Item> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if let Kind::Group(c) | Kind::Viewport { children: c, .. } = &item.kind {
            if let Some(found) = find(c, id) {
                return Some(found);
            }
        }
    }
    None
}

fn close(a: Affine, b: Affine, what: &str) {
    for i in 0..6 {
        assert!((a[i] - b[i]).abs() < 2e-3, "{what}: {a:?} != {b:?}");
    }
}

#[test]
fn transforms_compose_as_chrome_composes_them() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "200"), (StyleId::Height, "100")],
        &[(PropId::ViewBox, "0 0 100 50")],
    );
    let rect = [
        (StyleId::X, "10"),
        (StyleId::Y, "10"),
        (StyleId::Width, "20"),
        (StyleId::Height, "10"),
    ];
    // SVG's attribute grammar: a centre on rotate, unitless numbers.
    let mut a_rows = rect.to_vec();
    a_rows.push((
        StyleId::Transform,
        "rotate(30 10 10) translate(5 2) scale(2)",
    ));
    let a = d.node(SvgRect, &a_rows, &[]);
    // The individual properties first, about the fill box's centre.
    let mut b_rows = rect.to_vec();
    b_rows.extend([
        (StyleId::TransformBox, "fill-box"),
        (StyleId::TransformOrigin, "center"),
        (StyleId::Rotate, "15"),
        (StyleId::Translate, "3px 4px"),
        (StyleId::Scale, "1.5"),
        (StyleId::Transform, "skewX(10)"),
    ]);
    let b = d.node(SvgRect, &b_rows, &[]);
    // A percentage origin on the view box.
    let c = d.node(
        SvgGroup,
        &[(StyleId::TransformOrigin, "50% 50%"), (StyleId::Scale, "2")],
        &[],
    );
    let c_rect = d.node(SvgRect, &rect, &[]);
    d.children(c, &[c_rect]);
    // Keywords on an ellipse's fill box; an auto `ry` takes `rx`.
    let e = d.node(
        SvgEllipse,
        &[
            (StyleId::Cx, "50"),
            (StyleId::Cy, "25"),
            (StyleId::Rx, "10"),
            (StyleId::TransformBox, "fill-box"),
            (StyleId::TransformOrigin, "right bottom"),
            (StyleId::Rotate, "90"),
        ],
        &[],
    );
    // A nested viewport: percentages of the outer view box, its own.
    let nested = d.node(
        SvgViewport,
        &[
            (StyleId::X, "10%"),
            (StyleId::Y, "5"),
            (StyleId::Width, "50%"),
            (StyleId::Height, "20"),
        ],
        &[
            (PropId::ViewBox, "0 0 10 10"),
            (PropId::PreserveAspectRatio, "xMinYMid meet"),
        ],
    );
    let f = d.node(
        SvgRect,
        &[
            (StyleId::X, "1"),
            (StyleId::Y, "1"),
            (StyleId::Width, "2"),
            (StyleId::Height, "2"),
        ],
        &[],
    );
    d.children(nested, &[f]);
    d.children(svg, &[a, b, c, e, nested]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    // Chrome 154, `getCTM()` (to the outer svg's viewport, the view box's
    // scale of 2 included).
    close(
        find(&s.items, a).unwrap().ctm,
        [3.4641, 2.0, -2.0, 3.4641, 19.3397, 1.1436],
        "a",
    );
    close(
        find(&s.items, b).unwrap().ctm,
        [2.8978, 0.7765, -0.2655, 3.0347, -7.9730, -23.0495],
        "b",
    );
    close(
        find(&s.items, c_rect).unwrap().ctm,
        [4.0, 0.0, 0.0, 4.0, -100.0, -50.0],
        "c",
    );
    close(
        find(&s.items, e).unwrap().ctm,
        [0.0, 2.0, -2.0, 0.0, 190.0, -50.0],
        "d",
    );
    // Chrome's `getCTM()` on f stops at the nested viewport: [2,0,0,2,10,5].
    close(
        find(&s.items, f).unwrap().ctm,
        tf::mul(
            [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            [2.0, 0.0, 0.0, 2.0, 10.0, 5.0],
        ),
        "f",
    );
    let Kind::Shape(ellipse) = &find(&s.items, e).unwrap().kind else {
        panic!("an ellipse is a shape")
    };
    assert_eq!(ellipse.path.bounds(), Some((40.0, 15.0, 20.0, 20.0)));
}

#[test]
fn lengths_resolve_against_the_nearest_viewport() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "200"), (StyleId::Height, "100")],
        &[(PropId::ViewBox, "0 0 100 50")],
    );
    let line = d.node(
        SvgLine,
        &[],
        &[
            (PropId::X1, "10%"),
            (PropId::Y1, "50%"),
            (PropId::X2, "1in"),
            (PropId::Y2, "40"),
        ],
    );
    let circle = d.node(SvgCircle, &[(StyleId::R, "10%")], &[]);
    d.children(svg, &[line, circle]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    let Kind::Shape(l) = &find(&s.items, line).unwrap().kind else {
        panic!()
    };
    // Chrome: `x1.baseVal.value` 10, `y1` 25; 1in is 96 user units.
    assert_eq!(
        l.path.0,
        vec![
            exact_kernel::svg::Seg::Move(10.0, 25.0),
            exact_kernel::svg::Seg::Line(96.0, 40.0)
        ]
    );
    let Kind::Shape(c) = &find(&s.items, circle).unwrap().kind else {
        panic!()
    };
    // `r` against √((100² + 50²) / 2).
    let r = c.circle.unwrap().2;
    assert!((r - 7.9057).abs() < 1e-3, "{r}");
}

#[test]
fn inheritance_display_and_visibility() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[
            (StyleId::Width, "10"),
            (StyleId::Height, "10"),
            (StyleId::TextColor, "#0000ff"),
        ],
        &[],
    );
    let g = d.node(
        SvgGroup,
        &[
            (StyleId::Fill, "currentcolor"),
            (StyleId::Visibility, "hidden"),
        ],
        &[],
    );
    let r = [(StyleId::Width, "5"), (StyleId::Height, "5")];
    let hidden = d.node(SvgRect, &r, &[]);
    let mut shown_rows = r.to_vec();
    shown_rows.push((StyleId::Visibility, "visible"));
    let shown = d.node(SvgRect, &shown_rows, &[]);
    let mut none_rows = r.to_vec();
    none_rows.push((StyleId::Display, "none"));
    let gone = d.node(SvgRect, &none_rows, &[]);
    d.children(g, &[hidden, shown, gone]);
    d.children(svg, &[g]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    assert!(
        find(&s.items, gone).is_none(),
        "display: none renders nothing"
    );
    let Kind::Shape(h) = &find(&s.items, hidden).unwrap().kind else {
        panic!()
    };
    assert!(h.fill.is_none(), "visibility: hidden paints nothing");
    let Kind::Shape(v) = &find(&s.items, shown).unwrap().kind else {
        panic!()
    };
    // A visible child of a hidden group paints, and `currentcolor` is the
    // inherited `color`.
    let fill = v.fill.clone().unwrap();
    assert_eq!(
        fill.color.resolve(false),
        exact_kernel::Color::parse("#0000ff").unwrap()
    );
}

#[test]
fn a_view_box_is_the_natural_ratio() {
    let mut d = Doc::new();
    let svg = d.node(NodeType::Svg, &[], &[(PropId::ViewBox, "0 0 100 50")]);
    let sized = d.node(
        NodeType::Svg,
        &[(StyleId::Width, "60")],
        &[(PropId::ViewBox, "0 0 100 50")],
    );
    let bare = d.node(NodeType::Svg, &[], &[]);
    let k = d.kernel(&[svg, sized, bare], 400.0);
    let size = |id: u32| {
        let f = k.node(id).unwrap().frame;
        (f.width, f.height)
    };
    // Chrome 154: 400×200 (the offered width), 60×30, and 300×150.
    assert_eq!(size(svg), (400.0, 200.0));
    assert_eq!(size(sized), (60.0, 30.0));
    assert_eq!(size(bare), (300.0, 150.0));
}

/// LLP 1055.000 D15: an animated inherited value on a `g` reaches the
/// shapes that inherit it, and an animated `color` reaches `currentcolor`.
#[test]
fn animated_inherited_values_flow_to_descendants() {
    use exact_motion::{Engine, Keyframes, Property};
    let mut d = Doc::new();
    let svg = d.node(
        NodeType::Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    // The animation row resolved as the runner resolves it.
    let mut rows = vec![
        (StyleId::TextColor, "#0891b2"),
        (StyleId::Fill, "currentcolor"),
        (StyleId::StrokeDasharray, "4 4"),
    ];
    rows.push((
        StyleId::Animation,
        "tint 1s linear -500ms paused, march 1s linear -500ms paused",
    ));
    let g = d.node(NodeType::SvgGroup, &[], &[]);
    let mut patch = StyleProps::default();
    for (row, v) in &rows {
        patch
            .set_dynamic(*row, &StyleValue::Text((*v).into()))
            .unwrap();
    }
    let tint = Keyframes::parse("from{color:#0891b2}to{color:#be185d}").unwrap();
    let march = Keyframes::parse("from{stroke-dashoffset:0}to{stroke-dashoffset:12}").unwrap();
    patch
        .animation
        .resolve(|n| Some(if n == "tint" { &tint } else { &march }));
    d.ops.push(Op::SetStyle {
        id: g,
        patch: Box::new(patch),
    });
    let line = d.node(NodeType::SvgLine, &[], &[(PropId::X2, "40")]);
    d.children(g, &[line]);
    d.children(svg, &[g]);
    let mut ops = std::mem::take(&mut d.ops);
    ops.push(Op::SetChildren {
        id: 1,
        children: vec![svg],
    });
    ops.push(Op::AttachRoot { id: 1 });
    let mut k = Kernel::with_monospace();
    let receipt = k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(400.0, 800.0)).unwrap();
    let mut engine = Engine::new();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let node = k.node(svg).unwrap();
    let content = scene::content_box(&node);
    let s = scene::resolve(&k, &node, content, &|key, p| {
        engine.sampled_value(exact_kernel::motion_node(key), p)
    });
    let Kind::Shape(l) = &find(&s.items, line).unwrap().kind else {
        panic!()
    };
    assert_eq!(l.dash_offset, 6.0, "the g's paused dash offset, inherited");
    // The paused `tint` is half way: `currentcolor` fill follows it.
    let fill = l.fill.clone().unwrap().color.resolve(false);
    let want = exact_motion::color::parse("#0891b2")
        .unwrap()
        .lerp(exact_motion::color::parse("#be185d").unwrap(), 0.5)
        .to_rgba8();
    assert_eq!([fill.r(), fill.g(), fill.b(), fill.a()], want);
    let _ = Property::Color;
}

/// LLP 1055.000 D3, D7, D8: a duplicate id resolves to the candidate
/// nearest the reference; gradient defaults; `use` inherits from itself.
#[test]
fn references_resolve_nearest_and_instances_inherit_from_their_use() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let mut rects = Vec::new();
    let mut groups = Vec::new();
    for color in ["#ff0000", "#0000ff"] {
        let g = d.node(SvgGroup, &[], &[]);
        let defs = d.node(SvgDefs, &[], &[]);
        let grad = d.node(SvgLinearGradient, &[], &[(PropId::Id, "a")]);
        let stop = d.node(SvgStop, &[(StyleId::StopColor, color)], &[]);
        let rect = d.node(
            SvgRect,
            &[
                (StyleId::Width, "10"),
                (StyleId::Height, "10"),
                (StyleId::Fill, "url(#a)"),
            ],
            &[],
        );
        d.children(grad, &[stop]);
        d.children(defs, &[grad]);
        d.children(g, &[defs, rect]);
        groups.push(g);
        rects.push(rect);
    }
    // A two-stop gradient with every default: horizontal on the bbox.
    let defs = d.node(SvgDefs, &[], &[]);
    let grad = d.node(SvgLinearGradient, &[], &[(PropId::Id, "h")]);
    let s0 = d.node(SvgStop, &[(StyleId::StopColor, "#000000")], &[]);
    let s1 = d.node(
        SvgStop,
        &[(StyleId::StopColor, "#ffffff")],
        &[(PropId::Offset, "150%")],
    );
    d.children(grad, &[s0, s1]);
    let symbol = d.node(
        SvgSymbol,
        &[],
        &[(PropId::Id, "icon"), (PropId::ViewBox, "0 0 10 10")],
    );
    let dot = d.node(
        SvgCircle,
        &[
            (StyleId::R, "5"),
            (StyleId::Cx, "5"),
            (StyleId::Cy, "5"),
            (StyleId::Fill, "currentcolor"),
        ],
        &[],
    );
    d.children(symbol, &[dot]);
    d.children(defs, &[grad, symbol]);
    let bar = d.node(
        SvgRect,
        &[
            (StyleId::X, "10"),
            (StyleId::Width, "40"),
            (StyleId::Height, "10"),
            (StyleId::Fill, "url(#h)"),
        ],
        &[],
    );
    let use_ = d.node(
        SvgUse,
        &[
            (StyleId::X, "50"),
            (StyleId::Width, "20"),
            (StyleId::Height, "20"),
            (StyleId::TextColor, "#00ff00"),
        ],
        &[(PropId::Href, "#icon")],
    );
    let mut kids = groups.clone();
    kids.extend([defs, bar, use_]);
    d.children(svg, &kids);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    let fill = |id: u32| match &find(&s.items, id).unwrap().kind {
        Kind::Shape(sh) => sh.fill.clone().unwrap(),
        _ => panic!(),
    };
    let rgb = |c: exact_kernel::ColorValue| {
        let c = c.resolve(false);
        [c.r(), c.g(), c.b()]
    };
    assert_eq!(
        rgb(fill(rects[0]).color),
        [255, 0, 0],
        "the first instance's own"
    );
    assert_eq!(
        rgb(fill(rects[1]).color),
        [0, 0, 255],
        "the second instance's own"
    );
    let server = fill(bar).server.expect("a gradient");
    assert_eq!(
        server.kind,
        exact_kernel::svg::server::ServerKind::Linear {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 0.0
        }
    );
    assert_eq!(
        server.transform,
        [40.0, 0.0, 0.0, 10.0, 10.0, 0.0],
        "the bbox"
    );
    assert_eq!(server.stops[1].offset, 1.0, "clamped");
    // The instance: a viewport 20×20 at x=50 over the symbol's 10×10 view
    // box, its circle filled with the `use`'s colour.
    let Kind::Viewport { rect, children, .. } = &find(&s.items, use_).unwrap().kind else {
        panic!("a symbol instance is a viewport")
    };
    assert_eq!(*rect, (50.0, 0.0, 20.0, 20.0));
    let Kind::Shape(inst) = &children[0].kind else {
        panic!()
    };
    assert_eq!(rgb(inst.fill.clone().unwrap().color), [0, 255, 0]);
    assert!(children[0].uid != dot as u64, "an instance has its own key");
}

/// LLP 1055.000 D10: a `clipPath` resolves to its children's union in the
/// clipped element's user space, `objectBoundingBox` mapped to its box, its
/// own `clip-path` intersected; a missing one does not clip.
#[test]
fn clips_resolve_in_the_element_space() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let clip = d.node(
        SvgClipPath,
        &[(StyleId::ClipPath, "url(#outer)")],
        &[
            (PropId::Id, "c"),
            (PropId::ClipPathUnits, "objectBoundingBox"),
        ],
    );
    let disc = d.node(
        SvgCircle,
        &[
            (StyleId::Cx, "0.5"),
            (StyleId::Cy, "0.5"),
            (StyleId::R, "0.5"),
        ],
        &[],
    );
    d.children(clip, &[disc]);
    let outer = d.node(SvgClipPath, &[], &[(PropId::Id, "outer")]);
    let half = d.node(
        SvgRect,
        &[(StyleId::Width, "50"), (StyleId::Height, "100")],
        &[],
    );
    d.children(outer, &[half]);
    let rect = d.node(
        SvgRect,
        &[
            (StyleId::X, "20"),
            (StyleId::Y, "20"),
            (StyleId::Width, "40"),
            (StyleId::Height, "40"),
            (StyleId::ClipPath, "url(#c)"),
        ],
        &[],
    );
    let loose = d.node(
        SvgRect,
        &[
            (StyleId::Width, "10"),
            (StyleId::Height, "10"),
            (StyleId::ClipPath, "url(#nowhere)"),
        ],
        &[],
    );
    d.children(svg, &[clip, outer, rect, loose]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    let c = find(&s.items, rect)
        .unwrap()
        .clip
        .as_ref()
        .expect("clipped");
    // The unit disc on the 40×40 box at (20, 20).
    assert_eq!(c.shapes[0].path.bounds(), Some((20.0, 20.0, 40.0, 40.0)));
    let then = c.then.as_ref().expect("the clipPath's own clip");
    assert_eq!(then.shapes[0].path.bounds(), Some((0.0, 0.0, 50.0, 100.0)));
    assert!(
        find(&s.items, loose).unwrap().clip.is_none(),
        "a missing clip does not clip"
    );
    assert!(
        find(&s.items, clip).is_none(),
        "a clipPath is never painted"
    );
}

/// LLP 1055.000 D11: SVG text resolves into chunks of runs: a `tspan` with
/// its own `y` starts a chunk, white space collapses, `dx` shifts a run.
#[test]
fn text_resolves_into_chunks_of_runs() {
    use exact_kernel::svg::scene::Kind;
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let text = d.node(
        SvgText,
        &[(StyleId::TextAnchor, "middle"), (StyleId::FontSize, "12")],
        &[(PropId::TextX, "50 60 70"), (PropId::TextY, "10%")],
    );
    let a = d.node(SvgTSpan, &[], &[(PropId::Text, "  Q1   revenue ")]);
    let b = d.node(
        SvgTSpan,
        &[(StyleId::Fill, "#16a34a")],
        &[(PropId::Text, "+4%"), (PropId::TextDx, "2")],
    );
    let c = d.node(
        SvgTSpan,
        &[],
        &[(PropId::Text, "next"), (PropId::TextY, "30")],
    );
    d.children(text, &[a, b, c]);
    d.children(svg, &[text]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    let Kind::Text(t) = &find(&s.items, text).unwrap().kind else {
        panic!("a text item")
    };
    assert_eq!(t.chunks.len(), 2);
    let first = &t.chunks[0];
    assert_eq!(
        (first.x, first.y),
        (Some(50.0), Some(10.0)),
        "a list's first value; 10% of 100"
    );
    assert_eq!(first.anchor, exact_kernel::TextAnchor::Middle);
    let texts: Vec<&str> = first.runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["Q1 revenue ", "+4%"]);
    assert_eq!(first.runs[1].dx, 2.0);
    assert_eq!(
        first.runs[0].style.font_size, 12.0,
        "inherited from the text"
    );
    assert_eq!(
        (t.chunks[1].x, t.chunks[1].y),
        (None, Some(30.0)),
        "continues on x"
    );
}

// LLP 1055.000 D9: markers at a path's vertices, oriented, scaled by the
// stroke width, the reference point on the vertex; content inherits from
// the marker, not the shape.
#[test]
fn markers_sit_on_vertices() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let marker = d.node(
        SvgMarker,
        &[
            (StyleId::Fill, "#ff0000"),
            (StyleId::OverflowX, "hidden"),
            (StyleId::OverflowY, "hidden"),
        ],
        &[
            (PropId::Id, "m"),
            (PropId::MarkerWidth, "4"),
            (PropId::MarkerHeight, "4"),
            (PropId::RefX, "2"),
            (PropId::RefY, "2"),
            (PropId::Orient, "auto"),
        ],
    );
    let dot = d.node(
        SvgRect,
        &[(StyleId::Width, "4"), (StyleId::Height, "4")],
        &[],
    );
    d.children(marker, &[dot]);
    let path = d.node(
        SvgPath,
        &[
            (StyleId::StrokeWidth, "2"),
            (StyleId::Fill, "#0000ff"),
            (StyleId::MarkerStart, "url(#m)"),
            (StyleId::MarkerEnd, "url(#m)"),
        ],
        &[(PropId::D, "M10 10 L50 10 L50 50")],
    );
    d.children(svg, &[marker, path]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    assert_eq!(s.items.len(), 1, "the marker renders only where it is used");
    let Kind::Shape(shape) = &find(&s.items, path).unwrap().kind else {
        panic!("a shape")
    };
    assert_eq!(shape.markers.len(), 2, "start and end, no mid");
    let end = &shape.markers[1];
    let near = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4;
    // The reference point on the last vertex; the marker turned 90° and
    // doubled: its origin lands at (54, 46).
    assert!(near(tf::apply(end.ctm, (2.0, 2.0)), (50.0, 50.0)));
    assert!(near(tf::apply(end.ctm, (0.0, 0.0)), (54.0, 46.0)));
    let Kind::Viewport { children, clip, .. } = &end.kind else {
        panic!("a viewport")
    };
    assert!(*clip, "Contract gives a marker the UA's `overflow: hidden`");
    let Kind::Shape(inner) = &children[0].kind else {
        panic!("the marker's content")
    };
    assert_eq!(
        inner.fill.as_ref().map(|p| p.color),
        Some(exact_kernel::ColorValue::Fixed(exact_kernel::Color(
            0xff00_00ff
        ))),
        "content inherits from the marker"
    );
}

/// LLP 1055.000 D7, D10: a mask's region and content units against the
/// element's box, and a pattern's tile, content and template.
#[test]
fn masks_and_patterns_resolve_against_the_box() {
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let mask = d.node(
        SvgMask,
        &[(StyleId::MaskType, "alpha")],
        &[
            (PropId::Id, "m"),
            (PropId::MaskContentUnits, "objectBoundingBox"),
        ],
    );
    let dot = d.node(
        SvgCircle,
        &[
            (StyleId::Cx, "0.5"),
            (StyleId::Cy, "0.5"),
            (StyleId::R, "0.5"),
        ],
        &[],
    );
    d.children(mask, &[dot]);
    let pat = d.node(
        SvgPattern,
        &[(StyleId::Width, "25%"), (StyleId::Height, "0.5")],
        &[(PropId::Id, "p"), (PropId::PatternTransform, "rotate(45)")],
    );
    let cell = d.node(
        SvgRect,
        &[(StyleId::Width, "3"), (StyleId::Height, "3")],
        &[],
    );
    d.children(pat, &[cell]);
    let child = d.node(SvgPattern, &[], &[(PropId::Id, "q"), (PropId::Href, "#p")]);
    let rect = |d: &mut Doc, extra: (StyleId, &str)| {
        d.node(
            SvgRect,
            &[
                (StyleId::X, "20"),
                (StyleId::Y, "10"),
                (StyleId::Width, "40"),
                (StyleId::Height, "20"),
                extra,
            ],
            &[],
        )
    };
    let masked = rect(&mut d, (StyleId::SvgMask, "url(#m)"));
    let tiled = rect(&mut d, (StyleId::Fill, "url(#p)"));
    let templated = rect(&mut d, (StyleId::Fill, "url(#q)"));
    d.children(svg, &[mask, pat, child, masked, tiled, templated]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    let m = find(&s.items, masked)
        .unwrap()
        .mask
        .as_ref()
        .expect("masked");
    // −10%/−10%/120%/120% of the 40×20 box at (20, 10).
    let r = m.region;
    for (a, b) in [(r.0, 16.0), (r.1, 8.0), (r.2, 48.0), (r.3, 24.0)] {
        assert!((a - b).abs() < 1e-4, "{r:?}");
    }
    assert!(!m.luminance, "mask-type: alpha");
    // Content in box units: the unit disc lands on the box.
    let content = &m.items[0];
    let t = content.transform.expect("content units").matrix;
    close(t, [40.0, 0.0, 0.0, 20.0, 20.0, 10.0], "content units");
    let paint = |id: u32| match &find(&s.items, id).unwrap().kind {
        Kind::Shape(sh) => sh.fill.clone().expect("filled"),
        _ => unreachable!(),
    };
    let p = paint(tiled).pattern.expect("a pattern");
    // 25% and 0.5 of the box, from the box's origin.
    assert_eq!(p.tile, (20.0, 10.0, 10.0, 10.0));
    close(p.transform, tf::rotate(45.0), "patternTransform");
    let q = paint(templated).pattern.expect("the template's tile");
    assert_eq!(q.tile, p.tile, "href inherits the tile and the content");
    assert_eq!(q.items.len(), 1);
    assert!(find(&s.items, mask).is_none() && find(&s.items, pat).is_none());
}

/// LLP 1055.000 D14: a filter's region against the box, primitive inputs by
/// name, and a list whose functions chain after the reference.
#[test]
fn filters_resolve_into_one_chain() {
    use exact_kernel::svg::filter::{Input, Op};
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let filter = d.node(SvgFilter, &[], &[(PropId::Id, "f")]);
    let flood = d.node(
        SvgFe,
        &[(StyleId::FloodOpacity, "0.5")],
        &[(PropId::Fe, "feFlood"), (PropId::Result, "wash")],
    );
    let offset = d.node(
        SvgFe,
        &[],
        &[
            (PropId::Fe, "feOffset"),
            (PropId::In, "SourceAlpha"),
            (PropId::FeDx, "3"),
        ],
    );
    let comp = d.node(
        SvgFe,
        &[],
        &[
            (PropId::Fe, "feComposite"),
            (PropId::In, "wash"),
            (PropId::Operator, "in"),
        ],
    );
    d.children(filter, &[flood, offset, comp]);
    let rect = d.node(
        SvgRect,
        &[
            (StyleId::X, "10"),
            (StyleId::Y, "10"),
            (StyleId::Width, "50"),
            (StyleId::Height, "20"),
            (StyleId::Filter, "url(#f) blur(2px)"),
        ],
        &[],
    );
    d.children(svg, &[filter, rect]);
    let k = d.kernel(&[svg], 400.0);
    let s = resolve(&k, svg);
    let f = find(&s.items, rect)
        .unwrap()
        .filter
        .as_ref()
        .expect("filtered");
    let p = &f.primitives;
    assert_eq!(p.len(), 4, "three primitives and the blur");
    assert!(matches!(p[0].op, Op::Flood(c) if (c[3] - 0.5).abs() < 1e-6));
    assert_eq!(p[1].inputs[0], Input::SourceAlpha);
    assert!(matches!(p[1].op, Op::Offset(dx, _) if dx == 3.0));
    // `in="wash"` names the flood; `in2` is the previous result.
    assert_eq!(p[2].inputs, [Input::Result(0), Input::Result(1)]);
    // The blur's source is the reference's result.
    assert_eq!(p[3].inputs[0], Input::Result(2));
    assert!(p.iter().all(|q| q.linear == (q.op != Op::Blur(2.0, 2.0))));
    // −10%/−10%/120%/120% of the box, grown by the blur's reach.
    assert!(f.region.0 <= 5.0 && f.region.2 >= 60.0, "{:?}", f.region);
}

/// LLP 1055.000 D15, D19: presented geometry moves the shape; blend and
/// isolation reach the item.
#[test]
fn geometry_moves_and_blends_resolve() {
    use exact_motion::{Property, Value};
    use NodeType::*;
    let mut d = Doc::new();
    let svg = d.node(
        Svg,
        &[(StyleId::Width, "100"), (StyleId::Height, "100")],
        &[],
    );
    let dot = d.node(
        SvgCircle,
        &[
            (StyleId::Cx, "10"),
            (StyleId::Cy, "10"),
            (StyleId::R, "5"),
            (StyleId::MixBlendMode, "multiply"),
        ],
        &[],
    );
    let bar = d.node(
        SvgRect,
        &[
            (StyleId::Width, "10"),
            (StyleId::Height, "10"),
            (StyleId::Isolation, "isolate"),
        ],
        &[],
    );
    d.children(svg, &[dot, bar]);
    let k = d.kernel(&[svg], 400.0);
    let node = k.node(svg).unwrap();
    let content = scene::content_box(&node);
    let presented = |_: exact_kernel::NodeKey, p: Property| match p {
        Property::Cx => Some(Value::scalar(40.0)),
        Property::Y => Some(Value::scalar(30.0)),
        _ => None,
    };
    let s = scene::resolve(&k, &node, content, &presented);
    let d_item = find(&s.items, dot).unwrap();
    match &d_item.kind {
        Kind::Shape(sh) => assert_eq!(sh.circle.map(|c| (c.0, c.1)), Some((40.0, 10.0))),
        _ => unreachable!(),
    }
    assert_eq!(d_item.blend, 1, "multiply");
    let b = find(&s.items, bar).unwrap();
    assert!(b.isolate);
    match &b.kind {
        Kind::Shape(sh) => assert_eq!(sh.path.bounds().map(|r| r.1), Some(30.0)),
        _ => unreachable!(),
    }
}

/// LLP 1055.000 D15: a path's `d` under `transition` moves through the
/// engine and the scene draws the path between (the issue #123 chevron);
/// a pair with other commands changes at once, as Chrome shows it.
#[test]
fn a_transitioning_d_draws_the_path_between() {
    use exact_kernel::svg::Seg;
    use exact_motion::{Engine, PathValue, Property, Value};
    struct Shown<'e>(&'e Engine);
    impl scene::Present for Shown<'_> {
        fn value(&self, key: exact_kernel::NodeKey, p: Property) -> Option<Value> {
            self.0.sampled_value(exact_kernel::motion_node(key), p)
        }
        fn path(&self, key: exact_kernel::NodeKey) -> Option<PathValue> {
            self.0.presented_path(exact_kernel::motion_node(key))
        }
    }
    let mut d = Doc::new();
    let svg = d.node(
        NodeType::Svg,
        &[(StyleId::Width, "24"), (StyleId::Height, "24")],
        &[(PropId::ViewBox, "0 0 24 24")],
    );
    let chev = d.node(
        NodeType::SvgPath,
        &[(StyleId::Transition, "d 400ms linear")],
        &[(PropId::D, "M6 9 L12 15 L18 9")],
    );
    d.children(svg, &[chev]);
    d.children(1, &[svg]);
    d.ops.push(Op::AttachRoot { id: 1 });
    let mut k = Kernel::with_monospace();
    let receipt = k.apply(0, 1, &d.ops).unwrap();
    k.compute_layout(1, Offer::definite(100.0, 100.0)).unwrap();
    let mut engine = Engine::new();
    k.motion_sync(&receipt).apply(&mut engine).unwrap();
    let ys = |k: &Kernel, engine: &Engine| -> Vec<f32> {
        let node = k.node(svg).unwrap();
        let s = scene::resolve(k, &node, scene::content_box(&node), &Shown(engine));
        let Kind::Shape(shape) = &find(&s.items, chev).unwrap().kind else {
            panic!("a shape")
        };
        shape
            .path
            .0
            .iter()
            .map(|seg| match seg {
                Seg::Move(_, y) | Seg::Line(_, y) => *y,
                _ => f32::NAN,
            })
            .collect()
    };
    let flip = |k: &mut Kernel, engine: &mut Engine, epoch, d: &str| {
        let receipt = k.apply(0, epoch, &[prop(chev, PropId::D, d)]).unwrap();
        k.motion_sync(&receipt).apply(engine).unwrap();
    };
    assert_eq!(ys(&k, &engine), [9.0, 15.0, 9.0]);
    flip(&mut k, &mut engine, 2, "M6 15 L12 9 L18 15");
    assert_eq!(ys(&k, &engine), [9.0, 15.0, 9.0], "the start, at 0 ms");
    engine.advance(0.2).unwrap();
    assert_eq!(ys(&k, &engine), [12.0, 12.0, 12.0], "flat at 200 ms");
    engine.advance(0.4).unwrap();
    assert_eq!(ys(&k, &engine), [15.0, 9.0, 15.0], "settled on the row");
    // M L L against M Q: not interpolable, so at once.
    flip(&mut k, &mut engine, 3, "M4 12 Q12 2 20 12");
    assert_eq!(
        engine.presented_path(exact_kernel::motion_node(k.node(chev).unwrap().key)),
        None
    );
    assert!(engine.quiescent());
}
