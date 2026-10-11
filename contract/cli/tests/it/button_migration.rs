//! D9's source sweep: imported components compile in their application, where
//! their props and app-local native modules have meaning.

use std::path::{Path, PathBuf};

fn sources(path: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "contract")
            && !path.to_string_lossy().ends_with(".test.contract")
        {
            out.push(path);
        }
    }
}

#[test]
fn every_app_corpus_and_example_compiles_and_the_button_migration_is_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut paths = Vec::new();
    for dir in ["apps", "contract/corpus", "examples"] {
        sources(&root.join(dir), &mut paths);
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
        "source sweep: {compiled} roots, {} imported source files, {} failures",
        imported.len(),
        failures.len()
    );
    assert!(compiled > 80, "no roots found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
