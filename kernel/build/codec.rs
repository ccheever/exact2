const CODEC_PATHS: [&str; 6] = [
    "../vendor/taffy/src/style/grid.rs",
    "build.rs",
    "build/codec.rs",
    "src/style/compare.rs",
    "src/style/grid.rs",
    "src/wire/codec.rs",
];
const SCHEMA_PATH: &str = "tables/schema.json";
const DIGEST_DOMAIN: &[u8] = b"exact-kernel-schema-v1\0";

enum GridSeam {
    Decode,
    Dynamic,
}

fn emit_grid_seam(w: &mut String, id: &str, seam: GridSeam) -> bool {
    if !matches!(
        id,
        "GridAutoFlow"
            | "GridTemplateColumns"
            | "GridTemplateRows"
            | "GridColumn"
            | "GridRow"
            | "JustifyItems"
    ) {
        return false;
    }
    if id == "GridAutoFlow" {
        let line = match seam {
            GridSeam::Decode => "crate::style::decode_grid_rows(&mut out, mask, r)?;",
            GridSeam::Dynamic => "StyleId::GridAutoFlow | StyleId::GridTemplateColumns | StyleId::GridTemplateRows | StyleId::GridColumn | StyleId::GridRow | StyleId::JustifyItems => crate::style::set_grid_dynamic(self, id, value)?,",
        };
        writeln!(w, "            {line}").unwrap();
    }
    true
}

fn emit_style_encoding(w: &mut String, schema: &Schema) {
    w.push_str(
        "    /// Encode the rows in `mask` as a masked patch.\n\
         pub fn encode_masked(&self, mask: StyleMask, w: &mut Writer) {\n\
             self.encode_overlaid(mask, self, w);\n\
         }\n\
         /// Encode these rows, taking explicitly patched rows from `patch`.\n\
         pub(crate) fn encode_overlaid(&self, mask: StyleMask, patch: &StyleProps, w: &mut Writer) {\n\
             w.style_mask(mask);\n\
             w.style_mask(self.written.minus(patch.mask).union(patch.written.intersect(patch.mask)).intersect(mask));\n\
             for id in mask.iter() {\n\
                 let source = if patch.mask.has(id) { patch } else { self };\n\
                 match id {\n",
    );
    for row in &schema.styles {
        let id = pascal(&row.field);
        // Keep grid's optional wasm codec behind its existing linked seam.
        let encode = if matches!(
            id.as_str(),
            "GridAutoFlow" | "GridTemplateColumns" | "GridTemplateRows"
                | "GridColumn" | "GridRow" | "JustifyItems"
        ) {
            "crate::style::encode_grid_rows(source, StyleMask::of(id), w);".into()
        } else {
            parse_codec(&row.codec).encode_stmt(&format!("source.{}", at(row)))
        };
        writeln!(w, "                    StyleId::{id} => {{ {encode} }}").unwrap();
    }
    w.push_str("                }\n            }\n        }\n");
}

enum Codec {
    Dimension,
    LineHeight,
    F32,
    U8,
    U16,
    U32,
    I32,
    Rgba8,
    ColorValue,
    KeywordColor(&'static str),
    Vec2,
    Color2,
    Tracks,
    Placement,
    Transitions,
    Animations,
    CssValue {
        path: &'static str,
        variant: &'static str,
        error: &'static str,
    },
    Enum(String),
}
fn parse_codec(s: &str) -> Codec {
    match s {
        "dimension" => Codec::Dimension,
        "line-height" => Codec::LineHeight,
        "f32" => Codec::F32,
        "u8" => Codec::U8,
        "u16" => Codec::U16,
        "u32" => Codec::U32,
        "i32" => Codec::I32,
        "rgba8" => Codec::Rgba8,
        "color" => Codec::ColorValue,
        "auto-color" => Codec::KeywordColor("auto"),
        "current-color" => Codec::KeywordColor("currentcolor"),
        "vec2" => Codec::Vec2,
        "color2" => Codec::Color2,
        "tracks" => Codec::Tracks,
        "placement" => Codec::Placement,
        "transitions" => Codec::Transitions,
        "animations" => Codec::Animations,
        // @ref LLP 1055 D2 — SVG paint and dash lists travel as their CSS text.
        "paint" => Codec::CssValue {
            path: "crate::svg::Paint",
            variant: "Paint",
            error: "BadPaint",
        },
        "dasharray" => Codec::CssValue {
            path: "crate::svg::DashArray",
            variant: "DashArray",
            error: "BadDashArray",
        },
        // @ref LLP 1055.000 D5 — CSS transforms on SVG elements, as CSS text.
        "transform" => Codec::CssValue {
            path: "crate::svg::TransformList",
            variant: "Transform",
            error: "BadTransform",
        },
        "transform-origin" => Codec::CssValue {
            path: "crate::svg::TransformOrigin",
            variant: "TransformOrigin",
            error: "BadTransformOrigin",
        },
        "paint-order" => Codec::CssValue {
            path: "crate::svg::PaintOrder",
            variant: "PaintOrder",
            error: "BadPaintOrder",
        },
        "marker" => Codec::CssValue {
            path: "crate::svg::MarkerRef",
            variant: "Marker",
            error: "BadMarker",
        },
        "filter" => Codec::CssValue {
            path: "crate::svg::filter::FilterList",
            variant: "Filter",
            error: "BadFilter",
        },
        // @ref LLP 1043.000 §3 D1 — one parse/css/default codec for both shapes.
        "clip-path" => Codec::CssValue {
            path: "crate::clip::ClipPath",
            variant: "ClipPath",
            error: "BadClipPath",
        },
        "aspect-ratio" => Codec::CssValue {
            path: "crate::ratio::AspectRatio",
            variant: "AspectRatio",
            error: "BadAspectRatio",
        },
        "shape-outside" => Codec::CssValue {
            path: "exact_textflow::ShapeOutside",
            variant: "ShapeOutside",
            error: "BadShapeOutside",
        },
        // @ref LLP 1066 D1
        "background-image" => Codec::CssValue {
            path: "crate::gradient::BackgroundImage",
            variant: "BackgroundImage",
            error: "BadBackgroundImage",
        },
        // @ref LLP 1077 D1–D4
        "symbol-palette" => Codec::CssValue {
            path: "crate::style::symbols::SymbolPalette",
            variant: "SymbolPalette",
            error: "BadSymbolPalette",
        },
        "rotate-axis" => Codec::CssValue {
            path: "crate::style::space::RotateAxis",
            variant: "RotateAxis",
            error: "BadRotateAxis",
        },
        "backdrop-filter" => Codec::CssValue {
            path: "crate::style::BackdropFilter",
            variant: "BackdropFilter",
            error: "BadBackdropFilter",
        },
        "box-shadow" => Codec::CssValue {
            path: "crate::style::BoxShadows",
            variant: "BoxShadow",
            error: "BadBoxShadow",
        },
        "text-shadow" => Codec::CssValue {
            path: "crate::style::TextShadow",
            variant: "TextShadow",
            error: "BadTextShadow",
        },
        "mask-image" => Codec::CssValue {
            path: "crate::gradient::BackgroundImage",
            variant: "MaskImage",
            error: "BadMaskImage",
        },
        "corner-shape" => Codec::CssValue {
            path: "crate::corner::CornerShape",
            variant: "CornerShape",
            error: "BadCornerShape",
        },
        // @ref LLP 1057.003 D1 — drag timelines, CSS scroll-timeline's shape.
        "drag-timeline" => Codec::CssValue {
            path: "crate::timeline::DragTimeline",
            variant: "DragTimeline",
            error: "BadDragTimeline",
        },
        "animation-timeline" => Codec::CssValue {
            path: "crate::timeline::AnimationTimeline",
            variant: "AnimationTimeline",
            error: "BadAnimationTimeline",
        },
        "animation-range" => Codec::CssValue {
            path: "crate::timeline::AnimationRange",
            variant: "AnimationRange",
            error: "BadAnimationRange",
        },
        "timeline-scope" => Codec::CssValue {
            path: "crate::timeline::TimelineScope",
            variant: "TimelineScope",
            error: "BadTimelineScope",
        },
        other => match other.strip_prefix("enum:") {
            Some(name) => Codec::Enum(name.to_string()),
            None => panic!("schema: unknown codec `{other}`"),
        },
    }
}
impl Codec {
    fn rust_type(&self) -> String {
        match self {
            Codec::Dimension => "Dimension".into(),
            Codec::LineHeight => "LineHeight".into(),
            Codec::F32 => "f32".into(),
            Codec::U8 => "u8".into(),
            Codec::U16 => "u16".into(),
            Codec::U32 => "u32".into(),
            Codec::I32 => "i32".into(),
            Codec::Rgba8 => "Color".into(),
            Codec::ColorValue => "ColorValue".into(),
            Codec::KeywordColor(_) => "Option<ColorValue>".into(),
            Codec::Vec2 => "Vec2".into(),
            Codec::Color2 => "[Color; 2]".into(),
            Codec::Tracks => "GridTracks".into(),
            Codec::Placement => "GridPlacement".into(),
            Codec::Transitions => "Transitions".into(),
            Codec::Animations => "Animations".into(),
            Codec::CssValue { path, .. } => (*path).into(),
            Codec::Enum(name) => name.clone(),
        }
    }
    fn variant(&self) -> &'static str {
        match self {
            Codec::Dimension => "Dimension",
            Codec::LineHeight => "LineHeight",
            Codec::F32 => "F32",
            Codec::U8 => "U8",
            Codec::U16 => "U16",
            Codec::U32 => "U32",
            Codec::I32 => "I32",
            Codec::Rgba8 => "Rgba8",
            Codec::ColorValue => "ColorValue",
            Codec::KeywordColor(_) => "KeywordColor",
            Codec::Vec2 => "Vec2",
            Codec::Color2 => "Color2",
            Codec::Tracks => "Tracks",
            Codec::Placement => "Placement",
            Codec::Transitions => "Transitions",
            Codec::Animations => "Animations",
            Codec::CssValue { variant, .. } => variant,
            Codec::Enum(_) => "Enum",
        }
    }
    fn default_expr(&self, value: &serde_json::Value, field: &str) -> String {
        let num = |v: &serde_json::Value| -> f64 {
            v.as_f64()
                .unwrap_or_else(|| panic!("schema: style `{field}` default must be a number"))
        };
        // Refuse out-of-range defaults instead of silently saturating.
        let int = |v: &serde_json::Value, lo: f64, hi: f64| -> i64 {
            let n = num(v);
            assert!(
                n.fract() == 0.0 && n >= lo && n <= hi,
                "schema: style `{field}` default {n} is not an integer in range"
            );
            n as i64
        };
        match self {
            Codec::Dimension => match value {
                serde_json::Value::String(s) if s == "auto" => "Dimension::Auto".into(),
                serde_json::Value::Number(_) => format!("Dimension::Points({}f32)", num(value)),
                serde_json::Value::Null => {
                    panic!("schema: dimension style `{field}` needs a default")
                }
                _ => panic!("schema: bad dimension default on `{field}`"),
            },
            Codec::LineHeight => {
                assert_eq!(value.as_str(), Some("normal"));
                "LineHeight::Normal".into()
            }
            Codec::F32 => format!("{}f32", num(value)),
            Codec::U8 => format!("{}u8", int(value, 0.0, u8::MAX as f64)),
            Codec::U16 => format!("{}u16", int(value, 0.0, u16::MAX as f64)),
            Codec::U32 => format!("{}u32", int(value, 0.0, u32::MAX as f64)),
            Codec::I32 => format!("{}i32", int(value, i32::MIN as f64, i32::MAX as f64)),
            Codec::Rgba8 => format!("Color({}u32)", int(value, 0.0, u32::MAX as f64)),
            Codec::ColorValue => format!(
                "ColorValue::Fixed(Color({}u32))",
                int(value, 0.0, u32::MAX as f64)
            ),
            // The keyword itself, or a colour (`background-color` is
            // transparent initially and still takes `currentcolor`).
            Codec::KeywordColor(keyword) if value.is_number() => format!(
                "Some(ColorValue::Fixed(Color({}u32)))",
                int(value, 0.0, u32::MAX as f64)
            ),
            Codec::KeywordColor(keyword) => {
                assert_eq!(value.as_str(), Some(*keyword));
                "None".into()
            }
            Codec::Vec2 => {
                let arr = value
                    .as_array()
                    .unwrap_or_else(|| panic!("schema: vec2 default on `{field}` must be [x, y]"));
                format!("Vec2 {{ x: {}f32, y: {}f32 }}", num(&arr[0]), num(&arr[1]))
            }
            Codec::Color2 => {
                let arr = value.as_array().unwrap_or_else(|| {
                    panic!("schema: color2 default on `{field}` must be [a, b]")
                });
                format!(
                    "[Color({}u32), Color({}u32)]",
                    num(&arr[0]) as u32,
                    num(&arr[1]) as u32
                )
            }
            Codec::Tracks => {
                assert!(
                    value.is_null(),
                    "schema: `{field}` (tracks) cannot declare a default"
                );
                "GridTracks::default()".into()
            }
            // Paint's initial value differs by row: `fill` the text's colour
            // (LLP 1115.000 D3.8), `stroke` none, a gradient stop black.
            Codec::CssValue {
                variant: "Paint", ..
            } => match value.as_str() {
                Some("currentcolor") => "crate::svg::Paint::CurrentColor".into(),
                Some("black") => "crate::svg::Paint::BLACK".into(),
                Some("none") => "crate::svg::Paint::None".into(),
                Some("white") => "crate::svg::Paint::WHITE".into(),
                _ => panic!(
                    "schema: paint default on `{field}` must be currentcolor, black, white or none"
                ),
            },
            Codec::CssValue { path, .. } => format!("{path}::default()"),
            Codec::Transitions => {
                assert!(
                    value.is_null(),
                    "schema: `{field}` (transitions) cannot declare a default"
                );
                "Transitions::default()".into()
            }
            Codec::Animations => {
                assert!(
                    value.is_null(),
                    "schema: `{field}` (animations) cannot declare a default"
                );
                "Animations::default()".into()
            }
            Codec::Placement => {
                assert!(
                    value.is_null(),
                    "schema: `{field}` (placement) cannot declare a default"
                );
                "GridPlacement::default()".into()
            }
            Codec::Enum(name) => match value {
                serde_json::Value::Null => format!("{name}::default()"),
                serde_json::Value::String(s) => format!("{name}::{}", pascal(s)),
                _ => panic!("schema: enum default on `{field}` must be a string"),
            },
        }
    }
    fn decode_expr(&self, style_id: &str, _admits_auto: bool) -> String {
        match self {
            // Row-specific admission is checked once by StyleProps::validate_domain.
            Codec::Dimension => format!("r.dimension(StyleId::{style_id}, true)?"),
            Codec::LineHeight => "r.line_height()?".into(),
            Codec::F32 => "r.f32()?".into(),
            Codec::U8 => "r.u8()?".into(),
            Codec::U16 => "r.u16()?".into(),
            Codec::U32 => "r.u32()?".into(),
            Codec::I32 => "r.i32()?".into(),
            Codec::Rgba8 => "r.color()?".into(),
            Codec::ColorValue => "r.color_value()?".into(),
            Codec::KeywordColor(_) => "r.optional_color()?".into(),
            Codec::Vec2 => "r.vec2()?".into(),
            Codec::Color2 => "r.color2()?".into(),
            Codec::Tracks => "r.tracks_for_style()?".into(),
            Codec::Placement => "r.placement_for_style()?".into(),
            Codec::Transitions => "r.transitions()?".into(),
            Codec::Animations => "r.animations()?".into(),
            Codec::CssValue { path, error, .. } => format!("{path}::parse(r.string()?).ok_or(crate::error::DecodeError::{error})?"),
            Codec::Enum(name) => format!(
                "{{ let v = r.u8()?; {name}::from_wire(v).ok_or(DecodeError::UnknownEnumValue {{ style: StyleId::{style_id}, value: v }})? }}"
            ),
        }
    }
    fn encode_stmt(&self, access: &str) -> String {
        match self {
            Codec::Dimension => format!("w.dimension({access});"),
            Codec::LineHeight => format!("w.line_height({access});"),
            Codec::F32 => format!("w.f32({access});"),
            Codec::U8 => format!("w.u8({access});"),
            Codec::U16 => format!("w.u16({access});"),
            Codec::U32 => format!("w.u32({access});"),
            Codec::I32 => format!("w.i32({access});"),
            Codec::Rgba8 => format!("w.color({access});"),
            Codec::ColorValue => format!("w.color_value({access});"),
            Codec::KeywordColor(_) => format!("w.optional_color({access});"),
            Codec::Vec2 => format!("w.vec2({access});"),
            Codec::Color2 => format!("w.color2({access});"),
            Codec::Tracks => format!("w.tracks(&{access});"),
            Codec::Placement => format!("w.placement(&{access});"),
            Codec::Transitions => format!("w.transitions(&{access});"),
            Codec::Animations => format!("w.animations(&{access});"),
            // @ref LLP 1095 D1 — a colour-carrying value keeps its references.
            Codec::CssValue {
                variant:
                    "Paint" | "Filter" | "BackgroundImage" | "MaskImage" | "BoxShadow" | "TextShadow"
                    | "SymbolPalette",
                ..
            } => format!("w.string(&{access}.wire());"),
            Codec::CssValue { .. } => format!("w.string(&{access}.css());"),
            Codec::Enum(_) => format!("w.u8({access} as u8);"),
        }
    }
    /// Whether the field type is `Copy` (so encode can pass by value).
    fn is_copy(&self) -> bool {
        !matches!(
            self,
            Codec::Tracks
                | Codec::Placement
                | Codec::Transitions
                | Codec::Animations
                | Codec::CssValue { .. }
        )
    }
}

/// A keyword row's vocabulary as bits (`schema.json` `keywords`), emitted into
/// its enum's `impl`: the first value alone is the empty set, each later value
/// `i` is bit `i - 1`; repeats, unknown words and the empty list are `None`.
const KEYWORD_BITS: &str = r#"    /// A space-separated keyword list as a row's bits: the first value alone
    /// is 0, each other value `i` sets bit `i - 1`; repeats and unknown words are `None`.
    pub fn bits(text: &str) -> Option<u8> {
        let mut bits = 0u8;
        let mut words = 0;
        for word in text.split_ascii_whitespace() {
            words += 1;
            let i = Self::from_name(word)? as u8;
            if i == 0 { if words > 1 || text.split_ascii_whitespace().count() > 1 { return None; } continue; }
            let bit = 1u8 << (i - 1);
            if bits & bit != 0 { return None; }
            bits |= bit;
        }
        (words > 0).then_some(bits)
    }
    /// A row's bits as CSS: the set values in vocabulary order, or the first.
    pub fn css(bits: u8) -> String {
        let words: Vec<&str> = Self::ALL[1..].iter().filter(|v| bits & (1u8 << (**v as u8 - 1)) != 0).map(|v| v.name()).collect();
        if words.is_empty() { Self::ALL[0].name().to_string() } else { words.join(" ") }
    }
"#;

/// A colour row's initial value naming a role (`"CanvasText"`, LLP 1095
/// stage 2): the platform's own colour where a host has it, so an unstyled
/// node follows the system's appearance and contrast.
/// A row's initial value as Rust. @ref LLP 1095 stage 2 — a colour default
/// may name a role.
fn default_of(
    codec: &Codec,
    default: &serde_json::Value,
    field: &str,
    colors: &[[String; 8]],
) -> String {
    match (codec, default.as_str()) {
        (Codec::ColorValue, Some(name)) => role_default(colors, name, field),
        (Codec::KeywordColor(keyword), Some(name)) if name != *keyword => {
            format!("Some({})", role_default(colors, name, field))
        }
        _ => codec.default_expr(default, field),
    }
}

pub fn role_default(colors: &[[String; 8]], name: &str, field: &str) -> String {
    let id = colors
        .iter()
        .position(|c| c[0] == name)
        .unwrap_or_else(|| panic!("schema: style `{field}` default `{name}` is no role"));
    format!("ColorValue::Role({id}u8)")
}

/// @ref LLP 1095 D2 — the colour roles' type and table, from the schema's
/// `colors`: CSS's system colours, then Exact's.
pub fn write_color_roles(w: &mut String, colors: &[[String; 8]], rgba: impl Fn(&str) -> u32) {
    w.push_str(concat!(
        "/// One colour role (LLP 1095 D2): each Apple platform's class colour\n",
        "/// property (`@tint` for the view's inherited tint, `/a` for an alpha),\n",
        "/// and the two pairs a host without the platform's colour may show:\n",
        "/// Apple's, and the neutral one (LLP 1115.000 D3.2; `roles::Column`).\n",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n",
        "pub struct ColorRole {\n",
        "    /// The role's name: CSS's spelling for a system colour, which is how\n    /// it is written; else Exact's role, written `-exact-<name>` (LLP 1081\n    /// D2) and carried as `var(--exact-<name>, …)` on the web.\n    pub name: &'static str,\n",
        "    /// WebKit's `-apple-system-*` name this role answers to, or empty.\n    pub alias: &'static str,\n",
        "    /// `UIColor`'s class property.\n    pub ios: &'static str,\n",
        "    /// `NSColor`'s class property.\n    pub macos: &'static str,\n",
        "    /// Apple's, under a light scheme, RGBA.\n    pub light: u32,\n",
        "    /// Apple's, under a dark scheme, RGBA.\n    pub dark: u32,\n",
        "    /// The neutral column's, under a light scheme, RGBA.\n    pub neutral_light: u32,\n",
        "    /// The neutral column's, under a dark scheme, RGBA.\n    pub neutral_dark: u32,\n",
        "}\n",
        "/// Every role, by id (`ColorValue::Role`).\n",
        "pub const COLOR_ROLES: &[ColorRole] = &[\n",
    ));
    for [name, alias, ios, macos, light, dark, neutral_light, neutral_dark] in colors {
        w.push_str(&format!(
            "ColorRole {{ name: {name:?}, alias: {alias:?}, ios: {ios:?}, macos: {macos:?}, light: {:#010x}, dark: {:#010x}, neutral_light: {:#010x}, neutral_dark: {:#010x} }},\n",
            rgba(light),
            rgba(dark),
            rgba(neutral_light),
            rgba(neutral_dark)
        ));
    }
    w.push_str("];\n");
}
/// A `textStyles` row: name, WebKit alias, UIFont's and NSFont's text
/// style, weight, and the size at each of `textStyleBodies`.
type TextStyleRow = (String, String, String, String, u16, Vec<f32>);
/// `textStyles` (LLP 1115 D3): the platform's type ramp.
pub fn write_text_styles(w: &mut String, bodies: &[f32], styles: &[TextStyleRow]) {
    assert!(
        !bodies.is_empty() && bodies.windows(2).all(|p| p[0] < p[1]),
        "schema: textStyleBodies ascend"
    );
    assert!(styles.len() <= 64, "schema: at most 64 text styles");
    w.push_str(concat!(
        "/// One platform text style (LLP 1115 D3): written `-exact-<name>` (or\n",
        "/// its WebKit alias) as a `font-size`, and what a heading's level lowers to.\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub struct TextStyle {\n",
        "    /// The style's name, without `-exact-`.\n    pub name: &'static str,\n",
        "    /// WebKit's `-apple-system-*` name for it, or empty.\n    pub alias: &'static str,\n",
        "    /// `UIFont.TextStyle`'s.\n    pub ios: &'static str,\n",
        "    /// `NSFont.TextStyle`'s.\n    pub macos: &'static str,\n",
        "    /// Its weight, CSS's number.\n    pub weight: u16,\n",
        "    /// Its size at each of [`TEXT_STYLE_BODIES`].\n    pub sizes: &'static [f32],\n",
        "}\n",
    ));
    w.push_str(&format!(
        "/// The body sizes (root font sizes) the ramp is given at.\npub const TEXT_STYLE_BODIES: &[f32] = &{bodies:?};\n"
    ));
    w.push_str("/// Every text style, by id.\npub const TEXT_STYLES: &[TextStyle] = &[\n");
    for (name, alias, ios, macos, weight, sizes) in styles {
        assert_eq!(sizes.len(), bodies.len(), "schema: text style {name} has a size per body");
        w.push_str(&format!(
            "TextStyle {{ name: {name:?}, alias: {alias:?}, ios: {ios:?}, macos: {macos:?}, weight: {weight}, sizes: &{sizes:?} }},\n"
        ));
    }
    w.push_str("];\n");
}
/// `lengths` (LLP 1115.000 D10.3): the named lengths, each written
/// `-exact-<name>`. What one resolves to is `style/relative.rs`'s.
pub fn write_lengths(w: &mut String, lengths: &[String]) {
    assert!(
        lengths.iter().all(|name| !name.is_empty()
            && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')),
        "schema: a named length is kebab-case"
    );
    w.push_str(&format!(
        "/// Every named length, without `-exact-` (LLP 1115.000 D10.3).\npub const NAMED_LENGTHS: &[&str] = &{lengths:?};\n"
    ));
}
/// A `metrics` cell: a number (px on a length row), or the CSS text of the
/// row it is written on.
#[derive(Deserialize)]
#[serde(untagged)]
enum MetricCell {
    Number(f64),
    Text(String),
}
/// A `metrics` row: the name, the kind, then iOS's, the Mac's, the web's and
/// the neutral look's value; or the name, the kind and the neutral look's
/// value alone.
type MetricRow = Vec<MetricCell>;
/// One cell of a `kind` row as the generated value. A `style` cell is a
/// number or CSS text; a `length` is px as a number, or `em` or `rem` as
/// text; a `number` is a number. `unset` is no value in that column.
fn metric_cell(name: &str, kind: &str, cell: &MetricCell) -> String {
    match (kind, cell) {
        (_, MetricCell::Number(n)) => {
            assert!(n.is_finite(), "schema: metric {name} is a finite number");
            format!("MetricValue::Number({n:?})")
        }
        (_, MetricCell::Text(text)) if text == "unset" => "MetricValue::Text(\"unset\")".into(),
        ("style", MetricCell::Text(text)) => {
            assert!(
                !text.is_empty() && text.trim() == text && text.parse::<f64>().is_err(),
                "schema: metric {name}: a number is written as one; text is CSS, `unset` for no row"
            );
            format!("MetricValue::Text({text:?})")
        }
        ("length", MetricCell::Text(text)) => {
            let (unit, variant) = if text.ends_with("rem") {
                ("rem", "Rem")
            } else {
                ("em", "Em")
            };
            let n: f64 = text
                .strip_suffix(unit)
                .and_then(|n| n.parse().ok())
                .filter(|n: &f64| n.is_finite())
                .unwrap_or_else(|| panic!("schema: metric {name}: a length is px as a number, or `<n>em` or `<n>rem`: {text}"));
            format!("MetricValue::{variant}({n:?})")
        }
        _ => panic!("schema: metric {name}: a {kind} is a number"),
    }
}
/// `metrics` (LLP 1115.002 D6, D8.4; LLP 1115.000 D17): the looks Exact
/// draws itself, a column a platform or a look. What reads one is
/// `style/metrics.rs`. `taken` is every other name written `-exact-<name>`,
/// which a metric's must not be.
fn write_metrics(w: &mut String, metrics: &[MetricRow], taken: &[&str]) {
    w.push_str(concat!(
        "/// What a metric is in one column (LLP 1115.002 D6; LLP 1115.000 D17).\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub enum MetricValue {\n",
        "    /// Pixels, or a row's bare number.\n    Number(f64),\n",
        "    /// A `style` row's CSS text; `unset` in any kind: no value in this column.\n    Text(&'static str),\n",
        "    /// A length in `em`: this many of the control's font size.\n    Em(f64),\n",
        "    /// A length in `rem`: this many of the root font size.\n    Rem(f64),\n",
        "}\n",
        "/// What a metric's values are, which says what reads it.\n",
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n",
        "pub enum MetricKind {\n",
        "    /// The CSS value of the style row it is written on: a compiler's sheet names it `-exact-<name>`.\n    Style,\n",
        "    /// A length code reads (`style::metrics::length`).\n    Length,\n",
        "    /// A ratio, a weight or an opacity code reads (`style::metrics::number`).\n    Number,\n",
        "}\n",
        "/// One metric of a look Exact draws itself: its value in each column.\n",
        "#[derive(Debug, Clone, Copy, PartialEq)]\n",
        "pub struct Metric {\n",
        "    /// The metric's name, without `-exact-`.\n    pub name: &'static str,\n",
        "    /// What its values are.\n    pub kind: MetricKind,\n",
        "    /// On iOS.\n    pub ios: MetricValue,\n",
        "    /// On the Mac.\n    pub macos: MetricValue,\n",
        "    /// In the web's first look, which a process reads until its host says otherwise.\n    pub web: MetricValue,\n",
        "    /// In the neutral look: the web's, and a host's that paints its own controls.\n    pub neutral: MetricValue,\n",
        "}\n",
    ));
    let mut names = BTreeSet::new();
    let mut rows = String::new();
    let mut ids = String::new();
    for (index, row) in metrics.iter().enumerate() {
        let (MetricCell::Text(name), MetricCell::Text(kind)) = (&row[0], &row[1]) else {
            panic!("schema: a metric row opens with its name and its kind");
        };
        assert!(
            name.starts_with(|c: char| c.is_ascii_lowercase())
                && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'),
            "schema: a metric's name is kebab-case: {name}"
        );
        assert!(
            names.insert(name.as_str()) && !taken.contains(&name.as_str()),
            "schema: metric {name} is already an `-exact-` name"
        );
        let variant = match kind.as_str() {
            "style" => "Style",
            "length" => "Length",
            "number" => "Number",
            _ => panic!("schema: metric {name}: a kind is style, length or number"),
        };
        let unset = MetricCell::Text("unset".into());
        let [ios, macos, web, neutral] = match &row[2..] {
            [neutral] => [&unset, &unset, &unset, neutral],
            [ios, macos, web, neutral] => [ios, macos, web, neutral],
            _ => panic!("schema: metric {name}: a row is its name, its kind and a value for iOS, the Mac, the web and the neutral look, or for the neutral look alone"),
        }
        .map(|cell| metric_cell(name, kind, cell));
        rows.push_str(&format!(
            "Metric {{ name: {name:?}, kind: MetricKind::{variant}, ios: {ios}, macos: {macos}, web: {web}, neutral: {neutral} }},\n"
        ));
        if kind != "style" {
            ids.push_str(&format!(
                "    /// `{name}`.\n    pub const {}: usize = {index};\n",
                name.replace('-', "_").to_ascii_uppercase()
            ));
        }
    }
    w.push_str("/// Every metric, in the schema's order.\npub const METRICS: &[Metric] = &[\n");
    w.push_str(&rows);
    w.push_str("];\n");
    w.push_str("/// Where each metric that code reads is in [`METRICS`], by its name.\npub mod metric {\n");
    w.push_str(&ids);
    w.push_str("}\n");
}
/// Every name the schema has that is written `-exact-<name>` and is no
/// metric: a named length, a text style, a colour role.
fn exact_names(schema: &Schema) -> Vec<&str> {
    let lengths = schema.lengths.iter().map(String::as_str);
    let styles = schema.text_styles.iter().map(|s| s.0.as_str());
    let roles = schema.colors.iter().map(|c| c[0].as_str());
    lengths.chain(styles).chain(roles).collect()
}
/// `#rrggbb` or `#rrggbbaa` as RGBA.
fn hex_rgba(hex: &str) -> [u8; 4] {
    let h = hex
        .strip_prefix('#')
        .expect("schema: a tint is #rrggbb[aa]");
    assert!(matches!(h.len(), 6 | 8), "schema: a tint is #rrggbb[aa]");
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).expect("schema: a tint is hex");
    [
        byte(0),
        byte(2),
        byte(4),
        if h.len() == 8 { byte(6) } else { 255 },
    ]
}
