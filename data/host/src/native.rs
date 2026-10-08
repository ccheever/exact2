//! Native adapter over the same Ibex capabilities used by Exact's TS storage.
use super::Directories;
use exact_runner::{FailureKind, Outcome};
use ibex2::{
    grant::GrantSet,
    stdlib::{
        app_fs::{self, AppDirectories},
        fs::{self, FsOp, FsResult},
        sqlite::{self, Location, Provider},
    },
};
use serde_json::{json, Value};
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub(super) fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
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
                .ok_or_else(|| "storage: invalid byte".into())
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
fn execute(
    grants: &GrantSet,
    directories: Option<&AppDirectories>,
    op: &str,
    args: &Value,
) -> Result<Value, String> {
    let path = text(args, "path")?;
    if !path.starts_with("app:/") && !native_filesystem(op, args) {
        return Err("portable storage needs an app:/ path".into());
    }
    if op == "sqlite" || op == "sqlite.transaction" {
        let commands = commands(args)?;
        if op == "sqlite.transaction" && commands.iter().any(|(k, _)| k != "execute") {
            return Err("SQLite write transaction requires execute commands".into());
        }
        let path = app_fs::resolve_sqlite(grants, directories, path).map_err(error)?;
        let db = sqlite::Database::new(
            ibex2_sqlite::SqliteProvider
                .open(Location { path })
                .map_err(error)?,
        );
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
    if op == "fs.compressImage" {
        return compress_image(grants, directories, args);
    }
    let (operation, destination, data) = operation(op, args)?;
    if path.starts_with("app:/") && destination.is_some_and(|d| !d.starts_with("app:/")) {
        return Err("portable storage needs an app:/ destination".into());
    }
    let result = fs::run(
        grants,
        directories,
        operation,
        path,
        destination,
        data.as_deref(),
    )
    .map_err(error)?;
    Ok(fs_value(result))
}
/// A filesystem request's operation, its destination, and its bytes.
type Operation<'a> = (FsOp, Option<&'a str>, Option<Vec<u8>>);
fn operation<'a>(op: &str, args: &'a Value) -> Result<Operation<'a>, String> {
    Ok(match op {
        "fs.readFile" => (FsOp::ReadFile, None, None),
        "fs.writeFile" => (FsOp::WriteFile, None, Some(bytes(args)?)),
        "fs.atomicWriteFile" => (FsOp::AtomicWriteFile, None, Some(bytes(args)?)),
        "fs.appendFile" => (FsOp::AppendFile, None, Some(bytes(args)?)),
        "fs.mkdir" => (FsOp::Mkdir, None, None),
        "fs.rm" => (FsOp::Remove, None, None),
        "fs.stat" => (FsOp::Stat, None, None),
        "fs.readdir" => (FsOp::ReadDir, None, None),
        "fs.realpath" => (FsOp::Realpath, None, None),
        "fs.rename" => (FsOp::Rename, Some(text(args, "destination")?), None),
        "fs.copyFile" => (FsOp::CopyFile, Some(text(args, "destination")?), None),
        _ => return Err(format!("unsupported storage operation {op}")),
    })
}
/// `fs.compressImage` (LLP 1069.002 A1): the operation a TypeScript source's
/// `storage.fs.compressImage` runs, through ibex2's one executor and the
/// same codec, so a grant and a failure mean the same in either language.
fn compress_image(
    grants: &GrantSet,
    directories: Option<&AppDirectories>,
    args: &Value,
) -> Result<Value, String> {
    let (from, to) = (text(args, "path")?, text(args, "destination")?);
    let number = |k: &str| {
        args[k]
            .as_f64()
            .ok_or_else(|| format!("storage: {k} must be a number"))
    };
    let (max_dimension, max_bytes) =
        exact_data::image::check_limits(number("maxDimension")?, number("maxBytes")?)?;
    let file = fs::compress_image(
        grants,
        directories,
        // Only where there is a codec (LLP 1069.002 A1.3).
        cfg!(target_vendor = "apple").then_some(&image_codec as &fs::ImageCodec),
        from,
        to,
        max_dimension,
        max_bytes,
        // This worker runs the request to its end and no one gives up on it
        // (data/host/src/lib.rs): there is no right to write to take away.
        None,
    )
    .map_err(error)?;
    Ok(json!({
        "path": to,
        "type": "image/jpeg",
        "size": file.size,
        "width": file.width,
        "height": file.height
    }))
}
/// [`exact_data::image::compress`] as ibex2's codec.
fn image_codec(
    bytes: &[u8],
    max_dimension: u32,
    max_bytes: u64,
    deadline: std::time::Instant,
) -> Result<fs::CompressedImage, String> {
    exact_data::image::compress(bytes, max_dimension, max_bytes, deadline).map(|c| {
        fs::CompressedImage {
            bytes: c.bytes,
            width: c.width,
            height: c.height,
        }
    })
}
fn fs_value(result: FsResult) -> Value {
    match result {
        FsResult::Done => Value::Null,
        FsResult::Bytes(bytes) => {
            json!({"base64":exact_runner::agent::base64(&bytes)})
        }
        FsResult::Text(text) => Value::String(text),
        FsResult::Names(names) => json!(names),
        FsResult::Stat(stat) => json!({
            "size": stat.size,
            "isFile": stat.is_file,
            "isDirectory": stat.is_directory,
            "modifiedMs": stat.modified_ms
        }),
    }
}
/// A `doc:` path (LLP 1069.010 D1): the file or folder the person chose,
/// through ibex2's one executor for documents, the one a TypeScript
/// source's `storage.fs` runs, so a grant means the same in either language.
fn document_request(grants: &GrantSet, op: &str, args: &Value) -> Result<Value, String> {
    let path = text(args, "path")?;
    let (operation, _, data) = operation(op, args)?;
    fs::run_document(grants, Some(&documents), operation, path, data.as_deref())
        .map(fs_value)
        .map_err(error)
}
/// [`exact_data::documents`] as ibex2's table.
fn documents(path: &str) -> Result<fs::Document, String> {
    use exact_data::documents::{resolve, Resolved};
    Ok(match resolve(path)? {
        Resolved::Root(name) => fs::Document::Root(name),
        Resolved::Real(real) => fs::Document::Real(real),
    })
}
/// A scripted drive's answer when it names no scratch store: the web's
/// refusal, word for word (`storage-environment.js`, `storage-request.js`),
/// as a TypeScript module's `agent` refusal says it (trivia F7).
pub(super) fn agent_refusal() -> Outcome {
    Outcome::Storage(
        serde_json::to_vec(&json!({"error": "storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)"}))
            .unwrap(),
    )
}
/// Native disk operations do not need (or initialize) the app storage roots.
/// Classify malformed disk spellings too: the filesystem adapter refuses them
/// after source/grant admission, before opening any handle. This is not authority.
fn native_filesystem(op: &str, args: &Value) -> bool {
    cfg!(windows)
        && op.starts_with("fs.")
        && args["path"].as_str().is_some_and(|path| {
            !path.starts_with("app:/") && !exact_data::documents::is_document(path)
        })
}
/// Chosen documents and Windows disk requests require no app directories.
pub(super) fn independent_storage(payload: &[u8]) -> bool {
    serde_json::from_slice::<Value>(payload).is_ok_and(|r| {
        r["args"]["path"]
            .as_str()
            .is_some_and(exact_data::documents::is_document)
            || r["op"]
                .as_str()
                .is_some_and(|op| native_filesystem(op, &r["args"]))
    })
}
pub(super) fn run(paths: Option<&Directories>, grants: &str, payload: &[u8]) -> Outcome {
    let result = (|| {
        if payload.len() > exact_data::storage::MAX_BYTES {
            return Err("storage request exceeds its byte limit".into());
        }
        let request: Value = serde_json::from_slice(payload).map_err(error)?;
        if request["version"] != 1 {
            return Err("unsupported storage protocol".into());
        }
        let op = text(&request, "op")?;
        let grants = GrantSet::parse(&exact_runner::io_grants(grants)).map_err(error)?;
        let args = &request["args"];
        // Two app files, never a document or a disk path (LLP 1069.002 A1.1).
        if op == "fs.compressImage"
            && ![&args["path"], &args["destination"]]
                .iter()
                .all(|p| p.as_str().is_some_and(|p| p.starts_with("app:/")))
        {
            return Err("compressImage: needs app:/ paths".into());
        }
        if args["path"]
            .as_str()
            .is_some_and(exact_data::documents::is_document)
        {
            return document_request(&grants, op, args);
        }
        if native_filesystem(op, args) {
            return execute(&grants, None, op, args);
        }
        if !text(args, "path")?.starts_with("app:/") {
            return Err("portable storage needs an app:/ path".into());
        }
        let paths = paths.ok_or("storage is unavailable in an unconfigured host")?;
        for path in [&paths.data, &paths.cache, &paths.temporary] {
            std::fs::create_dir_all(path).map_err(error)?;
        }
        let directories =
            AppDirectories::new(&paths.data, &paths.cache, &paths.temporary).map_err(error)?;
        execute(&grants, Some(&directories), op, &request["args"])
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
