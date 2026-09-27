# LLP 1028: Wasm engines for the executor slot, measured

**Type:** Research
**Status:** Draft
**Systems:** Runner (the `DataSource` executor slot of LLP 1026 D2 and LLP 1027 D3 — which engine could sit in it, at what size and speed), Build (ahead-of-time compilation as a bake step), Apple host and Linux host (what a binary would link per platform), Delivery (LLP 1026's module card and runtime version, as the artifact shape changes), ibex (the `WebAssembly` global ibex's LLPs said the engine alone could supply)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-03
**Related:** LLP 1026 D2/D3/D5/D10/D12 and §5 (the wasmi executor as designed and measured 2026-09-02; the ABI; budgets; digest identity; Level A/B), LLP 1027 §5/§8/D8 (the lean Hermes VM measured; the trade that took the wasm module off 1026's staging; Rust and TypeScript routed by source name), LLP 1029 (the proposal these findings feed), `rules/RULES.md` §Scope (the boot path compiles nothing; modules ship as bytecode), `rules/DEFERRED.md` §Authoring models, ibex LLP 0057 §Related and 0057.000 (the WinterTC table: `WebAssembly.*` "the engine's; vanilla Hermes does not ship it — the one WinterTC item this program cannot supply"), ibex LLP 0066 ("Tier E cannot provide it. That is a decision, not a gap to fill"), LLP 1020 (the webview — the one place a JIT exists on iOS), LLP 1012 (determinism: the same module must give the same bytes on every host). Predecessor, research never authority: exact1 LLP 0517 (the wasm host interface), weird-castle `archive/exact1/server/sim-core/wasmtime-host` (a wasmtime host the predecessor ran). External: wasmtime 47.0.3 and its Pulley interpreter, wasmi 2.0.0, Cranelift, Shorebird (code push for Flutter), Apple's JIT and downloaded-code rules, WKWebView's process model.

## Summary

Charlie asked, 2026-09-03, how weird a wasm interpreter in exact2 would
be, then whether wasmi would give ibex2 `WebAssembly`, then how slow a
JavaScript `WebAssembly` global would be, then whether any platform but
iOS could JIT wasm, then how big wasmtime and Cranelift are and whether
some Rust could be patched over the air without all of it. This
document is the measured and researched answers, in that order, with
the confidence of each stated. It decides nothing; LLP 1029 proposes.

The headline, measured on this Mac with exact2's own release profile
(§2):

| Engine in a stripped binary | Delta over a 351 KB no-engine baseline | fib(30) vs native |
|---|---|---|
| wasmtime, runtime only — loads code compiled ahead of time | **+0.49 MB** | **1.25×** |
| wasmtime, runtime + the Pulley interpreter | **+0.58 MB** | 13× |
| wasmi (LLP 1026's choice) | **+1.0 MB** | 5.4× |
| wasmtime, runtime + Cranelift — compiling on the device | +5.6 MB | 1.3× |
| lean Hermes (LLP 1027 §5, for scale) | +1.8 MB | 7.5–29× |

What the numbers say: **Cranelift never has to ship.** A bake that
compiles ahead of time per target lets every platform that permits
native code run wasm at 1.25× native from a 0.49 MB runtime, and lets
iOS — which permits no JIT and no downloaded machine code — run the
same module under an interpreter that is smaller than the one LLP 1026
designed around. Two interpreters are candidates for iOS and they are a
real choice: wasmi is 2.5× faster than Pulley on this microbenchmark at
nearly twice the size. The rest of the thread's questions resolve the
same way once the numbers are in hand: a WKWebView is no route to a JIT
(§3 F7), Shorebird's function-level patching is possible for Rust only
at the data seam (F6), and a JavaScript `WebAssembly` global is a
separate consumer whose cost is the boundary, not the interpreter (F8).

## 1. The question, and what was already known

LLP 1026 D2 designed the executor slot's wasm form: the app's data crate
built a second time for `wasm32-unknown-unknown`, run by wasmi behind
`DataSource`, importless (D3), budgeted by fuel (D5), and — the part
that made it more than a dev-loop trick — native by digest identity
(D10): the binary embeds native code and runs the interpreter only
while an update's module differs from what is embedded. 1026 §5
measured it: +1.0 MB, 8–10× native on Caltrain's own crate, byte-
identical answers, 0.6 s from a data-crate edit to a rebuilt module.

LLP 1027 then put the lean Hermes VM in the same slot for TypeScript,
and its §8 trade struck the wasm module from 1026's staging: "the phone
carries at most one interpreter, chosen by the app's language. wasmi
returns only if LLP 1026 D6 (surfaces as wasm) is ever built, and then
for surfaces." That was a simplicity ruling, not a measurement, and it
was made with wasmi's number and none of wasmtime's.

The thread of 2026-09-03 reopened it from the other side — could
ibex2 have `WebAssembly`? — and the size question was the one nobody
had measured. This document measures it and gathers the rest.

## 2. Method

A scratch workspace outside the repo (the source is §5, verbatim; the
session's scratchpad held it): six Cargo projects, each standalone so
that Cargo's feature unification could not blend the wasmtime variants,
every one under **exact2's release profile** (`opt-level = 3`, `lto =
"thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"` —
Cargo.toml:15–20), so the deltas are what a host binary would pay.

- **The module.** A `no_std` `cdylib` for `wasm32-unknown-unknown`,
  release, three exports — `add(i32, i32)`, `fib(u32)` (recursive),
  `sum(u32)` (a loop) — 235 bytes, no imports.
- **The baseline.** The same `main` shape (read a file, time three
  closures, print) with the functions native, `black_box`ed, and no
  engine referenced.
- **wasmi 2.0.0**, `default-features = false`, `std` + `validate` +
  `stable`: what LLP 1026 §5 measured, re-measured here under the same
  profile as the others.
- **wasmtime 47.0.3**, `default-features = false`, in three shapes:
  `runtime` + `std` (deserializes a precompiled module; no compiler);
  the same plus `pulley` (the portable interpreter; deserializes a
  module precompiled *to Pulley bytecode*); `runtime` + `std` +
  `cranelift` (compiles on the machine, and is the build that produced
  the two precompiled artifacts for the other two).
- **The machine.** Apple M5 Max, macOS 26.6.2, rustc 1.97.0. Sizes are
  `stat` of the stripped binary; times are `Instant` around the call,
  fib(30) the best of three, the `add` round trip averaged over one
  million calls.

**What the probe does not measure, said plainly.** `sum` was
constant-folded by LLVM into a closed form in every executor and is
not a benchmark; it is left in the appendix so the next person does
not repeat it. `fib(30)` is 1.6 million calls, the interpreters' worst
case — 1026 §5's number on Caltrain's real crate (8–10× for wasmi) is
the better guide for seam-shaped work, and the two agree in band. The
precompiled artifacts carry a fixed overhead (≈50 KB for a 235-byte
module) that is a property of the format, not of the app. Nothing here
ran on a phone; a Pulley or wasmi number on an A-series core is owed
the same way LLP 1027 §10 Q7 owes Hermes's.

## 3. Findings

### F1 — Sizes, stripped, exact2's release profile, macOS arm64

| Binary | Bytes | Delta over baseline |
|---|---|---|
| baseline, no engine | 351,344 | — |
| wasmtime `runtime` + `std` (precompiled native code) | 838,640 | **+487,296 (0.49 MB)** |
| wasmtime `runtime` + `std` + `pulley` | 929,296 | **+577,952 (0.58 MB)** |
| wasmi `std` + `validate` + `stable` | 1,356,672 | **+1,005,328 (1.0 MB)** — matches 1026 §5 |
| wasmtime `runtime` + `std` + `cranelift` | 5,985,008 | +5,633,664 (5.6 MB) |
| lean Hermes, from LLP 1027 §5 | | +1,810,256 (1.8 MB) |

Confidence: high; reproducible from §5.

### F2 — Speeds, the same binaries, the same module

| Engine | `add` round trip | fib(30) | vs native | Load |
|---|---|---|---|---|
| native Rust | inlined | 1.58 ms | 1× | — |
| wasmtime, precompiled native | 9 ns | 1.98 ms | **1.25×** | deserialize 0.14 ms, instantiate 0.02 ms |
| Cranelift on the device | 9 ns | 2.09 ms | 1.32× | compile 2.1 ms, instantiate 0.01 ms |
| wasmi | 23 ns | 8.50 ms | **5.4×** | validate + instantiate 0.27 ms |
| Pulley | 45 ns | 20.84 ms | **13×** | deserialize 0.15 ms, instantiate 0.02 ms |

Precompiling to Pulley bytecode took 0.66 ms; the artifacts were
50,968 B (native) and 67,272 B (Pulley) for the 235-byte module.

Two readings. First, the interpreters are 4–10× apart from the
compiled forms and 2.5× apart from each other, and every one of them
lands in microseconds for seam-shaped calls (1026 §5: `board` at 10 µs
native was 95–106 µs under wasmi; two per tick is 200 µs). Second, the
JIT-versus-AOT question is moot: code compiled ahead of time by
Cranelift runs as fast as code compiled on the device, so the compiler
has no reason to be in a shipped binary.

Confidence: high for the ratios on this machine; the phone is owed.

### F3 — Cranelift never has to ship; the artifact is per target

wasmtime's `Module::serialize` / `deserialize` split the compiler from
the runtime. A bake step compiles the neutral `app.module.wasm` once
per target — `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`,
`aarch64-linux-android` if it ever exists, and `pulley64` for iOS — and
a host links `wasmtime` with `runtime` + `std` (+ `pulley` on the
phone), which is 0.49–0.58 MB and can be handed no source. That is
`rules/RULES.md` §Scope's boot rule taken literally for wasm: modules
ship as bytecode (Pulley) or as precompiled code, and the boot path
compiles nothing.

The price is the artifact's shape. 1026 D4's module card was one
platform-neutral file; a precompiled artifact is per target and is
**version-coupled to wasmtime** exactly as Hermes bytecode is to Hermes
(ibex2 `src/bytecode.rs`), so the runtime version (1026 D9) must carry
the wasmtime version as it carries the bytecode version (1027 D7). The
neutral `.wasm` can still travel beside the per-target files as the
canonical form and the fixture's input. Loading precompiled native
code on macOS needs executable memory, which under the hardened
runtime is an entitlement (`com.apple.security.cs.allow-jit` or
`allow-unsigned-executable-memory`); dev clients have it trivially, a
Developer ID build passes notarization with a justification, and
whether the **Mac App Store** accepts a binary that executes a
downloaded precompiled artifact is a review question, not a fact.
**Unverified here:** which of the two entitlements wasmtime's loader
needs on macOS 26 (an afternoon with `build.mjs`), and the store's
answer (a submission).

Confidence: high on the mechanism (the probe's `wt_runtime` and
`wt_pulley` binaries link no compiler and ran the artifacts
`wt_cranelift` wrote); medium on the macOS entitlement detail.

### F4 — On iOS the interpreter is a real choice: wasmi or Pulley

| | wasmi | Pulley |
|---|---|---|
| Size | +1.0 MB | +0.58 MB |
| fib(30) | 5.4× native | 13× native |
| Call round trip | 23 ns | 45 ns |
| What it loads | the neutral `.wasm`, validated on the device | Pulley bytecode from the bake, no validator on the device |
| Engines in the program | two (wasmi on iOS, wasmtime elsewhere) | one (wasmtime everywhere) |
| Budgets | fuel + `StoreLimits` (1026 D5) | fuel + `StoreLimits` + epoch interruption |

Pulley is the newer of the two (wasmtime's portable interpreter,
present in 47 as a first-class target); wasmi's register-based
executor has had two more years of tuning. The 2.5× on fib is one
microbenchmark on one machine. The size difference is 0.42 MB. Which
matters more is a ruling, and LLP 1029 §8 asks it.

Confidence: high on the numbers, medium on how they generalize to a
seam-shaped workload on a phone.

### F5 — Where a JIT is allowed, and which engines have one

| Platform | JIT | Loading downloaded machine code | Notes |
|---|---|---|---|
| iOS, a third-party app | never | never | No `MAP_JIT` entitlement for third parties; interpreted code is permitted under the downloaded-code clause (the one Expo Updates and Hermes bytecode live under) |
| iOS, a WKWebView | yes | n/a | Apple's WebContent process holds the entitlement; Lockdown Mode turns it off |
| macOS | yes | yes | Hardened runtime needs an entitlement; App Store accepts it |
| Linux | yes | yes | |
| Windows | yes | yes | |
| Android | yes | yes, with Play policy limits | wasmtime supports `aarch64-linux-android` |

Engines, from public knowledge, not run here except where §2 says:

- **wasmtime** — Cranelift (x86-64, aarch64, s390x, riscv64), the Winch
  baseline compiler, the **Pulley** interpreter for targets without a
  JIT, AOT via serialize/deserialize, fuel and epoch budgets. Pure
  Rust. Measured above.
- **wasmer** — Cranelift, LLVM, and Singlepass backends; a headless
  mode for precompiled artifacts; iOS by linking precompiled objects
  into the binary, which is embedding, not delivery.
- **V8** — Liftoff (baseline) and TurboFan (optimizing); jitless mode
  historically disabled wasm and now has the DrumBrake interpreter.
  Many times Hermes's size without a JIT; the wrong trade for an
  interpreter-only platform, which is why the thread's "swap Hermes
  for V8 in ibex2" was declined.
- **JavaScriptCore** — BBQ and OMG JITs, and an in-place interpreter
  (IPInt) added recently. Whether `WebAssembly` is exposed to a
  `JSContext` inside a third-party iOS app, where JSC has no JIT, is
  **unverified** here.
- **wasmi** — pure Rust, `no_std`-capable, register-based, fuel and
  limits. Measured above.
- **wasm3** — C, fast, the one non-Rust dependency it would be (1026
  D2); not measured.
- **Hermes** — runs no wasm at all; its old `-fwasm` intrinsics were
  for asm.js-style code, not a wasm runtime. ibex LLP 0057.000's table
  is right that vanilla Hermes cannot supply `WebAssembly.*`; it is
  wrong only in the inference that therefore nobody on ibex2's side of
  the seam can (F8).

Confidence: high on the platform rules, medium on engine details that
move with releases.

### F6 — Patching some code without all of it: Shorebird, and what Rust can do

Charlie: "could you patch certain modules of Rust without changing
them all … I think Flutter does something like that?" Flutter itself
does not; **Shorebird**, the third-party code push for Flutter, does.
Its iOS model: the app ships Dart compiled ahead of time; a patch runs
*changed* functions in Shorebird's interpreter and *unchanged* ones
from the app's own AOT snapshot, and its CLI reports the "link
percentage" — the share of code still running native. It can do this
at function granularity because both sides share the Dart heap and
object model, so an interpreted function can call a native one and
hand it objects. (Android details are not verified here.)

For Rust there is no shared heap between a native binary and a wasm
instance — a pointer on one side means nothing on the other — so the
only place the two can meet is a boundary that passes values, and
exact2 has exactly one: the data seam, where a **source** is a named
function from values to values (LLP 1004 D4, LLP 1005 §3). That gives
two granularities, and the corpus already has both halves:

- **Per crate** — LLP 1026 D10 as written: identity by the module's
  digest; a changed module runs interpreted until the next binary.
- **Per source name** — LLP 1027 D8 already routes each source name to
  an executor (`Either`). Key D10's digest per source instead of per
  module: the bake hashes, for each export, the wasm functions
  reachable from it; at run time a source whose digest equals the
  embedded one routes to the linked native crate, and a source whose
  digest differs routes to the module. One changed source is the only
  interpreted one, and the update can say "3 of 41 sources
  interpreted" as Shorebird says a percentage.

The requirement is that sources share no in-process mutable state,
which the seam already asks of them: state is the runner's Store (LLP
1018), a crate is a function of its inputs, and the byte-equality
fixture (1026 D3, 1027 D5) enforces determinism — a crate that kept a
cross-source cache in memory would fail it. Finer than a source is not
possible on iOS, because the "native" side of a finer split would be
downloaded machine code. Determinism of the wasm build across the two
bakes (the one that produced the binary, the one that produced the
update) is what makes a digest match mean "the same code"; Rust's
wasm32 output is deterministic for the same toolchain and inputs, and
a mismatch errs on the interpreted side, never the native one.

Confidence: high on the mechanism; medium on Shorebird's exact
internals (public descriptions, not source).

### F7 — A WKWebView as the JIT host on iOS: no

Charlie's read — "the IPC overhead makes talking to it too slow to be
practical for anything" — is right, with one narrow exception. The
web content is a separate process; `evaluateJavaScript` and
`WKScriptMessageHandler` are asynchronous XPC round trips carrying
JSON-serializable values, on the order of hundreds of microseconds to
a millisecond each; there is no shared memory; the process is tens of
megabytes, takes on the order of a hundred milliseconds to start, and
is suspended or killed in the background. A data-seam answer is
1–100 µs of work, so the hop is three to five orders of magnitude
more than the call. The exception is batch compute: a job that would
run for hundreds of milliseconds under an interpreter would still win
with a millisecond hop each way — and for LLP 1020's decks, whose own
wasm already lives inside the webview, the JIT is simply there. For
the seam, with Pulley or wasmi at 5–13× native in process, the
motivation is gone. (Numbers here are from experience, not measured;
the conclusion does not depend on their second digit.)

### F8 — A JavaScript `WebAssembly` global: possible on ibex2's side of the seam; the cost is the boundary

ibex's LLPs (0057.000 §Tier E, 0066) hold that `WebAssembly.*` is the
engine's to supply and that vanilla Hermes cannot, so "a consumer that
needs it needs a different engine build." wasmi or wasmtime on the
Rust side reverses the premise: `WebAssembly` becomes a binding file
beside `fetch.js` and `url.js` (`crates/ibex2/src/bindings/`) over
host functions through the same JSI seam, Rust owning the engine,
stores, and instances, JavaScript holding handles; imports are host
functions calling back into the runtime. The demand exists in ibex's
own survey (LLP 0059 §2: Rive and Lottie ship wasm).

What it would cost, estimated from the measured parts and not itself
measured:

- **Compute** runs at the interpreter's speed — the same band as
  Hermes running the equivalent JavaScript (F2 against 1027 §5), so a
  wasm library gains nothing over JavaScript on Hermes and runs
  roughly ten times slower than in Safari or Chrome. On a desktop host
  with precompiled code it would run near native, which is the web's
  model: compute in wasm, glue in JavaScript.
- **Each call across** is a JSI host call (45 ns measured, `metrics/
  ibex2-speed.jsonl`) plus numeric conversion plus the engine's call
  entry (9–45 ns, F2): 100–200 ns against a browser's 5–20 ns. Chatty
  APIs pay it on every call, in both directions.
- **Memory** must alias: `memory.buffer` has to be an ArrayBuffer over
  the instance's linear memory, or every Emscripten `HEAPU8` access
  becomes a host call and nothing real runs. wasmi has a static-memory
  constructor over a caller-owned slice and wasmtime's memories can be
  reserved at their maximum, so the base never moves; the spec's
  detach-on-grow semantics need a JSI path that is **unverified**
  (returning a fresh, longer buffer over the same base is what
  Emscripten glue actually reads).
- **Instantiation** scales roughly linearly, tens of milliseconds for
  a 2 MB module.

Rule of thumb: a library that budgets a millisecond per frame in the
browser lands near ten on an iPhone here — fine for a parser or a
codec, marginal for per-frame rendering. This is a different consumer
from the exact2 seam (a data source is a function, not a program —
LLP 1027 D9) and stacks on top of Hermes; it belongs to ibex2 with
its own trigger.

Confidence: medium; the spike that replaces the estimates is small
(wasmi behind one host function, a `Memory` whose buffer aliases, and
Rive's module instantiated and stepped once).

### F9 — The two interpreters already accepted are in the same band as these

LLP 1027 accepted the lean Hermes VM at +1.8 MB and 7.5–29× native for
TypeScript logic, per call in microseconds, after first pixel. Every
wasm engine here is smaller than that, and the two interpreters bracket
it in speed. Nothing measured here argues against 1027's ruling; what
it argues against is 1027 §8's inference that the wasm executor must
cost a second interpreter of the same size.

## 4. Confidence, gathered

| Claim | Basis |
|---|---|
| Sizes and speeds in F1–F2 | measured, this Mac, reproducible from §5 |
| AOT equals JIT; the compiler need not ship (F3) | measured (the runtime-only binaries ran the artifacts) |
| Per-target, version-coupled artifacts (F3) | wasmtime's documented contract |
| The macOS entitlement wasmtime's loader needs | unverified |
| Platform JIT rules (F5) | public, stable |
| Engine details (F5) | public knowledge as of this date; move with releases |
| Shorebird's iOS model (F6) | public descriptions; internals not read |
| Per-source digest routing (F6) | design reasoning over 1026 D10 + 1027 D8; unbuilt |
| WKWebView costs (F7) | experience, not measured; conclusion robust |
| JS `WebAssembly` global costs (F8) | estimated from measured parts; the aliasing path unverified |
| Phone numbers for any of it | owed |

## 5. Appendix — the probe, verbatim

Six standalone Cargo projects; `mod` builds with `cargo build --release
--target wasm32-unknown-unknown`, the rest with `cargo build --release`.
Run `wt_cranelift mod.wasm` first (it writes `mod.native.cwasm` and
`mod.pulley.cwasm`), then `wt_runtime mod.native.cwasm`, `wt_pulley
mod.pulley.cwasm`, `wasmi_host mod.wasm`, `baseline mod.wasm`. Every
`Cargo.toml` carries the same `[profile.release]` as the workspace's.

### `mod/Cargo.toml`

```toml
[package]
name = "probe_mod"
version = "0.0.0"
edition = "2021"
[lib]
crate-type = ["cdylib"]
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### `mod/src/lib.rs`

```rust
#![no_std]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 { a.wrapping_add(b) }
#[no_mangle]
pub extern "C" fn fib(n: u32) -> u32 { if n < 2 { n } else { fib(n - 1).wrapping_add(fib(n - 2)) } }
#[no_mangle]
pub extern "C" fn sum(n: u32) -> u64 {
    let mut s = 0u64; let mut i = 0u32;
    while i < n { s = s.wrapping_add((i as u64).wrapping_mul(2654435761)); i += 1; }
    s
}
```

### `baseline/Cargo.toml`

```toml
[package]
name = "baseline"
version = "0.0.0"
edition = "2021"
[dependencies]

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### `baseline/src/main.rs`

```rust

use std::hint::black_box;
fn fib(n: u32) -> u32 { if n < 2 { n } else { fib(n - 1).wrapping_add(fib(n - 2)) } }
fn sum(n: u32) -> u64 { let mut s = 0u64; let mut i = 0u32; while i < n { s = s.wrapping_add((i as u64).wrapping_mul(2654435761)); i += 1; } s }
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let bytes = std::fs::read(&path).expect("read");
    println!("read {} bytes", bytes.len());
    bench("native", |op, a, b| match op { 0 => black_box(a as i32).wrapping_add(black_box(b as i32)) as u64, 1 => fib(black_box(a as u32)) as u64, _ => sum(black_box(a as u32)) });
}


fn bench<F: FnMut(u8, u64, u64) -> u64>(label: &str, mut call: F) {
    use std::time::Instant;
    let n = 1_000_000u64;
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..n { acc = acc.wrapping_add(call(0, i, 1)); }
    let per_call = t.elapsed().as_nanos() as f64 / n as f64;
    let mut best = f64::MAX;
    let mut r = 0;
    for _ in 0..3 { let t = Instant::now(); r = call(1, 30, 0); best = best.min(t.elapsed().as_secs_f64() * 1e3); }
    let t = Instant::now();
    let s = call(2, 50_000_000, 0);
    let sum_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("{label}: add call {per_call:.0} ns · fib(30)={r} {best:.2} ms · sum(5e7)={s} {sum_ms:.2} ms · acc={acc}");
}
```

### `wasmi_host/Cargo.toml`

```toml
[package]
name = "wasmi_host"
version = "0.0.0"
edition = "2021"
[dependencies]
wasmi = { version = "2.0.0", default-features = false, features = ["std", "validate", "stable"] }
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### `wasmi_host/src/main.rs`

```rust

use wasmi::*;
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let bytes = std::fs::read(&path).expect("read");
    let t = std::time::Instant::now();
    let engine = Engine::default();
    let module = Module::new(&engine, &bytes[..]).expect("module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instance");
    println!("wasmi: load+instantiate {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let add = instance.get_typed_func::<(i32, i32), i32>(&store, "add").unwrap();
    let fib = instance.get_typed_func::<u32, u32>(&store, "fib").unwrap();
    let sum = instance.get_typed_func::<u32, u64>(&store, "sum").unwrap();
    bench("wasmi", |op, a, b| match op { 0 => add.call(&mut store, (a as i32, b as i32)).unwrap() as u64, 1 => fib.call(&mut store, a as u32).unwrap() as u64, _ => sum.call(&mut store, a as u32).unwrap() });
}


fn bench<F: FnMut(u8, u64, u64) -> u64>(label: &str, mut call: F) {
    use std::time::Instant;
    let n = 1_000_000u64;
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..n { acc = acc.wrapping_add(call(0, i, 1)); }
    let per_call = t.elapsed().as_nanos() as f64 / n as f64;
    let mut best = f64::MAX;
    let mut r = 0;
    for _ in 0..3 { let t = Instant::now(); r = call(1, 30, 0); best = best.min(t.elapsed().as_secs_f64() * 1e3); }
    let t = Instant::now();
    let s = call(2, 50_000_000, 0);
    let sum_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("{label}: add call {per_call:.0} ns · fib(30)={r} {best:.2} ms · sum(5e7)={s} {sum_ms:.2} ms · acc={acc}");
}
```

### `wt_cranelift/Cargo.toml`

```toml
[package]
name = "wt_cranelift"
version = "0.0.0"
edition = "2021"
[dependencies]
wasmtime = { version = "47.0.3", default-features = false, features = ["runtime", "std", "cranelift"] }
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### `wt_cranelift/src/main.rs`

```rust

use wasmtime::*;
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let bytes = std::fs::read(&path).expect("read");
    let engine = Engine::default();
    let t = std::time::Instant::now();
    let module = Module::new(&engine, &bytes).expect("module");
    println!("cranelift: compile {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let native = module.serialize().unwrap();
    std::fs::write("mod.native.cwasm", &native).unwrap();
    let mut cfg = Config::new();
    cfg.target("pulley64").unwrap();
    let peng = Engine::new(&cfg).unwrap();
    let t = std::time::Instant::now();
    let pulley = peng.precompile_module(&bytes).unwrap();
    println!("cranelift: pulley compile {:.3} ms · native cwasm {} B · pulley cwasm {} B", t.elapsed().as_secs_f64() * 1e3, native.len(), pulley.len());
    std::fs::write("mod.pulley.cwasm", &pulley).unwrap();
    run(&engine, &module, "cranelift");
}

fn run(engine: &wasmtime::Engine, module: &wasmtime::Module, label: &str) {
    use wasmtime::*;
    let mut store = Store::new(engine, ());
    let t = std::time::Instant::now();
    let instance = Instance::new(&mut store, module, &[]).expect("instance");
    println!("{label}: instantiate {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let add = instance.get_typed_func::<(i32, i32), i32>(&mut store, "add").unwrap();
    let fib = instance.get_typed_func::<u32, u32>(&mut store, "fib").unwrap();
    let sum = instance.get_typed_func::<u32, u64>(&mut store, "sum").unwrap();
    bench(label, |op, a, b| match op { 0 => add.call(&mut store, (a as i32, b as i32)).unwrap() as u64, 1 => fib.call(&mut store, a as u32).unwrap() as u64, _ => sum.call(&mut store, a as u32).unwrap() });
}


fn bench<F: FnMut(u8, u64, u64) -> u64>(label: &str, mut call: F) {
    use std::time::Instant;
    let n = 1_000_000u64;
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..n { acc = acc.wrapping_add(call(0, i, 1)); }
    let per_call = t.elapsed().as_nanos() as f64 / n as f64;
    let mut best = f64::MAX;
    let mut r = 0;
    for _ in 0..3 { let t = Instant::now(); r = call(1, 30, 0); best = best.min(t.elapsed().as_secs_f64() * 1e3); }
    let t = Instant::now();
    let s = call(2, 50_000_000, 0);
    let sum_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("{label}: add call {per_call:.0} ns · fib(30)={r} {best:.2} ms · sum(5e7)={s} {sum_ms:.2} ms · acc={acc}");
}
```

### `wt_runtime/Cargo.toml`

```toml
[package]
name = "wt_runtime"
version = "0.0.0"
edition = "2021"
[dependencies]
wasmtime = { version = "47.0.3", default-features = false, features = ["runtime", "std"] }
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### `wt_runtime/src/main.rs`

```rust

use wasmtime::*;
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let engine = Engine::default();
    let t = std::time::Instant::now();
    let module = unsafe { Module::deserialize_file(&engine, &path) }.expect("deserialize");
    println!("runtime: deserialize {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    run(&engine, &module, "wasmtime-aot");
}

fn run(engine: &wasmtime::Engine, module: &wasmtime::Module, label: &str) {
    use wasmtime::*;
    let mut store = Store::new(engine, ());
    let t = std::time::Instant::now();
    let instance = Instance::new(&mut store, module, &[]).expect("instance");
    println!("{label}: instantiate {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let add = instance.get_typed_func::<(i32, i32), i32>(&mut store, "add").unwrap();
    let fib = instance.get_typed_func::<u32, u32>(&mut store, "fib").unwrap();
    let sum = instance.get_typed_func::<u32, u64>(&mut store, "sum").unwrap();
    bench(label, |op, a, b| match op { 0 => add.call(&mut store, (a as i32, b as i32)).unwrap() as u64, 1 => fib.call(&mut store, a as u32).unwrap() as u64, _ => sum.call(&mut store, a as u32).unwrap() });
}


fn bench<F: FnMut(u8, u64, u64) -> u64>(label: &str, mut call: F) {
    use std::time::Instant;
    let n = 1_000_000u64;
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..n { acc = acc.wrapping_add(call(0, i, 1)); }
    let per_call = t.elapsed().as_nanos() as f64 / n as f64;
    let mut best = f64::MAX;
    let mut r = 0;
    for _ in 0..3 { let t = Instant::now(); r = call(1, 30, 0); best = best.min(t.elapsed().as_secs_f64() * 1e3); }
    let t = Instant::now();
    let s = call(2, 50_000_000, 0);
    let sum_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("{label}: add call {per_call:.0} ns · fib(30)={r} {best:.2} ms · sum(5e7)={s} {sum_ms:.2} ms · acc={acc}");
}
```

### `wt_pulley/Cargo.toml`

```toml
[package]
name = "wt_pulley"
version = "0.0.0"
edition = "2021"
[dependencies]
wasmtime = { version = "47.0.3", default-features = false, features = ["runtime", "std", "pulley"] }
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### `wt_pulley/src/main.rs`

```rust

use wasmtime::*;
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let mut cfg = Config::new();
    cfg.target("pulley64").unwrap();
    let engine = Engine::new(&cfg).unwrap();
    let t = std::time::Instant::now();
    let module = unsafe { Module::deserialize_file(&engine, &path) }.expect("deserialize");
    println!("pulley: deserialize {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    run(&engine, &module, "pulley");
}

fn run(engine: &wasmtime::Engine, module: &wasmtime::Module, label: &str) {
    use wasmtime::*;
    let mut store = Store::new(engine, ());
    let t = std::time::Instant::now();
    let instance = Instance::new(&mut store, module, &[]).expect("instance");
    println!("{label}: instantiate {:.3} ms", t.elapsed().as_secs_f64() * 1e3);
    let add = instance.get_typed_func::<(i32, i32), i32>(&mut store, "add").unwrap();
    let fib = instance.get_typed_func::<u32, u32>(&mut store, "fib").unwrap();
    let sum = instance.get_typed_func::<u32, u64>(&mut store, "sum").unwrap();
    bench(label, |op, a, b| match op { 0 => add.call(&mut store, (a as i32, b as i32)).unwrap() as u64, 1 => fib.call(&mut store, a as u32).unwrap() as u64, _ => sum.call(&mut store, a as u32).unwrap() });
}


fn bench<F: FnMut(u8, u64, u64) -> u64>(label: &str, mut call: F) {
    use std::time::Instant;
    let n = 1_000_000u64;
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..n { acc = acc.wrapping_add(call(0, i, 1)); }
    let per_call = t.elapsed().as_nanos() as f64 / n as f64;
    let mut best = f64::MAX;
    let mut r = 0;
    for _ in 0..3 { let t = Instant::now(); r = call(1, 30, 0); best = best.min(t.elapsed().as_secs_f64() * 1e3); }
    let t = Instant::now();
    let s = call(2, 50_000_000, 0);
    let sum_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("{label}: add call {per_call:.0} ns · fib(30)={r} {best:.2} ms · sum(5e7)={s} {sum_ms:.2} ms · acc={acc}");
}
```

### Output, 2026-09-03

```
cranelift: compile 2.106 ms
cranelift: pulley compile 0.659 ms · native cwasm 50968 B · pulley cwasm 67272 B
cranelift: instantiate 0.014 ms
cranelift: add call 9 ns · fib(30)=832040 2.09 ms · sum(5e7)=10331606895216278464 0.00 ms · acc=500000500000
runtime: deserialize 0.144 ms
wasmtime-aot: instantiate 0.021 ms
wasmtime-aot: add call 9 ns · fib(30)=832040 1.98 ms · sum(5e7)=10331606895216278464 0.00 ms · acc=500000500000
pulley: deserialize 0.151 ms
pulley: instantiate 0.021 ms
pulley: add call 45 ns · fib(30)=832040 20.84 ms · sum(5e7)=10331606895216278464 0.00 ms · acc=500000500000
wasmi: load+instantiate 0.269 ms
wasmi: add call 23 ns · fib(30)=832040 8.50 ms · sum(5e7)=10331606895216278464 0.02 ms · acc=500000500000
native: add call 0 ns · fib(30)=832040 1.58 ms · sum(5e7)=10331606895216278464 0.00 ms · acc=500000500000

baseline 351344 · wasmi_host 1356672 · wt_runtime 838640 · wt_pulley 929296 · wt_cranelift 5985008 (bytes, stripped)
```
