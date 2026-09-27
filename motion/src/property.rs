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
//!
//! Paint properties (LLP 1062) repaint without relayout: the colours
//! (`background-color`, `color`, the four border sides, `tint-color`) and
//! `box-shadow`. A colour is premultiplied sRGB, every channel 0–1, so
//! componentwise interpolation is CSS Color 4 §12.3's for legacy colours.
//! `box-shadow` is two engine properties: its geometry (offset and blur, in
//! points) and its colour, the shadow's opacity folded into the alpha, so a
//! shadow from `none` is CSS's transparent, zero-length padding.
//!
//! `layout` is not a CSS property: it is a node's laid-out box in its parent
//! (origin and size), which a `layout-transition` row animates (LLP 1063).
//! It is never authored in `transition` or `@keyframes`, so it is outside
//! [`Property::ALL`].
//!
//! A `path` node's `stroke-start` and `stroke-end` are fractions of its
//! length a vector layer trims its stroke to (LLP 1065); on the web they are
//! the registered custom properties [`Property::css_name`] names. Its `fill`
//! and `stroke` are paint too: colours, when neither end is `none` (SVG's
//! `<paint>` interpolates only colour to colour; a keyword is discrete).

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
    /// `background-color`.
    BackgroundColor = 5,
    /// `color`, the text colour.
    Color = 6,
    /// `border-top-color`.
    BorderTopColor = 7,
    /// `border-right-color`.
    BorderRightColor = 8,
    /// `border-bottom-color`.
    BorderBottomColor = 9,
    /// `border-left-color`.
    BorderLeftColor = 10,
    /// `tint-color`, a symbol image's colour.
    TintColor = 11,
    /// `box-shadow`'s offset and blur radius, points (`x`, `y`, `z`).
    BoxShadow = 12,
    /// `box-shadow`'s colour, its opacity folded into the alpha. Named only
    /// by `box-shadow`, never on its own.
    ShadowColor = 13,
    /// The laid-out box in the parent, in points: origin (`x`, `y`) and size
    /// (`z` wide, `w` high) (LLP 1063). Its target is layout's answer,
    /// observed by a host after layout; only a node's `layout-transition` row
    /// moves it, never `transition`.
    Layout = 14,
    /// A path's `stroke-start`: the fraction of its length the visible
    /// stroke starts at (LLP 1065).
    StrokeStart = 15,
    /// A path's `stroke-end`: the fraction of its length the visible stroke
    /// ends at (LLP 1065).
    StrokeEnd = 16,
    /// SVG `fill`, a path's fill colour (LLP 1065).
    Fill = 17,
    /// SVG `stroke`, a path's stroke colour (LLP 1065).
    Stroke = 18,
}

impl Property {
    /// Every authorable property, in wire order ([`Property::Layout`] is
    /// the host's, not an author's). The wire carries a discriminant, never
    /// an index here.
    pub const ALL: [Property; 18] = [
        Property::Translate,
        Property::Scale,
        Property::Rotate,
        Property::Opacity,
        Property::Height,
        Property::BackgroundColor,
        Property::Color,
        Property::BorderTopColor,
        Property::BorderRightColor,
        Property::BorderBottomColor,
        Property::BorderLeftColor,
        Property::TintColor,
        Property::BoxShadow,
        Property::ShadowColor,
        Property::StrokeStart,
        Property::StrokeEnd,
        Property::Fill,
        Property::Stroke,
    ];

    /// The paint properties: repainted, never laid out (LLP 1062), a path's
    /// `fill` and `stroke` among them (LLP 1065).
    pub const PAINT: [Property; 11] = [
        Property::BackgroundColor,
        Property::Color,
        Property::BorderTopColor,
        Property::BorderRightColor,
        Property::BorderBottomColor,
        Property::BorderLeftColor,
        Property::TintColor,
        Property::BoxShadow,
        Property::ShadowColor,
        Property::Fill,
        Property::Stroke,
    ];

    /// The CSS property name (`box-shadow-color` is the engine's own name
    /// for `box-shadow`'s colour half, and not CSS).
    pub fn name(self) -> &'static str {
        match self {
            Property::Translate => "translate",
            Property::Scale => "scale",
            Property::Rotate => "rotate",
            Property::Opacity => "opacity",
            Property::Height => "height",
            Property::BackgroundColor => "background-color",
            Property::Color => "color",
            Property::BorderTopColor => "border-top-color",
            Property::BorderRightColor => "border-right-color",
            Property::BorderBottomColor => "border-bottom-color",
            Property::BorderLeftColor => "border-left-color",
            Property::TintColor => "tint-color",
            Property::BoxShadow => "box-shadow",
            Property::ShadowColor => "box-shadow-color",
            Property::Layout => "layout",
            Property::StrokeStart => "stroke-start",
            Property::StrokeEnd => "stroke-end",
            Property::Fill => "fill",
            Property::Stroke => "stroke",
        }
    }

    /// The name a browser knows the property by: [`Property::name`], but
    /// `tint-color` and a path's stroke fractions, which the web host
    /// carries as registered custom properties (LLP 1062 D6, LLP 1065 D5).
    pub fn css_name(self) -> &'static str {
        match self {
            Property::TintColor => "--exact-tint",
            Property::StrokeStart => "--exact-stroke-start",
            Property::StrokeEnd => "--exact-stroke-end",
            p => p.name(),
        }
    }

    /// From an authorable CSS property name; `box-shadow`'s colour half has
    /// none. A stroke fraction also answers to its [`Property::css_name`],
    /// the name a keyframes rule carries it by.
    pub fn from_name(name: &str) -> Option<Property> {
        Property::ALL.into_iter().find(|p| {
            *p != Property::ShadowColor
                && (p.name() == name
                    || matches!(p, Property::StrokeStart | Property::StrokeEnd)
                        && p.css_name() == name)
        })
    }

    /// From the wire discriminant ([`Property::Layout`] is never on it).
    pub fn from_wire(value: u8) -> Option<Property> {
        Property::ALL.into_iter().find(|p| *p as u8 == value)
    }

    /// Whether the property is a colour: premultiplied, four channels.
    pub fn is_color(self) -> bool {
        matches!(
            self,
            Property::BackgroundColor
                | Property::Color
                | Property::BorderTopColor
                | Property::BorderRightColor
                | Property::BorderBottomColor
                | Property::BorderLeftColor
                | Property::TintColor
                | Property::ShadowColor
                | Property::Fill
                | Property::Stroke
        )
    }

    /// Whether a `spring()` drives the property as physics, carrying
    /// velocity across an interruption. The web lowers these springs to
    /// frames; every other property (paint, a path's stroke and paint) plays a spring
    /// as its curve from rest, a CSS `linear()` easing, on every host (LLP
    /// 1062 D3).
    pub fn springs(self) -> bool {
        matches!(
            self,
            Property::Translate
                | Property::Scale
                | Property::Rotate
                | Property::Opacity
                | Property::Height
                | Property::Layout
        )
    }

    /// How many components the value carries: two for `translate`, three
    /// for `box-shadow`'s geometry, four for a colour and for `layout`'s box,
    /// else one.
    pub fn components(self) -> usize {
        match self {
            Property::Translate => 2,
            Property::BoxShadow => 3,
            Property::Layout => 4,
            p if p.is_color() => 4,
            _ => 1,
        }
    }

    /// The CSS initial value when numeric and not paint. Height initially is
    /// `auto`, not zero: a host must adopt an eligible authored target
    /// explicitly. Paint has no identity a host could skip: a colour row's
    /// initial value is its own. A position has no identity.
    pub fn identity(self) -> Option<Value> {
        match self {
            Property::Translate => Some(Value::ZERO),
            Property::Scale | Property::Opacity | Property::StrokeEnd => Some(Value::scalar(1.0)),
            Property::Rotate | Property::StrokeStart => Some(Value::scalar(0.0)),
            _ => None,
        }
    }
}

/// A property value: up to four components. A property uses the first
/// [`Property::components`] and keeps the rest at zero, so one type serves
/// every property and every comparison is exact.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Value {
    /// First component (the scalar; a colour's premultiplied red).
    pub x: f64,
    /// Second component (`translate`'s y; premultiplied green).
    pub y: f64,
    /// Third component (a shadow's blur; premultiplied blue; a layout box's
    /// width).
    pub z: f64,
    /// Fourth component (a colour's alpha; a layout box's height).
    pub w: f64,
}

impl Value {
    /// Every component zero (also transparent black).
    pub const ZERO: Value = Value::four(0.0, 0.0, 0.0, 0.0);

    /// A scalar value.
    pub const fn scalar(x: f64) -> Value {
        Value::new(x, 0.0)
    }

    /// A two-component value.
    pub const fn new(x: f64, y: f64) -> Value {
        Value::four(x, y, 0.0, 0.0)
    }

    /// A four-component value.
    pub const fn four(x: f64, y: f64, z: f64, w: f64) -> Value {
        Value { x, y, z, w }
    }

    /// A colour from straight sRGB channels, each 0–1, premultiplied: the
    /// form CSS interpolates a colour with alpha in (CSS Color 4 §12.3).
    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Value {
        Value::four(r * a, g * a, b * a, a)
    }

    /// A colour from 8-bit straight channels.
    pub fn rgba8([r, g, b, a]: [u8; 4]) -> Value {
        let unit = |c: u8| c as f64 / 255.0;
        Value::rgba(unit(r), unit(g), unit(b), unit(a))
    }

    /// A colour value back to straight channels, each clamped to 0–1 as CSS
    /// clamps an out-of-gamut interpolation; transparent is transparent
    /// black.
    pub fn straight(self) -> [f64; 4] {
        let a = self.w.clamp(0.0, 1.0);
        if a == 0.0 {
            return [0.0; 4];
        }
        let c = |v: f64| (v / a).clamp(0.0, 1.0);
        [c(self.x), c(self.y), c(self.z), a]
    }

    /// Whether every component is finite.
    pub fn is_finite(self) -> bool {
        self.components().iter().all(|c| c.is_finite())
    }

    /// Whether the components past the property's own are zero.
    pub fn fits(self, property: Property) -> bool {
        self.components()[property.components()..]
            .iter()
            .all(|c| *c == 0.0)
    }

    /// Componentwise linear interpolation at `progress`.
    pub fn lerp(self, to: Value, progress: f64) -> Value {
        self.zip(to, |a, b| a + (b - a) * progress)
    }

    /// Each component through `f`.
    pub fn map(self, f: impl Fn(f64) -> f64) -> Value {
        Value::four(f(self.x), f(self.y), f(self.z), f(self.w))
    }

    /// Two values componentwise through `f`.
    pub fn zip(self, other: Value, f: impl Fn(f64, f64) -> f64) -> Value {
        Value::four(
            f(self.x, other.x),
            f(self.y, other.y),
            f(self.z, other.z),
            f(self.w, other.w),
        )
    }

    /// The components, in order.
    pub fn components(self) -> [f64; 4] {
        [self.x, self.y, self.z, self.w]
    }
}

impl core::ops::Sub for Value {
    type Output = Value;

    /// Componentwise difference.
    fn sub(self, other: Value) -> Value {
        self.zip(other, |a, b| a - b)
    }
}

impl core::ops::Add for Value {
    type Output = Value;

    /// Componentwise sum.
    fn add(self, other: Value) -> Value {
        self.zip(other, |a, b| a + b)
    }
}
