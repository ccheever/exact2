//! The TypeScript producer. @ref LLP 1027 D5 / LLP 1030.000 §7.
//! Type-check, bundle, compile HBC, then bake through that exact bytecode.
//! This crate is build-time only; `exact-js` never depends on the compiler.

#![deny(missing_docs)]

mod resident;
#[cfg(test)]
mod sources_tests;
pub use resident::Producer;

use contract::DataSource;
use exact_js::Module;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// An app's Cargo build-script entrypoint. Write the paired artifacts and
/// actual target/grants receipt to OUT_DIR; source files stay untouched.
/// This development slice requires an updater-free composition (store L=0).
pub fn build(app: &Path, platform: &str) -> Result<(), String> {
    build_sources(app, platform, None, None)
}

/// Bake a mixed app using each source's own identity and grants. Metadata reads
/// never invoke or activate the Rust implementation; first-frame values bake
/// through TypeScript alone.
pub fn build_mixed(app: &Path, platform: &str, rust: &dyn DataSource) -> Result<(), String> {
    build_sources(app, platform, Some(rust), None)
}

/// Bake a mixed app through its own composer (LLP 1027.002 §5 step 0): the
/// bake's TypeScript module is composed with the app's Rust source by
/// `compose`, so a resource only Rust owns bakes its first-frame value
/// through Rust, with no TypeScript placeholder. `rust_sources` names what
/// Rust owns, for the receipt the development producer seeds from.
pub fn build_mixed_with(
    app: &Path,
    platform: &str,
    rust: &dyn DataSource,
    rust_sources: &[&str],
    compose: impl FnOnce(Module) -> Box<dyn DataSource>,
) -> Result<(), String> {
    let composer = Composer {
        rust_sources: rust_sources.iter().map(|s| s.to_string()).collect(),
        compose: Box::new(compose),
    };
    build_sources(app, platform, Some(rust), Some(composer))
}

/// How a Cargo bake composes the TypeScript module with the app's Rust.
struct Composer<'a> {
    rust_sources: Vec<String>,
    compose: Box<dyn FnOnce(Module) -> Box<dyn DataSource> + 'a>,
}

/// A composed source, as `contract::bake` takes it.
struct Composed(Box<dyn DataSource>);

impl DataSource for Composed {
    fn query(
        &mut self,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, exact_runner::DataError> {
        self.0.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        self.0.answer(store, source, args)
    }
    fn answer_for(
        &mut self,
        target: exact_runner::Target,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        self.0.answer_for(target, store, source, args)
    }
    fn app_id(&self) -> &str {
        self.0.app_id()
    }
    fn grants(&self) -> &str {
        self.0.grants()
    }
    fn revision(&self) -> Option<&str> {
        self.0.revision()
    }
    fn bind(&mut self, plan: &exact_plan::Plan) {
        self.0.bind(plan)
    }
    fn ready(&self) -> bool {
        self.0.ready()
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.0.canvas_surfaces()
    }
}

/// What the development producer knows about the Rust half it cannot run:
/// the previous generation's plan and the sources Rust owns (from the last
/// Cargo bake's receipt). A Rust-owned resource keeps its previous
/// first-frame value until the next Cargo bake; one that has none refuses
/// by name.
pub struct Seed {
    values: BTreeMap<String, Result<exact_plan::Value, String>>,
}

impl Seed {
    /// From the previous plan's bytes and the receipt's `rustSources`.
    pub fn new(plan: &[u8], rust_sources: &[String]) -> Result<Seed, String> {
        let plan = exact_plan::Plan::decode(plan).map_err(|e| format!("previous plan: {e:?}"))?;
        let mut values: BTreeMap<String, Result<exact_plan::Value, String>> = BTreeMap::new();
        for row in plan.resources.iter() {
            let source = plan.str(row.source).to_string();
            if !rust_sources.contains(&source) {
                continue;
            }
            let name = plan.str(row.name).to_string();
            let value = if row.initial.len > 0 {
                exact_plan::Value::from_bytes(plan.bytes(row.initial))
                    .map_err(|e| format!("`{name}`: previous value: {e:?}"))
            } else {
                Err(format!(
                    "`{name}` has no baked value; a Cargo bake supplies it"
                ))
            };
            values
                .entry(source)
                .and_modify(|v| {
                    *v = Err(format!(
                        "`{name}` shares its Rust source with another resource; a Cargo bake resolves it"
                    ))
                })
                .or_insert(value);
        }
        Ok(Seed { values })
    }

    /// From the files on disk, when both exist; `None` otherwise.
    pub fn read(plan: Option<&Path>, receipt: Option<&Path>) -> Result<Option<Seed>, String> {
        let (Some(plan), Some(receipt)) = (plan, receipt) else {
            return Ok(None);
        };
        let (Ok(plan), Ok(receipt)) = (std::fs::read(plan), std::fs::read(receipt)) else {
            return Ok(None);
        };
        let receipt: serde_json::Value =
            serde_json::from_slice(&receipt).map_err(|e| format!("previous receipt: {e}"))?;
        let rust_sources: Vec<String> = receipt["rustSources"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if rust_sources.is_empty() {
            return Ok(None);
        }
        Seed::new(&plan, &rust_sources).map(Some)
    }
}

/// The bake's module beside the seed: Rust-owned sources answer from the
/// previous plan; everything else is the module's.
struct Seeded<'a> {
    module: Module,
    seed: &'a Seed,
}

impl DataSource for Seeded<'_> {
    fn query(
        &mut self,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_plan::Value, exact_runner::DataError> {
        match self.seed.values.get(source) {
            Some(Ok(value)) => Ok(value.clone()),
            Some(Err(message)) => Err(exact_runner::DataError::Unavailable(message.clone())),
            None => self.module.query(source, args),
        }
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        if self.seed.values.contains_key(source) {
            return self.query(source, args).map(exact_runner::Answer::Now);
        }
        self.module.answer(store, source, args)
    }
    fn answer_for(
        &mut self,
        target: exact_runner::Target,
        store: &mut exact_runner::Store,
        source: &str,
        args: &[exact_plan::Value],
    ) -> Result<exact_runner::Answer, exact_runner::DataError> {
        if self.seed.values.contains_key(source) {
            return self.query(source, args).map(exact_runner::Answer::Now);
        }
        self.module.answer_for(target, store, source, args)
    }
    fn app_id(&self) -> &str {
        self.module.app_id()
    }
    fn grants(&self) -> &str {
        self.module.grants()
    }
    fn revision(&self) -> Option<&str> {
        self.module.revision()
    }
    fn bind(&mut self, plan: &exact_plan::Plan) {
        self.module.bind(plan)
    }
    fn ready(&self) -> bool {
        self.module.ready()
    }
    fn canvas_surfaces(&self) -> Vec<(String, usize)> {
        self.module.canvas_surfaces()
    }
}

fn mixed_grants<'a>(
    javascript: &serde_json::Value,
    rust: Option<&'a dyn DataSource>,
) -> Result<Option<&'a str>, String> {
    rust.map(|source| {
        if javascript["appId"].as_str() != Some(source.app_id()) {
            return Err("mixed bake sources must declare the same app identity".into());
        }
        Ok(source.grants())
    })
    .transpose()
}

fn build_sources(
    app: &Path,
    platform: &str,
    rust: Option<&dyn DataSource>,
    composer: Option<Composer<'_>>,
) -> Result<(), String> {
    if !matches!(platform, "web" | "macos" | "ios" | "linux") {
        return Err(format!(
            "module client executor is not yet implemented for {platform}"
        ));
    }
    let stage = Scratch::new(&std::env::temp_dir())?;
    let baked = bake_in(
        app,
        &Tools::default(),
        &stage.0,
        &mut BTreeMap::new(),
        BakeMode::Production,
        composer,
        None,
    )?;
    let meta: serde_json::Value =
        serde_json::from_str(&baked.receipt).map_err(|e| e.to_string())?;
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("build requires OUT_DIR")?);
    for (name, bytes) in [
        ("app.plan", &baked.plan[..]),
        ("app.js", &baked.script[..]),
        ("app.hbc", &baked.bytecode[..]),
        ("app.module.json", baked.receipt.as_bytes()),
    ] {
        std::fs::write(out.join(name), bytes).map_err(|e| e.to_string())?;
    }
    let manifest = contract::Manifest::read(app)?;
    let target = std::env::var("TARGET").map_err(|e| e.to_string())?;
    let compat = exact_bake::compatibility_id_sources(
        app,
        platform,
        &target,
        &manifest,
        meta["grants"].as_str(),
        mixed_grants(&meta, rust)?,
    )?;
    if compat.inputs["store"]["L"] != "0" {
        return Err("module clients currently require deploy.store.<platform> = 0; signed module delivery is not implemented".into());
    }
    std::fs::write(out.join("compat.json"), compat.to_json()).map_err(|e| e.to_string())?;
    let rust_updates = compat.inputs["rustMode"] != "off"
        && compat.inputs["rustModule"]
            .as_str()
            .is_some_and(|module| !module.is_empty());
    let mut metadata = format!("pub const APP: &str = {:?};\npub const GRANTS: &str = {:?};\npub const REVISION: &str = {:?};\npub const RUST_UPDATES: bool = {rust_updates};\n", meta["appId"].as_str().ok_or("missing appId")?, meta["grants"].as_str().ok_or("missing grants")?, meta["module"]["sha256"].as_str().ok_or("missing module hash")?);
    // The module's Canvas 2D roster (LLP 1056 D1), known before it loads.
    metadata.push_str("#[allow(dead_code)]\npub const CANVAS_SURFACES: &[(&str, usize)] = &[");
    for (name, arity) in meta["surfaces"].as_object().into_iter().flatten() {
        metadata.push_str(&format!("({name:?}, {}), ", arity.as_u64().unwrap_or(0)));
    }
    metadata.push_str("];\n");
    // Where each module runs (LLP 1027.002 §6), typed for the executor crate
    // this platform links; the compatibility id already carries the same.
    let placement_type = if platform == "web" {
        "exact_js_web::Placement"
    } else {
        "exact_js::Placement"
    };
    for (language, name) in [
        ("typescript", "TYPESCRIPT_PLACEMENT"),
        ("rust", "RUST_PLACEMENT"),
    ] {
        let variant = match compat.inputs[format!("{language}Placement")].as_str() {
            Some("worker") => "Worker",
            _ => "Main",
        };
        metadata.push_str(&format!(
            "pub const {name}: {placement_type} = {placement_type}::{variant};\n"
        ));
    }
    std::fs::write(out.join("module.rs"), metadata).map_err(|e| e.to_string())?;
    {
        let mode = compat.inputs["rustMode"]
            .as_str()
            .ok_or("missing Rust policy")?;
        // The web links no Rust executor without a module to swap in (LLP 1047 D3).
        let mode = if platform == "web" {
            contract::web_rust_mode(&compat.inputs)
        } else {
            mode
        };
        let mut entry = contract::rust_entry("ExactEmbeddedData", "embedded_data()", mode)?;
        // The web entry links what the plan uses (LLP 1047 D3).
        if platform == "web" {
            let plan = exact_plan::Plan::decode(&baked.plan).map_err(|e| format!("{e:?}"))?;
            entry.push_str(&contract::web_linked(&plan, &compat.inputs));
        }
        std::fs::write(out.join("logic.rs"), entry).map_err(|e| e.to_string())?;
    }
    // Each captured source by name, not the app directory: Cargo scans a named
    // directory recursively, and an app outside this repo keeps its `target/`
    // (and a dev server's output) inside it, so every build dirtied the next
    // and the dev loop rebuilt forever. A new file matters once a watched one
    // names it, which is itself a change.
    let root = app.canonicalize().map_err(|e| e.to_string())?;
    let mounted = mounts(&root)?;
    for name in sources(&root)?.keys() {
        println!(
            "cargo:rerun-if-changed={}",
            origin(&root, &mounted, name).display()
        );
    }
    // And the Contract libraries the app uses, wherever they are installed.
    contract::rerun_if_changed(&root.join("app.contract"));
    Ok(())
}

/// The tools on the producer machine, not on a client.
#[derive(Clone)]
pub struct Tools {
    /// The pinned TypeScript compiler (`EXACT_TSC` overrides the repo tool).
    pub tsc: PathBuf,
    /// The bundler (`EXACT_ROLLDOWN`).
    pub rolldown: PathBuf,
    /// The compiler paired with the host's lean Hermes (`EXACT_HERMESC`).
    pub hermesc: PathBuf,
}

impl Default for Tools {
    fn default() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tool = |key: &str, fallback: PathBuf| {
            std::env::var_os(key).map(PathBuf::from).unwrap_or(fallback)
        };
        let arch = if std::env::consts::ARCH == "aarch64" {
            "arm64"
        } else {
            "x64"
        };
        Self {
            tsc: tool("EXACT_TSC", root.join("node_modules/.bin/tsc")),
            rolldown: tool("EXACT_ROLLDOWN", root.join("node_modules/.bin/rolldown")),
            hermesc: tool(
                "EXACT_HERMESC",
                if cfg!(target_os = "linux") {
                    root.join(format!("../ibex/tools/hermes-vanilla/hermesc-linux-{arch}"))
                } else {
                    // js/build.rs's fallback: the machine's cache when there is no sibling ibex.
                    let sibling =
                        root.join(format!("../ibex/tools/hermes-vanilla/hermesc-macos-{arch}"));
                    let cache = Path::new(&std::env::var_os("HOME").unwrap_or_default())
                        .join(".cache/exact/hermes-macos");
                    if std::env::var_os("EXACT_HERMES_DIR").is_none()
                        && !root.join("../ibex").exists()
                        && cache.join("engine").is_dir()
                    {
                        cache.join("hermesc")
                    } else {
                        sibling
                    }
                },
            ),
        }
    }
}

/// One complete producer result. No caller-visible output exists until all
/// stages succeed. The receipt binds the plan and both forms of the logic.
pub struct Baked {
    /// The first frame, baked through `bytecode` with an empty store.
    pub plan: Vec<u8>,
    /// The one script the browser will run in its private module environment.
    pub script: Vec<u8>,
    /// The bytecode run by native clients; never compiled on a client.
    pub bytecode: Vec<u8>,
    /// Generated declaration text used for this build's type check.
    pub declarations: String,
    /// App identity, grants, ABI and content hashes; not a signing receipt.
    pub receipt: String,
    /// Development-only source locations, keyed by the final plan digest.
    /// Absent from standalone and Cargo bakes, receipts and module payloads.
    pub source_map: Option<String>,
}

/// A temporary directory we created, cleaned on every success/refusal path.
struct Scratch(PathBuf);
impl Scratch {
    fn new(parent: &Path) -> Result<Self, String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let path = parent.join(format!(
                ".exact-js-bake-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("cannot allocate a private producer directory".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The generated declarations `app.ts` imports.
const DECLARATIONS: &str = "app.contract.d.ts";

/// Directories outside the app its TypeScript also imports, from the
/// manifest's `typescript.sources` (`{"core": "../../src/core"}`): each is
/// captured as if it sat beside `app.ts` under its name, so `./core/model`
/// resolves the same in the bake as in an editor given a link of that name.
/// A domain core shared with another app is imported, not copied.
fn mounts(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    const RESERVED: &[&str] = &[
        "assets",
        "deck",
        "gpu",
        "fonts",
        "node_modules",
        "target",
        "dist",
        "web",
        "apple",
        "linux",
    ];
    let manifest = root.join("app.json");
    if !manifest.exists() {
        return Ok(Vec::new());
    }
    let json: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?,
    )
    .map_err(|e| format!("{}: {e}", manifest.display()))?;
    let Some(declared) = json.pointer("/typescript/sources") else {
        return Ok(Vec::new());
    };
    let declared = declared
        .as_object()
        .ok_or("typescript.sources: an object of name → directory")?;
    let mut out = Vec::new();
    for (name, path) in declared {
        let fine = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !fine || RESERVED.contains(&name.as_str()) {
            return Err(format!(
                "typescript.sources.{name}: a lowercase name that is not one of {}",
                RESERVED.join(", ")
            ));
        }
        let path = path
            .as_str()
            .filter(|p| !Path::new(p).is_absolute())
            .ok_or(format!(
                "typescript.sources.{name}: a path relative to the app"
            ))?;
        let dir = root
            .join(path)
            .canonicalize()
            .map_err(|e| format!("typescript.sources.{name}: {path}: {e}"))?;
        let app = root.canonicalize().map_err(|e| e.to_string())?;
        if !dir.is_dir() || dir.starts_with(&app) || app.starts_with(&dir) {
            return Err(format!(
                "typescript.sources.{name}: {path} must be a directory outside the app that does not contain it"
            ));
        }
        out.push((name.clone(), dir));
    }
    Ok(out)
}

/// Where a captured source lives on disk: in the app, or in a mount.
fn origin(root: &Path, mounts: &[(String, PathBuf)], name: &Path) -> PathBuf {
    let mut parts = name.components();
    let first = parts
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    match mounts.iter().find(|(m, _)| Some(m) == first.as_ref()) {
        Some((_, dir)) => dir.join(parts.as_path()),
        None => root.join(name),
    }
}

/// The Contract packages the app's sources use (LLP 1091 D10), staged as
/// `node_modules/<name>/…`: each one's `package.json` and the `.contract`
/// files the compiler read from it, so the staged compile resolves them as the
/// original did. Returns the files and, per package, its staged directory and
/// where it lives, for mapping diagnostics and the source map back.
type Packages = (BTreeMap<PathBuf, Vec<u8>>, Vec<(PathBuf, PathBuf)>);
fn packages(app: &Path) -> Result<Packages, String> {
    let graph = contract::source_graph(&app.join("app.contract"));
    let mut files = BTreeMap::new();
    let mut roots: Vec<(PathBuf, PathBuf)> = Vec::new();
    // Each name a package was reached by is staged: one directory installed
    // under two names resolves under both in the stage as it did outside.
    for package in &graph.packages {
        let staged = Path::new("node_modules").join(&package.name);
        match roots.iter().find(|(at, _)| *at == staged) {
            Some((_, other)) if *other != package.root => {
                return Err(format!(
                    "two copies of the package `{}` ({} and {}) are used; the bake stages one",
                    package.name,
                    other.display(),
                    package.root.display()
                ))
            }
            Some(_) => continue,
            None => roots.push((staged.clone(), package.root.clone())),
        }
        let read = graph
            .sources
            .iter()
            .filter(|source| matches!(&source.origin, contract::Origin::Package { root, .. } if *root == package.root))
            .map(|source| &source.path)
            .chain([&package.manifest]);
        for path in read {
            let relative = path
                .strip_prefix(&package.root)
                .map_err(|e| e.to_string())?;
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            files.insert(staged.join(relative), bytes);
        }
    }
    Ok((files, roots))
}

/// Capture the app-local source graph, and the directories the manifest
/// mounts beside it. External/npm imports intentionally fail in the private
/// snapshot until dependency capture is implemented; they must not silently
/// resolve to unrelated files on the producer machine.
fn sources(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mounts = mounts(root)?;
    fn walk(
        root: &Path,
        at: &Path,
        out: &mut BTreeMap<PathBuf, Vec<u8>>,
        total: &mut usize,
        mounts: &[(String, PathBuf)],
        prefix: &Path,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(at).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if at == root && name == "__exact_build.tsbuildinfo" {
                return Err("__exact_build.tsbuildinfo is reserved for the producer".into());
            }
            if matches!(&*name, ".git" | "node_modules" | "target" | "dist")
                || name.starts_with(".exact-js-bake-")
                // The driver reads a test file; no build does, so editing
                // one rebuilds nothing (trivia F8).
                || name.ends_with(".test.contract")
                // Written beside app.ts for an editor (below); the bake makes its own.
                || (at == root && name == DECLARATIONS)
            {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if let Some((_, dir)) = mounts.iter().find(|(m, _)| at == root && **m == *name) {
                // An editor's link to the mounted directory is allowed and
                // skipped: the mount itself is captured below. Anything else
                // by that name would be two different files at one path.
                if kind.is_symlink() && path.canonicalize().ok().as_ref() == Some(dir) {
                    continue;
                }
                return Err(format!(
                    "{} is mounted from typescript.sources; the app cannot also have one",
                    path.display()
                ));
            }
            let captured = matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("ts" | "json" | "contract" | "ttf" | "otf")
            ) || ["assets", "deck", "gpu/shaders"]
                .iter()
                .any(|tree| relative.starts_with(tree));
            // Links are refused, except a document link outside the captured
            // trees (CLAUDE.md → AGENTS.md): no build reads one. The web
            // build's capture keeps the same rule (host/web-js/build.mjs).
            if kind.is_symlink() {
                let document = matches!(
                    path.extension().and_then(|s| s.to_str()),
                    Some("md" | "txt")
                );
                if captured || !document || path.is_dir() {
                    return Err(format!("source links are not captured: {}", path.display()));
                }
                continue;
            }
            if kind.is_dir() {
                walk(root, &path, out, total, mounts, prefix)?;
            } else if captured {
                if !kind.is_file() {
                    return Err(format!("source is not a regular file: {}", path.display()));
                }
                let size = entry.metadata().map_err(|e| e.to_string())?.len();
                if size > 16 << 20 || *total as u64 + size > 64 << 20 {
                    return Err("app source capture exceeds 64 MiB (16 MiB per file)".into());
                }
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                *total += bytes.len();
                out.insert(prefix.join(relative), bytes);
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    let mut total = 0;
    walk(root, root, &mut result, &mut total, &mounts, Path::new(""))?;
    for (name, dir) in &mounts {
        // Only what TypeScript imports: a shared core's tests and fixtures
        // stay behind (the bundle takes only what `app.ts` reaches anyway).
        let mut mounted = BTreeMap::new();
        walk(dir, dir, &mut mounted, &mut total, &[], Path::new(name))?;
        result.extend(
            mounted
                .into_iter()
                .filter(|(path, _)| path.extension().is_some_and(|e| e == "ts" || e == "json")),
        );
    }
    Ok(result)
}

fn run(tool: &Path, args: &[&str], cwd: &Path) -> Result<(), String> {
    // Package executables retain their upstream Node shebang. Run JavaScript
    // through Bun while preserving native and shell compiler overrides.
    use std::io::Read;
    let mut header = [0; 128];
    let len = std::fs::File::open(tool)
        .and_then(|mut file| file.read(&mut header))
        .unwrap_or(0);
    let shebang = String::from_utf8_lossy(&header[..len]);
    let javascript = shebang.lines().next().is_some_and(|line| {
        line.starts_with("#!")
            && line
                .split_whitespace()
                .any(|word| matches!(word.rsplit('/').next(), Some("node" | "bun")))
    });
    let mut command = if javascript {
        let mut command = exact_bake::bun();
        command.arg(tool);
        command
    } else {
        Command::new(tool)
    };
    let output = command
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("{}: {e}", tool.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} refused:\n{}{}",
            tool.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Build `app.contract` plus `app.ts`, including app-local imports. The app
/// exports `appId`, `grants`, and `answer`; the generated entry checks their
/// types and supplies the executor ABI. No source or source-adjacent generated
/// declaration is overwritten. This producer currently requires macOS Hermes.
pub fn bake(app: &Path, tools: &Tools) -> Result<Baked, String> {
    let stage = Scratch::new(&std::env::temp_dir())?;
    bake_in(
        app,
        tools,
        &stage.0,
        &mut BTreeMap::new(),
        BakeMode::Production,
        None,
        None,
    )
}

enum BakeMode<'a> {
    Production,
    Development {
        compiler: Option<&'a mut resident::Compiler>,
    },
}

fn bake_in(
    app: &Path,
    tools: &Tools,
    stage: &Path,
    previous: &mut BTreeMap<PathBuf, Vec<u8>>,
    mode: BakeMode<'_>,
    composer: Option<Composer<'_>>,
    seed: Option<&Seed>,
) -> Result<Baked, String> {
    if !exact_js::ENGINE_LINKED {
        return Err("TypeScript bake requires the lean Hermes executor on this producer".into());
    }
    let app = app.canonicalize().map_err(|e| e.to_string())?;
    let mut captured = sources(&app)?;
    let (package_files, package_roots) = packages(&app)?;
    captured.extend(package_files.clone());
    if !captured.contains_key(Path::new("app.ts"))
        || !captured.contains_key(Path::new("app.contract"))
    {
        return Err("TypeScript bake needs app.ts and app.contract".into());
    }
    if [
        "__exact_entry.ts",
        "__exact_tsconfig.json",
        "__exact_config.mjs",
        "__exact_paths.json",
        "__exact_canvas.js",
        "__exact_canvas.d.ts",
    ]
    .iter()
    .any(|name| captured.contains_key(Path::new(name)))
    {
        return Err(
            "__exact_entry.ts, __exact_tsconfig.json, __exact_config.mjs, __exact_paths.json and __exact_canvas.* are reserved for the producer"
                .into(),
        );
    }
    for name in previous.keys().filter(|name| !captured.contains_key(*name)) {
        let path = stage.join(name);
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        let mut parent = path.parent();
        while let Some(directory) = parent.filter(|directory| *directory != stage) {
            if std::fs::remove_dir(directory).is_err() {
                break;
            }
            parent = directory.parent();
        }
    }
    for (name, bytes) in &captured {
        let target = stage.join(name);
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        if previous.get(name) != Some(bytes) {
            std::fs::write(target, bytes).map_err(|e| e.to_string())?;
        }
    }
    *previous = captured.clone();
    // A changed graph (including a newly added import) is a refused capture.
    let mut again = sources(&app)?;
    again.extend(packages(&app)?.0);
    if again != captured {
        return Err("app sources changed during capture; retry the build".into());
    }
    // Staged packages first: the stage holds them too.
    let moves: Vec<(PathBuf, PathBuf)> = package_roots
        .iter()
        .map(|(staged, root)| (stage.join(staged), root.clone()))
        .chain([(stage.to_path_buf(), app.clone())])
        .collect();
    // Every independent refusal, one after another as `contract build`
    // prints them, in the bake's output and the dev overlay.
    let development = matches!(mode, BakeMode::Development { .. });
    let (plan, mut source_map) =
        contract::compile_path_all(&stage.join("app.contract"), development).map_err(|errors| {
            errors
                .into_iter()
                .map(|e| contract_error(e, &moves))
                .collect::<Vec<_>>()
                .join("\n")
        })?;
    if let Some(map) = source_map.as_mut() {
        map.relocate_sources_through(&moves)?;
    }
    let mut declarations = contract::typescript(&plan)?;
    // Canvas 2D (LLP 1056 D1): a module that exports `draw` and `surfaces`
    // speaks ABI 2, and only it carries the recorder.
    let app_ts = String::from_utf8_lossy(&captured[Path::new("app.ts")]).into_owned();
    let draws = exports(&app_ts, "draw");
    if draws != exports(&app_ts, "surfaces") {
        return Err(
            "app.ts exports `draw` and `surfaces` together, or neither (LLP 1056 D1)".into(),
        );
    }
    if draws {
        declarations.push_str(CANVAS_TYPES);
        write_changed(&stage.join("__exact_canvas.js"), RECORDER.as_bytes())?;
        write_changed(
            &stage.join("__exact_canvas.d.ts"),
            RECORDER_TYPES.as_bytes(),
        )?;
    }
    write_changed(&stage.join(DECLARATIONS), declarations.as_bytes())?;
    // And beside app.ts, so an editor type-checks the module against the plan
    // it will run with. Never captured as a source, never committed; written
    // only when it changes, so a watcher sees one event per contract change.
    if development {
        write_changed(&app.join(DECLARATIONS), declarations.as_bytes())?;
    }
    let mut entry = format!("import * as app from './app';\nimport type {{ Answer }} from './app.contract.d.ts';\nexport const abi = {};\nexport const appId: string = app.appId;\nexport const grants: string = app.grants;\nexport const answer: Answer = app.answer;\n", if draws { 2 } else { 1 });
    if draws {
        entry.push_str(CANVAS_ENTRY);
    }
    write_changed(&stage.join("__exact_entry.ts"), entry.as_bytes())?;
    write_changed(
        &stage.join("__exact_config.mjs"),
        resident::CONFIG.as_bytes(),
    )?;
    write_changed(
        &stage.join("__exact_paths.json"),
        serde_json::json!({"app": app, "mounts": mounts(&app)?})
            .to_string()
            .as_bytes(),
    )?;
    if let BakeMode::Development {
        compiler: Some(compiler),
    } = mode
    {
        compiler.compile(stage, &tools.hermesc)?;
    } else {
        compile_once(stage, tools)?;
        compile_bytecode(stage, &tools.hermesc)?;
    }
    let script = std::fs::read(stage.join("app.js")).map_err(|e| e.to_string())?;
    let bytecode = std::fs::read(stage.join("app.hbc")).map_err(|e| e.to_string())?;
    let mut module = Module::inspect(bytecode.clone())?;
    // The 100 ms per-call budget is a runtime promise about a device, not
    // about a build machine's load: a loaded Mac failed the Signal Clone's
    // bake at 104.8 ms, then passed on retry. A build fails on the source,
    // never on how busy the machine was (LLP 1027 §6).
    module.set_budget_ms(f64::INFINITY);
    let app_id = module.app_id().to_owned();
    let grants = module.grants().to_owned();
    let surfaces: serde_json::Map<String, serde_json::Value> = module
        .canvas_roster()
        .iter()
        .map(|(name, arity)| (name.clone(), (*arity).into()))
        .collect();
    let rust_sources = composer
        .as_ref()
        .map(|c| c.rust_sources.clone())
        .unwrap_or_default();
    // The first frame bakes through the declared owners (LLP 1027.002 §5
    // step 0): the app's composer in a Cargo bake; the previous plan's
    // values for the Rust half a development producer cannot run.
    let plan = match (composer, seed) {
        (Some(composer), _) => contract::bake(plan, Composed((composer.compose)(module))),
        (None, Some(seed)) => contract::bake(plan, Seeded { module, seed }),
        (None, None) => contract::bake(plan, module),
    }
    .map_err(|e| {
        source_map
            .as_ref()
            .map_or_else(|| e.to_string(), |map| map.bake_error(&e).to_string())
    })?
    .encode();
    let source_map = source_map.map(|map| map.json(&plan));
    let receipt = serde_json::json!({
        "version": 1, "appId": app_id, "grants": grants, "abi": if draws { 2 } else { 1 },
        "surfaces": surfaces,
        "rustSources": rust_sources,
        "bytecodeVersion": exact_js::BYTECODE_VERSION,
        "plan": {"file": "app.plan", "sha256": digest(&plan), "bytes": plan.len()},
        "module": {"file": "app.hbc", "sha256": digest(&bytecode), "bytes": bytecode.len()},
        "web": {"file": "app.js", "sha256": digest(&script), "bytes": script.len()},
    })
    .to_string();
    Ok(Baked {
        plan,
        script,
        bytecode,
        declarations,
        receipt,
        source_map,
    })
}

/// The TypeScript recorder (LLP 1056 D3), bundled into a module that draws.
const RECORDER: &str = include_str!("../../../canvas/recorder.js");

/// Its seam's type, for the generated entry's strict check.
const RECORDER_TYPES: &str = "export declare function canvasSeam(draw: (surface: string, args: any, ctx: any, frame: any) => unknown): { draw(request: unknown, host?: unknown): string; retire(retired: unknown): void };\n";

/// The entry's Canvas 2D half: the roster as JSON and the seam's two calls.
const CANVAS_ENTRY: &str = "import { canvasSeam } from './__exact_canvas.js';\nexport const surfacesJson: string = JSON.stringify(app.surfaces);\nconst seam = canvasSeam(app.draw);\nexport const drawCanvas = (request: string, host?: unknown): string => seam.draw(JSON.parse(request), host);\nexport const retireCanvases = (retired: string): void => seam.retire(JSON.parse(retired));\n";

/// What a drawing module's author types against (LLP 1056 D1, stages 1–2):
/// the context is the web's own interface, narrowed to the built members,
/// with `drawImage` and `createPattern` taking an image handle (D9). The web
/// build's type check appends the same file (host/web-js/build.mjs).
const CANVAS_TYPES: &str = include_str!("canvas-types.d.ts");

/// Whether `app.ts` exports `name` (a function, a binding, or in a list).
fn exports(source: &str, name: &str) -> bool {
    let word = |at: usize| {
        source[at + name.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$'))
    };
    [
        "export function ",
        "export async function ",
        "export const ",
        "export let ",
    ]
    .iter()
    .any(|prefix| {
        source
            .match_indices(&format!("{prefix}{name}"))
            .any(|(at, _)| word(at + prefix.len()))
    }) || source.match_indices("export {").any(|(at, _)| {
        let list = &source[at..at + source[at..].find('}').unwrap_or(0)];
        list.split(|c: char| c == ',' || c == '{' || c.is_whitespace())
            .any(|w| w == name)
    })
}

fn contract_error(mut error: contract::CompileError, moves: &[(PathBuf, PathBuf)]) -> String {
    // The staged path an app reads as its own, or its package's: the error's
    // file and each related one.
    let moves: Vec<_> = moves
        .iter()
        .map(|(staged, original)| {
            let canonical = staged.canonicalize().unwrap_or_else(|_| staged.clone());
            (staged, canonical, original)
        })
        .collect();
    let own = |path: &Path| {
        moves.iter().find_map(|(staged, canonical, original)| {
            path.strip_prefix(staged)
                .or_else(|_| path.strip_prefix(canonical))
                .ok()
                .map(|relative| original.join(relative))
        })
    };
    if let Some(path) = error.file.as_mut() {
        if let Some(mapped) = own(path) {
            *path = mapped.into_boxed_path();
        }
    }
    for path in error.related.iter_mut().filter_map(|r| r.file.as_mut()) {
        if let Some(mapped) = own(path) {
            *path = mapped;
        }
    }
    error.to_string()
}

/// With async break checks in every loop and function, so a host can
/// interrupt a running call from another thread (LLP 1048.000 D10).
fn compile_bytecode(stage: &Path, hermesc: &Path) -> Result<(), String> {
    run(
        hermesc,
        &[
            "-O",
            "-emit-async-break-check",
            "-emit-binary",
            "-out",
            "app.hbc",
            "app.js",
        ],
        stage,
    )
}

fn write_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(path).ok().as_deref() != Some(bytes) {
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn compile_once(stage: &Path, tools: &Tools) -> Result<(), String> {
    write_changed(
        &stage.join("__exact_config.mjs"),
        resident::CONFIG.as_bytes(),
    )?;
    let configured = exact_bake::bun()
        .args([
            "--input-type=module",
            "-e",
            "import { configure } from './__exact_config.mjs'; configure(process.cwd());",
        ])
        .current_dir(stage)
        .output()
        .map_err(|e| format!("tsconfig: {e}"))?;
    if !configured.status.success() {
        return Err(String::from_utf8_lossy(&configured.stderr).into_owned());
    }
    run(
        &tools.tsc,
        &["--project", "__exact_tsconfig.json", "--pretty", "false"],
        stage,
    )?;
    // No absolute import or dependency may escape the captured graph, and a
    // path mapping names only files inside it. This generated config is
    // producer-owned, not app configuration.
    std::fs::write(
        stage.join("__exact_bundle.mjs"),
        r#"
export default {
  input: '__exact_entry.ts',
  tsconfig: '__exact_tsconfig.json',
  plugins: [{ name: 'captured-sources', load(id) {
    if (!id.startsWith(process.cwd() + '/')) throw new Error('module outside captured app: ' + id);
    return null;
  }}],
};
"#,
    )
    .map_err(|e| e.to_string())?;
    run(
        &tools.rolldown,
        &[
            "--config",
            "__exact_bundle.mjs",
            "--configLoader",
            "native",
            "__exact_entry.ts",
            "--format",
            "iife",
            "--name",
            "exact",
            "--platform",
            "neutral",
            "--file",
            "app.js",
        ],
        stage,
    )?;
    Ok(())
}

impl Baked {
    /// Publish the complete result to a *new* directory. Existing output is
    /// never replaced; a dev server selects an accepted generation separately.
    pub fn write_new(&self, output: &Path) -> Result<(), String> {
        let output = std::path::absolute(output).map_err(|e| e.to_string())?;
        if output.exists() {
            return Err(format!("output already exists: {}", output.display()));
        }
        // Reserve the destination before writing, so another producer cannot
        // be overwritten by a later rename. The receipt is written last; this
        // directory is not a candidate until that complete receipt exists.
        std::fs::create_dir(&output).map_err(|e| e.to_string())?;
        let result = (|| {
            if let Some(map) = &self.source_map {
                std::fs::write(output.join("app.plan.map.json"), map).map_err(|e| e.to_string())?;
            }
            for (name, bytes) in [
                ("app.plan", &self.plan[..]),
                ("app.js", &self.script),
                ("app.hbc", &self.bytecode),
                ("app.contract.d.ts", self.declarations.as_bytes()),
                ("app.module.json", self.receipt.as_bytes()),
            ] {
                std::fs::write(output.join(name), bytes).map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&output);
        }
        result
    }
}
