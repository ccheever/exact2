//! Walk a project's module graph exactly as `ibex2 build` would — TypeScript
//! strip, ESM lowering, dependency scan, resolve — but compile nothing and
//! report every failure instead of stopping at the first.
//!
//! The measurement instrument behind LLP 0066: run it at an application's
//! entry and it says which modules cannot be lowered, which specifiers cannot
//! be resolved and by whom, and which modules are boot-eager (reachable by
//! static edges) against lazy (reachable only through a dynamic `import()`).
//!
//!     cargo run -p ibex2 --example graph -- <root> <entry> [out-dir]
//!
//! With an out-dir, `eager.txt` and `lazy.txt` list the two sets, which is
//! what the API-usage counts in LLP 0066 §3 were taken over. No engine is
//! needed: this is the loader alone.
use ibex2::loader::{self, Root};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

/// `require('...')` specifiers by scanning, as the build walk does.
fn requires_in(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("require(") {
        rest = &rest[at + "require(".len()..];
        let t = rest.trim_start();
        let quote = match t.chars().next() {
            Some(q @ ('\'' | '"')) => q,
            _ => continue,
        };
        let body = &t[1..];
        if let Some(end) = body.find(quote) {
            found.push(body[..end].to_string());
        }
    }
    found
}

/// Every static and dynamic dependency of one module's JavaScript.
fn dependencies(javascript: &str, spec: &str) -> Vec<(String, bool)> {
    let mut deps: Vec<(String, bool)> = ibex2::esm::dependencies(javascript, spec)
        .into_iter()
        .map(|d| (d, true))
        .collect();
    deps.extend(requires_in(javascript).into_iter().map(|d| (d, true)));
    deps.extend(
        ibex2::esm::dynamic_dependencies(javascript, spec)
            .into_iter()
            .map(|d| (d, false)),
    );
    deps
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root_path = PathBuf::from(&args[1]).canonicalize().expect("root");
    let entry = PathBuf::from(&args[2]).canonicalize().expect("entry");
    let name = format!(
        "./{}",
        entry
            .strip_prefix(&root_path)
            .expect("entry under root")
            .display()
    );
    let root = Root::Declared(root_path.clone());
    let cache = loader::ResolveCache::default();
    let out_dir: Option<PathBuf> = args.get(3).map(PathBuf::from);

    let mut queue = VecDeque::from([name.clone()]);
    let mut seen = BTreeSet::new();
    let mut lowered_ok = 0usize;
    let mut bytes = 0usize;
    let mut ext_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut in_node_modules = 0usize;
    let mut read_errors: Vec<(String, String)> = Vec::new();
    let mut lower_errors: Vec<(String, String)> = Vec::new();
    // specifier -> [(importer, error)]
    let mut resolve_errors: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut dyn_unresolved: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    // Static edges, for the eager/lazy split below.
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();

    while let Some(spec) = queue.pop_front() {
        if !seen.insert(spec.clone()) {
            continue;
        }
        let path = root_path.join(spec.trim_start_matches("./"));
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                read_errors.push((spec.clone(), e.to_string()));
                continue;
            }
        };
        bytes += source.len();
        if spec.contains("/node_modules/") {
            in_node_modules += 1;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        *ext_counts.entry(ext).or_default() += 1;

        let javascript = match loader::to_javascript(&source, &spec) {
            Ok(j) => j,
            Err(e) => {
                lower_errors.push((spec.clone(), format!("strip: {e}")));
                continue;
            }
        };
        match loader::lower_and_wrap(&source, &spec) {
            Ok(_) => lowered_ok += 1,
            Err(e) => lower_errors.push((spec.clone(), format!("lower: {e}"))),
        }

        for (dep, required) in dependencies(&javascript, &spec) {
            let outcome = loader::resolve_in(&cache, &root, &spec, &dep).and_then(|resolved| {
                if root_path.join(resolved.trim_start_matches("./")).is_file() {
                    Ok(resolved)
                } else {
                    Err(format!("resolved to {resolved}, which does not exist"))
                }
            });
            match outcome {
                Ok(resolved) => {
                    if required {
                        edges
                            .entry(spec.clone())
                            .or_default()
                            .push(resolved.clone());
                    }
                    queue.push_back(resolved);
                }
                Err(e) => {
                    let bucket = if required {
                        &mut resolve_errors
                    } else {
                        &mut dyn_unresolved
                    };
                    bucket
                        .entry(dep.clone())
                        .or_default()
                        .push((spec.clone(), e));
                }
            }
        }
    }

    // Eagerness propagates along static edges from the entry.
    let mut eager: BTreeSet<String> = BTreeSet::from([name]);
    loop {
        let before = eager.len();
        let additions: Vec<String> = eager
            .iter()
            .flat_map(|s| edges.get(s).cloned().unwrap_or_default())
            .collect();
        eager.extend(additions);
        if eager.len() == before {
            break;
        }
    }
    let eager_count = seen.iter().filter(|s| eager.contains(*s)).count();
    if let Some(dir) = &out_dir {
        std::fs::create_dir_all(dir).expect("out dir");
        let (eager_list, lazy_list): (Vec<&String>, Vec<&String>) =
            seen.iter().partition(|s| eager.contains(*s));
        let join = |v: Vec<&String>| v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n");
        std::fs::write(dir.join("eager.txt"), join(eager_list)).expect("eager.txt");
        std::fs::write(dir.join("lazy.txt"), join(lazy_list)).expect("lazy.txt");
    }

    println!("== graph reached from entry ==");
    println!(
        "boot-eager (static edges only): {eager_count}   lazy (only via dynamic import): {}",
        seen.len() - eager_count
    );
    println!(
        "modules: {}  ({in_node_modules} under node_modules)  bytes: {bytes}  lowered ok: {lowered_ok}",
        seen.len()
    );
    println!("by extension: {ext_counts:?}");
    println!();
    println!("== read errors: {} ==", read_errors.len());
    for (s, e) in &read_errors {
        println!("  {s}: {e}");
    }
    println!();
    println!("== strip/lower errors: {} ==", lower_errors.len());
    for (s, e) in &lower_errors {
        println!("  {s}: {e}");
    }
    println!();
    println!(
        "== unresolvable static imports/requires: {} distinct specifiers ==",
        resolve_errors.len()
    );
    for (dep, sites) in &resolve_errors {
        println!(
            "  {dep}  [{} importer(s)]  e.g. {} -> {}",
            sites.len(),
            sites[0].0,
            sites[0].1
        );
    }
    println!();
    println!(
        "== unresolvable dynamic imports: {} distinct specifiers ==",
        dyn_unresolved.len()
    );
    for (dep, sites) in &dyn_unresolved {
        println!(
            "  {dep}  [{} importer(s)]  e.g. {} -> {}",
            sites.len(),
            sites[0].0,
            sites[0].1
        );
    }
}
