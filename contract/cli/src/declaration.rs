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
            for (owner, members) in object(&json["members"]) {
                for (member, spec) in object(members) {
                    let decl = member_decl(owner, member, spec, package, &json, span)
                        .map_err(refuse)?;
                    file.modules.push(decl);
                }
            }
            groups(&json, &file).map_err(refuse)?;
            // An implementation made when the module loads, after first
            // pixel, rather than at its first call (LLP 1133.015 D16 item 1);
            // the hosts' builds read it from the interface.
            if !json["start"].is_null() && json["start"] != "load" {
                return Err(refuse("an interface's `start` is \"load\" or absent".into()));
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

/// The binding's groups (LLP 1133.011 D10 item 3): each object the binding
/// exports, its members by the name the binding gives them, each a declared
/// name, or a function over a source's field (`{"over": source, "field":
/// f}`, which answers whether it is `true`). A list names its members as
/// they are declared. The binding reads them; Contract names no group.
fn groups(json: &Value, file: &File) -> Result<(), String> {
    let declared = |name: &str| file.modules.iter().any(|m| m.declared == name);
    for (group, members) in object(&json["groups"]) {
        let members: Vec<(String, Value)> = match members {
            Value::Array(names) => (names.iter())
                .map(|n| (n.as_str().unwrap_or_default().to_owned(), n.clone()))
                .collect(),
            Value::Object(by) => by.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            _ => return Err(format!("group `{group}` is a list of names or an object")),
        };
        for (member, of) in members {
            let ok = match &of {
                Value::String(name) => declared(name),
                Value::Object(o) => {
                    o["over"].as_str().is_some_and(declared) && o["field"].is_string()
                }
                _ => false,
            };
            if !ok {
                return Err(format!(
                    "group `{group}`'s `{member}` is a declared name, or {{\"over\": a source, \"field\"}}"
                ));
            }
        }
    }
    Ok(())
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
            let fields = match keyed(fields_json) {
                Some(keys) => keyed_fields(name, keys, span)?,
                None => fields(object(fields_json), span)?,
            };
            Ok(ShapeDecl {
                name: name.clone(),
                fields,
                span,
            })
        })
        .collect()
}

/// A shape declared as a record keyed by string (LLP 1133.006 D8 item 1),
/// `{"record": {key: type}}`: its keys and their types, as the Clipboard
/// API's `ClipboardItem` maps a MIME type to its data. `None` for a shape of
/// fields.
fn keyed(shape: &Value) -> Option<&Map<String, Value>> {
    shape
        .as_object()
        .filter(|s| s.len() == 1)?
        .get("record")?
        .as_object()
}

/// A keyed shape's keys as Contract's optional fields: an item gives any of
/// them. A key that takes an `app:/` path or bytes (`{"type": "path", "or":
/// "bytes"}`) is a string to Contract, which never calls a name that takes
/// one (`"contract": false`); so is a string held to its UTF-8 bytes
/// (`{"type": "string", "maxBytes": n}`).
fn keyed_fields(shape: &str, keys: &Map<String, Value>, span: Span) -> Result<Vec<Field>, String> {
    keys.iter()
        .map(|(key, ty)| {
            let ty = match ty {
                Value::Object(_) if ty["type"] == "path" => {
                    if !matches!(ty["or"].as_str(), None | Some("bytes")) {
                        return Err(format!("`{shape}`'s key `{key}` is a path `or` bytes"));
                    }
                    "string"
                }
                Value::Object(_) if ty["type"] == "string" => "string",
                ty => ty.as_str().ok_or_else(|| {
                    format!("`{shape}`'s key `{key}` has a type, or is a path's object")
                })?,
            };
            Ok(Field {
                name: key.clone(),
                ty: TypeExpr::Option(Box::new(type_expr(ty, span)?), span),
                span,
            })
        })
        .collect()
}

fn fields(map: &Map<String, Value>, span: Span) -> Result<Vec<Field>, String> {
    map.iter()
        .map(|(name, ty)| {
            // A path option (LLP 1133.009 D10 item 4) is a string to Contract;
            // a list of them (`"list": true`, LLP 1133.024 D16: a mail's
            // attachments) an optional list of strings.
            let ty = match ty {
                Value::Object(_) if ty["type"] == "path" && ty["list"] == true => {
                    "option<string[]>"
                }
                Value::Object(_) if ty["type"] == "path" && ty["optional"] == true => {
                    "option<string>"
                }
                Value::Object(_) if ty["type"] == "path" => "string",
                // An option with bounds or a default, `{"type", "min", "max",
                // "default"}` (LLP 1133.013 D12 item 2): its type is `type`.
                Value::Object(_) => ty["type"]
                    .as_str()
                    .ok_or_else(|| format!("`{name}`'s object names its `type`, or is a path's"))?,
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
    // An answer typed by an argument (LLP 1133.016 D15 item 4), `{"shape":
    // "<option>", "else": "<Shape>"}`: a call whose literal option names a
    // shape answers that shape, any other `else`.
    let written = match &entry["answers"] {
        Value::Object(by) if by.get("shape").is_some_and(Value::is_string) => {
            by.get("else").cloned().unwrap_or(Value::Null)
        }
        other => other.clone(),
    };
    let answers = match (&written, kind) {
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
    // `none` is never held or refused. Without it, `request` says. Where
    // the interface declares several permissions, a name's `permission`
    // names the one it asks instead (`permission_of`).
    let admission = match &entry["permission"] {
        Value::Null => None,
        Value::String(s) if matches!(s.as_str(), "asks" | "request" | "none") => Some(s.as_str()),
        _ if interface["permission"].is_array() => None,
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
    // A WebAuthn dictionary passed whole (`json<T>`, LLP 1133.011 D10) is a
    // string to Contract, which may not call a name that takes or answers one.
    let data_only = match &entry["contract"] {
        Value::Null => false,
        Value::Bool(false) => true,
        other => {
            return Err(format!(
                "`{name}`'s `contract` is false or absent, not `{other}`"
            ))
        }
    };
    // A name whose signature carries bytes in every form (LLP 1133.006 D8
    // item 1) is data code's alone too, refused in Contract by its own id
    // (`contract-module-bytes`, LLP 1133.022 D19 item 4). An option that may
    // be a string or bytes (`string|bytes`) keeps the name in Contract, with
    // the string alone (LLP 1133.035 D2).
    let bytes = std::iter::once(&entry["answers"])
        .chain(object(&entry["options"]).values())
        .any(|t| carries_bytes(t, interface, &mut Vec::new()));
    let data_only = data_only || bytes;
    let takes_json = std::iter::once(&entry["answers"])
        .chain(object(&entry["options"]).values())
        .any(is_json);
    if takes_json && !data_only {
        return Err(format!(
            "`{name}` takes or answers a `json<…>`, which only a name for data code alone (`\"contract\": false`) may"
        ));
    }
    let (raised, hold, budget) = if kind == ModuleKind::Event {
        let raised = match entry["raised"].as_str() {
            Some(r @ ("activation" | "emit" | "wake")) => r,
            _ => {
                return Err(format!(
                    "the event `{name}` is `raised` by `activation`, `emit` or `wake`"
                ))
            }
        };
        // A wake's time is the platform's to give: at least a second, at
        // most WorkManager's ten minutes (LLP 1133.017 D6).
        let budget = match (raised, entry["budget"].as_u64()) {
            ("wake", Some(b @ 1_000..=600_000)) => b as u32,
            ("wake", _) => {
                return Err(format!(
                    "the wake event `{name}` declares its `budget`, from 1000 to 600000 ms"
                ))
            }
            (_, None) => 0,
            (_, Some(_)) => {
                return Err(format!(
                    "only an event a wake raises has a `budget`, not `{name}`"
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
        (raised.to_owned(), hold as u32, budget)
    } else {
        (String::new(), 0, 0)
    };
    let args = (entry["args"].as_array().into_iter().flatten())
        .map(|a| {
            a.as_str()
                .ok_or_else(|| format!("`{name}`'s `args` are types"))
                .and_then(|t| type_expr(t, span))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut substitute = substitute(entry, interface)?;
    // A ceremony's own options, beside which a call may carry only a
    // platform's words and another package's members (LLP 1133.014 D10).
    if entry["substitute"]["ceremony"].is_object() {
        let mut json: Value = serde_json::from_str(&substitute).unwrap_or_default();
        let takes: Vec<Value> = object(&entry["options"])
            .keys()
            .map(|k| Value::from(k.as_str()))
            .collect();
        json["takes"] = Value::Array(takes);
        substitute = json.to_string();
    }
    let effect = kind == ModuleKind::Send && answers.is_none();
    if admission.is_some() || effect || bytes {
        let mut json: Value = serde_json::from_str(&substitute).unwrap_or_default();
        if bytes {
            json["bytes"] = true.into();
        }
        if let Some(admission) = admission {
            json["admission"] = admission.into();
        }
        // A send that answers nothing (LLP 1133.004 D5), which the runner
        // runs once its commit stands; a send answering `unit` still answers.
        if effect {
            json["effect"] = true.into();
        }
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
        budget,
        handler: None,
        live: entry["live"] == true,
        request: request.to_owned(),
        sync: entry["sync"]["name"].as_str().map(str::to_owned),
        data_only,
        substitute,
        span,
    })
}

/// A member an interface declares on another interface's name (LLP
/// 1133.014 D10 item 1), as FedCM's WebIDL adds `identity` to
/// `CredentialRequestOptions`: `"members": {"@exact/passkeys#get":
/// {"identity": {"type", "answers", "origin", "foreground", "handles",
/// "substitute"}}}`. It is a row of the plan for data code alone, which the
/// runner reads when the owner's call carries the member's option
/// (`memberOf`, `member`); data code calls the owner, never the row.
fn member_decl(
    owner: &str,
    member: &str,
    spec: &Value,
    package: &str,
    interface: &Value,
    span: Span,
) -> Result<ModuleDecl, String> {
    let Some((owner_package, owner_name)) = owner.split_once('#') else {
        return Err(format!(
            "a member's owner is `<package>#<name>` (`@exact/passkeys#get`), not `{owner}`"
        ));
    };
    if owner_package == package {
        return Err(format!(
            "`{member}` is a member of the interface's own `{owner_name}`: declare it among its options"
        ));
    }
    let uses = interface["uses"].as_str().unwrap_or_default();
    let pinned = uses
        .strip_prefix(owner_package)
        .and_then(|v| v.strip_prefix('@'));
    if pinned.is_none_or(|v| v.is_empty() || v.starts_with(['^', '~', '>', '<', '*'])) {
        return Err(format!(
            "a member of `{owner}` needs `\"uses\": \"{owner_package}@<version>\"`, pinned exactly (LLP 1133.009 D3)"
        ));
    }
    let shape = |key: &str| -> Result<String, String> {
        let t = spec[key].as_str().unwrap_or_default();
        match interface["shapes"][t].is_object() {
            true => Ok(t.to_owned()),
            false => Err(format!(
                "member `{member}`'s `{key}` names one of the interface's shapes, not `{t}`"
            )),
        }
    };
    let options = shape("type")?;
    let answers = shape("answers")?;
    let mut sub = match &spec["substitute"] {
        Value::Null => Map::new(),
        Value::Object(map) => map.clone(),
        other => {
            return Err(format!(
                "member `{member}`'s `substitute` is an object, not `{other}`"
            ))
        }
    };
    let owner_extension = owner_package.rsplit('/').next().unwrap_or(owner_package);
    sub.insert(
        "memberOf".into(),
        format!("{owner_extension}.{owner_name}").into(),
    );
    sub.insert("member".into(), member.into());
    for field in ["origin", "foreground", "handles", "grants"] {
        if !spec[field].is_null() {
            sub.insert(field.into(), spec[field].clone());
        }
    }
    if !interface["grants"].is_null() && !sub.contains_key("grants") {
        sub.insert("grants".into(), interface["grants"].clone());
    }
    Ok(ModuleDecl {
        name: member.to_owned(),
        declared: member.to_owned(),
        package: package.to_owned(),
        extension: package.rsplit('/').next().unwrap_or(package).to_owned(),
        kind: ModuleKind::Send,
        options: vec![Field {
            name: member.to_owned(),
            ty: type_expr(&options, span)?,
            span,
        }],
        words: Vec::new(),
        answers: Some(type_expr(&answers, span)?),
        args: Vec::new(),
        raised: String::new(),
        hold: 0,
        budget: 0,
        handler: None,
        live: false,
        request: "always".to_owned(),
        sync: None,
        data_only: true,
        substitute: Value::Object(sub).to_string(),
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
/// lines (`nameGrants`: a list, or a line per argument, `"*"` for every
/// call), the answer's
/// `handles`, the option a polled source's period is (`poll`), and its path
/// options (`paths`), each option's `bounds`, a polled source's batch
/// (`samples`), the canvas input a send `feeds`, and a send made inside the
/// press (`gesture`). An `origin` of `"person"` refuses a tool or a wake
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
    // the runner reads it from any name of the interface. Its `agent` names
    // the words a drive may prefer for its implementation's agent path alone
    // (LLP 1133.012 §9: `speech-recognition.heard`), each with its type.
    // Its `images`, an `img` source scheme the package serves (LLP 1133.019
    // D11 item 1), is read by picture admission from any name too. Its
    // `tables` are what a `fill` reads by a field (`{"table", "by"}`: a
    // gamepad's labels by its family, LLP 1133.022 D2). Its `devices` says
    // how the substitute keeps a module of plugged-in devices (LLP 1133.037
    // §11: the agent word that lists them, the handle kind, whether an open
    // is exclusive, the record's fields and the targets with none).
    for field in [
        "permission",
        "grants",
        "authenticator",
        "agent",
        "images",
        "tables",
        "devices",
    ] {
        if !interface[field].is_null() {
            out.insert(field.into(), interface[field].clone());
        }
    }
    if let Some(images) = interface["images"].as_object() {
        images_checked(images)?;
    }
    if let Some(words) = interface["agent"].as_object() {
        if !words.values().all(|t| {
            t.as_str()
                .is_some_and(|t| type_expr(t, Span::default()).is_ok())
        }) {
            return Err("an interface's `agent` names each word's type".into());
        }
    }
    // A send that shows the person a choice (`choose`, LLP 1133.019 §10;
    // a sheet, LLP 1133.018 D13 item 4): its ticket, the agent word its
    // typed answer sets if it names one, its taps besides `cancel`, whether
    // it takes a typed answer, and the option that lets it take several ids.
    if let Some(choose) = out.get("choose") {
        let word_ok = choose["word"].is_null()
            || (choose["word"].as_str()).is_some_and(|w| !interface["agent"][w].is_null());
        let choices_ok = choose["choices"].is_null()
            || (choose["choices"].as_array())
                .is_some_and(|c| c.iter().all(|c| c.is_string() && c != "cancel"));
        let many_ok = choose["many"].is_null()
            || (choose["many"].as_str()).is_some_and(|o| !object(&entry["options"])[o].is_null());
        let typed_ok = choose["typed"].is_null() || choose["typed"].is_boolean();
        // The targets whose platform shows the sheet (LLP 1133.015 D17:
        // restoring asks on Apple alone); elsewhere the call goes on with no ticket.
        let targets_ok = choose["targets"].is_null()
            || (choose["targets"].as_array()).is_some_and(|t| t.iter().all(Value::is_string));
        // A tap its status rules out (LLP 1133.028 §7): one of its `choices`,
        // by a status source's field.
        let listed =
            |c: &str| (choose["choices"].as_array()).is_some_and(|l| l.iter().any(|x| x == c));
        let unless_ok = choose["unless"].is_null()
            || object(&choose["unless"]).iter().all(|(c, rule)| {
                listed(c) && rule["status"].is_string() && rule["field"].is_string()
            }) && choose["unless"].is_object();
        if !choose["ticket"].is_string()
            || !word_ok
            || !choices_ok
            || !many_ok
            || !typed_ok
            || !unless_ok
            || !targets_ok
        {
            return Err(format!(
                "`{name}`'s `choose` names its ticket, and may name one of the interface's `agent` words, its `choices` besides cancel, whether it is `typed`, the option that allows `many`, the choices its status rules out (`unless`), and the `targets` that show it"
            ));
        }
    }
    if let Value::Array(permissions) = &interface["permission"] {
        let (permission, by, any) = permission_of(entry, name, permissions)?;
        match permission {
            Some(p) => out.insert("permission".into(), p),
            None => out.remove("permission"),
        };
        if !by.is_empty() {
            out.insert("permissionBy".into(), Value::Object(by));
        }
        if !any.is_empty() {
            out.insert("permissionAny".into(), Value::Array(any));
        }
        // Every permission the interface declares, which a status source
        // shared by two answers both of (LLP 1133.019 D5: `state` and
        // `addState`), and a ticket or `prefer permission` finds by name.
        out.insert("permissionList".into(), Value::Array(permissions.clone()));
    }
    match &interface["substitute"] {
        Value::Null => {}
        Value::String(s) if s == "implementation" => {
            out.insert("implementation".into(), Value::Bool(true));
        }
        // The runner keeps the interface's declared lists where its
        // implementation is bound, and hands them over (LLP 1133.033 §13).
        Value::String(s) if s == "runner" => {
            out.insert("runner".into(), Value::Bool(true));
        }
        // An arrival the agent's words stand for (LLP 1133.035 D16 item 1):
        // the names a tag in the field settles, raises and ends, read by
        // the runner from any name of the interface.
        Value::Object(forms)
            if forms.len() == 1
                && forms["arrival"].as_object().is_some_and(|a| {
                    ["field", "from", "status", "scan", "write", "lock", "reading", "readingerror"]
                        .iter()
                        .all(|k| a.get(*k).is_some_and(Value::is_string))
                }) =>
        {
            out.insert("arrival".into(), forms["arrival"].clone());
        }
        other => {
            return Err(format!(
                "an interface's `substitute` is \"implementation\", \"runner\" or {{\"arrival\": {{\"field\", \"from\", \"status\", \"scan\", \"write\", \"lock\", \"reading\", \"readingerror\"}}}}, not `{other}`"
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
    // An option of a keyed shape (or a list of them) whose keys take a path:
    // each item's string at those keys is one (LLP 1133.006 D8 item 3).
    for (option, ty) in object(&entry["options"]) {
        let Some(keys) = ty
            .as_str()
            .and_then(|t| keyed(&interface["shapes"][bare(t).0]))
        else {
            continue;
        };
        let at: Vec<(&String, &Value)> = keys.iter().filter(|(_, t)| t["type"] == "path").collect();
        if let Some((key, first)) = at.first() {
            check(
                matches!(first["access"].as_str(), Some("read" | "write"))
                    && at.iter().all(|(_, t)| t["access"] == first["access"]),
                &format!(
                    "path key `{key}` has an `access` of \"read\" or \"write\", one for every key"
                ),
            )?;
            let keys: Vec<Value> = at.iter().map(|(k, _)| Value::from(k.as_str())).collect();
            paths.insert(
                option.clone(),
                serde_json::json!({ "type": "path", "access": first["access"], "keys": keys }),
            );
        }
    }
    if !paths.is_empty() {
        out.insert("paths".into(), Value::Object(paths));
    }
    // The fields of an option's records that name a URL or an app file,
    // `{"type", "admit": {field: "net.fetch"|"fs.read"|"fs.write"}}`: each
    // record's are admitted as a fetch's URL and a path option are (LLP
    // 1133.017 D3, a transfer's requests).
    let mut admits = Map::new();
    for (option, ty) in object(&entry["options"]) {
        let Some(fields) = ty.get("admit") else {
            continue;
        };
        let ok = fields.as_object().is_some_and(|f| {
            f.values()
                .all(|c| matches!(c.as_str(), Some("net.fetch" | "fs.read" | "fs.write")))
        });
        check(
            ok,
            &format!("option `{option}`'s `admit` maps fields to \"net.fetch\", \"fs.read\" or \"fs.write\""),
        )?;
        admits.insert(option.clone(), fields.clone());
    }
    if !admits.is_empty() {
        out.insert("admits".into(), Value::Object(admits));
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
    // Each option's bounds and default (`{"type", "min", "max", "default"}`):
    // the bake refuses a literal outside them, admission clamps a computed
    // one, and an omitted one takes the default (LLP 1133.013 D12 item 2).
    let mut bounds = Map::new();
    for (option, ty) in object(&entry["options"]) {
        if !ty.is_object() || ty["type"] == "path" {
            continue;
        }
        let mut b = Map::new();
        for key in ["min", "max", "default"] {
            if let Some(v) = ty.get(key) {
                check(
                    v.is_number(),
                    &format!("option `{option}`'s `{key}` is a number"),
                )?;
                b.insert(key.into(), v.clone());
            }
        }
        bounds.insert(option.clone(), Value::Object(b));
    }
    if !bounds.is_empty() {
        out.insert("bounds".into(), Value::Object(bounds));
    }
    // The options that name a canvas's surface (LLP 1133.013 D12 item 3).
    let surfaces: Vec<Value> = (object(&entry["options"]).iter())
        .filter(|(_, ty)| ty.as_str().is_some_and(|t| t.trim() == "surface"))
        .map(|(option, _)| Value::String(option.clone()))
        .collect();
    if !surfaces.is_empty() {
        out.insert("surfaceOptions".into(), Value::Array(surfaces));
    }
    // The options that take a CSS colour (LLP 1133.022 D10 item 2).
    let colors: Vec<Value> = (object(&entry["options"]).iter())
        .filter(|(_, ty)| ty.as_str().is_some_and(|t| bare(t).0 == "color"))
        .map(|(option, _)| Value::String(option.clone()))
        .collect();
    if !colors.is_empty() {
        out.insert("colorOptions".into(), Value::Array(colors));
    }
    // Where a call writes a shape's name (LLP 1133.016 D15 item 4), which
    // the bake writes as its JSON Schema: an option, or a field of the
    // records an option takes (`tools[].inputSchema`); and the option a
    // typed answer follows.
    let names = shape_names(entry, interface);
    if !names.is_empty() {
        out.insert("shapeNames".into(), Value::Array(names));
    }
    if let Some(by) = entry["answers"]["shape"].as_str() {
        check(
            names_option(entry, by),
            &format!("typed answer follows `{by}`, one of its shape-name options"),
        )?;
        out.insert("answersBy".into(), Value::String(by.to_owned()));
    }
    let option = |name: &Value| {
        name.as_str()
            .is_some_and(|o| !object(&entry["options"])[o].is_null())
    };
    // The options, and whether the answer, that are WebAuthn's dictionaries
    // passed whole: the Rust binding takes `serde_json::Value` for them.
    let json_options: Vec<Value> = (object(&entry["options"]).iter())
        .filter(|(_, ty)| is_json(ty))
        .map(|(option, _)| Value::String(option.clone()))
        .collect();
    if !json_options.is_empty() || is_json(&entry["answers"]) {
        let json =
            serde_json::json!({ "options": json_options, "answers": is_json(&entry["answers"]) });
        out.insert("json".into(), json);
    }
    if entry["contract"] == false {
        out.insert("dataOnly".into(), Value::Bool(true));
    }
    // The options the bake requires a literal for (LLP 1133.026 D19 item 4:
    // a `read` list the Android manifest derives each permission from).
    let literal: Vec<Value> = (object(&entry["options"]).iter())
        .filter(|(_, ty)| ty["literal"] == true)
        .map(|(option, _)| Value::String(option.clone()))
        .collect();
    if !literal.is_empty() {
        out.insert("literal".into(), Value::Array(literal));
    }
    // The options that name a handler, each a string literal (LLP 1133.034 D5).
    let handlers: Vec<Value> = (object(&entry["options"]).iter())
        .filter(|(_, ty)| ty.as_str().is_some_and(|t| bare(t).0 == "handler"))
        .map(|(option, _)| Value::String(option.clone()))
        .collect();
    if !handlers.is_empty() {
        out.insert("handlerOptions".into(), Value::Array(handlers));
    }
    if let Value::Array(permissions) = &interface["permission"] {
        permission_templates(entry, name, permissions, &mut out)?;
    }
    for (field, value) in object(entry) {
        match field.as_str() {
            // `{"whenAny": [options], "is": "app"}` holds a call to `app`
            // only when it gives one of those options (LLP 1133.012 D12
            // item 5): a recognition of a track is a capture start, of a
            // file not.
            // `{"none": ["tool", "wake"]}`: refused while any of those is
            // among the call's possible causes (LLP 1133.035 D16 item 6).
            "origin" => check(
                matches!(value.as_str(), Some("app" | "person"))
                    || (value["is"] == "app"
                        && value["whenAny"].as_array().is_some_and(|any| {
                            (any.iter()).all(|o| {
                                o.as_str()
                                    .is_some_and(|o| !object(&entry["options"])[o].is_null())
                            })
                        }))
                    || (value.as_object().is_some_and(|o| o.len() == 1)
                        && value["none"].as_array().is_some_and(|none| {
                            !none.is_empty()
                                && none.iter().all(|w| {
                                    matches!(w.as_str(), Some("device" | "wake" | "tool"))
                                })
                        })),
                "`origin` is \"app\", \"person\", {\"whenAny\": [its options], \"is\": \"app\"}, or {\"none\": [\"device\", \"wake\" or \"tool\"]}",
            )?,
            "foreground" => check(value.is_boolean(), "`foreground` is true or false")?,
            // A `foreground` name admitted while hidden when the app holds a
            // call, on iOS and Android (LLP 1133.033 D10).
            "inCall" => check(value.is_boolean(), "`inCall` is true or false")?,
            // A reader whose origin may not start the name joins a group the
            // app's own readers run (LLP 1133.036 D3).
            "join" => check(value.is_boolean(), "`join` is true or false")?,
            // A source whose answers are the person's own data, which no
            // tool's result may read (LLP 1133.026 D7, D19 item 6).
            "person" => check(value.is_boolean(), "`person` is true or false")?,
            "when" => check(value.is_object(), "`when` is an object of argument values")?,
            "grants" => check(
                value.is_array()
                    || (value.as_object()).is_some_and(|by| by.values().all(argument_grant)),
                "`grants` is a list of lines, or a line per argument: a line, a list of them, or {\"line\", \"matches\": \"target\"} with the code it `refuse`s and what an `omitted` argument stands for (\"page\")",
            )?,
            "withhold" => check(
                (value.as_array()).is_some_and(|paths| paths.iter().all(Value::is_string)),
                "`withhold` is a list of the answer's field paths",
            )?,
            "handles" => check(value.is_object(), "`handles` names each id field's kind")?,
            // A polled source's batch (LLP 1133.013 D12 item 1): the options
            // that bound it and set its rate, on a polled source.
            "samples" => check(
                value.is_object()
                    && ["bound", "frequency"]
                        .iter()
                        .all(|k| value[k].is_null() || option(&value[k]))
                    && !entry["poll"].is_null(),
                "`samples` is an object naming its own options, on a polled source",
            )?,
            // A send that feeds a canvas's input (D12 item 3).
            "feeds" => check(
                value.is_object() && option(&value["surface"]) && option(&value["sensor"]),
                "`feeds` names its surface and sensor options",
            )?,
            // A send made inside the press that asks it (D12 item 5).
            "gesture" => check(value.is_boolean(), "`gesture` is true or false")?,
            // The grant lines a request is satisfied by, narrowest first: a
            // permission decision grants the first the app declares (LLP
            // 1133.018 D13 item 3).
            "asks" => check(
                (value.as_array())
                    .is_some_and(|l| !l.is_empty() && l.iter().all(Value::is_string)),
                "`asks` lists the grant lines that satisfy it, narrowest first",
            )?,
            // A module name that writes the person's store under its line,
            // which derives the line's write permission where a platform
            // splits it (Android's `WRITE_CALENDAR`, D11).
            "writes" => check(value.is_boolean(), "`writes` is true or false")?,
            // What an app that uses the name declares to its platform beside
            // its grant line's own (LLP 1133.026 D19 item 4), by target
            // (`ios`: a usage key, written with the line's purpose;
            // `android`: a permission): a list, derived by the name's use,
            // or per option, a permission for each literal value the app
            // writes, or one for a flag it sets.
            "derives" => check(
                (value.as_object()).is_some_and(|by| {
                    by.iter().all(|(target, d)| {
                        matches!(target.as_str(), "ios" | "android")
                            && (d.as_array().is_some_and(|l| l.iter().all(Value::is_string))
                                || d.as_object().is_some_and(|o| {
                                    o.values().all(|v| {
                                        v.is_string()
                                            || v.as_object().is_some_and(|m| {
                                                m.values().all(Value::is_string)
                                            })
                                    })
                                }))
                    })
                }),
                "`derives` names, for `ios` or `android`, a list of what the name's use declares, or per option a permission for each value or the flag",
            )?,
            // A read of the app's own item, let through under another grant
            // line on the targets named (LLP 1133.019 D11 item 4).
            "owned" => check(
                (value.as_object()).is_some_and(|o| o.values().all(Value::is_string)),
                "`owned` names a grant line for each target",
            )?,
            "poll" => {
                let option = value.as_str().or(value["option"].as_str());
                let default = &value["default"];
                // `while`: the answer's field and the values for which the
                // poll goes on (LLP 1133.016 D15 item 3).
                let during = &value["while"];
                let during_ok = during.is_null()
                    || during.as_object().is_some_and(|w| {
                        w.values().all(|v| {
                            v.as_array()
                                .is_some_and(|vs| vs.iter().all(Value::is_string))
                        })
                    });
                check(
                    option.is_some_and(|o| !object(&entry["options"])[o].is_null())
                        && (default.is_null() || default.is_number())
                        && during_ok,
                    "`poll` names one of its options, or is {\"option\", \"default\", \"while\"}: a default period, and the answer's values it goes on for",
                )?
            }
            // The implementation lays the agent's preferred value over its
            // own answer, since what it does next depends on it (LLP
            // 1133.016 §9: a download started while `downloadable`).
            "overlays" => check(
                value == "implementation",
                "`overlays` is \"implementation\"",
            )?,
            // A send that may start a download (LLP 1133.016 D5, D10): the
            // availability it starts one in, and what the call needs then.
            "download" => check(
                value["when"].is_string(),
                "`download` names the availability `when` it starts one",
            )?,
            // The field path in a call's options that names data sources its
            // implementation may run (`invoke`, LLP 1133.016 D15 item 1); and
            // the handle option whose handlers a call's own handles take.
            "handlers" => check(value.is_string(), "`handlers` is a field path")?,
            // A live source whose last fresh answer seeds the next launch's
            // first frame (LLP 1133.015 D4, LLP 1027 D4).
            "kept" => check(
                value.is_boolean() && entry["live"] == true && !entry["source"].is_null(),
                "`kept` is true or false, on a live source",
            )?,
            // The handler options a pending call may call back through
            // `invoke`, each marked `final` when, once called, it holds the
            // call against its cancel (LLP 1133.034 D16 item 1).
            "callbacks" => check(
                (value.as_object()).is_some_and(|by| {
                    by.iter().all(|(option, spec)| {
                        (object(&entry["options"])[option].as_str())
                            .is_some_and(|t| bare(t).0 == "handler")
                            && spec.is_object()
                            && (spec["final"].is_null() || spec["final"].is_boolean())
                    })
                }),
                "`callbacks` names its handler options, each `{}` or `{\"final\": true}`",
            )?,
            // The package view whose press the web's call adopts (D16 item 7).
            "adopts" => check(
                value.as_str().is_some_and(|v| !interface["views"][v].is_null()),
                "`adopts` names one of the interface's views",
            )?,
            // The dictionaries of each payment method, by `supportedMethods`
            // (D16 item 4): the keys of its `data` and of a modifier's, the
            // grant line it needs, the targets that have it, the keys that
            // exclude one another, and the keys refused with a code.
            "methods" => check(
                (value.as_object()).is_some_and(|by| {
                    by.values().all(|m| {
                        let names = |k: &str| {
                            m[k].is_null()
                                || (m[k].as_array()).is_some_and(|l| l.iter().all(Value::is_string))
                        };
                        names("data")
                            && names("modifier")
                            && names("targets")
                            && names("exclusive")
                            && m["grant"].is_string()
                            && (m["refuse"].is_null() || m["refuse"].is_object())
                    })
                }),
                "`methods` gives each method's `grant`, and lists its `data`, `modifier`, `targets` and `exclusive` keys",
            )?,
            // A send's positional parameters, each one of its options in
            // order, and the group of sends whose calls land in the order
            // they were made (LLP 1133.006 D8 item 1, D2): the binding's.
            "params" => {
                check(
                    (value.as_array()).is_some_and(|p| p.iter().all(option))
                        && object(&entry["options"]).len() == value.as_array().map_or(0, Vec::len),
                    "`params` lists every one of its options, in order",
                )?;
                continue;
            }
            "sequence" => {
                check(value.is_string(), "`sequence` names the group its calls are ordered in")?;
                continue;
            }
            "handlersFrom" => check(
                value.as_str().is_some_and(|o| {
                    (object(&entry["options"])[o].as_str())
                        .is_some_and(|t| t.trim().starts_with("handle<"))
                }),
                "`handlersFrom` names one of its handle options",
            )?,
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

/// One argument's grant: a line (or any of a list) the app writes when the
/// argument is given, or a line whose target must be the argument's value
/// (`{"line": "auth.passkey", "matches": "target"}`, LLP 1133.011 D6), with
/// the code a refusal answers (`refuse`) and what an omitted argument stands
/// for (`"omitted": "page"`: the page's host on the web; none elsewhere). A
/// path through a list (`methodData[].data.merchantIdentifier`) matches each
/// element whose fields are `when`'s (LLP 1133.034 D13).
fn argument_grant(spec: &Value) -> bool {
    match spec {
        Value::String(_) => true,
        Value::Array(lines) => lines.iter().all(Value::is_string),
        Value::Object(o) => {
            o.get("line").is_some_and(Value::is_string)
                && o.get("matches").is_none_or(|m| m == "target")
                && o.get("refuse").is_none_or(Value::is_string)
                && o.get("omitted").is_none_or(|m| m == "page")
                && o.get("when").is_none_or(Value::is_object)
                && (o.keys()).all(|k| {
                    matches!(
                        k.as_str(),
                        "line" | "matches" | "refuse" | "omitted" | "when"
                    )
                })
        }
        _ => false,
    }
}

/// Whether a declaration's type is `json<T>`.
fn is_json(ty: &Value) -> bool {
    ty.as_str()
        .is_some_and(|t| t.trim().starts_with("json<") && t.trim().ends_with('>'))
}

/// The permission a name asks, where an interface declares several, each
/// with the grant lines it covers (`"grants"`; LLP 1133.008 D6, LLP 1133.009
/// D3: `camera` and `microphone`). A status source asks its own; a name
/// whose grants are a line per argument asks, per argument, the permission
/// of that argument's line (`permissionBy`; the runtime asks the ones the
/// call's arguments need), and the first as its default; a name with a list
/// of lines asks the permission that covers them; a name with no lines asks
/// none. A name may say which it asks (`"permission"`): one by its name,
/// none (`false`), or `"narrowest"`, the first of its grant lines' that the
/// app declares (`permissionAny`, in the order of its `grants`; LLP
/// 1133.019 D4: an add asks add-only access where the app has that line).
#[allow(clippy::type_complexity)]
fn permission_of(
    entry: &Value,
    name: &str,
    permissions: &[Value],
) -> Result<(Option<Value>, Map<String, Value>, Vec<Value>), String> {
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
        return Ok((Some(own.clone()), Map::new(), Vec::new()));
    }
    match &entry["permission"] {
        Value::Null => {}
        // An admission word, as with one permission: the name asks the
        // permission that covers its grant lines (LLP 1133.030 D12 item 1).
        Value::String(s) if matches!(s.as_str(), "asks" | "request" | "none") => {}
        Value::Bool(false) => return Ok((None, Map::new(), Vec::new())),
        Value::String(s) if s == "narrowest" => {
            let mut any = Vec::new();
            for line in lines(&entry["grants"]) {
                let p = covering(&Value::String(line))?;
                if !any.contains(&p) {
                    any.push(p);
                }
            }
            if any.is_empty() {
                return Err(format!(
                    "`{name}` asks the narrowest permission of no grant lines"
                ));
            }
            return Ok((any.first().cloned(), Map::new(), any));
        }
        Value::String(named) => {
            let p = (permissions.iter())
                .find(|p| p["name"] == named.as_str())
                .ok_or_else(|| {
                    format!("`{name}` asks permission `{named}`, which is not declared")
                })?;
            return Ok((Some(p.clone()), Map::new(), Vec::new()));
        }
        other => {
            let want = "a permission's name, \"narrowest\" or false";
            return Err(format!("`{name}`'s `permission` is {want}, not `{other}`"));
        }
    }
    match &entry["grants"] {
        Value::Object(by_argument) => {
            let mut by = Map::new();
            for (argument, line) in by_argument {
                by.insert(argument.clone(), covering(line)?);
            }
            Ok((by.values().next().cloned(), by, Vec::new()))
        }
        Value::Null => Ok((None, Map::new(), Vec::new())),
        line => Ok((Some(covering(line)?), Map::new(), Vec::new())),
    }
}

/// A name whose permission is chosen by an argument's value (LLP 1133.026
/// D19 item 1): `"permissionBy": {"type": "health.{}"}` asks `health.steps`
/// of `type="steps"`, a platform's word (`apple.quantityType`) before the
/// shared option it beats; `"permissionEach": {"read": "health.{}"}` asks
/// the permission of every listed value, and of every flag the call sets,
/// in one ticket. Each becomes a list of `[option, template]` pairs,
/// `permissionValue` and `permissionEach`, in the order the runtime tries
/// them. A template names a declared permission once `{}` is filled, or
/// one a `<prefix>.*` permission stands for.
fn permission_templates(
    entry: &Value,
    name: &str,
    permissions: &[Value],
    out: &mut Map<String, Value>,
) -> Result<(), String> {
    let declared = |p: &str| {
        (permissions.iter()).any(|d| {
            d["name"].as_str().is_some_and(|n| {
                n == p || n.strip_suffix('*').is_some_and(|pre| p.starts_with(pre))
            })
        })
    };
    for (field, key) in [
        ("permissionBy", "permissionValue"),
        ("permissionEach", "permissionEach"),
    ] {
        let Some(by) = entry[field].as_object() else {
            if !entry[field].is_null() {
                return Err(format!(
                    "`{name}`'s `{field}` maps options to permission names"
                ));
            }
            continue;
        };
        let mut pairs: Vec<(&String, &str)> = Vec::new();
        for (option, template) in by {
            let Some(template) = template.as_str() else {
                return Err(format!(
                    "`{name}`'s `{field}.{option}` is a permission name, `{{}}` for the value"
                ));
            };
            // Each value an enum option can take must name a declared permission.
            let ty = object(&entry["options"])
                .get(option.as_str())
                .cloned()
                .unwrap_or_default();
            let ty = ty
                .as_str()
                .or(ty["type"].as_str())
                .unwrap_or_default()
                .to_owned();
            let members: Vec<String> = match bare(&ty)
                .0
                .strip_prefix("enum<")
                .and_then(|t| t.strip_suffix('>'))
            {
                Some(m) => m.split('|').map(str::to_owned).collect(),
                None => vec!["x".to_owned()],
            };
            if let Some(missing) = members
                .iter()
                .map(|m| template.replace("{}", m))
                .find(|p| !declared(p))
            {
                return Err(format!(
                    "`{name}`'s `{field}.{option}` asks `{missing}`, which is not declared"
                ));
            }
            pairs.push((option, template));
        }
        // A platform's word beats the shared option it stands beside (D10);
        // a list's flags are asked after its values.
        let by_value = key == "permissionValue";
        pairs.sort_by_key(|(option, _)| option.contains('.') != by_value);
        let list = pairs
            .into_iter()
            .map(|(o, t)| serde_json::json!([o, t]))
            .collect();
        out.insert(key.into(), Value::Array(list));
    }
    Ok(())
}

/// The schemes a host owns, which no package's `images` may take (LLP
/// 1133.019 D11 item 1).
pub(crate) const HOST_SCHEMES: &[&str] = &[
    "app", "doc", "symbol", "data", "http", "https", "blob", "file", "asset",
];

/// An interface's `images`: `{"scheme", "grants", "permission"?, "owned"?}`,
/// its scheme a lowercase word no host owns.
fn images_checked(images: &Map<String, Value>) -> Result<(), String> {
    let scheme = images
        .get("scheme")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let word = !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_lowercase())
        && scheme
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !word {
        return Err(format!("`images.scheme` `{scheme}` is a lowercase word"));
    }
    if HOST_SCHEMES.contains(&scheme) {
        return Err(format!("`images.scheme` `{scheme}` is a host's own"));
    }
    if !images.get("grants").is_some_and(|g| {
        g.as_array()
            .is_some_and(|g| !g.is_empty() && g.iter().all(Value::is_string))
    }) {
        return Err("`images.grants` lists the grant lines a picture is drawn under".into());
    }
    if images.get("permission").is_some_and(|p| !p.is_string()) {
        return Err("`images.permission` names the permission a picture is drawn under".into());
    }
    if images.get("owned").is_some_and(|o| {
        !o.as_object()
            .is_some_and(|o| o.values().all(Value::is_string))
    }) {
        return Err("`images.owned` names a grant line for each target".into());
    }
    Ok(())
}

/// A declaration's type (LLP 1133.002 D13): `number`, `string`, `bool`,
/// `option<T>`, `T[]`, `enum<a|b>`, a shape's name, `strings-key`, a key of
/// the app's strings table, `handle<kind>`, the id of a native object the
/// session holds (LLP 1133.009 D10 item 10), `surface`, a canvas's surface
/// name (LLP 1133.013 D12 item 3), `color`, a CSS colour (LLP 1133.022 D10
/// item 2), `json<T>`, a WebAuthn dictionary passed whole (LLP 1133.011
/// D10), or `handler`, a function of the data module's `tools` (LLP
/// 1133.034 D5); each is a string here. `unit` answers nothing.
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
    if (ty.starts_with("handle<") || ty.starts_with("json<")) && ty.ends_with('>') {
        return Ok(TypeExpr::Named("string".to_owned(), span));
    }
    // One of the app's shapes by name, which the bake writes as its JSON
    // Schema (LLP 1133.016 D15 item 4): a string to Contract.
    if ty == "shape-name" {
        return Ok(TypeExpr::Named("string".to_owned(), span));
    }
    // A function of the data module's `tools` the module calls back while
    // its call is pending, named by a string literal (LLP 1133.034 D5).
    if ty == "handler" {
        return Ok(TypeExpr::Named("string".to_owned(), span));
    }
    let named = match ty {
        // Bytes are data code's (LLP 1133.006 D8 item 1): Contract never
        // calls a name that carries them, and takes the string alone of an
        // option that may be either (`string|bytes`).
        "strings-key" | "surface" | "color" | "bytes" | "string|bytes" => "string",
        "number" | "string" | "bool" | "unit" => ty,
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

/// Whether a value of `ty` carries bytes in every form: `bytes` itself, or
/// a record of the interface's shapes with such a field, at any depth.
fn carries_bytes(ty: &Value, interface: &Value, seen: &mut Vec<String>) -> bool {
    let Some(ty) = ty.as_str().or(ty["type"].as_str()) else {
        return false;
    };
    let named = bare(ty).0;
    if named == "bytes" {
        return true;
    }
    let Some(fields) = interface["shapes"][named].as_object() else {
        return false;
    };
    if seen.iter().any(|s| s == named) {
        return false;
    }
    seen.push(named.to_owned());
    fields.values().any(|t| carries_bytes(t, interface, seen))
}

/// `ty` without `option<…>` and a list's `[]`, and whether a list was there.
fn bare(ty: &str) -> (&str, bool) {
    let mut ty = ty.trim();
    let mut list = false;
    loop {
        if let Some(inner) = ty.strip_prefix("option<").and_then(|t| t.strip_suffix('>')) {
            ty = inner.trim();
        } else if let Some(inner) = ty.strip_suffix("[]") {
            ty = inner.trim();
            list = true;
        } else {
            return (ty, list);
        }
    }
}

/// The paths in a call of `entry` at which a shape's name is written: an
/// option of type `shape-name`, or such a field of the interface's shape an
/// option takes, `tools[].inputSchema`.
fn shape_names(entry: &Value, interface: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for (option, ty) in object(&entry["options"]) {
        let Some(ty) = ty.as_str() else { continue };
        let (named, list) = bare(ty);
        if named == "shape-name" {
            out.push(Value::String(option.clone()));
            continue;
        }
        for (field, fty) in object(&interface["shapes"][named]) {
            if fty.as_str().map(|t| bare(t).0) == Some("shape-name") {
                let at = if list { "[]." } else { "." };
                out.push(Value::String(format!("{option}{at}{field}")));
            }
        }
    }
    out
}

/// Whether `option` is one of `entry`'s `shape-name` options.
fn names_option(entry: &Value, option: &str) -> bool {
    (object(&entry["options"])[option].as_str()).is_some_and(|t| bare(t).0 == "shape-name")
}

fn object(value: &Value) -> &Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    value
        .as_object()
        .unwrap_or_else(|| EMPTY.get_or_init(Map::new))
}
