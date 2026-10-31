//! @fixture/vault's shim on the data module's native slot (LLP 1154 D3). It
//! names `exact_js` and the standard library only, and calls the package's
//! prebuilt library (`library/`) through its C ABI: the host admits its
//! storage, so it reads no grants.
//!
//! `{op: "answer"}` answers the library's number, which nothing else knows.
//! `{op: "open", path}` asks the host for `path`, says whether the file was
//! there before, and has the library open it with its own SQLite, write a
//! row and hold it, until `{op: "close"}` or the module drops.

use exact_js::{json, Json, NativeModule, NativeStorage, SqliteFile};
use std::ffi::{c_char, CString};

#[link(name = "vault")]
extern "C" {
    fn vault_answer() -> i32;
    fn vault_open(path: *const c_char) -> i32;
    fn vault_close();
}

pub struct Vault {
    app: String,
    storage: Option<NativeStorage>,
    /// The reservation of the file the library holds open.
    open: Option<SqliteFile>,
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

/// The host drops the module on unload and reload, maybe with the vault
/// open: the library closes it here, before the fields, and so the
/// reservation, drop.
impl Drop for Vault {
    fn drop(&mut self) {
        if self.open.is_some() {
            unsafe { vault_close() };
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
            Some("answer") => {
                let answer = unsafe { vault_answer() };
                Ok(json!({ "text": format!("{answer} for {}", self.app) }))
            }
            Some("open") => {
                if self.open.is_some() {
                    return Err("the vault is open: close it first".into());
                }
                let path = request["path"].as_str().ok_or("open needs a path")?;
                let storage = self.storage.as_ref().ok_or("the host gave no storage")?;
                let file = storage.sqlite(path)?;
                let real = file.path().to_str().ok_or("the path is not UTF-8")?;
                let real = CString::new(real).map_err(|e| e.to_string())?;
                let rows = unsafe { vault_open(real.as_ptr()) };
                if rows < 0 {
                    return Err(format!("the library could not open {path}"));
                }
                let name = (file.path().file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // Whether there was a database to keep, or the host made the file new.
                let kept = if file.existed() { "kept" } else { "new" };
                self.open = Some(file);
                Ok(json!({ "text": format!("opened {name} ({kept}): {rows} rows") }))
            }
            Some("close") => {
                // The library closes the file before the reservation ends.
                unsafe { vault_close() };
                self.open = None;
                Ok(json!({ "text": "closed" }))
            }
            _ => Err("vault answers answer, open and close".into()),
        }
    }
}
