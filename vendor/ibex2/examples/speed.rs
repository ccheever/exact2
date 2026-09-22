//! The speed numbers that matter for Ibex 2, in process, as one JSON line.
//!
//! `scripts/metrics.mjs` runs this and adds the process-level numbers (cold
//! start, RSS, sizes). Everything here is what LLP 0063 measured by hand,
//! made repeatable: the per-runtime floor by phase, a chain graph loaded from
//! source and from ahead-of-time bytecode, and the cost of crossing the
//! boundary once, synchronously and as an async host task.
//!
//!     cargo run -p ibex2 --release --features hermes --example speed
//!
//! Medians over repeated runs, because a single sample of anything here is
//! dominated by whatever else the machine was doing (LLP 0063 §2's note).
use ibex2::engine::hermes::{DynamicCode, Hermes};
use ibex2::loader::{ModuleGrants, Root};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    values[values.len() / 2]
}

// --- the floor -------------------------------------------------------------

struct Floor {
    create: f64,
    stdlib: f64,
    bindings: f64,
    freeze: f64,
    first_eval: f64,
}

impl Floor {
    fn total(&self) -> f64 {
        self.create + self.stdlib + self.bindings + self.freeze + self.first_eval
    }
}

fn floor() -> Floor {
    let t = Instant::now();
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    let create = ms(t.elapsed());
    let t = Instant::now();
    assert!(rt.install_stdlib());
    let stdlib = ms(t.elapsed());
    let t = Instant::now();
    rt.install_bindings().expect("bindings");
    let bindings = ms(t.elapsed());
    let t = Instant::now();
    rt.harden().expect("harden");
    let freeze = ms(t.elapsed());
    let t = Instant::now();
    rt.eval("1 + 1").expect("first eval");
    let first_eval = ms(t.elapsed());
    Floor {
        create,
        stdlib,
        bindings,
        freeze,
        first_eval,
    }
}

// --- a graph ---------------------------------------------------------------

/// Roughly the per-module size of Exact's measured graph (LLP 0057 §1).
const TARGET_MODULE_BYTES: usize = 9_600;

/// Plausible JavaScript rather than filler: the parser has to do real work.
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

/// A chain of `count` modules under a fresh directory, each requiring the
/// next, so every one is genuinely loaded rather than sitting on disk.
struct Graph {
    dir: PathBuf,
    bytes: usize,
}

impl Graph {
    fn build(count: usize) -> Self {
        let dir = std::env::temp_dir().join(format!("ibex2-speed-{count}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("graph dir");
        let mut bytes = 0;
        for i in 0..count {
            let next = (i + 1 < count).then_some(i + 1);
            let source = module_source(i, next);
            bytes += source.len();
            std::fs::write(dir.join(format!("m{i}.js")), &source).expect("write");
        }
        std::fs::write(
            dir.join("index.js"),
            "const head = require('./m0');\nexports.ok = head.run().tag;\n",
        )
        .expect("entry");
        Self { dir, bytes }
    }

    fn specifiers(&self, count: usize) -> Vec<String> {
        let mut all: Vec<String> = (0..count).map(|i| format!("./m{i}.js")).collect();
        all.push("./index.js".to_string());
        all
    }
}

impl Drop for Graph {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compiler_for(root: &Path) -> Option<ibex2::bytecode::Compiler> {
    ibex2::bytecode::Compiler::discover(&repo_root(), root.join(".ibex2/cache")).ok()
}

/// Compile every module ahead of time and write the manifest, as `ibex2
/// build` does. Returns the wall time, which is the dev-loop cost of a build.
fn build_bytecode(graph: &Graph, count: usize, compiler: &ibex2::bytecode::Compiler) -> Duration {
    let t = Instant::now();
    let mut manifest =
        ibex2::bytecode::Manifest::for_engine(ibex2::bytecode::Compiler::linked_engine());
    let mut artifacts = Vec::new();
    manifest.insert_edge("./", "./index.js", "./index.js");
    manifest.insert_edge("./index.js", "./m0", "./m0.js");
    for i in 0..count.saturating_sub(1) {
        manifest.insert_edge(
            &format!("./m{i}.js"),
            &format!("./m{}", i + 1),
            &format!("./m{}.js", i + 1),
        );
    }
    for spec in graph.specifiers(count) {
        let source =
            std::fs::read_to_string(graph.dir.join(spec.trim_start_matches("./"))).expect("read");
        let wrapped = ibex2::loader::lower_and_wrap(&source, &spec).expect("lower");
        let bytes = compiler.compile(&wrapped).expect("compile");
        manifest.insert(&spec, &compiler.key(&wrapped));
        artifacts.push((compiler.key(&wrapped), bytes));
    }
    manifest
        .write(&graph.dir.join(".ibex2/cache"))
        .expect("manifest");
    ibex2::bytecode::Bundle::write(&graph.dir.join(".ibex2/cache"), &artifacts).expect("bundle");
    t.elapsed()
}

/// Time from a runtime that is ready for application code to the entry
/// module — and everything it requires — having evaluated.
fn load(root: &Path, compiler: Option<ibex2::bytecode::Compiler>, precompiled: bool) -> f64 {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib());
    rt.install_bindings().expect("bindings");
    rt.set_loader_with(
        Root::Declared(root.to_path_buf()),
        ModuleGrants::none(),
        compiler,
        precompiled,
    )
    .expect("loader");
    rt.harden().expect("harden");
    let t = Instant::now();
    rt.run_entry("./index.js").expect("entry");
    ms(t.elapsed())
}

/// (median, minimum) over `runs`. The minimum is the number to track: it is
/// the cost with the least of the machine's other work in it, and LLP 0063
/// §2 says to take it for exactly that reason. The median says how noisy
/// the run was.
fn load_stats(
    root: &Path,
    compiler: Option<&ibex2::bytecode::Compiler>,
    precompiled: bool,
    runs: usize,
) -> (f64, f64) {
    // One warm run first: the point is steady-state cost, not page cache.
    let _ = load(root, compiler.cloned(), precompiled);
    let samples: Vec<f64> = (0..runs)
        .map(|_| load(root, compiler.cloned(), precompiled))
        .collect();
    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    (median(samples), min)
}

// --- the boundary ----------------------------------------------------------

/// A synchronous host call, through its public binding, in nanoseconds.
fn sync_host_call_ns() -> f64 {
    let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
    assert!(rt.install_stdlib());
    rt.install_bindings().expect("bindings");
    rt.eval("for (let i = 0; i < 1000; i++) performance.now();")
        .expect("warm");
    let n = 200_000u32;
    let program = format!("for (let i = 0; i < {n}; i++) performance.now();");
    let samples: Vec<f64> = (0..5)
        .map(|_| {
            let t = Instant::now();
            rt.eval(&program).expect("calls");
            t.elapsed().as_secs_f64() * 1e9 / n as f64
        })
        .collect();
    median(samples)
}

/// One async host task, round trip: `fs.readFile` of a tiny file, delivered
/// back through the loop and its microtask, in microseconds.
fn async_fs_roundtrip_us() -> f64 {
    let dir = std::env::temp_dir().join(format!("ibex2-speed-fs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let dir = dir.canonicalize().expect("canonical");
    let file = dir.join("tiny.txt");
    std::fs::write(&file, "x").expect("file");
    let n = 300u32;
    std::fs::write(
        dir.join("index.js"),
        format!(
            "let i = 0;\n\
             function step() {{ if (i++ === {n}) return; fs.readFile({:?}).then(step); }}\n\
             step();\n",
            file.to_string_lossy()
        ),
    )
    .expect("entry");
    let manifest = format!("[./index.js]\nfs.read {}\n", dir.to_string_lossy());

    let mut samples = Vec::new();
    for _ in 0..3 {
        let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
        assert!(rt.install_stdlib());
        rt.install_bindings().expect("bindings");
        rt.set_loader(
            Root::Declared(dir.clone()),
            ModuleGrants::parse(&manifest).expect("manifest"),
        )
        .expect("loader");
        rt.harden().expect("harden");
        let t = Instant::now();
        rt.run_entry("./index.js").expect("entry");
        rt.run_to_quiescence(Duration::from_secs(20));
        samples.push(t.elapsed().as_secs_f64() * 1e6 / n as f64);
    }
    let _ = std::fs::remove_dir_all(&dir);
    median(samples)
}

fn main() {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| out.push((k.to_string(), v));
    let num = |v: f64| format!("{v:.3}");

    // The floor: one cold sample (what a process pays first) and the warm median.
    let cold = floor();
    let warm: Vec<Floor> = (0..20).map(|_| floor()).collect();
    let pick = |f: fn(&Floor) -> f64| median(warm.iter().map(f).collect());
    put("floor_cold_ms", num(cold.total()));
    put("floor_ms", num(pick(|f| f.total())));
    put("floor_create_ms", num(pick(|f| f.create)));
    put("floor_stdlib_ms", num(pick(|f| f.stdlib)));
    put("floor_bindings_ms", num(pick(|f| f.bindings)));
    put("floor_freeze_ms", num(pick(|f| f.freeze)));
    put("floor_first_eval_ms", num(pick(|f| f.first_eval)));

    // A 100-module graph from source: the cost bytecode exists to remove.
    let small = Graph::build(100);
    put(
        "graph_100_source_ms",
        num(load_stats(&small.dir, None, false, 3).0),
    );

    // The same graph, and a 500-module one, from ahead-of-time bytecode.
    match compiler_for(&small.dir) {
        Some(compiler) => {
            build_bytecode(&small, 100, &compiler);
            put(
                "graph_100_bytecode_ms",
                num(load_stats(&small.dir, Some(&compiler), true, 5).0),
            );
            drop(small);
            let large = Graph::build(500);
            let compiler = compiler_for(&large.dir).expect("compiler");
            let build = build_bytecode(&large, 500, &compiler);
            put("graph_500_build_ms", num(ms(build)));
            put("graph_500_bytes", large.bytes.to_string());
            let (loaded, best) = load_stats(&large.dir, Some(&compiler), true, 7);
            put("graph_500_bytecode_ms", num(loaded));
            put("graph_500_bytecode_min_ms", num(best));
            put(
                "graph_500_bytecode_per_module_us",
                num(best * 1000.0 / 500.0),
            );
            // Where the per-module cost goes, each stage alone, best of 5:
            // resolving the specifier (containment canonicalizes, the probe
            // stats candidates), reading the artifact by key, and evaluating
            // the bytecode into a function. What load pays beyond their sum
            // is the loader's own work: the registry, require, the bindings,
            // the module and exports objects, and the call.
            let root = Root::Declared(large.dir.clone());
            let manifest =
                ibex2::bytecode::Manifest::read(&large.dir.join(".ibex2/cache")).expect("manifest");
            let specs: Vec<(String, String)> = (1..500)
                .map(|i| (format!("./m{}.js", i - 1), format!("./m{i}")))
                .collect();
            let resolve_ms = median(
                (0..5)
                    .map(|_| {
                        // One cache per pass, as one loader would have.
                        let cache = ibex2::loader::ResolveCache::default();
                        let t = Instant::now();
                        for (from, spec) in &specs {
                            ibex2::loader::resolve_in(&cache, &root, from, spec).expect("resolve");
                        }
                        ms(t.elapsed())
                    })
                    .collect(),
            );
            let keys: Vec<String> = (0..500)
                .map(|i| {
                    manifest
                        .get(&format!("./m{i}.js"))
                        .expect("key")
                        .to_string()
                })
                .collect();
            // The bundle read once plus one lookup and copy per module — what
            // a run pays now — beside the per-file reads it replaced.
            let read_ms = median(
                (0..5)
                    .map(|_| {
                        let t = Instant::now();
                        let bundle = ibex2::bytecode::Bundle::read(&large.dir.join(".ibex2/cache"))
                            .expect("bundle");
                        for key in &keys {
                            let _ = bundle.get(key).expect("artifact").to_vec();
                        }
                        ms(t.elapsed())
                    })
                    .collect(),
            );
            let files_ms = median(
                (0..5)
                    .map(|_| {
                        let t = Instant::now();
                        for key in &keys {
                            compiler.by_key(key).expect("artifact");
                        }
                        ms(t.elapsed())
                    })
                    .collect(),
            );
            put("graph_500_read_files_ms", num(files_ms));
            let artifacts: Vec<Vec<u8>> = keys
                .iter()
                .map(|k| compiler.by_key(k).expect("artifact"))
                .collect();
            let eval_ms = median(
                (0..5)
                    .map(|_| {
                        let mut rt = Hermes::new(DynamicCode::Closed).expect("runtime");
                        assert!(rt.install_stdlib());
                        let t = Instant::now();
                        for bytes in &artifacts {
                            rt.eval_bytes(bytes).expect("eval");
                        }
                        ms(t.elapsed())
                    })
                    .collect(),
            );
            put("graph_500_resolve_ms", num(resolve_ms));
            put("graph_500_read_ms", num(read_ms));
            put("graph_500_eval_ms", num(eval_ms));
            put("hermesc", "true".to_string());
        }
        None => {
            drop(small);
            put("hermesc", "false".to_string());
        }
    }

    put("sync_host_call_ns", num(sync_host_call_ns()));
    put("async_fs_roundtrip_us", num(async_fs_roundtrip_us()));

    let body: Vec<String> = out.iter().map(|(k, v)| format!("\"{k}\": {v}")).collect();
    println!("{{{}}}", body.join(", "));
}
