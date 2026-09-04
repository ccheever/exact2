# LLP 1032: JavaScript inside wasm, measured — QuickJS under wasmtime and wasmi against the lean Hermes VM

**Type:** Research
**Status:** Draft
**Systems:** Runner (the `DataSource` executor slot — whether one wasm engine could run the TypeScript half too, at what speed), Build (a JavaScript engine as a wasm module the bake precompiles per target), Apple host and Linux host (what a binary would carry instead of the lean Hermes VM), Delivery (the artifact's size per target), ibex (the engine build exact2 would stop sharing)
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1027 §3/§5/D3/D9 (why TypeScript, the lean Hermes VM measured at +1.81 MB and 7.5–29× native, the executor, the engine build shared with ibex), LLP 1028 F1–F4/F9 (the wasm engines measured: wasmtime runtime +0.49 MB at 1.25× native, Pulley +0.58 MB at 13×, wasmi +1.0 MB at 5.4× — on `fib`, the interpreters' best case; this document is their worst), LLP 1029 D2/D3/D6/§8 Q1 (engines named per app; wasmtime in the slot; "a binary carries the executors its app named"; Pulley or wasmi on iOS), LLP 1026 D2/D3/D5 (the importless module, the ABI shape this probe mirrors, budgets), LLP 1012 (determinism: the same module gives the same bytes on every host — held here across four engines), `rules/RULES.md` §Scope (the boot path compiles nothing; modules ship as bytecode), `CLAUDE.md` (optional capability is another executor, never a cargo feature). External: quickjs-ng 0.16.2 (d8e1cc6, 2026-09-04), wasmtime 47.0.3, wasmi 2.0.0, binaryen's `wasm-opt`, Homebrew llvm 23.1.0 with wasi-libc 33 and wasi-runtimes 23.1.0, Javy, StarlingMonkey, Porffor, Static Hermes, AssemblyScript (the landscape, F1).

## Summary

Charlie asked, 2026-09-04: *"If we have wasm, does that mean we don't need
hermes since we can just compile JS or whatever to WASM? would the
performance be much worse on wasmtime vs hermes? and does having a TS/JS VM
just add complexity that we don't benefit from?"* This document is the
measured answer. It decides nothing; LLP 1029 §8 is where the ruling lives.

The premise first (F1): there is no production compiler from JavaScript to
wasm that leaves the virtual machine behind. JavaScript's semantics need a
runtime, so every shipping path — Javy, StarlingMonkey, anything Emscripten
ever built — puts a JavaScript engine *inside* the wasm module and runs the
program as that engine's bytecode. The typed subsets that compile ahead of
time (AssemblyScript, Porffor, Static Hermes) are not TypeScript-with-npm,
or not shipping, or both. So "wasm instead of Hermes" means "a JavaScript
engine as a wasm module instead of Hermes linked natively," and that is a
thing one can build and time. This probe built it: quickjs-ng compiled to
wasm32, the exact-js prelude and Caltrain's TypeScript twin embedded as
QuickJS bytecode, the same `__exact_call(source, argsJson)` seam exact-js
drives through JSI, run under the three executors LLP 1028 measured, held to
the native crate's bytes on all twenty fixture cases, and timed per call the
way exact-js times its own — beside the lean Hermes VM through exact-js
itself, in the same binary, on the same day.

The headline, this Mac (Apple M5 Max), rustc 1.97.0, the quietest of four
runs, per call through the seam in microseconds (p50 of 2,000), with the
band across all seven runs stated in F3:

| executor | `defaultLocation` | `station` | `search` | `nearest` | `board` (26 departures) |
|---|---|---|---|---|---|
| native Rust crate | 0.04 | 0.04 | 0.38 | 0.67 | 5.6 |
| **lean Hermes VM, native, through exact-js** | **1.8** | **2.5** | **3.5** | **10.4** | **42.2** |
| QuickJS in wasm · wasmtime, precompiled native code | 2.8 | 4.6 | 6.3 | 20.2 | 80.1 |
| QuickJS in wasm · wasmtime, Pulley (the iOS shape) | 158.8 | 200.0 | 270.6 | 764.3 | 2,635 |
| QuickJS in wasm · wasmi (the other iOS shape) | 80.4 | 96.3 | 129.2 | 302.4 | 969 |

What the numbers say:

- **On a desktop, with code compiled ahead of time, JavaScript inside wasm
  runs at 1.5–3× the lean Hermes VM.** Usable. Not better.
- **On the phone's executors it runs at 40–134× Hermes** — Pulley — or
  17–64× — wasmi. That is an interpreter interpreting an interpreter, and
  the gap is three to four times wider than LLP 1028's `fib` predicted,
  because an engine's dispatch loop is the work a wasm interpreter does
  worst (F3). Caltrain's `board`, 42 µs under Hermes, is 2.6–5.1 ms under
  Pulley on an M5 Max; the phone is slower by a factor nobody has measured.
- **It is not smaller.** The engine as a Pulley artifact is 1.5–2.1 MB on top
  of wasmtime's 0.58 MB, against Hermes's 1.81 MB in the binary (F4).
- **It is one engine** — one ABI, fuel budgets for TypeScript too, an
  importless sandbox by construction, no C++ linked into any host, one
  over-the-air path — and that is real (F6). The phone's number is what it
  costs.

The honest reading of the three questions: wasm does not remove the need
for a JavaScript VM, it relocates one; on the executor iOS can run, the
relocated VM is one to two orders of magnitude slower than Hermes on the
same code; and the VM is the price of TypeScript, which LLP 1029 ruled in
for authoring reasons this measurement does not touch. What the numbers
argue against is a second stack *for no reason*; what they argue for is
Hermes in the TypeScript slot until Static Hermes, or a phone measurement,
changes the shape.

## 1. The question, and what was already known

LLP 1027 put the lean Hermes VM behind the seam for TypeScript and measured
it (§5): +1.81 MB linked, 7.5–29× native per call, in microseconds. LLP
1028 measured the wasm engines for the Rust half and found the runtime
smaller than the interpreter LLP 1026 designed around; LLP 1029 proposed
wasmtime in the slot, opt-in, beside Hermes. Charlie's question is whether
the two slots could be one: the TypeScript half compiled to wasm and run by
the executor the Rust half already needs.

Nobody had measured a JavaScript engine *under* a wasm interpreter on this
program's workload. LLP 1028 F8 estimated the reverse direction (wasm
libraries under Hermes) and said the cost was the boundary; this is the
other case, and the cost turns out to be the interpreter.

## 2. Method

A scratch directory outside the repo (§6 has every file verbatim); nothing
was added to the repo but this document.

- **The JavaScript.** `js/tests/fixtures/caltrain.ts` — Caltrain's data
  source in TypeScript, the executor's own fixture — bundled by the repo's
  Rolldown to the same IIFE the bake makes, and `js/src/prelude.js`, the
  executor's prelude (the seam, `console`, `fetch`, the store door),
  unchanged. Both compiled twice: `hermesc -O` to Hermes bytecode (what
  exact-js runs), and quickjs-ng's `qjsc -C -s` to QuickJS bytecode with the
  source stripped (what the wasm module runs).
- **The module.** `exact_qjs.c` (§6): quickjs-ng 0.16.2 — `quickjs.c`,
  `libregexp.c`, `libunicode.c`, `dtoa.c`, no `quickjs-libc` — plus a
  hundred lines that create one runtime and context, install `console.*`
  with the shim's `render` (a string as itself, else `JSON.stringify`, else
  `toString`) and a `__exact_host` that answers `undefined`, evaluate the
  prelude's and the twin's bytecode, and export the seam in LLP 1026 D3's
  shape: `exact_alloc`, `exact_init`, `exact_call(src, len, args, len) →
  (ptr << 32 | len)` into linear memory, `exact_error`, `exact_take_log`,
  `exact_string`, `exact_drain`. Compiled for `wasm32-wasip1` with
  Homebrew's llvm 23 against the wasi-libc sysroot, `-mexec-model=reactor`,
  `-O2` (and `-Os` for the size row), then `wasm-opt -O3` with
  `host/web/build.mjs`'s feature flags (`-Oz` over the `-Os` build). The
  module imports five WASI functions — `clock_time_get`, `fd_close`,
  `fd_fdstat_get`, `fd_seek`, `fd_write` — all of them the C runtime's,
  none the seam's; the host stubs every import (`fd_write` as a sink, the
  rest as zeros), so the module reaches nothing.
- **The host.** One Rust binary (§6) under exact2's release profile, linking
  wasmtime 47.0.3 (`runtime` + `std` + `cranelift` + `pulley`, the compiler
  for the bake step only), wasmi 2.0.0, and the repo's own `exact-js`,
  `exact-plan`, `exact-runner`, `caltrain-data`, and `contract`. It compiles
  the module once with Cranelift to precompiled native code and once to
  Pulley bytecode (the bake), writes both, and then runs five executors on
  the same twenty cases as `js/tests/caltrain.rs`, verbatim: the native
  crate (the oracle), the lean Hermes VM through `exact_js::Module::query`
  — exact-js's real path, bind, marshal, budget clock and all — and the
  QuickJS module under wasmtime-precompiled, wasmtime-Pulley, and wasmi,
  through a mirror of exact-js's `begin`/`step` that uses the crate's own
  `to_json`/`from_json` with the plan's shapes. Every executor's answer is
  compared to the crate's `Value::to_bytes()` (or its exact `DataError`),
  and its first `console` line to the shim's.
- **Timing.** LLP 1027 §5's five cases, p50 of 2,000 calls each, the whole
  seam per call (Value → JSON → engine → JSON → Value). Instantiation
  twenty times per executor: module load + instantiate, and runtime
  creation + both bytecodes evaluated (`exact_init`; for Hermes,
  `Module::loaded` + `bind`, which is create + prelude + module + the three
  identity reads).
- **Runs and noise.** Four runs on rustc 1.97.0 (the repo's toolchain, LLP
  1028's) alternating the `-O2` and `-Os` modules, and three earlier on
  rustc 1.94.0 that agree. Absolute numbers moved by up to 1.7× between
  runs — every executor together, the native crate included, so the
  machine (a peer session was building in this checkout) and not the
  engines; the ratios within a run are the guide, and F3 gives their bands.

**What the probe does not measure, said plainly.** QuickJS linked natively
(which would split "QuickJS is slower than Hermes" from "wasm costs this
much"); Javy's own toolchain, which wraps the same engine; a module with an
npm dependency; a JavaScript heap that grows linear memory under an
interpreter; fuel accounting, which was off (LLP 1026 D5's budgets would
add a few percent to Pulley and wasmi); the phone (F7).

## 3. Findings

### F1 — There is no JavaScript-to-wasm without a JavaScript VM

Wasm is a universal target for languages with a static memory model.
JavaScript is not one: prototype chains, dynamic properties, closures over
mutable environments, `eval`, and a garbage-collected heap need a runtime,
and the runtime is the VM. What exists, from public knowledge as of this
date:

| Path | What it is | Runs TypeScript-with-npm? | Shipping? |
|---|---|---|---|
| **Javy** (Bytecode Alliance) | QuickJS compiled to wasm; the program as QuickJS bytecode inside | yes (ES2023, no DOM) | yes — and it is what this probe built by hand |
| **StarlingMonkey** (Fastly, Bytecode Alliance) | SpiderMonkey compiled to wasm, Wizer-snapshotted | yes | yes; 5 MB and up, and no JIT inside wasm |
| **AssemblyScript** | a TypeScript-shaped language with static types and its own runtime | no — different semantics, no npm | yes |
| **Porffor** | an ahead-of-time JS→wasm compiler | a subset; conformance far from complete | pre-alpha |
| **Static Hermes** (Meta) | typed JavaScript compiled ahead of time (native or, experimentally, C/wasm); untyped code falls back to the interpreter | Flow-typed source for the fast path | the engine ships in React Native; the AOT path is experimental |
| **wasm GC** | the proposal that lets managed languages target wasm | Kotlin, Dart, Java, OCaml have front ends; JavaScript has none | n/a |
| **Hermes** | runs no wasm (LLP 1028 F5) | — | — |

So the measurable form of the question is Javy's: the engine inside the
module. Confidence: high on the shape of the landscape; medium on any
detail that moves with a release.

### F2 — The probe runs, and answers the crate's bytes on every engine

Twenty of twenty cases byte-identical to the native crate — six sources,
three argument sets of `board`, three of `search`, six error paths with the
crate's exact messages — under all three wasm executors, and under Hermes
through exact-js, in the same process. The first `console` line is the
same string on all four (`defaultLocation {"lat":37.3947,"lon":-122.0763}`).
LLP 1012's determinism rule held across two JavaScript engines and three
wasm executors without an edit to the twin, the prelude, or the seam.

Confidence: high; reproducible from §6.

### F3 — Speed: an interpreter inside an interpreter

Per call through the seam, microseconds, p50 of 2,000. The first table is
the quietest 1.97.0 run (the Summary's); the second is the band over all
four 1.97.0 runs, each executor against the lean Hermes VM *in the same
run*.

| executor | `defaultLocation` | `station` | `search` | `nearest` | `board` |
|---|---|---|---|---|---|
| native Rust crate | 0.04 | 0.04 | 0.38 | 0.67 | 5.62 |
| lean Hermes VM (exact-js) | 1.8 | 2.5 | 3.5 | 10.4 | 42.2 |
| QuickJS · wasmtime precompiled native | 2.8 | 4.6 | 6.3 | 20.2 | 80.1 |
| QuickJS · wasmtime Pulley | 158.8 | 200.0 | 270.6 | 764.3 | 2,635 |
| QuickJS · wasmi | 80.4 | 96.3 | 129.2 | 302.4 | 969 |

| against Hermes, four runs | `defaultLocation` | `station` | `search` | `nearest` | `board` |
|---|---|---|---|---|---|
| QuickJS · wasmtime precompiled native | 1.5–1.9× | 1.6–2.2× | 1.6–2.1× | 1.9–3.0× | 1.8–2.4× |
| QuickJS · wasmtime Pulley | 88–134× | 80–115× | 77–127× | 74–114× | 40–108× |
| QuickJS · wasmi | 42–64× | 39–59× | 37–53× | 29–46× | 17–40× |

(The three rustc 1.94.0 runs: precompiled 1.6–2.7×, Pulley 61–102×, wasmi
25–45× — the same bands.)

Three readings.

First, **on a desktop the relocated VM is close to Hermes.** Precompiled by
Cranelift, QuickJS in wasm answers at 1.5–3× the lean VM. Some of that is
QuickJS against Hermes and some is wasm's bounds checks and calling
convention; the probe does not split them (§2), and for the question it
does not matter — nothing on a desktop gets faster by the move, and
nothing gets unusably slower.

Second, **on the phone's executors the gap is one to two orders of
magnitude, and it is wider than LLP 1028 predicted.** 1028 F2 put Pulley
at 13× native and wasmi at 5.4× on `fib`; here the same two engines run
the QuickJS module at 33–57× and 12–29× the precompiled form. `fib` is what
a wasm interpreter does best — a hot loop of arithmetic and direct calls.
A JavaScript engine's inner loop is what it does worst: a large `switch`
over opcodes (an indirect branch per bytecode), tagged values loaded and
stored through linear memory on every operation, indirect calls through
tables for every property access and builtin, and a garbage collector
walking the heap — each of which is itself several interpreted
instructions. The ratio narrows on `board` (the largest case) because more
of its time is JSON serialization and allocation, which are memory
traffic in both shapes.

Third, **in absolute terms the seam survives on this Mac and not with
headroom.** `board` is 2.6–5.1 ms per call under Pulley and 1.0–2.1 ms
under wasmi; Caltrain's worst second is two `board` calls per tick — 5–10
ms of a 16.7 ms frame under Pulley on an M5 Max, per resource change, never
per frame. An A-series core is slower than this machine by a factor that
LLP 1027 §10 Q7 still owes for Hermes and that this document owes for the
wasm shape (F7). LLP 1029 D1's paved path is "move a source to Rust when it
measures hot"; under this executor most sources would measure hot.

Confidence: high on the ratios on this machine (seven runs, two toolchains,
two module builds); the absolute numbers carry the ±1.7× run-to-run noise
§2 names; the phone is owed.

### F4 — Size: the relocated VM is not smaller

| | bytes |
|---|---|
| `exact_qjs.wasm` — QuickJS-ng + prelude + twin as bytecode, `-O2`, `wasm-opt -O3` | 938,668 (gzip -9: 342,858) |
| the same, `-Os`, `wasm-opt -Oz` | 636,095 |
| precompiled native code, `aarch64-apple-darwin` (Cranelift, 0.4–0.8 s) | 2,304,040 (`-Os`: 1,754,856) |
| precompiled Pulley bytecode, `pulley64` (0.3–0.8 s) | 2,074,144 (`-Os`: 1,492,144) |
| the twin as QuickJS bytecode (`qjsc -s`) / as Hermes bytecode (`hermesc -O`) | 5,310 / 8,605 |
| the prelude as QuickJS bytecode / as Hermes bytecode | 5,174 / 8,560 |
| for scale: the lean Hermes VM, linked and dead-stripped, macOS arm64 (LLP 1027 §5) | +1,810,256 |
| for scale: wasmtime runtime / + Pulley / wasmi, linked (LLP 1028 F1) | +487,296 / +577,952 / +1,005,328 |

What a phone would carry for the TypeScript half under each design, LLP
1029 D6's accounting:

| design | in the binary | as an artifact | total |
|---|---|---|---|
| Hermes (LLP 1027 D3, today) | 1.81 MB | the app's bytecode, 9 KB for the twin | **1.8 MB** |
| QuickJS in wasm on wasmtime + Pulley | 0.58 MB | the engine precompiled for `pulley64`, 1.5–2.1 MB | **2.1–2.7 MB** |
| QuickJS in wasm on wasmi | 1.0 MB | the neutral module, 0.64–0.94 MB, validated on the device | **1.6–1.9 MB** |

The one shape that comes in under Hermes is wasmi loading the neutral
module — the shape LLP 1029 §6 took *off* the table (no validator on the
device) and the slower-to-load one (F5). The precompiled artifacts are
2.3–2.5× their wasm because Pulley bytecode and machine code are both
larger than the wasm they came from; the neutral module gzips to a third
of its size, but the device stores what it runs. A real app's QuickJS
bytecode is 38% smaller than its Hermes bytecode (the twin, the prelude:
the same ratio twice), which matters for over-the-air updates and for
nothing else at these sizes.

Confidence: high; `stat` of the files §6 produces.

### F5 — Boot: milliseconds, after first pixel

Per executor, p50 of twenty fresh instances, milliseconds; both columns
are what LLP 1027 D4 places after first pixel, so none of it is on the
boot path — and on the phone it is what a user waits for the first
store-reading resource behind.

| executor | module load + instantiate | runtime + prelude + module |
|---|---|---|
| lean Hermes VM (`Module::loaded` + `bind`) | 0.27–0.60 (everything) | — |
| QuickJS · wasmtime precompiled native (deserialize) | 0.19–0.41 | 0.20–0.43 |
| QuickJS · wasmtime Pulley (deserialize) | 0.07–0.23 | 2.6–5.1 |
| QuickJS · wasmi (validate + compile + instantiate) | 2.4–5.3 | 2.7–5.8 |

Runtime creation and two bytecode evaluations are 0.3 ms under Hermes and
under precompiled QuickJS, and 3–6 ms under either iOS interpreter: the
same 10–20× as F3, paid once. wasmi's validate-and-compile of a 0.9 MB
module is another 2–5 ms, which is why LLP 1029 D3 wanted the bake to do
it. Confidence: high on this machine.

### F6 — What one engine would still buy, and what it would cost

The question's third part — does the JavaScript VM add complexity without
benefit — has a precise answer now, because the alternative was built.

What the VM costs today is not the VM; it is the *second stack*: two ABIs
(JSON strings through JSI; bytes in linear memory), two budget mechanisms
(a wall clock for Hermes, fuel for wasm), two artifact formats with two
version couplings (bytecode to Hermes, `.cwasm` to wasmtime and target),
a C++ shim and per-platform Hermes archives the ibex repo produces (the
Linux lean build still owed), and 2.4 MB on a phone for an opted-in mixed
app (LLP 1029 D6). QuickJS in wasm collapses every one of those to one:
one engine in every binary, one ABI, fuel budgets for TypeScript too, an
importless sandbox by construction instead of by the lean VM's inability
to compile, no C++ in any host (the C compiles once, in the bake, into an
artifact pinned by digest — the same shape as the Hermes archive's
receipt), one over-the-air path for both halves, and the web unchanged
(the browser keeps running the JavaScript itself; the module is the
native executor's form only). Those are real, and F2 shows they come at no
cost in correctness.

What it costs is F3 and F4: the phone's executor at 40–134× Hermes, and
0.3–0.9 MB more on the phone. The desktop-only version of the trade
(precompiled QuickJS at 1.5–3×, with Hermes kept for iOS) does not
simplify anything — it is a third engine.

Two futures would change the shape, and neither is this year's: **Static
Hermes** compiling typed TypeScript ahead of time to native or wasm, which
is the only path on the map to one engine without an interpreter inside
it, and which ibex tracks (LLP 1027 §8a); and a wasm interpreter on iOS
fast enough that a JavaScript engine under it is within a small factor of
native Hermes, which neither Pulley nor wasmi is today by an order of
magnitude.

Confidence: high on the accounting (it is LLP 1029's, with F3–F5's
numbers); the futures are public roadmaps, medium.

### F7 — The phone: owed, and one plug-in away

No iPhone was connected on 2026-09-04 (`devicectl` lists Charlie's as
paired and unavailable). Two things are in hand for the run: wasmtime's
runtime + Pulley builds for `aarch64-apple-ios` on rustc 1.97.0 as a
static library (`ioscheck/` in §6 — Finished, 7.3 MB unstripped), so the
Pulley executor links into the iOS app; and the module's Pulley artifact is
the same file on every host. What the run needs: that library and the
probe's seam behind a debug hook in `ExactIOS`, the same twenty cases, and
the lean Hermes VM on the device — which is LLP 1027 §10 Q7's own owed
number and has to be built (`hermes.xcframework`'s device slice is the
full VM, 4.86 MB; the lean static link on iOS is not yet produced). The
ratio F3 measured should hold on the phone to within the difference
between how the two interpreters' inner loops fare on an A-series core;
the absolute will be worse than this Mac's by the factor LLP 1027 §5 also
declined to guess.

## 4. Confidence, gathered

| Claim | Basis |
|---|---|
| No JS→wasm without a VM (F1) | public landscape as of this date; medium on release-level detail |
| 20/20 bytes, identical logs, on four engines (F2) | run, this Mac; reproducible from §6 |
| The per-call ratios (F3) | seven runs, two toolchains, two module builds; high |
| The absolute microseconds (F3, F5) | this Mac, ±1.7× run-to-run; medium |
| Why the interpreter-in-interpreter gap is wider than `fib`'s | reasoning from the engines' shape; not profiled |
| Sizes (F4) | `stat`; high |
| The phone accounting (F4) | arithmetic over F4 and LLP 1027 §5 / 1028 F1; the device slice sizes are owed |
| Static Hermes and Pulley's roadmap (F6) | public; medium |
| Any phone number | owed (F7) |

## 5. What this feeds

- **LLP 1029 §8** gains the question this document answers — *could the
  TypeScript half share the wasm executor?* — with its number: not on the
  phone's interpreter, by one to two orders of magnitude, and not for
  size. D6's rule ("the lean Hermes VM if and only if there is an
  `app.ts`, one wasm runtime if and only if the composition names
  `Swappable`") stands on this measurement rather than on 1027's
  assumption.
- **LLP 1029 §7 stage 0** (the phone numbers) gains a third row beside
  Pulley-vs-wasmi and Hermes-on-device: this module under both, on the
  same device, the same afternoon.
- **LLP 1028 F2's caveat** — that `fib` is the interpreters' best case —
  now has its worst case measured: 33–57× between Pulley and precompiled
  code on an engine's dispatch loop, against 13× on `fib`.

Nothing in the repo changes on this document's account.

## 6. Appendix — the probe, verbatim

A scratch directory outside the repo (the session's scratchpad held it): `js/` (the twin bundled by the repo's Rolldown and compiled by `hermesc -O` and by quickjs-ng's `qjsc`), `wasm/` (the C module and its build), `host/` (the Rust probe). Nothing here entered the repo.

### The bytecode

```sh
# from the repo root; S is the scratch directory
node_modules/.bin/rolldown js/tests/fixtures/caltrain.ts --format iife --file $S/js/caltrain.js
../ibex/tools/hermes-vanilla/hermesc-macos-arm64 -O -emit-binary -out $S/js/caltrain.hbc $S/js/caltrain.js
cp js/src/prelude.js $S/js/prelude.js
# quickjs-ng d8e1cc6 (v0.16.2), cloned into $S/quickjs-ng; qjsc built natively with cmake --target qjsc
Q=$S/quickjs-ng/build-native/qjsc
$Q -C -s -N prelude_bc  -o $S/js/prelude_bc.c  $S/js/prelude.js
$Q -C -s -N caltrain_bc -o $S/js/caltrain_bc.c $S/js/caltrain.js
$Q -C -s -b -o $S/js/caltrain.qbc $S/js/caltrain.js   # 5,310 B (Hermes: 8,605 B)
$Q -C -s -b -o $S/js/prelude.qbc  $S/js/prelude.js    # 5,174 B (Hermes: 8,560 B)
```

### `wasm/exact_qjs.c`

```c
// QuickJS-ng inside a wasm module, behind exact2's data seam (the probe for
// the question "does wasm make Hermes unnecessary?"). The ABI mirrors LLP
// 1026 D3's shape: bytes in linear memory, one call per source, a JSON reply
// — the prelude's `__exact_call(source, argsJson)` exactly as exact-js drives
// it through JSI on Hermes.
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "quickjs.h"

extern const uint32_t prelude_bc_size;
extern const uint8_t prelude_bc[];
extern const uint32_t caltrain_bc_size;
extern const uint8_t caltrain_bc[];

#define EXPORT(name) __attribute__((export_name(name)))

typedef struct { char *p; size_t len, cap; } Buf;
static Buf result, logbuf, errbuf;

static void buf_push(Buf *b, const char *s, size_t n) {
  if (b->len + n > b->cap) {
    size_t c = b->cap ? b->cap * 2 : 4096;
    while (c < b->len + n) c *= 2;
    b->p = realloc(b->p, c);
    b->cap = c;
  }
  memcpy(b->p + b->len, s, n);
  b->len += n;
}
static uint64_t pack(const Buf *b) {
  return ((uint64_t)(uintptr_t)b->p << 32) | (uint64_t)b->len;
}

static JSRuntime *rt;
static JSContext *ctx;
static JSValue g_call;

EXPORT("exact_alloc") uint32_t exact_alloc(uint32_t n) { return (uint32_t)(uintptr_t)malloc(n); }
EXPORT("exact_free") void exact_free(uint32_t p) { free((void *)(uintptr_t)p); }
EXPORT("exact_error") uint64_t exact_error(void) { return pack(&errbuf); }
EXPORT("exact_take_log") uint64_t exact_take_log(void) {
  uint64_t r = pack(&logbuf);
  logbuf.len = 0;
  return r;
}

static void push_string(Buf *b, JSContext *c, JSValueConst v) {
  size_t n;
  const char *s = JS_ToCStringLen(c, &n, v);
  if (s) { buf_push(b, s, n); JS_FreeCString(c, s); }
}

static void take_exception(void) {
  JSValue e = JS_GetException(ctx);
  errbuf.len = 0;
  push_string(&errbuf, ctx, e);
  JSValue st = JS_GetPropertyStr(ctx, e, "stack");
  if (JS_IsString(st)) { buf_push(&errbuf, "\n", 1); push_string(&errbuf, ctx, st); }
  JS_FreeValue(ctx, st);
  JS_FreeValue(ctx, e);
}

// console.*: the shim's `render` — a string as itself, else JSON.stringify,
// else toString — joined by spaces, one line per call.
static JSValue js_console(JSContext *c, JSValueConst this_val, int argc, JSValueConst *argv) {
  for (int i = 0; i < argc; i++) {
    if (i) buf_push(&logbuf, " ", 1);
    if (JS_IsString(argv[i])) { push_string(&logbuf, c, argv[i]); continue; }
    JSValue j = JS_JSONStringify(c, argv[i], JS_UNDEFINED, JS_UNDEFINED);
    if (JS_IsString(j)) push_string(&logbuf, c, j);
    else { JS_FreeValue(c, j); j = JS_UNDEFINED; push_string(&logbuf, c, argv[i]); }
    JS_FreeValue(c, j);
  }
  buf_push(&logbuf, "\n", 1);
  return JS_UNDEFINED;
}

// `__exact_host(op, a, b)`: the store and the request ticket path. The
// probe's twenty cases touch neither; undefined is "nothing stored".
static JSValue js_host(JSContext *c, JSValueConst this_val, int argc, JSValueConst *argv) {
  return JS_UNDEFINED;
}

static int eval_bytecode(const uint8_t *buf, uint32_t len) {
  JSValue obj = JS_ReadObject(ctx, buf, len, JS_READ_OBJ_BYTECODE);
  if (JS_IsException(obj)) { take_exception(); return 1; }
  JSValue r = JS_EvalFunction(ctx, obj);
  if (JS_IsException(r)) { take_exception(); return 1; }
  JS_FreeValue(ctx, r);
  return 0;
}

// 0 ok; 1 the module threw (exact_error says); 2/3 no runtime/context.
EXPORT("exact_init") int32_t exact_init(void) {
  if (rt) return 0;
  rt = JS_NewRuntime();
  if (!rt) return 2;
  JS_SetMaxStackSize(rt, 512 * 1024);
  JS_SetMemoryLimit(rt, 64u << 20);
  ctx = JS_NewContext(rt);
  if (!ctx) return 3;
  JSValue g = JS_GetGlobalObject(ctx);
  JSValue console = JS_NewObject(ctx);
  static const char *levels[] = {"log", "info", "warn", "error", "debug"};
  for (int i = 0; i < 5; i++)
    JS_SetPropertyStr(ctx, console, levels[i], JS_NewCFunction(ctx, js_console, levels[i], 1));
  JS_SetPropertyStr(ctx, g, "console", console);
  JS_SetPropertyStr(ctx, g, "__exact_host", JS_NewCFunction(ctx, js_host, "__exact_host", 3));
  int bad = eval_bytecode(prelude_bc, prelude_bc_size) || eval_bytecode(caltrain_bc, caltrain_bc_size);
  if (!bad) {
    g_call = JS_GetPropertyStr(ctx, g, "__exact_call");
    if (!JS_IsFunction(ctx, g_call)) {
      errbuf.len = 0;
      buf_push(&errbuf, "no __exact_call", 15);
      bad = 1;
    }
  }
  JS_FreeValue(ctx, g);
  return bad;
}

// One string in the module's global object (`exact.abi`, `exact.appId`, `exact.grants`).
EXPORT("exact_string") uint64_t exact_string(uint32_t np, uint32_t nl) {
  result.len = 0;
  JSValue g = JS_GetGlobalObject(ctx);
  JSValue ex = JS_GetPropertyStr(ctx, g, "exact");
  JS_FreeValue(ctx, g);
  char name[64];
  if (nl >= sizeof name) return 0;
  memcpy(name, (const char *)(uintptr_t)np, nl);
  name[nl] = 0;
  JSValue v = JS_GetPropertyStr(ctx, ex, name);
  JS_FreeValue(ctx, ex);
  push_string(&result, ctx, v);
  JS_FreeValue(ctx, v);
  return pack(&result);
}

// `__exact_call(source, argsJson)` → the JSON reply in (ptr << 32 | len);
// 0 when the module threw, with exact_error() the reason.
EXPORT("exact_call") uint64_t exact_call(uint32_t sp, uint32_t sl, uint32_t ap, uint32_t al) {
  result.len = 0;
  JSValue args[2] = {
    JS_NewStringLen(ctx, (const char *)(uintptr_t)sp, sl),
    JS_NewStringLen(ctx, (const char *)(uintptr_t)ap, al),
  };
  JSValue r = JS_Call(ctx, g_call, JS_UNDEFINED, 2, args);
  JS_FreeValue(ctx, args[0]);
  JS_FreeValue(ctx, args[1]);
  if (JS_IsException(r)) { take_exception(); return 0; }
  push_string(&result, ctx, r);
  JS_FreeValue(ctx, r);
  return pack(&result);
}

// Drain the microtask queue (an async answer); 0 ok, 1 a job threw.
EXPORT("exact_drain") int32_t exact_drain(void) {
  JSContext *c;
  int n;
  while ((n = JS_ExecutePendingJob(rt, &c)) > 0) {}
  if (n < 0) { take_exception(); return 1; }
  return 0;
}
```

### `wasm/build.sh`

```sh
#!/bin/sh
# QuickJS-ng + the prelude + the twin, as one wasm32 module: exact2's bake
# shape for a data module (wasm-opt after), built with brew's llvm against
# the wasi-libc sysroot. Usage: build.sh [-O2|-Os|-Oz] [suffix]
set -e
S=$(cd "$(dirname "$0")/.." && pwd)
OPT=${1:--O2}
OUT=$S/wasm/exact_qjs${2:-}.wasm
CLANG=/opt/homebrew/opt/llvm/bin/clang
SYSROOT=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot
Q=$S/quickjs-ng
$CLANG --target=wasm32-wasip1 --sysroot=$SYSROOT $OPT -DNDEBUG \
  -D_WASI_EMULATED_PROCESS_CLOCKS -D_WASI_EMULATED_SIGNAL -funsigned-char \
  -Wno-implicit-fallthrough -Wno-sign-compare -Wno-unused-parameter \
  -I$Q -mexec-model=reactor \
  -Wl,--strip-debug -Wl,-z,stack-size=2097152 -Wl,--export=_initialize \
  -o "$OUT" \
  $S/wasm/exact_qjs.c $Q/quickjs.c $Q/libregexp.c $Q/libunicode.c $Q/dtoa.c \
  $S/js/prelude_bc.c $S/js/caltrain_bc.c \
  -lwasi-emulated-process-clocks -lwasi-emulated-signal -lm
ls -l "$OUT"
```

Then, the bake's own step: `wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --strip-debug --strip-producers` (`host/web/build.mjs`'s flags with `-O3` for `-Oz`), and `-Oz` over the `-Os` build for the size row.

### `host/Cargo.toml`

```toml
[package]
name = "qjsprobe"
version = "0.0.0"
edition = "2021"

[dependencies]
wasmtime = { version = "47.0.3", default-features = false, features = ["runtime", "std", "cranelift", "pulley"] }
wasmi = { version = "2.0.0", default-features = false, features = ["std", "validate", "stable"] }
exact-plan = { path = "/Users/ccheever/projects/exact2/plan" }
exact-runner = { path = "/Users/ccheever/projects/exact2/runner" }
exact-js = { path = "/Users/ccheever/projects/exact2/js" }
caltrain-data = { path = "/Users/ccheever/projects/exact2/apps/caltrain/data" }
contract = { path = "/Users/ccheever/projects/exact2/contract/cli" }
serde_json = "1"

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"

[patch.crates-io]
taffy = { path = "/Users/ccheever/projects/exact2/vendor/taffy" }
```

### `host/src/main.rs`

```rust
//! The probe: QuickJS-ng inside a wasm module (`wasm/exact_qjs*.wasm`) under
//! wasmtime with precompiled native code, wasmtime with Pulley, and wasmi —
//! against the lean Hermes VM (`exact-js`) and the native crate — on
//! Caltrain's twenty cases, held to the crate's bytes, timed per call through
//! the seam exactly as exact-js does it (Value → JSON → engine → JSON → Value).
use caltrain_data::{Caltrain, DAY_START_MS, DEFAULT_LOCATION};
use exact_js::{from_json, to_json, Shape};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Sig {
    params: Vec<Shape>,
    result: Shape,
}

/// exact-js's `bind`, verbatim in shape.
fn sigs(plan: &Plan) -> HashMap<String, Sig> {
    let mut out = HashMap::new();
    for row in &plan.sources {
        let start = row.params.start as usize;
        let end = start + row.params.len as usize;
        let params: Result<Vec<Shape>, String> = plan.source_params[start..end]
            .iter()
            .map(|p| Shape::from_plan(plan, p.ty))
            .collect();
        let result = Shape::from_plan(plan, row.ty);
        if let (Ok(params), Ok(result)) = (params, result) {
            out.insert(plan.str(row.name).to_string(), Sig { params, result });
        }
    }
    out
}

/// One engine behind the prelude's `__exact_call(source, argsJson)`.
trait Seam {
    fn call(&mut self, source: &str, args_json: &str) -> Result<String, String>;
    fn take_log(&mut self) -> String;
}

/// exact-js's `begin` + `step`, for a synchronous answer.
fn query(
    seam: &mut dyn Seam,
    sigs: &HashMap<String, Sig>,
    source: &str,
    args: &[Value],
) -> Result<Value, DataError> {
    let Some(sig) = sigs.get(source) else {
        return Err(DataError::UnknownSource(source.to_string()));
    };
    if args.len() != sig.params.len() {
        return Err(DataError::BadArguments(format!(
            "expected {} arguments, got {}",
            sig.params.len(),
            args.len()
        )));
    }
    let mut json_args = Vec::with_capacity(args.len());
    for (i, (arg, shape)) in args.iter().zip(&sig.params).enumerate() {
        json_args
            .push(to_json(arg, shape).map_err(|_| DataError::BadArguments(format!("argument {i}")))?);
    }
    let args_text = Json::Array(json_args).to_string();
    let text = seam
        .call(source, &args_text)
        .map_err(|e| DataError::Unavailable(format!("`{source}` threw: {e}")))?;
    let reply: Json = serde_json::from_str(&text).map_err(|e| {
        DataError::Unavailable(format!("`{source}` answered something other than JSON: {e}"))
    })?;
    let num = |k: &str| reply.get(k).and_then(Json::as_u64);
    match num("tag") {
        Some(0) => from_json(reply.get("value").unwrap_or(&Json::Null), &sig.result)
            .map_err(|e| DataError::Unavailable(format!("`{source}` answered outside its shape: {e}"))),
        Some(2) => {
            let message = reply
                .get("message")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            Err(match reply.get("kind").and_then(Json::as_str) {
                Some("UnknownSource") => DataError::UnknownSource(message),
                Some("BadArguments") => DataError::BadArguments(message),
                _ => DataError::Unavailable(message),
            })
        }
        other => Err(DataError::Unavailable(format!("`{source}` answered tag {other:?}"))),
    }
}

const SCRATCH: u32 = 1 << 16;

fn unpack(packed: u64) -> (usize, usize) {
    ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize)
}

/// The iovs of a WASI `fd_write`, summed: a sink that reports every byte written.
fn iov_total(data: &[u8], iovs: usize, n: usize) -> u32 {
    (0..n)
        .map(|i| {
            let off = iovs + i * 8 + 4;
            u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
        })
        .sum()
}

mod wt {
    use super::{iov_total, unpack, Seam, SCRATCH};
    use std::time::Instant;
    use wasmtime::*;

    pub struct Host {
        store: Store<()>,
        memory: Memory,
        call: TypedFunc<(u32, u32, u32, u32), u64>,
        error: TypedFunc<(), u64>,
        log: TypedFunc<(), u64>,
        scratch: u32,
    }

    /// Every import stubbed: WASI is the C runtime's, never the seam's.
    /// `fd_write` is a sink, the rest return zeros.
    fn link(engine: &Engine, module: &Module) -> Linker<()> {
        let mut linker = Linker::new(engine);
        for imp in module.imports() {
            let ExternType::Func(ft) = imp.ty() else { continue };
            let name = imp.name().to_string();
            let results: Vec<ValType> = ft.results().collect();
            linker
                .func_new(imp.module(), imp.name(), ft.clone(), move |mut caller, params, out| {
                    if name == "fd_write" {
                        let iovs = params[1].unwrap_i32() as usize;
                        let n = params[2].unwrap_i32() as usize;
                        let nw = params[3].unwrap_i32() as usize;
                        let mem = caller.get_export("memory").and_then(|e| e.into_memory()).unwrap();
                        let total = iov_total(mem.data(&caller), iovs, n);
                        mem.write(&mut caller, nw, &total.to_le_bytes()).unwrap();
                        out[0] = Val::I32(0);
                        return Ok(());
                    }
                    for (v, t) in out.iter_mut().zip(&results) {
                        *v = t.default_value().expect("a default value");
                    }
                    Ok(())
                })
                .unwrap();
        }
        linker
    }

    /// Instantiate and initialize: returns the host and (instantiate ms, exact_init ms).
    pub fn new(engine: &Engine, module: &Module) -> Result<(Host, [f64; 2]), String> {
        let linker = link(engine, module);
        let mut store = Store::new(engine, ());
        let t = Instant::now();
        let instance = linker.instantiate(&mut store, module).map_err(|e| e.to_string())?;
        if let Ok(init) = instance.get_typed_func::<(), ()>(&mut store, "_initialize") {
            init.call(&mut store, ()).map_err(|e| e.to_string())?;
        }
        let t_inst = t.elapsed().as_secs_f64() * 1e3;
        let memory = instance.get_memory(&mut store, "memory").ok_or("no memory export")?;
        let f = |store: &mut Store<()>, name: &str| instance.get_func(&mut *store, name).ok_or(format!("no export {name}"));
        let alloc = f(&mut store, "exact_alloc")?.typed::<u32, u32>(&store).map_err(|e| e.to_string())?;
        let call = f(&mut store, "exact_call")?.typed::<(u32, u32, u32, u32), u64>(&store).map_err(|e| e.to_string())?;
        let error = f(&mut store, "exact_error")?.typed::<(), u64>(&store).map_err(|e| e.to_string())?;
        let log = f(&mut store, "exact_take_log")?.typed::<(), u64>(&store).map_err(|e| e.to_string())?;
        let init = f(&mut store, "exact_init")?.typed::<(), i32>(&store).map_err(|e| e.to_string())?;
        let t = Instant::now();
        let rc = init.call(&mut store, ()).map_err(|e| e.to_string())?;
        let t_init = t.elapsed().as_secs_f64() * 1e3;
        let mut host = Host { store, memory, call, error, log, scratch: 0 };
        if rc != 0 {
            let e = host.error_text();
            return Err(format!("exact_init returned {rc}: {e}"));
        }
        host.scratch = alloc.call(&mut host.store, SCRATCH).map_err(|e| e.to_string())?;
        Ok((host, [t_inst, t_init]))
    }

    impl Host {
        fn read(&self, packed: u64) -> String {
            let (ptr, len) = unpack(packed);
            String::from_utf8_lossy(&self.memory.data(&self.store)[ptr..ptr + len]).into_owned()
        }
        fn error_text(&mut self) -> String {
            let p = self.error.call(&mut self.store, ()).unwrap_or(0);
            self.read(p)
        }
    }

    impl Seam for Host {
        fn call(&mut self, source: &str, args: &str) -> Result<String, String> {
            assert!(source.len() + args.len() <= SCRATCH as usize);
            let sp = self.scratch;
            let ap = sp + source.len() as u32;
            self.memory.write(&mut self.store, sp as usize, source.as_bytes()).map_err(|e| e.to_string())?;
            self.memory.write(&mut self.store, ap as usize, args.as_bytes()).map_err(|e| e.to_string())?;
            let packed = self
                .call
                .call(&mut self.store, (sp, source.len() as u32, ap, args.len() as u32))
                .map_err(|e| e.to_string())?;
            if packed == 0 {
                return Err(self.error_text());
            }
            Ok(self.read(packed))
        }
        fn take_log(&mut self) -> String {
            let p = self.log.call(&mut self.store, ()).unwrap_or(0);
            self.read(p)
        }
    }
}

mod wi {
    use super::{iov_total, unpack, Seam, SCRATCH};
    use std::time::Instant;
    use wasmi::*;

    pub struct Host {
        store: Store<()>,
        memory: Memory,
        call: TypedFunc<(u32, u32, u32, u32), u64>,
        error: TypedFunc<(), u64>,
        log: TypedFunc<(), u64>,
        scratch: u32,
    }

    fn link(engine: &Engine, module: &Module) -> Linker<()> {
        let mut linker = <Linker<()>>::new(engine);
        for imp in module.imports() {
            let ExternType::Func(ft) = imp.ty() else { continue };
            let name = imp.name().to_string();
            let results: Vec<ValType> = ft.results().to_vec();
            linker
                .func_new(imp.module(), imp.name(), ft.clone(), move |mut caller: Caller<'_, ()>, params: &[Val], out: &mut [Val]| {
                    if name == "fd_write" {
                        let iovs = params[1].i32().unwrap() as usize;
                        let n = params[2].i32().unwrap() as usize;
                        let nw = params[3].i32().unwrap() as usize;
                        let mem = caller.get_export("memory").and_then(Extern::into_memory).unwrap();
                        let total = iov_total(mem.data(&caller), iovs, n);
                        mem.write(&mut caller, nw, &total.to_le_bytes()).unwrap();
                        out[0] = Val::I32(0);
                        return Ok(());
                    }
                    for (v, t) in out.iter_mut().zip(&results) {
                        *v = Val::default_for_ty(*t);
                    }
                    Ok(())
                })
                .unwrap();
        }
        linker
    }

    pub fn new(engine: &Engine, module: &Module) -> Result<(Host, [f64; 2]), String> {
        let linker = link(engine, module);
        let mut store = Store::new(engine, ());
        let t = Instant::now();
        let instance = linker
            .instantiate_and_start(&mut store, module)
            .map_err(|e| e.to_string())?;
        if let Ok(init) = instance.get_typed_func::<(), ()>(&store, "_initialize") {
            init.call(&mut store, ()).map_err(|e| e.to_string())?;
        }
        let t_inst = t.elapsed().as_secs_f64() * 1e3;
        let memory = instance.get_memory(&store, "memory").ok_or("no memory export")?;
        let alloc = instance.get_typed_func::<u32, u32>(&store, "exact_alloc").map_err(|e| e.to_string())?;
        let call = instance.get_typed_func::<(u32, u32, u32, u32), u64>(&store, "exact_call").map_err(|e| e.to_string())?;
        let error = instance.get_typed_func::<(), u64>(&store, "exact_error").map_err(|e| e.to_string())?;
        let log = instance.get_typed_func::<(), u64>(&store, "exact_take_log").map_err(|e| e.to_string())?;
        let init = instance.get_typed_func::<(), i32>(&store, "exact_init").map_err(|e| e.to_string())?;
        let t = Instant::now();
        let rc = init.call(&mut store, ()).map_err(|e| e.to_string())?;
        let t_init = t.elapsed().as_secs_f64() * 1e3;
        let mut host = Host { store, memory, call, error, log, scratch: 0 };
        if rc != 0 {
            let e = host.error_text();
            return Err(format!("exact_init returned {rc}: {e}"));
        }
        host.scratch = alloc.call(&mut host.store, SCRATCH).map_err(|e| e.to_string())?;
        Ok((host, [t_inst, t_init]))
    }

    impl Host {
        fn read(&self, packed: u64) -> String {
            let (ptr, len) = unpack(packed);
            String::from_utf8_lossy(&self.memory.data(&self.store)[ptr..ptr + len]).into_owned()
        }
        fn error_text(&mut self) -> String {
            let p = self.error.call(&mut self.store, ()).unwrap_or(0);
            self.read(p)
        }
    }

    impl Seam for Host {
        fn call(&mut self, source: &str, args: &str) -> Result<String, String> {
            assert!(source.len() + args.len() <= SCRATCH as usize);
            let sp = self.scratch;
            let ap = sp + source.len() as u32;
            self.memory.write(&mut self.store, sp as usize, source.as_bytes()).map_err(|e| e.to_string())?;
            self.memory.write(&mut self.store, ap as usize, args.as_bytes()).map_err(|e| e.to_string())?;
            let packed = self
                .call
                .call(&mut self.store, (sp, source.len() as u32, ap, args.len() as u32))
                .map_err(|e| e.to_string())?;
            if packed == 0 {
                return Err(self.error_text());
            }
            Ok(self.read(packed))
        }
        fn take_log(&mut self) -> String {
            let p = self.log.call(&mut self.store, ()).unwrap_or(0);
            self.read(p)
        }
    }
}

fn loc(lat: f64, lon: f64) -> Value {
    Value::record(vec![Value::Number(lat), Value::Number(lon)])
}

/// `js/tests/caltrain.rs`'s twenty, verbatim.
fn cases() -> Vec<(&'static str, Vec<Value>)> {
    let (lat, lon) = DEFAULT_LOCATION;
    let noon = DAY_START_MS + 12.0 * 3_600_000.0;
    let s = Value::str;
    let n = Value::Number;
    vec![
        ("defaultLocation", vec![]),
        ("stations", vec![loc(lat, lon)]),
        ("nearest", vec![loc(lat, lon), n(3.0)]),
        ("nearest", vec![loc(37.7, -122.4), n(9.0)]),
        ("station", vec![s("paloalto"), loc(lat, lon)]),
        ("board", vec![s("mv"), s("north"), n(noon)]),
        ("board", vec![s("mv"), s("south"), n(DAY_START_MS + 7.0 * 3_600_000.0)]),
        ("board", vec![s("sf"), s("north"), n(noon)]),
        ("board", vec![s("sj"), s("north"), n(DAY_START_MS)]),
        ("search", vec![s("Palo"), loc(lat, lon)]),
        ("search", vec![s("san"), loc(lat, lon)]),
        ("search", vec![s(""), loc(lat, lon)]),
        ("nearest", vec![loc(lat, lon), n(10.0)]),
        ("nearest", vec![loc(lat, lon), n(1.5)]),
        ("stations", vec![]),
        ("stations", vec![loc(91.0, 0.0)]),
        ("station", vec![s("nowhere"), loc(lat, lon)]),
        ("board", vec![s("mv"), s("east"), n(noon)]),
        ("board", vec![n(7.0), s("north"), n(noon)]),
        ("bogus", vec![]),
    ]
}

/// LLP 1027 §5's five timed cases, as indices into `cases()`.
const TIMED: [(usize, &str); 5] = [
    (0, "defaultLocation"),
    (4, "station"),
    (9, "search"),
    (2, "nearest"),
    (5, "board"),
];
const N_TIMED: usize = 2000;
const N_INST: usize = 20;

fn p50(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

type Bytes = Result<Vec<u8>, DataError>;

fn outcome(r: Result<Value, DataError>) -> Bytes {
    r.map(|v| v.to_bytes())
}

struct Report {
    name: String,
    inst: [f64; 2],
    equal: usize,
    mismatches: Vec<String>,
    timed: Vec<f64>,
    log: String,
}

fn run_seam(
    name: &str,
    sigs: &HashMap<String, Sig>,
    expected: &[Bytes],
    mut make: impl FnMut() -> Result<(Box<dyn Seam>, [f64; 2]), String>,
) -> Report {
    let mut inst_a = Vec::new();
    let mut inst_b = Vec::new();
    let mut seam = None;
    for _ in 0..N_INST {
        let (s, t) = make().unwrap_or_else(|e| panic!("{name}: {e}"));
        inst_a.push(t[0]);
        inst_b.push(t[1]);
        seam = Some(s);
    }
    let mut seam = seam.unwrap();
    let cs = cases();
    let mut equal = 0;
    let mut mismatches = Vec::new();
    let log = {
        let _ = seam.take_log();
        let r = query(&mut *seam, sigs, cs[0].0, &cs[0].1);
        let _ = r;
        seam.take_log()
    };
    for (i, (source, args)) in cs.iter().enumerate() {
        let got = outcome(query(&mut *seam, sigs, source, args));
        if got == expected[i] {
            equal += 1;
        } else {
            mismatches.push(format!("{source} {args:?}: got {got:?}, expected {:?}", expected[i]));
        }
    }
    let mut timed = Vec::new();
    for (i, _) in TIMED {
        let (source, args) = &cs[i];
        let mut v = Vec::with_capacity(N_TIMED);
        for _ in 0..N_TIMED {
            let t = Instant::now();
            let _ = query(&mut *seam, sigs, source, args);
            v.push(t.elapsed().as_nanos() as f64 / 1e3);
        }
        timed.push(p50(&mut v));
    }
    Report { name: name.to_string(), inst: [p50(&mut inst_a), p50(&mut inst_b)], equal, mismatches, timed, log }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn main() {
    let probe = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let wasm_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| probe.join("wasm/exact_qjs.opt.wasm"));
    let wasm = std::fs::read(&wasm_path).expect("the wasm module");
    let plan = contract::compile_path(Path::new("/Users/ccheever/projects/exact2/apps/caltrain/app.contract")).expect("app.contract compiles");
    let sigs = sigs(&plan);
    let hbc = std::fs::read(probe.join("js/caltrain.hbc")).expect("caltrain.hbc");
    let qbc = std::fs::read(probe.join("js/caltrain.qbc")).expect("caltrain.qbc");

    // --- the bake: Cranelift once per target -----------------------------
    let engine_native = wasmtime::Engine::default();
    let t = Instant::now();
    let module_native = wasmtime::Module::new(&engine_native, &wasm).expect("cranelift compiles the module");
    let t_native = ms(t);
    let native_cwasm = module_native.serialize().unwrap();
    let native_path = wasm_path.with_extension("native.cwasm");
    std::fs::write(&native_path, &native_cwasm).unwrap();
    let mut cfg = wasmtime::Config::new();
    cfg.target("pulley64").unwrap();
    let engine_pulley = wasmtime::Engine::new(&cfg).unwrap();
    let t = Instant::now();
    let pulley_cwasm = engine_pulley.precompile_module(&wasm).unwrap();
    let t_pulley = ms(t);
    let pulley_path = wasm_path.with_extension("pulley.cwasm");
    std::fs::write(&pulley_path, &pulley_cwasm).unwrap();

    println!("## Artifacts\n");
    println!("| | bytes |\n|---|---|");
    println!("| module {} (QuickJS-ng + prelude + twin bytecode) | {} |", wasm_path.file_name().unwrap().to_string_lossy(), wasm.len());
    println!("| precompiled native (aarch64-apple-darwin) | {} — Cranelift {t_native:.0} ms |", native_cwasm.len());
    println!("| precompiled Pulley bytecode | {} — {t_pulley:.0} ms |", pulley_cwasm.len());
    println!("| twin as QuickJS bytecode (qjsc -s) | {} |", qbc.len());
    println!("| twin as Hermes bytecode (hermesc -O) | {} |", hbc.len());
    println!();

    // --- the oracle ----------------------------------------------------------
    let cs = cases();
    let mut native = Caltrain;
    let expected: Vec<Bytes> = cs.iter().map(|(s, a)| outcome(native.query(s, a))).collect();
    let mut native_timed = Vec::new();
    for (i, _) in TIMED {
        let (source, args) = &cs[i];
        let mut v = Vec::with_capacity(N_TIMED);
        for _ in 0..N_TIMED {
            let t = Instant::now();
            let _ = native.query(source, args);
            v.push(t.elapsed().as_nanos() as f64 / 1e3);
        }
        native_timed.push(p50(&mut v));
    }

    // --- Hermes through exact-js -------------------------------------------
    let mut reports = Vec::new();
    {
        let mut inst = Vec::new();
        let mut m = None;
        for _ in 0..N_INST {
            let t = Instant::now();
            let mut module = exact_js::Module::loaded(hbc.clone(), "com.exact.caltrain", "").expect("the twin loads");
            module.bind(&plan);
            inst.push(ms(t));
            m = Some(module);
        }
        let mut m = m.unwrap();
        let mut equal = 0;
        let mut mismatches = Vec::new();
        let _ = m.take_logs();
        let _ = m.query(cs[0].0, &cs[0].1);
        let log = m.take_logs().join("\n");
        for (i, (source, args)) in cs.iter().enumerate() {
            let got = outcome(m.query(source, args));
            if got == expected[i] { equal += 1 } else { mismatches.push(format!("{source}: got {got:?}")) }
        }
        let mut timed = Vec::new();
        for (i, _) in TIMED {
            let (source, args) = &cs[i];
            let mut v = Vec::with_capacity(N_TIMED);
            for _ in 0..N_TIMED {
                let t = Instant::now();
                let _ = m.query(source, args);
                v.push(t.elapsed().as_nanos() as f64 / 1e3);
            }
            timed.push(p50(&mut v));
        }
        reports.push(Report { name: "lean Hermes (exact-js)".into(), inst: [p50(&mut inst), 0.0], equal, mismatches, timed, log });
    }

    // --- QuickJS in wasm, three executors ------------------------------------
    reports.push(run_seam("QuickJS · wasmtime precompiled native", &sigs, &expected, || {
        let t = Instant::now();
        let module = unsafe { wasmtime::Module::deserialize_file(&engine_native, &native_path) }.map_err(|e| e.to_string())?;
        let t_load = ms(t);
        let (host, [t_inst, t_init]) = wt::new(&engine_native, &module)?;
        Ok((Box::new(host) as Box<dyn Seam>, [t_load + t_inst, t_init]))
    }));
    reports.push(run_seam("QuickJS · wasmtime Pulley", &sigs, &expected, || {
        let t = Instant::now();
        let module = unsafe { wasmtime::Module::deserialize_file(&engine_pulley, &pulley_path) }.map_err(|e| e.to_string())?;
        let t_load = ms(t);
        let (host, [t_inst, t_init]) = wt::new(&engine_pulley, &module)?;
        Ok((Box::new(host) as Box<dyn Seam>, [t_load + t_inst, t_init]))
    }));
    let engine_wasmi = wasmi::Engine::default();
    reports.push(run_seam("QuickJS · wasmi", &sigs, &expected, || {
        let t = Instant::now();
        let module = wasmi::Module::new(&engine_wasmi, &wasm[..]).map_err(|e| e.to_string())?;
        let t_load = ms(t);
        let (host, [t_inst, t_init]) = wi::new(&engine_wasmi, &module)?;
        Ok((Box::new(host) as Box<dyn Seam>, [t_load + t_inst, t_init]))
    }));

    // --- the table ---------------------------------------------------------------
    println!("## Per call through the seam, µs, p50 of {N_TIMED} (this Mac, release)\n");
    print!("| executor | load+instantiate ms | runtime+bytecode ms | cases equal |");
    for (_, n) in TIMED { print!(" {n} |"); }
    println!("\n|---|---|---|---|---|---|---|---|---|");
    print!("| native Rust crate | — | — | 20/20 |");
    for v in &native_timed { print!(" {v:.2} |"); }
    println!();
    for r in &reports {
        print!("| {} | {:.3} | {:.3} | {}/20 |", r.name, r.inst[0], r.inst[1], r.equal);
        for (v, n) in r.timed.iter().zip(&native_timed) { print!(" {v:.1} ({:.0}×) |", v / n); }
        println!();
    }
    println!();
    for r in &reports {
        println!("- {}: log after defaultLocation = {:?}", r.name, r.log.trim_end());
        for m in &r.mismatches { println!("  - MISMATCH {m}"); }
    }
}
```

### `ioscheck/` — does the Pulley host build for the phone?

```toml
[package]
name = "ioscheck"
version = "0.0.0"
edition = "2021"
[lib]
crate-type = ["staticlib"]
[dependencies]
wasmtime = { version = "47.0.3", default-features = false, features = ["runtime", "std", "pulley"] }
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

```rust
//! Does wasmtime's runtime + Pulley build for the phone? One function that
//! deserializes a precompiled Pulley module and instantiates it.
use wasmtime::*;
#[no_mangle]
pub extern "C" fn probe_pulley(bytes: *const u8, len: usize) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
    let mut cfg = Config::new();
    if cfg.target("pulley64").is_err() { return 1; }
    let Ok(engine) = Engine::new(&cfg) else { return 2 };
    let Ok(module) = (unsafe { Module::deserialize(&engine, bytes) }) else { return 3 };
    let mut store = Store::new(&engine, ());
    match Instance::new(&mut store, &module, &[]) { Ok(_) => 0, Err(_) => 4 }
}
```

`cargo +1.97.0 build --release --target aarch64-apple-ios`: Finished, `libioscheck.a` 7,309,160 B unstripped (a static archive, not a shipped size).
