//! Linux's synchronous native button look and face layout. LLP 1069.011.001 D1–D17.
use super::{control, look, rgba, Painter, Rect4, Shape};
use crate::text::{Paragraph, TextEngine};
use exact_kernel::generated::metric;
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

/// The neutral look's button type (LLP 1115.000 D17), as the page's sheet
/// sets it: the size the button inherits, or the one its `controlSize`
/// names (what a page writes for it at a 16 px root: the subheadline's 14
/// for mini and small, the body's 16 for medium, 18 for large), at weight
/// 500 on a line 1.25 of it. A size or weight the author wrote is theirs.
fn neutral_typography(style: &StyleProps, size: Option<f32>) -> StyleProps {
    let mut out = style.clone();
    out.font_family = 0;
    out.font_style = exact_kernel::FontStyle::Normal;
    out.line_height = exact_kernel::LineHeight::Number(look::number(metric::CONTROL_LINE_HEIGHT));
    out.letter_spacing = 0.0;
    if let Some(size) = size.filter(|_| !style.mask.has(StyleId::FontSize)) {
        out.font_size = size;
    }
    if !style.mask.has(StyleId::FontWeight) {
        out.font_weight = look::number(metric::CONTROL_WEIGHT) as u16;
    }
    if !style.mask.has(StyleId::WhiteSpace) {
        out.white_space = WhiteSpace::Normal;
    }
    if !style.mask.has(StyleId::TextAlign) {
        out.text_align = TextAlign::Center;
    }
    out
}

/// Whether a button of `style` is Material 3's filled button (`Button`; LLP
/// 1115.001 D9: `filled` and the prominent styles): under the Material look,
/// a `fill` style. Its metrics are `ButtonDefaults`', the `filled-button-`
/// rows; its type label-large; its shape a pill.
pub(crate) fn material_filled(style: Option<&str>) -> bool {
    look::material() && matches!(look(style.unwrap_or("bordered")), "fill" | "glass-fill")
}

/// A length of the Material look, dp.
fn material_dp(index: usize) -> f32 {
    exact_kernel::style::metrics::length_in(
        index,
        exact_kernel::style::metrics::Column::Material,
        0.0,
        0.0,
    )
    .expect("a Material row")
}

/// A number of the Material look.
fn material_number(index: usize) -> f32 {
    exact_kernel::style::metrics::number_in(index, exact_kernel::style::metrics::Column::Material)
        .expect("a Material row")
}

/// Material's label-large (14 sp on a 20 sp line, weight 500, 0.1 sp
/// tracking) at the control's starting size, 16 sp: 14 of it, by the
/// `controlSize`'s `scale`. What the author wrote wins.
fn material_typography(style: &StyleProps, scale: f32) -> StyleProps {
    let mut out = style.clone();
    out.font_family = 0;
    out.font_style = exact_kernel::FontStyle::Normal;
    out.line_height = exact_kernel::LineHeight::Number(20.0 / 14.0);
    out.letter_spacing = 0.1 * style.font_size / 16.0;
    if !style.mask.has(StyleId::FontSize) {
        out.font_size = style.font_size * 14.0 / 16.0 * scale;
    }
    if !style.mask.has(StyleId::FontWeight) {
        out.font_weight = 500;
    }
    if !style.mask.has(StyleId::WhiteSpace) {
        out.white_space = WhiteSpace::Normal;
    }
    if !style.mask.has(StyleId::TextAlign) {
        out.text_align = TextAlign::Center;
    }
    out
}

/// Geometry resolved by `ButtonFaceStyle::resolve_geometry` before host layout.
fn points(d: Dimension) -> f32 {
    match d {
        Dimension::Points(n) => n,
        _ => 0.0,
    }
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
    leading: bool,
    width: f32,
    height: f32,
}

fn face_layout(
    engine: &mut TextEngine,
    face: &exact_kernel::PressFace,
    rows: &ButtonFaceStyle,
    width: AxisOffer,
    filled: bool,
    bordered: bool,
) -> FaceLayout {
    let neutral = look::neutral();
    let tab_root = rows.neutral_tab_root;
    let tab_length = |id, em| {
        exact_kernel::style::metrics::length_in(
            id,
            exact_kernel::style::metrics::Column::Neutral,
            em,
            tab_root.expect("admitted neutral tab"),
        )
        .expect("neutral tab metric")
    };
    let declared = |s: &StyleProps, id| s.mask.union(s.written).has(id);
    let scale = match rows.button.control_size {
        ControlSize::Mini => 0.75,
        ControlSize::Small => 0.875,
        ControlSize::Medium => 1.0,
        ControlSize::Large => 1.125,
    };
    let said_size =
        rows.button
            .mask
            .has(StyleId::ControlSize)
            .then_some(match rows.button.control_size {
                ControlSize::Mini | ControlSize::Small => 14.0,
                ControlSize::Medium => 16.0,
                ControlSize::Large => 18.0,
            });
    let type_of = |s: &StyleProps| {
        if filled {
            material_typography(s, scale)
        } else if neutral {
            let mut style =
                neutral_typography(s, if tab_root.is_some() { None } else { said_size });
            if tab_root.is_some()
                && !declared(s, StyleId::FontSize)
                && !rows.button.written.has(StyleId::FontSize)
            {
                style.font_size = tab_length(metric::TABBAR_FONT, style.font_size);
            }
            style
        } else {
            typography(s, scale)
        }
    };
    let title_style = type_of(&rows.title);
    let subtitle_style = rows.subtitle.as_ref().map(|s| {
        let mut s = type_of(s);
        if !s.mask.has(StyleId::FontSize)
            && !(tab_root.is_some() && s.written.has(StyleId::FontSize))
        {
            s.font_size = title_style.font_size * 0.8;
        }
        s
    });
    let mut symbol_style = rows.symbol.clone();
    if !symbol_style.mask.has(StyleId::FontSize)
        && !(tab_root.is_some() && symbol_style.written.has(StyleId::FontSize))
    {
        symbol_style.font_size = title_style.font_size;
    }
    if tab_root.is_some() && !declared(&rows.symbol, StyleId::FontSize) {
        symbol_style.font_size = tab_length(metric::TABBAR_SYMBOL, title_style.font_size);
    }
    if !symbol_style.mask.has(StyleId::FontWeight) {
        symbol_style.font_weight = title_style.font_weight;
    }
    let mut column = matches!(
        face.placement,
        ButtonImagePlacement::Top | ButtonImagePlacement::Bottom
    );
    let leading = matches!(
        face.placement,
        ButtonImagePlacement::Leading | ButtonImagePlacement::Top
    );
    if tab_root.is_some() && !declared(&rows.button, StyleId::FlexDirection) {
        column = leading;
    }
    let offer = match width {
        AxisOffer::Definite(w) => Some(w),
        AxisOffer::MinContent => Some(0.0),
        AxisOffer::MaxContent => None,
    };
    // The ordinary neutral control's size removes its default padding/floor
    // on that axis. The more specific narrow-tab rule keeps its rem padding
    // and minimum until those properties themselves are authored.
    let em = title_style.font_size;
    let said = |id| (neutral || filled) && rows.button.written.has(id);
    let (said_width, said_height) = (said(StyleId::Width), said(StyleId::Height));
    let padding_block = neutral
        && !rows.neutral_role_floor
        && (said(StyleId::PaddingTop) || said(StyleId::PaddingBottom));
    let padding_inline = neutral
        && !rows.neutral_role_floor
        && (said(StyleId::PaddingLeft) || said(StyleId::PaddingRight));
    let lone_symbol =
        (face.symbol.is_some() || face.raster) && face.title.is_none() && face.subtitle.is_none();
    let inline = if tab_root.is_some() {
        tab_length(metric::TABBAR_PADDING_INLINE, em)
    } else if filled {
        material_dp(metric::FILLED_BUTTON_PADDING_INLINE) * scale
    } else if said_width {
        0.0
    } else if lone_symbol {
        look::px(metric::BUTTON_PADDING_INLINE_SYMBOL, em)
    } else {
        look::px(metric::BUTTON_PADDING_INLINE, em)
    };
    let block = if tab_root.is_some() {
        tab_length(metric::TABBAR_PADDING_BLOCK, em)
    } else if filled {
        material_dp(metric::FILLED_BUTTON_PADDING_BLOCK) * scale
    } else if said_height {
        0.0
    } else {
        look::px(metric::BUTTON_PADDING_BLOCK, em)
    };
    let edge = if tab_root.is_some() {
        0.0
    } else if neutral {
        look::px(metric::CONTROL_LINE, em)
    } else {
        0.0
    };
    let padding = [
        (StyleId::PaddingTop, rows.button.padding_top, 7.0, block),
        (
            StyleId::PaddingRight,
            rows.button.padding_right,
            9.0,
            inline,
        ),
        (
            StyleId::PaddingBottom,
            rows.button.padding_bottom,
            7.0,
            block,
        ),
        (StyleId::PaddingLeft, rows.button.padding_left, 9.0, inline),
    ]
    .map(|(id, d, default, look)| {
        let edge = if !bordered
            && match id {
                StyleId::PaddingTop | StyleId::PaddingBottom => padding_block,
                _ => padding_inline,
            } {
            0.0
        } else {
            edge
        };
        edge + if rows.button.mask.has(id) || (tab_root.is_some() && rows.button.written.has(id)) {
            points(d)
        } else if neutral || filled {
            look
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
        if tab_root.is_some() {
            let (id, own_gap) = if column {
                (StyleId::RowGap, rows.button.row_gap)
            } else {
                (StyleId::ColumnGap, rows.button.column_gap)
            };
            if declared(&rows.button, id) {
                own_gap
            } else if column {
                tab_length(metric::TABBAR_GAP, em)
            } else {
                0.0
            }
        } else {
            rows.image_gap.unwrap_or_else(|| {
                if neutral {
                    // The page's gap is a space in the button's type.
                    let mut wide = |text| {
                        engine
                            .paragraph(&super::text_spec(&title_style, text), None)
                            .width
                    };
                    (wide("a a") - wide("aa")).max(0.0)
                } else {
                    title_style.font_size * 0.5
                }
            })
        }
    } else {
        0.0
    };
    let floor = |said: bool| {
        if neutral && !said {
            look::px(metric::CONTROL_HEIGHT, em)
        } else if filled && !said {
            material_dp(metric::FILLED_BUTTON_HEIGHT) * scale
        } else {
            0.0
        }
    };
    let min_width = match filled && !said_width {
        true => material_dp(metric::FILLED_BUTTON_MIN_WIDTH) * scale,
        false => floor(said_width || padding_inline),
    };
    let min_height = if tab_root.is_some() {
        if declared(&rows.button, StyleId::MinHeight) {
            0.0
        } else {
            tab_length(metric::TABBAR_HEIGHT, em)
        }
    } else {
        floor(said_height || padding_block)
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
        leading,
        width: offer
            .filter(|_| width != AxisOffer::MinContent)
            .map_or((w + padding[1] + padding[3]).max(min_width), |off| {
                (w + padding[1] + padding[3]).max(min_width).min(off)
            }),
        height: (h + padding[0] + padding[2]).max(min_height),
    }
}

pub(crate) fn measure(engine: &mut TextEngine, request: &ButtonMeasureRequest) -> ButtonMeasure {
    let filled = material_filled(request.button_style.as_deref());
    let face = face_layout(
        engine,
        &request.face,
        &request.style,
        request.width,
        filled,
        request.button_style.as_deref().unwrap_or("bordered") == "bordered",
    );
    ButtonMeasure {
        width: face.width,
        height: face.height,
        provisional: false,
    }
}

impl Painter {
    /// Whether `id` shows pressed: a contact's press or a held one.
    pub(crate) fn presses(&self, id: exact_kernel::ViewId) -> bool {
        self.feedback.1 == Some(id) || self.held_press == Some(id)
    }

    /// The neutral look's button under its title (LLP 1115.000 D17), and the
    /// ink the title takes: emphasis is a fill, and a state adds one thing.
    /// The unsaid button and `bordered` are an outline in the label's ink and
    /// `gray` a neutral fill; `fill` is the accent under the ink that reads
    /// on it; `soft` and `text` are the accent as ink, on a tint of it or on
    /// nothing. A destructive button's accent is the red. Where a pointer
    /// hovers a wash of the ink lies over the fill, and a stronger one while
    /// pressed, but for `fill`, which shows UIKit's highlight (neutral.css);
    /// disabled, the ink is the grey and an accent fill a neutral.
    fn neutral_face(
        &mut self,
        node: &NodeRef<'_>,
        shape: &Shape,
        style: &str,
        gray: bool,
        ts: Transform,
    ) -> [u8; 4] {
        use look::{fade, number, role};
        let dark = self.dark;
        let disabled = node.props.bool(PropId::Disabled) == Some(true);
        let destructive = node.props.bool(PropId::Destructive) == Some(true);
        let authored = control::accent(node, dark);
        let accent = match authored {
            None if destructive => role("-exact-system-red", dark),
            _ => look::accents(authored, dark).0,
        };
        let (hover, pressed) = (
            !disabled && self.feedback.0 == Some(node.id),
            !disabled && self.presses(node.id),
        );
        let label = role("-exact-label", dark);
        if node.is_neutral_tab()
            && matches!(
                node.props.str(PropId::ButtonStyle).unwrap_or("bordered"),
                "bordered" | "gray"
            )
        {
            return if disabled {
                role("GrayText", dark)
            } else if node.props.bool(PropId::AccessibilitySelected) == Some(true) {
                look::accents(authored, dark).0
            } else if hover {
                label
            } else {
                role("-exact-secondary-label", dark)
            };
        }
        // A wash of a role where the fill is neutral, of the ink where it is
        // the accent's.
        let neutral_wash = || match (pressed, hover) {
            (true, _) => Some(role("-exact-tertiary-fill", dark)),
            (_, true) => Some(role("-exact-quaternary-fill", dark)),
            _ => None,
        };
        let ink_wash = |ink: [u8; 4], pressed_row, hover_row| match (pressed, hover) {
            (true, _) => Some(fade(ink, number(pressed_row))),
            (_, true) => Some(fade(ink, number(hover_row))),
            _ => None,
        };
        let line = look::px(metric::CONTROL_LINE, super::neutral::em(node));
        let (fill, edge, wash, ink) = match style {
            "fill" if disabled => (Some(role("-exact-tertiary-fill", dark)), None, None, label),
            "fill" if pressed => {
                let alpha = number(metric::BUTTON_PRESSED_ALPHA);
                let ink = fade(look::ink_on(accent), alpha);
                if dark {
                    let white = fade([255; 4], number(metric::BUTTON_PRESSED_LIGHTEN));
                    (Some(accent), None, Some(white), ink)
                } else {
                    (Some(fade(accent, alpha)), None, None, ink)
                }
            }
            "fill" => {
                let ink = look::ink_on(accent);
                let wash = hover.then(|| fade(ink, number(metric::BUTTON_HOVER_FILLED)));
                (Some(accent), None, wash, ink)
            }
            "soft" if disabled => (Some(role("-exact-tertiary-fill", dark)), None, None, label),
            "soft" => {
                let wash = ink_wash(
                    accent,
                    metric::BUTTON_PRESSED_TINTED,
                    metric::BUTTON_HOVER_TINTED,
                );
                let tint = fade(accent, number(metric::BUTTON_TINT_ALPHA));
                (Some(tint), None, wash, accent)
            }
            "text" => (None, None, neutral_wash(), accent),
            _ => {
                let ink = if destructive { accent } else { label };
                let fill = gray.then(|| role("-exact-secondary-fill", dark));
                let edge = (!gray).then(|| {
                    if disabled {
                        role("-exact-separator", dark)
                    } else {
                        look::line(dark)
                    }
                });
                (fill, edge, neutral_wash(), ink)
            }
        };
        for color in [fill, wash].into_iter().flatten() {
            self.backend.fill(shape, color, ts);
        }
        let bar_button = node.props.str(PropId::ButtonStyle).unwrap_or("bordered") == "bordered"
            && !matches!(
                node.props.str(PropId::AccessibilityRole),
                Some("tab" | "radio")
            )
            && node.inside_neutral_header();
        if let Some(color) = edge.filter(|_| !bar_button) {
            super::neutral::edge(self.backend.as_mut(), shape, line, color, ts);
        }
        if disabled {
            role("GrayText", dark)
        } else {
            ink
        }
    }

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
        let neutral = look::neutral();
        let disabled = node.props.bool(PropId::Disabled) == Some(true);
        let filled = material_filled(node.props.str(PropId::ButtonStyle));
        // The neutral look's disabled button is the grey ink, not a fade;
        // Material's is its own colours (below).
        let dim = |mut c: [u8; 4]| {
            if disabled && !neutral && !filled {
                c[3] = (c[3] as f32 * 0.45) as u8;
            }
            c
        };
        let authored_accent = control::accent(node, dark);
        let (accent, on_accent) = control::accents(authored_accent, dark);
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
                points(radius)
            } else if filled
                && matches!(
                    rows.button.control_corner_style,
                    ControlCornerStyle::Dynamic | ControlCornerStyle::Capsule
                )
            {
                rect.3 / 2.0
            } else if neutral {
                // One shape for every style, and a capsule where one is said.
                match rows.button.control_corner_style {
                    ControlCornerStyle::Capsule => rect.3 / 2.0,
                    _ => look::px(metric::CONTROL_RADIUS, rows.button.font_size),
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
        // Compose lays its button out in whole pixels: Material's is drawn
        // with its edges on the pixel grid.
        let drawn = match filled {
            true => {
                let grid = |v, vertical| super::material::on_grid(v, ts, vertical, self.scale);
                let (x0, y0) = (grid(rect.0, false), grid(rect.1, true));
                let (x1, y1) = (grid(rect.0 + rect.2, false), grid(rect.1 + rect.3, true));
                (x0, y0, x1 - x0, y1 - y0)
            }
            false => rect,
        };
        let shape = Shape::new(drawn, radii);
        let ink = match look {
            // Material's filled button: `primary` under `onPrimary`, and
            // disabled `onSurface` under `onSurfaceVariant`, each at its
            // `filled-button-disabled-` row's opacity
            // (`ButtonDefaults.buttonColors`).
            _ if filled && disabled && authored_accent.is_none() => {
                let at = |role: &str, index: usize| {
                    let mut c = look::role(role, dark);
                    c[3] = (f32::from(c[3]) * material_number(index)).round() as u8;
                    c
                };
                let container = at("CanvasText", metric::FILLED_BUTTON_DISABLED_CONTAINER);
                self.backend.fill(&shape, container, ts);
                at(
                    "-exact-secondary-label",
                    metric::FILLED_BUTTON_DISABLED_LABEL,
                )
            }
            _ if filled => {
                self.backend.fill(&shape, accent, ts);
                on_accent
            }
            "glass" | "glass-fill" if neutral && disabled => {
                // The look merges clear and ordinary glass; the authored name
                // retains which material replaces a disabled prominent fill.
                let material = exact_kernel::generated::material(if name.contains("clear") {
                    "glass-clear"
                } else {
                    "glass"
                })
                .unwrap();
                self.backend.fill(
                    &shape,
                    if dark { material.dark } else { material.light },
                    ts,
                );
                look::role("GrayText", dark)
            }
            _ if neutral && !look.contains("glass") => {
                self.neutral_face(node, &shape, look, name == "gray", ts)
            }
            "text" => accent,
            "soft" => {
                self.backend.fill(&shape, accent_paint(soft(38)), ts);
                accent
            }
            "fill" | "glass-fill" => {
                self.backend.fill(&shape, accent_paint(accent), ts);
                on_accent
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
            filled,
            name == "bordered",
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
        let tab = rows.neutral_tab_root.is_some();
        let vertical_free = (h - pt - pb - ch) / 2.0;
        let by = y
            + pt
            + if tab && !layout.column {
                vertical_free
            } else {
                vertical_free.max(0.0)
            };
        let leading = layout.leading;
        // In an overconstrained tab grid the text track can shrink below its
        // line. The image keeps its size; the centered title is clipped by the
        // authored button box, as on the page.
        let text_room = if tab && layout.column {
            (h - pt - pb - layout.symbol - layout.gap)
                .max(0.0)
                .min(layout.text.1)
        } else {
            layout.text.1
        };
        let text_shift = (text_room - layout.text.1) / 2.0;
        let (tx, ty, sx, sy) = if layout.column {
            (
                bx + (cw - layout.text.0) / 2.0,
                by + text_shift
                    + if leading {
                        layout.symbol + layout.gap
                    } else {
                        0.0
                    },
                bx + (cw - layout.symbol) / 2.0,
                by + if leading { 0.0 } else { text_room + layout.gap },
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
            let painted = super::material::FieldText::painted(p, (tx, ty), ts);
            self.labels.label(node.id, painted);
            let palette = [crate::text::RunPaint {
                color: color(&layout.title_style, ink),
                source: node.id,
            }];
            self.backend
                .text(&mut self.text.borrow_mut(), p, &palette, (tx, ty), ts);
        }
        if let (Some(p), Some(style)) = (&layout.subtitle, &layout.subtitle_style) {
            let palette = [crate::text::RunPaint {
                color: color(
                    style,
                    [
                        ink[0],
                        ink[1],
                        ink[2],
                        (u16::from(ink[3]) * 180 / 255) as u8,
                    ],
                ),
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
                neutral_tab_root: None,
                neutral_role_floor: false,
                title: StyleProps::default(),
                subtitle: None,
                symbol: StyleProps::default(),
                button: StyleProps::default(),
                image_gap: None,
            },
            button_style: None,
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
