//! `contract tamber gen ts | rust <app> [-o <dir>]` — every language's
//! generated names over the one table (LLP 1121 D12): from the design's IR,
//! never committed. They go in the app's `.exact/tamber/` (gitignored, as
//! all of `.exact/` is) unless `-o` names a directory.
//!
//! - `ts`: `tamber.ts`, a `<Kind>Token` union per kind (`ColorToken`) and
//!   each token's resolved CSS per full app tuple, and `tamber.css`, those
//!   values as custom properties per tuple, for code outside a plan. The
//!   data module's union is in `contract types`' declarations
//!   ([`crate::typescript`]).
//! - `rust`: `tamber.rs`, `pub const SURFACE_CARD: Token<Color>` per colour
//!   token, which `session.tamber().get(token, place)` resolves.
//!
//! They name the tokens a binding may (D7): the design's `exports` when it
//! lists them, else every token, which is what an app that can name tokens
//! outside its views keeps in its plan.
//!
//! Names map as D12 says: dots and hyphens become camelCase in TypeScript
//! (Swift and Kotlin with commit 5) and `SCREAMING_SNAKE` in Rust. A design
//! two of whose names collide after mapping is refused, naming both.

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str =
    "usage: contract tamber gen ts|rust <app dir | app.contract | file.tamber> [-o <dir>]";

/// `name` as a camelCase identifier: `surface.card` `surfaceCard`,
/// `-exact-label` `exactLabel`, `CanvasText` `canvasText`.
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    for word in name.split(['.', '-']).filter(|w| !w.is_empty()) {
        let mut chars = word.chars();
        let first = chars.next().expect("a word has a character");
        match out.is_empty() {
            true => out.extend(first.to_lowercase()),
            false => out.extend(first.to_uppercase()),
        }
        out.extend(chars);
    }
    out
}

/// `name` as a `SCREAMING_SNAKE` identifier: `surface.card` `SURFACE_CARD`,
/// `CanvasText` `CANVAS_TEXT`.
pub fn screaming(name: &str) -> String {
    let mut out = String::new();
    for word in name.split(['.', '-']).filter(|w| !w.is_empty()) {
        if !out.is_empty() {
            out.push('_');
        }
        let mut lower = false;
        for c in word.chars() {
            if c.is_uppercase() && lower {
                out.push('_');
            }
            lower = c.is_lowercase() || c.is_ascii_digit();
            out.extend(c.to_uppercase());
        }
    }
    out
}

/// The design's names that collide after `map`, as `a` and `b` pairs.
fn collisions(names: &[&str], map: fn(&str) -> String) -> Vec<(String, String)> {
    let mut seen: Vec<(String, &str)> = Vec::new();
    let mut out = Vec::new();
    for name in names {
        let mapped = map(name);
        match seen.iter().find(|(m, _)| *m == mapped) {
            Some((_, first)) => out.push((first.to_string(), name.to_string())),
            None => seen.push((mapped, name)),
        }
    }
    out
}

/// One token as the generators read it: its name, kind and CSS per tuple.
struct Token {
    name: String,
    kind: String,
    css: Vec<String>,
}

/// The tokens a binding may name (D7): the design's `exports` when it lists
/// them, else every token; what an app with bindings keeps in its plan.
fn tokens(folding: &exact_tamber::Folding) -> Vec<Token> {
    let exported = |t: &exact_tamber::Checked| {
        folding.design.exports.as_ref().is_none_or(|exports| {
            exports
                .iter()
                .any(|x| exact_tamber::exports_names(&x.value, &t.name, t.role.as_deref()))
        })
    };
    let vocab = contract_lower::tamber_vocab();
    folding
        .tokens
        .iter()
        .zip(&folding.design.tokens)
        .filter(|(t, _)| exported(t))
        .map(|(t, declared)| Token {
            name: t.name.clone(),
            kind: declared.kind.clone().unwrap_or_else(|| "color".into()),
            css: t.values.iter().map(|v| standalone(v, &vocab)).collect(),
        })
        .collect()
}

/// A value as CSS outside Exact: an Exact role (`-exact-label`) or a
/// platform colour is no colour a browser reads, so it is its fallback pair
/// (D7); a CSS system colour (`CanvasText`, `AccentColor`) stays.
fn standalone(value: &exact_tamber::Folded, vocab: &exact_tamber::Vocab) -> String {
    use exact_tamber::{Folded, Host};
    let fallback = || {
        exact_tamber::fallback(value, vocab).map_or_else(
            || value.css(),
            |[l, d]| match l == d {
                true => l,
                false => format!("light-dark({l}, {d})"),
            },
        )
    };
    match value {
        Folded::Host(Host::Role(row)) if row.starts_with("-exact-") => fallback(),
        Folded::Host(Host::Platform(..)) => fallback(),
        _ => value.css(),
    }
}

/// Each full tuple's key, `axis=value` by commas, in tuple order.
fn tuple_keys(folding: &exact_tamber::Folding) -> Vec<String> {
    let axes = &folding.design.axes;
    (0..folding.tuples)
        .map(|tuple| {
            let mut rest = tuple;
            let mut pins: Vec<String> = Vec::new();
            for axis in axes.iter().rev() {
                pins.push(format!(
                    "{}={}",
                    axis.name.value,
                    axis.values[rest % axis.values.len()]
                ));
                rest /= axis.values.len();
            }
            pins.reverse();
            pins.join(",")
        })
        .collect()
}

fn kind_type(kind: &str) -> String {
    let mut name = camel(kind);
    if let Some(first) = name.get(..1) {
        name = first.to_uppercase() + &name[1..];
    }
    format!("{name}Token")
}

/// `tamber.ts`: the unions, the axes, and each token's CSS per tuple.
pub fn typescript(folding: &exact_tamber::Folding, source: &str) -> String {
    let tokens = tokens(folding);
    let quoted = |s: &str| serde_json::to_string(s).expect("a string is JSON");
    let mut out = format!(
        "// @generated by `contract tamber gen ts` from {source}; do not edit, never committed.\n"
    );
    let mut kinds: Vec<&str> = Vec::new();
    for t in &tokens {
        if !kinds.contains(&t.kind.as_str()) {
            kinds.push(&t.kind);
        }
    }
    for kind in &kinds {
        let names: Vec<String> = tokens
            .iter()
            .filter(|t| t.kind == *kind)
            .map(|t| quoted(&t.name))
            .collect();
        let _ = writeln!(
            out,
            "export type {} = {};",
            kind_type(kind),
            names.join(" | ")
        );
    }
    let _ = writeln!(
        out,
        "/** Each full app tuple, `axis=value` by commas: the index of a token's `css`. */\nexport const tuples = [{}] as const;",
        tuple_keys(folding).iter().map(|k| quoted(k)).collect::<Vec<_>>().join(", ")
    );
    // Each token a property of `tokens`, by its camelCase name: a property
    // may be any word, a declaration of its own could clash.
    out.push_str("export const tokens = {\n");
    for t in &tokens {
        let css: Vec<String> = t.css.iter().map(|c| quoted(c)).collect();
        let _ = writeln!(
            out,
            "  {}: {{ name: {}, css: [{}] }},",
            camel(&t.name),
            quoted(&t.name),
            css.join(", ")
        );
    }
    out.push_str("} as const;\n");
    out
}

/// `tamber.css`: each token as `--<camelName>` per tuple, the first tuple's on
/// `:root` too; a page picks a tuple with `data-tamber="axis=value,…"`.
pub fn css(folding: &exact_tamber::Folding) -> String {
    let tokens = tokens(folding);
    let mut out = String::from(
        "/* @generated by `contract tamber gen ts`; do not edit, never committed. */\n",
    );
    for (i, key) in tuple_keys(folding).iter().enumerate() {
        let selector = format!("[data-tamber=\"{key}\"]");
        let _ = write!(
            out,
            "{}{{",
            if i == 0 {
                format!(":root,{selector}")
            } else {
                selector
            }
        );
        // By its camelCase name, which no two tokens share (`admitted`).
        for t in &tokens {
            let _ = write!(out, "--{}:{};", camel(&t.name), t.css[i]);
        }
        out.push_str("}\n");
    }
    out
}

/// `tamber.rs`: a `Token<Color>` per colour token (other kinds with stage 6).
pub fn rust(folding: &exact_tamber::Folding, source: &str) -> String {
    let mut out = format!(
        "// @generated by `contract tamber gen rust` from {source}; do not edit, never committed.\n\
         // A token is its name: `session.tamber().get(TOKEN, place)` resolves it (LLP 1121 D12).\n\
         use exact_runner::tamber::{{Color, Token}};\n"
    );
    for t in tokens(folding).iter().filter(|t| t.kind == "color") {
        let name = serde_json::to_string(&t.name).expect("a string is JSON");
        let _ = writeln!(
            out,
            "/// `{}`.\npub const {}: Token<Color> = Token::new({name});",
            t.name,
            screaming(&t.name)
        );
    }
    out
}

/// Refuse a design whose names collide after any mapping: each pair named.
pub fn admitted(folding: &exact_tamber::Folding) -> Result<(), String> {
    let tokens = tokens(folding);
    let names: Vec<&str> = tokens.iter().map(|t| t.name.as_str()).collect();
    let mut refused = Vec::new();
    // What no constant can be named: one with no letter, or that starts
    // with a digit.
    for name in &names {
        let mapped = camel(name);
        // `__proto__` in an object literal sets its prototype, no property.
        if !mapped.contains(|c: char| c.is_ascii_alphabetic())
            || mapped.starts_with(|c: char| c.is_ascii_digit())
            || mapped == "__proto__"
        {
            refused.push(format!("`{name}` is `{mapped}` in camelCase, which no binding can name: rename it (LLP 1121 D12)"));
        }
    }
    for (scheme, map) in [
        ("camelCase", camel as fn(&str) -> String),
        ("SCREAMING_SNAKE", screaming),
    ] {
        for (a, b) in collisions(&names, map) {
            refused.push(format!(
                "`{a}` and `{b}` are both `{}` in {scheme}: rename one (LLP 1121 D12)",
                map(&a)
            ));
        }
    }
    match refused.is_empty() {
        true => Ok(()),
        false => Err(refused.join("\n")),
    }
}

pub fn run(args: &[String]) -> ExitCode {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (mut target, mut input, mut out) = (None, None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" if i + 1 < args.len() => {
                out = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            t @ ("ts" | "rust") if target.is_none() => target = Some(t.to_string()),
            path if input.is_none() && !path.starts_with('-') => input = Some(path.to_string()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
        i += 1;
    }
    let (Some(target), Some(input)) = (target, input) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(path) = crate::tamber::design(Path::new(&input)) else {
        eprintln!("contract tamber gen: {input} has no design (an app.tamber, or the file app.json's \"tamber\" names)");
        return ExitCode::from(2);
    };
    let folding = match std::fs::read_to_string(&path)
        .map_err(|e| vec![e.to_string()])
        .and_then(|src| {
            exact_tamber::check(&src, &contract_lower::tamber_vocab()).map_err(|all| {
                all.iter()
                    .map(|d| format!("{}:{d}", path.display()))
                    .collect()
            })
        }) {
        Ok(folding) => folding,
        Err(all) => {
            for e in all {
                eprintln!("{e}");
            }
            return ExitCode::from(1);
        }
    };
    if let Err(why) = admitted(&folding) {
        eprintln!("{}: {why}", path.display());
        return ExitCode::from(1);
    }
    // The app's own `.exact/`: the directory named, or the one its
    // `app.contract` (or, given a design file alone, that file) is in.
    let app = Path::new(&input);
    let root = match app.is_dir() {
        true => app.to_path_buf(),
        false => app.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    let dir = out.unwrap_or_else(|| root.join(".exact").join("tamber"));
    let source = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let files: Vec<(&str, String)> = match target.as_str() {
        "ts" => vec![
            ("tamber.ts", typescript(&folding, &source)),
            ("tamber.css", css(&folding)),
        ],
        _ => vec![("tamber.rs", rust(&folding, &source))],
    };
    for (name, text) in files {
        let file = dir.join(name);
        // Unchanged, not written: a watcher sees nothing move.
        if std::fs::read_to_string(&file).is_ok_and(|old| old == text) {
            continue;
        }
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, text)) {
            eprintln!("contract tamber gen: {}: {e}", file.display());
            return ExitCode::from(2);
        }
        println!("{}", file.display());
    }
    ExitCode::SUCCESS
}
