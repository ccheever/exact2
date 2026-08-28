//! The animatable properties and their values.
//!
//! @ref LLP 1002 §2 (targets are style; the vocabulary is CSS's)
//!
//! v1 animates the four CSS properties a compositor applies without relayout:
//! the individual transform properties `translate`, `scale`, `rotate`, and
//! `opacity`. Names, units, identity values, and interpolation are CSS's:
//! `translate` is two lengths in points, `scale` one number, `rotate` an angle
//! in degrees, `opacity` a number; each interpolates componentwise and
//! linearly (CSS Transitions §4, "animation type: by computed value").

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
}

impl Property {
    /// Every property, in wire order.
    pub const ALL: [Property; 4] = [
        Property::Translate,
        Property::Scale,
        Property::Rotate,
        Property::Opacity,
    ];

    /// The CSS property name.
    pub fn name(self) -> &'static str {
        match self {
            Property::Translate => "translate",
            Property::Scale => "scale",
            Property::Rotate => "rotate",
            Property::Opacity => "opacity",
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
            Property::Scale | Property::Rotate | Property::Opacity => 1,
        }
    }

    /// The CSS initial value.
    pub fn identity(self) -> Value {
        match self {
            Property::Translate => Value::ZERO,
            Property::Scale | Property::Opacity => Value::scalar(1.0),
            Property::Rotate => Value::scalar(0.0),
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
