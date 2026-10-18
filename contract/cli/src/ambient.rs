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
        return failure(String::from_utf8_lossy(&output.stderr).trim().into());
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
