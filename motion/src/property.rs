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
//!
//! Paint motion (LLP 1062) adds the four border sides' colours, a symbol's
//! `tint-color`, and `box-shadow` as two engine properties: its geometry
//! (offset and blur, in points) and its colour, the shadow's opacity folded
//! into the alpha, so a shadow from `none` is CSS's transparent, zero-length
//! padding.
//!
//! `layout` is not a CSS property: it is a node's laid-out box in its parent
//! (origin and size), which a `layout-transition` row animates (LLP 1063).
//! It is never authored in `transition` or `@keyframes`, so it is outside
//! [`Property::ALL`] and never on the wire.

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
    /// `border-top-color` (LLP 1062).
    BorderTopColor = 17,
    /// `border-right-color`.
    BorderRightColor = 18,
    /// `border-bottom-color`.
    BorderBottomColor = 19,
    /// `border-left-color`.
    BorderLeftColor = 20,
    /// `tint-color`, a symbol image's colour.
    TintColor = 21,
    /// `box-shadow`'s offset and blur radius, points (`x`, `y`, `z`).
    BoxShadow = 22,
    /// `box-shadow`'s colour, its opacity folded into the alpha. Named only
    /// by `box-shadow`, never on its own.
    ShadowColor = 23,
    /// The laid-out box in the parent, in points: origin (`x`, `y`) and size
    /// (`z` wide, `w` high) (LLP 1063). Its target is layout's answer,
    /// observed by a host after layout; only a node's `layout-transition` row
    /// moves it, never `transition`.
    Layout = 24,
}

impl Property {
    /// Every authorable property, in wire order ([`Property::Layout`] is
    /// the host's, not an author's, and never on the wire).
    pub const ALL: [Property; 24] = [
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
        Property::BorderTopColor,
        Property::BorderRightColor,
        Property::BorderBottomColor,
        Property::BorderLeftColor,
        Property::TintColor,
        Property::BoxShadow,
        Property::ShadowColor,
    ];

    /// How many properties there are on the wire.
    pub const COUNT: usize = 24;

    /// The paint properties: repainted, never laid out. A native host's
    /// paint pass owns them (LLP 1062 D2): CSS's box colours and
    /// `box-shadow`, and an SVG element's `fill` and `stroke` (LLP 1055.000
    /// D6).
    pub const PAINT: [Property; 11] = [
        Property::Color,
        Property::BackgroundColor,
        Property::Fill,
        Property::Stroke,
        Property::BorderTopColor,
        Property::BorderRightColor,
        Property::BorderBottomColor,
        Property::BorderLeftColor,
        Property::TintColor,
        Property::BoxShadow,
        Property::ShadowColor,
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
            Property::BorderTopColor => "border-top-color",
            Property::BorderRightColor => "border-right-color",
            Property::BorderBottomColor => "border-bottom-color",
            Property::BorderLeftColor => "border-left-color",
            Property::TintColor => "tint-color",
            Property::BoxShadow => "box-shadow",
            Property::ShadowColor => "box-shadow-color",
            Property::Layout => "layout",
        }
    }

    /// The name a browser knows the property by: [`Property::name`], but
    /// `tint-color`, which the web host carries as the registered custom
    /// property `--exact-tint` (LLP 1062 D6).
    pub fn css_name(self) -> &'static str {
        match self {
            Property::TintColor => "--exact-tint",
            p => p.name(),
        }
    }

    /// From an authorable CSS property name; `box-shadow`'s colour half has
    /// none. `tint-color` also answers to its [`Property::css_name`], the
    /// name a keyframes rule carries it by.
    pub fn from_name(name: &str) -> Option<Property> {
        Property::ALL
            .into_iter()
            .find(|p| *p != Property::ShadowColor && (p.name() == name || p.css_name() == name))
    }

    /// From the wire discriminant ([`Property::Layout`] is never on it).
    pub fn from_wire(value: u8) -> Option<Property> {
        Property::ALL.get(value as usize).copied()
    }

    /// Whether the value is a colour: premultiplied, four channels.
    pub fn is_color(self) -> bool {
        matches!(
            self,
            Property::Color
                | Property::BackgroundColor
                | Property::Fill
                | Property::Stroke
                | Property::BorderTopColor
                | Property::BorderRightColor
                | Property::BorderBottomColor
                | Property::BorderLeftColor
                | Property::TintColor
                | Property::ShadowColor
        )
    }

    /// Whether a `spring()` drives the property as physics, carrying
    /// velocity across an interruption. The web lowers these springs to
    /// frames; every other property (paint, SVG geometry and paint) plays a
    /// spring as its curve from rest, a CSS `linear()` easing, on every host
    /// (LLP 1062 D3).
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

    /// The CSS initial value when numeric. Height initially is `auto`, not
    /// zero: a host must adopt an eligible authored target explicitly. A
    /// colour row's initial value is its own, and a position has none.
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
    /// A colour stored as premultiplied OKLab rather than sRGB (LLP 1100 D2).
    pub oklab: bool,
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
        Value {
            x,
            y,
            z,
            w,
            oklab: false,
        }
    }

    /// A colour from OKLab components and alpha, stored premultiplied.
    pub fn oklab(l: f64, a: f64, b: f64, alpha: f64) -> Value {
        Value {
            oklab: true,
            ..Value::four(l * alpha, a * alpha, b * alpha, alpha)
        }
    }

    /// The same colour as premultiplied OKLab (itself when it already is).
    pub fn to_oklab(self) -> Value {
        if self.oklab {
            return self;
        }
        let alpha = self.w.clamp(0.0, 1.0);
        if alpha == 0.0 {
            return Value::oklab(0.0, 0.0, 0.0, 0.0);
        }
        let rgb = [self.x / alpha, self.y / alpha, self.z / alpha].map(exact_color::srgb_to_linear);
        let [l, a, b] = exact_color::MixSpace::Oklab.from_linear_srgb(rgb);
        Value::oklab(l, a, b, alpha)
    }

    /// A colour's straight extended linear sRGB and alpha, unclipped.
    pub fn linear_srgb(self) -> ([f64; 3], f64) {
        let alpha = self.w.clamp(0.0, 1.0);
        if alpha == 0.0 {
            return ([0.0; 3], 0.0);
        }
        let c = [self.x / alpha, self.y / alpha, self.z / alpha];
        if self.oklab {
            (exact_color::MixSpace::Oklab.to_linear_srgb(c), alpha)
        } else {
            (c.map(exact_color::srgb_to_linear), alpha)
        }
    }

    /// A colour from straight sRGB components in 0–1, stored premultiplied:
    /// the form CSS interpolates a colour with alpha in (CSS Color 4 §12.3).
    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Value {
        Value::four(r * a, g * a, b * a, a)
    }

    /// A colour from 8-bit straight RGBA.
    pub fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Value {
        let c = |v: u8| v as f64 / 255.0;
        Value::rgba(c(r), c(g), c(b), c(a))
    }

    /// A colour value as straight 8-bit RGBA (alpha 0 is transparent black).
    pub fn to_rgba8(self) -> [u8; 4] {
        if self.oklab {
            let [r, g, b, a] = self.straight();
            let q = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
            return [q(r), q(g), q(b), q(a)];
        }
        let a = self.w.clamp(0.0, 1.0);
        let q = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        if a <= 0.0 {
            return [0, 0, 0, 0];
        }
        [q(self.x / a), q(self.y / a), q(self.z / a), q(a)]
    }

    /// A colour value back to straight channels, each clamped to 0–1 as CSS
    /// clamps an out-of-gamut interpolation; transparent is transparent
    /// black.
    pub fn straight(self) -> [f64; 4] {
        let a = self.w.clamp(0.0, 1.0);
        if a == 0.0 {
            return [0.0; 4];
        }
        if self.oklab {
            let ([r, g, b], a) = self.linear_srgb();
            let c = |v: f64| exact_color::linear_to_srgb(v).clamp(0.0, 1.0);
            return [c(r), c(g), c(b), a];
        }
        let c = |v: f64| (v / a).clamp(0.0, 1.0);
        [c(self.x), c(self.y), c(self.z), a]
    }

    /// Whether every component is finite.
    pub fn is_finite(self) -> bool {
        self.components().iter().all(|c| c.is_finite())
    }

    /// Componentwise linear interpolation at `progress`.
    pub fn lerp(self, to: Value, progress: f64) -> Value {
        self.zip(to, |a, b| a + (b - a) * progress)
    }

    /// Whether the value uses only the components `property` has.
    pub fn fits(self, property: Property) -> bool {
        self.components()[property.components()..]
            .iter()
            .all(|c| *c == 0.0)
    }

    /// Each component through `f`.
    pub fn map(self, f: impl Fn(f64) -> f64) -> Value {
        Value {
            oklab: self.oklab,
            ..Value::four(f(self.x), f(self.y), f(self.z), f(self.w))
        }
    }

    /// Two values componentwise through `f`; mixed encodings meet in OKLab.
    pub fn zip(self, other: Value, f: impl Fn(f64, f64) -> f64) -> Value {
        let (a, b) = if self.oklab != other.oklab {
            (self.to_oklab(), other.to_oklab())
        } else {
            (self, other)
        };
        Value {
            oklab: a.oklab,
            ..Value::four(f(a.x, b.x), f(a.y, b.y), f(a.z, b.z), f(a.w, b.w))
        }
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
