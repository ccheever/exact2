//! `ibex2` — run a JavaScript program.
//!
//! Deliberately small. LLP 0057 §5 observes that ibex 1 carries 137K lines of
//! CLI against a 152K-line runtime, and that `run`, `build`, and perhaps `repl`
//! is the whole surface a runtime needs. This is `run`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ibex2::loader::Root;

use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::ModuleGrants;

fn usage() -> &'static str {
    "usage: ibex2 run   <entry.js> [--root <dir>] [--grants <file>] [--budget-ms <n>]\n\
    \x20                        [--precompiled] [--no-compile]\n\
    \x20      ibex2 build <entry.js> [--root <dir>]\n\
     \n\
     run    Runs <entry.js>. Each module receives only the capabilities its\n\
     \x20      grant manifest names; without --grants, nothing is granted.\n\
     \x20      Modules are compiled to bytecode and cached under .ibex2/cache;\n\
     \x20      --precompiled refuses to compile anything on demand, and\n\
     \x20      --no-compile falls back to loading source.\n\
     \n\
     build  Compiles the whole graph to bytecode ahead of time, so `run\n\
     \x20      --precompiled` has everything it needs. rules/RULES.md forbids\n\
     \x20      compiling at runtime what could be built ahead of it.\n\
     \n\
     --root <dir>\n\
     \x20      The project. Modules cannot resolve outside it, and package\n\
     \x20      (bare) specifiers like `react` or `@scope/pkg` are ONLY resolved\n\
     \x20      when it is given — there is otherwise no project to look in, and\n\
     \x20      guessing one from a stray package.json or node_modules is how a\n\
     \x20      containment boundary quietly becomes your home directory.\n\
     \x20      Without it the boundary is the entry file's own directory, which\n\
     \x20      runs a self-contained program fine. Grant manifest keys are paths\n\
     \x20      from the root, so moving it changes what they name."
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("");
    if !matches!(command, "run" | "build") || args.len() < 2 {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    }

    let entry = PathBuf::from(&args[1]);
    let mut grants_path: Option<PathBuf> = None;
    let mut budget_ms: u64 = 30_000;
    let mut precompiled_only = false;
    let mut compile = true;
    let mut declared_root: Option<PathBuf> = None;
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--grants" => match rest.next() {
                Some(path) => grants_path = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--grants needs a file");
                    return ExitCode::from(2);
                }
            },
            "--budget-ms" => match rest.next().and_then(|v| v.parse().ok()) {
                Some(ms) => budget_ms = ms,
                None => {
                    eprintln!("--budget-ms needs a number");
                    return ExitCode::from(2);
                }
            },
            "--root" => match rest.next() {
                Some(path) => declared_root = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--root needs a directory");
                    return ExitCode::from(2);
                }
            },
            "--precompiled" => precompiled_only = true,
            "--no-compile" => compile = false,
            other => {
                eprintln!("unknown flag {other}\n\n{}", usage());
                return ExitCode::from(2);
            }
        }
    }

    let outcome = match command {
        "build" => build(&entry, declared_root.as_deref()),
        _ => run(
            &entry,
            declared_root.as_deref(),
            grants_path.as_deref(),
            budget_ms,
            compile,
            precompiled_only,
        ),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ibex2: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Work out the containment boundary and the entry's name within it.
///
/// With `--root`, the author has said what the project is: packages resolve,
/// and the entry is named relative to that root — `./apps/mobile/index.ts`,
/// not `./index.ts` — because a specifier is a path from the root and grants
/// are keyed by it.
///
/// Without it, the boundary falls back to the entry's own directory, which is
/// enough to run a self-contained program and not enough to say where a
/// package would come from. `loader::Root` carries that distinction so bare
/// specifiers can be refused with an explanation instead of resolved against a
/// boundary nobody chose.
///
/// @ref LLP 0065#5-the-root-must-be-declared
fn project_root(entry: &Path, declared: Option<&Path>) -> Result<(Root, String), String> {
    let entry = entry
        .canonicalize()
        .map_err(|e| format!("cannot open {}: {e}", entry.display()))?;
    match declared {
        Some(dir) => {
            let dir = dir
                .canonicalize()
                .map_err(|e| format!("cannot open --root {}: {e}", dir.display()))?;
            let relative = entry.strip_prefix(&dir).map_err(|_| {
                format!("{} is not inside --root {}", entry.display(), dir.display())
            })?;
            let name = format!("./{}", relative.to_string_lossy().replace('\\', "/"));
            Ok((Root::Declared(dir), name))
        }
        None => {
            let dir = entry
                .parent()
                .ok_or("entry has no directory")?
                .to_path_buf();
            let name = entry
                .file_name()
                .ok_or("entry has no file name")?
                .to_string_lossy()
                .into_owned();
            Ok((Root::EntryDirectory(dir), format!("./{name}")))
        }
    }
}

/// Where compiled modules live: beside the project, visible and deletable.
fn cache_dir(root: &Path) -> PathBuf {
    root.join(".ibex2/cache")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The engine these artifacts are built for.
fn engine_dir() -> PathBuf {
    match std::env::var("IBEX2_VANILLA_HERMES_DIR") {
        Ok(path) => PathBuf::from(path),
        Err(_) if cfg!(target_vendor = "apple") => repo_root().join("ios/Frameworks-vanilla"),
        Err(_) => repo_root().join("linux/Frameworks-vanilla"),
    }
}

/// The build's compiler: receipt required and verified against the engine
/// and hermesc on disk, artifacts keyed to the engine this binary links
/// (LLP 0058.000.001 §5).
#[cfg(feature = "loader")]
fn compiler_for_build(root: &Path) -> Result<ibex2::bytecode::Compiler, String> {
    ibex2::bytecode::Compiler::discover_for_engine(
        &repo_root(),
        cache_dir(root),
        &engine_dir(),
        true,
    )
}

/// A run's compiler: nothing hashed, and no compiler at all under
/// `--precompiled`, where a module missing from the manifest is refused.
fn compiler_for_run(
    root: &Path,
    precompiled_only: bool,
) -> Result<ibex2::bytecode::Compiler, String> {
    ibex2::bytecode::Compiler::for_run(
        &repo_root(),
        cache_dir(root),
        &engine_dir(),
        precompiled_only,
    )
}

/// Compile the whole reachable graph ahead of time.
#[cfg(not(feature = "loader"))]
fn build(_entry: &Path, _declared_root: Option<&Path>) -> Result<(), String> {
    Err("this build has no loader and cannot build; use a full build of ibex2".into())
}

#[cfg(feature = "loader")]
fn build(entry: &Path, declared_root: Option<&Path>) -> Result<(), String> {
    let (root, name) = project_root(entry, declared_root)?;
    // A build produces artifacts others will trust, so it requires the receipt.
    let compiler = compiler_for_build(&root)?;

    // Walk from the entry, following require() and import as the loader would.
    // Compiling every .js under the root instead would build files nothing
    // imports.
    // `name` is already a specifier — `./apps/mobile/index.ts` — because it is
    // a path from the root, not a bare file name. Prefixing it again produced
    // `././...`, which the loader then never matched at run time.
    let cache = ibex2::loader::ResolveCache::default();
    let mut queue = vec![name.clone()];
    let mut seen = std::collections::BTreeSet::new();
    let mut edges: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    // Bound to the engine this binary links, so another binary refuses it.
    let mut manifest =
        ibex2::bytecode::Manifest::for_engine(ibex2::bytecode::Compiler::linked_engine());
    let mut artifacts: Vec<(String, Vec<u8>)> = Vec::new();
    // The entry is loaded from "./" — the runtime's name for "no importer".
    manifest.insert_edge("./", &name, &name);
    let mut warnings: Vec<String> = Vec::new();
    let mut built = 0usize;

    while let Some(specifier) = queue.pop() {
        if !seen.insert(specifier.clone()) {
            continue;
        }
        let path = root.join(specifier.trim_start_matches("./"));
        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        // Scan the JavaScript this module *becomes*, not the source as written:
        // the JSX transform injects an import of `react/jsx-runtime` that
        // appears nowhere in the .tsx. Scanning the original silently produced
        // an incomplete graph that only failed later, at `run --precompiled`.
        let javascript = ibex2::loader::to_javascript(&source, &specifier)?;
        let wrapped = ibex2::loader::lower_and_wrap(&source, &specifier)?;
        let bytes = compiler
            .compile(&wrapped)
            .map_err(|e| format!("{specifier}: {e}"))?;
        // Recorded so the runtime finds this artifact without opening, reading,
        // or hashing the source again — and bundled, so it reads them all at once.
        manifest.insert(&specifier, &compiler.key(&wrapped));
        artifacts.push((compiler.key(&wrapped), bytes));
        built += 1;

        // LLP 0064 §3.1: an importer writing `import { n }` snapshots, so a
        // reassignment of `n` is invisible to it. The divergence is silent;
        // this is where it stops being.
        for binding in ibex2::esm::mutable_exports(&source, &specifier) {
            warnings.push(format!(
                "{specifier} exports `{binding}` and reassigns it. An importer writing \
                 `import {{ {binding} }}` will see a stale value; `import * as ns` reads \
                 through and is live (LLP 0064 §3.1)."
            ));
        }

        // Both module systems: import/export-from come from the parser,
        // require from a scan, because a require's specifier is an ordinary
        // call argument no module-syntax parse reports.
        let mut dependencies: Vec<(String, bool)> =
            ibex2::esm::dependencies(&javascript, &specifier)
                .into_iter()
                .map(|dependency| (dependency, true))
                .collect();
        dependencies.extend(
            requires_in(&javascript)
                .into_iter()
                .map(|dependency| (dependency, true)),
        );
        // A literal dynamic import is worth building — otherwise it is missing
        // from the manifest and fails under --precompiled — but it is
        // CONDITIONAL where a static import is not, so a target that does not
        // exist is a warning rather than a build failure. Code that guards an
        // optional import is correct.
        dependencies.extend(
            ibex2::esm::dynamic_dependencies(&javascript, &specifier)
                .into_iter()
                .map(|dependency| (dependency, false)),
        );

        for (dependency, required) in dependencies {
            match ibex2::loader::resolve_in(&cache, &root, &specifier, &dependency) {
                Ok(resolved) => {
                    if !required && !root.join(resolved.trim_start_matches("./")).exists() {
                        warnings.push(format!(
                            "{specifier} dynamically imports {dependency:?}, which does not \
                             exist. The call rejects at run time; it is not built."
                        ));
                        continue;
                    }
                    // Recorded, so a run resolves nothing the build already did.
                    manifest.insert_edge(&specifier, &dependency, &resolved);
                    edges
                        .entry(specifier.clone())
                        .or_default()
                        .push(resolved.clone());
                    queue.push(resolved);
                }
                // A require() this scan cannot resolve is not a build failure:
                // it may be inside a branch that never runs, and the loader
                // will report it properly if it is ever reached.
                Err(_) => continue,
            }
        }
    }

    // LLP 0064 §3.2: a cycle observes partial exports rather than raising a
    // ReferenceError, so one that works today can start returning undefined
    // after an unrelated reordering. Reported here because the graph is already
    // in hand and nothing else in the system will ever mention it.
    for cycle in find_cycles(&edges) {
        warnings.push(format!(
            "import cycle: {}. Cycles resolve to partial exports rather than failing, \
             so this will not error if the order changes (LLP 0064 §3.2).",
            cycle.join(" -> ")
        ));
    }

    for warning in &warnings {
        eprintln!("ibex2: warning: {warning}");
    }

    manifest.write(&cache_dir(&root))?;
    ibex2::bytecode::Bundle::write(&cache_dir(&root), &artifacts)?;
    println!("built {built} modules into {}", cache_dir(&root).display());
    Ok(())
}

/// Every import cycle in the graph, each reported once from its entry point.
#[cfg(feature = "loader")]
fn find_cycles(edges: &std::collections::BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let mut reported = std::collections::BTreeSet::new();
    let mut visited = std::collections::BTreeSet::new();

    // Iterative depth-first search carrying its own path, so a deep graph
    // cannot overflow the stack and the cycle can be named rather than merely
    // detected.
    for start in edges.keys() {
        if visited.contains(start) {
            continue;
        }
        let mut stack = vec![(start.clone(), 0usize)];
        let mut path = vec![start.clone()];
        while let Some((node, index)) = stack.pop() {
            let children = edges.get(&node).map(Vec::as_slice).unwrap_or(&[]);
            if index < children.len() {
                stack.push((node.clone(), index + 1));
                let child = &children[index];
                if let Some(at) = path.iter().position(|seen| seen == child) {
                    let mut cycle: Vec<String> = path[at..].to_vec();
                    cycle.push(child.clone());
                    // Normalize so the same cycle found from two entry points
                    // is reported once.
                    let mut key: Vec<String> = cycle[..cycle.len() - 1].to_vec();
                    key.sort();
                    if reported.insert(key) {
                        found.push(cycle);
                    }
                    continue;
                }
                path.push(child.clone());
                stack.push((child.clone(), 0));
            } else {
                visited.insert(node);
                path.pop();
            }
        }
    }
    found
}

#[cfg(feature = "loader")]
/// Find `require('...')` specifiers by scanning.
///
/// A scanner, not a parser: it can see a require inside a comment or a string
/// and will compile a module nothing imports. That is wasted work rather than a
/// wrong result. ES module specifiers come from the parser instead — see
/// `esm::dependencies`.
fn requires_in(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("require(") {
        rest = &rest[at + "require(".len()..];
        let rest_trimmed = rest.trim_start();
        let quote = match rest_trimmed.chars().next() {
            Some(q @ ('\'' | '"')) => q,
            _ => continue,
        };
        let body = &rest_trimmed[1..];
        if let Some(end) = body.find(quote) {
            found.push(body[..end].to_string());
        }
    }
    found
}

fn run(
    entry: &Path,
    declared_root: Option<&Path>,
    grants_path: Option<&Path>,
    budget_ms: u64,
    compile: bool,
    precompiled_only: bool,
) -> Result<(), String> {
    let (root, name) = project_root(entry, declared_root)?;
    #[cfg(not(feature = "loader"))]
    if !precompiled_only {
        return Err(
            "this build has no loader: run with --precompiled, on a graph built by a full build"
                .into(),
        );
    }

    let grants = match grants_path {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            ModuleGrants::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?
        }
        // No manifest means no authority — not ambient authority. A program
        // that needs the network must say so.
        None => ModuleGrants::none(),
    };

    let mut rt = Hermes::new(DynamicCode::Closed).ok_or("could not create a runtime")?;
    // What the engine itself puts on the global object, before anything of
    // ours: the baseline R5 subtracts. Everything else must be in
    // ALLOWED_GLOBALS by name — no prefix, no "looks like an intrinsic".
    let baseline: std::collections::BTreeSet<String> = rt.global_names().into_iter().collect();
    if !rt.install_stdlib() {
        return Err("could not install the standard library".into());
    }
    rt.install_bindings().map_err(|e| e.to_string())?;
    let compiler = if compile || precompiled_only {
        match compiler_for_run(&root, precompiled_only) {
            Ok(compiler) => Some(compiler),
            Err(message) if precompiled_only => return Err(message),
            // Without hermesc the runtime can still load source. That path is
            // forbidden for anything shippable (rules/RULES.md) and says so.
            Err(message) => {
                eprintln!("ibex2: {message}");
                eprintln!("ibex2: falling back to loading source; this compiles at runtime");
                None
            }
        }
    } else {
        None
    };
    rt.set_loader_with(root, grants, compiler, precompiled_only)?;

    // R4: intrinsics frozen after the standard library is installed and before
    // any module code runs.
    rt.harden().map_err(|e| e.to_string())?;

    // R5: assert what the global object carries, before running anything.
    let unexpected: Vec<String> = rt
        .global_names()
        .into_iter()
        .filter(|name| {
            !baseline.contains(name) && !ibex2::loader::ALLOWED_GLOBALS.contains(&name.as_str())
        })
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "refusing to run: unexpected globals {unexpected:?} (LLP 0067 R1)"
        ));
    }

    rt.run_entry(&name).map_err(|e| e.to_string())?;
    rt.run_to_quiescence(std::time::Duration::from_millis(budget_ms));

    for record in rt.drain_console() {
        match record.level {
            ibex2::stdlib::console::Level::Error | ibex2::stdlib::console::Level::Warn => {
                eprintln!("{}", record.message)
            }
            _ => println!("{}", record.message),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ibex2-bin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("apps/mobile")).unwrap();
        std::fs::write(dir.join("apps/mobile/index.ts"), "").unwrap();
        dir
    }

    /// `project_root` returns a *specifier*, already `./`-prefixed — not a bare
    /// file name. Regression test: `build` prefixed `./` a second time, wrote
    /// `././apps/mobile/index.ts` into the manifest, and `run --precompiled`
    /// then failed to find the entry it had just compiled. Only visible with a
    /// declared root and a nested entry, which is exactly the monorepo case.
    #[test]
    fn a_declared_root_names_the_entry_relative_to_it() {
        let dir = scratch("declared");
        let (root, name) = project_root(&dir.join("apps/mobile/index.ts"), Some(&dir)).unwrap();
        assert_eq!(name, "./apps/mobile/index.ts");
        assert!(root.packages_resolvable());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_undeclared_root_is_the_entry_directory_and_refuses_packages() {
        let dir = scratch("undeclared");
        let (root, name) = project_root(&dir.join("apps/mobile/index.ts"), None).unwrap();
        assert_eq!(name, "./index.ts");
        assert!(
            !root.packages_resolvable(),
            "an inferred root must not resolve packages"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The root is a containment boundary, so an entry outside it is a
    /// contradiction rather than something to accommodate.
    #[test]
    fn an_entry_outside_the_declared_root_is_refused() {
        let dir = scratch("outside");
        let entry = dir.join("apps/mobile/index.ts");
        assert!(
            project_root(&entry, Some(&dir.join("apps/mobile"))).is_ok(),
            "the entry is inside this root"
        );

        std::fs::create_dir_all(dir.join("other")).unwrap();
        let err = project_root(&entry, Some(&dir.join("other")))
            .expect_err("an entry outside the root must be refused");
        assert!(err.contains("is not inside --root"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
