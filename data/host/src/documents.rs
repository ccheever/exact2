//! `storage.fs` over a `doc:` path (LLP 1069.010 D1): the file or folder
//! the person chose, behind the handle a host minted
//! ([`exact_data::documents`]). The grant is the existing one over the
//! namespace, `fs.read doc:/` or `fs.write doc:/`, checked as ibex2 checks
//! an `app:/` path; then the operation runs on the real location. A
//! handle's own directory (`doc:/<n>`) holds only its entry.
use exact_data::documents::{resolve, Resolved};
use ibex2::grant::{GrantSet, Operation};
use serde_json::{json, Value};
use std::path::Path;

fn stat(path: &Path) -> Result<Value, String> {
    let m = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0.0, |d| d.as_millis() as f64);
    Ok(
        json!({"size": m.len(), "isFile": m.is_file(), "isDirectory": m.is_dir(), "modifiedMs": modified}),
    )
}

/// Run one storage operation on a `doc:` path under `grants`.
pub(super) fn execute(
    grants: &GrantSet,
    op: &str,
    args: &Value,
    data: Option<Vec<u8>>,
) -> Result<Value, String> {
    let path = super::native::text(args, "path")?;
    let write = matches!(
        op,
        "fs.writeFile" | "fs.atomicWriteFile" | "fs.appendFile" | "fs.mkdir" | "fs.rm"
    );
    let operation = if write {
        Operation::FsWrite { path: path.into() }
    } else {
        Operation::FsRead { path: path.into() }
    };
    if !grants.permits(&operation) {
        let grant = if write { "fs.write" } else { "fs.read" };
        return Err(format!("{op} {path}: not granted (needs `{grant} doc:/`)"));
    }
    let real = match resolve(path)? {
        Resolved::Root(name) => {
            return match op {
                "fs.readdir" => Ok(json!([name])),
                "fs.stat" => {
                    Ok(json!({"size": 0, "isFile": false, "isDirectory": true, "modifiedMs": 0}))
                }
                _ => Err(format!(
                    "{op} {path}: a handle's own directory is read-only"
                )),
            }
        }
        Resolved::Real(real) => real,
    };
    let fail = |e: std::io::Error| format!("{op} {path}: {e}");
    Ok(match op {
        "fs.readFile" => {
            let bytes = std::fs::read(&real).map_err(fail)?;
            json!({"base64": exact_runner::agent::base64(&bytes)})
        }
        "fs.stat" => stat(&real).map_err(|e| format!("{op} {path}: {e}"))?,
        "fs.readdir" => {
            let mut names: Vec<String> = std::fs::read_dir(&real)
                .map_err(fail)?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            json!(names)
        }
        "fs.writeFile" => {
            std::fs::write(&real, data.unwrap_or_default()).map_err(fail)?;
            Value::Null
        }
        "fs.appendFile" => {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&real)
                .and_then(|mut f| f.write_all(&data.unwrap_or_default()))
                .map_err(fail)?;
            Value::Null
        }
        "fs.atomicWriteFile" => {
            // Beside the target, so the rename stays on one volume.
            let name = real.file_name().map(|n| n.to_string_lossy().into_owned());
            let staged = real.with_file_name(format!(
                ".{}.exact-{}",
                name.unwrap_or_default(),
                std::process::id()
            ));
            std::fs::write(&staged, data.unwrap_or_default()).map_err(fail)?;
            std::fs::rename(&staged, &real).map_err(|e| {
                let _ = std::fs::remove_file(&staged);
                fail(e)
            })?;
            Value::Null
        }
        "fs.mkdir" => {
            std::fs::create_dir_all(&real).map_err(fail)?;
            Value::Null
        }
        "fs.rm" => {
            if real.is_dir() {
                std::fs::remove_dir(&real).map_err(fail)?;
            } else {
                std::fs::remove_file(&real).map_err(fail)?;
            }
            Value::Null
        }
        _ => return Err(format!("{op} is not available on a document")),
    })
}
