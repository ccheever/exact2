//! D9's source sweep: imported components compile in their application, where
//! their props and app-local native modules have meaning.

use std::collections::BTreeSet;
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

fn imported_sources(
    paths: &[PathBuf],
    mut graph: impl FnMut(&Path) -> contract::SourceGraph,
) -> BTreeSet<PathBuf> {
    let mut paths: Vec<_> = paths.iter().collect();
    // Try conventional roots first; every other candidate still gets a turn.
    paths.sort_by_key(|path| path.file_name().is_none_or(|name| name != "app.contract"));
    let mut completed = BTreeSet::new();
    let mut imported = BTreeSet::new();
    for path in paths {
        if completed.contains(path) {
            continue;
        }
        let graph = graph(path);
        // A failed load may have stopped before a file's remaining imports.
        if graph.errors.is_empty() {
            completed.extend(graph.sources.iter().map(|source| source.path.clone()));
        }
        imported.extend(graph.sources.into_iter().skip(1).map(|source| source.path));
    }
    imported
}

#[test]
fn source_sweep_does_not_reload_completed_imports() {
    let dir = std::env::temp_dir().join(format!("exact-sweep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let app = dir.join("app.contract");
    let branch = dir.join("a-branch.contract");
    let leaf = dir.join("leaf.contract");
    let other = dir.join("other.contract");
    std::fs::write(
        &app,
        "use Branch from \"./a-branch.contract\"\ncomponent App\n  view\n    Branch()\n",
    )
    .unwrap();
    std::fs::write(
        &branch,
        "use Leaf from \"./leaf.contract\"\ncomponent Branch\n  view\n    Leaf()\n",
    )
    .unwrap();
    std::fs::write(&leaf, "component Leaf\n  view\n    text \"leaf\"\n").unwrap();
    std::fs::write(
        &other,
        "use Leaf from \"./leaf.contract\"\ncomponent Other\n  view\n    Leaf()\n",
    )
    .unwrap();
    let paths = vec![branch.clone(), app.clone(), leaf.clone(), other.clone()];
    let mut read = Vec::new();
    let imported = imported_sources(&paths, |path| {
        read.push(path.to_path_buf());
        contract::source_graph(path)
    });
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(imported, BTreeSet::from([branch, leaf]));
    assert_eq!(
        read,
        vec![app, other],
        "completed imports must not be loaded and scoped again"
    );
}

#[test]
fn source_sweep_revisits_partial_import_graphs() {
    let dir = std::env::temp_dir().join(format!("exact-sweep-partial-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    let app = dir.join("app.contract");
    let broken = dir.join("broken.contract");
    let branch = dir.join("branch.contract");
    let leaf = dir.join("leaf.contract");
    std::fs::write(&app, "use Broken from \"./broken.contract\"\nuse Branch from \"./branch.contract\"\ncomponent App\n  view\n    Branch()\n").unwrap();
    std::fs::write(&broken, "use Leaf from \"./leaf.contract\"\nuse Missing from \"./missing.contract\"\ncomponent Broken\n  view\n    Leaf()\n").unwrap();
    std::fs::write(
        &branch,
        "use Leaf from \"./leaf.contract\"\ncomponent Branch\n  view\n    Leaf()\n",
    )
    .unwrap();
    std::fs::write(&leaf, "component Leaf\n  view\n    text \"leaf\"\n").unwrap();
    let paths = vec![app.clone(), broken.clone(), branch.clone(), leaf.clone()];
    let mut read = Vec::new();
    let imported = imported_sources(&paths, |path| {
        read.push(path.to_path_buf());
        contract::source_graph(path)
    });
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(imported, BTreeSet::from([broken.clone(), leaf]));
    assert_eq!(read, vec![app, broken, branch]);
}

// The design-tool stub is a server fixture. Its regen.mjs stages these two
// inputs under the conventional app names before compiling them together.
fn compile_sweep_root(
    root: &Path,
    path: &Path,
) -> Result<(exact_plan::Plan, contract::SourceMap), contract::CompileError> {
    if path != root.join("apps/design-tool/stub/app.contract") {
        return contract::compile_path_mapped(path);
    }
    let dir = std::env::temp_dir().join(format!("exact-sweep-design-stub-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    struct Staged(std::path::PathBuf);
    impl Drop for Staged {
        fn drop(&mut self) {
            let removed = std::fs::remove_dir_all(&self.0);
            if !std::thread::panicking() {
                removed.unwrap();
            }
        }
    }
    let staged = Staged(dir);
    std::fs::copy(path, staged.0.join("app.contract")).unwrap();
    std::fs::copy(
        path.with_file_name("dials.tamber"),
        staged.0.join("app.tamber"),
    )
    .unwrap();
    let result = contract::compile_path_mapped(&staged.0.join("app.contract"));
    result.map(|(plan, mut map)| {
        map.relocate_sources(&staged.0, path.parent().unwrap())
            .unwrap();
        (plan, map)
    })
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
    let imported = imported_sources(&paths, contract::source_graph);
    let mut failures = Vec::new();
    let mut compiled = 0;
    for path in paths.iter().filter(|path| !imported.contains(*path)) {
        match compile_sweep_root(&root, path) {
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
