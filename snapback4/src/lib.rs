#![forbid(unsafe_code)]
//! Snapback4's real native device, with the app's existing storage authority.
//!
//! An embedder constructs [`Module`] with its baked identity and grants, records
//! host directories using [`Module::configure_storage`], and calls it only after
//! first pixel. Construction and configuration do no I/O. The first `open` call
//! creates directories and opens SQLite; a validation candidate must receive no
//! storage configuration. Neither a JavaScript VM nor a transport is linked here.
//!
//! `open` takes `{op:"open", path:"app:/data/name.sqlite", origin, viewer,
//! backend?}`. It requires `sqlite.open` for that logical path. Backend is the
//! server's `/schema` JSON; omit it to reopen offline. The saved partition is
//! bound to the app, origin, and viewer, and another identity cannot adopt it.
//! A filename may contain ASCII letters, digits, `.`, `_`, `-`, and literal `%`;
//! percent sequences are never decoded by this adapter or Ibex's app paths.
//! Metadata `exact:device` is initialized once from OS entropy after opening;
//! clients combine it with a durable submission counter for offline write IDs.
//! `close` releases the current partition. All other requests and their
//! `{ok:...}` / `{denied:...}` envelopes are the device's Rust JSON dispatcher:
//! `state`, `sync_state`, `adopt`, `apply`, `observe_store`, `query`, `predict`,
//! `admit`, `settle`, `begin_send`, and the outbox/metadata operations. Queries,
//! predictions and admitted writes must name the opened viewer. Host configuration and authority
//! failures return `Err`; query/prediction refusals remain cards inside `ok`.
//! The embedder must keep one writer per partition path. `admit` atomically keeps
//! a write and its available prediction; lower-level outbox operations remain
//! separate. This adapter adds no database layer or process lock.
//!
//! The [`DataSource`] implementation exposes source `snapback4` with exactly one
//! JSON string argument and a JSON string answer. Its readiness is deferred until
//! `activate`; validation activation refuses storage even if configured. An app
//! may instead wrap the same inherent `call` in its TypeScript executor's native
//! hook. Shared application logic owns `/schema`, `/sync`, `/changes`, and
//! `/m/{operation}` through ordinary grant-checked fetch; this module owns the partition,
//! local named queries/predictions, watermark, and durable outbox only.

use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use ibex2::{
    boundary::admit,
    grant::{GrantSet, Operation},
    stdlib::app_fs::{resolve_sqlite, AppDirectories},
};
use serde_json::{json, Value as Json};
use snapback4_device::{ffi, native::Device};
use std::path::PathBuf;

const PARTITION: &str = "exact2:partition";

struct OpenPartition {
    device: Device,
    viewer: String,
    // Retain the directory capabilities for the lifetime of SQLite's connection.
    _directories: AppDirectories,
}

/// One native partition, independently owned by one application session.
pub struct Module {
    app_id: String,
    grant_text: String,
    grants: GrantSet,
    paths: Option<[PathBuf; 3]>,
    partition: Option<OpenPartition>,
    active: bool,
    validation: bool,
}

impl Module {
    /// Parse the baked authority without opening files or running app logic.
    pub fn new(app_id: &str, grants: &str) -> Result<Self, String> {
        if app_id.trim().is_empty() {
            return Err("Snapback4 requires the app's baked identity".into());
        }
        Ok(Self {
            app_id: app_id.into(),
            grant_text: grants.into(),
            grants: GrantSet::parse(grants)?,
            paths: None,
            partition: None,
            active: false,
            validation: false,
        })
    }

    /// Record absolute host-selected roots. No directory is created or opened.
    pub fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), String> {
        if self.partition.is_some() {
            return Err("close the Snapback4 partition before changing storage roots".into());
        }
        let paths = [data, cache, temporary];
        if paths.iter().any(|path| !path.is_absolute()) {
            return Err("app storage roots must be absolute".into());
        }
        self.paths = Some(paths);
        Ok(())
    }

    /// Dispatch after first pixel; the host is responsible for this ordering.
    /// Every call requires configured storage, including an offline reopen.
    pub fn call(&mut self, request: &Json) -> Result<Json, String> {
        if self.validation {
            return Err("Snapback4 storage is withheld during validation".into());
        }
        if self.paths.is_none() {
            return Err("Snapback4 app storage roots are not configured".into());
        }
        let op = text(request, "op")?;
        match op {
            "open" => self.open(request),
            "close" => {
                self.partition = None;
                Ok(json!({"ok": null}))
            }
            _ => {
                let partition = self
                    .partition
                    .as_mut()
                    .ok_or("Snapback4 partition is not open")?;
                if matches!(op, "query" | "predict") && text(request, "viewer")? != partition.viewer
                {
                    return Err("Snapback4 viewer differs from the opened partition".into());
                }
                if op == "set_meta" && text(request, "key")? == PARTITION {
                    return Err("Snapback4 partition identity is immutable".into());
                }
                if op == "admit" && request["entry"]["viewer"].as_str() != Some(&partition.viewer) {
                    return Err("Snapback4 viewer differs from the opened partition".into());
                }
                Ok(ffi::call(&mut partition.device, request.clone()))
            }
        }
    }

    fn open(&mut self, request: &Json) -> Result<Json, String> {
        if self.partition.is_some() {
            return Err("close the current Snapback4 partition before opening another".into());
        }
        let path = text(request, "path")?;
        path.strip_prefix("app:/data/")
            .filter(|leaf| {
                leaf.ends_with(".sqlite")
                    && leaf.len() > ".sqlite".len()
                    && !leaf.starts_with('.')
                    && leaf
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-%".contains(&b))
            })
            .ok_or("Snapback4 needs app:/data/<safe filename>.sqlite")?;
        // Admit before creating even the host-selected roots.
        admit(&self.grants, &Operation::SqliteOpen { path: path.into() })
            .map_err(|e| e.to_string())?;
        let origin = text(request, "origin")?;
        let viewer = text(request, "viewer")?;
        if origin.is_empty() || viewer.is_empty() {
            return Err("Snapback4 origin and viewer must be nonempty".into());
        }
        let identity =
            json!({"appId": self.app_id, "origin": origin, "viewer": viewer}).to_string();
        let backend = request
            .get("backend")
            .filter(|value| !value.is_null())
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| format!("Snapback4 backend: {e}"))?;
        let paths = self.paths.as_ref().ok_or("storage is not configured")?;
        for path in paths {
            std::fs::create_dir_all(path).map_err(|e| format!("app storage: {e}"))?;
        }
        let directories =
            AppDirectories::new(&paths[0], &paths[1], &paths[2]).map_err(|e| e.to_string())?;
        let physical =
            resolve_sqlite(&self.grants, Some(&directories), path).map_err(|e| e.to_string())?;
        let existed = physical.try_exists().map_err(|e| e.to_string())?;
        if !existed && backend.is_none() {
            return Err("this Snapback4 partition needs the server's backend once".into());
        }
        // Read the kept backend first. A mismatched opener must not replace its
        // schema/generation before we have verified the saved partition owner.
        let mut device = Device::open(&physical, if existed { None } else { backend.clone() })
            .map_err(|e| e.to_string())?;
        match device.meta(PARTITION).map_err(|e| e.to_string())? {
            Some(kept) if kept != identity => {
                return Err("Snapback4 partition belongs to another app, origin, or viewer".into());
            }
            None if existed => {
                return Err("Snapback4 database has no app partition identity".into());
            }
            None => device
                .set_meta(PARTITION, &identity)
                .map_err(|e| e.to_string())?,
            Some(_) => {}
        }
        if existed {
            if let Some(backend) = backend {
                device.adopt(backend).map_err(|e| e.to_string())?;
            }
        }
        if device
            .meta("exact:device")
            .map_err(|e| e.to_string())?
            .is_none()
        {
            let mut bytes = [0u8; 16];
            ibex2::stdlib::crypto::get_random_values(&mut bytes)
                .map_err(|e| format!("Snapback4 device entropy: {e}"))?;
            let identity: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            device
                .set_meta("exact:device", &identity)
                .map_err(|e| e.to_string())?;
        }
        self.partition = Some(OpenPartition {
            device,
            viewer: viewer.into(),
            _directories: directories,
        });
        Ok(json!({"ok": true}))
    }
}

fn text<'a>(request: &'a Json, key: &str) -> Result<&'a str, String> {
    request
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("Snapback4 {key} must be text"))
}

impl DataSource for Module {
    fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), DataError> {
        Module::configure_storage(self, data, cache, temporary).map_err(DataError::Unavailable)
    }

    fn activate(&mut self) -> Result<(), DataError> {
        self.active = true;
        Ok(())
    }

    fn activate_for_validation(&mut self) -> Result<(), DataError> {
        self.validation = true;
        self.active = true;
        Ok(())
    }

    fn ready(&self) -> bool {
        self.active
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source != "snapback4" {
            return Err(DataError::UnknownSource(source.into()));
        }
        if !self.active {
            return Err(DataError::Unavailable("Snapback4 is not activated".into()));
        }
        let [Value::Str(request)] = args else {
            return Err(DataError::BadArguments(
                "snapback4 takes one JSON request string".into(),
            ));
        };
        let request = serde_json::from_str(request)
            .map_err(|e| DataError::BadArguments(format!("Snapback4 request: {e}")))?;
        self.call(&request)
            .map(|answer| Value::str(&answer.to_string()))
            .map_err(DataError::Unavailable)
    }

    fn app_id(&self) -> &str {
        &self.app_id
    }

    fn grants(&self) -> &str {
        &self.grant_text
    }
}
