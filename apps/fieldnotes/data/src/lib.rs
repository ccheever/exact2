//! Fieldnotes' backup operation moved from TypeScript to portable Rust.
//! @ref LLP 1027.001 — preserve the operation, durable data and grants.
mod backup;

use exact_data::storage;
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Outcome, Store};
use serde_json::{json, Value as Json};

/// The existing Fieldnotes app identity, shared with its TypeScript module.
pub const APP: &str = "com.exact.fieldnotes";
/// The same storage and app revision grants in both language implementations.
pub const GRANTS: &str =
    "sqlite.open app:/data/fieldnotes.db\nfs.read app:/data/backups\nfs.write app:/data/backups\nsecret.keep fieldnotes.revision";
const CREATE: &str = "CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL, body TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0)";
// One SELECT owns the snapshot. Any valid 4 Mi UTF-16 backup fits in 12 MiB
// of quoted UTF-8 text; oversized text becomes NULL before crossing the ABI.
const SELECT: &str = "WITH sized AS (SELECT id, title, body, pinned, SUM(length(CAST(json_quote(title) AS BLOB)) + length(CAST(json_quote(body) AS BLOB))) OVER () AS backup_bytes FROM notes) SELECT id, CASE WHEN backup_bytes <= 12582912 THEN title END, CASE WHEN backup_bytes <= 12582912 THEN body END, pinned FROM sized ORDER BY pinned DESC, id DESC";
const DIRECTORY: &str = "app:/data/backups";
const PATH: &str = "app:/data/backups/fieldnotes.json";

enum Pending {
    Read,
    Directory { text: String, count: usize },
    Write { text: String, count: usize },
}

/// One serialized backup action, called under the Contract's pending-work guard.
/// All storage leaves as portable requests; this crate never opens a file or DB.
#[derive(Default)]
pub struct Backup {
    pending: Option<Pending>,
}

/// The two source owners of the Fieldnotes application.
pub type Data<J> = exact_data::Mixed<J, Backup>;

/// App composition: the existing TypeScript sources and the Rust backup source.
pub fn mixed<J: DataSource>(javascript: J) -> Data<J> {
    exact_data::Mixed::new(
        javascript,
        Backup::default(),
        &[
            "library",
            "saveNote",
            "readBackup",
            "restoreNotes",
            "deleteNote",
        ],
        &["backupNotes"],
    )
    .expect("Fieldnotes sources have distinct owners and the same app identity")
    .with_embedded_rust(|_| Ok(Backup::default()))
}

impl Backup {
    fn read(&mut self) -> Answer {
        self.pending = Some(Pending::Read);
        Answer::Later(storage::request(
            "sqlite",
            json!({"path":"app:/data/fieldnotes.db","commands":[
                {"kind":"execute","sql":CREATE,"params":[]},
                {"kind":"query","sql":SELECT,"params":[]}
            ]}),
        ))
    }

    fn status(
        store: &mut Store,
        result: Result<(String, usize), String>,
    ) -> Result<Answer, DataError> {
        let previous = store
            .get("fieldnotes.revision")
            .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value < 9_007_199_254_740_991)
            .unwrap_or(0);
        let revision = previous + 1;
        store.set("fieldnotes.revision", &revision.to_string())?;
        let (text, message, failed) = match result {
            Ok((text, count)) => (text, format!("Backup saved with {count} notes."), false),
            Err(message) => (String::new(), message, true),
        };
        Ok(Answer::Now(Value::record(vec![
            Value::Number(revision as f64),
            Value::str(&message),
            Value::Bool(failed),
            Value::str(&text),
        ])))
    }
}

fn notes(value: &Json) -> Result<Vec<backup::Note>, String> {
    let invalid = || "Invalid notebook storage response.".to_string();
    let rows = value[1]["rows"].as_array().ok_or_else(invalid)?;
    rows.iter()
        .map(|row| {
            if row[1].is_null() || row[2].is_null() {
                return Err(backup::TOO_LARGE.into());
            }
            Ok(backup::Note {
                id: row[0]["integer"].as_str().ok_or_else(invalid)?.into(),
                title: row[1].as_str().ok_or_else(invalid)?.into(),
                body: row[2].as_str().ok_or_else(invalid)?.into(),
                pinned: row[3]["integer"].as_str() == Some("1"),
            })
        })
        .collect()
}

impl DataSource for Backup {
    fn app_id(&self) -> &str {
        APP
    }

    fn grants(&self) -> &str {
        GRANTS
    }

    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(if source == "backupNotes" {
            DataError::Unavailable("storage is unavailable during bake".into())
        } else {
            DataError::UnknownSource(source.into())
        })
    }

    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
    ) -> Result<Answer, DataError> {
        if source != "backupNotes" {
            return Err(DataError::UnknownSource(source.into()));
        }
        if self.pending.is_some() {
            return Err(DataError::Unavailable(
                "A notebook backup is already pending.".into(),
            ));
        }
        store.observe_external_read();
        Ok(self.read())
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        _: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "backupNotes" {
            return Err(DataError::UnknownSource(source.into()));
        }
        let pending = self
            .pending
            .take()
            .ok_or_else(|| DataError::Unavailable("No notebook backup is pending.".into()))?;
        let value = match storage::response(outcome) {
            Ok(value) => value,
            Err(message) => return Self::status(store, Err(message)),
        };
        match pending {
            Pending::Read => {
                let mut all = backup::Builder::default();
                let rows = match notes(&value) {
                    Ok(page) => page,
                    Err(message) => return Self::status(store, Err(message)),
                };
                for note in rows {
                    if let Err(message) = all.push(&note) {
                        return Self::status(store, Err(message));
                    }
                }
                let count = all.count();
                let text = all.finish();
                self.pending = Some(Pending::Directory { text, count });
                Ok(Answer::Later(storage::request(
                    "fs.mkdir",
                    json!({"path":DIRECTORY}),
                )))
            }
            Pending::Directory { text, count } => {
                let request =
                    storage::request("fs.atomicWriteFile", json!({"path":PATH,"text":text}));
                self.pending = Some(Pending::Write { text, count });
                Ok(Answer::Later(request))
            }
            Pending::Write { text, count } => Self::status(store, Ok((text, count))),
        }
    }
}
