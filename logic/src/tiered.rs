//! One admitted generation: Wasm first, native promotion between data calls.
//! @ref LLP 1029.000 §4 — only explicitly stateless, portable modules qualify.
use crate::{native, wasm, Executor, MAX_MODULE};
use exact_logic_abi as abi;

// The outer receipt authenticates this complete container, including its native
// bytes and exact target. No unauthenticated companion is fetched later.
const MAGIC: &[u8; 8] = b"EXLT\x01\0\0\0";

struct Tiered {
    active: Box<dyn Executor>,
    native: Option<Vec<u8>>,
    metadata: Vec<u8>,
    binding: Option<Vec<u8>>,
    activated: bool,
}

pub(crate) fn load(bytes: &[u8]) -> Result<Box<dyn Executor>, String> {
    if bytes.len() > MAX_MODULE || bytes.len() < 13 || &bytes[..8] != MAGIC {
        return Err("invalid tiered Rust container".into());
    }
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if length == 0 || length >= bytes.len() - 12 {
        return Err("invalid tiered Rust module boundary".into());
    }
    let mut active = wasm::load(&bytes[12..12 + length])?;
    if !active.stateless() {
        return Err("tiered Rust requires export!(Data, constructor, stateless); use native or wasm for modules with private state".into());
    }
    let metadata = active.call(&abi::metadata_request())?;
    abi::metadata_reply(&metadata)?;
    // A disposable validation instance may reach this too. Loading an image
    // is shared, but app instances and business calls remain session-owned.
    let mut result = Tiered {
        active,
        native: Some(bytes[12 + length..].to_vec()),
        metadata,
        binding: None,
        activated: false,
    };
    result.prepare();
    eprintln!("exact rust: tiered executor ready (wasm)");
    Ok(Box::new(result))
}

impl Tiered {
    fn prepare(&mut self) -> bool {
        let Some(bytes) = &self.native else {
            return false;
        };
        match native::preload_if_idle(bytes) {
            Ok(ready) => ready,
            Err(error) => {
                self.refuse(error);
                false
            }
        }
    }

    fn refuse(&mut self, error: String) {
        self.native = None;
        eprintln!("exact rust: native promotion unavailable; continuing wasm: {error}");
    }

    fn promote(&mut self) {
        if !self.activated || !self.prepare() {
            return;
        }
        let prepare = || -> Result<Box<dyn Executor>, String> {
            let mut candidate = native::load(self.native.as_ref().unwrap())?;
            if !candidate.stateless() {
                return Err(
                    "native variant does not declare the stateless promotion contract".into(),
                );
            }
            if candidate.call(&abi::metadata_request())? != self.metadata {
                return Err("native variant changes the Wasm app identity or grants".into());
            }
            if let Some(binding) = &self.binding {
                abi::unit_reply(&candidate.call(binding)?)?;
            }
            // The explicit stateless contract requires effect-free local
            // initialization. Never replay answer/parse or app lifecycle events.
            abi::unit_reply(&candidate.call(&abi::activate_request())?)?;
            Ok(candidate)
        };
        match prepare() {
            Ok(candidate) => {
                self.active = candidate;
                self.native = None;
                self.binding = None;
                eprintln!("exact rust: tiered executor promoted (native)");
            }
            Err(error) => self.refuse(error),
        }
    }
}

impl Executor for Tiered {
    fn stateless(&self) -> bool {
        true
    }

    fn call(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        // Calls are serialized by the owning session. Host-owned requests keep
        // their tickets and explicit parse arguments; no request is reissued.
        if matches!(bytes.get(4), Some(3 | 4)) {
            self.promote();
        }
        let reply = self.active.call(bytes)?;
        if bytes.get(4) == Some(&1) && abi::unit_reply(&reply).is_ok() {
            self.binding = Some(bytes.to_vec());
        } else if bytes.get(4) == Some(&2) && abi::unit_reply(&reply).is_ok() {
            self.activated = true;
        }
        Ok(reply)
    }
}
