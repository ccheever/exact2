//! `filter` on an SVG element (LLP 1055.000 D14): references to `filter`
//! elements and CSS filter functions, resolved into one chain in the
//! element's user space.
//!
//! @ref LLP 1055.000 D14; Filter Effects 1 §9 (regions, units, inputs),
//! §12 (the functions' equivalent primitives, run in sRGB)
//!
//! Each entry's `SourceGraphic` is the previous entry's result, so `url(#a)
//! blur(2px)` blurs what `a` made. A reference to nothing that is a
//! `filter` makes the element not render, as Chrome does.

use super::{cascade, Resolver, Viewport};
use crate::generated::{ColorInterpolationFilters, NodeType, PropId, StyleId, StyleMask};
use crate::kernel::NodeRef;
use crate::style::Dimension;
use crate::svg::filter::{
    CompositeOp, Convolve, Filter, FilterFn, Input, Light, Lighting, Op, Primitive, Transfer,
    BLEND_MODES,
};
use crate::svg::Paint;

type Rect = (f32, f32, f32, f32);

/// Numbers in an attribute, separated by white space and/or commas.
fn numbers(text: Option<&str>) -> Vec<f32> {
    text.map_or(Vec::new(), |t| {
        t.split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|p| !p.is_empty())
            .filter_map(|p| exact_num::parse_f64(p).ok().map(|v| v as f32))
            .filter(|v| v.is_finite())
            .collect()
    })
}

fn number(text: Option<&str>, default: f32) -> f32 {
    numbers(text).first().copied().unwrap_or(default)
}

fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    (
        x0,
        y0,
        (a.0 + a.2).max(b.0 + b.2) - x0,
        (a.1 + a.3).max(b.1 + b.3) - y0,
    )
}

/// A straight sRGB colour, 0–1, with an opacity folded in.
fn rgba(c: crate::style::Color, opacity: f32) -> [f32; 4] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0 * opacity.clamp(0.0, 1.0),
    ]
}

/// `feColorMatrix type="saturate"`.
pub(crate) fn saturate(s: f32) -> [f32; 20] {
    [
        0.213 + 0.787 * s,
        0.715 - 0.715 * s,
        0.072 - 0.072 * s,
        0.0,
        0.0,
        0.213 - 0.213 * s,
        0.715 + 0.285 * s,
        0.072 - 0.072 * s,
        0.0,
        0.0,
        0.213 - 0.213 * s,
        0.715 - 0.715 * s,
        0.072 + 0.928 * s,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
}

/// `feColorMatrix type="hueRotate"`, degrees.
pub(crate) fn hue_rotate(deg: f32) -> [f32; 20] {
    let (s, c) = deg.to_radians().sin_cos();
    [
        0.213 + c * 0.787 - s * 0.213,
        0.715 - c * 0.715 - s * 0.715,
        0.072 - c * 0.072 + s * 0.928,
        0.0,
        0.0,
        0.213 - c * 0.213 + s * 0.143,
        0.715 + c * 0.285 + s * 0.140,
        0.072 - c * 0.072 - s * 0.283,
        0.0,
        0.0,
        0.213 - c * 0.213 - s * 0.787,
        0.715 - c * 0.715 + s * 0.715,
        0.072 + c * 0.928 + s * 0.072,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
}

const IDENTITY: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    0.0,
];

/// A chain being built: primitives, and the index of the last result.
struct Chain {
    primitives: Vec<Primitive>,
    region: Option<Rect>,
}

impl Chain {
    /// The input standing for this entry's `SourceGraphic`.
    fn source(&self) -> Input {
        match self.primitives.len() {
            0 => Input::SourceGraphic,
            n => Input::Result(n as u16 - 1),
        }
    }

    fn push(&mut self, op: Op, inputs: [Input; 2], sub: Rect) -> Input {
        self.primitives.push(Primitive {
            op,
            inputs,
            subregion: sub,
            linear: false,
        });
        Input::Result(self.primitives.len() as u16 - 1)
    }
}

/// One CSS filter function (Filter Effects 1 §12) appended to `chain`, its
/// `SourceGraphic` the chain so far, growing the region from `visual` (the
/// box it filters, with any stroke) by what it spreads. Colours resolve
/// under the `dark` appearance; `text_color` already has.
fn function(
    chain: &mut Chain,
    f: &FilterFn,
    visual: Option<Rect>,
    text_color: crate::style::Color,
    dark: bool,
) {
    let src = chain.source();
    let grow = |chain: &mut Chain, by: Rect| {
        if let Some(v) = visual {
            let r = (v.0 + by.0, v.1 + by.1, v.2 + by.2, v.3 + by.3);
            chain.region = Some(chain.region.map_or(r, |c| union(c, r)));
        }
    };
    let all = (-1.0e6, -1.0e6, 2.0e6, 2.0e6);
    let transfer = |chain: &mut Chain, f: [Transfer; 4]| {
        chain.push(Op::ComponentTransfer(Box::new(f)), [src, Input::None], all);
    };
    match f {
        FilterFn::Url(_) => {}
        FilterFn::Blur(s) => {
            chain.push(Op::Blur(*s, *s), [src, Input::None], all);
            let g = 3.0 * s;
            grow(&mut *chain, (-g, -g, 2.0 * g, 2.0 * g));
        }
        FilterFn::DropShadow(dx, dy, blur, color) => {
            // A reference resolves as the host reported it (LLP 1095 D1).
            let color = color.map_or(text_color, |c| c.resolve(dark));
            // The third length is the standard deviation itself (Filter
            // Effects 1 §10.9: `feGaussianBlur stdDeviation="[radius]"`),
            // not `box-shadow`'s blur radius of 2σ: halved, F3's shadow fell
            // off at half CSS's distance and its region was cut at 1.5σ
            // (the iPad frames: exact2 1680 px wide at a threshold of 6/255,
            // Chrome and SwiftUI 1751).
            let s = *blur;
            chain.push(
                Op::DropShadow(s, s, *dx, *dy, rgba(color, 1.0)),
                [src, Input::None],
                all,
            );
            let g = 3.0 * s;
            grow(
                &mut *chain,
                (
                    dx.min(0.0) - g,
                    dy.min(0.0) - g,
                    dx.abs() + 2.0 * g,
                    dy.abs() + 2.0 * g,
                ),
            );
        }
        FilterFn::Brightness(a) => {
            let l = Transfer::Linear(*a, 0.0);
            transfer(&mut *chain, [l.clone(), l.clone(), l, Transfer::Identity]);
        }
        FilterFn::Contrast(a) => {
            let l = Transfer::Linear(*a, 0.5 - 0.5 * a);
            transfer(&mut *chain, [l.clone(), l.clone(), l, Transfer::Identity]);
        }
        FilterFn::Invert(a) => {
            let t = Transfer::Table(vec![*a, 1.0 - a]);
            transfer(&mut *chain, [t.clone(), t.clone(), t, Transfer::Identity]);
        }
        FilterFn::Opacity(a) => transfer(
            &mut *chain,
            [
                Transfer::Identity,
                Transfer::Identity,
                Transfer::Identity,
                Transfer::Table(vec![0.0, *a]),
            ],
        ),
        FilterFn::Saturate(a) => {
            chain.push(Op::ColorMatrix(saturate(*a)), [src, Input::None], all);
        }
        FilterFn::HueRotate(a) => {
            chain.push(Op::ColorMatrix(hue_rotate(*a)), [src, Input::None], all);
        }
        FilterFn::Grayscale(a) => {
            let s = 1.0 - a;
            let m = [
                0.2126 + 0.7874 * s,
                0.7152 - 0.7152 * s,
                0.0722 - 0.0722 * s,
                0.0,
                0.0,
                0.2126 - 0.2126 * s,
                0.7152 + 0.2848 * s,
                0.0722 - 0.0722 * s,
                0.0,
                0.0,
                0.2126 - 0.2126 * s,
                0.7152 - 0.7152 * s,
                0.0722 + 0.9278 * s,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ];
            chain.push(Op::ColorMatrix(m), [src, Input::None], all);
        }
        FilterFn::Sepia(a) => {
            let s = 1.0 - a;
            let m = [
                0.393 + 0.607 * s,
                0.769 - 0.769 * s,
                0.189 - 0.189 * s,
                0.0,
                0.0,
                0.349 - 0.349 * s,
                0.686 + 0.314 * s,
                0.168 - 0.168 * s,
                0.0,
                0.0,
                0.272 - 0.272 * s,
                0.534 - 0.534 * s,
                0.131 + 0.869 * s,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ];
            chain.push(Op::ColorMatrix(m), [src, Input::None], all);
        }
    }
    if !matches!(
        f,
        FilterFn::Url(_) | FilterFn::Blur(_) | FilterFn::DropShadow(..)
    ) {
        grow(&mut *chain, (0.0, 0.0, 0.0, 0.0));
    }
}

/// CSS `filter` on an HTML box (Filter Effects 1 §12): the functions' chain
/// over a box of no size, so its region is how far past the box the
/// result reaches (a host adds the box's size); its subregions are the
/// whole region. `None` for `none`, or a list naming a `filter` element,
/// which a box does not take. Its colours, `currentcolor` (`text_color`)
/// included, resolve under the `dark` appearance.
pub fn box_filter(
    list: &crate::svg::filter::FilterList,
    text_color: crate::style::ColorValue,
    dark: bool,
) -> Option<Filter> {
    if list.0.is_empty() || list.0.iter().any(|f| matches!(f, FilterFn::Url(_))) {
        return None;
    }
    let mut chain = Chain {
        primitives: Vec::new(),
        region: None,
    };
    for f in &list.0 {
        function(
            &mut chain,
            f,
            Some((0.0, 0.0, 0.0, 0.0)),
            text_color.resolve(dark),
            dark,
        );
    }
    Some(Filter {
        region: chain.region.unwrap_or((0.0, 0.0, 0.0, 0.0)),
        primitives: chain.primitives,
    })
}

impl Resolver<'_, '_> {
    /// The filter `node`'s `filter` row makes, for an element whose object
    /// bounding box is `bbox`; `None` for `none`.
    pub(super) fn filter(
        &mut self,
        node: &NodeRef<'_>,
        bbox: Option<Rect>,
        stroke: f32,
        vp: Viewport,
    ) -> Option<Filter> {
        let list = &node.style.filter;
        if list.0.is_empty() {
            return None;
        }
        let nothing = Filter {
            region: (0.0, 0.0, 0.0, 0.0),
            primitives: Vec::new(),
        };
        // What a function's region grows from: the box with its stroke.
        let visual = bbox.map(|b| {
            (
                b.0 - stroke,
                b.1 - stroke,
                b.2 + 2.0 * stroke,
                b.3 + 2.0 * stroke,
            )
        });
        let mut chain = Chain {
            primitives: Vec::new(),
            region: None,
        };
        for f in &list.0 {
            if let FilterFn::Url(id) = f {
                let Some(target) = self
                    .kernel
                    .resolve_id(node.id, id)
                    .and_then(|t| self.kernel.node(t))
                    .filter(|t| t.node_type == NodeType::SvgFilter)
                else {
                    return Some(nothing);
                };
                let Some(region) = self.filter_element(&target, bbox, vp, &mut chain) else {
                    return Some(nothing);
                };
                chain.region = Some(chain.region.map_or(region, |c| union(c, region)));
            } else {
                function(
                    &mut chain,
                    f,
                    visual,
                    node.style.text_color.resolve(false),
                    false,
                );
            }
        }
        let region = chain.region.unwrap_or((0.0, 0.0, 0.0, 0.0));
        if !(region.2 > 0.0 && region.3 > 0.0) {
            return Some(nothing);
        }
        // A function's subregion is the whole region.
        for p in &mut chain.primitives {
            if p.subregion.2 >= 1.0e6 {
                p.subregion = region;
            }
        }
        Some(Filter {
            region,
            primitives: chain.primitives,
        })
    }

    /// A `filter` element's primitives, appended to `chain` with its
    /// `SourceGraphic` standing for the chain so far; its region, or
    /// `None` when it renders nothing.
    fn filter_element(
        &mut self,
        f: &NodeRef<'_>,
        bbox: Option<Rect>,
        vp: Viewport,
        chain: &mut Chain,
    ) -> Option<Rect> {
        let obb = f.props.str(PropId::FilterUnits) != Some("userSpaceOnUse");
        let pobb = f.props.str(PropId::PrimitiveUnits) == Some("objectBoundingBox");
        let bx = bbox.filter(|b| b.2 > 0.0 && b.3 > 0.0);
        let coord = |d: Option<Dimension>, default: f32, obb: bool, basis: f32| match d {
            Some(Dimension::Points(v)) => v,
            Some(Dimension::Percent(p)) if obb => p / 100.0,
            Some(Dimension::Percent(p)) => p / 100.0 * basis,
            _ if obb => default,
            _ => default * basis,
        };
        let row = |n: &NodeRef<'_>, s: StyleId, d: Dimension| n.style.mask.has(s).then_some(d);
        let raw = (
            coord(row(f, StyleId::X, f.style.x), -0.1, obb, vp.width),
            coord(row(f, StyleId::Y, f.style.y), -0.1, obb, vp.height),
            coord(row(f, StyleId::Width, f.style.width), 1.2, obb, vp.width),
            coord(row(f, StyleId::Height, f.style.height), 1.2, obb, vp.height),
        );
        let region = match (obb, bx) {
            (true, Some(b)) => (
                b.0 + raw.0 * b.2,
                b.1 + raw.1 * b.3,
                raw.2 * b.2,
                raw.3 * b.3,
            ),
            (false, _) => raw,
            (true, None) => return None,
        };
        if !(region.2 > 0.0 && region.3 > 0.0) {
            return None;
        }
        if pobb && bx.is_none() {
            return None;
        }
        // Lengths in primitive units: a fraction of the box's axis, or
        // user units.
        let (kx, ky) = match (pobb, bx) {
            (true, Some(b)) => (b.2, b.3),
            _ => (1.0, 1.0),
        };
        let inherited = f.computed_style(StyleMask::INHERITED);
        let fstyle = cascade(f, &inherited);
        let base = chain.primitives.len() as u16;
        let source = chain.source();
        let mut names: Vec<(&str, u16)> = Vec::new();
        let mut alpha: Option<Input> = None;
        let children: Vec<NodeRef<'_>> = f
            .children()
            .into_iter()
            .filter_map(|c| self.kernel.node(c))
            .filter(|c| c.node_type == NodeType::SvgFe)
            .collect();
        for p in &children {
            let style = cascade(p, &fstyle);
            let kind = p.props.str(PropId::Fe).unwrap_or("");
            let prop = |id: PropId| p.props.str(id);
            let mut resolve = |text: Option<&str>, chain: &mut Chain| -> Input {
                match text.map(str::trim) {
                    Some("SourceGraphic") => source,
                    Some("SourceAlpha") => {
                        if source == Input::SourceGraphic {
                            return Input::SourceAlpha;
                        }
                        *alpha.get_or_insert_with(|| {
                            let mut m = [0.0; 20];
                            m[18] = 1.0;
                            chain.push(Op::ColorMatrix(m), [source, Input::None], region)
                        })
                    }
                    Some(name) if !name.is_empty() => names
                        .iter()
                        .rev()
                        .find(|(n, _)| *n == name)
                        .map_or(Input::None, |(_, i)| Input::Result(*i)),
                    // The previous result, or the source for the first.
                    _ => match chain.primitives.len() as u16 {
                        n if n > base => Input::Result(n - 1),
                        _ => source,
                    },
                }
            };
            let i1 = resolve(prop(PropId::In), chain);
            let i2 = resolve(prop(PropId::In2), chain);
            let pair = |v: Option<&str>, d: f32| {
                let n = numbers(v);
                match n.as_slice() {
                    [] => (d, d),
                    [a] => (*a, *a),
                    [a, b, ..] => (*a, *b),
                }
            };
            let op = match kind {
                "feGaussianBlur" => {
                    let (sx, sy) = pair(prop(PropId::StdDeviation), 0.0);
                    Op::Blur(sx * kx, sy * ky)
                }
                "feOffset" => Op::Offset(
                    number(prop(PropId::FeDx), 0.0) * kx,
                    number(prop(PropId::FeDy), 0.0) * ky,
                ),
                "feFlood" => Op::Flood(self.flood(&style)),
                "feDropShadow" => {
                    let (sx, sy) = pair(prop(PropId::StdDeviation), 2.0);
                    let c = self.flood(&style);
                    Op::DropShadow(
                        sx * kx,
                        sy * ky,
                        number(prop(PropId::FeDx), 2.0) * kx,
                        number(prop(PropId::FeDy), 2.0) * ky,
                        c,
                    )
                }
                "feComposite" => {
                    let op = match prop(PropId::Operator).unwrap_or("over") {
                        "in" => CompositeOp::In,
                        "out" => CompositeOp::Out,
                        "atop" => CompositeOp::Atop,
                        "xor" => CompositeOp::Xor,
                        "arithmetic" => CompositeOp::Arithmetic,
                        "lighter" => CompositeOp::Lighter,
                        _ => CompositeOp::Over,
                    };
                    let k = [PropId::K1, PropId::K2, PropId::K3, PropId::K4]
                        .map(|id| number(prop(id), 0.0));
                    Op::Composite(op, k)
                }
                "feMerge" => {
                    let nodes: Vec<Input> = p
                        .children()
                        .into_iter()
                        .filter_map(|c| self.kernel.node(c))
                        .filter(|c| c.props.str(PropId::Fe) == Some("feMergeNode"))
                        .map(|c| resolve(c.props.str(PropId::In), chain))
                        .collect();
                    Op::Merge(nodes)
                }
                "feColorMatrix" => {
                    let v = numbers(prop(PropId::Values));
                    Op::ColorMatrix(match prop(PropId::Type).unwrap_or("matrix") {
                        "saturate" => saturate(v.first().copied().unwrap_or(1.0)),
                        "hueRotate" => hue_rotate(v.first().copied().unwrap_or(0.0)),
                        "luminanceToAlpha" => {
                            let mut m = [0.0; 20];
                            m[15..18].copy_from_slice(&[0.2125, 0.7154, 0.0721]);
                            m
                        }
                        _ if v.len() == 20 => v.try_into().unwrap_or(IDENTITY),
                        _ => IDENTITY,
                    })
                }
                "feBlend" => {
                    let mode = prop(PropId::Mode).unwrap_or("normal");
                    Op::Blend(BLEND_MODES.iter().position(|m| *m == mode).unwrap_or(0) as u8)
                }
                "feMorphology" => {
                    let (rx, ry) = pair(prop(PropId::FeRadius), 0.0);
                    Op::Morphology(prop(PropId::Operator) == Some("dilate"), rx * kx, ry * ky)
                }
                "feComponentTransfer" => {
                    let mut funcs = [
                        Transfer::Identity,
                        Transfer::Identity,
                        Transfer::Identity,
                        Transfer::Identity,
                    ];
                    for c in p.children().into_iter().filter_map(|c| self.kernel.node(c)) {
                        let slot = match c.props.str(PropId::Fe) {
                            Some("feFuncR") => 0,
                            Some("feFuncG") => 1,
                            Some("feFuncB") => 2,
                            Some("feFuncA") => 3,
                            _ => continue,
                        };
                        let q = |id: PropId| c.props.str(id);
                        funcs[slot] = match q(PropId::Type).unwrap_or("identity") {
                            "table" => Transfer::Table(numbers(q(PropId::TableValues))),
                            "discrete" => Transfer::Discrete(numbers(q(PropId::TableValues))),
                            "linear" => Transfer::Linear(
                                number(q(PropId::Slope), 1.0),
                                number(q(PropId::Intercept), 0.0),
                            ),
                            "gamma" => Transfer::Gamma(
                                number(q(PropId::Amplitude), 1.0),
                                number(q(PropId::Exponent), 1.0),
                                number(q(PropId::Offset), 0.0),
                            ),
                            _ => Transfer::Identity,
                        };
                    }
                    Op::ComponentTransfer(Box::new(funcs))
                }
                "feTile" => Op::Tile,
                "feTurbulence" => {
                    let (bx, by) = pair(prop(PropId::BaseFrequency), 0.0);
                    Op::Turbulence(
                        bx,
                        by,
                        number(prop(PropId::NumOctaves), 1.0).max(0.0) as u32,
                        number(prop(PropId::Seed), 0.0),
                        prop(PropId::Type) == Some("fractalNoise"),
                        prop(PropId::StitchTiles) == Some("stitch"),
                    )
                }
                "feDisplacementMap" => {
                    let ch = |id: PropId| match prop(id) {
                        Some("R") => 0,
                        Some("G") => 1,
                        Some("B") => 2,
                        _ => 3,
                    };
                    Op::Displacement(
                        number(prop(PropId::FeScale), 0.0) * kx,
                        ch(PropId::XChannelSelector),
                        ch(PropId::YChannelSelector),
                    )
                }
                "feConvolveMatrix" => {
                    let (ox, oy) = pair(prop(PropId::FeOrder), 3.0);
                    let (ox, oy) = (ox.max(1.0) as u32, oy.max(1.0) as u32);
                    let kernel = numbers(prop(PropId::KernelMatrix));
                    let sum: f32 = kernel.iter().sum();
                    let divisor = match number(prop(PropId::Divisor), 0.0) {
                        0.0 if sum != 0.0 => sum,
                        0.0 => 1.0,
                        d => d,
                    };
                    Op::Convolve(Box::new(Convolve {
                        order: (ox, oy),
                        target: (
                            number(prop(PropId::TargetX), (ox / 2) as f32) as u32,
                            number(prop(PropId::TargetY), (oy / 2) as f32) as u32,
                        ),
                        kernel,
                        divisor,
                        bias: number(prop(PropId::Bias), 0.0),
                        edge: match prop(PropId::EdgeMode) {
                            Some("wrap") => 1,
                            Some("none") => 2,
                            _ => 0,
                        },
                        preserve_alpha: prop(PropId::PreserveAlpha) == Some("true"),
                    }))
                }
                "feDiffuseLighting" | "feSpecularLighting" => {
                    let specular = kind == "feSpecularLighting";
                    let light = p
                        .children()
                        .into_iter()
                        .filter_map(|c| self.kernel.node(c))
                        .find_map(|c| {
                            let q = |id: PropId, d: f32| number(c.props.str(id), d);
                            Some(match c.props.str(PropId::Fe)? {
                                "feDistantLight" => Light::Distant(
                                    q(PropId::Azimuth, 0.0),
                                    q(PropId::Elevation, 0.0),
                                ),
                                "fePointLight" => Light::Point(
                                    q(PropId::LightX, 0.0),
                                    q(PropId::LightY, 0.0),
                                    q(PropId::LightZ, 0.0),
                                ),
                                "feSpotLight" => Light::Spot(
                                    [
                                        q(PropId::LightX, 0.0),
                                        q(PropId::LightY, 0.0),
                                        q(PropId::LightZ, 0.0),
                                        q(PropId::PointsAtX, 0.0),
                                        q(PropId::PointsAtY, 0.0),
                                        q(PropId::PointsAtZ, 0.0),
                                    ],
                                    q(PropId::SpecularExponent, 1.0),
                                    numbers(c.props.str(PropId::LimitingConeAngle))
                                        .first()
                                        .copied(),
                                ),
                                _ => return None,
                            })
                        });
                    let Some(light) = light else {
                        // No light: transparent black.
                        chain.push(Op::Flood([0.0; 4]), [Input::None, Input::None], region);
                        if let Some(name) = prop(PropId::Result) {
                            names.push((name, chain.primitives.len() as u16 - 1));
                        }
                        continue;
                    };
                    let color = self.lighting_color(&style);
                    let constant = if specular {
                        number(prop(PropId::SpecularConstant), 1.0)
                    } else {
                        number(prop(PropId::DiffuseConstant), 1.0)
                    };
                    Op::Lighting(Box::new(Lighting {
                        specular,
                        surface_scale: number(prop(PropId::SurfaceScale), 1.0),
                        constant,
                        exponent: number(prop(PropId::SpecularExponent), 1.0).clamp(1.0, 128.0),
                        color,
                        light,
                    }))
                }
                // Not a primitive this resolver draws (`feImage`): its
                // result is transparent black.
                _ => Op::Flood([0.0; 4]),
            };
            // The subregion: its own x, y, width, height (a fraction of the
            // box under objectBoundingBox), else the region's; clipped to it.
            let sub = {
                let b = bx.unwrap_or((0.0, 0.0, 1.0, 1.0));
                let len = |s: StyleId, d: Dimension, basis: f32, bo: f32, bs: f32| {
                    p.style.mask.has(s).then(|| match (pobb, d) {
                        (true, Dimension::Percent(v)) => bo + v / 100.0 * bs,
                        (true, d) => bo + crate::svg::length::resolve(d, 1.0) * bs,
                        (false, d) => crate::svg::length::resolve(d, basis),
                    })
                };
                let x = len(StyleId::X, p.style.x, vp.width, b.0, b.2).unwrap_or(region.0);
                let y = len(StyleId::Y, p.style.y, vp.height, b.1, b.3).unwrap_or(region.1);
                let w = len(StyleId::Width, p.style.width, vp.width, 0.0, b.2)
                    .unwrap_or(region.0 + region.2 - x);
                let h = len(StyleId::Height, p.style.height, vp.height, 0.0, b.3)
                    .unwrap_or(region.1 + region.3 - y);
                let x0 = x.max(region.0);
                let y0 = y.max(region.1);
                let x1 = (x + w).min(region.0 + region.2);
                let y1 = (y + h).min(region.1 + region.3);
                (x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
            };
            let linear = style.color_interpolation_filters == ColorInterpolationFilters::LinearRGB;
            chain.primitives.push(Primitive {
                op,
                inputs: [i1, i2],
                subregion: sub,
                linear,
            });
            if let Some(name) = prop(PropId::Result).filter(|n| !n.is_empty()) {
                names.push((name, chain.primitives.len() as u16 - 1));
            }
        }
        if chain.primitives.len() as u16 == base {
            // SVG 2: a filter with no primitives renders nothing.
            return None;
        }
        Some(region)
    }

    /// `flood-color` with `flood-opacity`, straight sRGB.
    fn flood(&self, style: &crate::generated::StyleProps) -> [f32; 4] {
        let c = match &style.rare.flood_color {
            Paint::CurrentColor => style.text_color.resolve(false),
            Paint::Color(c) => c.resolve(false),
            _ => crate::style::Color(0x0000_00ff),
        };
        rgba(c, style.rare.flood_opacity)
    }

    /// `lighting-color`, straight sRGB.
    fn lighting_color(&self, style: &crate::generated::StyleProps) -> [f32; 3] {
        let c = match &style.rare.lighting_color {
            Paint::CurrentColor => style.text_color.resolve(false),
            Paint::Color(c) => c.resolve(false),
            _ => crate::style::Color(0xffff_ffff),
        };
        let v = rgba(c, 1.0);
        [v[0], v[1], v[2]]
    }
}
