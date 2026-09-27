//! Filters (LLP 1055.000 D14): a `filter` reference or CSS filter
//! functions, resolved into a chain of primitives in the element's user
//! space. The pixels are the hosts' and `exact-svg-raster`'s.
//!
//! @ref LLP 1055.000 D14; Filter Effects 1 §9 (the `filter` element:
//! `filterUnits` `objectBoundingBox` with the region −10%/−10%/120%/120%,
//! `primitiveUnits` `userSpaceOnUse`), §9.5 (subregions), §9.6
//! (`color-interpolation-filters`), §12 (the CSS functions' equivalents)
//!
//! The chain crosses to a loaded module on Apple, so it has a flat wire
//! form ([`Filter::encode`], [`Filter::decode`]): numbers only, versioned by
//! the module's ABI.

/// An input to a primitive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    /// The element as drawn, without the filter.
    SourceGraphic,
    /// Its alpha, black.
    SourceAlpha,
    /// A primitive's result, by index.
    Result(u16),
    /// Transparent black (a name no earlier primitive gave, `FillPaint`,
    /// `StrokePaint`).
    None,
}

/// `feComposite`'s operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeOp {
    /// Porter-Duff over.
    Over,
    /// In.
    In,
    /// Out.
    Out,
    /// Atop.
    Atop,
    /// Xor.
    Xor,
    /// `k1·i1·i2 + k2·i1 + k3·i2 + k4`.
    Arithmetic,
    /// CSS Compositing's `lighter`.
    Lighter,
}

/// A `feComponentTransfer` function for one channel.
#[derive(Debug, Clone, PartialEq)]
pub enum Transfer {
    /// Unchanged.
    Identity,
    /// `tableValues`, interpolated.
    Table(Vec<f32>),
    /// `tableValues`, stepped.
    Discrete(Vec<f32>),
    /// `slope·C + intercept`.
    Linear(f32, f32),
    /// `amplitude·C^exponent + offset`.
    Gamma(f32, f32, f32),
}

/// A light source (`feDistantLight`, `fePointLight`, `feSpotLight`), in
/// user units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Light {
    /// Azimuth and elevation, degrees.
    Distant(f32, f32),
    /// x, y, z.
    Point(f32, f32, f32),
    /// x, y, z, points-at x, y, z, specular exponent, cone angle (degrees,
    /// or none).
    Spot([f32; 6], f32, Option<f32>),
}

/// What a primitive does. Lengths are user units; the host scales them.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// `feGaussianBlur`: σ per axis.
    Blur(f32, f32),
    /// `feOffset`.
    Offset(f32, f32),
    /// `feFlood`: straight sRGB, 0–1, opacity folded in.
    Flood([f32; 4]),
    /// `feComposite`, with `k1`–`k4`.
    Composite(CompositeOp, [f32; 4]),
    /// `feMerge`: its nodes' inputs, bottom first.
    Merge(Vec<Input>),
    /// `feColorMatrix`, every type as a 4×5 matrix over unpremultiplied
    /// colour.
    ColorMatrix([f32; 20]),
    /// `feDropShadow`: σ, offset, straight sRGB colour.
    DropShadow(f32, f32, f32, f32, [f32; 4]),
    /// `feBlend`: `mode` as the index into [`BLEND_MODES`].
    Blend(u8),
    /// `feMorphology`: dilate, radius per axis.
    Morphology(bool, f32, f32),
    /// `feComponentTransfer`: R, G, B, A.
    ComponentTransfer(Box<[Transfer; 4]>),
    /// `feTile`: the input's subregion repeated.
    Tile,
    /// `feTurbulence`: base frequency per axis, octaves, seed, fractal
    /// noise (else turbulence), stitched.
    Turbulence(f32, f32, u32, f32, bool, bool),
    /// `feDisplacementMap`: scale, and the channels (0 R … 3 A) for x, y.
    Displacement(f32, u8, u8),
    /// `feConvolveMatrix`: columns, rows, the kernel, divisor, bias,
    /// target x, y, edge mode (0 duplicate, 1 wrap, 2 none), preserve alpha.
    Convolve(Box<Convolve>),
    /// `feDiffuseLighting` (`specular` false) or `feSpecularLighting`.
    Lighting(Box<Lighting>),
}

/// `feConvolveMatrix`'s parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Convolve {
    /// Columns and rows.
    pub order: (u32, u32),
    /// Row-major, `order.0 × order.1` values.
    pub kernel: Vec<f32>,
    /// Never zero.
    pub divisor: f32,
    /// Added after dividing.
    pub bias: f32,
    /// The target cell.
    pub target: (u32, u32),
    /// 0 duplicate, 1 wrap, 2 none.
    pub edge: u8,
    /// Leave alpha alone.
    pub preserve_alpha: bool,
}

/// Lighting's parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Lighting {
    /// Specular (else diffuse).
    pub specular: bool,
    /// `surfaceScale`.
    pub surface_scale: f32,
    /// `diffuseConstant` or `specularConstant`.
    pub constant: f32,
    /// `specularExponent` (1–128).
    pub exponent: f32,
    /// `lighting-color`, straight sRGB 0–1.
    pub color: [f32; 3],
    /// The light.
    pub light: Light,
}

/// `feBlend`'s modes, by index (Compositing and Blending 1).
pub const BLEND_MODES: [&str; 16] = [
    "normal",
    "multiply",
    "screen",
    "overlay",
    "darken",
    "lighten",
    "color-dodge",
    "color-burn",
    "hard-light",
    "soft-light",
    "difference",
    "exclusion",
    "hue",
    "saturation",
    "color",
    "luminosity",
];

/// One primitive, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Primitive {
    /// What it does.
    pub op: Op,
    /// `in` and `in2`.
    pub inputs: [Input; 2],
    /// Its subregion, x, y, width, height, in user units.
    pub subregion: (f32, f32, f32, f32),
    /// `color-interpolation-filters: linearRGB` (else sRGB).
    pub linear: bool,
}

/// A resolved filter: its region and primitives, the last one's result
/// being what paints.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    /// The filter region, x, y, width, height, in the element's user space.
    pub region: (f32, f32, f32, f32),
    /// In order. Empty: the element is not drawn (SVG 2: a filter with no
    /// primitives renders nothing).
    pub primitives: Vec<Primitive>,
}

fn input_code(i: Input) -> f32 {
    match i {
        Input::SourceGraphic => -1.0,
        Input::SourceAlpha => -2.0,
        Input::None => -3.0,
        Input::Result(n) => n as f32,
    }
}

fn input_of(v: f32) -> Option<Input> {
    Some(match v {
        -1.0 => Input::SourceGraphic,
        -2.0 => Input::SourceAlpha,
        -3.0 => Input::None,
        v if v >= 0.0 && v <= u16::MAX as f32 && v.fract() == 0.0 => Input::Result(v as u16),
        _ => return None,
    })
}

impl Filter {
    /// The chain as numbers: the region, the count, then per primitive its
    /// op code, inputs, subregion, colour space and the op's numbers.
    pub fn encode(&self) -> Vec<f32> {
        let (x, y, w, h) = self.region;
        let mut out = vec![x, y, w, h, self.primitives.len() as f32];
        for p in &self.primitives {
            let (sx, sy, sw, sh) = p.subregion;
            let code = match &p.op {
                Op::Blur(..) => 0.0,
                Op::Offset(..) => 1.0,
                Op::Flood(_) => 2.0,
                Op::Composite(..) => 3.0,
                Op::Merge(_) => 4.0,
                Op::ColorMatrix(_) => 5.0,
                Op::DropShadow(..) => 6.0,
                Op::Blend(_) => 7.0,
                Op::Morphology(..) => 8.0,
                Op::ComponentTransfer(_) => 9.0,
                Op::Tile => 10.0,
                Op::Turbulence(..) => 11.0,
                Op::Displacement(..) => 12.0,
                Op::Convolve(_) => 13.0,
                Op::Lighting(_) => 14.0,
            };
            out.extend([
                code,
                input_code(p.inputs[0]),
                input_code(p.inputs[1]),
                sx,
                sy,
                sw,
                sh,
                p.linear as u8 as f32,
            ]);
            match &p.op {
                Op::Blur(a, b) | Op::Offset(a, b) => out.extend([*a, *b]),
                Op::Flood(c) => out.extend(c),
                Op::Composite(op, k) => {
                    out.push(*op as u8 as f32);
                    out.extend(k);
                }
                Op::Merge(inputs) => {
                    out.push(inputs.len() as f32);
                    out.extend(inputs.iter().map(|i| input_code(*i)));
                }
                Op::ColorMatrix(m) => out.extend(m),
                Op::DropShadow(a, b, c, d, color) => {
                    out.extend([*a, *b, *c, *d]);
                    out.extend(color);
                }
                Op::Blend(m) => out.push(*m as f32),
                Op::Morphology(dilate, rx, ry) => out.extend([*dilate as u8 as f32, *rx, *ry]),
                Op::ComponentTransfer(funcs) => {
                    for f in funcs.iter() {
                        match f {
                            Transfer::Identity => out.push(0.0),
                            Transfer::Table(v) | Transfer::Discrete(v) => {
                                let kind = if matches!(f, Transfer::Table(_)) {
                                    1.0
                                } else {
                                    2.0
                                };
                                out.extend([kind, v.len() as f32]);
                                out.extend(v);
                            }
                            Transfer::Linear(s, i) => out.extend([3.0, *s, *i]),
                            Transfer::Gamma(a, e, o) => out.extend([4.0, *a, *e, *o]),
                        }
                    }
                }
                Op::Tile => {}
                Op::Turbulence(bx, by, octaves, seed, fractal, stitch) => out.extend([
                    *bx,
                    *by,
                    *octaves as f32,
                    *seed,
                    *fractal as u8 as f32,
                    *stitch as u8 as f32,
                ]),
                Op::Displacement(scale, x, y) => out.extend([*scale, *x as f32, *y as f32]),
                Op::Convolve(c) => {
                    out.extend([c.order.0 as f32, c.order.1 as f32]);
                    out.extend(&c.kernel);
                    out.extend([
                        c.divisor,
                        c.bias,
                        c.target.0 as f32,
                        c.target.1 as f32,
                        c.edge as f32,
                        c.preserve_alpha as u8 as f32,
                    ]);
                }
                Op::Lighting(l) => {
                    out.extend([
                        l.specular as u8 as f32,
                        l.surface_scale,
                        l.constant,
                        l.exponent,
                    ]);
                    out.extend(l.color);
                    match l.light {
                        Light::Distant(a, e) => out.extend([0.0, a, e]),
                        Light::Point(x, y, z) => out.extend([1.0, x, y, z]),
                        Light::Spot(p, e, cone) => {
                            out.push(2.0);
                            out.extend(p);
                            out.extend([e, cone.unwrap_or(f32::NAN)]);
                        }
                    }
                }
            }
        }
        out
    }

    /// [`Filter::encode`]'s inverse; `None` for anything it did not write.
    pub fn decode(v: &[f32]) -> Option<Filter> {
        let mut r = Reader(v);
        let region = (r.f()?, r.f()?, r.f()?, r.f()?);
        let n = r.n(4096)?;
        let mut primitives = Vec::with_capacity(n);
        for _ in 0..n {
            let code = r.n(14)?;
            let inputs = [input_of(r.f()?)?, input_of(r.f()?)?];
            let subregion = (r.f()?, r.f()?, r.f()?, r.f()?);
            let linear = r.f()? != 0.0;
            let op = match code {
                0 => Op::Blur(r.f()?, r.f()?),
                1 => Op::Offset(r.f()?, r.f()?),
                2 => Op::Flood([r.f()?, r.f()?, r.f()?, r.f()?]),
                3 => {
                    let op = [
                        CompositeOp::Over,
                        CompositeOp::In,
                        CompositeOp::Out,
                        CompositeOp::Atop,
                        CompositeOp::Xor,
                        CompositeOp::Arithmetic,
                        CompositeOp::Lighter,
                    ][r.n(6)?];
                    Op::Composite(op, [r.f()?, r.f()?, r.f()?, r.f()?])
                }
                4 => {
                    let k = r.n(4096)?;
                    let mut inputs = Vec::with_capacity(k);
                    for _ in 0..k {
                        inputs.push(input_of(r.f()?)?);
                    }
                    Op::Merge(inputs)
                }
                5 => {
                    let mut m = [0.0; 20];
                    for v in m.iter_mut() {
                        *v = r.f()?;
                    }
                    Op::ColorMatrix(m)
                }
                6 => Op::DropShadow(
                    r.f()?,
                    r.f()?,
                    r.f()?,
                    r.f()?,
                    [r.f()?, r.f()?, r.f()?, r.f()?],
                ),
                7 => Op::Blend(r.n(BLEND_MODES.len() - 1)? as u8),
                8 => Op::Morphology(r.f()? != 0.0, r.f()?, r.f()?),
                9 => {
                    let mut f = || -> Option<Transfer> {
                        Some(match r.n(4)? {
                            0 => Transfer::Identity,
                            k @ (1 | 2) => {
                                let len = r.n(4096)?;
                                let mut vals = Vec::with_capacity(len);
                                for _ in 0..len {
                                    vals.push(r.f()?);
                                }
                                if k == 1 {
                                    Transfer::Table(vals)
                                } else {
                                    Transfer::Discrete(vals)
                                }
                            }
                            3 => Transfer::Linear(r.f()?, r.f()?),
                            _ => Transfer::Gamma(r.f()?, r.f()?, r.f()?),
                        })
                    };
                    let funcs = [f()?, f()?, f()?, f()?];
                    Op::ComponentTransfer(Box::new(funcs))
                }
                10 => Op::Tile,
                11 => Op::Turbulence(
                    r.f()?,
                    r.f()?,
                    r.n(64)? as u32,
                    r.f()?,
                    r.f()? != 0.0,
                    r.f()? != 0.0,
                ),
                12 => Op::Displacement(r.f()?, r.n(3)? as u8, r.n(3)? as u8),
                13 => {
                    let order = (r.n(64)? as u32, r.n(64)? as u32);
                    let mut kernel = Vec::new();
                    for _ in 0..order.0 * order.1 {
                        kernel.push(r.f()?);
                    }
                    Op::Convolve(Box::new(Convolve {
                        order,
                        kernel,
                        divisor: r.f()?,
                        bias: r.f()?,
                        target: (r.n(63)? as u32, r.n(63)? as u32),
                        edge: r.n(2)? as u8,
                        preserve_alpha: r.f()? != 0.0,
                    }))
                }
                _ => {
                    let specular = r.f()? != 0.0;
                    let (surface_scale, constant, exponent) = (r.f()?, r.f()?, r.f()?);
                    let color = [r.f()?, r.f()?, r.f()?];
                    let light = match r.n(2)? {
                        0 => Light::Distant(r.f()?, r.f()?),
                        1 => Light::Point(r.f()?, r.f()?, r.f()?),
                        _ => {
                            let p = [r.f()?, r.f()?, r.f()?, r.f()?, r.f()?, r.f()?];
                            let e = r.f()?;
                            let cone = r.0.first().copied()?;
                            r.0 = &r.0[1..];
                            Light::Spot(p, e, (!cone.is_nan()).then_some(cone))
                        }
                    };
                    Op::Lighting(Box::new(Lighting {
                        specular,
                        surface_scale,
                        constant,
                        exponent,
                        color,
                        light,
                    }))
                }
            };
            primitives.push(Primitive {
                op,
                inputs,
                subregion,
                linear,
            });
        }
        r.0.is_empty().then_some(Filter { region, primitives })
    }
}

struct Reader<'a>(&'a [f32]);

impl Reader<'_> {
    /// A finite number.
    fn f(&mut self) -> Option<f32> {
        let (v, rest) = self.0.split_first()?;
        self.0 = rest;
        v.is_finite().then_some(*v)
    }

    /// A whole number no greater than `max`.
    fn n(&mut self, max: usize) -> Option<usize> {
        let v = self.f()?;
        (v >= 0.0 && v.fract() == 0.0 && v <= max as f32).then_some(v as usize)
    }
}

/// One entry of CSS `filter` on an SVG element (Filter Effects 1 §7,
/// §12): a reference to a `filter`, or a filter function.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterFn {
    /// `url(#id)`.
    Url(Box<str>),
    /// `blur(<length>)`, px.
    Blur(f32),
    /// `brightness()`, as a number.
    Brightness(f32),
    /// `contrast()`.
    Contrast(f32),
    /// `drop-shadow(<length>{2,3} <color>?)`: dx, dy, blur (px), colour
    /// (`currentcolor` when none is given).
    DropShadow(f32, f32, f32, Option<crate::style::Color>),
    /// `grayscale()`.
    Grayscale(f32),
    /// `hue-rotate(<angle>)`, degrees.
    HueRotate(f32),
    /// `invert()`.
    Invert(f32),
    /// `opacity()`.
    Opacity(f32),
    /// `saturate()`.
    Saturate(f32),
    /// `sepia()`.
    Sepia(f32),
}

/// CSS `filter`: `none` (empty) or a list of [`FilterFn`]s, applied in
/// order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FilterList(pub Vec<FilterFn>);

/// A number or percentage argument; `None` when absent.
fn amount(arg: &str) -> Option<Option<f32>> {
    let t = arg.trim();
    if t.is_empty() {
        return Some(None);
    }
    let (n, k) = match t.strip_suffix('%') {
        Some(n) => (n, 0.01),
        None => (t, 1.0),
    };
    let v = exact_num::parse_f64(n.trim()).ok()? as f32 * k;
    (v.is_finite() && v >= 0.0).then_some(Some(v))
}

/// A length in px (a bare 0 admitted).
fn px(t: &str) -> Option<f32> {
    let t = t.trim();
    let n = t
        .strip_suffix("px")
        .unwrap_or(if t == "0" { t } else { "" });
    let v = exact_num::parse_f64(n.trim()).ok()? as f32;
    v.is_finite().then_some(v)
}

/// An angle in degrees (`deg`, `rad`, `grad`, `turn`; a bare 0).
fn angle(t: &str) -> Option<f32> {
    let t = t.trim();
    for (unit, k) in [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f32::consts::PI),
        ("turn", 360.0),
    ] {
        if let Some(n) = t.strip_suffix(unit) {
            let v = exact_num::parse_f64(n.trim()).ok()? as f32 * k;
            return v.is_finite().then_some(v);
        }
    }
    (t == "0").then_some(0.0)
}

impl FilterList {
    /// CSS's grammar: `none`, or functions separated by white space.
    pub fn parse(css: &str) -> Option<FilterList> {
        let t = css.trim();
        if t.eq_ignore_ascii_case("none") {
            return Some(FilterList::default());
        }
        let mut out = Vec::new();
        let mut rest = t;
        while !rest.is_empty() {
            let open = rest.find('(')?;
            let name = rest[..open].trim().to_ascii_lowercase();
            // The matching parenthesis: a colour may hold its own.
            let mut depth = 0;
            let close = open
                + rest[open..].char_indices().find_map(|(i, c)| {
                    match c {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    (depth == 0).then_some(i)
                })?;
            let arg = &rest[open + 1..close];
            rest = rest[close + 1..].trim_start();
            let one = |d: f32| amount(arg).map(|a| a.unwrap_or(d));
            out.push(match name.as_str() {
                "url" => {
                    let id = arg.trim().trim_matches(|c| c == '"' || c == '\'');
                    FilterFn::Url(id.strip_prefix('#').filter(|i| !i.is_empty())?.into())
                }
                "blur" => FilterFn::Blur(if arg.trim().is_empty() {
                    0.0
                } else {
                    px(arg).filter(|v| *v >= 0.0)?
                }),
                "brightness" => FilterFn::Brightness(one(1.0)?),
                "contrast" => FilterFn::Contrast(one(1.0)?),
                "grayscale" => FilterFn::Grayscale(one(1.0)?.min(1.0)),
                "invert" => FilterFn::Invert(one(1.0)?.min(1.0)),
                "opacity" => FilterFn::Opacity(one(1.0)?.min(1.0)),
                "saturate" => FilterFn::Saturate(one(1.0)?),
                "sepia" => FilterFn::Sepia(one(1.0)?.min(1.0)),
                "hue-rotate" => FilterFn::HueRotate(if arg.trim().is_empty() {
                    0.0
                } else {
                    angle(arg)?
                }),
                "drop-shadow" => {
                    let mut lengths = Vec::new();
                    let mut color = None;
                    for word in split_words(arg) {
                        match px(word) {
                            Some(v) if lengths.len() < 3 => lengths.push(v),
                            _ if color.is_none() => color = Some(crate::style::Color::parse(word)?),
                            _ => return None,
                        }
                    }
                    if lengths.len() < 2 || lengths.get(2).is_some_and(|b| *b < 0.0) {
                        return None;
                    }
                    FilterFn::DropShadow(
                        lengths[0],
                        lengths[1],
                        lengths.get(2).copied().unwrap_or(0.0),
                        color,
                    )
                }
                _ => return None,
            });
        }
        Some(FilterList(out))
    }

    /// The value as CSS reads it.
    pub fn css(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        let n = |v: f32| exact_num::Shortest(v as f64).to_string();
        self.0
            .iter()
            .map(|f| match f {
                FilterFn::Url(id) => format!("url(#{id})"),
                FilterFn::Blur(v) => format!("blur({}px)", n(*v)),
                FilterFn::Brightness(v) => format!("brightness({})", n(*v)),
                FilterFn::Contrast(v) => format!("contrast({})", n(*v)),
                FilterFn::Grayscale(v) => format!("grayscale({})", n(*v)),
                FilterFn::HueRotate(v) => format!("hue-rotate({}deg)", n(*v)),
                FilterFn::Invert(v) => format!("invert({})", n(*v)),
                FilterFn::Opacity(v) => format!("opacity({})", n(*v)),
                FilterFn::Saturate(v) => format!("saturate({})", n(*v)),
                FilterFn::Sepia(v) => format!("sepia({})", n(*v)),
                FilterFn::DropShadow(x, y, b, c) => {
                    let color = c.map_or(String::new(), |c| format!(" #{:08x}", c.0));
                    format!("drop-shadow({}px {}px {}px{color})", n(*x), n(*y), n(*b))
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Every number is finite.
    pub fn is_finite(&self) -> bool {
        true
    }

    /// Whether the list is `none`.
    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }
}

/// Words separated by white space, a function's parentheses kept whole.
fn split_words(t: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, None);
    for (i, c) in t.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    out.push(&t[s..i]);
                }
                continue;
            }
            _ => {}
        }
        if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push(&t[s..]);
    }
    out
}
