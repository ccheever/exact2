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
//! SVG 2's `stroke-dashoffset` and `r` are scalars in user units (LLP 1055 D6).

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
}

impl Property {
    /// Every property, in wire order.
    pub const ALL: [Property; 7] = [
        Property::Translate,
        Property::Scale,
        Property::Rotate,
        Property::Opacity,
        Property::Height,
        Property::StrokeDashoffset,
        Property::R,
    ];

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

    /// How many components the value carries: two for `translate`, else one.
    pub fn components(self) -> usize {
        match self {
            Property::Translate => 2,
            Property::Scale
            | Property::Rotate
            | Property::Opacity
            | Property::Height
            | Property::StrokeDashoffset
            | Property::R => 1,
        }
    }

    /// The CSS initial value when numeric. Height initially is `auto`, not
    /// zero: a host must adopt an eligible authored target explicitly.
    pub fn identity(self) -> Option<Value> {
        match self {
            Property::Translate => Some(Value::ZERO),
            Property::Scale | Property::Opacity => Some(Value::scalar(1.0)),
            Property::Rotate | Property::StrokeDashoffset | Property::R => Some(Value::scalar(0.0)),
            Property::Height => None,
        }
    }
}

/// A property value: up to two components. A scalar property uses `x` and
/// keeps `y` at zero, so one type serves every property and every comparison
/// is exact.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Value {
    /// First component (or the scalar).
    pub x: f64,
    /// Second component (`translate` only).
    pub y: f64,
}

impl Value {
    /// Both components zero.
    pub const ZERO: Value = Value { x: 0.0, y: 0.0 };

    /// A scalar value.
    pub const fn scalar(x: f64) -> Value {
        Value { x, y: 0.0 }
    }

    /// A two-component value.
    pub const fn new(x: f64, y: f64) -> Value {
        Value { x, y }
    }

    /// Whether both components are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Componentwise linear interpolation at `progress`.
    pub fn lerp(self, to: Value, progress: f64) -> Value {
        Value {
            x: self.x + (to.x - self.x) * progress,
            y: self.y + (to.y - self.y) * progress,
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
        }
    }
}
