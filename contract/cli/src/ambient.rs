//! Reuse the bake's ambient-API analysis before a Contract build succeeds.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) fn check(input: &Path) -> Vec<contract::CompileError> {
    let app = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !app.join("app.ts").is_file() {
        return Vec::new();
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bun = std::env::var_os("BUN")
        .map(PathBuf::from)
        .or_else(|| {
            let path = PathBuf::from(std::env::var_os("HOME")?).join(".bun-1.4.2/bin/bun");
            path.is_file().then_some(path)
        })
        .unwrap_or_else(|| "bun".into());
    let output = Command::new(bun)
        .arg(root.join("js/bake/src/typescript.mjs"))
        .arg("--ambient")
        .arg(app)
        .output();
    let failure = |message: String| {
        vec![contract::CompileError {
            pass: "analyze",
            id: "analyze-ambient-api".into(),
            message,
            span: contract_syntax::Span::default(),
            file: Some(app.join("app.ts").into_boxed_path()),
            related: Box::new([]),
        }]
    };
    let output = match output {
        Ok(output) => output,
        Err(error) => return failure(format!("ambient API check: {error}")),
    };
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return failure(said.clone() + &ignored_hint(app, &said));
    }
    let found: Vec<serde_json::Value> = match serde_json::from_slice(&output.stdout) {
        Ok(found) => found,
        Err(error) => {
            return failure(format!(
                "ambient API check: invalid diagnostic reply: {error}"
            ))
        }
    };
    found
        .into_iter()
        .map(|v| contract::CompileError {
            pass: "analyze",
            id: "analyze-ambient-api".into(),
            message: v["message"]
                .as_str()
                .unwrap_or("unavailable ambient API")
                .into(),
            span: contract_syntax::Span::point(
                v["line"].as_u64().unwrap_or(1) as u32,
                v["col"].as_u64().unwrap_or(1) as u32,
            ),
            file: v["file"]
                .as_str()
                .map(|s| PathBuf::from(s).into_boxed_path()),
            related: Box::new([]),
        })
        .collect()
}

/// What to say of each relative import the bundle couldn't resolve that
/// names a file git ignores in `app` (Expose's `./local`, which its hosts'
/// build scripts write before a bake): a file this checkout doesn't have,
/// and how one gets it. Empty when there is none.
fn ignored_hint(app: &Path, said: &str) -> String {
    let mut out = String::new();
    for part in said.split("Could not resolve '").skip(1) {
        let Some(import) = part.split('\'').next() else {
            continue;
        };
        if !import.starts_with("./") && !import.starts_with("../") {
            continue;
        }
        // Against the app's directory, where app.ts is: an import from a
        // module in a subdirectory is looked for there too, so the hint is
        // only ever missing or extra for one, never wrong about app.ts's.
        let candidates = ["", ".ts", ".tsx", ".js", ".mjs"].map(|ext| format!("{import}{ext}"));
        if candidates.iter().any(|c| app.join(c).is_file()) {
            continue;
        }
        let ignored = candidates.iter().find(|c| {
            Command::new("git")
                .arg("-C")
                .arg(app)
                .args(["check-ignore", "-q", "--"])
                .arg(c.as_str())
                .status()
                .is_ok_and(|s| s.success())
        });
        if let Some(file) = ignored {
            let line = format!(
                "\n`{import}` is `{file}`, a file git ignores here, so a fresh checkout doesn't have it: it is local to a developer's machine. The app's own build writes it (a host crate's build.rs, or its README says how); build it through a host once, or create the file, then build again."
            );
            if !out.contains(&line) {
                out.push_str(&line);
            }
        }
    }
    out
}
