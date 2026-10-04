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
use exact_motion::{Easing, Property, TimingFunction, Transition, TransitionProperty, Transitions};
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
    // @ref LLP 1061's ruling: the row and host feedback are independent
    // CSS numbers, multiplied through `scale` without writing `transform`.
    let press = press_composes(style);
    for id in style.mask.iter() {
        let value = style.get(id);
        match (id, &value) {
            (StyleId::Transition, RowValue::Transitions(t)) => {
                let (mut text, spring_skipped) = transition_css(t);
                if press {
                    // CSS transitions sit outside the additive animation stack.
                    // Transition the row's typed number so a press multiplies
                    // its current value, not the transition's final target.
                    text = text.replace("scale ", "--exact-scale ");
                    if let Some(tr) = t.matching(Property::Scale) {
                        let mut tr = tr.clone();
                        tr.property = TransitionProperty::Property(Property::Scale);
                        let (scale, _) = transition_css(&Transitions(vec![tr]));
                        if !scale.is_empty() {
                            if !text.is_empty() {
                                text.push(',');
                            }
                            text.push_str("scale 0s,");
                            text.push_str(&scale.replace("scale ", "--exact-scale "));
                        }
                    }
                }
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
            // @ref LLP 1055 D5/D7 — the browser runs it; the rule it names
            // is in the page's stylesheet (`Batch::keyframes`).
            (StyleId::Animation, RowValue::Animations(a)) => {
                if let Some(link) = crate::link::linked().animations.filter(|_| !a.0.is_empty()) {
                    push_text!(&mut out, "animation:{};", (link.list)(a, press));
                }
            }
            // @ref LLP 1063 — not CSS properties: custom properties the page's
            // presence module reads (`presence-glue.js`), inherited by nothing
            // it reads, since it reads only the element's own declaration. The
            // exit's own list also rides its `exit` op (a virtualized row leaves
            // as its wrapper, which declares none); here it declares the row,
            // so the module is fetched before the first exit needs it. The
            // rules it names are in the page's stylesheet, as `animation`'s.
            (StyleId::ExitAnimation, RowValue::Animations(a)) => {
                if let Some(link) = crate::link::linked().animations.filter(|_| !a.0.is_empty()) {
                    push_text!(
                        &mut out,
                        "--exact-exit-animation:{};",
                        (link.list)(a, press)
                    );
                }
            }
            // @ref LLP 1057.003 D2 — a drag timeline is no timeline the
            // browser runs: custom properties the page's drag code
            // (motion-glue.js) reads. A bound consumer's animations are
            // paused (after `animation`, bit 114, so its shorthand cannot
            // reset the play state) and the drag seeks them.
            (StyleId::DragTimeline, RowValue::DragTimeline(d)) => {
                if d.name.is_some() {
                    push_text!(&mut out, "--exact-drag-timeline:{};", d.css());
                }
            }
            (StyleId::AnimationTimeline, RowValue::AnimationTimeline(t)) => {
                if t.0.is_some() {
                    push_text!(
                        &mut out,
                        "--exact-animation-timeline:{};animation-play-state:paused;",
                        t.css()
                    );
                }
            }
            (StyleId::AnimationRange, RowValue::AnimationRange(r)) => {
                if r.0.is_some() {
                    push_text!(&mut out, "--exact-animation-range:{};", r.css());
                }
            }
            // @ref LLP 1057.003 D4 — CSS's own row, for the timelines the
            // browser resolves (scroll timelines, D5), and a custom property
            // the drag code's lookup reads (motion-glue.js `timelineSource`).
            (StyleId::TimelineScope, RowValue::TimelineScope(s)) => {
                if **s != exact_kernel::timeline::TimelineScope::None {
                    let s = s.css();
                    push_text!(
                        &mut out,
                        "timeline-scope:{};--exact-timeline-scope:{};",
                        s,
                        s
                    );
                }
            }
            (StyleId::LayoutTransition, RowValue::Transitions(t)) => {
                if let Some(text) = layout_transition_css(t) {
                    push_text!(&mut out, "--exact-layout-transition:{};", text);
                }
            }
            // @ref LLP 1077 D1 — Apple's continuous curve is no CSS keyword:
            // the web's stand-in is `superellipse(K)` over the radius scaled
            // to the same reach (kernel `APPLE_ON_THE_WEB`, declared in LLP
            // 1001). CSS's own keywords are the browser's.
            (StyleId::CornerShape, RowValue::CornerShape(c)) => {
                let (k, _) = exact_kernel::corner::APPLE_ON_THE_WEB;
                let web = exact_kernel::corner::CornerShape(c.0.map(|corner| match corner {
                    exact_kernel::corner::Corner::AppleContinuous => {
                        exact_kernel::corner::Corner::Superellipse(k)
                    }
                    other => other,
                }));
                push_text!(&mut out, "corner-shape:{};", web.css());
            }
            (
                StyleId::BorderRadiusTopLeft
                | StyleId::BorderRadiusTopRight
                | StyleId::BorderRadiusBottomRight
                | StyleId::BorderRadiusBottomLeft,
                RowValue::Dimension(d),
            ) if apple_corner(style, id) => {
                property(&mut out, id);
                out.push_str(":calc(");
                dimension(&mut out, *d);
                push_text!(
                    &mut out,
                    " * {});",
                    exact_num::Shortest32(exact_kernel::corner::APPLE_ON_THE_WEB.1)
                );
            }
            // @ref LLP 1077 D8 — `rotate` is the angle and its axis, and
            // `translate` x, y and z: one declaration each.
            (StyleId::Rotate, RowValue::Number(n)) if style.rotate_axis.0 != [0.0, 0.0, 1.0] => {
                push_text!(&mut out, "rotate:{} ", style.rotate_axis.css());
                num_into(&mut out, *n as f32);
                out.push_str("deg;");
            }
            (StyleId::Translate, RowValue::Vec2(v)) if style.translate_z != 0.0 => {
                out.push_str("translate:");
                for n in [v.x, v.y, style.translate_z] {
                    num_into(&mut out, n);
                    out.push_str("px ");
                }
                out.pop();
                out.push(';');
            }
            (StyleId::Perspective, RowValue::Number(n)) if *n == 0.0 => {
                out.push_str("perspective:none;")
            }
            (StyleId::Scale, RowValue::Number(n)) if press => {
                out.push_str("--exact-scale:");
                num_into(&mut out, *n as f32);
                out.push(';');
            }
            (StyleId::PressScale, RowValue::Number(n)) => {
                if press {
                    out.push_str("scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:");
                    num_into(&mut out, *n as f32);
                    out.push(';');
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
            (StyleId::GridAutoFlow, RowValue::Enum(flow)) => {
                out.push_str("grid-auto-flow:");
                out.push_str(flow);
                out.push(';');
            }
            // @ref LLP 1069.011 D8 — the accent also as an inherited custom
            // property a native button's look reads; `auto` is the browser's.
            (StyleId::AccentColor, _) if lowered(id, &value) => {
                property(&mut out, id);
                out.push(':');
                declared(&mut out, id, &value);
                out.push_str(";--exact-accent:");
                let start = out.len();
                declared(&mut out, id, &value);
                if &out[start..] == "auto" {
                    out.truncate(start);
                    out.push_str("AccentColor");
                }
                out.push(';');
            }
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
    (out, skipped)
}

/// Whether a node's press feedback composes through `--exact-scale`
/// (LLP 1061): its `scale` is the important product of the row's number and
/// the feedback's factor, so an animation of `scale` reaches it only through
/// `--exact-scale`.
pub(crate) fn press_composes(style: &StyleProps) -> bool {
    style.mask.has(StyleId::PressScale) && style.press_scale != 1.0
}

/// Whether `a` needs the pressable node's rule: its keyframes animate `scale`.
fn press_rule(a: &exact_motion::animation::Animation, press: bool) -> bool {
    press
        && a.keyframes
            .0
            .iter()
            .any(|f| f.values.iter().any(|(p, _)| *p == Property::Scale))
}

/// The `@keyframes` rule's name an animation plays on a node: its own, or, on
/// a pressable node whose keyframes animate `scale`, the rule that also
/// animates `--exact-scale`. An animated custom property keeps Chrome from
/// compositing the whole animation, so only the nodes that read one get it.
pub fn keyframes_name(a: &exact_motion::animation::Animation, press: bool) -> String {
    let mut name = a.name.clone();
    if press_rule(a, press) {
        name.push_str("-exact-press");
    }
    name
}

/// The body of the rule [`keyframes_name`] names. The pressable node's copies
/// each `scale` keyframe into `--exact-scale`: its important `scale`
/// composition wins over the animation's own.
pub fn keyframes_css(a: &exact_motion::animation::Animation, press: bool) -> String {
    let text = a.keyframes.css();
    if !press_rule(a, press) {
        return text;
    }
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("scale:") {
        let end = at + rest[at..].find(';').expect("a keyframe declaration ends");
        out.push_str(&rest[..=end]);
        out.push_str("--exact-");
        out.push_str(&rest[at..=end]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Send each `@keyframes` rule `style`'s animations name, and its exit's,
/// that the page lacks (`sent`), once, by name (LLP 1055 D7).
pub(crate) fn send_keyframes(
    sent: &mut exact_kernel::SortedSet<String>,
    style: &StyleProps,
    batch: &mut crate::batch::Batch,
) {
    let Some(link) = crate::link::linked().animations else {
        return;
    };
    let press = press_composes(style);
    for a in style.animation.0.iter().chain(&style.exit_animation.0) {
        let name = (link.name)(a, press);
        if !sent.contains(&name) {
            batch.keyframes(&name, &(link.body)(a, press));
            sent.insert(name);
        }
    }
}

/// An `animation` list as CSS, naming [`keyframes_name`]'s rules.
pub fn animations_css(a: &exact_motion::animation::Animations, press: bool) -> String {
    a.css_named(&|a| std::borrow::Cow::Owned(keyframes_name(a, press)))
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

pub(crate) fn css_string(value: &str) -> String {
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

/// Whether the corner a radius row sizes is `-apple-continuous` (LLP 1077 D1).
fn apple_corner(style: &StyleProps, id: StyleId) -> bool {
    let i = match id {
        StyleId::BorderRadiusTopLeft => 0,
        StyleId::BorderRadiusTopRight => 1,
        StyleId::BorderRadiusBottomRight => 2,
        _ => 3,
    };
    style.corner_shape.0[i] == exact_kernel::corner::Corner::AppleContinuous
}

/// One row → one declaration, by the CSS rule for its name and codec.
/// Whether a row's value is one CSS declaration here.
fn lowered(id: StyleId, value: &RowValue<'_>) -> bool {
    match value {
        RowValue::Vec2(_) => id == StyleId::Translate,
        // Written with `rotate` and `translate` (LLP 1077 D8).
        RowValue::RotateAxis(_) => false,
        // Apple's affordances: no CSS property (LLP 1077 §5).
        RowValue::SymbolPalette(_) => false,
        _ if matches!(
            id,
            StyleId::SymbolRendering
                | StyleId::SymbolValue
                | StyleId::SymbolEffect
                | StyleId::PressHaptic
                | StyleId::ContentTransition
                | StyleId::ScrollEdgeEffect
                | StyleId::HoverEffect
                | StyleId::SmartInvert
        ) =>
        {
            false
        }
        RowValue::Number(_) if id == StyleId::TranslateZ => false,
        RowValue::Color2(_)
        | RowValue::Transitions(_)
        | RowValue::Animations(_)
        | RowValue::DragTimeline(_)
        | RowValue::AnimationTimeline(_)
        | RowValue::AnimationRange(_)
        | RowValue::TimelineScope(_) => false,
        _ => true,
    }
}

/// The row's CSS property, appended: its name with `-` for `_`, but for the
/// few spelled here.
fn property(out: &mut String, id: StyleId) {
    // Every node's every row asks: each name is spelled once per process.
    static NAMES: [std::sync::OnceLock<String>; 256] = [const { std::sync::OnceLock::new() }; 256];
    out.push_str(NAMES[id as usize].get_or_init(|| {
        let mut name = String::new();
        spell_property(&mut name, id);
        name
    }));
}

fn spell_property(out: &mut String, id: StyleId) {
    let name = match id {
        StyleId::TextColor => return out.push_str("color"),
        StyleId::TintColor => return out.push_str("--exact-tint"),
        StyleId::PositionType => return out.push_str("position"),
        StyleId::BackdropBlur => return out.push_str("backdrop-filter"),
        StyleId::SvgMask => return out.push_str("mask"),
        // @ref LLP 1077 D7 — the Compat Standard's prefixed names.
        StyleId::TextStrokeWidth => return out.push_str("-webkit-text-stroke-width"),
        StyleId::TextStrokeColor => return out.push_str("-webkit-text-stroke-color"),
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
        // A system colour is its pair: Chrome has no `-apple-system-*`.
        RowValue::ColorValue(c @ ColorValue::System(_)) => {
            declared(out, id, &RowValue::ColorValue(c.pair()))
        }
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
        RowValue::DragTimeline(d) => out.push_str(&d.css()),
        RowValue::AnimationTimeline(t) => out.push_str(&t.css()),
        RowValue::AnimationRange(r) => out.push_str(&r.css()),
        RowValue::TimelineScope(s) => out.push_str(&s.css()),
        RowValue::Paint(p) => out.push_str(&p.css()),
        RowValue::DashArray(d) => out.push_str(&d.css()),
        RowValue::Transform(t) => out.push_str(&t.css()),
        RowValue::TransformOrigin(t) => out.push_str(&t.css()),
        RowValue::PaintOrder(p) => out.push_str(&p.css()),
        RowValue::Marker(m) => out.push_str(&m.css()),
        RowValue::Filter(f) => out.push_str(&f.css()),
        // The kernel's canonical CSS: explicit stops, `#rrggbbaa` colours and
        // `light-dark()` pairs the browser resolves per element (LLP 1034
        // D2); the browser mixes premultiplied, as CSS says (LLP 1066).
        RowValue::BackgroundImage(g) | RowValue::MaskImage(g) => out.push_str(&g.css()),
        RowValue::TextShadow(s) => out.push_str(&s.css()),
        RowValue::BoxShadow(s) => out.push_str(&s.css()),
        RowValue::CornerShape(c) => out.push_str(&c.css()),
        RowValue::RotateAxis(_) | RowValue::SymbolPalette(_) => {}
        RowValue::Tracks(tracks) => out.push_str(tracks.css()),
        RowValue::Placement(placement) => out.push_str(&placement.css()),
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
            // SVG's unitless numbers (LLP 1055 D2); `r`, `cx`, `cy` are lengths.
            | StyleId::FillOpacity
            | StyleId::StrokeOpacity
            | StyleId::StrokeMiterlimit
            | StyleId::StrokeWidth
            | StyleId::StrokeDashoffset
            | StyleId::StopOpacity
            | StyleId::FloodOpacity => num_into(out, *n as f32),
            StyleId::Rotate => {
                num_into(out, *n as f32);
                out.push_str("deg");
            }
            // @ref LLP 1053.000 D1 — `none` is 0, which paints nothing and
            // makes no backdrop root, where `blur(0px)` would.
            StyleId::BackdropBlur if *n == 0.0 => out.push_str("none"),
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
        RowValue::Color2(_) | RowValue::Transitions(_) | RowValue::Animations(_) => {}
    }
}

/// A `transition` row as CSS; `true` when a spring was left out. A spring
/// drives the compositor rows as physics, lowered to frames by the host; the
/// properties it does not ([`Property::springs`]: paint, SVG geometry)
/// play its curve from rest as `linear()`, as every native host does (LLP
/// 1062 D3).
pub fn transition_css(t: &Transitions) -> (String, bool) {
    let mut text = String::new();
    let mut spring = false;
    let mut push = |name: &str, duration: f64, easing: &Easing, delay: f64| {
        if !text.is_empty() {
            text.push(',');
        }
        let _ = write!(
            text,
            "{name} {}s {} {}s",
            num(duration as f32),
            easing_css(easing),
            num(delay as f32)
        );
    };
    for tr in &t.0 {
        match &tr.timing {
            TimingFunction::Spring(config) => {
                let names: Vec<&str> = match tr.property {
                    // Every property no spring drives as physics, the four
                    // sides as their shorthand.
                    TransitionProperty::All => Property::ALL
                        .into_iter()
                        .filter(|p| !p.springs() && *p != Property::ShadowColor)
                        .map(|p| match p {
                            Property::BorderTopColor => "border-color",
                            p => p.css_name(),
                        })
                        .filter(|n| !n.starts_with("border-") || *n == "border-color")
                        .collect(),
                    TransitionProperty::BorderColor => vec!["border-color"],
                    TransitionProperty::Property(p) if !p.springs() => vec![p.css_name()],
                    TransitionProperty::Property(_) => Vec::new(),
                };
                spring |= match tr.property {
                    TransitionProperty::All => true,
                    TransitionProperty::Property(p) => p.springs(),
                    TransitionProperty::BorderColor => false,
                };
                if !names.is_empty() {
                    let (duration, easing) = config.easing();
                    for name in names {
                        push(name, duration, &easing, tr.delay);
                    }
                }
            }
            TimingFunction::Easing(e) => {
                push(transition_property(tr), tr.duration, e, tr.delay);
            }
        }
    }
    (text, spring)
}

fn transition_property(tr: &Transition) -> &'static str {
    tr.property.css_name()
}

/// A `layout-transition` row as the presence module reads it: duration and
/// delay in milliseconds, then a CSS easing (LLP 1063). A spring is
/// `spring(stiffness, damping, mass)`: the module lowers each move itself,
/// from its displacement and velocity in points, to the grid and rest
/// threshold the engine settles on natively. `None` when no declaration
/// covers layout.
pub fn layout_transition_css(t: &Transitions) -> Option<String> {
    let tr = t.matching(exact_motion::Property::Layout)?;
    let easing = match &tr.timing {
        TimingFunction::Easing(e) => easing_css(e),
        TimingFunction::Spring(c) => format!(
            "spring({}, {}, {})",
            num(c.stiffness as f32),
            num(c.damping as f32),
            num(c.mass as f32)
        ),
    };
    Some(format!(
        "{} {} {easing}",
        num((tr.duration * 1000.0) as f32),
        num((tr.delay * 1000.0) as f32)
    ))
}

/// A CSS `<easing-function>` from the motion crate's spelling.
pub fn easing_css(e: &Easing) -> String {
    e.css()
}

pub(crate) fn dimension(out: &mut String, d: Dimension) {
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
        // The browser resolves the segment itself too (LLP 1078 D6): the
        // text is CSS-ENV-1's, untouched.
        Dimension::Segment(var, x, y, plus) => exact_kernel::style::env::css(var, x, y, plus, out),
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

    /// LLP 1062 D3: a spring on paint is its curve from rest as `linear()`,
    /// the easing the native engine plays; the compositor rows stay the
    /// host's frames, and `all` names each property the spring does not drive.
    #[test]
    fn a_paint_spring_is_its_curve_as_linear() {
        let config = exact_motion::SpringConfig {
            stiffness: 180.0,
            damping: 12.0,
            mass: 1.0,
        };
        let (duration, easing) = config.easing();
        let curve = format!("{}s {} 0.1s", num(duration as f32), easing.css());
        let t = Transitions::parse("background-color spring(180, 12, 1) 0s 100ms").unwrap();
        assert_eq!(
            transition_css(&t),
            (format!("background-color {curve}"), false)
        );
        let t = Transitions::parse("opacity 1s, all spring(180, 12, 1) 0s 100ms").unwrap();
        let (text, skipped) = transition_css(&t);
        assert!(skipped, "the compositor rows' spring is the host's");
        let names: Vec<&str> = text
            .split(&format!(" {curve}"))
            .map(|n| n.trim_start_matches(','))
            .collect();
        assert_eq!(
            names,
            [
                "opacity 1s ease 0s,stroke-dashoffset",
                "r",
                "color",
                "background-color",
                "fill",
                "stroke",
                "cx",
                "cy",
                "x",
                "y",
                "rx",
                "ry",
                "border-color",
                "--exact-tint",
                "box-shadow",
                ""
            ]
        );
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
            // LLP 1077 D4: one row, CSS's list.
            css(
                &[(
                    StyleId::BoxShadow,
                    StyleValue::Text(
                        "0 2.5px 12px #11223359, inset 0 1px 0 2px light-dark(#00000080, #ffffff)"
                            .into(),
                    ),
                )],
                &[],
            ),
            css(
                &[(StyleId::BoxShadow, StyleValue::Text("none".into()))],
                &[],
            ),
            css(
                &[(StyleId::TextTransform, StyleValue::Text("uppercase".into()))],
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
            "box-shadow:0px 2.5px 12px 0px #11223359, inset 0px 1px 0px 2px light-dark(#00000080, #ffffffff);",
            "box-shadow:none;",
            "text-transform:uppercase;",
            "font-variant-numeric:tabular-nums;white-space:nowrap;",
            "font-variant-numeric:normal;",
            "white-space:pre-line;",
            "width:33.5px;opacity:0.125;",
        ];
        assert_eq!(cases, golden);
    }

    /// LLP 1053: `aspect-ratio` as authored (never a rounded float),
    /// `direction`, the `flex-grow` longhand and `transform-origin` reach
    /// the page as CSS.
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
            // LLP 1061 D6: canonical, each axis a percentage or px.
            (
                vec![(StyleId::TransformOrigin, t("top left"))],
                "transform-origin:0% 0%;",
            ),
            (
                vec![(StyleId::TransformOrigin, StyleValue::Percent(25.0))],
                "transform-origin:25% 50%;",
            ),
            (
                vec![(StyleId::TransformOrigin, t("right 4px 0"))],
                "transform-origin:100% 4px;",
            ),
            (
                vec![(StyleId::FlexGrow, StyleValue::Number(1.0))],
                "flex-grow:1;",
            ),
        ] {
            assert_eq!(css(&rows, &[]), want);
        }
    }

    #[test]
    fn every_grid_row_has_css_and_none_is_skipped() {
        let t = |s: &str| StyleValue::Text(s.into());
        for (row, value, want) in [
            (
                StyleId::GridTemplateColumns,
                t("repeat(2, minmax(80px, 1fr)) 25%"),
                "grid-template-columns:repeat(2, minmax(80px, 1fr)) 25%;",
            ),
            (
                StyleId::GridTemplateRows,
                t("40px auto min-content max-content"),
                "grid-template-rows:40px auto min-content max-content;",
            ),
            (
                StyleId::GridColumn,
                t("-3 / span 2"),
                "grid-column:-3 / span 2;",
            ),
            (StyleId::GridRow, t("2 / -1"), "grid-row:2 / -1;"),
            (
                StyleId::GridAutoFlow,
                t("row dense"),
                "grid-auto-flow:row dense;",
            ),
            (StyleId::JustifyItems, t("center"), "justify-items:center;"),
        ] {
            let mut style = StyleProps::default();
            style.set_dynamic(row, &value).unwrap();
            let (text, skipped) = css_text(&style, &[]);
            assert_eq!(text, want, "{row:?}");
            assert!(skipped.is_empty(), "{row:?}: {skipped:?}");
        }
    }

    /// LLP 1066: a gradient is one `background-image` declaration after the
    /// colour it paints over; a `light-dark()` stop is the browser's to
    /// resolve, and `none` clears.
    #[test]
    fn a_3d_rotate_and_translate_are_one_declaration_each() {
        let t = |s: &str| StyleValue::Text(s.into());
        let text = css(
            &[
                (StyleId::Rotate, t("y 30deg")),
                (StyleId::RotateAxis, t("y 30deg")),
                (StyleId::Translate, t("1px 2px 3px")),
                (StyleId::TranslateZ, t("1px 2px 3px")),
                (StyleId::Perspective, StyleValue::Number(800.0)),
            ],
            &[],
        );
        assert!(text.contains("rotate:y 30deg;"), "{text}");
        assert!(text.contains("translate:1px 2px 3px;"), "{text}");
        assert!(text.contains("perspective:800px;"), "{text}");
        assert_eq!(text.matches("rotate").count(), 1, "{text}");
    }

    #[test]
    fn text_stroke_is_the_compat_standards_two_properties() {
        let t = |s: &str| StyleValue::Text(s.into());
        let text = css(
            &[
                (StyleId::TextStrokeWidth, t("2px #ff0000")),
                (StyleId::TextStrokeColor, t("2px #ff0000")),
            ],
            &[],
        );
        assert!(text.contains("-webkit-text-stroke-width:2px;"), "{text}");
        assert!(text.contains("-webkit-text-stroke-color:"), "{text}");
    }

    #[test]
    fn apple_continuous_is_a_superellipse_over_a_scaled_radius() {
        let t = |s: &str| StyleValue::Text(s.into());
        let text = css(
            &[
                (StyleId::BorderRadiusTopLeft, StyleValue::Number(10.0)),
                (StyleId::BorderRadiusTopRight, StyleValue::Number(10.0)),
                (StyleId::CornerShape, t("-apple-continuous squircle")),
            ],
            &[],
        );
        assert!(
            text.contains("border-top-left-radius:calc(10px * 1.52);"),
            "{text}"
        );
        assert!(text.contains("border-top-right-radius:10px;"), "{text}");
        assert!(
            text.contains("corner-shape:superellipse(1.6) squircle superellipse(1.6) squircle;"),
            "{text}"
        );
    }

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
        // LLP 1008: the admitted snap alignments are CSS's own.
        assert_eq!(
            css(&[(StyleId::ScrollSnapAlign, t("end"))], &[]),
            "scroll-snap-align:end;"
        );
        // LLP 1066 D7: the browser's own `fixed`, the gradient box the viewport.
        assert_eq!(
            css(&[(StyleId::BackgroundAttachment, t("fixed"))], &[]),
            "background-attachment:fixed;"
        );
    }

    /// The feedback's separate factor leaves the row's scale and
    /// authored transition list intact; 1 needs no effect.
    #[test]
    fn a_press_scale_keeps_the_authored_scale_and_transitions() {
        let n = StyleValue::Number;
        let t = |s: &str| StyleValue::Text(s.into());
        assert_eq!(
            css(&[(StyleId::PressScale, n(0.97))], &[]),
            "scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.97;"
        );
        assert_eq!(
            css(
                &[
                    (StyleId::Scale, n(1.5)),
                    (StyleId::Transition, t("all 200ms ease")),
                    (StyleId::PressScale, n(0.994)),
                ],
                &[]
            ),
            "--exact-scale:1.5;transition:all 0.2s ease 0s,scale 0s,--exact-scale 0.2s ease 0s;scale:calc(var(--exact-scale,1) * var(--exact-press-factor,1))!important;--exact-press:0.994;"
        );
        assert_eq!(css(&[(StyleId::PressScale, n(1.0))], &[]), "");
        assert_eq!(
            css(
                &[
                    (StyleId::Transition, t("opacity 1s")),
                    (StyleId::PressScale, n(1.0))
                ],
                &[]
            ),
            "transition:opacity 1s ease 0s;"
        );
    }
    /// Only a pressable node's scale animation also animates
    /// `--exact-scale`, under a rule of its own: an animated custom property
    /// keeps Chrome from compositing the animation (a pan/zoom re-rastered
    /// its picture every frame).
    #[test]
    fn only_a_pressable_node_animates_the_scale_custom_property() {
        use exact_motion::animation::{Animations, Keyframes};
        let mut list = Animations::parse("grow 1s linear infinite, fade 2s").unwrap();
        list.0[0].keyframes =
            Keyframes::parse("from{scale:1;translate:0px 0px}to{scale:1.6}").unwrap();
        list.0[1].keyframes = Keyframes::parse("to{opacity:0}").unwrap();
        let (grow, fade) = (&list.0[0], &list.0[1]);
        assert_eq!(keyframes_name(grow, false), "grow");
        assert!(
            !keyframes_css(grow, false).contains("--exact"),
            "{}",
            keyframes_css(grow, false)
        );
        assert_eq!(keyframes_name(grow, true), "grow-exact-press");
        assert!(keyframes_css(grow, true).contains("scale:1.6;--exact-scale:1.6;"));
        // A pressable node's animation that leaves `scale` alone keeps its rule.
        assert_eq!(keyframes_name(fade, true), "fade");
        assert_eq!(animations_css(&list, false), list.css());
        assert_eq!(
            animations_css(&list, true),
            list.css().replace(" grow,", " grow-exact-press,")
        );
    }
}
