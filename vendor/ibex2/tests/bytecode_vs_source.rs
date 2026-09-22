//! Does ahead-of-time bytecode remove the per-compile-unit cost, or only the
//! parse?
//!
//! `eval_overhead.rs` shows a fixed ~2ms per `evaluateJavaScript` call, which
//! is what makes a 570-module graph take most of a second. Whether that cost is
//! PARSING or COMPILE-UNIT CREATION decides the loader's architecture:
//!
//!   - if parsing, bytecode alone fixes it and modules can stay separate;
//!   - if unit creation, bundling is required as well, and 570 .hbc files would
//!     be no better than 570 .js files.
//!
//! Requires hermesc:
//!     ./scripts/build-hermes.sh --vanilla   (installs tools/hermes-vanilla/)
//!
//!     cargo test -p ibex2 --features hermes --release --test bytecode_vs_source -- --ignored --nocapture

#![cfg(all(feature = "hermes", feature = "loader"))]

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use ibex2::engine::hermes::{DynamicCode, Hermes};

fn hermesc() -> Option<PathBuf> {
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(format!("tools/hermes-vanilla/hermesc-macos-{arch}"));
    path.exists().then_some(path)
}

/// ~9.6KB of plausible JavaScript, wrapped as a function expression so the
/// script's completion value is the module function — the shape the loader
/// actually evaluates.
fn module_unit(i: usize) -> String {
    let mut body = String::new();
    let mut n = 0;
    while body.len() < 9_600 {
        body.push_str(&format!(
            "function h{i}_{n}(a,b){{const s=a*{n}+b;if(s>100)return{{k:'l',s}};return{{k:'s',s}};}}\n\
             const t{i}_{n}={{id:{n},ap:h{i}_{n}}};\n"
        ));
        n += 1;
    }
    // Every helper is REACHABLE from exports, or `-O` deletes all of them and
    // the comparison measures empty modules. A first version of this generator
    // ended in `return 0`, and 9.7KB of source with 90 functions compiled to
    // 224 bytes containing 2 — which made bytecode look free because there was
    // nothing left to load.
    let keep: Vec<String> = (0..n).map(|k| format!("t{i}_{k}")).collect();
    format!(
        "(function (module, exports, require, fetch) {{\n{body}\n\
         exports.table = [{}];\n\
         exports.run = () => exports.table[0].ap(1, 2);\n\
         return {i};\n}});\n",
        keep.join(", ")
    )
}

fn compile(hermesc: &PathBuf, source: &str, out: &PathBuf) -> Vec<u8> {
    let js = out.with_extension("js");
    std::fs::write(&js, source).expect("write js");
    let status = Command::new(hermesc)
        .args(["-emit-binary", "-O", "-out"])
        .arg(out)
        .arg(&js)
        .status()
        .expect("run hermesc");
    assert!(status.success(), "hermesc failed for {}", js.display());
    std::fs::read(out).expect("read hbc")
}

#[test]
#[ignore]
fn bytecode_versus_source_separate_versus_bundled() {
    let Some(hermesc) = hermesc() else {
        eprintln!("hermesc not found; run ./scripts/build-hermes.sh --vanilla");
        return;
    };

    let count = 570;
    let dir = std::env::temp_dir().join(format!("ibex2-hbc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");

    let sources: Vec<String> = (0..count).map(module_unit).collect();
    let source_bytes: usize = sources.iter().map(String::len).sum();

    eprintln!("compiling {count} modules to bytecode (build-time work)...");
    let separate: Vec<Vec<u8>> = sources
        .iter()
        .enumerate()
        .map(|(i, s)| compile(&hermesc, s, &dir.join(format!("m{i}.hbc"))))
        .collect();
    let bundled_source: String = sources.concat();
    let bundled = compile(&hermesc, &bundled_source, &dir.join("bundle.hbc"));
    let hbc_bytes: usize = separate.iter().map(Vec::len).sum();

    // Guard against measuring nothing. A module of ~9.6KB with ~90 functions
    // must not compile to a few hundred bytes; if it does, the optimizer has
    // eliminated the body and the comparison is meaningless.
    let per_module = hbc_bytes / count;
    assert!(
        per_module > 2_000,
        "bytecode is {per_module} bytes per module — the code was optimized away, \
         so this measurement would be comparing empty programs"
    );

    let mb = |b: usize| b as f64 / 1_048_576.0;
    let mut rows: Vec<(&str, std::time::Duration)> = Vec::new();

    // 1. Source, one evaluation per module — what the loader does today.
    {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("rt");
        let t = Instant::now();
        for s in &sources {
            rt.eval(s).expect("source eval");
        }
        rows.push(("source, 570 units", t.elapsed()));
    }

    // 2. Source, one evaluation — what a bundler gives.
    {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("rt");
        let t = Instant::now();
        rt.eval(&bundled_source).expect("bundled source");
        rows.push(("source, 1 unit", t.elapsed()));
    }

    // 3. Bytecode, one evaluation per module — the question.
    {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("rt");
        let t = Instant::now();
        for b in &separate {
            rt.eval_bytes(b).expect("hbc eval");
        }
        rows.push(("bytecode, 570 units", t.elapsed()));
    }

    // 4. Bytecode, one evaluation — bundled and compiled ahead of time.
    {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("rt");
        let t = Instant::now();
        rt.eval_bytes(&bundled).expect("bundle eval");
        rows.push(("bytecode, 1 unit", t.elapsed()));
    }

    println!(
        "\n=== {count} modules: {:.2}MB source, {:.2}MB bytecode ===",
        mb(source_bytes),
        mb(hbc_bytes)
    );
    for (label, elapsed) in &rows {
        println!(
            "  {label:24} {:>9.2}ms {:>9.3}ms/module",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_secs_f64() * 1000.0 / count as f64
        );
    }
    println!();
    println!("  budget (rules/RULES.md): 30ms to app entry");

    let _ = std::fs::remove_dir_all(&dir);
}
