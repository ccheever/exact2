//! The animatable properties and their values.
//!
//! @ref LLP 1002 §2 (targets are style; the vocabulary is CSS's)
//!
//! v1 animates the four CSS properties a compositor applies without relayout:
//! the individual transform properties `translate`, `scale`, `rotate`, and
//! `opacity`. Names, units, identity values, and interpolation are CSS's:
//! `translate` is two lengths in points, `scale` one number, `rotate` an angle
//! in degrees, `opacity` a number; each interpolates componentwise and
//! linearly (CSS Transitions §4, "animation type: by computed value"). The
//! numeric `height` trial adds a scalar in pixels, with host-owned admission
//! and layout (LLP 1041 §8.12). Its CSS initial `auto` has no numeric value.
//! SVG 2's `stroke-dashoffset` and `r` are scalars in user units (LLP 1055 D6),
//! as are the geometry rows `cx`, `cy`, `x`, `y`, `rx` and `ry` (LLP 1055.000 D15).
//! Colours (`color`, `background-color`, `fill`, `stroke`) are four
//! components, premultiplied sRGB red, green, blue and alpha in 0–1, so
//! componentwise interpolation is CSS Color 4's premultiplied interpolation
//! of legacy colours (LLP 1055.000 D6).

/// One animatable property.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Property {
    /// `translate: <x> <y>`, in points.
    Translate = 0,
    /// `scale: <n>`, uniform; one is the natural size.
    Scale = 1,
    /// `rotate: <angle>`, in degrees.
    Rotate = 2,
    /// `opacity: <n>`, zero to one.
    Opacity = 3,
    /// Numeric CSS `height`, in logical pixels; host admission is explicit.
    Height = 4,
    /// SVG 2 `stroke-dashoffset`, in user units (LLP 1055 D6).
    StrokeDashoffset = 5,
    /// SVG 2 `r`, a circle's radius in user units (LLP 1055 D6).
    R = 6,
    /// CSS `color` (LLP 1055.000 D6).
    Color = 7,
    /// CSS `background-color`.
    BackgroundColor = 8,
    /// SVG `fill`, when it is a colour.
    Fill = 9,
    /// SVG `stroke`, when it is a colour.
    Stroke = 10,
    /// SVG 2 `cx`, in user units (LLP 1055.000 D15).
    Cx = 11,
    /// SVG 2 `cy`.
    Cy = 12,
    /// SVG 2 `x`.
    X = 13,
    /// SVG 2 `y`.
    Y = 14,
    /// SVG 2 `rx`.
    Rx = 15,
    /// SVG 2 `ry`.
    Ry = 16,
}

impl Property {
    /// Every property, in wire order.
    pub const ALL: [Property; 17] = [
        Property::Translate,
        Property::Scale,
        Property::Rotate,
        Property::Opacity,
        Property::Height,
        Property::StrokeDashoffset,
        Property::R,
        Property::Color,
        Property::BackgroundColor,
        Property::Fill,
        Property::Stroke,
        Property::Cx,
        Property::Cy,
        Property::X,
        Property::Y,
        Property::Rx,
        Property::Ry,
    ];

    /// How many properties there are.
    pub const COUNT: usize = 17;

    /// The CSS property name.
    pub fn name(self) -> &'static str {
        match self {
            Property::Translate => "translate",
            Property::Scale => "scale",
            Property::Rotate => "rotate",
            Property::Opacity => "opacity",
            Property::Height => "height",
            Property::StrokeDashoffset => "stroke-dashoffset",
            Property::R => "r",
            Property::Color => "color",
            Property::BackgroundColor => "background-color",
            Property::Fill => "fill",
            Property::Stroke => "stroke",
            Property::Cx => "cx",
            Property::Cy => "cy",
            Property::X => "x",
            Property::Y => "y",
            Property::Rx => "rx",
            Property::Ry => "ry",
        }
    }

    /// From the CSS property name.
    pub fn from_name(name: &str) -> Option<Property> {
        Property::ALL.into_iter().find(|p| p.name() == name)
    }

    /// From the wire discriminant.
    pub fn from_wire(value: u8) -> Option<Property> {
        Property::ALL.get(value as usize).copied()
    }

    /// Whether the value is a colour.
    pub fn is_color(self) -> bool {
        matches!(
            self,
            Property::Color | Property::BackgroundColor | Property::Fill | Property::Stroke
        )
    }

    /// How many components the value carries: two for `translate`, four
    /// for a colour, else one.
    pub fn components(self) -> usize {
        match self {
            Property::Translate => 2,
            Property::Color | Property::BackgroundColor | Property::Fill | Property::Stroke => 4,
            Property::Scale
            | Property::Rotate
            | Property::Opacity
            | Property::Height
            | Property::StrokeDashoffset
            | Property::R
            | Property::Cx
            | Property::Cy
            | Property::X
            | Property::Y
            | Property::Rx
            | Property::Ry => 1,
        }
    }

    /// The CSS initial value when numeric. Height initially is `auto`, not
    /// zero: a host must adopt an eligible authored target explicitly.
    pub fn identity(self) -> Option<Value> {
        match self {
            Property::Translate => Some(Value::ZERO),
            Property::Scale | Property::Opacity => Some(Value::scalar(1.0)),
            Property::Rotate
            | Property::StrokeDashoffset
            | Property::R
            | Property::Cx
            | Property::Cy
            | Property::X
            | Property::Y
            | Property::Rx
            | Property::Ry => Some(Value::scalar(0.0)),
            Property::Height
            | Property::Color
            | Property::BackgroundColor
            | Property::Fill
            | Property::Stroke => None,
        }
    }
}

/// A property value: up to four components. A scalar property uses `x`, a
/// `translate` `x` and `y`, a colour all four (premultiplied red, green,
/// blue, alpha); unused components stay zero, so one type serves every
/// property and every comparison is exact.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Value {
    /// First component (or the scalar).
    pub x: f64,
    /// Second component.
    pub y: f64,
    /// Third component (colours only).
    pub z: f64,
    /// Fourth component (colours: alpha).
    pub w: f64,
}

impl Value {
    /// Every component zero.
    pub const ZERO: Value = Value {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };

    /// A scalar value.
    pub const fn scalar(x: f64) -> Value {
        Value {
            x,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    /// A two-component value.
    pub const fn new(x: f64, y: f64) -> Value {
        Value {
            x,
            y,
            z: 0.0,
            w: 0.0,
        }
    }

    /// A colour from straight sRGB components in 0–1, stored premultiplied.
    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Value {
        Value {
            x: r * a,
            y: g * a,
            z: b * a,
            w: a,
        }
    }

    /// A colour from 8-bit straight RGBA.
    pub fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Value {
        let c = |v: u8| v as f64 / 255.0;
        Value::rgba(c(r), c(g), c(b), c(a))
    }

    /// A colour value as straight 8-bit RGBA (alpha 0 is transparent black).
    pub fn to_rgba8(self) -> [u8; 4] {
        let a = self.w.clamp(0.0, 1.0);
        let q = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        if a <= 0.0 {
            return [0, 0, 0, 0];
        }
        [q(self.x / a), q(self.y / a), q(self.z / a), q(a)]
    }

    /// Whether every component is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite() && self.w.is_finite()
    }

    /// Componentwise linear interpolation at `progress`.
    pub fn lerp(self, to: Value, progress: f64) -> Value {
        Value {
            x: self.x + (to.x - self.x) * progress,
            y: self.y + (to.y - self.y) * progress,
            z: self.z + (to.z - self.z) * progress,
            w: self.w + (to.w - self.w) * progress,
        }
    }

    /// Whether the value uses only the components `property` has.
    pub fn fits(self, property: Property) -> bool {
        match property.components() {
            1 => self.y == 0.0 && self.z == 0.0 && self.w == 0.0,
            2 => self.z == 0.0 && self.w == 0.0,
            _ => true,
        }
    }
}

impl core::ops::Sub for Value {
    type Output = Value;

    /// Componentwise difference.
    fn sub(self, other: Value) -> Value {
        Value {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
            w: self.w - other.w,
        }
    }
}

impl core::ops::Add for Value {
    type Output = Value;

    /// Componentwise sum.
    fn add(self, other: Value) -> Value {
        Value {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    }
}
