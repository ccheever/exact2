//! The Lean half: many cases in one generated module, one `lean --run`.
//!
//! Each case contributes the `contract lean` embedding of its program, its
//! oracle transcript and its events; `main` prints `Contract.Observe.run`
//! for each after a `#case <i>` line.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One case, ready for Lean: the program's embedding (a `def` named
/// `name`), the oracle term and the event terms.
pub struct LeanCase {
    /// The `def`'s name.
    pub name: String,
    /// `def <name> : Contract.Program := …`.
    pub program: String,
    /// The oracle, in `Contract.OracleText`'s format.
    pub oracle: String,
    /// `Contract.Observe.Event` terms.
    pub events: Vec<String>,
}

/// The semantics' Lake project: `semantics/` beside this crate.
pub fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("semantics/")
        .to_path_buf()
}

/// `lake`, from `PATH` or elan's default place.
pub fn lake() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let elan = Path::new(&home).join(".elan/bin/lake");
        if elan.is_file() {
            return elan;
        }
    }
    PathBuf::from("lake")
}

/// Build the semantics library once (its modules are compiled natively, so
/// the generated `main` calls the interpreter as machine code).
pub fn build() -> Result<(), String> {
    let out = Command::new(lake())
        .args([
            "build",
            "Contract.Observe",
            "Contract.OracleText",
            "Contract.LowerCheck",
        ])
        .current_dir(project())
        .output()
        .map_err(|e| format!("lake build: {e} (is Lean installed? see semantics/README.md)"))?;
    if !out.status.success() {
        return Err(format!(
            "lake build failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

/// The generated module for `cases`.
pub fn module(cases: &[LeanCase]) -> String {
    let mut s = String::from("import Contract.Observe\nimport Contract.OracleText\nopen Contract\n\nset_option maxRecDepth 100000\nset_option maxHeartbeats 0\n\n");
    for c in cases {
        s.push_str(&c.program);
        s.push_str(&format!(
            "\ndef {}_oracle : Oracle := OracleText.parse! {}\n\ndef {}_events : List Observe.Event := [{}]\n\n",
            c.name,
            contract::lean::string(&c.oracle),
            c.name,
            c.events.join(", ")
        ));
    }
    s.push_str("def main : IO Unit := do\n");
    for (i, c) in cases.iter().enumerate() {
        s.push_str(&format!(
            "  IO.println \"#case {i}\"\n  for l in Observe.run {n} {n}_oracle {n}_events do IO.println l\n",
            n = c.name
        ));
    }
    if cases.is_empty() {
        s.push_str("  pure ()\n");
    }
    s
}

/// Whether [`run`] reuses observations kept from earlier runs (`quick` and
/// `verify` turn it on; `DIFFTEST_CACHE=1` or `0` decides for any command).
pub static CACHE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn caching() -> bool {
    match std::env::var("DIFFTEST_CACHE").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => CACHE.load(std::sync::atomic::Ordering::Relaxed),
    }
}

/// A digest of everything that decides what Lean prints for a case beside
/// the case itself: the semantics' sources, the toolchain, the Lake file.
fn library_digest() -> &'static str {
    static DIGEST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DIGEST.get_or_init(|| {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "lean") {
                    out.push(p);
                }
            }
        }
        let root = project();
        let mut files = vec![
            root.join("Contract.lean"),
            root.join("lean-toolchain"),
            root.join("lakefile.toml"),
        ];
        walk(&root.join("Contract"), &mut files);
        files.sort();
        let mut h = Digest::default();
        for f in files {
            h.add(
                f.strip_prefix(&root)
                    .unwrap_or(&f)
                    .to_string_lossy()
                    .as_bytes(),
            );
            h.add(&std::fs::read(&f).unwrap_or_default());
        }
        h.hex()
    })
}

/// Two independent 64-bit FNV-1a lanes: a cache key, not a signature.
#[derive(Clone, Copy)]
struct Digest(u64, u64);

impl Default for Digest {
    fn default() -> Self {
        Digest(0xcbf2_9ce4_8422_2325, 0x6c62_272e_07bb_0142)
    }
}

impl Digest {
    fn add(&mut self, bytes: &[u8]) {
        for &b in bytes.iter().chain(&[0xff]) {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            self.1 = (self.1 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3 ^ 0x1000);
        }
    }
    fn hex(self) -> String {
        format!("{:016x}{:016x}", self.0, self.1)
    }
}

/// Where a case's observation is kept: keyed by its embedding (whatever
/// its `def`'s name), oracle and events, and the library.
fn cache_file(dir: &Path, c: &LeanCase) -> PathBuf {
    let mut h = Digest::default();
    h.add(library_digest().as_bytes());
    h.add(
        c.program
            .replacen(&format!("def {} ", c.name), "def _ ", 1)
            .as_bytes(),
    );
    h.add(c.oracle.as_bytes());
    for e in &c.events {
        h.add(e.as_bytes());
    }
    dir.join("cache").join(format!("{}.txt", h.hex()))
}

/// What `f` answers for a generated module's text, kept under `dir` with
/// [`CACHE`] on and answered from there while the module and the library
/// are unchanged.
pub fn memo(
    dir: &Path,
    module: &str,
    f: impl FnOnce() -> Result<Vec<String>, String>,
) -> Result<Vec<String>, String> {
    if !caching() {
        return f();
    }
    let mut h = Digest::default();
    h.add(library_digest().as_bytes());
    h.add(module.as_bytes());
    let file = dir.join("cache").join(format!("{}.txt", h.hex()));
    if let Ok(t) = std::fs::read_to_string(&file) {
        return Ok(t.lines().map(str::to_string).collect());
    }
    let lines = f()?;
    let _ = std::fs::create_dir_all(dir.join("cache"));
    let tmp = file.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&tmp, lines.join("\n")).is_ok() {
        let _ = std::fs::rename(&tmp, &file);
    }
    Ok(lines)
}

/// Run `cases` in one Lean process; each case's observation lines, in
/// order. `dir` keeps the generated module for a failure's reproduction.
/// With [`CACHE`] on, a case whose embedding, oracle, events and library
/// are those of a case run before is answered from what that run printed
/// (Lean's run is deterministic), and only the rest reach Lean.
pub fn run(cases: &[LeanCase], dir: &Path, tag: &str) -> Result<Vec<Vec<String>>, String> {
    if !caching() {
        return run_uncached(cases, dir, tag);
    }
    let files: Vec<PathBuf> = cases.iter().map(|c| cache_file(dir, c)).collect();
    let mut results: Vec<Option<Vec<String>>> = files
        .iter()
        .map(|f| {
            std::fs::read_to_string(f)
                .ok()
                .map(|t| t.lines().map(str::to_string).collect())
        })
        .collect();
    let missing: Vec<usize> = (0..cases.len()).filter(|&i| results[i].is_none()).collect();
    if !missing.is_empty() {
        let todo: Vec<LeanCase> = missing
            .iter()
            .map(|&i| crate::clone_case(&cases[i]))
            .collect();
        let fresh = run_uncached(&todo, dir, tag)?;
        let _ = std::fs::create_dir_all(dir.join("cache"));
        for (&i, lines) in missing.iter().zip(fresh) {
            // Whole or not at all: a reader never sees half an observation.
            let tmp = files[i].with_extension(format!("{}.tmp", std::process::id()));
            if std::fs::write(&tmp, lines.join("\n")).is_ok() {
                let _ = std::fs::rename(&tmp, &files[i]);
            }
            results[i] = Some(lines);
        }
    }
    Ok(results.into_iter().map(Option::unwrap_or_default).collect())
}

fn run_uncached(cases: &[LeanCase], dir: &Path, tag: &str) -> Result<Vec<Vec<String>>, String> {
    if cases.is_empty() {
        return Ok(Vec::new());
    }
    let text = run_module(&module(cases), dir, tag)?;
    let mut results: Vec<Vec<String>> = Vec::new();
    for line in text {
        if line.starts_with("#case ") {
            results.push(Vec::new());
        } else if let Some(last) = results.last_mut() {
            last.push(line);
        }
    }
    if results.len() != cases.len() {
        return Err(format!(
            "{tag}: expected {} cases, got {}",
            cases.len(),
            results.len()
        ));
    }
    Ok(results)
}

/// Run a generated module's `main` in one Lean process; its output lines.
/// `dir` keeps the module for a failure's reproduction.
pub fn run_module(module: &str, dir: &Path, tag: &str) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // One module per process and batch: runs side by side never share one.
    let file = dir.join(format!("{tag}-{}.lean", std::process::id()));
    std::fs::write(&file, module).map_err(|e| format!("{}: {e}", file.display()))?;
    let out = Command::new(lake())
        .args(["env", "lean", "--run"])
        .arg(&file)
        .current_dir(project())
        .output()
        .map_err(|e| format!("lake env lean: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{}: lean failed:\n{}{}",
            file.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

/// Print `Number.jsToString` of each double (by its bits) in one Lean
/// process: the semantics' number printing, for comparison with the
/// runner's `format_number` over many values at once.
pub fn numbers(bits: &[u64], dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(format!("numbers-{}.lean", std::process::id()));
    let data: Vec<String> = bits.iter().map(|b| format!("{b:016x}")).collect();
    let module = format!(
        "import Contract.OracleText\nopen Contract\n\ndef data : String := \"{}\"\n\n\
         def main : IO Unit := do\n  for w in data.splitOn \" \" do\n    \
         match OracleText.hexN 16 0 w.toList with\n    \
         | .some (b, _) => IO.println (Number.jsToString (F64.ofBits (UInt64.ofNat b)))\n    \
         | .none => IO.println \"?\"\n",
        data.join(" ")
    );
    std::fs::write(&file, module).map_err(|e| format!("{}: {e}", file.display()))?;
    let out = Command::new(lake())
        .args(["env", "lean", "--run"])
        .arg(&file)
        .current_dir(project())
        .output()
        .map_err(|e| format!("lake env lean: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{}: lean failed:\n{}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}
