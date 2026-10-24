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
            // A path option (LLP 1133.009 D10 item 4) is a string to Contract.
            let ty = match ty {
                Value::Object(_) if ty["type"] == "path" && ty["optional"] == true => {
                    "option<string>"
                }
                Value::Object(_) if ty["type"] == "path" => "string",
                ty => ty
                    .as_str()
                    .ok_or_else(|| format!("`{name}`'s type is a string, or a path's object"))?,
            };
            Ok(Field {
                name: name.clone(),
                ty: type_expr(ty, span)?,
                span,
            })
        })
        .collect()
}

/// One entry of `names`: a source or a send.
fn module(
    entry: &Value,
    package: &str,
    interface: &Value,
    span: Span,
) -> Result<ModuleDecl, String> {
    let (kind, name) = match (entry["source"].as_str(), entry["send"].as_str()) {
        (Some(name), None) => (ModuleKind::Source, name),
        (None, Some(name)) => (ModuleKind::Send, name),
        _ => return Err("each of `names` is a `source` or a `send`".into()),
    };
    let answers = entry["answers"]
        .as_str()
        .ok_or_else(|| format!("`{name}` names what it `answers`"))?;
    let request = match &entry["request"] {
        Value::Bool(true) => "always",
        Value::String(s) if s == "when-prompt" => "when-prompt",
        Value::Null | Value::Bool(false) => "never",
        other => return Err(format!("`{name}`'s `request` is `{other}`")),
    };
    Ok(ModuleDecl {
        name: name.to_owned(),
        declared: name.to_owned(),
        package: package.to_owned(),
        extension: package.rsplit('/').next().unwrap_or(package).to_owned(),
        kind,
        options: fields(object(&entry["options"]), span)?,
        words: Vec::new(),
        answers: type_expr(answers, span)?,
        live: entry["live"] == true,
        request: request.to_owned(),
        sync: entry["sync"]["name"].as_str().map(str::to_owned),
        substitute: substitute(entry, interface)?,
        span,
    })
}

/// A name's declared substitute, and what the runner's admission reads
/// beside it, as the plan carries it (LLP 1133.002 §6, D14; LLP 1133.009
/// D10): what the agent's runner answers with no implementation bound; the
/// interface's `permission` and `grants`, the lines the app writes one of
/// (D6); `implementation` where the interface is its own substitute
/// (`"substitute": "implementation"`, LLP 1133.006 D8); and the name's own
/// `origin`, `foreground` and the `when` that conditions them, its grant
/// lines (`nameGrants`: a list, or a line per argument), the answer's
/// `handles`, the option a polled source's period is (`poll`), and its path
/// options (`paths`). An `origin` of `"person"` refuses a tool or a wake
/// `PERMISSION_DENIED`, `person-required`, in every state (LLP 1133.010 D12).
fn substitute(entry: &Value, interface: &Value) -> Result<String, String> {
    let name = entry["source"]
        .as_str()
        .or(entry["send"].as_str())
        .unwrap_or_default();
    let mut out = match &entry["substitute"] {
        Value::Null => Map::new(),
        Value::Object(map) => map.clone(),
        other => return Err(format!("a `substitute` is an object, not `{other}`")),
    };
    // An interface's `authenticator` is the entry a data capability calls in
    // process, with the store's operations' substitute (LLP 1133.010 D13, §7):
    // the runner reads it from any name of the interface.
    for field in ["permission", "grants", "authenticator"] {
        if !interface[field].is_null() {
            out.insert(field.into(), interface[field].clone());
        }
    }
    if let Value::Array(permissions) = &interface["permission"] {
        let (permission, by) = permission_of(entry, name, permissions)?;
        match permission {
            Some(p) => out.insert("permission".into(), p),
            None => out.remove("permission"),
        };
        if !by.is_empty() {
            out.insert("permissionBy".into(), Value::Object(by));
        }
    }
    match &interface["substitute"] {
        Value::Null => {}
        Value::String(s) if s == "implementation" => {
            out.insert("implementation".into(), Value::Bool(true));
        }
        other => {
            return Err(format!(
                "an interface's `substitute` is \"implementation\", not `{other}`"
            ))
        }
    }
    let check = |ok: bool, what: &str| match ok {
        true => Ok(()),
        false => Err(format!("`{name}`'s {what}")),
    };
    // Each path option, by name, with the access admission checks (LLP
    // 1133.009 D10 item 4).
    let mut paths = Map::new();
    for (option, ty) in object(&entry["options"]) {
        if ty["type"] != "path" {
            continue;
        }
        check(
            matches!(ty["access"].as_str(), Some("read" | "write")),
            &format!("path option `{option}` has an `access` of \"read\" or \"write\""),
        )?;
        paths.insert(option.clone(), ty.clone());
    }
    if !paths.is_empty() {
        out.insert("paths".into(), Value::Object(paths));
    }
    // The options that name a handle: a read whose every one is empty names
    // no object, and a poll does not ask it again (LLP 1133.009 D10 item 7).
    let handles: Vec<Value> = (object(&entry["options"]).iter())
        .filter(|(_, ty)| ty.as_str().is_some_and(|t| t.trim().starts_with("handle<")))
        .map(|(option, _)| Value::String(option.clone()))
        .collect();
    if !handles.is_empty() {
        out.insert("handleOptions".into(), Value::Array(handles));
    }
    for (field, value) in object(entry) {
        match field.as_str() {
            "origin" => check(
                matches!(value.as_str(), Some("app" | "person")),
                "`origin` is \"app\" or \"person\"",
            )?,
            "foreground" => check(value.is_boolean(), "`foreground` is true or false")?,
            "when" => check(value.is_object(), "`when` is an object of argument values")?,
            "grants" => check(
                value.is_array() || value.is_object(),
                "`grants` is a list of lines, or a line per argument",
            )?,
            "handles" => check(value.is_object(), "`handles` names each id field's kind")?,
            "poll" => {
                let option = value.as_str().or(value["option"].as_str());
                let default = &value["default"];
                check(
                    option.is_some_and(|o| !object(&entry["options"])[o].is_null())
                        && (default.is_null() || default.is_number()),
                    "`poll` names one of its options, or is {\"option\", \"default\"} with a default period",
                )?
            }
            _ => continue,
        }
        let key = if field == "grants" {
            "nameGrants"
        } else {
            field
        };
        out.insert(key.into(), value.clone());
    }
    Ok(Value::Object(out).to_string())
}

/// The permission a name asks, where an interface declares several, each
/// with the grant lines it covers (`"grants"`; LLP 1133.008 D6, LLP 1133.009
/// D3: `camera` and `microphone`). A status source asks its own; a name
/// whose grants are a line per argument asks, per argument, the permission
/// of that argument's line (`permissionBy`; the runtime asks the ones the
/// call's arguments need), and the first as its default; a name with a list
/// of lines asks the permission that covers them; a name with no lines asks
/// none.
fn permission_of(
    entry: &Value,
    name: &str,
    permissions: &[Value],
) -> Result<(Option<Value>, Map<String, Value>), String> {
    let lines = |v: &Value| -> Vec<String> {
        match v {
            Value::String(l) => vec![l.clone()],
            Value::Array(ls) => ls
                .iter()
                .filter_map(|l| l.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        }
    };
    let covering = |line: &Value| -> Result<Value, String> {
        let wanted = lines(line);
        (permissions.iter())
            .find(|p| lines(&p["grants"]).iter().any(|g| wanted.contains(g)))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "`{name}`'s grant {} is covered by no declared `permission`",
                    wanted.join(" or ")
                )
            })
    };
    if let Some(own) = permissions.iter().find(|p| p["status"] == name) {
        return Ok((Some(own.clone()), Map::new()));
    }
    match &entry["grants"] {
        Value::Object(by_argument) => {
            let mut by = Map::new();
            for (argument, line) in by_argument {
                by.insert(argument.clone(), covering(line)?);
            }
            Ok((by.values().next().cloned(), by))
        }
        Value::Null => Ok((None, Map::new())),
        line => Ok((Some(covering(line)?), Map::new())),
    }
}

/// A declaration's type (LLP 1133.002 D13): `number`, `string`, `bool`,
/// `option<T>`, `T[]`, `enum<a|b>`, a shape's name, `strings-key`, a key of
/// the app's strings table, or `handle<kind>`, the id of a native object the
/// session holds (LLP 1133.009 D10 item 10); both are strings here.
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
    if ty.starts_with("handle<") && ty.ends_with('>') {
        return Ok(TypeExpr::Named("string".to_owned(), span));
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
