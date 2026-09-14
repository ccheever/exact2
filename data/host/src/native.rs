//! Native adapter over the same Ibex capabilities used by Exact's TS storage.
use super::Directories;
use exact_runner::{FailureKind, Outcome};
use ibex2::{
    grant::GrantSet,
    host,
    stdlib::{app_fs::AppDirectories, sqlite},
};
use serde_json::{json, Value};
use std::sync::Arc;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str()
        .ok_or_else(|| format!("storage: {k} must be a string"))
}
fn bytes(v: &Value) -> Result<Vec<u8>, String> {
    if let Some(t) = v.get("text").and_then(Value::as_str) {
        return Ok(t.as_bytes().to_vec());
    }
    v["bytes"]
        .as_array()
        .ok_or("storage: bytes must be an array")?
        .iter()
        .map(|n| {
            n.as_u64()
                .filter(|n| *n <= 255)
                .map(|n| n as u8)
                .ok_or("storage: invalid byte".into())
        })
        .collect()
}
fn sql_value(v: &Value) -> Result<sqlite::Value, String> {
    Ok(match v {
        Value::Null => sqlite::Value::Null,
        Value::String(s) => sqlite::Value::Text(s.clone()),
        Value::Number(n) => {
            let n = n.as_f64().ok_or("invalid SQLite number")?;
            if n.fract() == 0.0 {
                if n.abs() > 9_007_199_254_740_991.0 {
                    return Err("unsafe SQLite integer; use a tagged integer".into());
                }
                sqlite::Value::Integer(n as i64)
            } else {
                sqlite::Value::Real(n)
            }
        }
        Value::Object(o) if o.len() == 1 && o.contains_key("integer") => sqlite::Value::Integer({
            let text = text(v, "integer")?;
            let n = text.parse::<i64>().map_err(error)?;
            if n.to_string() != text && text != "-0" {
                return Err("invalid SQLite integer spelling".into());
            }
            n
        }),
        Value::Object(o) if o.len() == 1 && o.contains_key("bytes") => {
            sqlite::Value::Blob(bytes(v)?)
        }
        _ => return Err("invalid SQLite parameter".into()),
    })
}
fn value(v: sqlite::Value) -> Value {
    match v {
        sqlite::Value::Null => Value::Null,
        sqlite::Value::Text(s) => s.into(),
        sqlite::Value::Real(n) => json!(n),
        sqlite::Value::Integer(n) => json!({"integer":n.to_string()}),
        sqlite::Value::Blob(b) => json!({"bytes":b}),
    }
}
fn commands(args: &Value) -> Result<Vec<(String, sqlite::Command)>, String> {
    let rows = args["commands"]
        .as_array()
        .ok_or("storage: commands must be an array")?;
    if rows.len() > 10000 {
        return Err("storage: too many SQLite commands".into());
    }
    rows.iter()
        .map(|c| {
            let kind = text(c, "kind")?;
            if !["execute", "query"].contains(&kind) {
                return Err("invalid SQLite command kind".into());
            }
            let params = c["params"]
                .as_array()
                .ok_or("storage: params must be an array")?
                .iter()
                .map(sql_value)
                .collect::<Result<_, _>>()?;
            Ok((
                kind.into(),
                sqlite::Command {
                    sql: text(c, "sql")?.into(),
                    params,
                },
            ))
        })
        .collect()
}
fn execute(b: &host::Bindings, op: &str, args: &Value) -> Result<Value, String> {
    let path = text(args, "path")?;
    if !path.starts_with("app:/") {
        return Err("portable storage needs an app:/ path".into());
    }
    if op == "sqlite" || op == "sqlite.transaction" {
        let commands = commands(args)?;
        if op == "sqlite.transaction" && commands.iter().any(|(k, _)| k != "execute") {
            return Err("SQLite write transaction requires execute commands".into());
        }
        let db = b.sqlite.open(path).map_err(error)?;
        let result = (|| {
            if op == "sqlite.transaction" {
                let result = db
                    .transaction(&commands.into_iter().map(|(_, c)| c).collect::<Vec<_>>())
                    .map_err(error)?;
                return Ok(json!(result.into_iter().map(|r|json!({"changes":r.changes.to_string(),"lastInsertRowid":r.last_insert_rowid.to_string()})).collect::<Vec<_>>()));
            }
            commands.into_iter().map(|(kind,c)|{
                if kind=="query"{let r=db.query(&c.sql,&c.params).map_err(error)?;Ok(json!({"columns":r.columns,"rows":r.rows.into_iter().map(|row|row.into_iter().map(value).collect::<Vec<_>>()).collect::<Vec<_>>()}))}
                else {let r=db.execute(&c.sql,&c.params).map_err(error)?;Ok(json!({"changes":r.changes.to_string(),"lastInsertRowid":r.last_insert_rowid.to_string()}))}
            }).collect::<Result<Vec<_>,String>>().map(Value::Array)
        })();
        let close = db.close().map_err(error);
        return match result {
            Ok(v) => close.map(|_| v),
            Err(e) => Err(e),
        };
    }
    match op {
        "fs.readFile"=>b.fs.read_file(path).map(|bytes|json!({"base64":exact_runner::agent::base64(&bytes)})).map_err(error),
        "fs.writeFile"=>b.fs.write_file(path,&bytes(args)?).map(|_|Value::Null).map_err(error),
        "fs.atomicWriteFile"=>b.fs.atomic_write_file(path,&bytes(args)?).map(|_|Value::Null).map_err(error),
        "fs.appendFile"=>b.fs.append_file(path,&bytes(args)?).map(|_|Value::Null).map_err(error),
        "fs.mkdir"=>b.fs.mkdir(path).map(|_|Value::Null).map_err(error),
        "fs.rm"=>b.fs.remove(path).map(|_|Value::Null).map_err(error),
        "fs.stat"=>b.fs.stat(path).map(|s|json!({"size":s.size,"isFile":s.is_file,"isDirectory":s.is_directory,"modifiedMs":s.modified_ms})).map_err(error),
        "fs.readdir"=>b.fs.read_dir(path).map(|v|json!(v)).map_err(error),
        "fs.realpath"=>b.fs.realpath(path).map(Value::String).map_err(error),
        "fs.rename"|"fs.copyFile"=>{
            let to=text(args,"destination")?;if !to.starts_with("app:/"){return Err("portable storage needs an app:/ destination".into())}
            if op=="fs.rename"{b.fs.rename(path,to)}else{b.fs.copy_file(path,to)}.map(|_|Value::Null).map_err(error)
        }
        _=>Err(format!("unsupported storage operation {op}")),
    }
}
pub(super) fn run(paths: &Directories, grants: &str, payload: &[u8]) -> Outcome {
    let result = (|| {
        if payload.len() > exact_data::storage::MAX_BYTES {
            return Err("storage request exceeds its byte limit".into());
        }
        let request: Value = serde_json::from_slice(payload).map_err(error)?;
        if request["version"] != 1 {
            return Err("unsupported storage protocol".into());
        }
        let op = text(&request, "op")?;
        let grants = GrantSet::parse(grants).map_err(error)?;
        for path in [&paths.data, &paths.cache, &paths.temporary] {
            std::fs::create_dir_all(path).map_err(error)?;
        }
        let directories =
            AppDirectories::new(&paths.data, &paths.cache, &paths.temporary).map_err(error)?;
        let host = host::Host::new()
            .with_app_directories(directories)
            .with_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider));
        execute(&host.endow(grants), op, &request["args"])
    })();
    match result {
        Ok(v) => {
            let bytes = serde_json::to_vec(&v).unwrap();
            if bytes.len() > exact_data::storage::MAX_BYTES {
                Outcome::Failed {
                    kind: FailureKind::Unsupported,
                    message: "storage result exceeds its byte limit".into(),
                }
            } else {
                Outcome::Storage(bytes)
            }
        }
        Err(message) => Outcome::Storage(serde_json::to_vec(&json!({"error":message})).unwrap()),
    }
}
