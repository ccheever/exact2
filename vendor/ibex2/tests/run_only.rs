//! A run-only binary: the `loader` feature off, so no Oxc is compiled or
//! linked. It runs precompiled artifacts from the manifest and the bundle,
//! resolves nothing at run time, and refuses everything else.
//!
//!     cargo test -p ibex2 --no-default-features --features hermes --test run_only
#![cfg(all(feature = "hermes", not(feature = "loader")))]

use ibex2::bytecode::{Bundle, Compiler, Manifest};
use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::{self, ModuleGrants, Root};

#[test]
fn a_run_only_binary_runs_a_precompiled_graph_and_nothing_else() {
    let dir = std::env::temp_dir().join(format!("ibex2-run-only-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // CommonJS, which needs no lowering — a full build would have lowered
    // anything else before it got here.
    let modules = [
        ("./index.js", "console.log(require('./a').n + 1);"),
        ("./a.js", "exports.n = 41;"),
    ];
    for (spec, source) in modules {
        std::fs::write(dir.join(spec.trim_start_matches("./")), source).unwrap();
    }
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cache = dir.join(".ibex2/cache");
    let Ok(compiler) = Compiler::discover(&repo, cache.clone()) else {
        return;
    };
    let mut manifest = Manifest::for_engine(Compiler::linked_engine());
    manifest.insert_edge("./", "./index.js", "./index.js");
    manifest.insert_edge("./index.js", "./a", "./a.js");
    let mut artifacts = Vec::new();
    for (spec, source) in modules {
        let wrapped = loader::wrap(source);
        let bytes = compiler.compile(&wrapped).unwrap();
        manifest.insert(spec, &compiler.key(&wrapped));
        artifacts.push((compiler.key(&wrapped), bytes));
    }
    manifest.write(&cache).unwrap();
    Bundle::write(&cache, &artifacts).unwrap();

    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib());
    rt.install_bindings().expect("bindings");
    rt.set_loader_with(
        Root::Declared(dir.clone()),
        ModuleGrants::none(),
        Some(compiler),
        true,
    )
    .expect("loader");
    rt.harden().expect("harden");
    rt.run_entry("./index.js").expect("entry");
    rt.run_to_quiescence(std::time::Duration::from_secs(5));
    let out: Vec<String> = rt.drain_console().into_iter().map(|r| r.message).collect();
    assert_eq!(out, vec!["42"]);

    // Nothing else: no source, no TypeScript, no package resolution.
    assert!(loader::lower_and_wrap("export const x = 1;", "./x.js")
        .unwrap_err()
        .contains("no loader"));
    assert!(loader::to_javascript("const x: number = 1;", "./x.ts")
        .unwrap_err()
        .contains("no loader"));
    assert!(
        loader::resolve(&Root::Declared(dir.clone()), "./index.js", "react")
            .unwrap_err()
            .contains("resolves nothing")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
