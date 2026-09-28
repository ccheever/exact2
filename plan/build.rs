//! Generates the plan format from `tables/format.json`: row structs, the
//! `Plan` container, the canonical encoder, the validating decoder, the opcode
//! and stdlib enums, the enum vocabularies, and `FORMAT_DIGEST`.
//!
//! The generator fails closed: a table or codec it cannot validate is a build
//! error. Output goes to `OUT_DIR/format.rs` and is never committed.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

const SCHEMA_PATH: &str = "tables/format.json";

#[derive(Deserialize)]
struct Schema {
    #[serde(rename = "formatVersion")]
    format_version: u32,
    /// Extra header fields after the four fixed integers, in declaration
    /// order (LLP 1023 D5: `app_id`; LLP 1038 D2: `router`).
    #[serde(default)]
    header: Vec<Field>,
    tables: Vec<Table>,
    enums: BTreeMap<String, Vec<String>>,
    opcodes: Vec<Opcode>,
    stdlib: Vec<StdlibEntry>,
}

#[derive(Deserialize)]
struct Table {
    name: String,
    fields: Vec<Field>,
}

#[derive(Deserialize)]
struct Field {
    name: String,
    codec: String,
}

#[derive(Deserialize)]
struct Opcode {
    name: String,
    operands: Vec<String>,
}

#[derive(Deserialize)]
struct StdlibEntry {
    name: String,
    params: Vec<String>,
    returns: String,
}

#[derive(Clone)]
enum Codec {
    U8,
    U16,
    U32,
    I32,
    F64,
    Bool,
    Str,
    Enum(String),
    Idx(String),
    Opt(String),
    Range(String),
    Code,
    Bytes,
}

fn parse_codec(s: &str) -> Codec {
    match s {
        "u8" => Codec::U8,
        "u16" => Codec::U16,
        "u32" => Codec::U32,
        "i32" => Codec::I32,
        "f64" => Codec::F64,
        "bool" => Codec::Bool,
        "str" => Codec::Str,
        "code" => Codec::Code,
        "bytes" => Codec::Bytes,
        other => {
            if let Some(n) = other.strip_prefix("enum:") {
                Codec::Enum(n.into())
            } else if let Some(t) = other.strip_prefix("idx:") {
                Codec::Idx(t.into())
            } else if let Some(t) = other.strip_prefix("opt:") {
                Codec::Opt(t.into())
            } else if let Some(t) = other.strip_prefix("range:") {
                Codec::Range(t.into())
            } else {
                panic!("format: unknown codec `{other}`")
            }
        }
    }
}

fn pascal(s: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            up = true;
        } else if up {
            out.extend(ch.to_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn rust_type(c: &Codec) -> String {
    match c {
        Codec::U8 => "u8".into(),
        Codec::U16 => "u16".into(),
        Codec::U32 => "u32".into(),
        Codec::I32 => "i32".into(),
        Codec::F64 => "f64".into(),
        Codec::Bool => "bool".into(),
        Codec::Str => "StrId".into(),
        Codec::Enum(n) => n.clone(),
        Codec::Idx(t) => format!("{}Id", pascal(t)),
        Codec::Opt(t) => format!("Option<{}Id>", pascal(t)),
        Codec::Range(t) => format!("{}Range", pascal(t)),
        Codec::Code => "Code".into(),
        Codec::Bytes => "Bytes".into(),
    }
}

fn validate(schema: &Schema) {
    assert_eq!(
        schema.format_version, 5,
        "format: unsupported formatVersion"
    );
    let tables: BTreeSet<&str> = schema.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tables.len(), schema.tables.len(), "format: duplicate table");
    for f in &schema.header {
        // @ref LLP 1038 D2 — optional table references in the header.
        if f.codec != "string" {
            match parse_codec(&f.codec) {
                Codec::Opt(t) => assert!(tables.contains(t.as_str()), "format: header table `{t}`"),
                _ => panic!("format: unsupported header codec `{}`", f.codec),
            }
        }
    }
    for t in &schema.tables {
        let mut names = BTreeSet::new();
        for f in &t.fields {
            assert!(
                names.insert(&f.name),
                "format: duplicate field `{}` in `{}`",
                f.name,
                t.name
            );
            match parse_codec(&f.codec) {
                Codec::Idx(r) | Codec::Opt(r) | Codec::Range(r) => {
                    assert!(
                        tables.contains(r.as_str()),
                        "format: `{}` refers to unknown table `{r}`",
                        t.name
                    )
                }
                Codec::Enum(e) => {
                    assert!(schema.enums.contains_key(&e), "format: unknown enum `{e}`")
                }
                _ => {}
            }
        }
    }
    for (name, values) in &schema.enums {
        assert!(
            !values.is_empty() && values.len() <= 255,
            "format: enum `{name}` size"
        );
        let set: BTreeSet<_> = values.iter().collect();
        assert_eq!(set.len(), values.len(), "format: enum `{name}` duplicates");
    }
    let mut ops = BTreeSet::new();
    for op in &schema.opcodes {
        assert!(
            ops.insert(&op.name),
            "format: duplicate opcode `{}`",
            op.name
        );
        for o in &op.operands {
            match parse_codec(o) {
                Codec::Idx(r) => assert!(
                    tables.contains(r.as_str()),
                    "format: opcode `{}` operand table `{r}`",
                    op.name
                ),
                Codec::Enum(e) => assert!(
                    e == "Stdlib" || schema.enums.contains_key(&e),
                    "format: opcode `{}` enum `{e}`",
                    op.name
                ),
                Codec::U8 | Codec::U16 | Codec::U32 | Codec::F64 | Codec::Str => {}
                _ => panic!(
                    "format: opcode `{}` has an operand codec with no VM meaning",
                    op.name
                ),
            }
        }
    }
    assert!(schema.opcodes.len() <= 255, "format: opcodes exceed u8");
    // @ref LLP 1038 D3 — reserved shapes and typed lists in the roster.
    let mut fns = BTreeSet::new();
    for f in &schema.stdlib {
        assert!(fns.insert(&f.name), "format: duplicate stdlib `{}`", f.name);
        let returns = std::iter::once((&f.returns, false));
        for (t, param) in f.params.iter().map(|p| (p, true)).chain(returns) {
            // A parameter spelled as string literals, `"a" | "b"` (LLP
            // 1054.000.003 D9): a string the compiler requires written as one.
            let literals = param
                && t.split(" | ").all(|l| {
                    l.len() > 2
                        && l.starts_with('"')
                        && l.ends_with('"')
                        && !l[1..l.len() - 1].contains('"')
                });
            assert!(
                literals
                    || matches!(
                        t.as_str(),
                        "number"
                            | "string"
                            | "bool"
                            | "any"
                            | "Router"
                            | "Entry"
                            | "list<Router>"
                            | "list<Entry>"
                            | "list<string>"
                    ),
                "format: stdlib `{}` type `{t}`",
                f.name
            );
        }
    }
    assert!(schema.stdlib.len() <= 255, "format: stdlib exceeds u8");
}

fn digest(canonical: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(b"EXACT.PLAN.FORMAT.SHA256.V1\0");
    h.update(canonical.as_bytes());
    let out = h.finalize();
    u64::from_le_bytes(out[..8].try_into().unwrap())
}

fn main() {
    println!("cargo:rerun-if-changed={SCHEMA_PATH}");
    println!("cargo:rerun-if-changed=build.rs");
    let text = std::fs::read_to_string(SCHEMA_PATH).expect("read tables/format.json");
    let value: serde_json::Value = serde_json::from_str(&text).expect("format.json parses");
    let schema: Schema = serde_json::from_value(value.clone()).expect("format.json shape");
    validate(&schema);
    let canonical = serde_json::to_string(&value).unwrap();
    let digest = digest(&canonical);

    let mut w = String::new();
    let _ = writeln!(
        w,
        "// Generated by build.rs from {SCHEMA_PATH}. Do not edit."
    );
    let _ = writeln!(w, "use crate::bytes::{{Reader, Writer}};");
    let _ = writeln!(
        w,
        "use crate::{{Bytes, Code, CodeError, PlanError, StrId}};"
    );
    let _ = writeln!(w);
    let _ = writeln!(w, "/// Domain-separated SHA-256 (first 8 bytes, little-endian) of the canonical format table.");
    let _ = writeln!(w, "pub const FORMAT_DIGEST: u64 = {digest:#018x};");
    let _ = writeln!(w, "/// The format version the digest was computed under.");
    let _ = writeln!(
        w,
        "pub const FORMAT_VERSION: u32 = {};",
        schema.format_version
    );
    let _ = writeln!(w);

    // ---- enums ------------------------------------------------------------
    let mut enums: Vec<(&String, &Vec<String>)> = schema.enums.iter().collect();
    enums.sort();
    let stdlib_names: Vec<String> = schema.stdlib.iter().map(|f| f.name.clone()).collect();
    let mut all_enums: Vec<(String, Vec<String>)> = enums
        .iter()
        .map(|(n, v)| ((*n).clone(), (*v).clone()))
        .collect();
    all_enums.push(("Stdlib".into(), stdlib_names));
    for (name, values) in &all_enums {
        let _ = writeln!(w, "/// `{name}` vocabulary; the wire ordinal is the index.");
        let _ = writeln!(w, "#[repr(u8)]");
        let _ = writeln!(
            w,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
        );
        let _ = writeln!(w, "pub enum {name} {{");
        for (i, v) in values.iter().enumerate() {
            let _ = writeln!(w, "    /// `{v}`");
            let _ = writeln!(w, "    {} = {i},", pascal(v));
        }
        let _ = writeln!(w, "}}");
        let _ = writeln!(w, "impl {name} {{");
        let _ = writeln!(w, "    /// Every value, in wire order.");
        let _ = writeln!(
            w,
            "    pub const ALL: [{name}; {}] = [{}];",
            values.len(),
            values
                .iter()
                .map(|v| format!("{name}::{}", pascal(v)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(w, "    /// From the wire ordinal.");
        let _ = writeln!(
            w,
            "    pub fn from_wire(v: u8) -> Option<Self> {{ Self::ALL.get(v as usize).copied() }}"
        );
        let _ = writeln!(w, "    /// The declared name.");
        let _ = writeln!(w, "    pub fn name(self) -> &'static str {{ match self {{");
        for v in values {
            let _ = writeln!(w, "        {name}::{} => \"{v}\",", pascal(v));
        }
        let _ = writeln!(w, "    }} }}");
        let _ = writeln!(w, "    /// From the declared name.");
        let _ = writeln!(w, "    pub fn from_name(s: &str) -> Option<Self> {{ Self::ALL.into_iter().find(|v| v.name() == s) }}");
        if name == "Stdlib" {
            let _ = writeln!(w, "    /// Declared argument count.");
            let _ = writeln!(
                w,
                "    pub fn arity(self) -> usize {{ self.params().len() }}"
            );
            let _ = writeln!(
                w,
                "    /// Declared parameter types, as the table spells them."
            );
            let _ = writeln!(
                w,
                "    pub fn params(self) -> &'static [&'static str] {{ match self {{"
            );
            for f in &schema.stdlib {
                let ps: Vec<String> = f.params.iter().map(|p| format!("{p:?}")).collect();
                let _ = writeln!(
                    w,
                    "        Stdlib::{} => &[{}],",
                    pascal(&f.name),
                    ps.join(", ")
                );
            }
            let _ = writeln!(w, "    }} }}");
            let _ = writeln!(w, "    /// Declared return type, as the table spells it.");
            let _ = writeln!(
                w,
                "    pub fn returns(self) -> &'static str {{ match self {{"
            );
            for f in &schema.stdlib {
                let _ = writeln!(
                    w,
                    "        Stdlib::{} => \"{}\",",
                    pascal(&f.name),
                    f.returns
                );
            }
            let _ = writeln!(w, "    }} }}");
        }
        let _ = writeln!(w, "}}");
        let _ = writeln!(w);
    }

    // ---- opcodes ----------------------------------------------------------
    let _ = writeln!(
        w,
        "/// The expression VM's opcodes; the wire byte is the index."
    );
    let _ = writeln!(w, "#[repr(u8)]");
    let _ = writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]");
    let _ = writeln!(w, "pub enum Opcode {{");
    for (i, op) in schema.opcodes.iter().enumerate() {
        let _ = writeln!(w, "    /// `{}` {:?}", op.name, op.operands);
        let _ = writeln!(w, "    {} = {i},", op.name);
    }
    let _ = writeln!(w, "}}");
    let _ = writeln!(w, "/// One operand's layout.");
    let _ = writeln!(w, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
    let _ = writeln!(
        w,
        "pub enum Operand {{ U8, U16, U32, F64, Str, Enum(&'static str), Idx(&'static str) }}"
    );
    let _ = writeln!(w, "impl Opcode {{");
    let _ = writeln!(w, "    /// Every opcode, in wire order.");
    let _ = writeln!(
        w,
        "    pub const ALL: [Opcode; {}] = [{}];",
        schema.opcodes.len(),
        schema
            .opcodes
            .iter()
            .map(|o| format!("Opcode::{}", o.name))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(w, "    /// From the wire byte.");
    let _ = writeln!(
        w,
        "    pub fn from_wire(v: u8) -> Option<Self> {{ Self::ALL.get(v as usize).copied() }}"
    );
    let _ = writeln!(w, "    /// The operand layout.");
    let _ = writeln!(
        w,
        "    pub fn operands(self) -> &'static [Operand] {{ match self {{"
    );
    for op in &schema.opcodes {
        let ops: Vec<String> = op
            .operands
            .iter()
            .map(|o| match parse_codec(o) {
                Codec::U8 => "Operand::U8".into(),
                Codec::U16 => "Operand::U16".into(),
                Codec::U32 => "Operand::U32".into(),
                Codec::F64 => "Operand::F64".into(),
                Codec::Str => "Operand::Str".into(),
                Codec::Enum(e) => format!("Operand::Enum(\"{e}\")"),
                Codec::Idx(t) => format!("Operand::Idx(\"{t}\")"),
                _ => unreachable!(),
            })
            .collect();
        let _ = writeln!(w, "        Opcode::{} => &[{}],", op.name, ops.join(", "));
    }
    let _ = writeln!(w, "    }} }}");
    let _ = writeln!(w, "}}");
    let _ = writeln!(w);

    // ---- ids, ranges, rows ------------------------------------------------
    for t in &schema.tables {
        let p = pascal(&t.name);
        let _ = writeln!(w, "/// Validated row index into `{}`.", t.name);
        let _ = writeln!(
            w,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
        );
        let _ = writeln!(w, "pub struct {p}Id(pub u32);");
        let _ = writeln!(w, "/// Validated contiguous run of `{}` rows.", t.name);
        let _ = writeln!(
            w,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]"
        );
        let _ = writeln!(w, "pub struct {p}Range {{ pub start: u32, pub len: u32 }}");
        let _ = writeln!(w, "impl {p}Range {{");
        let _ = writeln!(w, "    /// The row ids in the run.");
        let _ = writeln!(w, "    pub fn iter(self) -> impl Iterator<Item = {p}Id> {{ (self.start..self.start + self.len).map({p}Id) }}");
        let _ = writeln!(w, "    /// Whether the run is empty.");
        let _ = writeln!(w, "    pub fn is_empty(self) -> bool {{ self.len == 0 }}");
        let _ = writeln!(w, "}}");
        let _ = writeln!(w, "/// One `{}` row.", t.name);
        let _ = writeln!(w, "#[derive(Debug, Clone, PartialEq)]");
        let _ = writeln!(w, "pub struct {p}Row {{");
        for f in &t.fields {
            let _ = writeln!(w, "    /// `{}`", f.name);
            let _ = writeln!(
                w,
                "    pub {}: {},",
                f.name,
                rust_type(&parse_codec(&f.codec))
            );
        }
        let _ = writeln!(w, "}}");
        let _ = writeln!(w);
    }

    // ---- Plan -------------------------------------------------------------
    let _ = writeln!(w, "/// A plan: pools plus tables. Constructed by a compiler or decoded from bytes; a decoded plan is validated whole before any row is readable.");
    let _ = writeln!(w, "#[derive(Debug, Clone, PartialEq, Default)]");
    let _ = writeln!(w, "pub struct Plan {{");
    let _ = writeln!(
        w,
        "    /// The kernel schema digest the plan's ordinals were emitted against."
    );
    let _ = writeln!(w, "    pub kernel_schema_digest: u64,");
    let _ = writeln!(
        w,
        "    /// The compiler identity (crate version plus configuration digest)."
    );
    let _ = writeln!(w, "    pub compiler_identity: u64,");
    for f in &schema.header {
        let _ = writeln!(
            w,
            "    /// Header field `{}` (declared in format.json's `header`).",
            f.name
        );
        let ty = if f.codec == "string" {
            "String".into()
        } else {
            rust_type(&parse_codec(&f.codec))
        };
        let _ = writeln!(w, "    pub {}: {ty},", f.name);
    }
    let _ = writeln!(w, "    /// Interned strings.");
    let _ = writeln!(w, "    pub strings: Vec<String>,");
    let _ = writeln!(w, "    /// Bytecode pool.");
    let _ = writeln!(w, "    pub code: Vec<u8>,");
    let _ = writeln!(
        w,
        "    /// Data pool (compiled values): borrowed from a static plan's bytes, else owned."
    );
    let _ = writeln!(w, "    pub data: std::borrow::Cow<'static, [u8]>,");
    for t in &schema.tables {
        let _ = writeln!(w, "    /// `{}` rows.", t.name);
        let _ = writeln!(w, "    pub {}: Vec<{}Row>,", t.name, pascal(&t.name));
    }
    let _ = writeln!(w, "}}");
    let _ = writeln!(w);
    let _ = writeln!(w, "/// Magic at byte 0 of an encoded plan.");
    let _ = writeln!(w, "pub const MAGIC: &[u8; 4] = b\"EXPL\";");
    let _ = writeln!(w);
    let _ = writeln!(w, "impl Plan {{");
    for t in &schema.tables {
        let p = pascal(&t.name);
        let _ = writeln!(w, "    /// One `{}` row by validated id.", t.name);
        let singular = t.name.trim_end_matches('s');
        let accessor = if matches!(singular, "type" | "match" | "ref" | "fn" | "mod") {
            format!("{singular}_")
        } else {
            singular.to_string()
        };
        let _ = writeln!(
            w,
            "    pub fn {accessor}(&self, id: {p}Id) -> &{p}Row {{ &self.{}[id.0 as usize] }}",
            t.name
        );
    }
    let _ = writeln!(w, "    /// One interned string.");
    let _ = writeln!(
        w,
        "    pub fn str(&self, id: StrId) -> &str {{ &self.strings[id.0 as usize] }}"
    );
    let _ = writeln!(w, "    /// The bytes of one code range.");
    let _ = writeln!(w, "    pub fn code(&self, c: Code) -> &[u8] {{ &self.code[c.offset as usize..(c.offset + c.len) as usize] }}");
    let _ = writeln!(w, "    /// The bytes of one data range.");
    let _ = writeln!(w, "    pub fn bytes(&self, b: Bytes) -> &[u8] {{ &self.data[b.offset as usize..(b.offset + b.len) as usize] }}");
    // Every code range a row names: what a scan of the plan's calls walks,
    // one validated body at a time (LLP 1054.000.003 D8). Plain loops and a
    // `dyn` callback: this is in every web core, and an iterator chain or a
    // generic callback is compiled once per table.
    let _ = writeln!(w, "    /// Call `f` with every code range a table row names, table by table: each one a validated body, so a walk of each is a walk of every instruction a run can reach.");
    let _ = write!(w, "    pub fn each_code(&self, f: &mut dyn FnMut(Code)) {{");
    for t in &schema.tables {
        for fl in t
            .fields
            .iter()
            .filter(|fl| matches!(parse_codec(&fl.codec), Codec::Code))
        {
            let _ = write!(w, " for r in &self.{} {{ f(r.{}); }}", t.name, fl.name);
        }
    }
    let _ = writeln!(w, " }}");

    // encode
    let _ = writeln!(w, "    /// Canonical encoding: header, pools, then every table in declaration order with fixed-width fields. Deterministic for equal plans.");
    let _ = writeln!(w, "    pub fn encode(&self) -> Vec<u8> {{");
    let _ = writeln!(w, "        let mut w = Writer::default();");
    let _ = writeln!(w, "        w.bytes(MAGIC); w.u32(FORMAT_VERSION); w.u64(FORMAT_DIGEST); w.u64(self.kernel_schema_digest); w.u64(self.compiler_identity);");
    for f in &schema.header {
        if f.codec == "string" {
            let _ = writeln!(w, "        w.string(&self.{});", f.name);
        } else {
            let _ = writeln!(
                w,
                "        w.u32(self.{}.map_or(u32::MAX, |v| v.0));",
                f.name
            );
        }
    }
    let _ = writeln!(
        w,
        "        w.u32(self.strings.len() as u32); for s in &self.strings {{ w.string(s); }}"
    );
    let _ = writeln!(
        w,
        "        w.u32(self.code.len() as u32); w.bytes(&self.code);"
    );
    let _ = writeln!(
        w,
        "        w.u32(self.data.len() as u32); w.bytes(&self.data);"
    );
    for t in &schema.tables {
        let _ = writeln!(w, "        w.u32(self.{}.len() as u32);", t.name);
        let _ = writeln!(w, "        for r in &self.{} {{", t.name);
        for f in &t.fields {
            let stmt = match parse_codec(&f.codec) {
                Codec::U8 => format!("w.u8(r.{});", f.name),
                Codec::U16 => format!("w.u16(r.{});", f.name),
                Codec::U32 => format!("w.u32(r.{});", f.name),
                Codec::I32 => format!("w.i32(r.{});", f.name),
                Codec::F64 => format!("w.f64(r.{});", f.name),
                Codec::Bool => format!("w.u8(r.{} as u8);", f.name),
                Codec::Str => format!("w.u32(r.{}.0);", f.name),
                Codec::Enum(_) => format!("w.u8(r.{} as u8);", f.name),
                Codec::Idx(_) => format!("w.u32(r.{}.0);", f.name),
                Codec::Opt(_) => format!("w.u32(r.{}.map_or(u32::MAX, |v| v.0));", f.name),
                Codec::Range(_) => format!("w.u32(r.{f}.start); w.u32(r.{f}.len);", f = f.name),
                Codec::Code | Codec::Bytes => {
                    format!("w.u32(r.{f}.offset); w.u32(r.{f}.len);", f = f.name)
                }
            };
            let _ = writeln!(w, "            {stmt}");
        }
        let _ = writeln!(w, "        }}");
    }
    let _ = writeln!(w, "        w.into_vec()");
    let _ = writeln!(w, "    }}");

    // decode
    let _ = writeln!(w, "    /// Decode and validate whole. Every index, range, code, and data reference is checked before the plan is returned; a failure names the row.");
    let _ = writeln!(
        w,
        "    pub fn decode(bytes: &[u8]) -> Result<Plan, PlanError> {{ Self::decode_from(bytes, None) }}"
    );
    let _ = writeln!(w, "    /// [`Plan::decode`] of bytes that live as long as the program (a plan linked into it): the data pool, a baked app's largest part, stays in those bytes instead of a copy.");
    let _ = writeln!(
        w,
        "    pub fn decode_static(bytes: &'static [u8]) -> Result<Plan, PlanError> {{ Self::decode_from(bytes, Some(bytes)) }}"
    );
    let _ = writeln!(
        w,
        "    fn decode_from(bytes: &[u8], lent: Option<&'static [u8]>) -> Result<Plan, PlanError> {{"
    );
    let _ = writeln!(w, "        let mut r = Reader::new(bytes);");
    let _ = writeln!(
        w,
        "        if r.bytes(4)? != MAGIC {{ return Err(PlanError::BadMagic); }}"
    );
    let _ = writeln!(w, "        let version = r.u32()?; if version != FORMAT_VERSION {{ return Err(PlanError::UnsupportedVersion(version)); }}");
    let _ = writeln!(w, "        let digest = r.u64()?; if digest != FORMAT_DIGEST {{ return Err(PlanError::FormatDigestMismatch {{ expected: FORMAT_DIGEST, actual: digest }}); }}");
    let _ = writeln!(
        w,
        "        let kernel_schema_digest = r.u64()?; let compiler_identity = r.u64()?;"
    );
    for f in &schema.header {
        let expr = if f.codec == "string" {
            "r.string()?".into()
        } else {
            let Codec::Opt(t) = parse_codec(&f.codec) else {
                unreachable!()
            };
            format!(
                "{{ let v = r.u32()?; if v == u32::MAX {{ None }} else {{ Some({}Id(v)) }} }}",
                pascal(&t)
            )
        };
        let _ = writeln!(w, "        let {} = {expr};", f.name);
    }
    let _ = writeln!(w, "        let n = r.count()?; let mut strings = Vec::with_capacity(n.min(crate::bytes::RESERVE)); for _ in 0..n {{ strings.push(r.string()?); }}");
    let _ = writeln!(
        w,
        "        let n = r.count()?; let code = r.bytes(n)?.to_vec();"
    );
    let _ = writeln!(
        w,
        "        let n = r.count()?; let at = bytes.len() - r.remaining(); let pool = r.bytes(n)?; let data: std::borrow::Cow<'static, [u8]> = match lent {{ Some(lent) => std::borrow::Cow::Borrowed(&lent[at..at + n]), None => std::borrow::Cow::Owned(pool.to_vec()) }};"
    );
    // The rows read through a sticky reader: no branch per field, and the
    // first failing read's error, as an early return would give it.
    let _ = writeln!(w, "        let mut s = crate::bytes::Sticky::new(r);");
    for t in &schema.tables {
        let p = pascal(&t.name);
        let _ = writeln!(
            w,
            "        let n = s.count(); let mut {} = Vec::with_capacity(n.min(crate::bytes::RESERVE)); for _ in 0..n {{ if s.failed() {{ break; }}",
            t.name
        );
        let _ = writeln!(w, "            {} .push({p}Row {{", t.name);
        for f in &t.fields {
            let expr = match parse_codec(&f.codec) {
                Codec::U8 => "s.u8()".to_string(),
                Codec::U16 => "s.u16()".to_string(),
                Codec::U32 => "s.u32()".to_string(),
                Codec::I32 => "s.i32()".to_string(),
                Codec::F64 => "s.f64()".to_string(),
                Codec::Bool => "s.u8() != 0".to_string(),
                Codec::Str => "StrId(s.u32())".to_string(),
                Codec::Enum(e) => format!("{{ let v = s.u8(); match {e}::from_wire(v) {{ Some(e) => e, None => s.fail(PlanError::UnknownEnum {{ table: \"{}\", field: \"{}\", value: v }}, {e}::ALL[0]) }} }}", t.name, f.name),
                Codec::Idx(tt) => format!("{}Id(s.u32())", pascal(&tt)),
                Codec::Opt(tt) => format!("{{ let v = s.u32(); if v == u32::MAX {{ None }} else {{ Some({}Id(v)) }} }}", pascal(&tt)),
                Codec::Range(tt) => format!("{}Range {{ start: s.u32(), len: s.u32() }}", pascal(&tt)),
                Codec::Code => "Code { offset: s.u32(), len: s.u32() }".to_string(),
                Codec::Bytes => "Bytes { offset: s.u32(), len: s.u32() }".to_string(),
            };
            let _ = writeln!(w, "                {}: {expr},", f.name);
        }
        let _ = writeln!(w, "            }});");
        let _ = writeln!(w, "        }}");
    }
    let _ = writeln!(w, "        let r = s.finish()?;");
    let _ = writeln!(
        w,
        "        if !r.is_empty() {{ return Err(PlanError::TrailingBytes(r.remaining())); }}"
    );
    let _ = write!(
        w,
        "        let plan = Plan {{ kernel_schema_digest, compiler_identity, strings, code, data"
    );
    for f in &schema.header {
        let _ = write!(w, ", {}", f.name);
    }
    for t in &schema.tables {
        let _ = write!(w, ", {}", t.name);
    }
    let _ = writeln!(w, " }};");
    let _ = writeln!(w, "        plan.validate()?;");
    let _ = writeln!(w, "        Ok(plan)");
    let _ = writeln!(w, "    }}");

    // validate
    let _ = writeln!(w, "    /// Check every cross-reference. A compiler-built plan must pass this before it is encoded; a decoded plan has passed it.");
    let _ = writeln!(w, "    pub fn validate(&self) -> Result<(), PlanError> {{");
    let _ = writeln!(w, "        let strings = self.strings.len() as u64; let code_len = self.code.len() as u64; let data_len = self.data.len() as u64;");
    let _ = writeln!(w, "        let _ = (strings, code_len, data_len);");
    for f in &schema.header {
        if f.codec != "string" {
            let Codec::Opt(t) = parse_codec(&f.codec) else {
                unreachable!()
            };
            let _ = writeln!(w, "        if let Some(v) = self.{f} {{ if v.0 as usize >= self.{t}.len() {{ return Err(PlanError::BadReference {{ table: \"header\", row: 0, field: \"{f}\" }}); }} }}", f = f.name);
        }
    }
    for t in &schema.tables {
        let _ = writeln!(
            w,
            "        for (i, r) in self.{}.iter().enumerate() {{",
            t.name
        );
        let _ = writeln!(w, "            let at = |field: &'static str| PlanError::BadReference {{ table: \"{}\", row: i as u32, field }};", t.name);
        for f in &t.fields {
            let check = match parse_codec(&f.codec) {
                Codec::Str => format!("if r.{f}.0 as u64 >= strings {{ return Err(at(\"{f}\")); }}", f = f.name),
                Codec::Idx(tt) => format!("if r.{f}.0 as usize >= self.{tt}.len() {{ return Err(at(\"{f}\")); }}", f = f.name),
                Codec::Opt(tt) => format!("if let Some(v) = r.{f} {{ if v.0 as usize >= self.{tt}.len() {{ return Err(at(\"{f}\")); }} }}", f = f.name),
                Codec::Range(tt) => format!("if r.{f}.start as u64 + r.{f}.len as u64 > self.{tt}.len() as u64 {{ return Err(at(\"{f}\")); }}", f = f.name),
                Codec::Code => format!("if r.{f}.offset as u64 + r.{f}.len as u64 > code_len {{ return Err(at(\"{f}\")); }} self.check_code(r.{f}).map_err(|e| e.at(\"{}\", i as u32, \"{f}\"))?;", t.name, f = f.name),
                Codec::Bytes => format!("if r.{f}.offset as u64 + r.{f}.len as u64 > data_len {{ return Err(at(\"{f}\")); }}", f = f.name),
                Codec::F64 => format!("if !r.{f}.is_finite() {{ return Err(at(\"{f}\")); }}", f = f.name),
                _ => String::new(),
            };
            if !check.is_empty() {
                let _ = writeln!(w, "            {check}");
            }
        }
        let _ = writeln!(w, "            let _ = &at;");
        let _ = writeln!(w, "        }}");
    }
    let _ = writeln!(w, "        self.validate_semantics()");
    let _ = writeln!(w, "    }}");

    // check_code: framing + index operands
    let _ = writeln!(w, "    /// Walk one code range: every byte is an opcode with well-framed operands, every index operand resolves, and the range ends with `Return`.");
    let _ = writeln!(
        w,
        "    pub fn check_code(&self, c: Code) -> Result<(), CodeError> {{"
    );
    let _ = writeln!(
        w,
        "        let bytes = &self.code[c.offset as usize..(c.offset + c.len) as usize];"
    );
    let _ = writeln!(
        w,
        "        if bytes.is_empty() {{ return Err(CodeError::Empty); }}"
    );
    let _ = writeln!(
        w,
        "        let mut r = Reader::new(bytes); let mut last = None;"
    );
    let _ = writeln!(w, "        let mut boundaries: Vec<usize> = Vec::new(); let mut jumps: Vec<(usize, u32)> = Vec::new(); let mut bodies: Vec<(usize, usize)> = Vec::new();");
    let _ = writeln!(w, "        while !r.is_empty() {{");
    let _ = writeln!(w, "            let pc = r.position(); boundaries.push(pc); let b = r.u8().map_err(|_| CodeError::Truncated {{ pc }})?;");
    let _ = writeln!(w, "            let op = Opcode::from_wire(b).ok_or(CodeError::UnknownOpcode {{ pc, byte: b }})?;");
    let _ = writeln!(
        w,
        "            for operand in op.operands() {{ match operand {{"
    );
    let _ = writeln!(
        w,
        "                Operand::U8 => {{ r.u8().map_err(|_| CodeError::Truncated {{ pc }})?; }}"
    );
    let _ = writeln!(w, "                Operand::U16 => {{ r.u16().map_err(|_| CodeError::Truncated {{ pc }})?; }}");
    let _ = writeln!(w, "                Operand::U32 => {{ let v = r.u32().map_err(|_| CodeError::Truncated {{ pc }})?; if matches!(op, Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfNone | Opcode::Map | Opcode::Filter) {{ jumps.push((pc, v)); }} if matches!(op, Opcode::Map | Opcode::Filter) {{ bodies.push((pc, v as usize)); }} }}");
    let _ = writeln!(w, "                Operand::F64 => {{ let v = r.f64().map_err(|_| CodeError::Truncated {{ pc }})?; if !v.is_finite() {{ return Err(CodeError::NonFinite {{ pc }}); }} }}");
    let _ = writeln!(w, "                Operand::Str => {{ let v = r.u32().map_err(|_| CodeError::Truncated {{ pc }})?; if v as usize >= self.strings.len() {{ return Err(CodeError::BadIndex {{ pc, table: \"strings\", index: v }}); }} }}");
    let _ = writeln!(w, "                Operand::Enum(name) => {{ let v = r.u8().map_err(|_| CodeError::Truncated {{ pc }})?; let ok = match *name {{");
    for (name, _) in &all_enums {
        let _ = writeln!(
            w,
            "                    \"{name}\" => {name}::from_wire(v).is_some(),"
        );
    }
    let _ = writeln!(w, "                    _ => false }}; if !ok {{ return Err(CodeError::BadEnum {{ pc, name, value: v }}); }} }}");
    let _ = writeln!(w, "                Operand::Idx(table) => {{ let v = r.u32().map_err(|_| CodeError::Truncated {{ pc }})?; let len = match *table {{");
    for t in &schema.tables {
        let _ = writeln!(
            w,
            "                    \"{}\" => self.{}.len(),",
            t.name, t.name
        );
    }
    let _ = writeln!(w, "                    _ => 0 }}; if v as usize >= len {{ return Err(CodeError::BadIndex {{ pc, table, index: v }}); }} }}");
    let _ = writeln!(w, "            }} }}");
    let _ = writeln!(w, "            last = Some(op);");
    let _ = writeln!(w, "        }}");
    let _ = writeln!(
        w,
        "        if last != Some(Opcode::Return) {{ return Err(CodeError::NoReturn); }}"
    );
    let _ = writeln!(w, "        // Control flow is forward-only and instruction-aligned, so a body always terminates; a callback body repeats once per item of a list (LLP 1017.003 D5).");
    let _ = writeln!(w, "        boundaries.push(bytes.len());");
    let _ = writeln!(w, "        for &(pc, target) in &jumps {{ let t = target as usize; if t <= pc || boundaries.binary_search(&t).is_err() {{ return Err(CodeError::BadJump {{ pc, target }}); }} }}");
    // @ref LLP 1017.003 D5: a `Map`/`Filter` callback body runs from its
    // opcode to its end once per item. No jump leaves a body, none lands
    // inside one from outside (its end is fine: a branch around it), and
    // bodies nest, so each run ends at its end.
    let _ = writeln!(w, "        for &(b, end) in &bodies {{ for &(pc, target) in &jumps {{ let t = target as usize; let inside = b < pc && pc < end; if (inside && t > end) || (!inside && b < t && t < end) {{ return Err(CodeError::BadJump {{ pc, target }}); }} }} }}");
    let _ = writeln!(w, "        Ok(())");
    let _ = writeln!(w, "    }}");
    let _ = writeln!(w, "}}");

    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("format.rs");
    std::fs::write(out, w).unwrap();
}
