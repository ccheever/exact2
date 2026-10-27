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
//! Rows a presenter cannot use yet are named, not guessed. Beside the rows,
//! `"written"` names the declarations the author wrote on this node (LLP
//! 1115.000 D2): a presenter reads it where the platform's answer stands for
//! what the author left unsaid, and an inherited value is never in it.

use crate::gradient::gradient_json;
use exact_kernel::style::ColorValue;
use exact_kernel::{
    Dimension, Env, NodeRef, NodeType, Overflow, PositionArea, PropId, PropValue, RowValue,
    StyleId, StyleMask, StyleProps, StyleValue,
};
use exact_motion::Property;
use std::fmt::Write as _;

mod axes;
#[cfg(test)]
mod crossing_tests;
#[cfg(test)]
mod flow_tests;
mod kept;
#[cfg(test)]
mod negative_rows;
use kept::kept_geometry;

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
    style_json_sized(style, env, StyleMask::EMPTY)
}

/// Layout rows no Apple presenter reads: the kernel sized and placed the
/// box, and the frame op carries the result. Leaving them out is fewer
/// bytes to write and read per node, and the dictionaries of a template's
/// instances that differ only in size (a waveform's bars) come out equal.
/// `width` and `height` still cross for a `video` (`keep_size`): its view
/// asks whether its box waits on its metadata (`HeavyLeaves.created`).
/// `box_sizing` crosses: a projected segmented control fills its tablist's
/// content box, and a border-box minimum must also hold the padding and
/// border around it (LLP 1059 D2a).
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
            | "grid_auto_flow"
            | "grid_template_columns"
            | "grid_template_rows"
            | "grid_column"
            | "grid_row"
            | "justify_items"
            | "field_sizing"
            | "text_transform"
    )
}

/// [`style_json`], with the rows in `keep` sent though the presenter
/// otherwise reads them from the frame (`width`, `height`, a gap).
pub fn style_json_sized(style: &StyleProps, env: &Env, keep: StyleMask) -> (String, Vec<Skipped>) {
    // An undefined segment length is its row's initial value (LLP 1078 D3).
    let resolved = style.env_resolved(env);
    style_json_rows(&resolved, Some(env), keep)
}

/// Native button face geometry was resolved by the kernel against its live
/// environment. Serialise the same points for fitting and presentation.
pub(crate) fn style_json_resolved(style: &StyleProps) -> (String, Vec<Skipped>) {
    style_json_rows(style, None, StyleMask::EMPTY)
}

pub(crate) fn style_json_rows(
    style: &StyleProps,
    env: Option<&Env>,
    keep: StyleMask,
) -> (String, Vec<Skipped>) {
    // Written in place: a list row's mount builds one of these per node,
    // and a `String` per value (`format!`) was most of its cost.
    let mut out = String::with_capacity(256);
    out.push('{');
    let mut skipped = Vec::new();
    let mut first = true;
    // Colour rows that name a system colour (LLP 1077 D13), by row.
    let mut systems: Vec<(&str, &str)> = Vec::new();
    // @ref LLP 1115.000 D2 — the declarations the author wrote here.
    let mut written = String::new();
    for id in style.mask.union(style.written).iter() {
        let name = id.name();
        // LLP 1043.000 M3: the presenter will use resolved shapes.
        if matches!(id, StyleId::WrapFlow | StyleId::ShapeMargin) {
            continue;
        }
        let size = matches!(name, "width" | "height");
        if matches!(
            name,
            "translate" | "scale" | "rotate" | "opacity" | "animation"
        ) || !keep.has(id) && (presenter_ignores(name) || size)
        {
            // A size is layout's and an animation the engine's; that its
            // author wrote one is still said (a glass the author animates
            // in does not also materialize).
            if (size || name == "animation") && style.written.has(id) {
                let comma = if written.is_empty() { "" } else { "," };
                let _ = write!(written, "{comma}\"{name}\"");
            }
            continue;
        }
        if !style.mask.has(id) {
            let comma = if written.is_empty() { "" } else { "," };
            let _ = write!(written, "{comma}\"{name}\"");
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
            RowValue::PositionTryFallbacks(v) => {
                let _ = write!(out, "\"{}\"", v.css());
                true
            }
            // Sent as laid out: clamped at zero on a row with no negative
            // values.
            RowValue::Dimension(d) => {
                let d = match env.map_or(d, |e| d.resolve(e)) {
                    Dimension::Points(v) | Dimension::Percent(v)
                        if v < 0.0 && exact_kernel::style::calc::nonnegative_row(id) =>
                    {
                        Dimension::Points(0.0)
                    }
                    d => d,
                };
                push_length(&mut out, d, env);
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
            // A role WebKit names is also in `system_colors`, for vibrancy (LLP 1077 D13).
            RowValue::ColorValue(c) => {
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
            // `backdrop-filter`: ordered typed operations, never reparsed by Swift.
            RowValue::BackdropFilter(list) if list.is_none() => false,
            RowValue::BackdropFilter(list) => {
                out.push('[');
                for (i, op) in list.0.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    let (name, n) = match op {
                        exact_kernel::style::BackdropOp::Blur(n) => ("blur", *n),
                        exact_kernel::style::BackdropOp::Saturate(n) => ("saturate", *n),
                    };
                    out.push_str(&format!("{{\"{name}\":"));
                    push_num(&mut out, n);
                    out.push('}');
                }
                out.push(']');
                true
            }
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
            RowValue::FontVariations(v) => axes::font_axes_json(&mut out, v),
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
                if id == StyleId::ZIndex {
                    push_int(&mut out, n as i64);
                } else {
                    push_num(&mut out, n as f32);
                }
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
            // Its colours resolve per appearance (LLP 1095 D5): `p` light,
            // and `pd` dark when that differs, the view picking by its own.
            RowValue::Filter(list) => {
                let program = |dark| {
                    exact_kernel::svg::scene::box_filter(list, style.text_color, dark)
                        .map(|f| f.encode())
                };
                match exact_kernel::svg::scene::box_filter(list, style.text_color, false) {
                    Some(f) => {
                        let light = f.encode();
                        let push = |out: &mut String, key: &str, p: &[f32]| {
                            out.push_str(key);
                            out.push('[');
                            for (i, v) in p.iter().enumerate() {
                                if i > 0 {
                                    out.push(',');
                                }
                                push_num(out, *v);
                            }
                            out.push(']');
                        };
                        push(&mut out, "{\"p\":", &light);
                        if let Some(dark) = program(true).filter(|d| *d != light) {
                            push(&mut out, ",\"pd\":", &dark);
                        }
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
            if style.written.has(id) {
                let comma = if written.is_empty() { "" } else { "," };
                let _ = write!(written, "{comma}\"{name}\"");
            }
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
        first = false;
    }
    if !written.is_empty() {
        if !first {
            out.push(',');
        }
        let _ = write!(out, "\"written\":[{written}]");
    }
    out.push('}');
    (out, skipped)
}

/// A length row: a calculation tree with its handle and environment, which
/// Swift resolves against its basis by the kernel's evaluator
/// (`exact_calc_resolve`, LLP 1116 D5); anything else as [`push_dimension`]
/// writes it.
fn push_length(out: &mut String, d: Dimension, env: Option<&exact_kernel::Env>) {
    let Dimension::Compare(c) = d else {
        return push_dimension(out, d);
    };
    // A native button's face arrives resolved without its environment: its
    // terms read the default one
    let fallback = exact_kernel::Env::default();
    crate::calc::write(out, c, env.unwrap_or(&fallback));
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
        Dimension::Env(..)
        | Dimension::Region(..)
        | Dimension::Segment(..)
        | Dimension::Viewport(..)
        | Dimension::Compare(..) => {
            unreachable!("resolved, and a calculation written by push_length")
        }
    }
}

/// `[r,g,b,a]`, the channels as integers.
/// A colour row's value as the presenters read it: four channels, or a
/// `light-dark()` pair of them (LLP 1034 D1).
/// A colour row's wire form: four channels, a `light-dark()` pair of them,
/// or a reference (LLP 1095 D1) as `{"sys": <name>, "c": <pair>}`: this
/// platform's class colour property (or `@tint`, `named:<Asset>`), which the
/// presenter resolves per view against its traits, and the fallback pair.
/// A `-exact-platform-color()` with no name for this platform crosses as its fallback.
pub(crate) fn push_color_value(out: &mut String, c: ColorValue) {
    // @ref LLP 1095 D4 — a role the theme replaces crosses as what replaces it.
    let c = c.themed();
    // LLP 1100 D3: no sRGB clip rides along; nothing here converts it.
    if let ColorValue::Profiled(id) = c {
        if let Some(p) = exact_kernel::style::profiled::profiled(id) {
            push_profiled(out, &p, 1.0);
            return;
        }
    }
    if let Some((v, alpha)) = c.moving_linear() {
        let _ = write!(
            out,
            "{{\"cs\":[{{\"s\":\"srgb-linear\",\"v\":[{},{},{},{}]}}],\"c\":",
            v[0], v[1], v[2], alpha
        );
        push_color_value(out, c.fallback());
        out.push('}');
        return;
    }
    if let ColorValue::Wide(id) = c {
        if let Some(w) = exact_kernel::style::wide::wide(id) {
            out.push_str("{\"cs\":[");
            push_wide(out, w.light);
            if let Some(d) = w.dark {
                out.push(',');
                push_wide(out, d);
            }
            out.push_str("],\"c\":");
            push_color_value(out, c.fallback());
            out.push('}');
            return;
        }
    }
    if let Some(name) = native_name(c) {
        out.push_str("{\"sys\":\"");
        out.push_str(&name);
        out.push_str("\",\"c\":");
        push_color_value(out, c.fallback());
        out.push('}');
        return;
    }
    match c.fallback() {
        ColorValue::Fixed(c) => push_rgba(out, [c.r(), c.g(), c.b(), c.a()]),
        ColorValue::LightDark(l, d) => {
            out.push('[');
            push_rgba(out, [l.r(), l.g(), l.b(), l.a()]);
            out.push(',');
            push_rgba(out, [d.r(), d.g(), d.b(), d.a()]);
            out.push(']');
        }
        ColorValue::Role(_)
        | ColorValue::Platform(_)
        | ColorValue::Wide(_)
        | ColorValue::Moving(..)
        | ColorValue::Profiled(_) => push_rgba(out, [0; 4]),
    }
}

/// The platform colour `c` names here (`{"sys": …}` on the wire): a role's, or
/// a `-exact-platform-color()`'s where it has a name for this platform.
fn native_name(c: ColorValue) -> Option<std::borrow::Cow<'static, str>> {
    match c {
        // An id past the table names no role: its transparent fallback.
        ColorValue::Role(id) => exact_kernel::style::roles::role_of(id).map(|r| {
            std::borrow::Cow::Borrowed(if cfg!(target_os = "macos") {
                r.macos
            } else {
                r.ios
            })
        }),
        ColorValue::Platform(id) => exact_kernel::style::roles::platform(id).and_then(|p| {
            let name = if cfg!(target_os = "macos") {
                p.macos.clone()
            } else {
                p.ios.clone()
            };
            name.map(|n| std::borrow::Cow::Owned(n.into_string()))
        }),
        _ => None,
    }
}

/// Whether `c` is a colour its author fixed: any that names no system
/// colour (the app's own asset, `named:`, is the author's), as the presenter
/// tells one from its wire form (`fixesInk`). A surface that crosses its
/// colours already resolved says so beside them.
/// @ref LLP 1115.000 D7.9
pub(crate) fn fixes_ink(c: ColorValue) -> bool {
    native_name(c).is_none_or(|name| name.starts_with("named:"))
}

/// A profile's colour as `{"cs": [{"s": "cg:<name>" | "icc:<asset>", "i":
/// <intent>, "v": [components…, alpha]}]}` (LLP 1100 D3).
pub(crate) fn push_profiled(
    out: &mut String,
    p: &exact_kernel::style::profiled::ProfiledValue,
    opacity: f32,
) {
    use exact_kernel::style::profiled::Source;
    let (space, intent) = match &p.source {
        Source::Named(name) => (format!("cg:{name}"), "relative-colorimetric"),
        Source::Icc { src, intent } => (format!("icc:{src}"), &**intent),
    };
    let values: Vec<String> = p
        .components
        .iter()
        .chain([&(p.alpha * opacity)])
        .map(|v| v.to_string())
        .collect();
    let _ = write!(
        out,
        "{{\"cs\":[{{\"s\":{space:?},\"i\":{intent:?},\"v\":[{}]}}]}}",
        values.join(",")
    );
}

/// One wide colour as `{"s": <space>, "v": [c0, c1, c2, alpha]}`. Core
/// Graphics names sRGB and Display P3; any other space crosses as extended
/// linear sRGB, unclipped (LLP 1100 D2).
pub(crate) fn push_wide(out: &mut String, w: exact_color::Wide) {
    use exact_color::Space;
    let (space, v) = match w.space {
        Space::Srgb => ("srgb", w.c),
        Space::DisplayP3 => ("display-p3", w.c),
        _ => ("srgb-linear", w.linear_srgb()),
    };
    let _ = write!(
        out,
        "{{\"s\":\"{space}\",\"v\":[{},{},{},{}]}}",
        v[0], v[1], v[2], w.alpha
    );
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
/// row says otherwise; both axes compute through CSS Overflow §3. The presenter scrolls and clips from these, never from the node
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
    let x = if s.mask.has(StyleId::OverflowX) {
        s.overflow_x
    } else {
        Overflow::Visible
    };
    (x.computed(y), y.computed(x))
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
    // LLP 1100 D2: Oklab motion keeps its gamut frame by frame.
    if v.oklab {
        let (linear, alpha) = v.linear_srgb();
        return ColorValue::moving(linear, alpha);
    }
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

/// Whether a `color` moving on the node is its author's: its keyframes
/// name it, or it is a text field, which inherits none (see
/// [`style_json_presented`]).
fn says_moving_color(node: &NodeRef<'_>) -> bool {
    node.node_type == NodeType::TextInput
        || node.style.animation.properties().contains(&Property::Color)
}

/// A leaving view's last style with its presented paint over it: its node
/// is gone, so the rows are the ones it last showed (LLP 1063 D5). `own`:
/// the exit's keyframes name `color`, so a moving one is its author's and
/// `"written"` names it, as [`style_json_presented`] does.
pub fn restyle_presented(last: &str, env: &Env, shown: &Shown, own: bool) -> String {
    let mut paint = StyleProps::default();
    paint_over(&mut paint, shown);
    for (p, id) in [
        (Property::BorderTopColor, StyleId::BorderColorTop),
        (Property::BorderRightColor, StyleId::BorderColorRight),
        (Property::BorderBottomColor, StyleId::BorderColorBottom),
        (Property::BorderLeftColor, StyleId::BorderColorLeft),
    ] {
        if let Some(c) = shown.get(p) {
            let side = match p {
                Property::BorderTopColor => &mut paint.border_color_top,
                Property::BorderRightColor => &mut paint.border_color_right,
                Property::BorderBottomColor => &mut paint.border_color_bottom,
                _ => &mut paint.border_color_left,
            };
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
    let mut rows: Vec<String> = kept.chain(over).collect();
    if own && shown.get(Property::Color).is_some() {
        match rows.iter_mut().find(|e| e.starts_with("\"written\":[")) {
            Some(w) if !w.contains("\"text_color\"") => {
                w.insert_str("\"written\":[".len(), "\"text_color\",");
            }
            Some(_) => {}
            None => rows.push("\"written\":[\"text_color\"]".into()),
        }
    }
    format!("{{{}}}", rows.join(","))
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
/// `shown` is the node's own motion. A moving `color` is its author's where
/// its keyframes name `color`, or on a text field, whose colour stops at it
/// (a platform start, LLP 1104 D4): `"written"` names it then, as a field
/// reads its colour only where the node says one (LLP 1115 D4). A
/// transition over an inherited colour is not the node's say.
pub fn style_json_presented(
    node: &NodeRef<'_>,
    env: &Env,
    shown: &Shown,
) -> (String, Vec<Skipped>) {
    style_json_shown(node, env, shown, true)
}

/// [`style_json_presented`] with `shown` an ancestor's moving `color`,
/// which the node inherits: not its own, so not written.
pub fn style_json_inherited(
    node: &NodeRef<'_>,
    env: &Env,
    shown: &Shown,
) -> (String, Vec<Skipped>) {
    style_json_shown(node, env, shown, false)
}

fn style_json_shown(
    node: &NodeRef<'_>,
    env: &Env,
    shown: &Shown,
    own: bool,
) -> (String, Vec<Skipped>) {
    let rows = if matches!(
        node.node_type,
        NodeType::Text
            | NodeType::TextInput
            | NodeType::Image
            | NodeType::Control
            | NodeType::Video
    ) {
        StyleMask::INHERITED
    } else {
        // `pointer-events` is inherited too: a box under a `none` parent
        // passes the pointer through wherever it paints, translated out of
        // its parent's box included (feed's toast, x2apps repro
        // pointer-events-inherit-translate). `visibility` too: a hidden
        // ancestor keeps a descendant's own `visible`, and one that does
        // not set it inherits `hidden`. The initial `visible` stays
        // unmarked and is not sent. So is `dynamic-range-limit`
        // (LLP 1100 D8).
        StyleMask::of(StyleId::TextColor)
            .union(StyleMask::of(StyleId::Direction))
            .union(StyleMask::of(StyleId::Cursor))
            .union(StyleMask::of(StyleId::PointerEvents))
            .union(StyleMask::of(StyleId::Visibility))
            .union(StyleMask::of(StyleId::DynamicRangeLimit))
            // A subtree's scheme reaches every view below the node that
            // sets it, so one lifted out of it (a popover, a dialog) keeps
            // it (LLP 1034 §8).
            .union(StyleMask::of(StyleId::ColorScheme))
    };
    let mut computed = node.computed_style(rows.union(node.style.written));
    // PlaceholderColor's initial payload is an unread sentinel; absence
    // selects the platform placeholder colour, including after a clear. So
    // for the pressed paint: a cleared row is no pressed look (LLP 1061 D7).
    computed.mask =
        computed.mask.union(computed.written.minus(
            StyleMask::of(StyleId::PlaceholderColor).union(crate::looks::press_paint_rows()),
        ));
    computed.mask.set(StyleId::TextColor);
    paint_over(&mut computed, shown);
    if own && shown.get(Property::Color).is_some() && says_moving_color(node) {
        computed.written.set(StyleId::TextColor);
    }
    // The presenter must inset editors/images and paint the same border area
    // that the kernel laid out. Authored widths survive separately in the node.
    let widths = computed.border_widths_in(env);
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
    let (mut json, skipped) = style_json_sized(&computed, env, kept_geometry(node));
    // A modal dialog is placed in the viewport by AppKit: its insets cross. A popover (its area,
    // or the web's default: `PositionArea.painted`) is placed by the host against its invoker by
    // its margin box: its margins cross (a dialog used as one, both), `auto` and % read as 0.
    let placed = [
        StyleId::MarginTop,
        StyleId::MarginRight,
        StyleId::MarginBottom,
        StyleId::MarginLeft,
    ];
    let insets = [StyleId::Top, StyleId::Right, StyleId::Bottom, StyleId::Left];
    // A sticky box's vertical insets say whether it sticks while its
    // scroller scrolls: an iOS header's search field stays in view.
    let vertical = [StyleId::Top, StyleId::Bottom];
    let both = [insets, placed].concat();
    let placed_box = computed.position_area != PositionArea::None
        || node.props.str(exact_kernel::PropId::Popover).is_some();
    let crossing: Option<&[StyleId]> =
        if node.props.str(exact_kernel::PropId::SemanticTag) == Some("dialog") {
            Some(if placed_box { &both } else { &insets })
        } else if placed_box {
            Some(&placed)
        } else if node.layout_position() == exact_kernel::PositionType::Sticky {
            Some(&vertical)
        } else {
            None
        };
    if let Some(rows) = crossing {
        for &id in rows {
            if !computed.mask.has(id) {
                continue;
            }
            if let RowValue::Dimension(d) = computed.get(id) {
                let mut value = String::new();
                push_length(&mut value, d.resolve(env), Some(env));
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
        Overflow::Clip => "clip",
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
    json = crate::looks::with_looks(json, node, env, current);
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
pub fn glass_auto_props(
    node: &NodeRef<'_>,
    out: &mut exact_kernel::SortedMap<&'static str, String>,
) {
    if let Some(points) = glass_auto_spacing(node) {
        out.insert(PropId::GlassGroup.name(), num(points));
        out.insert("glassGroupAuto", "true".to_string());
    }
}

/// Shortest exact decimal for a number: `24`, not `24.0`; `0.5`.
pub fn num(n: f32) -> String {
    let mut s = String::new();
    push_num(&mut s, n);
    s
}
