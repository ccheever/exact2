//! Snapback4's device and client as wasm, for an Exact app on the web.
//!
//! The same client as native (`exact-snapback4-client`), over the device's
//! memory store with the core's web choices (the page's NFC and Web Crypto).
//! The page persists the store: it hands back what it kept to the
//! constructor, and after each call takes the changes ([`WebSnapback::drain`])
//! to commit in one Exact SQLite transaction. `ts/web.ts` is the page's side;
//! `web/build.mjs` builds this with wasm-bindgen's web target.

use exact_snapback4_client::{dispatch, partition, Client, Core, NEEDS_BACKEND};
use serde_json::{json, Value as Json};
use snapback4_device::{
    device::Device,
    memory::{Change, MemoryStore},
    opening::Opening,
};
use wasm_bindgen::prelude::*;

struct Host {
    app_id: String,
    kept: Vec<Change>,
    device: Option<(Device<MemoryStore>, String)>,
}

impl Core for Host {
    fn call(&mut self, request: Json) -> Result<Json, String> {
        match request["op"].as_str().unwrap_or_default() {
            "open" => self.open(&request),
            "close" => {
                self.device = None;
                Ok(json!({"ok": null}))
            }
            _ => {
                let (device, viewer) = self
                    .device
                    .as_mut()
                    .ok_or("Snapback4 partition is not open")?;
                partition::guard(&request, viewer)?;
                Ok(snapback4_device::call::call(device, request))
            }
        }
    }
}

impl Host {
    fn open(&mut self, request: &Json) -> Result<Json, String> {
        if self.device.is_some() {
            return Err("close the current Snapback4 partition before opening another".into());
        }
        let text = |key: &str| {
            request[key]
                .as_str()
                .filter(|t| !t.is_empty())
                .ok_or_else(|| format!("Snapback4 {key} must be nonempty text"))
        };
        if !partition::valid_path(text("path")?) {
            return Err("Snapback4 needs app:/data/<safe filename>.sqlite".into());
        }
        let (origin, viewer) = (text("origin")?, text("viewer")?);
        let backend = request
            .get("backend")
            .filter(|b| !b.is_null())
            .map(Json::to_string);
        let opening =
            Opening::parse(backend.as_deref()).map_err(|e| format!("Snapback4 backend: {e}"))?;
        let existed = !self.kept.is_empty();
        if !existed && matches!(opening, Opening::Kept) {
            return Err(NEEDS_BACKEND.into());
        }
        let mut store = MemoryStore::new();
        store.load(self.kept.drain(..)).map_err(|e| e.to_string())?;
        // A kept partition opens on its own backend; adopting another is the round's.
        let opening = if existed { Opening::Kept } else { opening };
        let mut device = opening.open(store).map_err(|e| e.to_string())?;
        partition::bind(
            &mut device,
            &partition::identity(&self.app_id, origin, viewer),
            existed,
            || {
                let mut bytes = [0u8; 16];
                getrandom_fill(&mut bytes)?;
                Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
            },
        )?;
        self.device = Some((device, viewer.into()));
        Ok(json!({"ok": true}))
    }
}

fn getrandom_fill(bytes: &mut [u8]) -> Result<(), String> {
    let crypto = js_sys::Reflect::get(&js_sys::global(), &"crypto".into())
        .map_err(|_| "Snapback4: no crypto")?;
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    let get = js_sys::Reflect::get(&crypto, &"getRandomValues".into())
        .map_err(|_| "Snapback4: no getRandomValues")?;
    js_sys::Function::from(get)
        .call1(&crypto, &array)
        .map_err(|_| "Snapback4: getRandomValues failed")?;
    array.copy_to(bytes);
    Ok(())
}

/// One partition's device and client in the page.
#[wasm_bindgen]
pub struct WebSnapback {
    host: Host,
    client: Option<Client>,
}

#[wasm_bindgen]
impl WebSnapback {
    /// A session over what the page kept: `kept` is the JSON array of
    /// `{s, k, v}` changes it committed, in any order (keys are unique).
    #[wasm_bindgen(constructor)]
    pub fn new(app: &str, kept: &str) -> Result<WebSnapback, JsError> {
        if app.trim().is_empty() {
            return Err(JsError::new("Snapback4 requires the app's identity"));
        }
        let kept: Vec<Change> = if kept.is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(kept)
                .map_err(|e| JsError::new(&format!("Snapback4 kept changes: {e}")))?
        };
        Ok(WebSnapback {
            host: Host {
                app_id: app.into(),
                kept,
                device: None,
            },
            client: None,
        })
    }

    /// One JSON call (the dispatch `ts/snapback.ts` speaks): `{ok}`,
    /// `{denied}` or `{error}`.
    pub fn call(&mut self, request: &str) -> String {
        match serde_json::from_str::<Json>(request) {
            Err(error) => json!({"error": format!("Snapback4 request: {error}")}),
            Ok(request) => dispatch(&mut self.client, &mut self.host, &request)
                .unwrap_or_else(|error| json!({"error": error})),
        }
        .to_string()
    }

    /// The changes since the last drain, a JSON array of `{s, k, v}`, for
    /// the page to commit in one transaction before it calls again.
    pub fn drain(&mut self) -> String {
        let Some((device, _)) = self.host.device.as_ref() else {
            return "[]".into();
        };
        let mut text = String::from("[");
        loop {
            let chunk = device.store().drain_chunk();
            if chunk == "[]" {
                break;
            }
            if text.len() > 1 {
                text.push(',');
            }
            text.push_str(&chunk[1..chunk.len() - 1]);
        }
        text.push(']');
        text
    }
}
