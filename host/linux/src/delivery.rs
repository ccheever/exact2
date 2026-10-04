//! Optional app-supplied delivery composition (LLP 1030 D4).
//! The host owns sessions and assets; the higher adapter owns store policy.
use crate::image::AssetResolver;
use exact_runner::Delivery;
use std::sync::Arc;

/// A pinned plan with its complete asset resolver.
pub struct Selection {
    /// The selected entry, absent for the embedded plan.
    pub entry: Option<String>,
    /// The sequence belonging to this exact entry.
    pub seq: u64,
    /// Validated plan bytes.
    pub plan: Arc<[u8]>,
    /// Complete selected assets; missing names never fall through to embedded files.
    pub assets: AssetResolver,
}

/// Operations the presenter needs from a linked delivery adapter.
/// A binary-only app supplies none and creates no store or check thread.
pub trait Store {
    /// Pin a candidate and its asset resolver for this boot.
    fn prepare_selected(&mut self) -> Option<Selection>;
    /// Count a selected boot.
    fn boot_started(&mut self);
    /// Refuse the selected plan.
    fn entry_refused(&mut self, entry: &str, why: &str);
    /// Refuse assets belonging to the last prepared selection.
    fn selection_corrupt(&mut self, why: &str);
    /// Mark the selected boot successful.
    fn boot_succeeded(&mut self);
    /// Take a boot diagnostic.
    fn take_note(&mut self) -> Option<String>;
    /// The completion wake in the display's poll set.
    #[cfg(unix)]
    fn fd(&self) -> std::os::unix::io::RawFd;
    /// Start a check, returning whether one started.
    fn check(&self) -> bool;
    /// Take the latest asynchronous outcome.
    fn take_line(&mut self) -> Option<String>;
    /// Prepare a complete staged generation without changing live state.
    fn prepare_activation(&self) -> Result<Option<Selection>, String>;
    /// Commit the exact entry and sequence accepted by the host.
    fn commit_activation(&mut self, entry: Option<String>, seq: u64) -> Result<(), String>;
    /// Candidate stream facts, before the live store changes.
    fn staged_stream_into(&self, delivery: &mut Delivery);
    /// Copy the latest delivery facts into the runner.
    fn status_into(&self, delivery: &mut Delivery);
}

/// One inseparable Rust replacement, already authenticated by the asset resolver.
#[derive(Clone, Debug)]
pub struct Module {
    /// UTF-8 pairing receipt.
    pub receipt: String,
    /// Opaque wasm or native library bytes; never executed during preparation.
    pub bytes: Vec<u8>,
}

impl Module {
    /// Read both signed assets before admitting either into a data source.
    pub fn resolve(assets: &AssetResolver) -> Result<Option<Self>, String> {
        Self::resolve_at(assets, "rust")
    }

    /// Read the producer's selected local development executor directory.
    pub fn local(plan: &std::path::Path, compat: &str) -> Result<Option<Self>, String> {
        let root = plan.parent().unwrap_or_else(|| std::path::Path::new("."));
        let rust = root.join("rust");
        if !rust.exists() {
            return Ok(None);
        }
        let json: serde_json::Value = serde_json::from_str(compat).map_err(|e| e.to_string())?;
        let mode = json["inputs"]["rustMode"].as_str().unwrap_or("off");
        let prefix = match mode {
            "wasm" => "rust/wasm".to_string(),
            "native" | "tiered" => format!("rust/{mode}/{}", json["target"].as_str().unwrap_or("")),
            _ => return Err("Rust replacement is disabled in this binary".into()),
        };
        let root = root.to_path_buf();
        let read: AssetResolver = Arc::new(move |name| {
            let path = root.join(name);
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.len() > 32 << 20 => {
                    return Err("Rust replacement exceeds 32 MiB".into())
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.to_string()),
                _ => {}
            }
            match std::fs::read(path) {
                Ok(bytes) if bytes.len() <= 32 << 20 => Ok(Some(bytes.into())),
                Ok(_) => Err("Rust replacement exceeds 32 MiB".into()),
                Err(e) => Err(e.to_string()),
            }
        });
        Self::resolve_at(&read, &prefix)?.map_or_else(
            || Err("no Rust replacement matching this binary's executor and target".into()),
            |module| Ok(Some(module)),
        )
    }

    fn resolve_at(assets: &AssetResolver, prefix: &str) -> Result<Option<Self>, String> {
        let Some(receipt) = assets(&format!("{prefix}/app.module.json"))? else {
            for file in [
                "app.module.wasm",
                "app.module.dylib",
                "app.module.so",
                "app.module.dll",
                "app.module.bin",
            ] {
                if assets(&format!("{prefix}/{file}"))?.is_some() {
                    return Err("Rust replacement is missing its pairing receipt".into());
                }
            }
            return Ok(None);
        };
        if receipt.len() > 1 << 20 {
            return Err("Rust receipt exceeds 1 MiB".into());
        }
        let receipt = String::from_utf8(receipt.to_vec()).map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&receipt).map_err(|e| e.to_string())?;
        let file = json["module"]["file"].as_str().unwrap_or("");
        if ![
            "app.module.wasm",
            "app.module.dylib",
            "app.module.so",
            "app.module.dll",
            "app.module.bin",
        ]
        .contains(&file)
        {
            return Err("invalid Rust module filename".into());
        }
        let bytes =
            assets(&format!("{prefix}/{file}"))?.ok_or("Rust replacement is missing its module")?;
        if bytes.len() > 32 << 20 {
            return Err("Rust replacement exceeds 32 MiB".into());
        }
        Ok(Some(Self {
            receipt,
            bytes: bytes.to_vec(),
        }))
    }

    /// Pair against the candidate plan and this binary's baked authority.
    pub fn replacement<D: exact_runner::DataSource>(
        &self,
        plan: &[u8],
        admitted: &D,
    ) -> Result<D, String> {
        admitted
            .replacement(plan, &self.receipt, self.bytes.clone())
            .map_err(|e| format!("Rust replacement: {e:?}"))
    }
}
