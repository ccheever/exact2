//! The vocabulary's candidates (LLP 1086 D3): the string patterns of the
//! `match name` arms in `src/tags.rs`'s four lookups — `tag`, `attr`,
//! `renamed`, `html_tag` — and each style row's default from the kernel's
//! schema, which the kernel's generated code does not carry.
//!
//! Only the patterns left of `=>` are read; arm bodies are skipped by
//! bracket depth, so a value literal (`"flex"`, `"pre-wrap"`) is never a
//! candidate. An arm whose pattern is anything but string literals joined by
//! `|`, or `_`, fails the build: the scan is total, never sampled.

use std::fmt::Write as _;
use std::path::Path;

const LOOKUPS: [(&str, &str); 4] = [
    ("tag", "TAG_CANDIDATES"),
    ("attr", "ATTR_CANDIDATES"),
    ("renamed", "RENAMED_CANDIDATES"),
    ("html_tag", "HTML_CANDIDATES"),
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Str(String),
    Open(char),
    Close(char),
    Arrow,
    Comma,
    Bar,
    Other(String),
}

/// Rust tokens enough to find arm boundaries: strings, brackets, `=>`, `,`,
/// `|`; comments dropped; every other run of characters is `Other`. Each
/// token carries its 1-based line.
fn lex(src: &str) -> Vec<(Tok, usize)> {
    let b = src.as_bytes();
    let (mut i, mut line, mut out) = (0, 1, Vec::new());
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            _ if c.is_whitespace() => i += 1,
            '/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            '/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    line += usize::from(b[i] == b'\n');
                    i += 1;
                }
                i += 2;
            }
            '"' => {
                let start = line;
                let mut s = String::new();
                i += 1;
                while b[i] != b'"' {
                    if b[i] == b'\\' {
                        s.push('\\');
                        i += 1;
                    }
                    line += usize::from(b[i] == b'\n');
                    let ch = src[i..].chars().next().unwrap();
                    s.push(ch);
                    i += ch.len_utf8();
                }
                i += 1;
                out.push((Tok::Str(s), start));
            }
            // A char literal (`'x'`, `'\n'`); otherwise a lifetime.
            '\'' if b.get(i + 2) == Some(&b'\'') => {
                out.push((Tok::Other(src[i..i + 3].into()), line));
                i += 3;
            }
            '\'' if b.get(i + 1) == Some(&b'\\') => {
                let end = i + 2 + src[i + 2..].find('\'').unwrap() + 1;
                out.push((Tok::Other(src[i..end].into()), line));
                i = end;
            }
            '(' | '[' | '{' => {
                out.push((Tok::Open(c), line));
                i += 1;
            }
            ')' | ']' | '}' => {
                out.push((Tok::Close(c), line));
                i += 1;
            }
            '=' if b.get(i + 1) == Some(&b'>') => {
                out.push((Tok::Arrow, line));
                i += 2;
            }
            ',' => {
                out.push((Tok::Comma, line));
                i += 1;
            }
            '|' if b.get(i + 1) != Some(&b'|') => {
                out.push((Tok::Bar, line));
                i += 1;
            }
            _ if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < b.len() && ((b[i] as char).is_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push((Tok::Other(src[start..i].into()), line));
            }
            _ => {
                let ch = src[i..].chars().next().unwrap();
                out.push((Tok::Other(ch.into()), line));
                i += ch.len_utf8();
            }
        }
    }
    out
}

/// The string patterns of the first `match` on the function's parameter.
fn scan(toks: &[(Tok, usize)], function: &str) -> Vec<String> {
    let at = |k: usize| toks.get(k).map(|t| &t.0);
    let def = (0..toks.len())
        .find(|&k| {
            at(k) == Some(&Tok::Other("fn".into()))
                && at(k + 1) == Some(&Tok::Other(function.into()))
                && at(k + 2) == Some(&Tok::Open('('))
        })
        .unwrap_or_else(|| panic!("tags.rs: no `fn {function}(`"));
    let Some(Tok::Other(param)) = at(def + 3) else {
        panic!("tags.rs: `fn {function}` has no named parameter");
    };
    let mut k = (def..toks.len())
        .find(|&k| {
            at(k) == Some(&Tok::Other("match".into()))
                && at(k + 1) == Some(&Tok::Other(param.clone()))
                && at(k + 2) == Some(&Tok::Open('{'))
        })
        .unwrap_or_else(|| panic!("tags.rs: `fn {function}` has no `match {param} {{`"))
        + 3;
    let mut names = Vec::new();
    loop {
        // The pattern: string literals joined by `|`, or `_`.
        let line = toks[k].1;
        if at(k) == Some(&Tok::Close('}')) {
            return names;
        }
        let mut pattern = Vec::new();
        while at(k) != Some(&Tok::Arrow) {
            pattern.push(toks[k].0.clone());
            k += 1;
        }
        k += 1;
        let wildcard = pattern == [Tok::Other("_".into())];
        let literals = pattern.iter().enumerate().all(|(n, t)| match t {
            Tok::Str(s) => n % 2 == 0 && !s.contains('\\'),
            Tok::Bar => n % 2 == 1,
            _ => false,
        }) && pattern.len() % 2 == 1;
        if !wildcard && !literals {
            panic!(
                "contract/lower/src/tags.rs:{line}: an arm of `{function}`'s `match {param}` has a pattern that is not string literals or `_` ({pattern:?}); `contract vocab` lists these arms by scanning their patterns (LLP 1086 D3), so keep them literal"
            );
        }
        for t in pattern {
            if let Tok::Str(s) = t {
                names.push(s);
            }
        }
        // The body: a block, or everything up to the next `,` at depth 0.
        let mut depth = 0usize;
        let block = at(k) == Some(&Tok::Open('{'));
        loop {
            match at(k).unwrap_or_else(|| panic!("tags.rs: `fn {function}`'s match is unclosed")) {
                Tok::Open(_) => depth += 1,
                Tok::Close(_) if depth == 0 => break,
                Tok::Close(_) => {
                    depth -= 1;
                    if block && depth == 0 {
                        k += 1;
                        if at(k) == Some(&Tok::Comma) {
                            k += 1;
                        }
                        break;
                    }
                }
                Tok::Comma if depth == 0 => {
                    k += 1;
                    break;
                }
                _ => {}
            }
            k += 1;
        }
    }
}

/// A schema default as text: a string as is, a number or list as JSON.
fn default_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let tags_path = Path::new(&dir).join("src/tags.rs");
    let schema_path = Path::new(&dir).join("../../kernel/tables/schema.json");
    println!("cargo:rerun-if-changed=src/tags.rs");
    println!("cargo:rerun-if-changed={}", schema_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    let toks = lex(&std::fs::read_to_string(&tags_path).unwrap());
    let mut out = String::from(
        "// Generated by contract/lower/build.rs from src/tags.rs and kernel/tables/schema.json.\n",
    );
    for (function, constant) in LOOKUPS {
        let names = scan(&toks, function);
        writeln!(out, "/// `{function}`'s matched spellings.").unwrap();
        writeln!(out, "pub const {constant}: &[&str] = &{names:?};").unwrap();
    }

    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&schema_path).unwrap()).unwrap();
    let enums = &schema["enums"];
    let mut defaults = Vec::new();
    for row in schema["styles"].as_array().expect("schema.json: `styles`") {
        let field = row["field"]
            .as_str()
            .expect("schema.json: a style's `field`");
        let codec = row["codec"].as_str().unwrap_or("");
        let default = match codec.strip_prefix("enum:") {
            Some(name) if row["default"].is_null() => default_text(&enums[name]["default"]),
            _ => default_text(&row["default"]),
        };
        if let Some(default) = default {
            defaults.push((field.to_owned(), default));
        }
    }
    writeln!(
        out,
        "/// Each style row's default, by its kernel field name."
    )
    .unwrap();
    writeln!(
        out,
        "pub const STYLE_DEFAULTS: &[(&str, &str)] = &{defaults:?};"
    )
    .unwrap();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&out_dir).join("vocab.rs"), out).unwrap();
}
