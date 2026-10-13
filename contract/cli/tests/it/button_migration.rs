//! D9's source sweep: imported components compile in their application, where
//! their props and app-local native modules have meaning.

use std::path::{Path, PathBuf};

/// The T3 Code clone: its roots compile in about 300 s in a debug build (some
/// 120,000 nodes), so they are the async lane's (the test at the end).
const T3_CODE: &str = "examples/t3-code";

fn sources(path: &Path, skip: &[PathBuf], out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if skip.contains(&path) {
            continue;
        }
        if path.is_dir() {
            sources(&path, skip, out);
        } else if path.extension().is_some_and(|e| e == "contract")
            && !path.to_string_lossy().ends_with(".test.contract")
        {
            out.push(path);
        }
    }
}

/// Compiles every root under `dirs` (a file no other file there imports) and
/// returns how many compiled and every failure, the button migration's included.
fn sweep(dirs: &[&str], skip: &[&str]) -> (usize, Vec<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let skip: Vec<_> = skip.iter().map(|dir| root.join(dir)).collect();
    let mut paths = Vec::new();
    for dir in dirs {
        sources(&root.join(dir), &skip, &mut paths);
    }
    let imported: std::collections::BTreeSet<_> = paths
        .iter()
        .flat_map(|path| {
            contract::source_graph(path)
                .sources
                .into_iter()
                .skip(1)
                .map(|source| source.path)
        })
        .collect();
    let mut failures = Vec::new();
    let mut compiled = 0;
    for path in paths.iter().filter(|path| !imported.contains(*path)) {
        match contract::compile_path_mapped(path) {
            Ok((_, map)) => {
                compiled += 1;
                for (file, span) in map.button_migrations() {
                    failures.push(format!(
                        "{}:{}:{}: styled default button needs appearance=\"none\"",
                        file.display(),
                        span.line,
                        span.col
                    ));
                }
                if path.file_name().is_some_and(|n| n == "terminal.contract") {
                    if let Err(all) = contract::compile_path_terminal(path) {
                        failures.extend(all.into_iter().map(|e| e.to_string()));
                    }
                }
            }
            Err(e) => failures.push(e.to_string()),
        }
    }
    eprintln!(
        "source sweep of {}: {compiled} roots, {} imported source files, {} failures",
        dirs.join(", "),
        imported.len(),
        failures.len()
    );
    (compiled, failures)
}

#[test]
fn every_app_corpus_and_example_compiles_and_the_button_migration_is_complete() {
    let (compiled, failures) = sweep(&["apps", "contract/corpus", "examples"], &[T3_CODE]);
    assert!(compiled > 80, "no roots found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[ignore = "async lane: compiles the T3 Code clone's roots, about 300 s in a debug build; bun scripts/async.mjs runs it"]
fn the_t3_code_example_compiles_and_its_button_migration_is_complete() {
    // Its generated sources are built, not committed (rules/RULES.md): make them first.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let generated =
        std::process::Command::new(std::env::var_os("BUN").unwrap_or_else(|| "bun".into()))
            .arg(root.join(T3_CODE).join("tools/generate/generate.mjs"))
            .current_dir(&root)
            .status()
            .expect("Bun runs the T3 Code clone's generator (name it with BUN)");
    assert!(
        generated.success(),
        "{T3_CODE}/tools/generate/generate.mjs failed"
    );
    let (compiled, failures) = sweep(&[T3_CODE], &[]);
    assert!(compiled > 0, "no roots found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
