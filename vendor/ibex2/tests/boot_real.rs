//! A real boot: runtime construction through to application code running.
//!
//! The floor (`boot_floor.rs`) measures what a program pays before its own code
//! exists. This measures what it pays to actually load and run one, at the
//! scale LLP 0057 §1 records for Exact's real graph: 570 modules, 5.47MB —
//! about 9.6KB per module.
//!
//! The point is the per-module cost of loading from SOURCE, because that is
//! what LLP 0062 R3's ahead-of-time requirement exists to remove, and the
//! number here is what says how urgent that is.
//!
//!     cargo test -p ibex2 --features hermes --release --test boot_real -- --ignored --nocapture

#![cfg(all(feature = "hermes", feature = "loader"))]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::{ModuleGrants, Root};

/// Roughly the per-module size of Exact's measured graph.
const TARGET_MODULE_BYTES: usize = 9_600;

/// A module body that is plausible JavaScript rather than filler: the parser
/// has to do real work on it, which is the cost being measured.
fn module_source(index: usize, requires: Option<usize>) -> String {
    let mut out = String::with_capacity(TARGET_MODULE_BYTES + 512);
    if let Some(dep) = requires {
        out.push_str(&format!("const dep{dep} = require('./m{dep}');\n"));
    }
    let mut n = 0;
    while out.len() < TARGET_MODULE_BYTES {
        out.push_str(&format!(
            "function helper{index}_{n}(a, b) {{\n  \
               const scaled = a * {n} + b;\n  \
               if (scaled > 100) return {{ kind: 'large', scaled, tag: 'm{index}' }};\n  \
               return {{ kind: 'small', scaled, tag: 'm{index}' }};\n\
             }}\n\
             const table{index}_{n} = {{ id: {n}, name: 'entry-{n}', apply: helper{index}_{n} }};\n"
        ));
        n += 1;
    }
    out.push_str(&format!(
        "exports.id = {index};\nexports.run = () => table{index}_0.apply(1, 2);\n"
    ));
    out
}

struct Graph {
    dir: PathBuf,
    bytes: usize,
}

impl Graph {
    /// A chain, so every module is genuinely loaded through `require` rather
    /// than being dead files on disk.
    fn build(count: usize) -> Self {
        let dir = std::env::temp_dir().join(format!("ibex2-boot-{count}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let mut bytes = 0;
        for i in 0..count {
            let next = if i + 1 < count { Some(i + 1) } else { None };
            let source = module_source(i, next);
            bytes += source.len();
            std::fs::write(dir.join(format!("m{i}.js")), &source).expect("write");
        }
        std::fs::write(
            dir.join("index.js"),
            "const head = require('./m0');\nexports.ok = head.run().tag;\n",
        )
        .expect("write entry");
        Self { dir, bytes }
    }
}

impl Drop for Graph {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Boot {
    floor: Duration,
    modules: Duration,
}

fn boot(root: &Path) -> Boot {
    boot_with(root, None, false)
}

fn compiler_for(root: &Path) -> Option<ibex2::bytecode::Compiler> {
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    ibex2::bytecode::Compiler::discover(&repo, root.join(".ibex2/cache")).ok()
}

fn boot_with(
    root: &Path,
    compiler: Option<ibex2::bytecode::Compiler>,
    precompiled_only: bool,
) -> Boot {
    let t = Instant::now();
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib());
    rt.install_bindings().expect("bindings");
    rt.set_loader_with(
        Root::Declared(root.to_path_buf()),
        ModuleGrants::none(),
        compiler,
        precompiled_only,
    )
    .expect("loader");
    rt.harden().expect("harden");
    let floor = t.elapsed();

    let t = Instant::now();
    rt.run_entry("./index.js").expect("entry");
    let modules = t.elapsed();

    Boot { floor, modules }
}

#[test]
#[ignore]
fn real_boot_by_graph_size() {
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("\n=== Ibex 2 real boot, loading from source (release) ===");
    println!(
        "  {:>7} {:>9} {:>10} {:>11} {:>11} {:>9}",
        "modules", "source", "floor", "modules", "total", "per-mod"
    );

    for count in [1usize, 10, 50, 100, 250, 570] {
        let graph = Graph::build(count);
        // One warm run first: the point is steady-state cost, not page cache.
        let _ = boot(&graph.dir);

        let runs = 5;
        let (mut floor, mut modules) = (Duration::ZERO, Duration::ZERO);
        for _ in 0..runs {
            let b = boot(&graph.dir);
            floor += b.floor;
            modules += b.modules;
        }
        let floor = floor / runs;
        let modules = modules / runs;
        println!(
            "  {:>7} {:>8.2}MB {:>9.2}ms {:>10.2}ms {:>10.2}ms {:>8.3}ms",
            count,
            graph.bytes as f64 / 1_048_576.0,
            ms(floor),
            ms(modules),
            ms(floor + modules),
            ms(modules) / count as f64
        );
    }

    // The same graphs, loaded from ahead-of-time bytecode.
    println!("\n=== the same graphs, from precompiled bytecode ===");
    println!(
        "  {:>7} {:>10} {:>11} {:>11} {:>9}",
        "modules", "floor", "modules", "total", "per-mod"
    );
    for count in [1usize, 10, 100, 570] {
        let graph = Graph::build(count);
        let Some(compiler) = compiler_for(&graph.dir) else {
            println!("  (hermesc not found; run ./scripts/build-hermes.sh --vanilla)");
            break;
        };
        // Build first — including the manifest, so the measured runs never
        // open a source file.
        let mut manifest =
            ibex2::bytecode::Manifest::for_engine(ibex2::bytecode::Compiler::linked_engine());
        for i in 0..count {
            let specifier = format!("./m{i}.js");
            let source = std::fs::read_to_string(graph.dir.join(format!("m{i}.js"))).expect("read");
            let wrapped = ibex2::loader::wrap(&source);
            compiler.compile(&wrapped).expect("compile");
            manifest.insert(&specifier, &compiler.key(&wrapped));
        }
        let entry = std::fs::read_to_string(graph.dir.join("index.js")).expect("read");
        let wrapped = ibex2::loader::wrap(&entry);
        compiler.compile(&wrapped).expect("compile");
        manifest.insert("./index.js", &compiler.key(&wrapped));
        manifest
            .write(&graph.dir.join(".ibex2/cache"))
            .expect("manifest");

        let runs = 5;
        let (mut floor, mut modules) = (Duration::ZERO, Duration::ZERO);
        for _ in 0..runs {
            let b = boot_with(&graph.dir, Some(compiler.clone()), true);
            floor += b.floor;
            modules += b.modules;
        }
        let floor = floor / runs;
        let modules = modules / runs;
        println!(
            "  {:>7} {:>9.2}ms {:>10.2}ms {:>10.2}ms {:>8.3}ms",
            count,
            ms(floor),
            ms(modules),
            ms(floor + modules),
            ms(modules) / count as f64
        );
    }

    println!();
    println!("  budget (rules/RULES.md): 30ms to app entry");
    println!("  LLP 0057 §1 records Exact's real graph as 570 modules / 5.47MB,");
    println!("  and the current runtime as 155ms of parse per launch.");
    println!();
    println!("  The engine build matters: a debugger-enabled Hermes is ~35% slower");
    println!("  here and halves parse throughput. Point IBEX2_VANILLA_HERMES_DIR at a");
    println!("  --vanilla --release engine for a production-shaped number.");
}
