#![forbid(unsafe_code)]
//! Snapback4's native device for Exact apps, with the app's storage authority,
//! and the client protocol over it (`exact-snapback4-client`).
//!
//! An embedder constructs [`Module`] with its baked identity and grants, records
//! host directories using [`Module::configure_storage`], and calls it only after
//! first pixel. Construction and configuration do no I/O; the first `open`
//! creates directories and opens SQLite, and requires `sqlite.open` for its
//! path. A validation candidate must receive no storage configuration. Neither
//! a JavaScript VM nor a transport is linked here: the client hands each HTTP
//! exchange to its driver (`ts/snapback.ts` over `fetch`, or a Rust source's
//! `Answer::Later`). The JSON call is [`exact_snapback4_client::dispatch`]'s.
//! The embedder must keep one writer per partition path.

pub use exact_snapback4_client::{
    self as client, Client, Config, Core, Fetch, Reply, Step, NEEDS_BACKEND,
};

use exact_plan::Value;
use exact_runner::{DataError, DataSource};
use exact_snapback4_client::partition;
use ibex2::{
    boundary::{admit, HostError},
    grant::{GrantSet, Operation},
    stdlib::app_fs::{resolve_sqlite, AppDirectories},
};
use serde_json::{json, Value as Json};
use snapback4_device::{ffi, native::Device, opening::Opening};
use std::path::PathBuf;

struct OpenPartition {
    device: Device,
    viewer: String,
    // Retain the directory capabilities for the lifetime of SQLite's connection.
    _directories: AppDirectories,
}

/// The device under the app's authority: grants, directories, one partition.
pub struct Host {
    app_id: String,
    grants: GrantSet,
    paths: Option<[PathBuf; 3]>,
    partition: Option<OpenPartition>,
}

/// One native partition and its client, owned by one application session.
pub struct Module {
    host: Host,
    client: Option<Client>,
    grant_text: String,
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
            host: Host {
                app_id: app_id.into(),
                grants: GrantSet::parse(grants)?,
                paths: None,
                partition: None,
            },
            client: None,
            grant_text: grants.into(),
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
        if self.host.partition.is_some() {
            return Err("close the Snapback4 partition before changing storage roots".into());
        }
        let paths = [data, cache, temporary];
        if paths.iter().any(|path| !path.is_absolute()) {
            return Err("app storage roots must be absolute".into());
        }
        self.host.paths = Some(paths);
        Ok(())
    }

    /// The client and the device it drives, for a Rust caller.
    pub fn parts(&mut self) -> (Option<&mut Client>, &mut Host) {
        (self.client.as_mut(), &mut self.host)
    }

    /// Dispatch after first pixel; the host is responsible for this ordering.
    /// Every call requires configured storage, including an offline reopen.
    pub fn call(&mut self, request: &Json) -> Result<Json, String> {
        if self.validation {
            return Err("Snapback4 storage is withheld during validation".into());
        }
        if self.host.paths.is_none() {
            return Err("Snapback4 app storage roots are not configured".into());
        }
        exact_snapback4_client::dispatch(&mut self.client, &mut self.host, request)
    }
}

impl Core for Host {
    fn call(&mut self, request: Json) -> Result<Json, String> {
        match text(&request, "op")? {
            "open" => self.open(&request),
            "close" => {
                self.partition = None;
                Ok(json!({"ok": null}))
            }
            _ => {
                let partition = self
                    .partition
                    .as_mut()
                    .ok_or("Snapback4 partition is not open")?;
                partition::guard(&request, &partition.viewer)?;
                Ok(ffi::call(&mut partition.device, request))
            }
        }
    }
}

impl Host {
    fn open(&mut self, request: &Json) -> Result<Json, String> {
        if self.partition.is_some() {
            return Err("close the current Snapback4 partition before opening another".into());
        }
        let path = text(request, "path")?;
        if !partition::valid_path(path) {
            return Err("Snapback4 needs app:/data/<safe filename>.sqlite".into());
        }
        // Admit before creating even the host-selected roots.
        admit(&self.grants, &Operation::SqliteOpen { path: path.into() }).map_err(|e| match e {
            // Name the file and the lines that would admit it, as the web's storage does.
            HostError::Denied { .. } => {
                let dir = path.rsplit_once('/').map_or(path, |(dir, _)| dir);
                format!("{e} {path}: no grant covers it; grant `sqlite.open {path}`, or `sqlite.open {dir}` for every file there (a grant covers its path and what is below it, by whole names)")
            }
            e => e.to_string(),
        })?;
        let origin = text(request, "origin")?;
        let viewer = text(request, "viewer")?;
        if origin.is_empty() || viewer.is_empty() {
            return Err("Snapback4 origin and viewer must be nonempty".into());
        }
        let identity = partition::identity(&self.app_id, origin, viewer);
        let backend = request
            .get("backend")
            .filter(|value| !value.is_null())
            .map(Json::to_string);
        let opening =
            Opening::parse(backend.as_deref()).map_err(|e| format!("Snapback4 backend: {e}"))?;
        let paths = self.paths.as_ref().ok_or("storage is not configured")?;
        for path in paths {
            std::fs::create_dir_all(path).map_err(|e| format!("app storage: {e}"))?;
        }
        let directories =
            AppDirectories::new(&paths[0], &paths[1], &paths[2]).map_err(|e| e.to_string())?;
        let physical =
            resolve_sqlite(&self.grants, Some(&directories), path).map_err(|e| e.to_string())?;
        let existed = physical.try_exists().map_err(|e| e.to_string())?;
        if !existed && matches!(opening, Opening::Kept) {
            return Err(NEEDS_BACKEND.into());
        }
        // A kept partition opens on its own backend, so a mismatched opener
        // changes nothing before the owner check below. Adopting a newer
        // backend is the sync round's, after it observes the store.
        let opening = if existed { Opening::Kept } else { opening };
        let mut device = Device::open_with(&physical, opening).map_err(|e| e.to_string())?;
        partition::bind(&mut device, &identity, existed, || {
            let mut bytes = [0u8; 16];
            ibex2::stdlib::crypto::get_random_values(&mut bytes)
                .map_err(|e| format!("Snapback4 device entropy: {e}"))?;
            Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
        })?;
        self.partition = Some(OpenPartition {
            device,
            viewer: viewer.into(),
            _directories: directories,
        });
        Ok(json!({"ok": true}))
    }
}

/// A round's exchange as a Rust source hands it to the host
/// (`Answer::Later`): at `origin`, with the source's credentials
/// (`authorization: Bearer …`, or a development persona).
pub fn host_request(
    fetch: &Fetch,
    origin: &str,
    headers: &[(String, String)],
) -> exact_runner::Request {
    let url = format!("{}{}", origin.trim_end_matches('/'), fetch.path);
    let mut request = match &fetch.body {
        Some(body) => exact_runner::Request::post_json(&url, &body.to_string()),
        None => exact_runner::Request::get(&url),
    };
    request.method = fetch.method.into();
    request.headers.extend(headers.iter().cloned());
    request
}

/// What the host brought back, for [`Client::deliver`] or [`Client::changed`].
pub fn host_reply(outcome: &exact_runner::Outcome) -> Reply {
    match outcome {
        exact_runner::Outcome::Response(response) => Reply::Http {
            status: response.status,
            body: serde_json::from_slice(&response.body).unwrap_or(Json::Null),
        },
        exact_runner::Outcome::Failed { message, .. } => Reply::Failed(message.clone()),
        _ => Reply::Failed("Snapback4 expected an HTTP response".into()),
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
        let [request @ exact_plan::str_value!()] = args else {
            return Err(DataError::BadArguments(
                "snapback4 takes one JSON request string".into(),
            ));
        };
        let request = serde_json::from_str(request.text())
            .map_err(|e| DataError::BadArguments(format!("Snapback4 request: {e}")))?;
        self.call(&request)
            .map(|answer| Value::str(&answer.to_string()))
            .map_err(DataError::Unavailable)
    }

    fn app_id(&self) -> &str {
        &self.host.app_id
    }

    fn grants(&self) -> &str {
        &self.grant_text
    }
}
