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
    CssValue { path: &'static str, variant: &'static str, error: &'static str },
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
        "paint" => Codec::CssValue { path: "crate::svg::Paint", variant: "Paint", error: "BadPaint" },
        "dasharray" => Codec::CssValue { path: "crate::svg::DashArray", variant: "DashArray", error: "BadDashArray" },
        // @ref LLP 1055.000 D5 — CSS transforms on SVG elements, as CSS text.
        "transform" => Codec::CssValue { path: "crate::svg::TransformList", variant: "Transform", error: "BadTransform" },
        "transform-origin" => Codec::CssValue { path: "crate::svg::TransformOrigin", variant: "TransformOrigin", error: "BadTransformOrigin" },
        "paint-order" => Codec::CssValue { path: "crate::svg::PaintOrder", variant: "PaintOrder", error: "BadPaintOrder" },
        "marker" => Codec::CssValue { path: "crate::svg::MarkerRef", variant: "Marker", error: "BadMarker" },
        "filter" => Codec::CssValue { path: "crate::svg::filter::FilterList", variant: "Filter", error: "BadFilter" },
        // @ref LLP 1043.000 §3 D1 — one parse/css/default codec for both shapes.
        "clip-path" => Codec::CssValue { path: "crate::clip::ClipPath", variant: "ClipPath", error: "BadClipPath" },
        "aspect-ratio" => Codec::CssValue { path: "crate::ratio::AspectRatio", variant: "AspectRatio", error: "BadAspectRatio" },
        "shape-outside" => Codec::CssValue { path: "exact_textflow::ShapeOutside", variant: "ShapeOutside", error: "BadShapeOutside" },
        // @ref LLP 1066 D1
        "background-image" => Codec::CssValue { path: "crate::gradient::BackgroundImage", variant: "BackgroundImage", error: "BadBackgroundImage" },
        // @ref LLP 1057.002 §6.7 — spike A: drag timelines, CSS scroll-timeline's shape.
        "drag-timeline" => Codec::CssValue { path: "crate::timeline::DragTimeline", variant: "DragTimeline", error: "BadDragTimeline" },
        "animation-timeline" => Codec::CssValue { path: "crate::timeline::AnimationTimeline", variant: "AnimationTimeline", error: "BadAnimationTimeline" },
        "animation-range" => Codec::CssValue { path: "crate::timeline::AnimationRange", variant: "AnimationRange", error: "BadAnimationRange" },
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
            // Paint's initial value differs by row: `fill` black, `stroke` none.
            Codec::CssValue { variant: "Paint", .. } => match value.as_str() {
                Some("black") => "crate::svg::Paint::BLACK".into(),
                Some("none") => "crate::svg::Paint::None".into(),
                Some("white") => "crate::svg::Paint::WHITE".into(),
                _ => panic!("schema: paint default on `{field}` must be black, white or none"),
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
            Codec::Placement => format!("w.placement({access});"),
            Codec::Transitions => format!("w.transitions(&{access});"),
            Codec::Animations => format!("w.animations(&{access});"),
            Codec::CssValue { .. } => format!("w.string(&{access}.css());"),
            Codec::Enum(_) => format!("w.u8({access} as u8);"),
        }
    }
    /// Whether the field type is `Copy` (so encode can pass by value).
    fn is_copy(&self) -> bool {
        !matches!(
            self,
            Codec::Tracks | Codec::Transitions | Codec::Animations | Codec::CssValue { .. }
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
