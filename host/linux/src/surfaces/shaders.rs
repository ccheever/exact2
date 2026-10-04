//! Primary runtime shader packs, admitted together before any source changes.
//! Device-free ownership deliberately never reads or registers shader sources.
use super::{Abi, Surfaces, LIMIT};
use crate::image::Assets;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, sync::Arc};

pub(super) type Pack = BTreeMap<String, Arc<[u8]>>;
type Shader = unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> u32;

fn read_pack(compat: &Value, assets: &Assets) -> Result<Pack, String> {
    let mut pack = Pack::new();
    let mut remaining = LIMIT;
    for row in compat["inputs"]["gpuSurfaces"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let name = row["name"]
            .as_str()
            .ok_or("shader roster name is not text")?;
        if name.is_empty()
            || !name
                .bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
            || pack.contains_key(name)
        {
            return Err(format!("shader `{name}`: invalid or duplicate roster name"));
        }
        let asset = format!("shaders/{name}.wgsl");
        let refuse = |reason: String| format!("shader `{name}` ({asset}): {reason}");
        let bytes = if assets.is_selected() {
            // The selected resolver owns its signed roster and integrity. Never
            // fall back to entry zero, even on a tombstone or integrity error.
            assets.read_asset(&asset).map_err(&refuse)?
        } else if let Some(path) = assets.path(&asset) {
            let file = std::fs::File::open(path).map_err(|e| refuse(e.to_string()))?;
            if file.metadata().map_err(|e| refuse(e.to_string()))?.len() > remaining as u64 {
                return Err(refuse("shader pack exceeds 256 MiB".into()));
            }
            // Bound the actual read too, including a file that grows after stat.
            let mut bytes = Vec::new();
            file.take(remaining as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| refuse(e.to_string()))?;
            Some(Arc::from(bytes))
        } else {
            None
        }
        .ok_or_else(|| refuse("missing source".into()))?;
        if bytes.len() > remaining {
            return Err(refuse("shader pack exceeds 256 MiB".into()));
        }
        remaining -= bytes.len();
        std::str::from_utf8(&bytes).map_err(|e| refuse(format!("invalid UTF-8: {e}")))?;
        if !assets.is_selected() {
            let cards: Vec<_> = compat["embedded"]["assets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|card| card["name"] == asset)
                .collect();
            if cards.len() != 1
                || cards[0]["bytes"].as_u64() != Some(bytes.len() as u64)
                || cards[0]["sha256"].as_str()
                    != Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
            {
                return Err(refuse("baked size or digest mismatch".into()));
            }
        }
        pack.insert(name.into(), bytes);
    }
    Ok(pack)
}

impl Abi {
    pub(super) fn prepare_shaders(
        &self,
        compat: &Value,
        assets: &Assets,
    ) -> Result<Option<Pack>, String> {
        if !self.rendered {
            return Ok(None);
        }
        let pack = read_pack(compat, assets)?;
        // Read integrity first: a cached good namespace must not hide a corrupt
        // selected generation. Ordinary surface binds never enter this path.
        if pack == self.shaders {
            return Ok(None);
        }
        for name in ["gpu_shader_validate", "gpu_shaders_clear", "gpu_shader"] {
            unsafe { self.library.get::<*const ()>(name.as_bytes()) }
                .map_err(|e| format!("GPU ABI {name}: {e}"))?;
        }
        for (name, bytes) in &pack {
            if unsafe {
                self.symbol::<Shader>(b"gpu_shader_validate")(
                    name.as_ptr(),
                    name.len(),
                    bytes.as_ptr(),
                    bytes.len(),
                )
            } != 0
            {
                return Err(format!(
                    "shader `{name}`: {}",
                    self.error().unwrap_or("validation refused".into())
                ));
            }
        }
        Ok(Some(pack))
    }

    pub(super) fn commit_shaders(&mut self, pack: Option<Pack>) -> Result<(), String> {
        let Some(pack) = pack else {
            return Ok(());
        };
        self.replace_shaders(&pack)?;
        self.shaders = pack;
        Ok(())
    }

    fn replace_shaders(&self, pack: &Pack) -> Result<(), String> {
        unsafe { self.symbol::<unsafe extern "C" fn()>(b"gpu_shaders_clear")() };
        for (name, bytes) in pack {
            if unsafe {
                self.symbol::<Shader>(b"gpu_shader")(
                    name.as_ptr(),
                    name.len(),
                    bytes.as_ptr(),
                    bytes.len(),
                )
            } != 0
            {
                // An ABI refusal after successful validation is exceptional,
                // but must not leave a partial namespace advertised as ready.
                let error = self.error().unwrap_or("registration refused".into());
                unsafe { self.symbol::<unsafe extern "C" fn()>(b"gpu_shaders_clear")() };
                for (old_name, old_bytes) in &self.shaders {
                    if unsafe {
                        self.symbol::<Shader>(b"gpu_shader")(
                            old_name.as_ptr(),
                            old_name.len(),
                            old_bytes.as_ptr(),
                            old_bytes.len(),
                        )
                    } != 0
                    {
                        return Err(format!(
                            "shader `{name}`: {error}; restoring prior namespace failed: {}",
                            self.error().unwrap_or_default()
                        ));
                    }
                }
                return Err(format!("shader `{name}`: {error}"));
            }
        }
        Ok(())
    }
}

impl Surfaces {
    pub(crate) fn prepare_shaders(
        &self,
        compat: &str,
        assets: &Assets,
    ) -> Result<Option<Pack>, String> {
        let Some(abi) = self.abis.get("") else {
            return Ok(None);
        };
        let compat =
            serde_json::from_str(compat).map_err(|e| format!("shader compatibility: {e}"))?;
        abi.prepare_shaders(&compat, assets)
    }

    /// Shader registration and selected-store acceptance are one transaction.
    /// No host/assets swap follows unless both succeed.
    pub(crate) fn activate_shaders(
        &mut self,
        pack: Option<Pack>,
        accept: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let previous = pack
            .as_ref()
            .and_then(|_| self.abis.get("").map(|abi| abi.shaders.clone()));
        self.commit_shaders(pack)?;
        if let Err(error) = accept() {
            self.commit_shaders(previous)
                .map_err(|rollback| format!("{error}; restoring shaders failed: {rollback}"))?;
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn commit_shaders(&mut self, pack: Option<Pack>) -> Result<(), String> {
        if let Some(abi) = self.abis.get_mut("") {
            abi.commit_shaders(pack)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
