use exact_kernel::StyleProps;

/// A node's presentation values: what the motion engine says to paint.
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
    /// A path's `stroke-start` and `stroke-end` (LLP 1065 D4).
    pub stroke: (f32, f32),
}

impl Presented {
    /// Nothing moved.
    pub const IDENTITY: Presented = Presented {
        translate: (0.0, 0.0),
        scale: 1.0,
        rotate: 0.0,
        opacity: 1.0,
        stroke: (0.0, 1.0),
    };

    /// The committed style's values (what the engine starts from).
    pub fn from_style(s: &StyleProps) -> Presented {
        Presented {
            translate: (s.translate.x, s.translate.y),
            scale: s.scale,
            rotate: s.rotate,
            opacity: s.opacity,
            stroke: (s.stroke_start, s.stroke_end),
        }
    }

    pub(super) fn moves(&self) -> bool {
        self.translate != (0.0, 0.0) || self.scale != 1.0 || self.rotate != 0.0
    }
}
