//! Generates the kernel's typed vocabulary from `tables/schema.json`.
//!
//! One declaration authority, generated code: node types, prop ids and their
//! value kinds, every style row (its field, codec, mask bit, defaults, and
//! layout/text effects), the enum vocabularies, the opcode list, and the
//! schema digest that every EXWF frame must carry. The generated file is built
//! into `OUT_DIR` and never committed.
//!
//! This generator fails closed: any table it cannot validate is a build error,
//! never a skipped row.

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const SCHEMA_PATH: &str = "tables/schema.json";
const DIGEST_DOMAIN: &[u8] = b"exact-kernel-schema-v1\0";

#[derive(Deserialize)]
struct Schema {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(rename = "nodeTypes")]
    node_types: Vec<NodeTypeRow>,
    props: Vec<PropRow>,
    enums: std::collections::BTreeMap<String, EnumDef>,
    styles: Vec<StyleRow>,
    opcodes: Vec<OpcodeRow>,
}

#[derive(Deserialize)]
struct NodeTypeRow {
    id: u8,
    name: String,
}

#[derive(Deserialize)]
struct PropRow {
    id: u16,
    name: String,
    kind: String,
    #[serde(default)]
    measure: bool,
}

#[derive(Deserialize)]
struct EnumDef {
    values: Vec<String>,
    default: String,
}

#[derive(Deserialize)]
struct StyleRow {
    bit: u32,
    field: String,
    codec: String,
    #[serde(rename = "admitsAuto", default)]
    admits_auto: bool,
    #[serde(default)]
    layout: bool,
    #[serde(default)]
    text: bool,
    #[serde(default)]
    default: serde_json::Value,
}

#[derive(Deserialize)]
struct OpcodeRow {
    id: u16,
    name: String,
}

/// Convert `snake_case`/`kebab-case`/`camelCase` to `PascalCase`.
fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            upper = true;
            continue;
        }
        if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Convert `camelCase` to `snake_case`.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.extend(ch.to_lowercase());
        } else if ch == '-' {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out
}

enum Codec {
    Dimension,
    F32,
    U8,
    U16,
    U32,
    I32,
    Rgba8,
    ColorValue,
    Vec2,
    Color2,
    Tracks,
    Placement,
    Transitions,
    Enum(String),
}

fn parse_codec(s: &str) -> Codec {
    match s {
        "dimension" => Codec::Dimension,
        "f32" => Codec::F32,
        "u8" => Codec::U8,
        "u16" => Codec::U16,
        "u32" => Codec::U32,
        "i32" => Codec::I32,
        "rgba8" => Codec::Rgba8,
        "color" => Codec::ColorValue,
        "vec2" => Codec::Vec2,
        "color2" => Codec::Color2,
        "tracks" => Codec::Tracks,
        "placement" => Codec::Placement,
        "transitions" => Codec::Transitions,
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
            Codec::F32 => "f32".into(),
            Codec::U8 => "u8".into(),
            Codec::U16 => "u16".into(),
            Codec::U32 => "u32".into(),
            Codec::I32 => "i32".into(),
            Codec::Rgba8 => "Color".into(),
            Codec::ColorValue => "ColorValue".into(),
            Codec::Vec2 => "Vec2".into(),
            Codec::Color2 => "[Color; 2]".into(),
            Codec::Tracks => "GridTracks".into(),
            Codec::Placement => "GridPlacement".into(),
            Codec::Transitions => "Transitions".into(),
            Codec::Enum(name) => name.clone(),
        }
    }

    fn variant(&self) -> &'static str {
        match self {
            Codec::Dimension => "Dimension",
            Codec::F32 => "F32",
            Codec::U8 => "U8",
            Codec::U16 => "U16",
            Codec::U32 => "U32",
            Codec::I32 => "I32",
            Codec::Rgba8 => "Rgba8",
            Codec::ColorValue => "ColorValue",
            Codec::Vec2 => "Vec2",
            Codec::Color2 => "Color2",
            Codec::Tracks => "Tracks",
            Codec::Placement => "Placement",
            Codec::Transitions => "Transitions",
            Codec::Enum(_) => "Enum",
        }
    }

    fn default_expr(&self, value: &serde_json::Value, field: &str) -> String {
        let num = |v: &serde_json::Value| -> f64 {
            v.as_f64()
                .unwrap_or_else(|| panic!("schema: style `{field}` default must be a number"))
        };
        // Integer defaults must be integral and in range; saturating casts would
        // silently turn a typo into a different default.
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
            Codec::Transitions => {
                assert!(
                    value.is_null(),
                    "schema: `{field}` (transitions) cannot declare a default"
                );
                "Transitions::default()".into()
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
            Codec::F32 => "r.f32()?".into(),
            Codec::U8 => "r.u8()?".into(),
            Codec::U16 => "r.u16()?".into(),
            Codec::U32 => "r.u32()?".into(),
            Codec::I32 => "r.i32()?".into(),
            Codec::Rgba8 => "r.color()?".into(),
            Codec::ColorValue => "r.color_value()?".into(),
            Codec::Vec2 => "r.vec2()?".into(),
            Codec::Color2 => "r.color2()?".into(),
            Codec::Tracks => "r.tracks_for_style()?".into(),
            Codec::Placement => "r.placement_for_style()?".into(),
            Codec::Transitions => "r.transitions()?".into(),
            Codec::Enum(name) => format!(
                "{{ let v = r.u8()?; {name}::from_wire(v).ok_or(DecodeError::UnknownEnumValue {{ style: StyleId::{style_id}, value: v }})? }}"
            ),
        }
    }

    fn encode_stmt(&self, access: &str) -> String {
        match self {
            Codec::Dimension => format!("w.dimension({access});"),
            Codec::F32 => format!("w.f32({access});"),
            Codec::U8 => format!("w.u8({access});"),
            Codec::U16 => format!("w.u16({access});"),
            Codec::U32 => format!("w.u32({access});"),
            Codec::I32 => format!("w.i32({access});"),
            Codec::Rgba8 => format!("w.color({access});"),
            Codec::ColorValue => format!("w.color_value({access});"),
            Codec::Vec2 => format!("w.vec2({access});"),
            Codec::Color2 => format!("w.color2({access});"),
            Codec::Tracks => format!("w.tracks(&{access});"),
            Codec::Placement => format!("w.placement({access});"),
            Codec::Transitions => format!("w.transitions(&{access});"),
            Codec::Enum(_) => format!("w.u8({access} as u8);"),
        }
    }

    /// Whether the field type is `Copy` (so encode can pass by value).
    fn is_copy(&self) -> bool {
        !matches!(self, Codec::Tracks | Codec::Transitions)
    }
}

fn validate(schema: &Schema) {
    assert_eq!(
        schema.schema_version, 1,
        "schema: unsupported schemaVersion"
    );

    let mut ids = BTreeSet::new();
    for (i, row) in schema.node_types.iter().enumerate() {
        assert_eq!(
            row.id as usize, i,
            "schema: node type ids must be contiguous from 0"
        );
        assert!(
            ids.insert(row.name.clone()),
            "schema: duplicate node type `{}`",
            row.name
        );
    }

    let mut prop_ids = BTreeSet::new();
    let mut prop_names = BTreeSet::new();
    for row in &schema.props {
        assert!(
            prop_ids.insert(row.id),
            "schema: duplicate prop id {}",
            row.id
        );
        assert!(
            prop_names.insert(row.name.clone()),
            "schema: duplicate prop `{}`",
            row.name
        );
        assert!(
            matches!(row.kind.as_str(), "str" | "bool" | "int" | "float"),
            "schema: prop `{}` has unknown kind `{}`",
            row.name,
            row.kind
        );
    }

    for (name, def) in &schema.enums {
        assert!(
            !def.values.is_empty(),
            "schema: enum `{name}` has no values"
        );
        assert!(def.values.len() <= 255, "schema: enum `{name}` exceeds u8");
        let set: BTreeSet<_> = def.values.iter().collect();
        assert_eq!(
            set.len(),
            def.values.len(),
            "schema: enum `{name}` has duplicate values"
        );
        assert!(
            def.values.contains(&def.default),
            "schema: enum `{name}` default is not a member"
        );
    }

    let mut fields = BTreeSet::new();
    for (i, row) in schema.styles.iter().enumerate() {
        assert_eq!(
            row.bit as usize, i,
            "schema: style bits must be contiguous from 0 (`{}`)",
            row.field
        );
        assert!(
            fields.insert(row.field.clone()),
            "schema: duplicate style field `{}`",
            row.field
        );
        let codec = parse_codec(&row.codec);
        if let Codec::Enum(name) = &codec {
            assert!(
                schema.enums.contains_key(name),
                "schema: style `{}` references unknown enum `{name}`",
                row.field
            );
        }
        if row.admits_auto {
            assert!(
                matches!(codec, Codec::Dimension),
                "schema: admitsAuto only applies to dimension rows (`{}`)",
                row.field
            );
        }
    }
    assert!(
        schema.styles.len() <= 64 * 4,
        "schema: more style rows than the mask can carry"
    );

    for row in &schema.styles {
        match parse_codec(&row.codec) {
            Codec::Enum(name) => match &row.default {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) => assert!(
                    schema.enums[name.as_str()].values.contains(s),
                    "schema: style `{}` default `{s}` is not a member of enum `{name}`",
                    row.field
                ),
                _ => panic!(
                    "schema: style `{}` enum default must be a string",
                    row.field
                ),
            },
            Codec::Tracks | Codec::Placement => {
                assert!(
                    row.default.is_null(),
                    "schema: style `{}` cannot declare a default",
                    row.field
                )
            }
            _ => {}
        }
    }

    let mut op_ids = BTreeSet::new();
    let mut op_names = BTreeSet::new();
    for row in &schema.opcodes {
        assert!(row.id != 0, "schema: opcode 0 is reserved");
        assert!(op_ids.insert(row.id), "schema: duplicate opcode {}", row.id);
        assert!(
            op_names.insert(row.name.clone()),
            "schema: duplicate opcode `{}`",
            row.name
        );
    }
}

fn digest(canonical: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update(canonical.as_bytes());
    let bytes = hasher.finalize();
    let mut first = [0u8; 8];
    first.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(first)
}

fn generate(schema: &Schema, digest: u64) -> String {
    let mut o = String::new();
    let w = &mut o;
    let mask_words = schema.styles.len().div_ceil(64).max(1);

    writeln!(
        w,
        "// Generated by build.rs from {SCHEMA_PATH}. Do not edit."
    )
    .unwrap();
    writeln!(w, "use crate::error::{{DecodeError, StyleDomainError}};").unwrap();
    writeln!(
        w,
        "use crate::error::StyleValueError;\nuse crate::style::{{Color, ColorValue, Dimension, GridPlacement, GridTracks, RowValue, StyleValue, Transitions, Vec2, MAX_GRID_TRACKS}};"
    )
    .unwrap();
    writeln!(w, "use crate::wire::codec::{{Reader, Writer}};").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "/// Domain-separated SHA-256 (first 8 bytes, little-endian) of the canonical schema."
    )
    .unwrap();
    writeln!(w, "pub const SCHEMA_DIGEST: u64 = {digest:#018x};").unwrap();
    writeln!(w, "/// The schema version the digest was computed under.").unwrap();
    writeln!(
        w,
        "pub const SCHEMA_VERSION: u32 = {};",
        schema.schema_version
    )
    .unwrap();
    writeln!(w).unwrap();

    // ---- NodeType --------------------------------------------------------
    writeln!(w, "/// Kernel node category. The closed v1 tag set.").unwrap();
    writeln!(w, "#[repr(u8)]").unwrap();
    writeln!(
        w,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
    )
    .unwrap();
    writeln!(w, "pub enum NodeType {{").unwrap();
    for row in &schema.node_types {
        writeln!(w, "    {} = {},", row.name, row.id).unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl NodeType {{").unwrap();
    writeln!(w, "    /// Every node type, in id order.").unwrap();
    writeln!(
        w,
        "    pub const ALL: [NodeType; {}] = [{}];",
        schema.node_types.len(),
        schema
            .node_types
            .iter()
            .map(|r| format!("NodeType::{}", r.name))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    writeln!(
        w,
        "    /// Decode a wire discriminant; unknown values are `None`."
    )
    .unwrap();
    writeln!(w, "    pub fn from_wire(value: u8) -> Option<Self> {{").unwrap();
    writeln!(w, "        match value {{").unwrap();
    for row in &schema.node_types {
        writeln!(w, "            {} => Some(NodeType::{}),", row.id, row.name).unwrap();
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// The tag name as authored.").unwrap();
    writeln!(w, "    pub fn name(self) -> &'static str {{").unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.node_types {
        writeln!(w, "            NodeType::{} => \"{}\",", row.name, row.name).unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Look a tag name up; unknown names are `None`.").unwrap();
    writeln!(w, "    pub fn from_name(name: &str) -> Option<Self> {{").unwrap();
    writeln!(w, "        match name {{").unwrap();
    for row in &schema.node_types {
        writeln!(
            w,
            "            \"{}\" => Some(NodeType::{}),",
            row.name, row.name
        )
        .unwrap();
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();

    // ---- PropKind / PropId -----------------------------------------------
    writeln!(w, "/// Declared value type of a prop. The wire carries values typed; a mismatch is a decode rejection.").unwrap();
    writeln!(w, "#[repr(u8)]").unwrap();
    writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
    writeln!(
        w,
        "pub enum PropKind {{ Str = 0, Bool = 1, Int = 2, Float = 3 }}"
    )
    .unwrap();
    writeln!(w, "impl PropKind {{").unwrap();
    writeln!(w, "    /// Decode a wire discriminant.").unwrap();
    writeln!(w, "    pub fn from_wire(value: u8) -> Option<Self> {{").unwrap();
    writeln!(w, "        match value {{ 0 => Some(PropKind::Str), 1 => Some(PropKind::Bool), 2 => Some(PropKind::Int), 3 => Some(PropKind::Float), _ => None }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "/// Interned property identifier. The discriminant is the wire id."
    )
    .unwrap();
    writeln!(w, "#[repr(u16)]").unwrap();
    writeln!(
        w,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
    )
    .unwrap();
    writeln!(w, "pub enum PropId {{").unwrap();
    for row in &schema.props {
        writeln!(w, "    {} = {},", pascal(&row.name), row.id).unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl PropId {{").unwrap();
    writeln!(w, "    /// Every prop, in id order.").unwrap();
    writeln!(
        w,
        "    pub const ALL: [PropId; {}] = [{}];",
        schema.props.len(),
        schema
            .props
            .iter()
            .map(|r| format!("PropId::{}", pascal(&r.name)))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    writeln!(w, "    /// Decode a wire id; unknown ids are `None`.").unwrap();
    writeln!(w, "    pub fn from_wire(value: u16) -> Option<Self> {{").unwrap();
    writeln!(w, "        match value {{").unwrap();
    for row in &schema.props {
        writeln!(
            w,
            "            {} => Some(PropId::{}),",
            row.id,
            pascal(&row.name)
        )
        .unwrap();
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// The authored prop name.").unwrap();
    writeln!(w, "    pub fn name(self) -> &'static str {{").unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.props {
        writeln!(
            w,
            "            PropId::{} => \"{}\",",
            pascal(&row.name),
            row.name
        )
        .unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Look a prop name up; unknown names are `None`.").unwrap();
    writeln!(w, "    pub fn from_name(name: &str) -> Option<Self> {{").unwrap();
    writeln!(w, "        match name {{").unwrap();
    for row in &schema.props {
        writeln!(
            w,
            "            \"{}\" => Some(PropId::{}),",
            row.name,
            pascal(&row.name)
        )
        .unwrap();
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// The declared value kind.").unwrap();
    writeln!(w, "    pub fn kind(self) -> PropKind {{").unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.props {
        let kind = match row.kind.as_str() {
            "str" => "Str",
            "bool" => "Bool",
            "int" => "Int",
            _ => "Float",
        };
        writeln!(
            w,
            "            PropId::{} => PropKind::{},",
            pascal(&row.name),
            kind
        )
        .unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Whether a change to this prop invalidates a text leaf's intrinsic size."
    )
    .unwrap();
    writeln!(w, "    pub fn affects_measure(self) -> bool {{").unwrap();
    let measure: Vec<_> = schema
        .props
        .iter()
        .filter(|r| r.measure)
        .map(|r| format!("PropId::{}", pascal(&r.name)))
        .collect();
    if measure.is_empty() {
        writeln!(w, "        false").unwrap();
    } else {
        writeln!(w, "        matches!(self, {})", measure.join(" | ")).unwrap();
    }
    writeln!(w, "    }}").unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();

    // ---- Enums -----------------------------------------------------------
    for (name, def) in &schema.enums {
        writeln!(
            w,
            "/// Wire vocabulary `{name}`; the discriminant is the wire byte."
        )
        .unwrap();
        writeln!(w, "#[repr(u8)]").unwrap();
        writeln!(
            w,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]"
        )
        .unwrap();
        writeln!(w, "pub enum {name} {{").unwrap();
        for (i, v) in def.values.iter().enumerate() {
            if *v == def.default {
                writeln!(w, "    #[default]").unwrap();
            }
            writeln!(w, "    {} = {},", pascal(v), i).unwrap();
        }
        writeln!(w, "}}").unwrap();
        writeln!(w, "impl {name} {{").unwrap();
        writeln!(w, "    /// Every value, in wire order.").unwrap();
        writeln!(
            w,
            "    pub const ALL: [{name}; {}] = [{}];",
            def.values.len(),
            def.values
                .iter()
                .map(|v| format!("{name}::{}", pascal(v)))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
        writeln!(w, "    /// Decode a wire byte; unknown values are `None`.").unwrap();
        writeln!(w, "    pub fn from_wire(value: u8) -> Option<Self> {{").unwrap();
        writeln!(w, "        match value {{").unwrap();
        for (i, v) in def.values.iter().enumerate() {
            writeln!(w, "            {i} => Some({name}::{}),", pascal(v)).unwrap();
        }
        writeln!(w, "            _ => None,").unwrap();
        writeln!(w, "        }}").unwrap();
        writeln!(w, "    }}").unwrap();
        writeln!(w, "    /// The authored spelling.").unwrap();
        writeln!(w, "    pub fn name(self) -> &'static str {{").unwrap();
        writeln!(w, "        match self {{").unwrap();
        for v in &def.values {
            writeln!(w, "            {name}::{} => \"{v}\",", pascal(v)).unwrap();
        }
        writeln!(w, "        }}").unwrap();
        writeln!(w, "    }}").unwrap();
        writeln!(
            w,
            "    /// Look an authored spelling up; unknown spellings are `None`, never a fallback."
        )
        .unwrap();
        writeln!(w, "    pub fn from_name(name: &str) -> Option<Self> {{").unwrap();
        writeln!(w, "        match name {{").unwrap();
        for v in &def.values {
            writeln!(w, "            \"{v}\" => Some({name}::{}),", pascal(v)).unwrap();
        }
        writeln!(w, "            _ => None,").unwrap();
        writeln!(w, "        }}").unwrap();
        writeln!(w, "    }}").unwrap();
        writeln!(w, "}}").unwrap();
        writeln!(w).unwrap();
    }

    // ---- StyleId / StyleCodec --------------------------------------------
    writeln!(w, "/// Wire codec of a style row.").unwrap();
    writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
    writeln!(w, "pub enum StyleCodec {{ Dimension, F32, U8, U16, U32, I32, Rgba8, ColorValue, Vec2, Color2, Tracks, Placement, Transitions, Enum }}").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "/// One style row; the discriminant is the mask bit.").unwrap();
    writeln!(w, "#[repr(u8)]").unwrap();
    writeln!(
        w,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
    )
    .unwrap();
    writeln!(w, "pub enum StyleId {{").unwrap();
    for row in &schema.styles {
        writeln!(w, "    {} = {},", pascal(&row.field), row.bit).unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl StyleId {{").unwrap();
    writeln!(w, "    /// Number of style rows.").unwrap();
    writeln!(w, "    pub const COUNT: usize = {};", schema.styles.len()).unwrap();
    writeln!(w, "    /// Every row, in bit order.").unwrap();
    writeln!(
        w,
        "    pub const ALL: [StyleId; {}] = [{}];",
        schema.styles.len(),
        schema
            .styles
            .iter()
            .map(|r| format!("StyleId::{}", pascal(&r.field)))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    writeln!(w, "    /// The mask bit.").unwrap();
    writeln!(w, "    pub const fn bit(self) -> u32 {{ self as u32 }}").unwrap();
    writeln!(
        w,
        "    /// The row for a mask bit; out-of-range bits are `None`."
    )
    .unwrap();
    writeln!(w, "    pub fn from_bit(bit: u32) -> Option<Self> {{").unwrap();
    writeln!(w, "        if (bit as usize) < Self::COUNT {{ Some(Self::ALL[bit as usize]) }} else {{ None }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// The field name.").unwrap();
    writeln!(w, "    pub fn name(self) -> &'static str {{").unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.styles {
        writeln!(
            w,
            "            StyleId::{} => \"{}\",",
            pascal(&row.field),
            row.field
        )
        .unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Look a field name up.").unwrap();
    writeln!(w, "    pub fn from_name(name: &str) -> Option<Self> {{").unwrap();
    writeln!(w, "        match name {{").unwrap();
    for row in &schema.styles {
        writeln!(
            w,
            "            \"{}\" => Some(StyleId::{}),",
            row.field,
            pascal(&row.field)
        )
        .unwrap();
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// The wire codec.").unwrap();
    writeln!(w, "    pub fn codec(self) -> StyleCodec {{").unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.styles {
        writeln!(
            w,
            "            StyleId::{} => StyleCodec::{},",
            pascal(&row.field),
            parse_codec(&row.codec).variant()
        )
        .unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// For an enum row, the wire ordinal of `name`; `None` for other rows and unknown names.").unwrap();
    writeln!(
        w,
        "    pub fn enum_from_name(self, name: &str) -> Option<u8> {{"
    )
    .unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.styles {
        if let Codec::Enum(name) = parse_codec(&row.codec) {
            writeln!(
                w,
                "            StyleId::{} => {name}::from_name(name).map(|v| v as u8),",
                pascal(&row.field)
            )
            .unwrap();
        }
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    for (method, doc, pred) in [
        (
            "admits_auto",
            "Whether `auto` is a legal value (dimension rows only).",
            (|r: &StyleRow| r.admits_auto) as fn(&StyleRow) -> bool,
        ),
        ("affects_layout", "Whether a change re-runs layout.", |r| {
            r.layout
        }),
        (
            "affects_text",
            "Whether a change invalidates text measurement.",
            |r| r.text,
        ),
    ] {
        writeln!(w, "    /// {doc}").unwrap();
        writeln!(w, "    pub fn {method}(self) -> bool {{").unwrap();
        let members: Vec<_> = schema
            .styles
            .iter()
            .filter(|r| pred(r))
            .map(|r| format!("StyleId::{}", pascal(&r.field)))
            .collect();
        if members.is_empty() {
            writeln!(w, "        false").unwrap();
        } else {
            writeln!(w, "        matches!(self, {})", members.join(" | ")).unwrap();
        }
        writeln!(w, "    }}").unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();

    // ---- StyleMask -------------------------------------------------------
    writeln!(w, "/// Number of 64-bit words in the style mask.").unwrap();
    writeln!(w, "pub const STYLE_MASK_WORDS: usize = {mask_words};").unwrap();
    writeln!(
        w,
        "/// Which style rows are set. One bit per row, in row order."
    )
    .unwrap();
    writeln!(
        w,
        "#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]"
    )
    .unwrap();
    writeln!(
        w,
        "pub struct StyleMask {{ pub words: [u64; STYLE_MASK_WORDS] }}"
    )
    .unwrap();
    let word_mask = |pred: &dyn Fn(&StyleRow) -> bool| -> String {
        let mut words = vec![0u64; mask_words];
        for r in schema.styles.iter().filter(|r| pred(r)) {
            words[(r.bit / 64) as usize] |= 1u64 << (r.bit % 64);
        }
        words
            .iter()
            .map(|x| format!("{x:#018x}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    writeln!(w, "impl StyleMask {{").unwrap();
    writeln!(w, "    /// No rows set.").unwrap();
    writeln!(
        w,
        "    pub const EMPTY: StyleMask = StyleMask {{ words: [0; STYLE_MASK_WORDS] }};"
    )
    .unwrap();
    writeln!(w, "    /// Every row set.").unwrap();
    writeln!(
        w,
        "    pub const ALL: StyleMask = StyleMask {{ words: [{}] }};",
        word_mask(&|_| true)
    )
    .unwrap();
    writeln!(w, "    /// Rows whose change re-runs layout.").unwrap();
    writeln!(
        w,
        "    pub const LAYOUT: StyleMask = StyleMask {{ words: [{}] }};",
        word_mask(&|r| r.layout)
    )
    .unwrap();
    writeln!(w, "    /// Rows whose change invalidates text measurement.").unwrap();
    writeln!(
        w,
        "    pub const TEXT: StyleMask = StyleMask {{ words: [{}] }};",
        word_mask(&|r| r.text)
    )
    .unwrap();
    writeln!(
        w,
        "    /// Bits above the last row. A wire mask with any of these set is rejected."
    )
    .unwrap();
    writeln!(
        w,
        "    pub const RESERVED: StyleMask = StyleMask {{ words: [{}] }};",
        {
            let mut words = vec![u64::MAX; mask_words];
            for r in &schema.styles {
                words[(r.bit / 64) as usize] &= !(1u64 << (r.bit % 64));
            }
            words
                .iter()
                .map(|x| format!("{x:#018x}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    )
    .unwrap();
    writeln!(w, "    /// A mask with exactly one row set.").unwrap();
    writeln!(w, "    pub const fn of(id: StyleId) -> StyleMask {{").unwrap();
    writeln!(w, "        let mut words = [0u64; STYLE_MASK_WORDS];").unwrap();
    writeln!(
        w,
        "        words[(id as u32 / 64) as usize] |= 1u64 << (id as u32 % 64);"
    )
    .unwrap();
    writeln!(w, "        StyleMask {{ words }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Whether a row is set.").unwrap();
    writeln!(w, "    pub const fn has(self, id: StyleId) -> bool {{").unwrap();
    writeln!(
        w,
        "        self.words[(id as u32 / 64) as usize] & (1u64 << (id as u32 % 64)) != 0"
    )
    .unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Set a row.").unwrap();
    writeln!(w, "    pub fn set(&mut self, id: StyleId) {{ self.words[(id as u32 / 64) as usize] |= 1u64 << (id as u32 % 64); }}").unwrap();
    writeln!(w, "    /// Clear a row.").unwrap();
    writeln!(w, "    pub fn clear(&mut self, id: StyleId) {{ self.words[(id as u32 / 64) as usize] &= !(1u64 << (id as u32 % 64)); }}").unwrap();
    writeln!(w, "    /// Whether no row is set.").unwrap();
    writeln!(
        w,
        "    pub fn is_empty(self) -> bool {{ self.words.iter().all(|w| *w == 0) }}"
    )
    .unwrap();
    writeln!(w, "    /// Whether any row is set in both masks.").unwrap();
    writeln!(w, "    pub fn intersects(self, other: StyleMask) -> bool {{ self.words.iter().zip(other.words.iter()).any(|(a, b)| a & b != 0) }}").unwrap();
    writeln!(w, "    /// Union.").unwrap();
    writeln!(w, "    pub fn union(self, other: StyleMask) -> StyleMask {{ let mut out = self; for (a, b) in out.words.iter_mut().zip(other.words.iter()) {{ *a |= b; }} out }}").unwrap();
    writeln!(w, "    /// Difference (`self` minus `other`).").unwrap();
    writeln!(w, "    pub fn minus(self, other: StyleMask) -> StyleMask {{ let mut out = self; for (a, b) in out.words.iter_mut().zip(other.words.iter()) {{ *a &= !b; }} out }}").unwrap();
    writeln!(w, "    /// Number of rows set.").unwrap();
    writeln!(
        w,
        "    pub fn count(self) -> u32 {{ self.words.iter().map(|w| w.count_ones()).sum() }}"
    )
    .unwrap();
    writeln!(w, "    /// Iterate the set rows in bit order.").unwrap();
    writeln!(w, "    pub fn iter(self) -> impl Iterator<Item = StyleId> {{ StyleId::ALL.into_iter().filter(move |id| self.has(*id)) }}").unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();

    // ---- StyleProps ------------------------------------------------------
    writeln!(
        w,
        "/// Every style row plus the mask of rows explicitly set. Doubles as a masked patch:"
    )
    .unwrap();
    writeln!(
        w,
        "/// only rows whose mask bit is set are meaningful when applied."
    )
    .unwrap();
    writeln!(w, "#[derive(Debug, Clone, PartialEq)]").unwrap();
    writeln!(w, "pub struct StyleProps {{").unwrap();
    for row in &schema.styles {
        writeln!(
            w,
            "    pub {}: {},",
            row.field,
            parse_codec(&row.codec).rust_type()
        )
        .unwrap();
    }
    writeln!(w, "    /// Rows explicitly set.").unwrap();
    writeln!(w, "    pub mask: StyleMask,").unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl Default for StyleProps {{").unwrap();
    writeln!(w, "    fn default() -> Self {{").unwrap();
    writeln!(w, "        StyleProps {{").unwrap();
    for row in &schema.styles {
        let codec = parse_codec(&row.codec);
        writeln!(
            w,
            "            {}: {},",
            row.field,
            codec.default_expr(&row.default, &row.field)
        )
        .unwrap();
    }
    writeln!(w, "            mask: StyleMask::EMPTY,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl StyleProps {{").unwrap();
    writeln!(
        w,
        "    /// Copy every row set in `patch.mask` from `patch`, and mark it set here."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn apply_patch(&mut self, patch: &StyleProps) {{"
    )
    .unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let clone = if parse_codec(&row.codec).is_copy() {
            ""
        } else {
            ".clone()"
        };
        writeln!(
            w,
            "        if patch.mask.has(StyleId::{id}) {{ self.{f} = patch.{f}{clone}; }}",
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "        self.mask = self.mask.union(patch.mask);").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Reset every row in `mask` to its default and mark it unset."
    )
    .unwrap();
    writeln!(w, "    pub fn clear(&mut self, mask: StyleMask) {{").unwrap();
    writeln!(w, "        let d = StyleProps::default();").unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        writeln!(
            w,
            "        if mask.has(StyleId::{id}) {{ self.{f} = d.{f}; }}",
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "        self.mask = self.mask.minus(mask);").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Rows in `patch` whose explicit state or value differs from this style."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn changed_mask(&self, patch: &StyleProps) -> StyleMask {{"
    )
    .unwrap();
    writeln!(w, "        let mut changed = StyleMask::EMPTY;").unwrap();
    writeln!(w, "        for id in patch.mask.iter() {{").unwrap();
    writeln!(
        w,
        "            if !self.mask.has(id) || self.get(id) != patch.get(id) {{ changed.set(id); }}"
    )
    .unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "        changed").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Rows in `mask` that are currently explicit and would actually be cleared."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn cleared_mask(&self, mask: StyleMask) -> StyleMask {{"
    )
    .unwrap();
    writeln!(w, "        let mut changed = StyleMask::EMPTY;").unwrap();
    writeln!(
        w,
        "        for id in mask.iter() {{ if self.mask.has(id) {{ changed.set(id); }} }}"
    )
    .unwrap();
    writeln!(w, "        changed").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Decode a masked patch: the mask words, then each set row's value in bit order."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn decode_patch(r: &mut Reader<'_>) -> Result<StyleProps, DecodeError> {{"
    )
    .unwrap();
    writeln!(w, "        let mask = r.style_mask()?;").unwrap();
    writeln!(w, "        let mut out = StyleProps::default();").unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let codec = parse_codec(&row.codec);
        writeln!(
            w,
            "        if mask.has(StyleId::{id}) {{ out.{f} = {}; }}",
            codec.decode_expr(&id, row.admits_auto),
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "        out.mask = mask;").unwrap();
    writeln!(
        w,
        "        out.validate_domain().map_err(DecodeError::from)?;"
    )
    .unwrap();
    writeln!(w, "        Ok(out)").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Encode the rows in `mask` as a masked patch.").unwrap();
    writeln!(
        w,
        "    pub fn encode_masked(&self, mask: StyleMask, w: &mut Writer) {{"
    )
    .unwrap();
    writeln!(w, "        w.style_mask(mask);").unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let codec = parse_codec(&row.codec);
        writeln!(
            w,
            "        if mask.has(StyleId::{id}) {{ {} }}",
            codec.encode_stmt(&format!("self.{}", row.field))
        )
        .unwrap();
    }
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// The first masked row carrying an infinite or NaN number, if any."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn check_finite(&self) -> Result<(), StyleId> {{"
    )
    .unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let test = match parse_codec(&row.codec) {
            Codec::F32 | Codec::Dimension | Codec::Tracks | Codec::Transitions => {
                format!("self.{}.is_finite()", row.field)
            }
            Codec::Vec2 => format!(
                "self.{f}.x.is_finite() && self.{f}.y.is_finite()",
                f = row.field
            ),
            _ => continue,
        };
        writeln!(
            w,
            "        if self.mask.has(StyleId::{id}) && !({test}) {{ return Err(StyleId::{id}); }}"
        )
        .unwrap();
    }
    writeln!(w, "        Ok(())").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Validate every masked row against the schema's closed value domain."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn validate_domain(&self) -> Result<(), StyleDomainError> {{"
    )
    .unwrap();
    writeln!(
        w,
        "        self.check_finite().map_err(StyleDomainError::NonFinite)?;"
    )
    .unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let field = &row.field;
        match parse_codec(&row.codec) {
            Codec::Dimension if !row.admits_auto => {
                writeln!(w, "        if self.mask.has(StyleId::{id}) && matches!(self.{field}, Dimension::Auto) {{ return Err(StyleDomainError::AutoNotAdmitted(StyleId::{id})); }}").unwrap();
            }
            Codec::Tracks => {
                writeln!(w, "        if self.mask.has(StyleId::{id}) && self.{field}.0.len() > MAX_GRID_TRACKS {{ return Err(StyleDomainError::TooManyTracks {{ style: StyleId::{id}, count: self.{field}.0.len() }}); }}").unwrap();
            }
            Codec::Placement => {
                writeln!(w, "        if self.mask.has(StyleId::{id}) && !self.{field}.is_valid() {{ return Err(StyleDomainError::InvalidGridSpan(StyleId::{id})); }}").unwrap();
            }
            Codec::Transitions => {
                writeln!(w, "        if self.mask.has(StyleId::{id}) {{ self.{field}.validate().map_err(StyleDomainError::InvalidTransition)?; }}").unwrap();
            }
            _ => {}
        }
    }
    writeln!(w, "        Ok(())").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Set one row from an untyped value and mark it. The one place a producer"
    )
    .unwrap();
    writeln!(
        w,
        "    /// that names rows by id turns a value into a row; refused typed, nothing changed."
    )
    .unwrap();
    writeln!(w, "    pub fn set_dynamic(&mut self, id: StyleId, value: &StyleValue) -> Result<(), StyleValueError> {{").unwrap();
    writeln!(w, "        match id {{").unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let f = &row.field;
        let stmt = match parse_codec(&row.codec) {
            Codec::Dimension => format!("self.{f} = value.dimension(id, {})?;", row.admits_auto),
            Codec::F32 => format!("self.{f} = value.f32(id)?;"),
            Codec::U8 => format!("self.{f} = value.int(id, 0.0, u8::MAX as f64)? as u8;"),
            Codec::U16 => format!("self.{f} = value.int(id, 0.0, u16::MAX as f64)? as u16;"),
            Codec::U32 => format!("self.{f} = value.int(id, 0.0, u32::MAX as f64)? as u32;"),
            Codec::I32 => format!("self.{f} = value.int(id, i32::MIN as f64, i32::MAX as f64)? as i32;"),
            Codec::Rgba8 => format!("self.{f} = value.color(id)?;"),
            Codec::ColorValue => format!("self.{f} = value.color_value(id)?;"),
            Codec::Vec2 => format!("self.{f} = value.vec2(id)?;"),
            Codec::Enum(name) => format!(
                "self.{f} = {name}::from_name(value.text(id)?).ok_or(StyleValueError::UnknownEnumValue {{ style: id }})?;"
            ),
            Codec::Transitions => format!(
                "self.{f} = Transitions::parse(value.text(id)?).map_err(|_| StyleValueError::BadTransition {{ style: id }})?;"
            ),
            Codec::Color2 | Codec::Tracks | Codec::Placement => {
                "return Err(StyleValueError::Unsupported { style: id });".to_string()
            }
        };
        writeln!(w, "            StyleId::{id} => {{ {stmt} }}").unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "        self.mask.set(id);").unwrap();
    writeln!(w, "        Ok(())").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Read one row by id, untyped. Every codec has a form; nothing is skipped."
    )
    .unwrap();
    writeln!(w, "    pub fn get(&self, id: StyleId) -> RowValue<'_> {{").unwrap();
    writeln!(w, "        match id {{").unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let f = &row.field;
        let expr = match parse_codec(&row.codec) {
            Codec::Dimension => format!("RowValue::Dimension(self.{f})"),
            Codec::F32 | Codec::U8 | Codec::U16 | Codec::U32 | Codec::I32 => {
                format!("RowValue::Number(self.{f} as f64)")
            }
            Codec::Rgba8 => format!("RowValue::Color(self.{f})"),
            Codec::ColorValue => format!("RowValue::ColorValue(self.{f})"),
            Codec::Vec2 => format!("RowValue::Vec2(self.{f})"),
            Codec::Color2 => format!("RowValue::Color2(self.{f})"),
            Codec::Enum(_) => format!("RowValue::Enum(self.{f}.name())"),
            Codec::Tracks => format!("RowValue::Tracks(&self.{f})"),
            Codec::Placement => format!("RowValue::Placement(self.{f})"),
            Codec::Transitions => format!("RowValue::Transitions(&self.{f})"),
        };
        writeln!(w, "            StyleId::{id} => {expr},").unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// Encode this patch (the rows in `self.mask`).").unwrap();
    writeln!(
        w,
        "    pub fn encode_patch(&self, w: &mut Writer) {{ self.encode_masked(self.mask, w); }}"
    )
    .unwrap();
    writeln!(w, "}}").unwrap();
    writeln!(w).unwrap();

    // ---- OpCode ----------------------------------------------------------
    writeln!(
        w,
        "/// The closed wire-visible op list for EXWF frame revision 1."
    )
    .unwrap();
    writeln!(w, "#[repr(u16)]").unwrap();
    writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
    writeln!(w, "pub enum OpCode {{").unwrap();
    for row in &schema.opcodes {
        writeln!(w, "    {} = {},", row.name, row.id).unwrap();
    }
    writeln!(w, "}}").unwrap();
    writeln!(w, "impl OpCode {{").unwrap();
    writeln!(w, "    /// Every opcode, in id order.").unwrap();
    writeln!(
        w,
        "    pub const ALL: [OpCode; {}] = [{}];",
        schema.opcodes.len(),
        schema
            .opcodes
            .iter()
            .map(|r| format!("OpCode::{}", r.name))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    writeln!(
        w,
        "    /// Decode a wire opcode; unknown values are `None`."
    )
    .unwrap();
    writeln!(w, "    pub fn from_wire(value: u16) -> Option<Self> {{").unwrap();
    writeln!(w, "        match value {{").unwrap();
    for row in &schema.opcodes {
        writeln!(w, "            {} => Some(OpCode::{}),", row.id, row.name).unwrap();
    }
    writeln!(w, "            _ => None,").unwrap();
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "    /// The opcode name.").unwrap();
    writeln!(w, "    pub fn name(self) -> &'static str {{").unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.opcodes {
        writeln!(w, "            OpCode::{} => \"{}\",", row.name, row.name).unwrap();
    }
    writeln!(w, "        }}").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(w, "}}").unwrap();

    let _ = snake; // reserved for future table columns
    o
}

/// Drop every object key that starts with `_`, recursively.
fn strip_prose(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| (k, strip_prose(v)))
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(strip_prose).collect())
        }
        other => other,
    }
}

fn main() {
    println!("cargo:rerun-if-changed={SCHEMA_PATH}");
    println!("cargo:rerun-if-changed=build.rs");
    let raw = fs::read_to_string(SCHEMA_PATH).expect("read tables/schema.json");
    let schema: Schema = serde_json::from_str(&raw).expect("parse tables/schema.json");
    validate(&schema);

    // Canonical form: serde_json's default map is ordered, so re-serializing the
    // parsed value sorts keys and normalizes whitespace.
    let value: serde_json::Value = serde_json::from_str(&raw).expect("parse tables/schema.json");
    // Prose keys (`_about`, `_styles`, ...) are documentation, not schema: editing a
    // comment must not rotate the digest and refuse every producer.
    let value = strip_prose(value);
    let canonical = serde_json::to_string(&value).expect("serialize canonical schema");
    let digest = digest(&canonical);

    let code = generate(&schema, digest);
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("schema.rs");
    fs::write(&out, code).expect("write generated schema.rs");
}
