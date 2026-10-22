//! A package's module declaration, its `exact.json` (LLP 1133.001 D1, LLP
//! 1133.002 D13), read as the file a `use` names: its shapes, and its
//! sources and sends as [`ModuleDecl`]s. Reading a declaration never links
//! (LLP 1133.001 D2 rule 2): an interface reads its endorsed
//! implementations' words on every target, and brings their result and
//! status shapes in by a `use` of each, as a Contract file would.

use contract_syntax::{
    Field, File, ModuleDecl, ModuleKind, PlatformWords, ShapeDecl, Span, TypeExpr, UseDecl, UseName,
};
use serde_json::{Map, Value};
use std::path::Path;

/// A refusal: its id and message; the caller adds the `use`'s span.
type Refusal = (&'static str, String);

/// Read `text`, the declaration at `path`, as unit `source_id`.
pub(crate) fn read(path: &Path, text: &str, source_id: u32) -> Result<File, Refusal> {
    let span = Span {
        line: 1,
        col: 0,
        end_col: 0,
        source_id,
    };
    let refuse = |message: String| {
        (
            "contract-use-declaration",
            format!("{}: {message}", path.display()),
        )
    };
    let json: Value = serde_json::from_str(text).map_err(|e| refuse(format!("not JSON: {e}")))?;
    let kind = json["kind"].as_str().unwrap_or_default();
    let mut file = File {
        shapes: shapes(&json, span).map_err(refuse)?,
        ..File::default()
    };
    match kind {
        "interface" => {
            let package = json["name"]
                .as_str()
                .ok_or_else(|| refuse("an interface has a `name`".into()))?;
            let dir = path.parent().unwrap_or(Path::new("."));
            let implementations = endorsed(dir).map_err(refuse)?;
            for (subpath, implementation) in &implementations {
                let names: Vec<UseName> = object(&implementation["shapes"])
                    .keys()
                    .map(|name| UseName {
                        name: name.clone(),
                        alias: None,
                        span,
                    })
                    .collect();
                if !names.is_empty() {
                    file.uses.push(UseDecl {
                        names,
                        path: format!("./{subpath}/exact.json"),
                        span,
                    });
                }
            }
            for entry in json["names"].as_array().into_iter().flatten() {
                let mut decl = module(entry, package, &json, span).map_err(refuse)?;
                for (subpath, implementation) in &implementations {
                    let platform = implementation["platform"].as_str().unwrap_or(subpath);
                    let words = &implementation["options"][&decl.declared];
                    let words = fields(object(words), span).map_err(refuse)?;
                    if !words.is_empty() {
                        decl.words.push(PlatformWords {
                            platform: platform.to_owned(),
                            package: format!("{package}/{subpath}"),
                            words,
                        });
                    }
                }
                file.modules.push(decl);
            }
        }
        "implementation" => {
            // Its platform-only names (D2) are read when one is first
            // declared and imported; stage 0 has none.
            if json["names"].as_array().is_some_and(|names| !names.is_empty()) {
                return Err(refuse(
                    "an implementation's own sources and sends are not read yet (LLP 1133.001 D2)".into(),
                ));
            }
        }
        other => {
            return Err(refuse(format!(
                "`kind` is `{other}`; a declaration `use` reads is an `interface` or an `implementation`"
            )))
        }
    }
    Ok(file)
}

/// The implementations an interface's own package carries, by subpath, from
/// its `package.json` `exports` (LLP 1133.001 D1: `./apple`, `./android`).
fn endorsed(dir: &Path) -> Result<Vec<(String, Value)>, String> {
    let manifest = dir.join("package.json");
    let text =
        std::fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{}: not JSON: {e}", manifest.display()))?;
    let mut out = Vec::new();
    for (key, target) in object(&json["exports"]) {
        let (Some(subpath), Some(target)) = (key.strip_prefix("./"), target.as_str()) else {
            continue;
        };
        if target != format!("./{subpath}/exact.json") {
            continue;
        }
        let at = dir.join(subpath).join("exact.json");
        let text = std::fs::read_to_string(&at).map_err(|e| format!("{}: {e}", at.display()))?;
        let implementation: Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", at.display()))?;
        if implementation["kind"] == "implementation" {
            out.push((subpath.to_owned(), implementation));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Each shape, its fields in name order: `serde_json` here keeps no key
/// order, and none is meant, since a record crosses the seam by name.
fn shapes(json: &Value, span: Span) -> Result<Vec<ShapeDecl>, String> {
    object(&json["shapes"])
        .iter()
        .map(|(name, fields_json)| {
            Ok(ShapeDecl {
                name: name.clone(),
                fields: fields(object(fields_json), span)?,
                span,
            })
        })
        .collect()
}

fn fields(map: &Map<String, Value>, span: Span) -> Result<Vec<Field>, String> {
    map.iter()
        .map(|(name, ty)| {
            let ty = ty
                .as_str()
                .ok_or_else(|| format!("`{name}`'s type is a string"))?;
            Ok(Field {
                name: name.clone(),
                ty: type_expr(ty, span)?,
                span,
            })
        })
        .collect()
}

/// One entry of `names`: a source, a send, or an event (LLP 1133.004 D6).
fn module(
    entry: &Value,
    package: &str,
    interface: &Value,
    span: Span,
) -> Result<ModuleDecl, String> {
    let (kind, name) = match (
        entry["source"].as_str(),
        entry["send"].as_str(),
        entry["event"].as_str(),
    ) {
        (Some(name), None, None) => (ModuleKind::Source, name),
        (None, Some(name), None) => (ModuleKind::Send, name),
        (None, None, Some(name)) => (ModuleKind::Event, name),
        _ => return Err("each of `names` is a `source`, a `send` or an `event`".into()),
    };
    // A send may answer nothing (`"answers": null`), and is then a statement
    // (LLP 1133.004 D5); a source always answers, and an event never does.
    let answers = match (&entry["answers"], kind) {
        (Value::String(t), ModuleKind::Source | ModuleKind::Send) => Some(type_expr(t, span)?),
        (Value::Null, ModuleKind::Send) if entry.get("answers").is_some() => None,
        (Value::Null, ModuleKind::Event) => None,
        (_, ModuleKind::Event) => return Err(format!("the event `{name}` answers nothing")),
        _ => {
            return Err(format!(
                "`{name}` names what it `answers`, or `null` for a send that answers nothing"
            ))
        }
    };
    // Per name, how the interface's permission admits it (LLP 1133.004 §7):
    // `asks` holds a request while `prompt` and refuses while `denied`;
    // `request` asks while `prompt` and answers the status in every state;
    // `none` is never held or refused. Without it, `request` says.
    let admission = match &entry["permission"] {
        Value::Null => None,
        Value::String(s) if matches!(s.as_str(), "asks" | "request" | "none") => Some(s.as_str()),
        other => {
            return Err(format!(
                "`{name}`'s `permission` is `asks`, `request` or `none`, not `{other}`"
            ))
        }
    };
    let request = match (&entry["request"], admission) {
        (_, Some("asks" | "request")) => "always",
        (_, Some(_)) => "never",
        (Value::Bool(true), None) => "always",
        (Value::String(s), None) if s == "when-prompt" => "when-prompt",
        (Value::Null | Value::Bool(false), None) => "never",
        (other, None) => return Err(format!("`{name}`'s `request` is `{other}`")),
    };
    let (raised, hold) = if kind == ModuleKind::Event {
        let raised = match entry["raised"].as_str() {
            Some(r @ ("activation" | "emit")) => r,
            _ => {
                return Err(format!(
                    "the event `{name}` is `raised` by `activation` or `emit`"
                ))
            }
        };
        if entry["on"] != "root" {
            return Err(format!(
                "the event `{name}` is heard `on` `root`, the one place every app has"
            ));
        }
        let hold = entry["hold"]
            .as_u64()
            .filter(|h| (1..=256).contains(h))
            .ok_or_else(|| format!("the event `{name}` `hold`s from 1 to 256 deliveries"))?;
        (raised.to_owned(), hold as u32)
    } else {
        (String::new(), 0)
    };
    let args = (entry["args"].as_array().into_iter().flatten())
        .map(|a| {
            a.as_str()
                .ok_or_else(|| format!("`{name}`'s `args` are types"))
                .and_then(|t| type_expr(t, span))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut substitute = substitute(&entry["substitute"], interface)?;
    if let Some(admission) = admission {
        let mut json: Value = serde_json::from_str(&substitute).unwrap_or_default();
        json["admission"] = admission.into();
        substitute = json.to_string();
    }
    // Options converted as Web IDL's `[EnforceRange] unsigned long long`
    // (the Badging API's `contents`): admission's to check, the bake's for a
    // literal.
    if let Some(ranged) = entry["enforceRange"].as_array() {
        let mut json: Value = serde_json::from_str(&substitute).unwrap_or_default();
        json["enforceRange"] = Value::Array(ranged.clone());
        substitute = json.to_string();
    }
    Ok(ModuleDecl {
        name: name.to_owned(),
        declared: name.to_owned(),
        package: package.to_owned(),
        extension: package.rsplit('/').next().unwrap_or(package).to_owned(),
        kind,
        options: fields(object(&entry["options"]), span)?,
        words: Vec::new(),
        answers,
        args,
        raised,
        hold,
        handler: None,
        live: entry["live"] == true,
        request: request.to_owned(),
        sync: entry["sync"]["name"].as_str().map(str::to_owned),
        substitute,
        span,
    })
}

/// A name's declared substitute, with its interface's `permission` and
/// `grants` beside it, as the plan carries it: what the agent's runner
/// answers with no implementation bound (LLP 1133.002 §6), and the grant
/// lines the app writes one of (D6).
fn substitute(declared: &Value, interface: &Value) -> Result<String, String> {
    let mut out = match declared {
        Value::Null => Map::new(),
        Value::Object(map) => map.clone(),
        other => return Err(format!("a `substitute` is an object, not `{other}`")),
    };
    for field in ["permission", "grants"] {
        if !interface[field].is_null() {
            out.insert(field.into(), interface[field].clone());
        }
    }
    Ok(Value::Object(out).to_string())
}

/// A declaration's type (LLP 1133.002 D13): `number`, `string`, `bool`,
/// `option<T>`, `T[]`, `enum<a|b>`, a shape's name, or `strings-key`, a
/// key of the app's strings table, which is a string here.
fn type_expr(ty: &str, span: Span) -> Result<TypeExpr, String> {
    let ty = ty.trim();
    if let Some(inner) = ty.strip_suffix("[]") {
        return Ok(TypeExpr::List(Box::new(type_expr(inner, span)?), span));
    }
    if let Some(inner) = ty.strip_prefix("option<").and_then(|t| t.strip_suffix('>')) {
        return Ok(TypeExpr::Option(Box::new(type_expr(inner, span)?), span));
    }
    if let Some(members) = ty.strip_prefix("enum<").and_then(|t| t.strip_suffix('>')) {
        let members: Vec<String> = members.split('|').map(|m| m.trim().to_owned()).collect();
        if members.iter().any(String::is_empty) {
            return Err(format!("`{ty}` has an empty member"));
        }
        return Ok(TypeExpr::Enum(members, span));
    }
    let named = match ty {
        "strings-key" => "string",
        "number" | "string" | "bool" => ty,
        shape
            if shape.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && shape.starts_with(|c: char| c.is_ascii_uppercase()) =>
        {
            shape
        }
        _ => return Err(format!("`{ty}` is not a declaration's type")),
    };
    Ok(TypeExpr::Named(named.to_owned(), span))
}

fn object(value: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    value
        .as_object()
        .unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}
