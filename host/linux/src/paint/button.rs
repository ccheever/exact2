//! Linux's synchronous native button look and face layout. LLP 1069.011.001 D1–D17.
use super::{control, rgba, Painter, Rect4, Shape};
use crate::text::{Paragraph, TextEngine};
use exact_kernel::{
    AxisOffer, ButtonFaceStyle, ButtonImagePlacement, ButtonMeasure, ButtonMeasureRequest,
    ControlCornerStyle, ControlSize, Dimension, NodeRef, PropId, StyleId, StyleProps, TextAlign,
    WhiteSpace,
};
use std::rc::Rc;
use tiny_skia::Transform;

pub(crate) fn look(name: &str) -> &'static str {
    exact_kernel::generated::button_style(name).map_or("ua", |s| s.look)
}

fn typography(style: &StyleProps, scale: f32) -> StyleProps {
    let mut out = style.clone();
    // Linux's control typography is independent of an ancestor's font.
    out.font_family = 0;
    out.font_style = exact_kernel::FontStyle::Normal;
    out.line_height = exact_kernel::LineHeight::Normal;
    out.letter_spacing = 0.0;
    if !style.mask.has(StyleId::FontSize) {
        out.font_size = 16.0 * scale;
    }
    if !style.mask.has(StyleId::FontWeight) {
        out.font_weight = 400;
    }
    if !style.mask.has(StyleId::WhiteSpace) {
        out.white_space = WhiteSpace::Normal;
    }
    if !style.mask.has(StyleId::TextAlign) {
        out.text_align = TextAlign::Center;
    }
    out
}

struct FaceLayout {
    title: Option<Rc<Paragraph>>,
    subtitle: Option<Rc<Paragraph>>,
    title_style: StyleProps,
    subtitle_style: Option<StyleProps>,
    symbol_style: StyleProps,
    padding: [f32; 4],
    text: (f32, f32),
    symbol: f32,
    gap: f32,
    column: bool,
    width: f32,
    height: f32,
}

fn face_layout(
    engine: &mut TextEngine,
    face: &exact_kernel::PressFace,
    rows: &ButtonFaceStyle,
    width: AxisOffer,
) -> FaceLayout {
    let scale = match rows.button.control_size {
        ControlSize::Mini => 0.75,
        ControlSize::Small => 0.875,
        ControlSize::Medium => 1.0,
        ControlSize::Large => 1.125,
    };
    let title_style = typography(&rows.title, scale);
    let subtitle_style = rows.subtitle.as_ref().map(|s| {
        let mut s = typography(s, scale);
        if !s.mask.has(StyleId::FontSize) {
            s.font_size = title_style.font_size * 0.8;
        }
        s
    });
    let mut symbol_style = rows.symbol.clone();
    if !symbol_style.mask.has(StyleId::FontSize) {
        symbol_style.font_size = title_style.font_size;
    }
    if !symbol_style.mask.has(StyleId::FontWeight) {
        symbol_style.font_weight = title_style.font_weight;
    }
    let column = matches!(
        face.placement,
        ButtonImagePlacement::Top | ButtonImagePlacement::Bottom
    );
    let offer = match width {
        AxisOffer::Definite(w) => Some(w),
        AxisOffer::MinContent => Some(0.0),
        AxisOffer::MaxContent => None,
    };
    let length = |d: Dimension| match d {
        Dimension::Points(n) => n,
        Dimension::Percent(n) => offer.unwrap_or(0.0) * n / 100.0,
        Dimension::Calc(n, p) => n + offer.unwrap_or(0.0) * p / 100.0,
        _ => 0.0,
    };
    let padding = [
        (StyleId::PaddingTop, rows.button.padding_top, 7.0),
        (StyleId::PaddingRight, rows.button.padding_right, 9.0),
        (StyleId::PaddingBottom, rows.button.padding_bottom, 7.0),
        (StyleId::PaddingLeft, rows.button.padding_left, 9.0),
    ]
    .map(|(id, d, default)| {
        if rows.button.mask.has(id) {
            length(d)
        } else {
            default * scale
        }
    });
    let symbol = if face.symbol.is_some() {
        symbol_style.font_size
    } else {
        0.0
    };
    let gap = if symbol > 0.0 && (face.title.is_some() || face.subtitle.is_some()) {
        rows.image_gap.unwrap_or(title_style.font_size * 0.5)
    } else {
        0.0
    };
    let natural = [
        (face.title.as_deref(), Some(&title_style)),
        (face.subtitle.as_deref(), subtitle_style.as_ref()),
    ]
    .into_iter()
    .filter_map(|(text, style)| text.zip(style))
    .map(|(t, s)| engine.paragraph(&super::text_spec(s, t), None).width)
    .fold(0.0f32, f32::max);
    let room = offer
        .map(|w| (w - padding[1] - padding[3] - if column { 0.0 } else { symbol + gap }).max(0.0));
    let room = room.map(|room| room.min(natural));
    let mut paragraph = |text: Option<&str>, style: &StyleProps| {
        text.map(|t| {
            let p = engine.paragraph(&super::text_spec(style, t), room);
            if style.white_space == WhiteSpace::Nowrap {
                room.and_then(|w| p.ellipsized(w)).unwrap_or(p)
            } else {
                p
            }
        })
    };
    let title = paragraph(face.title.as_deref(), &title_style);
    let subtitle = subtitle_style
        .as_ref()
        .and_then(|s| paragraph(face.subtitle.as_deref(), s));
    let extent = title
        .as_ref()
        .map_or(0.0, |p| p.width)
        .max(subtitle.as_ref().map_or(0.0, |p| p.width));
    let tw = room.filter(|w| *w > 0.0).unwrap_or(extent);
    let th = title.as_ref().map_or(0.0, |p| p.height) + subtitle.as_ref().map_or(0.0, |p| p.height);
    let (w, h) = if column {
        (tw.max(symbol), th + symbol + gap)
    } else {
        (tw + symbol + gap, th.max(symbol))
    };
    FaceLayout {
        title,
        subtitle,
        title_style,
        subtitle_style,
        symbol_style,
        padding,
        text: (tw, th),
        symbol,
        gap,
        column,
        width: offer
            .filter(|_| width != AxisOffer::MinContent)
            .map_or(w + padding[1] + padding[3], |off| {
                (w + padding[1] + padding[3]).min(off)
            }),
        height: h + padding[0] + padding[2],
    }
}

pub(crate) fn measure(engine: &mut TextEngine, request: &ButtonMeasureRequest) -> ButtonMeasure {
    let face = face_layout(engine, &request.face, &request.style, request.width);
    ButtonMeasure {
        width: face.width,
        height: face.height,
        provisional: false,
    }
}

impl Painter {
    pub(super) fn button_control(
        &mut self,
        node: &NodeRef<'_>,
        rect: Rect4,
        ts: Transform,
        face: &exact_kernel::PressFace,
        rows: &ButtonFaceStyle,
        focused: bool,
    ) {
        let dark = self.dark;
        let disabled = node.props.bool(PropId::Disabled) == Some(true);
        let dim = |mut c: [u8; 4]| {
            if disabled {
                c[3] = (c[3] as f32 * 0.45) as u8;
            }
            c
        };
        let authored_accent = control::accent(node, dark);
        let accent = authored_accent.unwrap_or(control::ACCENT);
        let accent_paint = |c| if authored_accent.is_some() { c } else { dim(c) };
        let soft = |a| [accent[0], accent[1], accent[2], a];
        let label = if dark { [255; 4] } else { [0, 0, 0, 255] };
        let name = node.props.str(PropId::ButtonStyle).unwrap_or("bordered");
        let look = look(name);
        let radii = [
            (
                StyleId::BorderRadiusTopLeft,
                rows.button.border_radius_top_left,
            ),
            (
                StyleId::BorderRadiusTopRight,
                rows.button.border_radius_top_right,
            ),
            (
                StyleId::BorderRadiusBottomRight,
                rows.button.border_radius_bottom_right,
            ),
            (
                StyleId::BorderRadiusBottomLeft,
                rows.button.border_radius_bottom_left,
            ),
        ]
        .map(|(id, radius)| {
            if rows.button.mask.has(id) {
                match radius {
                    Dimension::Points(n) => n,
                    Dimension::Percent(n) => rect.2.min(rect.3) * n / 100.0,
                    Dimension::Calc(n, p) => n + rect.2.min(rect.3) * p / 100.0,
                    _ => 0.0,
                }
            } else {
                match rows.button.control_corner_style {
                    ControlCornerStyle::Capsule => rect.3 / 2.0,
                    ControlCornerStyle::Small => 4.0,
                    ControlCornerStyle::Medium => 6.0,
                    ControlCornerStyle::Large => 10.0,
                    ControlCornerStyle::Dynamic if look.contains("glass") => rect.3 / 2.0,
                    ControlCornerStyle::Dynamic => 6.0,
                }
            }
        });
        let shape = Shape::new(rect, radii);
        let ink = match look {
            "text" => accent,
            "soft" => {
                self.backend.fill(&shape, accent_paint(soft(38)), ts);
                accent
            }
            "fill" | "glass-fill" => {
                self.backend.fill(&shape, accent_paint(accent), ts);
                [255; 4]
            }
            "glass" => {
                self.backend.fill(
                    &shape,
                    dim(if dark {
                        [38, 38, 41, 179]
                    } else {
                        [255, 255, 255, 179]
                    }),
                    ts,
                );
                label
            }
            _ => {
                self.backend.fill(
                    &shape,
                    dim(if dark {
                        [72, 72, 74, 255]
                    } else {
                        [198, 198, 200, 255]
                    }),
                    ts,
                );
                self.backend.fill(
                    &shape.inset(1.0),
                    dim(if dark {
                        [44, 44, 46, 255]
                    } else {
                        [239, 239, 239, 255]
                    }),
                    ts,
                );
                label
            }
        };
        let layout = face_layout(
            &mut self.text.borrow_mut(),
            face,
            rows,
            AxisOffer::Definite(rect.2),
        );
        let (x, y, w, h) = rect;
        let [pt, pr, pb, pl] = layout.padding;
        let (cw, ch) = if layout.column {
            (
                layout.text.0.max(layout.symbol),
                layout.text.1 + layout.symbol + layout.gap,
            )
        } else {
            (
                layout.text.0 + layout.symbol + layout.gap,
                layout.text.1.max(layout.symbol),
            )
        };
        let free = (w - pl - pr - cw).max(0.0);
        let align = layout
            .title_style
            .text_align
            .physical(layout.title_style.direction);
        let bx = x
            + pl
            + match align {
                TextAlign::Left | TextAlign::Start => 0.0,
                TextAlign::Right | TextAlign::End => free,
                _ => free / 2.0,
            };
        let by = y + pt + ((h - pt - pb - ch) / 2.0).max(0.0);
        let leading = matches!(
            face.placement,
            ButtonImagePlacement::Leading | ButtonImagePlacement::Top
        );
        let (tx, ty, sx, sy) = if layout.column {
            (
                bx + (cw - layout.text.0) / 2.0,
                by + if leading {
                    layout.symbol + layout.gap
                } else {
                    0.0
                },
                bx + (cw - layout.symbol) / 2.0,
                by + if leading {
                    0.0
                } else {
                    layout.text.1 + layout.gap
                },
            )
        } else {
            (
                bx + if leading {
                    layout.symbol + layout.gap
                } else {
                    0.0
                },
                by + (ch - layout.text.1) / 2.0,
                bx + if leading {
                    0.0
                } else {
                    layout.text.0 + layout.gap
                },
                by + (ch - layout.symbol) / 2.0,
            )
        };
        let color = |style: &StyleProps, fallback| {
            if style.mask.has(StyleId::TextColor) {
                rgba(style.text_color.resolve(dark))
            } else if authored_accent.is_some() {
                fallback
            } else {
                dim(fallback)
            }
        };
        self.backend.push_clip(&Shape::rect(rect), ts);
        if let Some(p) = &layout.title {
            let palette = [crate::text::RunPaint {
                color: color(&layout.title_style, ink),
                source: node.id,
            }];
            self.backend
                .text(&mut self.text.borrow_mut(), p, &palette, (tx, ty), ts);
        }
        if let (Some(p), Some(style)) = (&layout.subtitle, &layout.subtitle_style) {
            let palette = [crate::text::RunPaint {
                color: color(style, [ink[0], ink[1], ink[2], 180]),
                source: node.id,
            }];
            self.backend.text(
                &mut self.text.borrow_mut(),
                p,
                &palette,
                (tx, ty + layout.title.as_ref().map_or(0.0, |p| p.height)),
                ts,
            );
        }
        if let Some(symbol) = &face.symbol {
            let tint = if layout.symbol_style.mask.has(StyleId::TintColor) {
                layout
                    .symbol_style
                    .tint_color
                    .map(|c| rgba(c.resolve(dark)))
                    .unwrap_or(ink)
            } else {
                color(&layout.title_style, ink)
            };
            self.symbol_face(
                symbol,
                &layout.symbol_style,
                (sx, sy, layout.symbol, layout.symbol),
                tint,
                ts,
            );
        }
        self.backend.pop_clip();
        if focused && !disabled {
            self.field_ring(&shape, accent, ts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ButtonMeasureRequest {
        ButtonMeasureRequest {
            face: exact_kernel::PressFace {
                title: Some("A native title with several words to wrap".into()),
                fits: true,
                ..Default::default()
            },
            style: ButtonFaceStyle {
                title: StyleProps::default(),
                subtitle: None,
                symbol: StyleProps::default(),
                button: StyleProps::default(),
                image_gap: None,
            },
            button_style: "bordered".into(),
            width: AxisOffer::MaxContent,
        }
    }
    #[test]
    fn synchronous_height_for_width_wraps_clamps_and_counts_padding_once() {
        let engine = TextEngine::shared();
        let mut engine = engine.borrow_mut();
        let mut r = request();
        let natural = measure(&mut engine, &r);
        assert!(!natural.provisional);
        r.width = AxisOffer::Definite(120.0);
        let wrapped = measure(&mut engine, &r);
        assert!(wrapped.height > natural.height);
        r.style.title.line_clamp = 1;
        let clamped = measure(&mut engine, &r);
        assert!(clamped.height < wrapped.height);
        r.width = AxisOffer::MaxContent;
        r.style.title.line_clamp = 0;
        let before = measure(&mut engine, &r);
        for id in [StyleId::PaddingTop, StyleId::PaddingBottom] {
            r.style
                .button
                .set_dynamic(id, &exact_kernel::StyleValue::Number(20.0))
                .unwrap();
        }
        let after = measure(&mut engine, &r);
        assert_eq!(after.height - before.height, 26.0);
    }
    #[test]
    fn semantic_subtitle_symbol_placement_sizes_and_authored_axes_share_the_measure() {
        let engine = TextEngine::shared();
        let mut engine = engine.borrow_mut();
        let mut r = request();
        r.face.title = Some("Lock".into());
        let title = measure(&mut engine, &r);
        r.face.subtitle = Some("Protected".into());
        r.style.subtitle = Some(StyleProps::default());
        let subtitle = measure(&mut engine, &r);
        assert!(subtitle.height > title.height);
        r.face.symbol = Some("lock".into());
        let leading = measure(&mut engine, &r);
        assert!(leading.width > subtitle.width);
        r.face.placement = ButtonImagePlacement::Top;
        let top = measure(&mut engine, &r);
        assert!(top.height > leading.height);
        r.style.button.control_size = ControlSize::Large;
        let large = measure(&mut engine, &r);
        assert!(large.height > top.height);
        r.style
            .symbol
            .set_dynamic(StyleId::FontSize, &exact_kernel::StyleValue::Number(40.0))
            .unwrap();
        assert!(measure(&mut engine, &r).height > large.height);
    }
}
