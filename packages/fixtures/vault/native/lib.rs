//! @fixture/vault's shim on the data module's native slot (LLP 1154 D3). It
//! names `exact_js` and the standard library only: the host admits its
//! storage, so it reads no grants.
//!
//! `{op: "ping"}` answers `PING`, which nothing else knows. `{op: "open",
//! path}` asks the host for `path`, says whether the file was there before,
//! and holds it open as a module's own SQLite would, until `{op: "close"}`.

use exact_js::{json, Json, NativeModule, NativeStorage, SqliteFile};

/// What only this module answers.
const PING: &str = "vault 7f3a";

pub struct Vault {
    app: String,
    storage: Option<NativeStorage>,
    /// The open file, then its reservation: a tuple drops in order, so the
    /// file closes before the host may give it to another SQLite.
    open: Option<(std::fs::File, SqliteFile)>,
}

impl Vault {
    pub fn new(app: &str) -> Self {
        Self {
            app: app.to_owned(),
            storage: None,
            open: None,
        }
    }
}

impl NativeModule for Vault {
    fn configure_storage(&mut self, storage: NativeStorage) -> Result<(), String> {
        self.storage = Some(storage);
        Ok(())
    }

    fn call(&mut self, request: &Json) -> Result<Json, String> {
        match request["op"].as_str() {
            Some("ping") => Ok(json!({ "text": format!("{PING} for {}", self.app) })),
            Some("open") => {
                let path = request["path"].as_str().ok_or("open needs a path")?;
                let storage = self.storage.as_ref().ok_or("the host gave no storage")?;
                let file = storage.sqlite(path)?;
                let handle = std::fs::OpenOptions::new()
                    .append(true)
                    .open(file.path())
                    .map_err(|e| format!("{}: {e}", file.path().display()))?;
                let name = file
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned());
                // Whether there was a database to keep, or the host made the file new.
                let kept = if file.existed() { "kept" } else { "new" };
                self.open = Some((handle, file));
                Ok(json!({ "text": format!("opened {} ({kept})", name.unwrap_or_default()) }))
            }
            Some("close") => {
                self.open = None;
                Ok(json!({ "text": "closed" }))
            }
            _ => Err("vault answers ping, open and close".into()),
        }
    }
}
