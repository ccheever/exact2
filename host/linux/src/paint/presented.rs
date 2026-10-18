//! What the motion engine says to paint over a node's style rows: the four
//! compositor values, an SVG shape's geometry (LLP 1055.000 D15), paint
//! motion's colours and shadow while they move (LLP 1055.000 D6, LLP 1062),
//! and a layout transition's box (LLP 1063).

use super::shadow::ShadowPaint;
use super::BoxPaint;
use exact_kernel::StyleProps;
use exact_motion::{Property, Value};
use tiny_skia::Transform;

/// A node's presentation values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presented {
    /// The view's resolved appearance, when supplied by the host.
    pub dark: Option<bool>,
    /// `translate`'s lengths, points.
    pub translate: (f32, f32),
    /// `translate`'s percentages of the border box (chess diary #4),
    /// resolved where the box is painted ([`Presented::translate_at`]).
    pub translate_percent: (f32, f32),
    /// Uniform.
    pub scale: f32,
    /// Host feedback, multiplied into scale without entering the motion engine.
    pub press: f32,
    /// How far a press shows its pressed paint, 0 to 1 (LLP 1061 D7): the
    /// painter mixes the node's background and its subtree's text toward
    /// its press colours by it.
    pub press_paint: f32,
    /// Degrees.
    pub rotate: f32,
    /// Zero to one.
    pub opacity: f32,
    /// An SVG shape's presented `r`, `stroke-dashoffset`, `cx`, `cy`, `x`, `y`, `rx`, `ry`.
    pub svg: [Option<f32>; 8],
    /// Presented paint while it moves (LLP 1055.000 D6, LLP 1062): one slot
    /// per [`Property::PAINT`]; `None` paints the row.
    pub colors: PaintValues,
    /// A layout transition's offset from the laid-out origin and scale of
    /// the laid-out size, `[dx, dy, sx, sy]` (LLP 1063).
    pub layout: [f32; 4],
    /// What the reader plays instead of the engine (`crate::host::lower`'s
    /// bits): painted at the underlying values, in a layer it moves.
    pub lowered: u8,
    /// Whether its `:hover` and `:focus-visible` looks hold (LLP 1121 D8).
    pub looks: [bool; 2],
}

impl Presented {
    /// Nothing moved.
    pub const IDENTITY: Presented = Presented {
        dark: None,
        translate: (0.0, 0.0),
        translate_percent: (0.0, 0.0),
        scale: 1.0,
        press: 1.0,
        press_paint: 0.0,
        rotate: 0.0,
        opacity: 1.0,
        svg: [None; 8],
        colors: PaintValues::NONE,
        layout: [0.0, 0.0, 1.0, 1.0],
        lowered: 0,
        looks: [false; 2],
    };

    /// The committed style's values (what the engine starts from).
    pub fn from_style(s: &StyleProps) -> Presented {
        Presented {
            dark: None,
            translate: (s.translate.x, s.translate.y),
            translate_percent: (s.translate_percent.x, s.translate_percent.y),
            scale: s.scale,
            press: 1.0,
            press_paint: 0.0,
            rotate: s.rotate,
            opacity: s.opacity,
            svg: [None; 8],
            colors: PaintValues::NONE,
            layout: Presented::IDENTITY.layout,
            lowered: 0,
            looks: [false; 2],
        }
    }

    /// The used translation of a `w` × `h` border box: its lengths plus
    /// its percentages of the box, as CSS resolves `translate`.
    pub fn translate_at(&self, w: f32, h: f32) -> (f32, f32) {
        let (px, py) = self.translate_percent;
        (
            self.translate.0 + px / 100.0 * w,
            self.translate.1 + py / 100.0 * h,
        )
    }

    pub(super) fn moves(&self) -> bool {
        self.translate != (0.0, 0.0)
            || self.translate_percent != (0.0, 0.0)
            || self.scale != 1.0
            || self.press != 1.0
            || self.rotate != 0.0
            || self.layout != Presented::IDENTITY.layout
    }

    /// The box `(x, y, w, h)` painted through its presentation: CSS's
    /// individual transforms about `origin`, then outermost the layout
    /// transition's offset (LLP 1063); its size is the surface's alone
    /// ([`Presented::surface`]).
    pub(super) fn transform(
        &self,
        (x, y, w, h): (f32, f32, f32, f32),
        (ox, oy): (f32, f32),
    ) -> Transform {
        let (cx, cy) = (x + ox, y + oy);
        let [dx, dy, ..] = self.layout;
        let (tx, ty) = self.translate_at(w, h);
        Transform::from_translate(cx + dx + tx, cy + dy + ty)
            .pre_rotate(self.rotate)
            .pre_scale(self.scale * self.press, self.scale * self.press)
            .pre_translate(-cx, -cy)
    }

    /// The box's surface — its background, border, shadow and the clip it
    /// puts on its children — at the size a layout transition shows, from
    /// its top-left corner. Its content and children keep the laid-out
    /// geometry: a growing card reveals its title, never squashes it.
    pub(super) fn surface(&self, (x, y, w, h): (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        let [.., sx, sy] = self.layout;
        (x, y, (w * sx).max(0.0), (h * sy).max(0.0))
    }
}

impl super::PaintedBox {
    /// Undo only this node's feedback in its own reference box. The painted
    /// matrix keeps the authored scale, rotation, origin and all ancestors.
    pub(crate) fn unpressed(mut self, origin: exact_kernel::svg::TransformOrigin) -> Self {
        if let Some((ts, rect)) = self.affine {
            let (ox, oy) = origin.resolve(rect.2, rect.3);
            let ts = ts
                .pre_translate(rect.0 + ox, rect.1 + oy)
                .pre_scale(1. / self.press, 1. / self.press)
                .pre_translate(-rect.0 - ox, -rect.1 - oy);
            self.affine = Some((ts, rect));
            self.rect = super::bbox(ts, rect);
            self.press = 1.;
        }
        self
    }
}

/// Paint motion's presented values, one slot per [`Property::PAINT`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintValues([Option<Value>; Property::PAINT.len()]);

impl PaintValues {
    /// None: every paint property shows its row.
    pub const NONE: PaintValues = PaintValues([None; Property::PAINT.len()]);

    fn slot(property: Property) -> usize {
        Property::PAINT
            .iter()
            .position(|p| *p == property)
            .expect("a paint property")
    }

    /// Paint `value` over the property's row, or the row again.
    pub fn set(&mut self, property: Property, value: Option<Value>) {
        self.0[Self::slot(property)] = value;
    }

    /// Whether anything is presented over the rows.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// A presented value, if one is.
    pub fn value(&self, property: Property) -> Option<Value> {
        self.0[Self::slot(property)]
    }

    /// A presented colour, straight 8-bit channels.
    pub fn color(&self, property: Property) -> Option<[u8; 4]> {
        self.0[Self::slot(property)].map(Value::to_rgba8)
    }
}

impl BoxPaint {
    /// The captured box with paint motion's values over its rows.
    pub(super) fn presented(mut self, paint: &PaintValues) -> BoxPaint {
        if paint.is_empty() {
            return self;
        }
        if let Some(c) = paint.color(Property::BackgroundColor) {
            self.background = c;
        }
        let sides = [
            Property::BorderTopColor,
            Property::BorderRightColor,
            Property::BorderBottomColor,
            Property::BorderLeftColor,
        ];
        for (color, side) in self.colors.iter_mut().zip(sides) {
            if let Some(c) = paint.color(side) {
                *color = c;
            }
        }
        let geometry = paint.0[PaintValues::slot(Property::BoxShadow)];
        let color = paint.color(Property::ShadowColor);
        if geometry.is_some() || color.is_some() {
            self.shadows = ShadowPaint::over(std::mem::take(&mut self.shadows), geometry, color);
        }
        self
    }

    /// The box with its pressed background (LLP 1061 D7) `progress` of the
    /// way over its own.
    pub(super) fn pressed(mut self, style: &StyleProps, progress: f32, dark: bool) -> BoxPaint {
        if progress > 0.0 && style.mask.has(exact_kernel::StyleId::PressBackgroundColor) {
            let pressed = super::rgba(style.press_background_color.resolve(dark));
            self.background = press_mix(self.background, pressed, progress);
        }
        self
    }
}

impl BoxPaint {
    /// The box in its states (LLP 1121 D8): its `:hover` then its
    /// `:focus-visible` look while they hold (`shown.looks`), at once, then
    /// the press's paint at its progress (LLP 1061 D7, which `:active`
    /// lowers to).
    pub(super) fn states(
        self,
        node: &exact_kernel::NodeRef<'_>,
        shown: &Presented,
        dark: bool,
    ) -> BoxPaint {
        let look = |state| {
            node.looks()
                .iter()
                .find(|(s, _)| *s == state)
                .map(|(_, l)| l)
        };
        // `currentcolor` in a look is the node's colour as shown (a colour
        // transition's), as in its rows.
        let computed = node.computed_row(exact_kernel::StyleId::TextColor, |s| s.text_color);
        let current = (shown.colors.color(Property::Color))
            .unwrap_or_else(|| super::rgba(computed.resolve(dark)));
        let mut paint = self;
        for (state, on) in [
            (exact_kernel::LOOK_HOVER, shown.looks[0]),
            (exact_kernel::LOOK_FOCUS_VISIBLE, shown.looks[1]),
        ] {
            if let Some(l) = look(state).filter(|_| on) {
                paint = paint.looked(l, current, dark);
            }
        }
        paint.pressed(node.style, shown.press_paint, dark)
    }

    /// The box with a look's rows in place of its own: its background, its
    /// border sides' colours and its shadow.
    fn looked(mut self, look: &StyleProps, current: [u8; 4], dark: bool) -> BoxPaint {
        use exact_kernel::StyleId::*;
        // A row it clears (`unset`) is written with no value: CSS's initial
        // one (a transparent background, a `currentcolor` side, no shadow).
        let names = |id| look.mask.union(look.written).has(id);
        let rgba = |c: Option<exact_kernel::ColorValue>| {
            c.map_or(current, |c| super::rgba(c.resolve(dark)))
        };
        if names(BackgroundColor) {
            self.background = match look.mask.has(BackgroundColor) {
                true => rgba(look.background_color),
                false => [0; 4],
            };
        }
        let sides = [
            (BorderColorTop, look.border_color_top),
            (BorderColorRight, look.border_color_right),
            (BorderColorBottom, look.border_color_bottom),
            (BorderColorLeft, look.border_color_left),
        ];
        for (color, (id, side)) in self.colors.iter_mut().zip(sides) {
            if names(id) {
                *color = rgba(side.filter(|_| look.mask.has(id)));
            }
        }
        if names(BoxShadow) {
            self.shadows = match look.mask.has(BoxShadow) {
                true => ShadowPaint::capture(look, dark),
                false => Vec::new(),
            };
        }
        self
    }
}

/// `to` mixed `t` of the way over `from`, straight 8-bit channels, as CSS
/// interpolates colours: premultiplied, so a transparent end fades rather
/// than darkens.
pub(super) fn press_mix(from: [u8; 4], to: [u8; 4], t: f32) -> [u8; 4] {
    let [a, b] = [from, to].map(|c| Value::rgba8(c[0], c[1], c[2], c[3]));
    let t = f64::from(t.clamp(0.0, 1.0));
    let mix = |x: f64, y: f64| x + (y - x) * t;
    let w = mix(a.w, b.w);
    match w > 0.0 {
        true => Value::rgba(mix(a.x, b.x) / w, mix(a.y, b.y) / w, mix(a.z, b.z) / w, w).to_rgba8(),
        false => [0, 0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Point;

    #[test]
    fn a_layout_springs_size_overshoot_is_clamped_to_zero() {
        let shown = Presented {
            layout: [5.0, -8.0, -0.5, -2.0],
            ..Presented::IDENTITY
        };
        assert_eq!(
            shown.surface((10.0, 20.0, 100.0, 40.0)),
            (10.0, 20.0, 0.0, 0.0)
        );
    }

    #[test]
    fn a_layout_transition_moves_the_box_and_sizes_only_its_surface() {
        let map = |p: &Presented, x: f32, y: f32| {
            let mut point = [Point::from_xy(x, y)];
            p.transform((10.0, 20.0, 100.0, 40.0), (50.0, 20.0))
                .map_points(&mut point);
            (point[0].x, point[0].y)
        };
        let grow = Presented {
            layout: [5.0, -8.0, 1.0, 0.25],
            ..Presented::IDENTITY
        };
        // Moved by the offset, never scaled: its content keeps its size.
        assert_eq!(map(&grow, 10.0, 20.0), (15.0, 12.0));
        assert_eq!(map(&grow, 110.0, 60.0), (115.0, 52.0));
        assert_eq!(
            grow.surface((10.0, 20.0, 100.0, 40.0)),
            (10.0, 20.0, 100.0, 10.0)
        );
        // An authored scale stays about the center.
        let both = Presented { scale: 0.5, ..grow };
        assert_eq!(map(&both, 10.0, 20.0), (40.0, 22.0));
    }
}
