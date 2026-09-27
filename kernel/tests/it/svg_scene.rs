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
    let fill = v.fill.unwrap();
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
