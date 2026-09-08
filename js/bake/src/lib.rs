//! The TypeScript producer. @ref LLP 1027 D5 / LLP 1030.000 §7.
//! Type-check, bundle, compile HBC, then bake through that exact bytecode.
//! This crate is build-time only; `exact-js` never depends on the compiler.

#![deny(missing_docs)]

mod resident;
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
    if !matches!(platform, "web" | "macos" | "ios") {
        return Err(format!(
            "module client executor is not yet implemented for {platform}"
        ));
    }
    let baked = bake(app, &Tools::default())?;
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
    let compat =
        contract::compatibility_id(app, platform, &target, &manifest, meta["grants"].as_str())?;
    if compat.inputs["store"]["L"] != "0" {
        return Err("module clients currently require deploy.store.<platform> = 0; signed module delivery is not implemented".into());
    }
    std::fs::write(out.join("compat.json"), compat.to_json()).map_err(|e| e.to_string())?;
    let metadata = format!("pub const APP: &str = {:?};\npub const GRANTS: &str = {:?};\npub const REVISION: &str = {:?};\n", meta["appId"].as_str().ok_or("missing appId")?, meta["grants"].as_str().ok_or("missing grants")?, meta["module"]["sha256"].as_str().ok_or("missing module hash")?);
    std::fs::write(out.join("module.rs"), metadata).map_err(|e| e.to_string())?;
    println!("cargo:rerun-if-changed={}", app.display());
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
                root.join(format!("../ibex/tools/hermes-vanilla/hermesc-macos-{arch}")),
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

/// Capture the app-local source graph. External/npm imports intentionally fail
/// in the private snapshot until dependency capture is implemented; they must
/// not silently resolve to unrelated files on the producer machine.
fn sources(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    fn walk(
        root: &Path,
        at: &Path,
        out: &mut BTreeMap<PathBuf, Vec<u8>>,
        total: &mut usize,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(at).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(&*name, ".git" | "node_modules" | "target" | "dist")
                || name.starts_with(".exact-js-bake-")
            {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                return Err(format!("source links are not captured: {}", path.display()));
            }
            if kind.is_dir() {
                walk(root, &path, out, total)?;
            } else if matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("ts" | "json" | "contract")
            ) {
                if !kind.is_file() {
                    return Err(format!("source is not a regular file: {}", path.display()));
                }
                let size = entry.metadata().map_err(|e| e.to_string())?.len();
                if size > 16 << 20 || *total as u64 + size > 64 << 20 {
                    return Err("app source capture exceeds 64 MiB (16 MiB per file)".into());
                }
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                *total += bytes.len();
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), bytes);
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result, &mut 0)?;
    Ok(result)
}

fn run(tool: &Path, args: &[&str], cwd: &Path) -> Result<(), String> {
    let output = Command::new(tool)
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
    bake_in(app, tools, &stage.0, &mut BTreeMap::new(), None)
}

fn bake_in(
    app: &Path,
    tools: &Tools,
    stage: &Path,
    previous: &mut BTreeMap<PathBuf, Vec<u8>>,
    compiler: Option<&mut resident::Compiler>,
) -> Result<Baked, String> {
    if !exact_js::ENGINE_LINKED {
        return Err("TypeScript bake requires the lean Hermes executor on this producer".into());
    }
    let app = app.canonicalize().map_err(|e| e.to_string())?;
    let captured = sources(&app)?;
    if !captured.contains_key(Path::new("app.ts"))
        || !captured.contains_key(Path::new("app.contract"))
    {
        return Err("TypeScript bake needs app.ts and app.contract".into());
    }
    if captured.contains_key(Path::new("__exact_entry.ts"))
        || captured.contains_key(Path::new("__exact_tsconfig.json"))
    {
        return Err(
            "__exact_entry.ts and __exact_tsconfig.json are reserved for the producer".into(),
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
    if sources(&app)? != captured {
        return Err("app sources changed during capture; retry the build".into());
    }
    let plan = contract::compile_path(&stage.join("app.contract")).map_err(|e| e.to_string())?;
    let declarations = contract::typescript(&plan)?;
    write_changed(&stage.join("app.contract.d.ts"), declarations.as_bytes())?;
    let entry = format!("import * as app from './app';\nimport type {{ Answer }} from './app.contract.d.ts';\nexport const abi = {};\nexport const appId: string = app.appId;\nexport const grants: string = app.grants;\nexport const answer: Answer = app.answer;\n", exact_js::ABI);
    write_changed(&stage.join("__exact_entry.ts"), entry.as_bytes())?;
    if let Some(compiler) = compiler {
        compiler.compile(stage)?;
    } else {
        compile_once(stage, tools)?;
    }
    run(
        &tools.hermesc,
        &["-O", "-emit-binary", "-out", "app.hbc", "app.js"],
        stage,
    )?;
    let script = std::fs::read(stage.join("app.js")).map_err(|e| e.to_string())?;
    let bytecode = std::fs::read(stage.join("app.hbc")).map_err(|e| e.to_string())?;
    let module = Module::inspect(bytecode.clone())?;
    let app_id = module.app_id().to_owned();
    let grants = module.grants().to_owned();
    let plan = contract::bake(plan, module)
        .map_err(|e| e.to_string())?
        .encode();
    let receipt = serde_json::json!({
        "version": 1, "appId": app_id, "grants": grants, "abi": exact_js::ABI,
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
    })
}

fn write_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(path).ok().as_deref() != Some(bytes) {
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn compile_once(stage: &Path, tools: &Tools) -> Result<(), String> {
    run(
        &tools.tsc,
        &[
            "--noEmit",
            "--strict",
            "--target",
            "ES2020",
            "--module",
            "ESNext",
            "--moduleResolution",
            "bundler",
            "--lib",
            "ES2020,DOM",
            "--pretty",
            "false",
            "__exact_entry.ts",
        ],
        stage,
    )?;
    // No path alias, absolute import, or dependency may escape the captured
    // graph. This generated config is producer-owned, not app configuration.
    std::fs::write(
        stage.join("__exact_bundle.mjs"),
        r#"
export default {
  input: '__exact_entry.ts',
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
