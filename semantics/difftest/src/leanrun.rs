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

/// Run `cases` in one Lean process; each case's observation lines, in
/// order. `dir` keeps the generated module for a failure's reproduction.
pub fn run(cases: &[LeanCase], dir: &Path, tag: &str) -> Result<Vec<Vec<String>>, String> {
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
         | .some (b, _) => IO.println (Number.jsToString (Float.ofBits (UInt64.ofNat b)))\n    \
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
