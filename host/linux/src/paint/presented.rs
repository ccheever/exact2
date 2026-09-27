//! What the motion engine says to paint over a node's style rows: the four
//! compositor values, paint motion's colours and shadow while they move
//! (LLP 1062), and a path's stroke fractions (LLP 1065).

use super::shadow::ShadowPaint;
use super::BoxPaint;
use exact_kernel::StyleProps;
use exact_motion::{Property, Value};
use tiny_skia::Transform;

/// A node's presentation values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presented {
    /// Points.
    pub translate: (f32, f32),
    /// Uniform.
    pub scale: f32,
    /// Degrees.
    pub rotate: f32,
    /// Zero to one.
    pub opacity: f32,
    /// Paint motion's values, each while it differs from its row.
    pub paint: PaintValues,
    /// A path's `stroke-start` and `stroke-end` (LLP 1065 D4).
    pub stroke: (f32, f32),
    /// A layout transition's offset from the laid-out origin and scale of
    /// the laid-out size, `[dx, dy, sx, sy]` (LLP 1063).
    pub layout: [f32; 4],
}

impl Presented {
    /// Nothing moved.
    pub const IDENTITY: Presented = Presented {
        translate: (0.0, 0.0),
        scale: 1.0,
        rotate: 0.0,
        opacity: 1.0,
        paint: PaintValues::NONE,
        stroke: (0.0, 1.0),
        layout: [0.0, 0.0, 1.0, 1.0],
    };

    /// The committed style's values (what the engine starts from).
    pub fn from_style(s: &StyleProps) -> Presented {
        Presented {
            translate: (s.translate.x, s.translate.y),
            scale: s.scale,
            rotate: s.rotate,
            opacity: s.opacity,
            paint: PaintValues::NONE,
            stroke: (s.stroke_start, s.stroke_end),
            layout: Presented::IDENTITY.layout,
        }
    }

    pub(super) fn moves(&self) -> bool {
        self.translate != (0.0, 0.0)
            || self.scale != 1.0
            || self.rotate != 0.0
            || self.layout != Presented::IDENTITY.layout
    }

    /// The box `(x, y, w, h)` painted through its presentation: CSS's
    /// individual transforms about its center, then outermost the layout
    /// transition's offset and scale from its top-left corner, as a web FLIP
    /// places it (LLP 1063).
    pub(super) fn transform(&self, (x, y, w, h): (f32, f32, f32, f32)) -> Transform {
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let [dx, dy, sx, sy] = self.layout;
        Transform::from_translate(x + dx, y + dy)
            .pre_scale(sx, sy)
            .pre_translate(-x, -y)
            .pre_translate(cx + self.translate.0, cy + self.translate.1)
            .pre_rotate(self.rotate)
            .pre_scale(self.scale, self.scale)
            .pre_translate(-cx, -cy)
    }
}

/// Paint motion's presented values, one slot per [`Property::PAINT`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintValues([Option<Value>; 9]);

impl PaintValues {
    /// None: every paint property shows its row.
    pub const NONE: PaintValues = PaintValues([None; 9]);

    fn slot(property: Property) -> usize {
        property as usize - Property::BackgroundColor as usize
    }

    /// Paint `value` over the property's row, or the row again.
    pub fn set(&mut self, property: Property, value: Option<Value>) {
        self.0[Self::slot(property)] = value;
    }

    /// Whether anything is presented over the rows.
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// A presented colour, straight 8-bit channels.
    pub fn color(&self, property: Property) -> Option<[u8; 4]> {
        self.0[Self::slot(property)].map(|v| v.straight().map(|c| (c * 255.0).round() as u8))
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
            self.shadow = ShadowPaint::over(self.shadow, geometry, color);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Point;

    #[test]
    fn a_layout_box_is_placed_from_its_top_left_outside_the_authored_transforms() {
        let map = |p: &Presented, x: f32, y: f32| {
            let mut point = [Point::from_xy(x, y)];
            p.transform((10.0, 20.0, 100.0, 40.0))
                .map_points(&mut point);
            (point[0].x, point[0].y)
        };
        let grow = Presented {
            layout: [5.0, -8.0, 1.0, 0.25],
            ..Presented::IDENTITY
        };
        // Its top-left moves by the offset; its size is the scaled one.
        assert_eq!(map(&grow, 10.0, 20.0), (15.0, 12.0));
        assert_eq!(map(&grow, 110.0, 60.0), (115.0, 22.0));
        // An authored scale stays about the center, inside the layout box.
        let both = Presented { scale: 0.5, ..grow };
        assert_eq!(map(&both, 60.0, 40.0), (65.0, 17.0));
        assert_eq!(map(&both, 10.0, 20.0), (40.0, 14.5));
    }
}
