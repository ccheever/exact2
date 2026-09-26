//! Style rows → CSS declarations.
//!
//! @ref `rules/RULES.md` §Scope (the web is the standard: a row's CSS name is
//! the row's name with `-` for `_`, and its value the CSS value; the few rows
//! that are not one CSS property each are listed here by hand)
//! @ref LLP 1002 D2 (`transition` as CSS; a spring is the one declared
//! deviation and is not emitted as CSS)
//!
//! Every set row of a node becomes one declaration, read through the
//! kernel's generated `StyleProps::get`, so a row added to `schema.json`
//! reaches the page with no change here unless it needs a unit or a name
//! this table does not know — in which case it is skipped and named in
//! [`Skipped`], never guessed.

use exact_kernel::style::ColorValue;
use exact_kernel::{Color, Dimension, Display, Overflow, RowValue, StyleId, StyleProps};
use exact_motion::{
    Easing, StepPosition, TimingFunction, Transition, TransitionProperty, Transitions,
};
use exact_num::{push_text, Piece, Shortest32};
use std::fmt::Write as _;

/// Rows this host knows it does not lower (and why), so an author sees a
/// reason instead of silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The row.
    pub row: StyleId,
    /// Why.
    pub reason: &'static str,
}

/// The `cssText` for a node's set rows, plus what was skipped.
pub fn css_text(style: &StyleProps, font_names: &[String]) -> (String, Vec<Skipped>) {
    let mut out = String::new();
    let mut skipped = Vec::new();
    let mut shadow: Option<(f32, f32, f32, Color, f32)> = None;
    for id in style.mask.iter() {
        let value = style.get(id);
        match (id, &value) {
            // Rows that compose into one CSS property.
            (StyleId::ShadowOffset, RowValue::Vec2(v)) => {
                let s = shadow.get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0));
                s.0 = v.x;
                s.1 = v.y;
            }
            (StyleId::ShadowRadius, RowValue::Number(n)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .2 = *n as f32
            }
            // A shadow is composed into one `box-shadow` string here rather
            // than emitted as its own declaration, so its colour is resolved
            // rather than handed over: CSS has no way to say "this shadow's
            // colour is scheme-aware" inside a composed value. A pair on a
            // shadow takes its light half (LLP 1034 §5).
            (StyleId::ShadowColor, RowValue::Color(c)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .3 = *c
            }
            (StyleId::ShadowColor, RowValue::ColorValue(v)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .3 = v.resolve(false)
            }
            (StyleId::ShadowOpacity, RowValue::Number(n)) => {
                shadow
                    .get_or_insert((0.0, 0.0, 0.0, Color::TRANSPARENT, 0.0))
                    .4 = *n as f32
            }
            (StyleId::Transition, RowValue::Transitions(t)) => {
                let (text, spring_skipped) = transition_css(t);
                if !text.is_empty() {
                    push_text!(&mut out, "transition:{};", text);
                }
                if spring_skipped {
                    skipped.push(Skipped {
                        row: id,
                        reason: "spring transitions are lowered to keyframes by the host, not to CSS `transition`",
                    });
                }
            }
            (StyleId::FontFamily, RowValue::Number(index)) => {
                if let Some(family) = font_names.get(*index as usize) {
                    let value = if is_generic_family(family) {
                        generic_stack(family).to_string()
                    } else {
                        css_string(family)
                    };
                    push_text!(&mut out, "font-family:{};", value);
                } else {
                    skipped.push(Skipped {
                        row: id,
                        reason: "font stack id is absent from the plan catalog",
                    });
                }
            }
            (StyleId::LineClamp, RowValue::Number(n)) => {
                if *n > 0.0 {
                    // The legacy clamp requires an old flex box and clipping. It
                    // cannot replace a modern flex/grid/hidden box or a scroller.
                    // Keep those authored semantics; unsupported clamp is named.
                    if style.display != Display::Block
                        || style.overflow_x == Overflow::Scroll
                        || style.overflow_y == Overflow::Scroll
                    {
                        skipped.push(Skipped {
                            row: id,
                            reason: "legacy line-clamp requires a non-scrolling block",
                        });
                    } else {
                        push_text!(
                            &mut out,
                            "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:{};overflow:hidden;",
                            exact_num::Shortest(*n)
                        );
                    }
                }
            }
            // @ref LLP 1053 §0 G4 — the row's bits back to CSS keywords.
            (StyleId::FontVariantNumeric, _) => push_text!(
                &mut out,
                "font-variant-numeric:{};",
                exact_kernel::FontVariantNumeric::css(style.font_variant_numeric)
            ),
            (StyleId::GridTemplateColumns, _)
            | (StyleId::GridTemplateRows, _)
            | (StyleId::GridColumn, _)
            | (StyleId::GridRow, _)
            | (StyleId::GridAutoFlow, _)
            | (StyleId::JustifyItems, _) => skipped.push(Skipped {
                row: id,
                reason: "grid rows are not lowered in v1",
            }),
            _ if lowered(id, &value) => {
                property(&mut out, id);
                out.push(':');
                declared(&mut out, id, &value);
                out.push(';');
            }
            _ => skipped.push(Skipped {
                row: id,
                reason: "no CSS lowering for this row's codec",
            }),
        }
    }
    if let Some((x, y, radius, color, opacity)) = shadow {
        let c = Color::rgba(
            color.r(),
            color.g(),
            color.b(),
            (color.a() as f32 * opacity.clamp(0.0, 1.0)) as u8,
        );
        out.push_str("box-shadow:");
        for n in [x, y, radius] {
            num_into(&mut out, n);
            out.push_str("px ");
        }
        rgba_into(&mut out, c);
        out.push(';');
    }
    (out, skipped)
}

fn is_generic_family(value: &str) -> bool {
    matches!(
        value,
        "system-ui"
            | "ui-sans-serif"
            | "sans-serif"
            | "ui-serif"
            | "serif"
            | "ui-monospace"
            | "monospace"
            | "ui-rounded"
    )
}

/// A generic family as a stack every browser renders. Only Safari knows the
/// `ui-*` families: elsewhere a bare one names no font and the text falls to
/// the browser's default, Times (every Markdown code block in Chrome). Each
/// carries the CSS generic it means — for sans-serif and rounded, through
/// `system-ui`, the face Apple's `ui-*` families are.
fn generic_stack(family: &str) -> &str {
    match family {
        "ui-monospace" => "ui-monospace,monospace",
        "ui-serif" => "ui-serif,serif",
        "ui-sans-serif" => "ui-sans-serif,system-ui,sans-serif",
        "ui-rounded" => "ui-rounded,system-ui,sans-serif",
        other => other,
    }
}

fn css_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\a "),
            '\r' => out.push_str("\\d "),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\{:x} ", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One row → one declaration, by the CSS rule for its name and codec.
/// Whether a row's value is one CSS declaration here.
fn lowered(id: StyleId, value: &RowValue<'_>) -> bool {
    match value {
        RowValue::Vec2(_) => id == StyleId::Translate,
        RowValue::Color2(_)
        | RowValue::Tracks(_)
        | RowValue::Placement(_)
        | RowValue::Transitions(_) => false,
        _ => true,
    }
}

/// The row's CSS property, appended: its name with `-` for `_`, but for the
/// few spelled here.
fn property(out: &mut String, id: StyleId) {
    let name = match id {
        StyleId::TextColor => return out.push_str("color"),
        StyleId::TintColor => return out.push_str("--exact-tint"),
        StyleId::PositionType => return out.push_str("position"),
        StyleId::BackdropBlur => return out.push_str("backdrop-filter"),
        id => id.name(),
    };
    for (prefix, suffix) in [
        ("border_radius_", "-radius"),
        ("border_width_", "-width"),
        ("border_style_", "-style"),
        ("border_color_", "-color"),
    ] {
        if let Some(side) = name.strip_prefix(prefix) {
            out.push_str("border-");
            dashed(out, side);
            return out.push_str(suffix);
        }
    }
    dashed(out, name);
}

fn dashed(out: &mut String, name: &str) {
    for (i, word) in name.split('_').enumerate() {
        if i > 0 {
            out.push('-');
        }
        out.push_str(word);
    }
}

/// A [`lowered`] row's CSS value, appended.
fn declared(out: &mut String, id: StyleId, value: &RowValue<'_>) {
    match value {
        RowValue::Dimension(d) => dimension(out, *d),
        RowValue::Color(c) => rgba_into(out, *c),
        // The browser resolves this one (LLP 1034 D2): handed the function
        // it does so per element against the inherited `color-scheme`, with
        // no work of ours and no repaint pass. This is the whole reason the
        // kernel keeps the pair instead of flattening it.
        RowValue::ColorValue(ColorValue::Fixed(c)) => rgba_into(out, *c),
        RowValue::ColorValue(ColorValue::LightDark(l, d)) => {
            out.push_str("light-dark(");
            rgba_into(out, *l);
            out.push_str(", ");
            rgba_into(out, *d);
            out.push(')');
        }
        RowValue::LineHeight(v) => out.push_str(&v.css()),
        RowValue::Enum(e) => out.push_str(e),
        RowValue::ClipPath(p) => out.push_str(&p.css()),
        RowValue::ShapeOutside(p) => out.push_str(&p.css()),
        RowValue::AspectRatio(r) => out.push_str(&r.css()),
        // The kernel's canonical CSS: explicit stops, `#rrggbbaa` colours and
        // `light-dark()` pairs the browser resolves per element (LLP 1034
        // D2); the browser mixes premultiplied, as CSS says (LLP 1056).
        RowValue::BackgroundImage(g) => out.push_str(&g.css()),
        RowValue::Vec2(v) => {
            num_into(out, v.x);
            out.push_str("px ");
            num_into(out, v.y);
            out.push_str("px");
        }
        RowValue::Number(n) => match id {
            StyleId::FlexGrow
            | StyleId::FlexShrink
            | StyleId::Opacity
            | StyleId::ZIndex
            | StyleId::FontWeight
            | StyleId::Scale
            | StyleId::ShadowOpacity => num_into(out, *n as f32),
            StyleId::Rotate => {
                num_into(out, *n as f32);
                out.push_str("deg");
            }
            StyleId::BackdropBlur => {
                out.push_str("blur(");
                num_into(out, *n as f32);
                out.push_str("px)");
            }
            _ => {
                num_into(out, *n as f32);
                out.push_str("px");
            }
        },
        RowValue::Color2(_)
        | RowValue::Tracks(_)
        | RowValue::Placement(_)
        | RowValue::Transitions(_) => {}
    }
}

/// A `transition` row as CSS; `true` when a spring was left out.
pub fn transition_css(t: &Transitions) -> (String, bool) {
    let mut text = String::new();
    let mut spring = false;
    for tr in &t.0 {
        match &tr.timing {
            TimingFunction::Spring(_) => spring = true,
            TimingFunction::Easing(e) => {
                if !text.is_empty() {
                    text.push(',');
                }
                let _ = write!(
                    text,
                    "{} {}s {} {}s",
                    transition_property(tr),
                    num(tr.duration as f32),
                    easing_css(e),
                    num(tr.delay as f32)
                );
            }
        }
    }
    (text, spring)
}

fn transition_property(tr: &Transition) -> &'static str {
    match tr.property {
        TransitionProperty::All => "all",
        TransitionProperty::Property(p) => p.name(),
    }
}

/// A CSS `<easing-function>` from the motion crate's spelling.
pub fn easing_css(e: &Easing) -> String {
    match e {
        Easing::Linear => "linear".into(),
        Easing::Ease => "ease".into(),
        Easing::EaseIn => "ease-in".into(),
        Easing::EaseOut => "ease-out".into(),
        Easing::EaseInOut => "ease-in-out".into(),
        Easing::CubicBezier { x1, y1, x2, y2 } => format!(
            "cubic-bezier({},{},{},{})",
            num(*x1 as f32),
            num(*y1 as f32),
            num(*x2 as f32),
            num(*y2 as f32)
        ),
        Easing::Steps { count, position } => format!(
            "steps({count},{})",
            match position {
                StepPosition::JumpStart => "jump-start",
                StepPosition::JumpEnd => "jump-end",
                StepPosition::JumpNone => "jump-none",
                StepPosition::JumpBoth => "jump-both",
            }
        ),
        Easing::PiecewiseLinear(stops) => {
            let mut text = String::from("linear(");
            for (i, s) in stops.iter().enumerate() {
                if i > 0 {
                    text.push(',');
                }
                let _ = write!(
                    text,
                    "{} {}%",
                    num(s.output as f32),
                    num((s.input * 100.0) as f32)
                );
            }
            text.push(')');
            text
        }
    }
}

fn dimension(out: &mut String, d: Dimension) {
    match d {
        Dimension::Auto => out.push_str("auto"),
        Dimension::Points(p) => {
            num_into(out, p);
            out.push_str("px");
        }
        Dimension::Percent(p) => {
            num_into(out, p);
            out.push('%');
        }
        Dimension::Calc(p, plus) => {
            out.push_str("calc(");
            num_into(out, p);
            out.push_str(if plus < 0.0 { "% - " } else { "% + " });
            num_into(out, plus.abs());
            out.push_str("px)");
        }
        // The browser resolves the inset itself (under `viewport-fit=cover`,
        // which the glue sets from the root's prop; zero otherwise).
        Dimension::Env(edge, plus) => {
            let inset = if plus == 0.0 { "env(" } else { "calc(env(" };
            out.push_str(inset);
            out.push_str("safe-area-inset-");
            out.push_str(edge.name());
            out.push(')');
            if plus != 0.0 {
                out.push_str(if plus < 0.0 { " - " } else { " + " });
                num_into(out, plus.abs());
                out.push_str("px)");
            }
        }
    }
}

/// `rgba(r,g,b,a)` with the alpha as a fraction.
pub fn rgba(c: Color) -> String {
    let mut out = String::new();
    rgba_into(&mut out, c);
    out
}

fn rgba_into(out: &mut String, c: Color) {
    out.push_str("rgba(");
    for channel in [c.r(), c.g(), c.b()] {
        num_into(out, f32::from(channel));
        out.push(',');
    }
    num_into(out, c.a() as f32 / 255.0);
    out.push(')');
}

/// Shortest exact decimal for a number: `24`, not `24.0`; `0.5`; `1.2`.
pub fn num(n: f32) -> String {
    let mut out = String::new();
    num_into(&mut out, n);
    out
}

/// [`num`], appended, without the formatter (the slow part of a page's
/// first styles): a whole number as an integer, the rest shortest.
fn num_into(out: &mut String, n: f32) {
    if n.fract() == 0.0 && n.abs() < 1e9 {
        (n as i64).push_to(out);
    } else {
        Shortest32(n).push_to(out);
    }
}

#[cfg(test)]
mod flow_tests {
    use super::*;
    #[test]
    fn clamp_does_not_replace_layout_visibility_or_scrolling() {
        for (row, value) in [
            (StyleId::Display, "flex"),
            (StyleId::Display, "grid"),
            (StyleId::Display, "none"),
            (StyleId::OverflowX, "scroll"),
            (StyleId::OverflowY, "scroll"),
        ] {
            let mut style = StyleProps::default();
            style
                .set_dynamic(row, &exact_kernel::StyleValue::Text(value.into()))
                .unwrap();
            let (before, _) = css_text(&style, &[]);
            style
                .set_dynamic(StyleId::LineClamp, &exact_kernel::StyleValue::Number(2.0))
                .unwrap();
            let (after, skipped) = css_text(&style, &[]);
            assert_eq!(before, after, "{row:?}: {value}");
            assert_eq!(
                skipped.iter().map(|s| s.row).collect::<Vec<_>>(),
                [StyleId::LineClamp]
            );
            style
                .set_dynamic(StyleId::LineClamp, &exact_kernel::StyleValue::Number(0.0))
                .unwrap();
            assert!(css_text(&style, &[]).1.is_empty());
        }
    }

    #[test]
    fn exclusion_rows_keep_authored_css_visible() {
        let mut s = StyleProps::default();
        for (id, value) in [
            (StyleId::WrapFlow, "both"),
            (StyleId::ShapeOutside, "circle()"),
            (StyleId::ShapeMargin, "8px"),
        ] {
            s.set_dynamic(id, &exact_kernel::StyleValue::Text(value.into()))
                .unwrap();
        }
        let (css, skipped) = css_text(&s, &[]);
        assert!(css.contains("shape-outside:circle(closest-side at 50% 50%);"));
        assert!(css.contains("shape-margin:8px;"));
        assert!(css.contains("wrap-flow:both;"));
        assert!(skipped.is_empty());
    }
}

#[cfg(test)]
mod writer_tests {
    use super::*;

    /// The writers before they wrote in place: joined parts.
    fn transition_joined(t: &Transitions) -> (String, bool) {
        let mut parts = Vec::new();
        let mut spring = false;
        for tr in &t.0 {
            match &tr.timing {
                TimingFunction::Spring(_) => spring = true,
                TimingFunction::Easing(e) => parts.push(format!(
                    "{} {}s {} {}s",
                    transition_property(tr),
                    num(tr.duration as f32),
                    easing_css(e),
                    num(tr.delay as f32)
                )),
            }
        }
        (parts.join(","), spring)
    }

    #[test]
    fn transitions_and_linear_easings_write_what_joining_wrote() {
        for text in [
            "opacity 1s",
            "opacity 250ms ease-in-out, all 0.5s cubic-bezier(0.4, 0, 0.2, 1) 100ms, translate spring(180, 12, 1)",
            "opacity 1s steps(4, jump-both), height 200ms linear 50ms",
            "opacity 1s linear(0, 0.2, 0.6 60%, 0.8, 1), translate 1s linear(0 0% 20%, 1 80% 100%)",
            "translate spring(180, 12, 1)",
        ] {
            let t = Transitions::parse(text).unwrap();
            assert_eq!(transition_css(&t), transition_joined(&t), "{text}");
        }
    }
}

#[cfg(test)]
mod declaration_tests {
    use super::*;
    use exact_kernel::StyleValue;

    fn css(rows: &[(StyleId, StyleValue)], fonts: &[&str]) -> String {
        let mut style = StyleProps::default();
        for (id, value) in rows {
            style.set_dynamic(*id, value).unwrap();
        }
        let fonts: Vec<String> = fonts.iter().map(|f| f.to_string()).collect();
        css_text(&style, &fonts).0
    }

    /// The declarations `css_text` composes itself, as the `write!`-built
    /// text had them, byte for byte.
    #[test]
    fn composed_declarations_keep_their_text() {
        let cases = [
            css(
                &[(StyleId::FontFamily, StyleValue::Number(0.0))],
                &["Inter \"Var\"\n"],
            ),
            css(
                &[(StyleId::FontFamily, StyleValue::Number(1.0))],
                &["x", "ui-monospace"],
            ),
            css(
                &[(
                    StyleId::Transition,
                    StyleValue::Text(
                        "opacity 250ms ease-in-out, all 0.5s cubic-bezier(0.4, 0, 0.2, 1) 100ms"
                            .into(),
                    ),
                )],
                &[],
            ),
            css(&[(StyleId::LineClamp, StyleValue::Number(3.0))], &[]),
            css(
                &[
                    (StyleId::ShadowOffset, StyleValue::Vec2(0.0, 2.5)),
                    (StyleId::ShadowRadius, StyleValue::Number(12.0)),
                    (StyleId::ShadowColor, StyleValue::Text("#11223380".into())),
                    (StyleId::ShadowOpacity, StyleValue::Number(0.35)),
                ],
                &[],
            ),
            css(
                &[
                    (StyleId::ShadowOffset, StyleValue::Vec2(-1.0, 1e9)),
                    (StyleId::ShadowRadius, StyleValue::Number(0.1)),
                ],
                &[],
            ),
            css(
                &[
                    (
                        StyleId::FontVariantNumeric,
                        StyleValue::Text("tabular-nums".into()),
                    ),
                    (StyleId::WhiteSpace, StyleValue::Text("nowrap".into())),
                ],
                &[],
            ),
            css(
                &[(
                    StyleId::FontVariantNumeric,
                    StyleValue::Text("normal".into()),
                )],
                &[],
            ),
            css(
                &[(StyleId::WhiteSpace, StyleValue::Text("pre-line".into()))],
                &[],
            ),
            css(
                &[
                    (StyleId::Opacity, StyleValue::Number(0.125)),
                    (StyleId::Width, StyleValue::Number(33.5)),
                ],
                &[],
            ),
        ];
        let golden = [
            r#"font-family:"Inter \"Var\"\a ";"#,
            "font-family:ui-monospace,monospace;",
            "transition:opacity 0.25s ease-in-out 0s,all 0.5s cubic-bezier(0.4,0,0.2,1) 0.1s;",
            "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:3;overflow:hidden;",
            "box-shadow:0px 2.5px 12px rgba(17,34,51,0.17254902);",
            "box-shadow:-1px 1000000000px 0.1px rgba(0,0,0,0);",
            "font-variant-numeric:tabular-nums;white-space:nowrap;",
            "font-variant-numeric:normal;",
            "white-space:pre-line;",
            "width:33.5px;opacity:0.125;",
        ];
        assert_eq!(cases, golden);
    }

    /// LLP 1053: `aspect-ratio` as authored (never a rounded float),
    /// `direction`, and the `flex-grow` longhand reach the page as CSS.
    #[test]
    fn layout_rows_keep_their_css() {
        let t = |s: &str| StyleValue::Text(s.into());
        for (rows, want) in [
            (
                vec![(StyleId::AspectRatio, t("16/9"))],
                "aspect-ratio:16 / 9;",
            ),
            (
                vec![(StyleId::AspectRatio, StyleValue::Number(2.0))],
                "aspect-ratio:2 / 1;",
            ),
            (
                vec![(StyleId::AspectRatio, t("4/3 auto"))],
                "aspect-ratio:auto 4 / 3;",
            ),
            (
                vec![(StyleId::AspectRatio, StyleValue::Auto)],
                "aspect-ratio:auto;",
            ),
            (vec![(StyleId::Direction, t("rtl"))], "direction:rtl;"),
            (
                vec![(StyleId::FlexGrow, StyleValue::Number(1.0))],
                "flex-grow:1;",
            ),
        ] {
            assert_eq!(css(&rows, &[]), want);
        }
    }

    /// LLP 1056: a gradient is one `background-image` declaration after the
    /// colour it paints over; a `light-dark()` stop is the browser's to
    /// resolve, and `none` clears.
    #[test]
    fn background_image_is_one_declaration_over_the_colour() {
        let t = |s: &str| StyleValue::Text(s.into());
        assert_eq!(
            css(
                &[
                    (StyleId::BackgroundColor, t("#102030")),
                    (StyleId::BackgroundImage, t("linear-gradient(to top, transparent, light-dark(#fff, #000) 40%)")),
                ],
                &[]
            ),
            "background-color:rgba(16,32,48,1);background-image:linear-gradient(0deg, #00000000 0%, light-dark(#ffffffff, #000000ff) 40%);"
        );
        assert_eq!(
            css(&[(StyleId::BackgroundImage, t("radial-gradient(circle at 10px bottom, #000 25%, #fff)"))], &[]),
            "background-image:radial-gradient(circle farthest-corner at 10px 100%, #000000ff 25%, #ffffffff 100%);"
        );
        assert_eq!(
            css(&[(StyleId::BackgroundImage, t("none"))], &[]),
            "background-image:none;"
        );
    }
}
