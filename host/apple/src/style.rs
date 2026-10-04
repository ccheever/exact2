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
    Dimension, Env, NodeRef, NodeType, Overflow, PropId, PropValue, RowValue, StyleId, StyleMask,
    StyleProps, StyleValue,
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
    style_json_sized(style, env, false)
}

/// Layout rows no Apple presenter reads: the kernel sized and placed the
/// box, and the frame op carries the result. Leaving them out is fewer
/// bytes to write and read per node, and the dictionaries of a template's
/// instances that differ only in size (a waveform's bars) come out equal.
/// `width` and `height` still cross for a `video` (`keep_size`): its view
/// asks whether its box waits on its metadata (`HeavyLeaves.created`).
fn presenter_ignores(name: &str) -> bool {
    matches!(
        name,
        "min_width"
            | "min_height"
            | "max_width"
            | "max_height"
            | "margin_top"
            | "margin_right"
            | "margin_bottom"
            | "margin_left"
            | "flex_direction"
            | "flex_wrap"
            | "justify_content"
            | "align_items"
            | "align_self"
            | "align_content"
            | "flex_grow"
            | "flex_shrink"
            | "flex_basis"
            | "top"
            | "right"
            | "bottom"
            | "left"
            | "row_gap"
            | "column_gap"
            | "box_sizing"
            | "grid_auto_flow"
            | "grid_template_columns"
            | "grid_template_rows"
            | "grid_column"
            | "grid_row"
            | "justify_items"
            | "field_sizing"
            | "text_transform"
            | "border_style_top"
            | "border_style_right"
            | "border_style_bottom"
            | "border_style_left"
    )
}

/// [`style_json`], with `width` and `height` kept when `keep_size`.
pub fn style_json_sized(style: &StyleProps, env: &Env, keep_size: bool) -> (String, Vec<Skipped>) {
    // An undefined segment length is its row's initial value (LLP 1078 D3).
    let resolved = style.env_resolved(env);
    let style = &*resolved;
    // Written in place: a list row's mount builds one of these per node,
    // and a `String` per value (`format!`) was most of its cost.
    let mut out = String::with_capacity(256);
    out.push('{');
    let mut skipped = Vec::new();
    let mut first = true;
    // Colour rows that name a system colour (LLP 1077 D13), by row.
    let mut systems: Vec<(&str, &str)> = Vec::new();
    for id in style.mask.iter() {
        let name = id.name();
        // LLP 1043.000 M3: the presenter will use resolved shapes.
        if matches!(id, StyleId::WrapFlow | StyleId::ShapeMargin) {
            continue;
        }
        if matches!(name, "translate" | "scale" | "rotate" | "opacity") || presenter_ignores(name) {
            continue;
        }
        if !keep_size && matches!(name, "width" | "height") {
            continue;
        }
        let mark = out.len();
        if !first {
            out.push(',');
        }
        out.push('"');
        out.push_str(name);
        out.push_str("\":");
        let wrote = match style.get(id) {
            RowValue::ShapeOutside(_) => false, // LLP 1043.000 M3
            // Layout only (LLP 1053 G1): the kernel sizes the box.
            RowValue::AspectRatio(_) => false,
            RowValue::Dimension(d) => {
                push_dimension(&mut out, d.resolve(env));
                true
            }
            // @ref LLP 1061 D6 — `[x, y]`, each points or `{"pct": n}`.
            RowValue::TransformOrigin(o) => {
                out.push('[');
                push_dimension(&mut out, o.x);
                out.push(',');
                push_dimension(&mut out, o.y);
                out.push(']');
                true
            }
            RowValue::Color(c) => {
                push_rgba(&mut out, [c.r(), c.g(), c.b(), c.a()]);
                true
            }
            // A colour a row holds (LLP 1034 D1/D2). A fixed one crosses as
            // the four channels it always did; a `light-dark()` pair crosses
            // as both, because the presenter resolves it against the
            // *owning view's* appearance and must re-resolve when that
            // changes. Flattening here would be the kernel choosing, which
            // is exactly what D2 forbids.
            RowValue::ColorValue(ColorValue::Fixed(c)) => {
                push_rgba(&mut out, [c.r(), c.g(), c.b(), c.a()]);
                true
            }
            RowValue::ColorValue(ColorValue::LightDark(l, d)) => {
                out.push('[');
                push_rgba(&mut out, [l.r(), l.g(), l.b(), l.a()]);
                out.push(',');
                push_rgba(&mut out, [d.r(), d.g(), d.b(), d.a()]);
                out.push(']');
                true
            }
            // Its pair, as any colour; `system_colors` names it.
            RowValue::ColorValue(c @ ColorValue::System(_)) => {
                push_color_value(&mut out, c);
                systems.extend(c.system_name().map(|s| (name, s)));
                true
            }
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
                let _ = write!(
                    out,
                    "{{\"rule\":\"{rule}\",\"commands\":[{}]}}",
                    commands.join(",")
                );
                true
            }
            // @ref LLP 1077 D2 — a mask is a gradient, as the background's.
            RowValue::MaskImage(g) => match g.gradient() {
                Some(g) => {
                    out.push_str(&gradient_json(g));
                    true
                }
                None => false,
            },
            // @ref LLP 1077 D4 — `[{"o":[x,y],"b":blur,"s":spread,"i":1,"c":colour}]`,
            // the first painted on top; `i` only on an inset one.
            RowValue::BoxShadow(list) if list.0.is_empty() => false,
            RowValue::BoxShadow(list) => {
                out.push('[');
                for (i, s) in list.0.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str("{\"o\":[");
                    push_num(&mut out, s.offset.x);
                    out.push(',');
                    push_num(&mut out, s.offset.y);
                    out.push_str("],\"b\":");
                    push_num(&mut out, s.blur);
                    out.push_str(",\"s\":");
                    push_num(&mut out, s.spread);
                    if s.inset {
                        out.push_str(",\"i\":1");
                    }
                    out.push_str(",\"c\":");
                    push_color_value(&mut out, s.color);
                    out.push('}');
                }
                out.push(']');
                true
            }
            // @ref LLP 1077 D3 — `{"o":[x,y],"b":blur,"c":colour}`; no `c`
            // is currentcolor, the text's own.
            RowValue::TextShadow(t) => match t.shadow() {
                Some(s) => {
                    out.push_str("{\"o\":[");
                    push_num(&mut out, s.offset.x);
                    out.push(',');
                    push_num(&mut out, s.offset.y);
                    out.push_str("],\"b\":");
                    push_num(&mut out, s.blur);
                    if let Some(c) = s.color {
                        out.push_str(",\"c\":");
                        push_color_value(&mut out, c);
                    }
                    out.push('}');
                    true
                }
                None => false,
            },
            // @ref LLP 1077 D10 — the palette's colours, each four channels
            // or a light/dark pair of them.
            RowValue::SymbolPalette(p) if p.0.is_empty() => false,
            RowValue::SymbolPalette(p) => {
                out.push('[');
                for (i, c) in p.0.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    push_color_value(&mut out, *c);
                }
                out.push(']');
                true
            }
            // @ref LLP 1077 D8 — `[x, y, z]`; the z axis (CSS's initial) is
            // no row.
            RowValue::RotateAxis(a) if a.0 == [0.0, 0.0, 1.0] => false,
            RowValue::RotateAxis(a) => {
                out.push('[');
                push_num(&mut out, a.0[0]);
                out.push(',');
                push_num(&mut out, a.0[1]);
                out.push(',');
                push_num(&mut out, a.0[2]);
                out.push(']');
                true
            }
            // @ref LLP 1077 D1 — four corners: K, "inf", "-inf" or "apple".
            // All `round` is no row: the hosts' arcs.
            RowValue::CornerShape(c) if c.is_round() => false,
            RowValue::CornerShape(c) => {
                out.push('[');
                for (i, corner) in c.0.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    match *corner {
                        exact_kernel::corner::Corner::AppleContinuous => out.push_str("\"apple\""),
                        exact_kernel::corner::Corner::Superellipse(k) if k == f32::INFINITY => {
                            out.push_str("\"inf\"")
                        }
                        exact_kernel::corner::Corner::Superellipse(k) if k == f32::NEG_INFINITY => {
                            out.push_str("\"-inf\"")
                        }
                        exact_kernel::corner::Corner::Superellipse(k) => push_num(&mut out, k),
                    }
                }
                out.push(']');
                true
            }
            // One layer is its object; several (LLP 1077 D5) an array of
            // them, the first on top.
            RowValue::BackgroundImage(g) => match g.layers() {
                [] => false, // `none`: nothing to paint
                [one] => {
                    out.push_str(&gradient_json(one));
                    true
                }
                layers => {
                    let parts: Vec<String> = layers.iter().map(gradient_json).collect();
                    out.push('[');
                    out.push_str(&parts.join(","));
                    out.push(']');
                    true
                }
            },
            RowValue::Enum(e) => {
                out.push('"');
                out.push_str(e);
                out.push('"');
                true
            }
            RowValue::Vec2(v) => {
                out.push('[');
                push_num(&mut out, v.x);
                out.push(',');
                push_num(&mut out, v.y);
                out.push(']');
                true
            }
            RowValue::LineHeight(v) => {
                match v {
                    exact_kernel::LineHeight::Number(n) => push_num(&mut out, n),
                    _ => {
                        let _ = write!(out, "\"{}\"", v.css());
                    }
                }
                true
            }
            RowValue::Number(n) => {
                push_num(&mut out, n as f32);
                true
            }
            RowValue::Transitions(_) => false, // the engine's, not the presenter's
            // @ref LLP 1057.003 D2 — drag timelines are the engine's too.
            RowValue::DragTimeline(_)
            | RowValue::AnimationTimeline(_)
            | RowValue::AnimationRange(_)
            | RowValue::TimelineScope(_) => false,
            // @ref LLP 1055 D4/D7 — the `svg` scene and CA specs carry these.
            RowValue::Paint(_)
            | RowValue::DashArray(_)
            | RowValue::Transform(_)
            | RowValue::PaintOrder(_)
            | RowValue::Marker(_)
            | RowValue::Animations(_) => false,
            // @ref LLP 1055.000 D14 — CSS `filter` on a box: its chain over a
            // box of no size (the region is how far past the box it reaches)
            // and how far it reads; the presenter adds the box's size.
            RowValue::Filter(list) => {
                match exact_kernel::svg::scene::box_filter(list, style.text_color.resolve(false)) {
                    Some(f) => {
                        out.push_str("{\"p\":[");
                        for (i, v) in f.encode().iter().enumerate() {
                            if i > 0 {
                                out.push(',');
                            }
                            push_num(&mut out, *v);
                        }
                        out.push(']');
                        if let Some((units, pixels)) = f.reach() {
                            out.push_str(",\"rc\":[");
                            push_num(&mut out, units);
                            out.push(',');
                            push_num(&mut out, pixels);
                            out.push(']');
                        }
                        out.push('}');
                        true
                    }
                    None => false,
                }
            }
            RowValue::Color2(_) | RowValue::Tracks(_) | RowValue::Placement(_) => {
                skipped.push(Skipped {
                    row: id,
                    reason: "grid rows are not lowered in v1",
                });
                false
            }
        };
        if wrote {
            first = false;
        } else {
            out.truncate(mark);
        }
    }
    if !systems.is_empty() {
        if !first {
            out.push(',');
        }
        out.push_str("\"system_colors\":{");
        for (i, (row, system)) in systems.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(out, "\"{row}\":\"{system}\"");
        }
        out.push('}');
    }
    out.push('}');
    (out, skipped)
}

fn push_dimension(out: &mut String, d: Dimension) {
    match d {
        Dimension::Auto => out.push_str("\"auto\""),
        Dimension::Points(p) => push_num(out, p),
        Dimension::Percent(p) => {
            out.push_str("{\"pct\":");
            push_num(out, p);
            out.push('}');
        }
        Dimension::Calc(p, x) => {
            out.push_str("{\"pct\":");
            push_num(out, p);
            out.push_str(",\"px\":");
            push_num(out, x);
            out.push('}');
        }
        Dimension::Env(..) | Dimension::Segment(..) | Dimension::Viewport(..) => {
            unreachable!("resolved")
        }
    }
}

/// `[r,g,b,a]`, the channels as integers.
/// A colour row's value as the presenters read it: four channels, or a
/// `light-dark()` pair of them (LLP 1034 D1).
fn push_color_value(out: &mut String, c: ColorValue) {
    match c.pair() {
        ColorValue::Fixed(c) => push_rgba(out, [c.r(), c.g(), c.b(), c.a()]),
        ColorValue::LightDark(l, d) => {
            out.push('[');
            push_rgba(out, [l.r(), l.g(), l.b(), l.a()]);
            out.push(',');
            push_rgba(out, [d.r(), d.g(), d.b(), d.a()]);
            out.push(']');
        }
        ColorValue::System(_) => {} // `pair` never returns one
    }
}

fn push_rgba(out: &mut String, channels: [u8; 4]) {
    out.push('[');
    for (i, c) in channels.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_int(out, i64::from(c));
    }
    out.push(']');
}

pub(crate) fn push_int(out: &mut String, n: i64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = n.unsigned_abs();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    if n < 0 {
        out.push('-');
    }
    out.push_str(std::str::from_utf8(&buf[i..]).expect("digits"));
}

/// A number as `num` spells it, written in place.
pub fn push_num(out: &mut String, n: f32) {
    if n.fract() == 0.0 && n.abs() < 1e9 {
        push_int(out, n as i64);
    } else if (n * 4.0).fract() == 0.0 && n.abs() < 1e7 {
        // Quarters (a 1.5 radius, a half-point frame) are exact in binary,
        // and `{n}` spells them just so: write them without `fmt`.
        let q = (n * 4.0) as i64;
        if q < 0 {
            out.push('-');
        }
        push_int(out, q.abs() / 4);
        out.push_str([".0", ".25", ".5", ".75"][(q.abs() % 4) as usize]);
    } else {
        let _ = write!(out, "{n}");
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
        Overflow::Auto
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
    // non-visible one computes to `auto` (CSS Overflow §3).
    if x == Overflow::Visible && y != Overflow::Visible {
        x = Overflow::Auto;
    } else if y == Overflow::Visible && x != Overflow::Visible {
        y = Overflow::Auto;
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
        computed.background_color = Some(fixed(c));
        computed.mask.set(StyleId::BackgroundColor);
    }
    if let Some(c) = shown.get(Property::TintColor) {
        computed.tint_color = Some(fixed(c));
        computed.mask.set(StyleId::TintColor);
    }
    // @ref LLP 1077 D4 — the engine moves the list's first shadow (one
    // from `none` is CSS's transparent, zero-length one).
    let (geometry, color) = (
        shown.get(Property::BoxShadow),
        shown.get(Property::ShadowColor),
    );
    if geometry.is_some() || color.is_some() {
        let mut list = computed.box_shadow.0.clone();
        if list.is_empty() {
            list.push(exact_kernel::BoxShadow {
                color: ColorValue::Fixed(exact_kernel::Color::TRANSPARENT),
                offset: exact_kernel::Vec2 { x: 0.0, y: 0.0 },
                blur: 0.0,
                spread: 0.0,
                inset: false,
            });
        }
        if let Some(g) = geometry {
            list[0].offset = exact_kernel::Vec2 {
                x: g.x as f32,
                y: g.y as f32,
            };
            list[0].blur = g.z as f32;
        }
        if let Some(c) = color {
            list[0].color = fixed(c);
        }
        computed.box_shadow = exact_kernel::style::BoxShadows(list);
        computed.mask.set(StyleId::BoxShadow);
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
    // A colour the motion now paints is no longer the system colour it was
    // (LLP 1077 D13): its row leaves `system_colors`, the others stay.
    let kept = entries(if last.is_empty() { "{}" } else { last })
        .into_iter()
        .filter(|e| !taken.contains(&key(e)))
        .filter_map(|e| {
            match e
                .strip_prefix("\"system_colors\":{")
                .and_then(|r| r.strip_suffix('}'))
            {
                Some(rows) => {
                    let rows: Vec<_> = rows
                        .split(',')
                        .filter(|r| !taken.contains(&key(r)))
                        .collect();
                    (!rows.is_empty()).then(|| format!("\"system_colors\":{{{}}}", rows.join(",")))
                }
                None => Some(e.to_string()),
            }
        });
    let over = over.into_iter().map(str::to_string);
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
        // `pointer-events` is inherited too: a box under a `none` parent
        // passes the pointer through wherever it paints, translated out of
        // its parent's box included (feed's toast, x2apps repro
        // pointer-events-inherit-translate).
        StyleMask::of(StyleId::TextColor)
            .union(StyleMask::of(StyleId::Direction))
            .union(StyleMask::of(StyleId::Cursor))
            .union(StyleMask::of(StyleId::PointerEvents))
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
    // `currentcolor` (feed F1) is the presented `color`, as a side's is.
    let current = computed.text_color;
    computed.background_color = Some(computed.background_color.unwrap_or(current));
    computed.tint_color = Some(computed.tint_color.unwrap_or(current));
    let (mut json, skipped) = style_json_sized(&computed, env, node.node_type == NodeType::Video);
    // A modal's top layer is positioned in the viewport by AppKit, outside
    // its authored parent. Keep only the existing inset rows for dialogs.
    if node.props.str(exact_kernel::PropId::SemanticTag) == Some("dialog") {
        for id in [StyleId::Top, StyleId::Right, StyleId::Bottom, StyleId::Left] {
            if !computed.mask.has(id) {
                continue;
            }
            if let RowValue::Dimension(d) = computed.get(id) {
                let mut value = String::new();
                push_dimension(&mut value, d.resolve(env));
                let comma = if json == "{}" { "" } else { "," };
                json.pop();
                let _ = write!(json, "{comma}\"{}\":{value}}}", id.name());
            }
        }
    }
    let (x, y) = effective_overflow(node);
    let name = |o: Overflow| match o {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
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
    // @ref LLP 1053.000.000.000 D2 — an auto glass group's spacing rides the
    // string the host compares, so a gap, direction or display change sends it.
    if let Some(points) = glass_auto_spacing(node) {
        let head = format!("{{\"glass_group_spacing\":{}", num(points));
        json = if json == "{}" {
            head + "}"
        } else {
            head + "," + &json[1..]
        };
    }
    (json, skipped)
}

/// `glassGroup="auto"`'s spacing (LLP 1053.000.000.000 D2): the gap along the
/// main axis in points — `column-gap` in a flex row, `row-gap` in a flex
/// column, the smaller of the two in a grid, `0` otherwise — clamped to
/// 0–10,000; `None` unless the prop is the reserved `-1`.
pub fn glass_auto_spacing(node: &NodeRef<'_>) -> Option<f32> {
    use exact_kernel::{Display, FlexDirection};
    if !matches!(node.props.get(PropId::GlassGroup), Some(PropValue::Float(f)) if *f == -1.0) {
        return None;
    }
    let s = node.style;
    let gap = match s.display {
        Display::Flex => match s.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => s.column_gap,
            FlexDirection::Column | FlexDirection::ColumnReverse => s.row_gap,
        },
        Display::Grid => s.row_gap.min(s.column_gap),
        _ => 0.0,
    };
    Some(gap.clamp(0.0, 10_000.0))
}

/// [`glass_auto_spacing`] in a node's props: the points as `glassGroup` and
/// `glassGroupAuto`, so a change to the prop sends them.
pub fn glass_auto_props(node: &NodeRef<'_>, out: &mut std::collections::BTreeMap<String, String>) {
    if let Some(points) = glass_auto_spacing(node) {
        out.insert(PropId::GlassGroup.name().to_string(), num(points));
        out.insert("glassGroupAuto".to_string(), "true".to_string());
    }
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
        // @ref LLP 1077 D5 — `[from, x%, xpx, y%, ypx]`.
        GradientKind::Conic { from, at } => {
            let axis = |l: Length| match l {
                Length::Percent(p) => format!("{},0", num(p)),
                Length::Px(px) => format!("0,{}", num(px)),
            };
            format!("\"conic\":[{},{},{}]", num(from), axis(at[0]), axis(at[1]))
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
    let mut s = String::new();
    push_num(&mut s, n);
    s
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

    #[test]
    fn only_dialogs_keep_viewport_positioning_rows() {
        use exact_kernel::{Kernel, MonospaceMeasurer, Op, PropId};
        let mut kernel = Kernel::new(Box::new(MonospaceMeasurer::default()));
        let mut style = StyleProps::default();
        style
            .set_dynamic(StyleId::Left, &StyleValue::Number(12.0))
            .unwrap();
        style
            .set_dynamic(StyleId::Bottom, &StyleValue::Text("calc(10% + 8px)".into()))
            .unwrap();
        kernel
            .apply(
                0,
                1,
                &[
                    Op::CreateView {
                        id: 1,
                        node_type: NodeType::View,
                    },
                    Op::SetStyle {
                        id: 1,
                        patch: Box::new(style),
                    },
                ],
            )
            .unwrap();
        let ordinary: serde_json::Value =
            serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0)
                .unwrap();
        assert!(ordinary.get("left").is_none());
        assert!(ordinary.get("bottom").is_none());
        kernel
            .apply(
                0,
                2,
                &[Op::SetProp {
                    id: 1,
                    prop: PropId::SemanticTag,
                    value: "dialog".into(),
                }],
            )
            .unwrap();
        let dialog: serde_json::Value =
            serde_json::from_str(&style_json_for(&kernel.node(1).unwrap(), &Env::default()).0)
                .unwrap();
        assert_eq!(dialog["left"], 12);
        assert_eq!(dialog["bottom"], serde_json::json!({ "pct": 10, "px": 8 }));
    }

    /// Quarters are written as `{n}` writes them.
    #[test]
    fn quarters_spell_as_display_does() {
        for n in [
            0.25f32,
            0.5,
            0.75,
            1.5,
            -0.5,
            -2.25,
            123.75,
            9_999_999.5,
            0.1,
            1.0 / 3.0,
            -0.0,
            7.0,
        ] {
            let mut s = String::new();
            push_num(&mut s, n);
            let want = if n.fract() == 0.0 {
                format!("{}", n as i64)
            } else {
                format!("{n}")
            };
            assert_eq!(s, want, "{n}");
        }
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

    #[test]
    fn a_moving_colour_leaves_system_colours_and_the_rest_stay() {
        // LLP 1077 D13: the row the motion paints is no longer a system
        // colour; an untouched one keeps its name.
        let last = r#"{"text_color":[[0,0,0,255],[255,255,255,255]],"background_color":[[120,120,128,51],[120,120,128,92]],"system_colors":{"text_color":"-apple-system-label","background_color":"-apple-system-fill"}}"#;
        let mut shown = Shown::default();
        shown.set(
            Property::BackgroundColor,
            Some(exact_motion::Value::rgba8(255, 0, 0, 128)),
        );
        let json: serde_json::Value =
            serde_json::from_str(&restyle_presented(last, &Env::default(), &shown)).unwrap();
        assert_eq!(
            json["system_colors"],
            serde_json::json!({"text_color": "-apple-system-label"})
        );
        assert_eq!(
            json["background_color"],
            serde_json::json!([255, 0, 0, 128])
        );
        shown.set(
            Property::Color,
            Some(exact_motion::Value::rgba8(0, 0, 255, 255)),
        );
        let json: serde_json::Value =
            serde_json::from_str(&restyle_presented(last, &Env::default(), &shown)).unwrap();
        assert!(json.get("system_colors").is_none(), "{json}");
    }
}
