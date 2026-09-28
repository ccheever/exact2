//! Style rows → a typed dictionary, keyed by row name.
//!
//! @ref LLP 1008 §2
//!
//! Every set row of a node becomes one entry, read through the kernel's
//! generated `StyleProps::get`: dimensions as numbers in points (an `env()`
//! length resolved against the kernel's environment — the presenter sees
//! points, and a change of the insets re-sends the dictionary), `"auto"`, or
//! `{"pct": n}` (`{"pct": n, "px": m}` for a `calc()` of both); colors as
//! `[r,g,b,a]` bytes; enums as their CSS spelling;
//! `vec2` as `[x,y]`; numbers as numbers. The four motion targets
//! (`translate`, `scale`, `rotate`, `opacity`) are left out: a presenter
//! applies their *presentation* values from `present` ops, never the style.
//! Rows a presenter cannot use yet are named, not guessed.

use exact_kernel::style::ColorValue;
use exact_kernel::{
    Dimension, Env, NodeRef, NodeType, Overflow, RowValue, StyleId, StyleMask, StyleProps,
    StyleValue,
};
use exact_motion::Property;
use std::fmt::Write as _;

/// A row this host does not lower (and why).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The row.
    pub row: StyleId,
    /// Why.
    pub reason: &'static str,
}

/// The style dictionary for a node's set rows, as a JSON object, plus what
/// was skipped.
pub fn style_json(style: &StyleProps, env: &Env) -> (String, Vec<Skipped>) {
    let mut out = String::from("{");
    let mut skipped = Vec::new();
    let mut first = true;
    for id in style.mask.iter() {
        let name = id.name();
        // LLP 1043.000 M3: the presenter will use resolved shapes.
        if matches!(id, StyleId::WrapFlow | StyleId::ShapeMargin) {
            continue;
        }
        let value = match style.get(id) {
            RowValue::ShapeOutside(_) => continue, // LLP 1043.000 M3
            // Layout only (LLP 1053 G1): the kernel sizes the box.
            RowValue::AspectRatio(_) => continue,
            RowValue::Dimension(d) => dimension(d.resolve(env)),
            // @ref LLP 1061 D6 — `[x, y]`, each points or `{"pct": n}`.
            RowValue::TransformOrigin(o) => format!("[{},{}]", dimension(o.x), dimension(o.y)),
            RowValue::Color(c) => format!("[{},{},{},{}]", c.r(), c.g(), c.b(), c.a()),
            // A colour a row holds (LLP 1034 D1/D2). A fixed one crosses as
            // the four channels it always did; a `light-dark()` pair crosses
            // as both, because the presenter resolves it against the
            // *owning view's* appearance and must re-resolve when that
            // changes. Flattening here would be the kernel choosing, which
            // is exactly what D2 forbids.
            RowValue::ColorValue(ColorValue::Fixed(c)) => {
                format!("[{},{},{},{}]", c.r(), c.g(), c.b(), c.a())
            }
            RowValue::ColorValue(ColorValue::LightDark(l, d)) => format!(
                "[[{},{},{},{}],[{},{},{},{}]]",
                l.r(),
                l.g(),
                l.b(),
                l.a(),
                d.r(),
                d.g(),
                d.b(),
                d.a()
            ),
            RowValue::ClipPath(p) => {
                let commands: Vec<_> = p
                    .commands()
                    .iter()
                    .map(|(command, values)| {
                        let values = values.iter().map(|n| num(*n)).collect::<Vec<_>>().join(",");
                        format!("[\"{command}\",[{values}]]")
                    })
                    .collect();
                let rule = match p.rule() {
                    exact_kernel::FillRule::Evenodd => "evenodd",
                    exact_kernel::FillRule::Nonzero => "nonzero",
                };
                format!(
                    "{{\"rule\":\"{rule}\",\"commands\":[{}]}}",
                    commands.join(",")
                )
            }
            RowValue::BackgroundImage(g) => match g.gradient() {
                Some(g) => gradient_json(g),
                None => continue, // `none`: nothing to paint
            },
            RowValue::Enum(e) => format!("\"{e}\""),
            RowValue::Vec2(v) => format!("[{},{}]", num(v.x), num(v.y)),
            RowValue::LineHeight(v) => match v {
                exact_kernel::LineHeight::Number(n) => num(n),
                _ => format!("\"{}\"", v.css()),
            },
            RowValue::Number(n) => num(n as f32),
            RowValue::Transitions(_) => continue, // the engine's, not the presenter's
            // @ref LLP 1055 D4/D7 — the `svg` scene and CA specs carry these.
            RowValue::Paint(_)
            | RowValue::DashArray(_)
            | RowValue::Transform(_)
            | RowValue::PaintOrder(_)
            | RowValue::Marker(_)
            | RowValue::Filter(_)
            | RowValue::Animations(_) => continue,
            RowValue::Color2(_) | RowValue::Tracks(_) | RowValue::Placement(_) => {
                skipped.push(Skipped {
                    row: id,
                    reason: "grid rows are not lowered in v1",
                });
                continue;
            }
        };
        if matches!(name, "translate" | "scale" | "rotate" | "opacity") {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "\"{name}\":{value}");
    }
    out.push('}');
    (out, skipped)
}

fn dimension(d: Dimension) -> String {
    match d {
        Dimension::Auto => "\"auto\"".to_string(),
        Dimension::Points(p) => num(p),
        Dimension::Percent(p) => format!("{{\"pct\":{}}}", num(p)),
        Dimension::Calc(p, x) => format!("{{\"pct\":{},\"px\":{}}}", num(p), num(x)),
        Dimension::Env(..) => unreachable!("resolved"),
    }
}

/// A node's effective overflow per axis — the kernel's own rule
/// (`StyleProps::to_taffy`): a `ScrollView`/`List` scrolls on y unless its
/// row says otherwise, and an unset x follows a non-visible y (CSS Overflow
/// §3). The presenter scrolls and clips from these, never from the node
/// type.
pub fn effective_overflow(node: &NodeRef<'_>) -> (Overflow, Overflow) {
    let s = node.style;
    let y = if s.mask.has(StyleId::OverflowY) {
        s.overflow_y
    } else if node.node_type.scrolls_by_default() {
        Overflow::Scroll
    } else {
        Overflow::Visible
    };
    let mut x = if s.mask.has(StyleId::OverflowX) {
        s.overflow_x
    } else {
        Overflow::Visible
    };
    let mut y = y;
    // Symmetric, as the kernel computes: a `visible` axis beside a
    // non-visible one is scrollable (CSS's `auto`; the schema has no `auto`).
    if x == Overflow::Visible && y != Overflow::Visible {
        x = Overflow::Scroll;
    } else if y == Overflow::Visible && x != Overflow::Visible {
        y = Overflow::Scroll;
    }
    (x, y)
}

/// The style dictionary with CSS inheritance resolved and effective
/// overflow: derived values that must reach the presenter even when no row
/// is set. A text node or an editor gets every inherited row's computed
/// value — the font rows, alignment and colour it measures and paints with,
/// as a `<span>` in a `<div>` would — and any other node its computed colour.
/// A row resolved to its initial value stays out (the presenter carries
/// CSS's defaults), except colour, which always crosses. The kernel touches
/// the descendants an inherited change reaches (LLP 1035.000 D4), so this is
/// re-sent by the ordinary update path, never re-derived per frame.
pub fn style_json_for(node: &NodeRef<'_>, env: &Env) -> (String, Vec<Skipped>) {
    style_json_presented(node, env, &Shown::default())
}

/// Presented paint over the rows while it moves (LLP 1055.000 D6, LLP
/// 1062): one slot per [`Property::PAINT`]; `None` shows the row.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shown(pub [Option<exact_motion::Value>; Property::PAINT.len()]);

impl Shown {
    /// The presented value of one paint property.
    pub fn get(&self, property: Property) -> Option<exact_motion::Value> {
        let i = Property::PAINT.iter().position(|p| *p == property)?;
        self.0[i]
    }

    /// Set one paint property's presented value.
    pub fn set(&mut self, property: Property, value: Option<exact_motion::Value>) {
        if let Some(i) = Property::PAINT.iter().position(|p| *p == property) {
            self.0[i] = value;
        }
    }
}

fn fixed(v: exact_motion::Value) -> ColorValue {
    let [r, g, b, a] = v.to_rgba8();
    ColorValue::Fixed(exact_kernel::Color(u32::from_be_bytes([r, g, b, a])))
}

/// The presented colour, background, tint and shadow over `computed`'s rows.
fn paint_over(computed: &mut StyleProps, shown: &Shown) {
    if let Some(c) = shown.get(Property::Color) {
        computed.text_color = fixed(c);
        computed.mask.set(StyleId::TextColor);
    }
    if let Some(c) = shown.get(Property::BackgroundColor) {
        computed.background_color = fixed(c);
        computed.mask.set(StyleId::BackgroundColor);
    }
    if let Some(c) = shown.get(Property::TintColor) {
        computed.tint_color = fixed(c);
        computed.mask.set(StyleId::TintColor);
    }
    // A shadow's opacity is folded into its presented colour's alpha.
    if let Some(g) = shown.get(Property::BoxShadow) {
        computed.shadow_offset = exact_kernel::Vec2 {
            x: g.x as f32,
            y: g.y as f32,
        };
        computed.shadow_radius = g.z as f32;
        computed.mask.set(StyleId::ShadowOffset);
        computed.mask.set(StyleId::ShadowRadius);
    }
    if let Some(c) = shown.get(Property::ShadowColor) {
        computed.shadow_color = fixed(c);
        computed.shadow_opacity = 1.0;
        computed.mask.set(StyleId::ShadowColor);
        computed.mask.set(StyleId::ShadowOpacity);
    }
}

/// A leaving view's last style with its presented paint over it: its node
/// is gone, so the rows are the ones it last showed (LLP 1063 D5).
pub fn restyle_presented(last: &str, env: &Env, shown: &Shown) -> String {
    let mut paint = StyleProps::default();
    paint_over(&mut paint, shown);
    let sides = [
        (
            Property::BorderTopColor,
            StyleId::BorderColorTop,
            &mut paint.border_color_top,
        ),
        (
            Property::BorderRightColor,
            StyleId::BorderColorRight,
            &mut paint.border_color_right,
        ),
        (
            Property::BorderBottomColor,
            StyleId::BorderColorBottom,
            &mut paint.border_color_bottom,
        ),
        (
            Property::BorderLeftColor,
            StyleId::BorderColorLeft,
            &mut paint.border_color_left,
        ),
    ];
    for (p, id, side) in sides {
        if let Some(c) = shown.get(p) {
            *side = Some(fixed(c));
            paint.mask.set(id);
        }
    }
    let over = style_json(&paint, env).0;
    let over = entries(&over);
    let key = |e: &str| e.split_once("\":").map(|(k, _)| k.to_owned());
    let taken: Vec<_> = over.iter().map(|e| key(e)).collect();
    let kept = entries(if last.is_empty() { "{}" } else { last })
        .into_iter()
        .filter(|e| !taken.contains(&key(e)));
    format!("{{{}}}", kept.chain(over).collect::<Vec<_>>().join(","))
}

/// The top-level `"key":value` entries of a JSON object the host wrote.
fn entries(json: &str) -> Vec<&str> {
    let inner = &json[1..json.len() - 1];
    let (mut out, mut start, mut depth) = (Vec::new(), 0, 0);
    let (mut string, mut escaped) = (false, false);
    for (i, c) in inner.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if string => escaped = true,
            '"' => string = !string,
            '[' | '{' if !string => depth += 1,
            ']' | '}' if !string => depth -= 1,
            ',' if !string && depth == 0 => {
                out.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !inner.is_empty() {
        out.push(&inner[start..]);
    }
    out
}

/// [`style_json_for`] with presented paint over the rows while a colour or
/// shadow animation or transition moves them (LLP 1055.000 D6, LLP 1062):
/// `color` (and the `currentcolor` border sides that follow it), the
/// background, the border sides, a symbol's tint, and the shadow.
pub fn style_json_presented(
    node: &NodeRef<'_>,
    env: &Env,
    shown: &Shown,
) -> (String, Vec<Skipped>) {
    let rows = if matches!(
        node.node_type,
        NodeType::Text | NodeType::TextInput | NodeType::Image | NodeType::Control
    ) {
        StyleMask::INHERITED
    } else {
        StyleMask::of(StyleId::TextColor).union(StyleMask::of(StyleId::Direction))
    };
    let mut computed = node.computed_style(rows);
    computed.mask.set(StyleId::TextColor);
    paint_over(&mut computed, shown);
    // The presenter must inset editors/images and paint the same border area
    // that the kernel laid out. Authored widths survive separately in the node.
    let widths = computed.border_widths();
    for (id, width) in [
        StyleId::BorderWidthTop,
        StyleId::BorderWidthRight,
        StyleId::BorderWidthBottom,
        StyleId::BorderWidthLeft,
    ]
    .into_iter()
    .zip(widths)
    {
        if width != 0.0 || computed.mask.has(id) {
            computed
                .set_dynamic(id, &StyleValue::Number(width as f64))
                .unwrap();
        }
    }
    if widths.iter().any(|width| *width > 0.0) {
        let [top, right, bottom, left] = computed.border_colors(computed.text_color);
        let side = |p: Property, c: ColorValue| Some(shown.get(p).map_or(c, fixed));
        computed.border_color_top = side(Property::BorderTopColor, top);
        computed.border_color_right = side(Property::BorderRightColor, right);
        computed.border_color_bottom = side(Property::BorderBottomColor, bottom);
        computed.border_color_left = side(Property::BorderLeftColor, left);
        for id in [
            StyleId::BorderColorTop,
            StyleId::BorderColorRight,
            StyleId::BorderColorBottom,
            StyleId::BorderColorLeft,
        ] {
            computed.mask.set(id);
        }
    }
    let (mut json, skipped) = style_json(&computed, env);
    let (x, y) = effective_overflow(node);
    let name = |o: Overflow| match o {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
    };
    if x != Overflow::Visible || y != Overflow::Visible {
        let head = format!(
            "{{\"overflow_x\":\"{}\",\"overflow_y\":\"{}\"",
            name(x),
            name(y)
        );
        json = if json == "{}" {
            head + "}"
        } else {
            head + "," + &json[1..]
        };
    }
    (json, skipped)
}

/// A gradient for the presenter (LLP 1066): its shape — `linear` degrees,
/// a `to <corner>` as `[right, bottom]`, or `radial` as `[circle, extent,
/// x%, xpx, y%, ypx]` (extent: closest-side, closest-corner, farthest-side,
/// farthest-corner) — and `stops` flat as `t, r, g, b, a`, positions 0–1.
/// The box, so the placement, is the presenter's. Stops are already expanded
/// to mix as CSS's premultiplied ones do (Core Graphics and Core Animation
/// mix unpremultiplied); a `light-dark()` gradient also carries `dark`, and
/// the view picks by its own appearance (LLP 1034 D2).
fn gradient_json(g: &exact_kernel::gradient::Gradient) -> String {
    use exact_kernel::gradient::{premultiplied_ramp, Direction, GradientKind, Length};
    let stops = |dark: bool| {
        let parts: Vec<String> = premultiplied_ramp(&g.resolved(dark))
            .into_iter()
            .map(|(at, c)| format!("{},{},{},{},{}", num(at), c.r(), c.g(), c.b(), c.a()))
            .collect();
        format!("[{}]", parts.join(","))
    };
    let shape = match g.kind {
        GradientKind::Linear(Direction::Angle(deg)) => format!("\"linear\":{}", num(deg)),
        GradientKind::Linear(Direction::Corner { right, bottom }) => {
            format!("\"corner\":[{},{}]", u8::from(right), u8::from(bottom))
        }
        GradientKind::Radial { circle, extent, at } => {
            let axis = |l: Length| match l {
                Length::Percent(p) => format!("{},0", num(p)),
                Length::Px(px) => format!("0,{}", num(px)),
            };
            format!(
                "\"radial\":[{},{},{},{}]",
                u8::from(circle),
                extent as u8,
                axis(at[0]),
                axis(at[1])
            )
        }
    };
    let dark = if g.is_scheme_aware() {
        format!(",\"dark\":{}", stops(true))
    } else {
        String::new()
    };
    format!("{{{shape},\"stops\":{}{dark}}}", stops(false))
}

/// Shortest exact decimal for a number: `24`, not `24.0`; `0.5`.
pub fn num(n: f32) -> String {
    if n.fract() == 0.0 && n.abs() < 1e9 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    #[test]
    fn exclusion_rows_wait_for_resolved_shape_batches() {
        let mut s = StyleProps::default();
        for (id, value) in [
            (StyleId::WrapFlow, "both"),
            (StyleId::ShapeOutside, "circle()"),
            (StyleId::ShapeMargin, "8px"),
        ] {
            s.set_dynamic(id, &StyleValue::Text(value.into())).unwrap();
        }
        assert_eq!(style_json(&s, &Env::default()), ("{}".into(), vec![]));
    }

    /// LLP 1061 D2: the press scale is the presenter's to show, so it crosses
    /// as a number, where the engine's own `scale` never does.
    #[test]
    fn press_scale_crosses_and_the_motion_scale_does_not() {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::PressScale, &StyleValue::Number(0.97))
            .unwrap();
        s.set_dynamic(StyleId::Scale, &StyleValue::Number(2.0))
            .unwrap();
        assert_eq!(
            style_json(&s, &Env::default()),
            (r#"{"press_scale":0.97}"#.into(), vec![])
        );
    }

    /// LLP 1066: a gradient crosses as its shape and ready-to-mix stops; a
    /// `light-dark()` one carries both appearances, and `none` nothing.
    #[test]
    fn a_gradient_crosses_as_shape_and_stops() {
        let json = |css: &str| {
            let mut s = StyleProps::default();
            s.set_dynamic(StyleId::BackgroundImage, &StyleValue::Text(css.into()))
                .unwrap();
            style_json(&s, &Env::default()).0
        };
        assert_eq!(
            json("linear-gradient(to right, transparent, #fff 40%)"),
            r#"{"background_image":{"linear":90,"stops":[0,255,255,255,0,0.4,255,255,255,255,1,255,255,255,255]}}"#
        );
        assert_eq!(
            json("linear-gradient(to top left, light-dark(#000, #fff), #f00)"),
            r#"{"background_image":{"corner":[0,0],"stops":[0,0,0,0,255,1,255,0,0,255],"dark":[0,255,255,255,255,1,255,0,0,255]}}"#
        );
        assert_eq!(
            json("radial-gradient(circle closest-side at 10px 25%, #000, #fff)"),
            r#"{"background_image":{"radial":[1,0,0,10,25,0],"stops":[0,0,0,0,255,1,255,255,255,255]}}"#
        );
        assert_eq!(json("none"), "{}");
    }
}
