//! Generates the kernel's typed vocabulary from `tables/schema.json`.
//!
//! Generates node/prop/style vocabularies, symbols, opcodes and the schema
//! digest into `OUT_DIR`; generated files are never committed.
//!
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
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
    symbols: Vec<[String; 3]>,
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
    /// The row follows CSS inheritance (LLP 1035.000 D1).
    #[serde(default)]
    inherited: bool,
    #[serde(default)]
    default: serde_json::Value,
    /// A `u8` row authored as CSS keywords: the named vocabulary's first value
    /// is the empty set (0) and each later value `i` is bit `i - 1`.
    #[serde(default)]
    keywords: Option<String>,
}
#[derive(Deserialize)]
struct OpcodeRow {
    id: u16,
    name: String,
}
/// Convert CSS words, `snake_case`/`kebab-case`/`camelCase` to `PascalCase`.
fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch.is_ascii_whitespace() {
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
/// The body of a generated `name()`: one packed string indexed by the
/// discriminant through u16 end offsets. A `match` compiles to a pointer
/// table and a length table, eight bytes a name; this is two. A discriminant
/// no row takes (a retired id) names nothing.
fn packed_name(ty: &str, rows: &[(u64, &str)]) -> String {
    let (packed, ends) = packed_table(ty, rows);
    format!("packed_name({packed:?}, &{ends:?}, self as usize)")
}

include!("build/names.rs");
include!("build/codec.rs");
fn validate(schema: &Schema) {
    let mut roles = BTreeSet::new();
    for [role, apple, path] in &schema.symbols {
        assert!(
            !role.is_empty() && role.bytes().all(|c| c.is_ascii_lowercase() || c == b'-'),
            "schema: invalid symbol role"
        );
        assert!(roles.insert(role), "schema: duplicate symbol role {role}");
        assert!(
            !apple.is_empty() && !path.is_empty(),
            "schema: incomplete symbol {role}"
        );
        assert!(
            path.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b" .,-".contains(&c)),
            "schema: invalid symbol path {role}"
        );
        // A filled state is its own role, named as Apple names it, beside its outline.
        let base = role.strip_suffix("-fill");
        assert!(
            base.is_some() == apple.ends_with(".fill")
                && base.is_none_or(|base| schema.symbols.iter().any(|row| row[0] == base)),
            "schema: symbol {role}: `-fill` is exactly Apple's `.fill`, beside its role"
        );
    }
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
        if let Some(name) = &row.keywords {
            assert!(
                matches!(codec, Codec::U8) && row.default.as_f64() == Some(0.0),
                "schema: keywords apply to u8 rows defaulting to 0 (`{}`)",
                row.field
            );
            assert!(
                schema.enums.get(name).is_some_and(|e| e.values.len() <= 9),
                "schema: style `{}` keywords `{name}` must name an enum of at most 9 values",
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
    writeln!(w, "/// Portable symbol roles declared by the schema.").unwrap();
    writeln!(
        w,
        "pub const SYMBOL_ROLES: &[&str] = &{:?};",
        schema.symbols.iter().map(|row| &row[0]).collect::<Vec<_>>()
    )
    .unwrap();
    writeln!(
        w,
        "/// Resolve a role to its Apple name, browser path, and whether that path is\n/// filled (even-odd) rather than stroked; never accepts a platform name."
    )
    .unwrap();
    writeln!(
        w,
        "pub fn symbol(role: &str) -> Option<(&'static str, &'static str, bool)> {{ match role {{"
    )
    .unwrap();
    for [role, apple, path] in &schema.symbols {
        let filled = role.ends_with("-fill");
        writeln!(w, "{role:?} => Some(({apple:?}, {path:?}, {filled})),").unwrap();
    }
    writeln!(w, "_ => None, }} }}").unwrap();
    writeln!(
        w,
        "// Generated by build.rs from {SCHEMA_PATH}. Do not edit."
    )
    .unwrap();
    writeln!(w, "use crate::error::{{DecodeError, StyleDomainError}};").unwrap();
    writeln!(
        w,
        "use crate::error::StyleValueError;\nuse crate::style::{{Color, ColorValue, Dimension, LineHeight, GridPlacement, GridTracks, RowValue, StyleValue, Transitions, Animations, Vec2, MAX_GRID_TRACKS}};"
    )
    .unwrap();
    writeln!(w, "use crate::wire::codec::{{Reader, Writer}};").unwrap();
    w.push_str(concat!(
        "/// Name `i` of a packed table: `names[ends[i - 1]..ends[i]]`.\n",
        "fn packed_name(names: &'static str, ends: &'static [u16], i: usize) -> &'static str {\n",
        "    let start = if i == 0 { 0 } else { usize::from(ends[i - 1]) };\n",
        "    &names[start..usize::from(ends[i])]\n",
        "}\n",
        "/// The index of `name` in a packed table of distinct nonempty names.\n",
        "fn packed_find(names: &'static str, ends: &'static [u16], name: &str) -> Option<usize> {\n",
        "    let mut start = 0;\n",
        "    for (i, &end) in ends.iter().enumerate() {\n",
        "        let end = usize::from(end);\n",
        "        if names[start..end] == *name {\n",
        "            return Some(i);\n",
        "        }\n",
        "        start = end;\n",
        "    }\n",
        "    None\n",
        "}\n",
        "/// What `#[derive(Debug)]` writes for a variant spelled `pascal(name)` by\n",
        "/// build.rs: each word's first letter raised; `_`, `-` and spaces dropped.\n",
        "fn debug_pascal(name: &str, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {\n",
        "    use ::core::fmt::Write as _;\n",
        "    for word in name.split(|c: char| c == '_' || c == '-' || c.is_ascii_whitespace()) {\n",
        "        let mut chars = word.chars();\n",
        "        if let Some(first) = chars.next() {\n",
        "            f.write_char(first.to_ascii_uppercase())?;\n",
        "            f.write_str(chars.as_str())?;\n",
        "        }\n",
        "    }\n",
        "    Ok(())\n",
        "}\n",
    ));
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
    // ---- NodeType --------------------------------------------------------
    writeln!(w, "/// Kernel node category. The closed v1 tag set.").unwrap();
    writeln!(w, "#[repr(u8)]").unwrap();
    writeln!(
        w,
        "#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
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
    let rows: Vec<(u64, &str)> = schema
        .node_types
        .iter()
        .map(|r| (u64::from(r.id), r.name.as_str()))
        .collect();
    writeln!(
        w,
        "    pub fn name(self) -> &'static str {{ {} }}",
        packed_name("NodeType", &rows)
    )
    .unwrap();
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
    let rows: Vec<(&str, &str)> = schema
        .node_types
        .iter()
        .map(|r| (r.name.as_str(), r.name.as_str()))
        .collect();
    debug_from_name(w, "NodeType", &rows);
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
    writeln!(
        w,
        "/// Interned property identifier. The discriminant is the wire id."
    )
    .unwrap();
    writeln!(w, "#[repr(u16)]").unwrap();
    writeln!(
        w,
        "#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
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
    let rows: Vec<(u64, &str)> = schema
        .props
        .iter()
        .map(|r| (u64::from(r.id), r.name.as_str()))
        .collect();
    writeln!(
        w,
        "    pub fn name(self) -> &'static str {{ {} }}",
        packed_name("PropId", &rows)
    )
    .unwrap();
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
    let idents: Vec<String> = schema.props.iter().map(|r| pascal(&r.name)).collect();
    let rows: Vec<(&str, &str)> = idents
        .iter()
        .zip(&schema.props)
        .map(|(i, r)| (i.as_str(), r.name.as_str()))
        .collect();
    debug_from_name(w, "PropId", &rows);
    // ---- Enums -----------------------------------------------------------
    for (name, def) in &schema.enums {
        writeln!(
            w,
            "/// Wire vocabulary `{name}`; the discriminant is the wire byte."
        )
        .unwrap();
        writeln!(w, "#[repr(u8)]").unwrap();
        writeln!(w, "#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]").unwrap();
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
        let rows: Vec<(u64, &str)> = def
            .values
            .iter()
            .enumerate()
            .map(|(i, v)| (i as u64, v.as_str()))
            .collect();
        let (names, ends) = packed_table(name, &rows);
        writeln!(w, "    const NAMES: &'static str = {names:?};").unwrap();
        writeln!(w, "    const ENDS: &'static [u16] = &{ends:?};").unwrap();
        writeln!(
            w,
            "    pub fn name(self) -> &'static str {{ packed_name(Self::NAMES, Self::ENDS, self as usize) }}"
        )
        .unwrap();
        writeln!(
            w,
            "    /// Look an authored spelling up; unknown spellings are `None`, never a fallback."
        )
        .unwrap();
        writeln!(w, "    pub fn from_name(name: &str) -> Option<Self> {{").unwrap();
        writeln!(
            w,
            "        packed_find(Self::NAMES, Self::ENDS, name).map(|i| Self::ALL[i])"
        )
        .unwrap();
        writeln!(w, "    }}").unwrap();
        if schema
            .styles
            .iter()
            .any(|r| r.keywords.as_deref() == Some(name.as_str()))
        {
            write!(w, "{KEYWORD_BITS}").unwrap();
        }
        writeln!(w, "}}").unwrap();
        let idents: Vec<String> = def.values.iter().map(|v| pascal(v)).collect();
        let rows: Vec<(&str, &str)> = idents
            .iter()
            .zip(&def.values)
            .map(|(i, v)| (i.as_str(), v.as_str()))
            .collect();
        debug_from_name(w, name, &rows);
    }
    // ---- StyleId / StyleCodec --------------------------------------------
    writeln!(w, "/// Wire codec of a style row.").unwrap();
    writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
    writeln!(w, "pub enum StyleCodec {{ Dimension, LineHeight, F32, U8, U16, U32, I32, Rgba8, ColorValue, KeywordColor, Vec2, Color2, Tracks, Placement, Transitions, Animations, ClipPath, ShapeOutside, AspectRatio, Paint, DashArray, Transform, TransformOrigin, Enum }}").unwrap();
    writeln!(w, "/// One style row; the discriminant is the mask bit.").unwrap();
    writeln!(w, "#[repr(u8)]").unwrap();
    writeln!(
        w,
        "#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
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
    let rows: Vec<(u64, &str)> = schema
        .styles
        .iter()
        .map(|r| (u64::from(r.bit), r.field.as_str()))
        .collect();
    let (names, ends) = packed_table("StyleId", &rows);
    assert_eq!(ends.len(), schema.styles.len(), "StyleId: bits are dense");
    writeln!(w, "    const NAMES: &'static str = {names:?};").unwrap();
    writeln!(w, "    const ENDS: &'static [u16] = &{ends:?};").unwrap();
    writeln!(
        w,
        "    pub fn name(self) -> &'static str {{ packed_name(Self::NAMES, Self::ENDS, self as usize) }}"
    )
    .unwrap();
    writeln!(w, "    /// Look a field name up.").unwrap();
    writeln!(w, "    pub fn from_name(name: &str) -> Option<Self> {{").unwrap();
    writeln!(
        w,
        "        packed_find(Self::NAMES, Self::ENDS, name).map(|i| Self::ALL[i])"
    )
    .unwrap();
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
    writeln!(
        w,
        "    /// Accepted spellings in wire order for enum rows; empty for other codecs."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn enum_names(self) -> &'static [&'static str] {{"
    )
    .unwrap();
    writeln!(w, "        match self {{").unwrap();
    for row in &schema.styles {
        let name = match parse_codec(&row.codec) {
            Codec::Enum(name) => Some(name),
            _ => row.keywords.clone(),
        };
        if let Some(name) = name {
            writeln!(
                w,
                "            StyleId::{} => &{:?},",
                pascal(&row.field),
                schema.enums[&name].values
            )
            .unwrap();
        }
    }
    writeln!(w, "            _ => &[],").unwrap();
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
        (
            "inherited",
            "Whether the row follows CSS inheritance: a node without its own row takes the nearest logical ancestor's computed value.",
            |r| r.inherited,
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
    let idents: Vec<String> = schema.styles.iter().map(|r| pascal(&r.field)).collect();
    let rows: Vec<(&str, &str)> = idents
        .iter()
        .zip(&schema.styles)
        .map(|(i, r)| (i.as_str(), r.field.as_str()))
        .collect();
    debug_from_name(w, "StyleId", &rows);
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
        "    /// Rows that follow CSS inheritance (LLP 1035.000 D1)."
    )
    .unwrap();
    writeln!(
        w,
        "    pub const INHERITED: StyleMask = StyleMask {{ words: [{}] }};",
        word_mask(&|r| r.inherited)
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
    writeln!(w, "    /// Intersection.").unwrap();
    writeln!(w, "    pub fn intersect(self, other: StyleMask) -> StyleMask {{ let mut out = self; for (a, b) in out.words.iter_mut().zip(other.words.iter()) {{ *a &= b; }} out }}").unwrap();
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
    // Compact, transient before/after values for inheritance invalidation.
    // Derive the field set from the same schema as StyleMask::INHERITED.
    writeln!(w, "#[derive(Debug)]\npub(crate) struct InheritedStyle {{").unwrap();
    for row in schema.styles.iter().filter(|r| r.inherited) {
        writeln!(
            w,
            "    {}: {},",
            row.field,
            parse_codec(&row.codec).rust_type()
        )
        .unwrap();
    }
    writeln!(w, "}}\nimpl InheritedStyle {{").unwrap();
    writeln!(
        w,
        "    pub(crate) fn new(from: &StyleProps) -> Self {{ Self {{"
    )
    .unwrap();
    for row in schema.styles.iter().filter(|r| r.inherited) {
        let clone = if parse_codec(&row.codec).is_copy() {
            ""
        } else {
            ".clone()"
        };
        writeln!(w, "        {f}: from.{f}{clone},", f = row.field).unwrap();
    }
    writeln!(w, "    }} }}").unwrap();
    writeln!(
        w,
        "    pub(crate) fn copy_rows(&mut self, from: &StyleProps, mask: StyleMask) {{"
    )
    .unwrap();
    for row in schema.styles.iter().filter(|r| r.inherited) {
        let id = pascal(&row.field);
        let clone = if parse_codec(&row.codec).is_copy() {
            ""
        } else {
            ".clone()"
        };
        writeln!(
            w,
            "        if mask.has(StyleId::{id}) {{ self.{f} = from.{f}{clone}; }}",
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    pub(crate) fn changed_mask(&self, other: &Self) -> StyleMask {{"
    )
    .unwrap();
    writeln!(w, "        let mut changed = StyleMask::EMPTY;").unwrap();
    for row in schema.styles.iter().filter(|r| r.inherited) {
        let id = pascal(&row.field);
        writeln!(
            w,
            "        if self.{f} != other.{f} {{ changed.set(StyleId::{id}); }}",
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "        changed\n    }}\n}}").unwrap();
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
    // One masked copy serves the patch, the reset and the row copy.
    writeln!(w, "        self.copy_rows(patch, patch.mask);").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Reset every row in `mask` to its default and mark it unset."
    )
    .unwrap();
    writeln!(w, "    pub fn clear(&mut self, mask: StyleMask) {{").unwrap();
    writeln!(w, "        let before = self.mask;").unwrap();
    writeln!(w, "        self.copy_rows(&StyleProps::default(), mask);").unwrap();
    writeln!(w, "        self.mask = before.minus(mask);").unwrap();
    writeln!(w, "    }}").unwrap();
    writeln!(
        w,
        "    /// Copy the rows in `mask` from `from` and mark them set here, whatever `from`'s own mask says."
    )
    .unwrap();
    writeln!(
        w,
        "    pub fn copy_rows(&mut self, from: &StyleProps, mask: StyleMask) {{"
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
            "        if mask.has(StyleId::{id}) {{ self.{f} = from.{f}{clone}; }}",
            f = row.field
        )
        .unwrap();
    }
    writeln!(w, "        self.mask = self.mask.union(mask);").unwrap();
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
    // Rows in bit order, which is schema order: the first failing row is
    // the one a row-by-row check would name.
    writeln!(
        w,
        "        match self.mask.iter().find(|&id| !self.get(id).is_finite()) {{ Some(id) => Err(id), None => Ok(()) }}"
    )
    .unwrap();
    writeln!(w, "    }}").unwrap();
    // The row-by-row form it replaced, for the test that holds them equal.
    writeln!(w, "    #[cfg(test)]").unwrap();
    writeln!(
        w,
        "    pub(crate) fn check_finite_rows(&self) -> Result<(), StyleId> {{"
    )
    .unwrap();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let test = match parse_codec(&row.codec) {
            Codec::F32
            | Codec::Dimension
            | Codec::LineHeight
            | Codec::Tracks
            | Codec::Transitions
            | Codec::Animations => {
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
            Codec::LineHeight => {
                writeln!(w, "        if self.mask.has(StyleId::{id}) && !self.{field}.is_valid() {{ return Err(StyleDomainError::InvalidLineHeight); }}").unwrap();
            }
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
            Codec::Animations => {
                writeln!(w, "        if self.mask.has(StyleId::{id}) {{ self.{field}.validate().map_err(StyleDomainError::InvalidAnimation)?; }}").unwrap();
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
    // Rows that convert alike share one conversion, then store by row: the
    // conversion (and its refusal) is written once per codec, not per row.
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for row in &schema.styles {
        let id = pascal(&row.field);
        let conv = match parse_codec(&row.codec) {
            Codec::U8 if row.keywords.is_some() => format!(
                "{}::bits(value.text(id)?).ok_or(StyleValueError::UnknownEnumValue {{ style: id }})?",
                row.keywords.as_deref().unwrap()
            ),
            Codec::Dimension => format!("value.dimension(id, {})?", row.admits_auto),
            Codec::LineHeight => "value.line_height(id)?".to_string(),
            Codec::F32 => "value.f32(id)?".to_string(),
            Codec::U8 => "value.int(id, 0.0, u8::MAX as f64)? as u8".to_string(),
            Codec::U16 => "value.int(id, 0.0, u16::MAX as f64)? as u16".to_string(),
            Codec::U32 => "value.int(id, 0.0, u32::MAX as f64)? as u32".to_string(),
            Codec::I32 => "value.int(id, i32::MIN as f64, i32::MAX as f64)? as i32".to_string(),
            Codec::Rgba8 => "value.color(id)?".to_string(),
            Codec::ColorValue => "value.color_value(id)?".to_string(),
            Codec::KeywordColor(keyword) => format!("value.keyword_color(id, {keyword:?})?"),
            Codec::Vec2 => "value.vec2(id)?".to_string(),
            Codec::Enum(name) => format!(
                "{name}::from_name(value.text(id)?).ok_or(StyleValueError::UnknownEnumValue {{ style: id }})?"
            ),
            Codec::Transitions => "Transitions::parse(value.text(id)?).map_err(|_| StyleValueError::BadTransition { style: id })?".to_string(),
            // Names resolve against the plan's `@keyframes` after this (LLP 1055 D5).
            Codec::Animations => "Animations::parse(value.text(id)?).map_err(|_| StyleValueError::BadAnimation { style: id })?".to_string(),
            Codec::CssValue { path, error, .. } => format!("{path}::parse(&value.css_text(id)?).ok_or(StyleValueError::{error} {{ style: id }})?"),
            Codec::Color2 | Codec::Tracks | Codec::Placement => String::new(),
        };
        match groups.iter_mut().find(|(c, _)| *c == conv) {
            Some((_, rows)) => rows.push((id, row.field.clone())),
            None => groups.push((conv, vec![(id, row.field.clone())])),
        }
    }
    for (conv, rows) in &groups {
        let pattern = rows
            .iter()
            .map(|(id, _)| format!("StyleId::{id}"))
            .collect::<Vec<_>>()
            .join(" | ");
        if conv.is_empty() {
            writeln!(w, "            {pattern} => return Err(StyleValueError::Unsupported {{ style: id }}),").unwrap();
        } else if let [(_, f)] = rows.as_slice() {
            writeln!(w, "            {pattern} => {{ self.{f} = {conv}; }}").unwrap();
        } else {
            writeln!(w, "            {pattern} => {{").unwrap();
            writeln!(w, "                let v = {conv};").unwrap();
            writeln!(w, "                match id {{").unwrap();
            for (id, f) in rows {
                writeln!(w, "                    StyleId::{id} => self.{f} = v,").unwrap();
            }
            writeln!(w, "                    _ => {{}}").unwrap();
            writeln!(w, "                }}").unwrap();
            writeln!(w, "            }}").unwrap();
        }
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
            Codec::LineHeight => format!("RowValue::LineHeight(self.{f})"),
            Codec::Dimension => format!("RowValue::Dimension(self.{f})"),
            Codec::F32 | Codec::U8 | Codec::U16 | Codec::U32 | Codec::I32 => {
                format!("RowValue::Number(self.{f} as f64)")
            }
            Codec::Rgba8 => format!("RowValue::Color(self.{f})"),
            Codec::ColorValue => format!("RowValue::ColorValue(self.{f})"),
            Codec::KeywordColor(keyword) => {
                format!("self.{f}.map(RowValue::ColorValue).unwrap_or(RowValue::Enum({keyword:?}))")
            }
            Codec::Vec2 => format!("RowValue::Vec2(self.{f})"),
            Codec::Color2 => format!("RowValue::Color2(self.{f})"),
            Codec::Enum(_) => format!("RowValue::Enum(self.{f}.name())"),
            Codec::Tracks => format!("RowValue::Tracks(&self.{f})"),
            Codec::Placement => format!("RowValue::Placement(self.{f})"),
            Codec::Transitions => format!("RowValue::Transitions(&self.{f})"),
            Codec::Animations => format!("RowValue::Animations(&self.{f})"),
            Codec::CssValue { variant, .. } => format!("RowValue::{variant}(&self.{f})"),
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
    // ---- OpCode ----------------------------------------------------------
    writeln!(
        w,
        "/// The closed wire-visible op list for EXWF frame revision 1."
    )
    .unwrap();
    writeln!(w, "#[repr(u16)]").unwrap();
    writeln!(w, "#[derive(Clone, Copy, PartialEq, Eq, Hash)]").unwrap();
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
    let rows: Vec<(u64, &str)> = schema
        .opcodes
        .iter()
        .map(|r| (u64::from(r.id), r.name.as_str()))
        .collect();
    writeln!(
        w,
        "    pub fn name(self) -> &'static str {{ {} }}",
        packed_name("OpCode", &rows)
    )
    .unwrap();
    writeln!(w, "}}").unwrap();
    let rows: Vec<(&str, &str)> = schema
        .opcodes
        .iter()
        .map(|r| (r.name.as_str(), r.name.as_str()))
        .collect();
    debug_from_name(w, "OpCode", &rows);
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
    println!("cargo:rerun-if-changed=build/codec.rs");
    println!("cargo:rerun-if-changed=build/names.rs");
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
