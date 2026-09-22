//! Ahead-of-time compilation, and the cache that makes it invisible.
//!
//! `rules/RULES.md` forbids compiling at runtime anything that could be built
//! ahead of time, and `NOT-DOING.md` names it outright: *modules ship as
//! bytecode*. LLP 0063 measures what that is worth — roughly 2ms of fixed cost
//! per module becomes about 9 microseconds — but the rule came first and the
//! measurement only says how much it costs to keep ignoring it.
//!
//! **The artifact is the wrapper, not the source.** Whatever text is compiled
//! here is what runs, so the wrapper's shape is part of the cache key: change
//! the parameter list and every artifact is stale, correctly.
//!
//! @ref LLP 0063#5-bytecode — the measurement this implements
//! @ref LLP 0058.000.000#6-module-binding-globals-and-bootstrap — the module surface this builds artifacts for

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Bumped when anything about how sources become artifacts changes in a way
/// the inputs below do not already capture. Changing it invalidates every
/// cached artifact, which is the point.
const ARTIFACT_VERSION: u32 = 3;

/// Compiles module wrappers to Hermes bytecode, caching by content.
#[derive(Debug, Clone)]
pub struct Compiler {
    /// Absent on the shipping path: a `--precompiled` run never compiles, so
    /// it never looks for a compiler, and an artifact missing from the
    /// manifest is refused rather than built.
    hermesc: Option<PathBuf>,
    cache_dir: PathBuf,
    /// Identity of the compiler binary, folded into every key.
    ///
    /// Bytecode is version-coupled to the engine that runs it: a mismatched
    /// Hermes refuses the artifact outright. Binding the cache to the exact
    /// compiler that produced it makes an engine change a cache miss rather
    /// than a runtime failure.
    toolchain: String,
    /// The engine these artifacts are for, from its HermesInputReceipt.
    ///
    /// LLP 0058.000.001 §5: a ModuleReceipt's digest domain includes the
    /// HermesInputReceipt digest. Folding it into the key is the enforcement —
    /// bytecode built against one engine cannot be found, let alone loaded,
    /// under another.
    engine: Option<String>,
    /// The engine this binary links (`linked_engine`), as a field so a test
    /// can build two compilers that differ in it and compare their keys.
    linked: String,
}

impl Compiler {
    /// This compiler with a different linked-engine digest. Tests only.
    #[cfg(test)]
    pub(crate) fn with_linked(mut self, digest: &str) -> Self {
        self.linked = digest.to_string();
        self
    }

    /// Find `hermesc` beside the vanilla engine, or wherever
    /// `IBEX2_HERMESC` points, binding artifacts to the engine's receipt when
    /// one is installed.
    ///
    /// `require_receipt` is the shipping posture. Without it a missing receipt
    /// is tolerated — a machine can compile before its engine has been
    /// receipted — but the absence is folded into the artifact key, so
    /// artifacts built without a receipt never masquerade as artifacts built
    /// with one.
    ///
    /// **With it, a missing receipt is refused.** An unreceipted engine is
    /// indistinguishable from a patched one to a reader that only checks
    /// receipts it can find, and "no evidence" is not evidence.
    pub fn discover_for_engine(
        repo_root: &Path,
        cache_dir: PathBuf,
        engine_dir: &Path,
        require_receipt: bool,
    ) -> Result<Self, String> {
        let receipt = crate::receipt::HermesInput::read(engine_dir).ok();
        if require_receipt && receipt.is_none() {
            return Err(format!(
                "the engine at {} has no HermesInputReceipt, so nothing attests it is unpatched\n\
                 produce one with: node scripts/hermes-input-receipt.mjs {}",
                engine_dir.display(),
                engine_dir.display()
            ));
        }
        if let Some(receipt) = &receipt {
            if !receipt.is_vanilla() {
                return Err(format!(
                    "the engine at {} is not vanilla: its receipt records {} applied patches",
                    engine_dir.display(),
                    receipt.patches_applied
                ));
            }
            // Hashes the engine and compiler on disk against the receipt. This
            // is the BUILD posture and it is the only place that hashes
            // anything: a run does not, because it does not run these files —
            // it runs the engine linked into it, whose digest was baked in at
            // link time (`linked_engine`).
            receipt.verify_binary(engine_dir)?;
        }
        let hermesc = Self::find_hermesc(repo_root)?;
        if let Some(expected) = receipt.as_ref().and_then(|r| r.compiler_digest.as_deref()) {
            let actual = format!(
                "sha256-{}",
                hex(&Sha256::digest(std::fs::read(&hermesc).map_err(
                    |e| format!("cannot read {}: {e}", hermesc.display()),
                )?))
            );
            if actual != expected {
                return Err(format!(
                    "the receipt describes a different hermesc than the one present\n  \
                     receipt: {expected}\n  actual:  {actual}"
                ));
            }
        }
        Self::with_engine(hermesc, cache_dir, receipt)
    }

    /// The compiler for a run: the receipt's digests, read and never
    /// re-hashed, and a compiler only if one may be needed.
    ///
    /// A `--precompiled` run touches the receipt (2 KB) and the manifest and
    /// nothing else before the floor. The previous design hashed the framework
    /// dylib and `hermesc` at every start — 25 ms of a 30 ms budget — to verify
    /// files the runtime does not run; the engine it runs is linked in, and
    /// `linked_engine` is its identity.
    pub fn for_run(
        repo_root: &Path,
        cache_dir: PathBuf,
        engine_dir: &Path,
        precompiled_only: bool,
    ) -> Result<Self, String> {
        let receipt = crate::receipt::HermesInput::read(engine_dir).ok();
        if let Some(receipt) = &receipt {
            if !receipt.is_vanilla() {
                return Err(format!(
                    "the engine at {} is not vanilla: its receipt records {} applied patches",
                    engine_dir.display(),
                    receipt.patches_applied
                ));
            }
        }
        let hermesc = if precompiled_only {
            None
        } else {
            Some(Self::find_hermesc(repo_root)?)
        };
        let toolchain = match (
            receipt.as_ref().and_then(|r| r.compiler_digest.clone()),
            &hermesc,
        ) {
            (Some(digest), _) => digest,
            (None, Some(hermesc)) => hash_file(hermesc)?,
            (None, None) => "no-compiler".to_string(),
        };
        Ok(Self {
            hermesc,
            cache_dir,
            toolchain,
            engine: receipt.map(|receipt| receipt.binary_digest),
            linked: Self::linked_engine().to_string(),
        })
    }

    /// The engine linked into this binary, baked in at link time by
    /// `build.rs` as a digest of the archive actually linked. What artifacts
    /// are keyed by and what a manifest is checked against.
    pub fn linked_engine() -> &'static str {
        option_env!("IBEX2_LINKED_ENGINE_DIGEST").unwrap_or("no-linked-engine")
    }

    fn find_hermesc(repo_root: &Path) -> Result<PathBuf, String> {
        let hermesc = hermesc_path(repo_root);
        if !hermesc.exists() {
            return Err(format!(
                "hermesc not found at {}\n\
                 build it with the platform's build-hermes script and --vanilla\n\
                 (or point IBEX2_HERMESC at one)",
                hermesc.display()
            ));
        }
        Ok(hermesc)
    }

    /// Find `hermesc` beside the vanilla engine, or wherever
    /// `IBEX2_HERMESC` points.
    pub fn discover(repo_root: &Path, cache_dir: PathBuf) -> Result<Self, String> {
        let hermesc = hermesc_path(repo_root);
        if !hermesc.exists() {
            return Err(format!(
                "hermesc not found at {}\n\
                 build it with the platform's build-hermes script and --vanilla\n\
                 (or point IBEX2_HERMESC at one)",
                hermesc.display()
            ));
        }
        Self::new(hermesc, cache_dir)
    }

    pub fn new(hermesc: PathBuf, cache_dir: PathBuf) -> Result<Self, String> {
        Self::with_engine(hermesc, cache_dir, None)
    }

    /// Bind this compiler's artifacts to a specific engine receipt.
    pub fn with_engine(
        hermesc: PathBuf,
        cache_dir: PathBuf,
        engine: Option<crate::receipt::HermesInput>,
    ) -> Result<Self, String> {
        // The receipt already records the compiler's digest; hash the binary
        // only when there is no receipt to say.
        let toolchain = match engine.as_ref().and_then(|r| r.compiler_digest.clone()) {
            Some(digest) => digest,
            None => hash_file(&hermesc)?,
        };
        Ok(Self {
            hermesc: Some(hermesc),
            cache_dir,
            toolchain,
            engine: engine.map(|receipt| receipt.binary_digest),
            linked: Self::linked_engine().to_string(),
        })
    }

    /// The cache key for one wrapped module.
    ///
    /// Everything that can change the output is an input: the exact bytes
    /// compiled, the compiler that compiles them, and the artifact scheme.
    /// Nothing else may be — in particular **not the module's grants**, or
    /// bytecode would depend on policy and changing a manifest would mean
    /// recompiling. The wrapper is deliberately grant-independent: a module
    /// granted nothing receives a binding that refuses, not an absent one.
    pub fn key(&self, wrapped: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(ARTIFACT_VERSION.to_le_bytes());
        hasher.update(self.linked.as_bytes());
        hasher.update(self.toolchain.as_bytes());
        hasher.update(
            self.engine
                .as_deref()
                .unwrap_or("no-engine-receipt")
                .as_bytes(),
        );
        hasher.update((wrapped.len() as u64).to_le_bytes());
        hasher.update(wrapped.as_bytes());
        hex(&hasher.finalize())
    }

    pub fn artifact_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{key}.hbc"))
    }

    /// Bytecode for `wrapped`, from cache when it is there.
    pub fn compile(&self, wrapped: &str) -> Result<Vec<u8>, String> {
        let key = self.key(wrapped);
        let artifact = self.artifact_path(&key);
        if let Ok(bytes) = std::fs::read(&artifact) {
            if is_hermes_bytecode(&bytes) {
                return Ok(bytes);
            }
            // A truncated or foreign file in the cache is a miss, not a
            // failure: the cache is derived data and may always be rebuilt.
        }
        let bytes = self.compile_uncached(wrapped)?;
        self.write_atomically(&artifact, &bytes)?;
        Ok(bytes)
    }

    /// Bytecode for an artifact key, without touching source at all.
    ///
    /// The shipping path: no read, no hash, no compile.
    pub fn by_key(&self, key: &str) -> Result<Vec<u8>, String> {
        let bytes = std::fs::read(self.artifact_path(key))
            .map_err(|_| format!("no precompiled artifact {key}"))?;
        if !is_hermes_bytecode(&bytes) {
            return Err(format!("cached artifact {key} is not Hermes bytecode"));
        }
        Ok(bytes)
    }

    /// Bytecode for `wrapped`, only if it is already built.
    ///
    /// Still hashes the source to find the artifact, so prefer `by_key` with a
    /// manifest on the startup path.
    pub fn cached_only(&self, wrapped: &str) -> Result<Vec<u8>, String> {
        let key = self.key(wrapped);
        let artifact = self.artifact_path(&key);
        let bytes = std::fs::read(&artifact)
            .map_err(|_| format!("no precompiled artifact for this module ({key})"))?;
        if !is_hermes_bytecode(&bytes) {
            return Err(format!("cached artifact {key} is not Hermes bytecode"));
        }
        Ok(bytes)
    }

    fn compile_uncached(&self, wrapped: &str) -> Result<Vec<u8>, String> {
        std::fs::create_dir_all(&self.cache_dir)
            .map_err(|e| format!("cannot create {}: {e}", self.cache_dir.display()))?;

        // hermesc reads a file, so the source goes beside its artifact and is
        // removed after. Named by key so concurrent builds of different modules
        // cannot collide.
        let key = self.key(wrapped);
        let source_path = self.cache_dir.join(format!("{key}.js"));
        let out_path = self.cache_dir.join(format!("{key}.hbc.tmp"));
        std::fs::write(&source_path, wrapped)
            .map_err(|e| format!("cannot write {}: {e}", source_path.display()))?;

        let Some(hermesc) = &self.hermesc else {
            return Err(
                "not in the build manifest, and a --precompiled run compiles nothing; \
                 run `ibex2 build`"
                    .into(),
            );
        };
        let output = std::process::Command::new(hermesc)
            .args(["-emit-binary", "-O", "-out"])
            .arg(&out_path)
            .arg(&source_path)
            .output()
            .map_err(|e| format!("cannot run {}: {e}", hermesc.display()))?;
        let _ = std::fs::remove_file(&source_path);

        if !output.status.success() {
            let _ = std::fs::remove_file(&out_path);
            return Err(format!(
                "hermesc failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let bytes = std::fs::read(&out_path)
            .map_err(|e| format!("cannot read {}: {e}", out_path.display()))?;
        let _ = std::fs::remove_file(&out_path);

        if !is_hermes_bytecode(&bytes) {
            return Err("hermesc produced something that is not bytecode".into());
        }
        Ok(bytes)
    }

    /// Write through a temporary, so a reader never sees a partial artifact and
    /// two builders racing on the same module both end with a whole one.
    fn write_atomically(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        let temp = path.with_extension(format!("hbc.{}", std::process::id()));
        std::fs::write(&temp, bytes)
            .map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
        std::fs::rename(&temp, path).map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            format!("cannot install {}: {e}", path.display())
        })
    }
}

fn hermesc_path(repo_root: &Path) -> PathBuf {
    match std::env::var("IBEX2_HERMESC") {
        Ok(path) => PathBuf::from(path),
        Err(_) => {
            let arch = if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "x64"
            };
            repo_root.join(format!(
                "tools/hermes-vanilla/hermesc-{}-{arch}",
                std::env::consts::OS
            ))
        }
    }
}

/// Every artifact of a build in one file, read once at start.
///
/// The per-artifact files stay as the compile cache — a rebuild recompiles
/// only what changed — but a run opened, read, and closed one of them per
/// module: 14 of the 15 µs a module cost after the resolve cache, five
/// hundred syscall triples for 5 MB. The bundle is those same bytes in one
/// file behind an index, read with one call and served from memory. Keys are
/// the manifest's artifact keys; a key the bundle lacks falls back to its
/// file, so a bundle is never required, only faster.
///
/// Layout: magic, `u32` count, then per entry `u16` key length, the key,
/// `u64` offset, `u64` length; then the data, offsets relative to its start.
/// Nothing here is parsed by anything but this file.
#[derive(Debug, Default)]
pub struct Bundle {
    data: Vec<u8>,
    index: std::collections::HashMap<String, (usize, usize)>,
}

const BUNDLE_MAGIC: &[u8; 8] = b"IBX2BNDL";

impl Bundle {
    pub fn path(cache_dir: &Path) -> PathBuf {
        cache_dir.join("bundle.bin")
    }

    /// Write `artifacts` — key and bytecode — as one file, atomically.
    pub fn write(cache_dir: &Path, artifacts: &[(String, Vec<u8>)]) -> Result<(), String> {
        std::fs::create_dir_all(cache_dir)
            .map_err(|e| format!("cannot create {}: {e}", cache_dir.display()))?;
        let mut out = Vec::with_capacity(
            artifacts
                .iter()
                .map(|(k, b)| k.len() + b.len() + 18)
                .sum::<usize>()
                + 12,
        );
        out.extend_from_slice(BUNDLE_MAGIC);
        out.extend_from_slice(&(artifacts.len() as u32).to_le_bytes());
        let mut offset = 0u64;
        for (key, bytes) in artifacts {
            let key_len =
                u16::try_from(key.len()).map_err(|_| format!("artifact key too long: {key}"))?;
            out.extend_from_slice(&key_len.to_le_bytes());
            out.extend_from_slice(key.as_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            offset += bytes.len() as u64;
        }
        for (_, bytes) in artifacts {
            out.extend_from_slice(bytes);
        }
        let path = Self::path(cache_dir);
        let tmp = path.with_extension("bin.tmp");
        std::fs::write(&tmp, &out).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// The bundle under `cache_dir`, or `None` when there is none or it is
    /// not one this code wrote.
    pub fn read(cache_dir: &Path) -> Option<Self> {
        Self::parse(std::fs::read(Self::path(cache_dir)).ok()?)
    }

    fn parse(data: Vec<u8>) -> Option<Self> {
        let mut at = 0usize;
        let take = |at: &mut usize, n: usize| -> Option<&[u8]> {
            let slice = data.get(*at..*at + n)?;
            *at += n;
            Some(slice)
        };
        if take(&mut at, 8)? != BUNDLE_MAGIC {
            return None;
        }
        let count = u32::from_le_bytes(take(&mut at, 4)?.try_into().ok()?) as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let key_len = u16::from_le_bytes(take(&mut at, 2)?.try_into().ok()?) as usize;
            let key = std::str::from_utf8(take(&mut at, key_len)?)
                .ok()?
                .to_string();
            let offset = u64::from_le_bytes(take(&mut at, 8)?.try_into().ok()?) as usize;
            let len = u64::from_le_bytes(take(&mut at, 8)?.try_into().ok()?) as usize;
            entries.push((key, offset, len));
        }
        let start = at;
        let mut index = std::collections::HashMap::with_capacity(count);
        for (key, offset, len) in entries {
            let from = start.checked_add(offset)?;
            let to = from.checked_add(len)?;
            if to > data.len() {
                return None;
            }
            index.insert(key, (from, to));
        }
        Some(Self { data, index })
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.index.get(key).map(|&(from, to)| &self.data[from..to])
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

/// Resolved specifier to artifact key, written by the build.
///
/// Without this, finding a module's artifact means hashing its wrapped source —
/// so a "precompiled" run still reads and hashes every byte of source to
/// discover what it already has. Measured: that left a 570-module boot at
/// 157ms when evaluating the same bytecode takes about 3ms. The manifest is
/// what makes precompiled mean *precompiled*.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    entries: std::collections::BTreeMap<String, String>,
    /// The linked-engine digest the artifacts were built for. Checked against
    /// the running binary's own at load, so a cache built by one engine is
    /// refused by another rather than found by key and fed to it.
    engine: Option<String>,
    /// Every edge the build resolved: (importer, specifier as written) to the
    /// resolved specifier. A run that has the edge resolves nothing — no
    /// resolver, no directory listing, no `realpath` — which is what lets a
    /// run-only binary carry no resolver at all (LLP 0065 §3.3).
    edges: std::collections::BTreeMap<(String, String), String>,
}

impl Manifest {
    pub fn new() -> Self {
        Self::default()
    }

    /// A manifest for artifacts built by `compiler`'s engine.
    pub fn for_engine(engine: &str) -> Self {
        Self {
            entries: Default::default(),
            engine: Some(engine.to_string()),
            edges: Default::default(),
        }
    }

    pub fn engine(&self) -> Option<&str> {
        self.engine.as_deref()
    }

    /// Record that `specifier`, written inside `from`, resolved to `resolved`.
    pub fn insert_edge(&mut self, from: &str, specifier: &str, resolved: &str) {
        self.edges.insert(
            (from.to_string(), specifier.to_string()),
            resolved.to_string(),
        );
    }

    /// What the build resolved `specifier` to, inside `from`, if it did.
    pub fn edge(&self, from: &str, specifier: &str) -> Option<&str> {
        self.edges
            .get(&(from.to_string(), specifier.to_string()))
            .map(String::as_str)
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn insert(&mut self, specifier: &str, key: &str) {
        self.entries.insert(specifier.to_string(), key.to_string());
    }

    pub fn get(&self, specifier: &str) -> Option<&str> {
        self.entries.get(specifier).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// One `specifier<TAB>key` per line. Deliberately not JSON: this is read on
    /// the startup path, and a dependency-free line format cannot become a
    /// parser's problem.
    pub fn serialize(&self) -> String {
        let header = self
            .engine
            .as_deref()
            .map(|engine| format!("#engine\t{engine}\n"))
            .unwrap_or_default();
        header
            + &self
                .edges
                .iter()
                .map(|((from, specifier), resolved)| {
                    format!("#edge\t{from}\t{specifier}\t{resolved}\n")
                })
                .collect::<String>()
            + &self
                .entries
                .iter()
                .map(|(specifier, key)| format!("{specifier}\t{key}\n"))
                .collect::<String>()
    }

    pub fn parse(text: &str) -> Self {
        let mut manifest = Manifest::new();
        for line in text.lines() {
            if let Some((specifier, rest)) = line.split_once('\t') {
                match specifier.trim() {
                    "#engine" => manifest.engine = Some(rest.trim().to_string()),
                    "#edge" => {
                        let mut parts = rest.split('\t');
                        if let (Some(from), Some(specifier), Some(resolved)) =
                            (parts.next(), parts.next(), parts.next())
                        {
                            manifest.insert_edge(from.trim(), specifier.trim(), resolved.trim());
                        }
                    }
                    header if header.starts_with('#') => {}
                    specifier => manifest.insert(specifier, rest.trim()),
                }
            }
        }
        manifest
    }

    pub fn path(cache_dir: &Path) -> PathBuf {
        cache_dir.join("manifest.tsv")
    }

    pub fn write(&self, cache_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(cache_dir)
            .map_err(|e| format!("cannot create {}: {e}", cache_dir.display()))?;
        std::fs::write(Self::path(cache_dir), self.serialize())
            .map_err(|e| format!("cannot write the manifest: {e}"))
    }

    pub fn read(cache_dir: &Path) -> Option<Self> {
        std::fs::read_to_string(Self::path(cache_dir))
            .ok()
            .map(|text| Self::parse(&text))
    }
}

/// The HBC magic. Hermes refuses anything else, and so should the cache.
pub fn is_hermes_bytecode(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..4] == [0xc6, 0x1f, 0xbc, 0x03]
}

/// `sha256-<hex>` of a file, the receipt's own convention.
fn hash_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(format!("sha256-{}", hex(&Sha256::digest(&bytes))))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiler(name: &str) -> Option<(Compiler, PathBuf)> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cache = std::env::temp_dir().join(format!("ibex2-hbc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        Compiler::discover(&root, cache.clone())
            .ok()
            .map(|c| (c, cache))
    }

    #[test]
    fn magic_recognises_bytecode_and_nothing_else() {
        assert!(is_hermes_bytecode(&[0xc6, 0x1f, 0xbc, 0x03, 0, 0, 0, 0]));
        assert!(!is_hermes_bytecode(b"function f(){}"));
        assert!(!is_hermes_bytecode(&[0xc6, 0x1f]));
        assert!(!is_hermes_bytecode(&[]));
    }

    #[test]
    fn the_key_covers_the_source_and_nothing_covers_the_grants() {
        let Some((c, cache)) = compiler("key") else {
            return;
        };
        let a = c.key("(function(){ return 1; })");
        let b = c.key("(function(){ return 2; })");
        assert_ne!(a, b, "different sources must not share an artifact");
        assert_eq!(a, c.key("(function(){ return 1; })"), "keys are stable");
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn compiling_produces_bytecode_and_the_second_call_is_cached() {
        let Some((c, cache)) = compiler("compile") else {
            return;
        };
        let source = "(function (module, exports, require, fetch) { return 42; })";
        let first = c.compile(source).expect("compile");
        assert!(is_hermes_bytecode(&first));
        assert!(c.artifact_path(&c.key(source)).exists());

        // Second call reads the artifact; identical bytes prove it did not
        // silently recompile into something different.
        let second = c.compile(source).expect("cached");
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn a_corrupt_cache_entry_is_a_miss_rather_than_a_failure() {
        let Some((c, cache)) = compiler("corrupt") else {
            return;
        };
        let source = "(function () { return 1; })";
        c.compile(source).expect("compile");
        let artifact = c.artifact_path(&c.key(source));
        std::fs::write(&artifact, b"not bytecode").expect("corrupt it");

        let recovered = c.compile(source).expect("recompiled");
        assert!(is_hermes_bytecode(&recovered));
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn cached_only_refuses_what_has_not_been_built() {
        let Some((c, cache)) = compiler("strict") else {
            return;
        };
        let source = "(function () { return 7; })";
        assert!(c.cached_only(source).is_err(), "must not compile on demand");
        c.compile(source).expect("compile");
        assert!(c.cached_only(source).is_ok());
        let _ = std::fs::remove_dir_all(cache);
    }

    /// Artifacts are bound to the engine they were built for, so bytecode from
    /// one engine cannot be found under another (LLP 0058.000.001 §5).
    #[test]
    fn the_artifact_key_binds_the_engine_receipt() {
        let Some((plain, cache)) = compiler("engine") else {
            return;
        };
        let bound = Compiler::with_engine(
            plain
                .hermesc
                .clone()
                .expect("a discovered compiler has hermesc"),
            cache.clone(),
            Some(crate::receipt::HermesInput {
                binary_digest: "sha256-engine-a".into(),
                variant: "release".into(),
                patch_set_digest: crate::receipt::CANONICAL_EMPTY_PATCH_SET.into(),
                patches_applied: 0,
                compiler_digest: None,
            }),
        )
        .expect("compiler");
        let other = Compiler::with_engine(
            plain
                .hermesc
                .clone()
                .expect("a discovered compiler has hermesc"),
            cache.clone(),
            Some(crate::receipt::HermesInput {
                binary_digest: "sha256-engine-b".into(),
                variant: "release".into(),
                patch_set_digest: crate::receipt::CANONICAL_EMPTY_PATCH_SET.into(),
                patches_applied: 0,
                compiler_digest: None,
            }),
        )
        .expect("compiler");

        let source = "(function () { return 1; })";
        assert_ne!(bound.key(source), other.key(source), "engines share a key");
        assert_ne!(
            bound.key(source),
            plain.key(source),
            "receipted == unreceipted"
        );
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn a_manifest_round_trips_and_finds_artifacts_without_source() {
        let Some((c, cache)) = compiler("manifest") else {
            return;
        };
        let source = "(function () { return 5; })";
        let bytes = c.compile(source).expect("compile");

        let mut manifest = Manifest::new();
        manifest.insert("./a.js", &c.key(source));
        manifest.write(&cache).expect("write");

        let read = Manifest::read(&cache).expect("read");
        assert_eq!(read.len(), 1);
        let key = read.get("./a.js").expect("entry");
        assert_eq!(c.by_key(key).expect("by key"), bytes);
        assert!(c.by_key("nonexistent").is_err());
        let _ = std::fs::remove_dir_all(cache);
    }

    #[test]
    fn a_syntax_error_is_reported_rather_than_cached() {
        let Some((c, cache)) = compiler("syntax") else {
            return;
        };
        let err = c
            .compile("(function () { this is not javascript }")
            .unwrap_err();
        assert!(err.contains("hermesc failed"), "{err}");
        let _ = std::fs::remove_dir_all(cache);
    }

    /// The manifest carries the engine its artifacts were built for, as a
    /// header line the entry parser skips, and it survives a round trip.
    #[test]
    fn a_manifest_records_its_engine_and_reads_it_back() {
        let mut manifest = Manifest::for_engine("sha256-abc");
        manifest.insert("./a.js", "k1");
        manifest.insert("./b.js", "k2");
        let text = manifest.serialize();
        assert!(text.starts_with("#engine\tsha256-abc\n"), "{text}");
        let back = Manifest::parse(&text);
        assert_eq!(back.engine(), Some("sha256-abc"));
        assert_eq!(back.get("./a.js"), Some("k1"));
        assert_eq!(back.len(), 2, "the header is not an entry");
        // Edges round-trip too, and are not entries either.
        manifest.insert_edge("./a.js", "./b", "./b.js");
        manifest.insert_edge("./", "./a.js", "./a.js");
        let back = Manifest::parse(&manifest.serialize());
        assert_eq!(back.edge("./a.js", "./b"), Some("./b.js"));
        assert_eq!(back.edge("./", "./a.js"), Some("./a.js"));
        assert_eq!(back.edge("./a.js", "./c"), None);
        assert_eq!(back.len(), 2);
        assert_eq!(back.edge_count(), 2);
        // A manifest from before engine binding has no engine, and says so.
        assert_eq!(Manifest::parse("./a.js\tk1\n").engine(), None);
    }

    /// The key folds in the engine this binary links, so a rebuild against a
    /// different engine is a cache miss rather than bytecode fed to the wrong
    /// VM — and the constant is baked, never read from disk at run time.
    #[test]
    fn the_linked_engine_is_part_of_every_key() {
        let linked = Compiler::linked_engine();
        assert!(
            linked.starts_with("sha256-") || linked == "no-linked-engine",
            "{linked}"
        );
        // Two compilers alike in everything but the engine they link key the
        // same wrapper differently — which is what makes a cache built by one
        // binary a miss, not a hit, under another.
        let cache = std::env::temp_dir().join(format!("ibex2-linked-key-{}", std::process::id()));
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok(one) = Compiler::discover(&repo, cache.clone()) else {
            return;
        };
        let other = one.clone().with_linked("sha256-another-engine");
        assert_ne!(one.key("(function () {})"), other.key("(function () {})"));
        assert_eq!(
            one.key("(function () {})"),
            one.clone().key("(function () {})")
        );
    }

    #[test]
    fn a_bundle_round_trips_and_a_missing_key_is_none() {
        let dir = std::env::temp_dir().join(format!("ibex2-bundle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let artifacts = vec![
            ("k1".to_string(), b"one".to_vec()),
            ("k2".to_string(), vec![0u8; 10_000]),
            ("k3".to_string(), Vec::new()),
        ];
        Bundle::write(&dir, &artifacts).unwrap();
        let bundle = Bundle::read(&dir).unwrap();
        assert_eq!(bundle.len(), 3);
        assert_eq!(bundle.get("k1"), Some(&b"one"[..]));
        assert_eq!(bundle.get("k2").map(|b| b.len()), Some(10_000));
        assert_eq!(bundle.get("k3"), Some(&[][..]));
        assert_eq!(bundle.get("nope"), None);
        // Not ours, or truncated: no bundle, never a panic.
        std::fs::write(Bundle::path(&dir), b"IBX2BNDL\x01\x00\x00\x00\xff").unwrap();
        assert!(Bundle::read(&dir).is_none());
        std::fs::write(Bundle::path(&dir), b"something else").unwrap();
        assert!(Bundle::read(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
