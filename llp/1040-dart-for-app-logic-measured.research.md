# LLP 1040: Dart for app logic, measured — against TypeScript on Hermes, on the phone, on the web, and for agents

**Type:** Research
**Status:** Draft
**Systems:** Runner (the `DataSource` executor slot — whether Dart could take TypeScript's place behind the seam), Build (Dart's compilers as bake steps: AOT snapshots, dart2js, dart2wasm, dart2bytecode), Apple host and Linux host (what a binary would embed: a Dart VM and its core snapshot), Web host (Dart's two web targets against the browser running the bake's script), Delivery (what an over-the-air Dart update could be on each platform)
**Author:** Claude (Opus 5) for Charlie Cheever
**Date:** 2026-09-14
**Related:** LLP 1027 §3/§5/D1a/D2/D3/D5/D6/D7/D8/D10 (why TypeScript; the lean Hermes VM measured at +1.81 MB and 1.2–41 µs per call; `fetch`; marshaling; the executor; the bake; the browser as executor; bytecode delivery; mixed with Rust; what a module may use — every row this document holds Dart against), LLP 1027.000 (explicit time and randomness — the guards Dart's JavaScript runtime trips, F5), LLP 1027.001 (language parity — the promise a third language would join), LLP 1028 F2/F4/F5/F6 (the wasm engines; the iOS interpreter choice; where code may be loaded; Shorebird's link percentage), LLP 1029.000 §1/§2 (wasmi on iOS, native libraries on desktop, the store policies it cites; no heap migration), LLP 1030 D1 (the TypeScript row of the delivery inventory), LLP 1032 (the precedent: another route to app logic measured against lean Hermes, with its probe verbatim), LLP 1037 (DartNative's lessons — Dart *above* the seam, a different question), `rules/RULES.md` §Scope (no app JS before first pixel; modules ship as bytecode), `rules/DEFERRED.md` §Authoring models ("Logic below the data seam is TypeScript by default or Rust") and §Surfaces (no executor-private state migration), `CLAUDE.md` (the web is the standard). External, each checked as §4 says: the Dart SDK 3.12.2 on this Mac (Flutter 3.44.2) and dart-lang/sdk's `3.13.0` and `main` sources; the `pkg/dynamic_modules` README; dart-lang/sdk#54394 and #63611; Shorebird's "How we built code push"; wasmtime `release-47.0.0` (RELEASES, `code_translator.rs`, stability docs) and wasmtime#12251; the wasmi README; webassembly.org's `features.json`; dart.dev's number representation page; Flutter's WebAssembly page; GitHub Octoverse 2025; the Stack Overflow Developer Survey 2025; AutoCodeBench (arXiv 2508.09101); arXiv 2602.17955; dart-lang/ai's skills README.

## Summary

Charlie asked, 2026-09-14: *"Would it make any sense to consider Dart as
an alternative to the TS layer of the stack here? How would performance,
size, etc. compare? How would OTA updates be?"* — and, while the answer
was being measured: *"Also consider how good agents are at authoring each
language as part of this evaluation, and how well it will work on the web
platform."* It was answered in conversation the same day; this is the
record he asked for (*"we'll just keep a record of this research"*). It
decides nothing and is not linked into `llp/current/`.

The probe (§6, in the session's scratchpad, outside the repo) ports the
TypeScript twin of Caltrain's data crate (`js/tests/fixtures/caltrain.ts`)
to Dart source by source, and drives the twenty cases of
`js/tests/caltrain.rs` through every Dart compiler. Every target answered
the twin's values, 20 of 20 — and that test holds the twin byte-identical
to the Rust crate.

| | TypeScript (built, LLP 1027) | Dart |
|---|---|---|
| Per call through the JSON seam, this Mac | 0.59–24.5 µs on Hermes | 0.87–31.4 µs compiled AOT; 9.7–344 µs compiled to JavaScript and run on Hermes |
| The engine on a phone | lean Hermes, **+1.81 MB** all-in | **≥2.2 MB**: 1.30 MB of VM machine code + a 0.93 MB core snapshot, before any interpreter |
| The module | 11 KB of bytecode | 38 KB AOT (iOS) · 63 KB dart2js · 59 KB wasm + 11 KB loader · 8.6 KB Dart bytecode |
| An update on iOS | bytecode under an unchanged binary | AOT: never · Dart bytecode: the stock VM refuses it · Shorebird: Flutter only · wasm: loads in neither wasmi nor wasmtime 47 · JavaScript on Hermes: works, at 6–17× |
| On the web | the browser runs the bake's script, 0.2–7 µs | 2–11× slower, 11× larger gzipped as JavaScript, number semantics that differ from native Dart |
| Agents | #1 on GitHub by contributors; 43.6% of developers | 5.9% of developers; no benchmark ranks the two |

The answer as measured: **no — not as a replacement for the TypeScript
layer, and not as a third language now.** Dart's speed is compiled
machine code, and iOS takes no downloaded machine code; every route to
an over-the-air Dart update on iOS is Flutter-only (Shorebird),
experimental and absent from stock runtimes (Dart's own bytecode
interpreter), unloadable by exact2's wasm engines today (dart2wasm), or
6–17× slower than TypeScript on the same interpreter (dart2js on Hermes).
Through the seam's JSON, compiled Dart is not faster than Hermes anyway
(F2): Hermes's JSON and string builtins are C++, and Dart's codec is Dart.
On the web Dart is two compile targets, each larger and slower than the
script TypeScript already is, one of them with number semantics that
differ from native Dart's. The evidence of volume for agents is on
TypeScript's side, and no benchmark ranks the two languages.

§5 names what would reopen it: Dart's dynamic modules in stock AOT
runtimes, dart2wasm emitting the exception instructions wasmtime accepts,
wasmi gaining GC, or a product reason to meet Flutter developers in their
language — for which dart2js on Hermes is an experiment that needs no
runtime change.

## 1. The question, and what was already known

The "TS layer" is LLP 1027's: an app's logic below the data seam as
`app.ts`, bundled by Rolldown and compiled by `hermesc` at bake — to
bytecode for the native hosts, and as the same script for the web — then
run after first pixel by the lean, bytecode-only Hermes VM through
`exact-js`, or by the browser itself in a private realm (1027 D3–D6). Its
measured costs are +1.81 MB linked on macOS arm64 and 1.2–41 µs per call
through the executor (1027 §5). Its update is a bytecode file under an
unchanged binary: a physical iPhone swapped modules over the development
URL (1027 D6), and signed production delivery of module generations is
still owed (1027 D5). Rust holds the hot sources beside it (1027 D8),
replaceable through LLP 1029.000's native libraries on desktop and wasmi
on iOS. LLP 1027.000 refuses ambient time and randomness; LLP 1027.001
promises that an operation can move between the two languages without
changing its data or its permissions.

Two documents measured alternatives in the same slot — LLP 1028 the wasm
engines, LLP 1032 a JavaScript VM inside wasm — and neither considered
another *language*. Dart is the obvious one to ask about: a sound type
system, an AOT compiler, a mobile VM that Flutter ships on iOS every day,
two web compilers, and in Shorebird the one working precedent for code
push over compiled code (1028 F6).

## 2. Method

**The port.** `caltrain.dart` (§6) follows `caltrain.ts` source by source
and message by message, with the seam as a function from a source name
and JSON arguments to a JSON string — `{"value": …}` or
`{"error": {kind, message}}` — the JSON-both-ways shape `exact-js`
crosses (1027 D2). The twenty cases (`cases.txt`) are those of
`js/tests/caltrain.rs`. Answers are compared as parsed JSON, key order
included, with exact float equality, against the twin running on Hermes
(`compare.mjs`). The comparison parses rather than diffs because Dart's
native targets print a whole double as `1787918880000.0` where JavaScript
prints `1787918880000`; the executor's marshaling parses numbers the same
way whichever spelling arrives.

**The targets.** Dart 3.12.2 (Flutter 3.44.2 stable, engine `77e2e94`):
the VM's JIT (`dart run`); AOT (`dart compile exe`, and `aot-snapshot`
for size); dart2js (`-O2`, then `hermesc -O -emit-binary`, run by the
Hermes CLI); dart2wasm (`-O2`, run by Node 24.18.0's V8 and by Bun
1.3.12's JavaScriptCore); dart2bytecode (size only — no stock runtime
loads it, F4). For iOS arm64: Flutter's frontend server (`--aot --tfa`,
product) and `ios-release/gen_snapshot_arm64
--snapshot_kind=app-aot-macho-dylib --strip`, which is the binary a
Flutter release build ships as `App.framework`. TypeScript: the twin
bundled by `bun build --format=iife` with the prelude and harness,
compiled by `hermesc -O`, run by the vanilla Hermes CLI that sits beside
the `hermesc` 1027 §5 used (`~/projects/ibex/tools/hermes-vanilla/` —
the full VM, whose interpreter is the lean VM's); and on V8 and
JavaScriptCore for the web.

**The harness.** One algorithm in Dart (`bench.dart`) and in JavaScript
(`harness.js`): 300 ms of warm-up, then 41 samples, each running batches
of 16 calls until at least 20 ms have passed; per-call time is a sample's
time over its call count; the minimum and the median are printed.
*Seam* is: parse the arguments, answer, serialize the result. *Compute*
is the answer alone, over arguments parsed once. The Hermes CLI has only
`Date.now()`, so the 20 ms floor bounds its quantization at 5%. The five
calls are 1027 §5's.

**Sizes.** `stat` and `gzip -9`. The Dart VM's share of Flutter's iOS
release engine is attributed from that engine's dSYM: every defined
symbol inside `__text`, sized as the gap to the next symbol and
classified by demangled name (`attribute.py`) — machine code only, not
constant data.

**The machine.** Apple M5 Max (6 performance and 12 efficiency cores),
macOS 26.6.2, **shared with other sessions**: load average 12–25
throughout. Engines were alternated round by round (`rounds.sh`), and the
tables report the minimum over rounds — three for dart2js on Hermes,
three for TypeScript on Hermes (against a bundle rebuilt for the final
harness), six for AOT. Ratios hold; second digits do not. The Rust column
is 1027 §5's p50 from 2026-09-03, values in and out without JSON.

**Public facts.** Gathered by three research passes the same day; the
load-bearing ones were then checked against primary sources, or against
source code at the tagged release, and §4 says which is which. Dart's
current stable is 3.13.3; where a fact depends on the release, `3.13.0`'s
source was read.

**Not measured, said plainly.** Anything on a phone. A Dart VM embedded
in a Rust host: none was built (F3 says why). Dart's bytecode interpreter:
no stock runtime has it (F4). dart2wasm under wasmtime: it does not load
(F4). The boundary a Dart executor would add, which for TypeScript is the
difference between F2's Hermes column and 1027 §5's 1.2–41 µs. One note
for whoever reruns it: the first launch of each freshly linked AOT binary
waited 2.5–5 minutes in `dyld` on this Mac while macOS evaluated it;
`rounds.sh` waits that out before timing.

## 3. Findings

### F1 — The port runs on every Dart target and answers the twin's values

| Target | Answers equal to the twin's | A whole double prints as |
|---|---|---|
| Dart VM, JIT | 20 / 20 | `1787918880000.0` |
| Dart AOT (`dart compile exe`) | 20 / 20 | `1787918880000.0` |
| dart2js, as Hermes bytecode | 20 / 20 | `1787918880000` |
| dart2wasm, on V8 | 20 / 20 | `1787918880000.0` |

(Every row is the final draft, `module.dart` over §6's `caltrain.dart`;
the first draft, before F2's record-to-class rewrite, also answered 20 of
20 under AOT.)

`dart analyze` passed the first draft. Four points needed knowing Dart
rather than reading the analyzer, and each would bite a port in either
direction:

- **`List.sort` is not stable.** `dart:core`: *"The sort function is not
  guaranteed to be stable"* (`lib/core/list.dart:485` in 3.12.2).
  JavaScript's sort and Rust's `sort_by` are stable; the port breaks ties
  by original index to keep the twin's order.
- **An index out of range throws**, where JavaScript reads `undefined`;
  each argument accessor checks the length first.
- **`jsonDecode` returns `int` or `double` by spelling** (`3` against
  `3.0`), so numeric arguments are read as `num`.
- **Records in a generic list are a performance cliff** — F2.

Confidence: high; reproducible from §6.

### F2 — Speed: through JSON, AOT Dart is Hermes's equal; alone it is 1.6–3.9× faster; compiled to JavaScript on Hermes it is 6–17× slower

Per call, µs, minimum over rounds (the JIT column is a single run):

| Call | TypeScript on Hermes | Dart AOT | Dart JIT | dart2js on Hermes | Rust (1027 §5) |
|---|---|---|---|---|---|
| **Seam** — JSON both ways | | | | | |
| `defaultLocation()` | 0.59 | 0.87 | 0.96 | 9.69 | |
| `station("paloalto", loc)` | 0.96 | 0.94 | 1.14 | 10.16 | |
| `search("Palo", loc)` | 1.83 | 1.27 | 1.47 | 12.50 | |
| `nearest(loc, 3)` | 7.10 | 3.25 | 3.61 | 41.67 | |
| `board("mv", "north", noon)` | 24.51 | 31.41 | 34.67 | 343.75 | |
| **Compute** — arguments parsed once, nothing serialized | | | | | |
| `defaultLocation()` | 0.30 | 0.39 | 0.54 | 5.30 | 0.04 |
| `station` | 0.41 | 0.14 | 0.18 | 2.41 | 0.08 |
| `search` | 1.36 | 0.35 | 0.54 | 3.66 | 0.29 |
| `nearest` | 6.01 | 1.56 | 1.83 | 21.19 | 0.79 |
| `board` | 18.94 | 11.68 | 15.04 | 178.57 | 5.46 |

Three readings.

- **The seam is builtins, and Hermes's builtins are C++.** Parsing and
  serializing JSON, building strings and allocating small objects dominate
  a seam-shaped call. Hermes's `JSON.parse` and `JSON.stringify` are
  native; Dart's `jsonDecode` and `jsonEncode` are Dart, compiled. Through
  the seam AOT Dart therefore takes 0.46–1.49× Hermes's time: less on
  `nearest` and `search`, the same on `station`, more on `board` (26
  records out) and on `defaultLocation` (whose one line of work is a
  `jsonEncode` for its log line). On computation alone AOT Dart is
  1.6–3.9× faster than Hermes on the four sources that compute, and
  1.2–2.1× slower than 1027 §5's Rust on the same four. The sources hot
  enough to justify leaving an interpreter are already Rust's (1027 D8).
- **Dart compiled to JavaScript carries its runtime into the
  interpreter.** dart2js's maps, its type checks and a JSON encoder written
  in Dart all run as bytecode on Hermes: 6–17× TypeScript's seam time on
  the same engine, 3–17× its compute time.
- **A cliff the types do not show.** The first draft kept `board`'s
  departures as `List<(double, int, Map<String, Object>)>` and sorted it.
  `board_probe.dart` isolates it: 121 µs with the record, 21 µs with a
  three-field class, 13 µs with the class unsorted (AOT; the JIT gives
  125, 17 and 14). The final port uses the class. It is the kind of cost
  an agent writing Dart meets with no signal from the analyzer.

For scale: 1027 §5's TypeScript numbers through the real executor (JSI
crossings and the Rust side's decode included) are 1.2–41 µs, so the
boundary adds roughly 0.6–16 µs to the Hermes seam column. A Dart
executor would add a boundary of its own, unmeasured.

Confidence: high on the ratios on this machine under load; nothing here
ran on a phone, for either language.

### F3 — Size: the Dart VM and its core snapshot outweigh the lean Hermes VM before any interpreter is added

| | TypeScript | Dart |
|---|---|---|
| Engine | lean Hermes, linked and dead-stripped, **+1,810,256 B** (1027 §5, macOS arm64, everything included) | the Dart VM's machine code in Flutter 3.44.2's iOS arm64 release engine, **≈1.30 MB** of `__text`, constant data excluded (below) |
| Fixed per program | — | the core-library AOT snapshot, **925,712 B** (320,802 B gzipped) for hello world, iOS arm64 |
| This module, native | **11,143 B** of bytecode (1027 §5) | **+37,984 B** of AOT code on iOS (963,696 B in all) |
| This module, web | 5,916 B of script (1,888 B gzipped) | 62,616 B from dart2js `-O2` (20,955 B gzipped; 60,222 B at `-O4`), 113,489 B once compiled to Hermes bytecode |
| | | 59,129 B from dart2wasm `-O2` (24,264 B gzipped) + 11,419 B loader |
| Bytecode for an interpreter | (the 11,143 B above) | 8,553 B of Dart bytecode (5,092 B gzipped) — no stock runtime loads it (F4) |

So Dart starts at about **2.2 MB** on the phone — 1.30 MB of VM code plus
the 0.93 MB snapshot, with the VM's constant data still uncounted —
against 1.81 MB for Hermes all-in, and before anything that would let it
update (F4). For desktop scale, the standalone `dartaotruntime` (with
`dart:io` and BoringSSL) is 4,760,064 B, and `dart compile exe` of the
benchmark 5,814,656 B.

The engine's 6.97 MB of `__text`, attributed over 26,610 symbols from its
dSYM:

| Component | `__text` | Share |
|---|---|---|
| Skia | 1.36 MB | 19.5% |
| **Dart VM** (`dart::`, the `Dart_` API, runtime entries, double-conversion) | **1.30 MB** | 18.6% |
| Flutter engine | 1.02 MB | 14.6% |
| Impeller | 0.91 MB | 13.1% |
| Unattributed C and Swift symbols | 0.63 MB | 9.1% |
| BoringSSL (named symbols; more sits in the unattributed row) | 0.44 MB | 6.3% |
| HarfBuzz | 0.31 MB | 4.5% |
| ICU | 0.30 MB | 4.3% |
| `dart:io` embedder (`dart::bin`) | 0.23 MB | 3.3% |
| libc++ templates | 0.20 MB | 2.8% |
| tonic (Flutter's Dart bindings) | 0.13 MB | 1.9% |
| Image codecs, zlib, other | 0.14 MB | 2.0% |

A Rust host embedding Dart would need neither Skia nor Impeller, could
leave out `dart:io` and BoringSSL (the host does I/O, LLP 1016), and
would need the VM row plus whatever part of the unattributed row is VM.
1.30 MB is a floor for the VM's code, not an estimate of a finished
embedding.

There is no such embedding to link. rmacnak-google, on dart-lang/sdk#63611
(2026-06-17): *"We don't distribute a libdart.so. Every embedder has to
build the VM from source, so there's nothing to be ABI compatible with.
The embedding API is also a terrible mess we wouldn't want to freeze."*
dcharkes replied (2026-06-24): *"That's currently true, but we'd like to
be able to change that."* `exact-js` links a vanilla lean VM through a
336-line C++ shim (`js/src/shim.cc`); a Dart executor would begin with a
VM build per platform.

Confidence: high on the measured sizes; medium on the attribution (symbol
gaps, classification by name, code only).

### F4 — Over the air on iOS: each Dart route is closed, experimental, or slower than what exists

**AOT snapshots are machine code.** iOS runs no downloaded machine code
(1028 F5; App Review Guideline 2.5.2 as 1029.000 §2 cites it). On macOS
and Linux a new snapshot could be swapped the way 1029.000 swaps a native
library; on Android, Google Play's policy against downloaded native code
applies (1029.000 §2). On iOS the route TypeScript takes — an interpreter
— is the only one open. Dart has three candidates for it, and a fourth by
way of JavaScript.

**Dart's own bytecode interpreter, "Dart Dynamic Modules."** It exists:
`runtime/vm/interpreter.cc` is compiled under `#if
defined(DART_DYNAMIC_MODULES)` (dart-lang/sdk `main`), and the 3.12.2 SDK
on this Mac ships the compiler — `dart2bytecode` compiled the module to
8,553 B. The stock runtimes refuse it: `dartvm --interpreter module.dbc`
prints `Dart_LoadScriptFromBytecode: Cannot load bytecode as dynamic
modules are disabled.`, and `dartaotruntime` contains the string `Loading
of dynamic modules is not supported.` The README (`pkg/dynamic_modules`,
read 2026-09-14):

- *"Dart Dynamic Modules is a work-in-progress and experimental. Like any
  experimental Dart feature, that means it might change in breaking ways
  and we are not yet committed to adopting it."*
- *"the feature is also not available by default in Dart and Flutter
  SDKs, in part because just enabling it introduces code-size bloat and
  performance effects that are unnecessary for existing applications
  today."*
- Asked whether it is code push: *"No. Dart Dynamic Modules is a language
  runtime technology, similar to that available in many other
  languages."*
- *"Dynamic modules can only define new code. If they define libraries
  that already exist in the host application or in previous dynamic
  modules, the module is going to be rejected."*
- *"Dynamic modules are allowed to load only if the same application
  code, dynamic-interface, and version of the SDK tools were used to build
  both the host app and the dynamic module."*
- Its priorities: *"collaborative sharing of prototypes, and mobile
  development without JITing."*

Flutter's CI builds experimental engines with it, including
`ci/ios_release_ddm` (`engine/src/flutter/ci/builders/mac_ios_engine_ddm.json`,
flutter `master`). No published speed or size numbers were found. Part of
the shape fits exact2: an update applied at the next launch is simply a
different module against the same host, and "the same application code
… and version of the SDK tools" is what the compatibility id already pins
for Hermes bytecode (1027 D7, LLP 1030). Part does not: replacing a module
inside a running process — the development reload, and activation within
a session — collides with "already exist … in previous dynamic modules"
unless the VM's isolate is recreated (not investigated). And the maturity
and the unmeasured cost sit on top of F3's 2.2 MB.

**Shorebird.** Flutter-only: it replaces Flutter's engine. Changed
functions run in its interpreter while unchanged code stays compiled
(1028 F6). From "How we built code push" (2024-05-17): *"our current
(unoptimized) interpreter is about 100x slower than executing Dart AOT
code on a CPU."* There is no route for a host that is not Flutter.

**dart2wasm in exact2's wasm engines.** wasmi — LLP 1029.000's iOS
interpreter — lists `gc`, `exception-handling` and `function-references`
as planned, not supported (README). wasmtime 47.0.0 enables GC and
exception handling by default (RELEASES), and its Pulley interpreter runs
both; cfallin, on wasmtime#12251 (2026-01-07): *"Yes, Pulley supports GC
and exception handling! And, as far as I know, it should work on iOS:
execution with Pulley doesn't generate or jump to any native code."*
(Pulley is Tier 2 and `aarch64-apple-ios` Tier 3.) But wasmtime 47's
translator refuses what Dart emits — `"legacy exception handling proposal
is not supported"` (`code_translator.rs:643`) — and Dart 3.13.0's
`code_generator.dart` emits only the legacy form: eight `catch_legacy`
and `rethrow_` sites, no `try_table` or `throw_ref`. The switch is agreed
for about November: mkustermann on dart-lang/sdk#54394 (2026-08-27),
quoting *"My suggestion would be to switch entirely to new exception
instructions in the stable release coming out roughly November this
year"*, wrote *"We reached agreement on this."*

Loading is the first gap, not the last. In its default mode the module is
written against a JavaScript host. Its imports, measured (`imports.mjs`):
26 functions from the generated `dart2wasm` object (`JSON.stringify`,
`Number.prototype.toString`, `BigInt.prototype.toString`, `new
Error().stack`, `String(o)`, `===`, …), 8 `wasm:js-string` builtins,
`Math.sin`, `Math.cos` and `Math.asin`, 177 string constants as
`externref` globals, and `WebAssembly.JSTag`. The experimental
`--standalone` mode trades JavaScript for 82 Dart-specific host imports
(the `@pragma('wasm:import'` sites in 3.13.0's
`sdk/lib/_internal/wasm/standalone/embedder.dart`) — timers, stack traces,
regular expressions, number formatting — which its document says *"are
not stable, and might change in future Dart versions."* So a Dart module
on the phone through wasm needs, at the least: Dart's exnref release;
wasmtime with a GC collector and Pulley in place of wasmi (LLP 1028 F4's
choice, reopened); and those imports implemented in Rust. Its speed under
Pulley is unmeasured.

**dart2js on Hermes.** Works today, with nothing added: the module ran on
the Hermes CLI with no preamble and answered 20 of 20, so 1027 D7's
`module` card would carry it unchanged. The price is F2's 6–17× and F3's
tenfold bytecode.

Confidence: high on every quoted fact (§4) and on the measured imports;
medium on how dynamic modules would fit exact2's generations (reasoning,
unbuilt).

### F5 — The web: TypeScript is the platform's language; Dart is two compile targets

exact2's web host runs the script `hermesc` compiled, in a private iframe
realm, after first paint, through one wasm import (1027 D6). Dart would
put one of two compilers' output in that place.

Per call through the seam, µs, one run each:

| Call | TypeScript, V8 | dart2js, V8 | dart2wasm, V8 | TypeScript, JSC | dart2js, JSC | dart2wasm, JSC |
|---|---|---|---|---|---|---|
| `defaultLocation` | 0.34 | 0.78 | 0.91 | 0.23 | 0.98 | 1.78 |
| `station` | 0.35 | 0.96 | 1.10 | 0.22 | 0.95 | 2.51 |
| `search` | 0.69 | 1.47 | 1.48 | 0.51 | 1.35 | 3.79 |
| `nearest` | 1.27 | 3.54 | 3.51 | 1.00 | 3.48 | 7.45 |
| `board` | 7.22 | 31.25 | 19.54 | 4.39 | 23.59 | 40.81 |

V8 is Node 24.18.0's; JSC is Bun 1.3.12's JavaScriptCore, which is not
Safari's build. dart2js takes 2.1–5.4× TypeScript's time on the same
engine; dart2wasm 2.2–3.1× on V8 and 7.5–11.4× on JavaScriptCore. Every
number is microseconds; speed is not Dart's problem on the web. Its
problems are:

- **Size.** 20,955 B gzipped from dart2js against TypeScript's 1,888 B for
  the same module; 24,264 B + 3,192 B from dart2wasm.
- **Numbers mean different things per target.** dart.dev: on the web
  *"integers lose precision past 53 bits"*, `x is int` is true for a
  double with no fractional part, bitwise operators truncate operands to
  32 bits, and `1.0.toString()` is `"1"` where native Dart gives `"1.0"`.
  The probe shows the last directly (F1), with dart2wasm printing as
  native does. A split of native AOT and web dart2js breaks LLP 1027.001
  §2 by construction — in the module's own strings and arithmetic, where
  the seam's marshaling cannot see it. Running dart2js on every host
  restores parity at F2's price.
- **Browsers.** dart2wasm needs WasmGC (webassembly.org: Chrome 119,
  Firefox 120, Safari 18.2) and emits legacy exceptions until the switch
  planned for November; exnref and JS string builtins together raise the
  floor to Chrome 137, Firefox 134 and Safari 26.2. Flutter's WebAssembly
  page names Chromium 119+, says Firefox and Safari are blocked by
  renderer bugs, and falls back to JavaScript when WasmGC is missing.
  (Plain dart2wasm, without Flutter's renderer, ran on JavaScriptCore
  here.)
- **The guards.** dart2js's runtime reaches for what LLP 1027.000
  refuses. Identity hash codes: `if(t==null){t=Math.random()*0x3fffffff|0`
  (the module's output, line 143). Asynchrony: the scheduler tries
  `self.scheduleImmediate`, then a `MutationObserver` over a `document`,
  then `setImmediate`, then a timer that throws *"`setTimeout()` not
  found."* without one (`async_probe.js`, lines 1040–1047 and 1504–1505).
  The native prelude refuses timers and ambient randomness, so a Dart
  module would need a scheduler shim and an exemption for the runtime's
  own hashing — a narrower determinism guarantee than TypeScript's.

Confidence: high on the measured rows and the quoted facts; medium on the
browser floors after November, which are planned rather than shipped.

### F6 — The edit loop: a Dart compile is one to two orders of magnitude slower than `hermesc`

Warm compile times on this Mac, under load, single runs: `dart compile
kernel` 0.27 s; `js -O2` 0.32–0.57 s; `wasm -O2` 0.49–0.91 s;
`aot-snapshot` 0.70–0.96 s; `exe` 0.74–1.07 s; `dart2bytecode` about
50 ms. TypeScript: `hermesc` takes 7 ms for the module (1027 §5), and a
Fieldnotes logic edit is 32 ms from save to DOM (1027 D5). Dart's
celebrated hot reload is its JIT keeping the heap across an edit; exact2
migrates no executor-private heap (`rules/DEFERRED.md` §Surfaces,
1029.000 §1) and restarts with carry, so it would use Dart through one of
the compilers above, not through hot reload.

Confidence: medium — single warm runs, not distributions.

### F7 — Agents: the evidence of volume is TypeScript's; nothing ranks the two

- **Volume.** GitHub Octoverse 2025: *"August 2025 marks the first time
  TypeScript emerged as the most used language on GitHub, surpassing
  Python by ~42k contributors"* (2,636,006 monthly contributors); the
  report does not mention Dart. Stack Overflow Developer Survey 2025, all
  respondents: TypeScript 43.6%, Dart 5.9% (professional developers:
  48.8% and 6.1%).
- **Benchmarks.** None was found that compares current frontier agents on
  the two. AutoCodeBench (arXiv 2508.09101, 2025) includes both and
  contradicts itself: on its full set Dart scores higher (Claude Opus 4:
  Dart 54.0, TypeScript 47.2; o3-high: 55.0 and 47.2), on its Lite subset
  TypeScript does (Opus 4: 59.6 against 75.8; o3-high: 67.0 against
  77.4) — and both languages' problems were machine-translated (Java to
  Dart, JavaScript to TypeScript, its §2.2.5).
- **What Dart's own team says.** dart-lang/ai's skills README: *"your AI
  coding assistant has no idea how to use it properly. It guesses APIs,
  invents patterns, and hallucinates methods that don't exist."* The
  language is also moving beneath the models: primary constructors
  arrived in Dart 3.13 (August 2026).
- **TypeScript's own failure mode.** arXiv 2602.17955 (2026): *"AI agents
  are 9x more prone to use the 'any' keyword"* than humans writing
  TypeScript. Dart's sound, runtime-checked types would close that door
  inside a module; the seam's shape check (1027 D2) already refuses a
  wrong-shaped answer at the boundary, in either language.
- **This port, as one data point.** Clean under `dart analyze` on the
  first draft and 20 of 20 on the first run — and then F1's four points
  and F2's sixfold cliff, none of which the analyzer reports.

Confidence: high on the quoted facts; low on any ranking of agent
quality, which no source here supports.

### F8 — What Dart would cost exact2, item by item

| What TypeScript has | Where | What Dart would need |
|---|---|---|
| An executor over a vanilla lean VM and a 336-line C++ shim | 1027 D3 | A VM built from source per platform (F3); the embedding lifecycle; a product AOT runtime and core snapshot per target — or the dynamic-modules build, for updates (F4) |
| `await` over the host's tickets, with a microtask drain | 1027 D1a | The embedder driving Dart's message loop and microtasks, completing `Future`s from host tickets |
| `fetch`, `store`, `storage` (files, SQLite), URL, text and base64, bound to Ibex | 1027 D1a/D10, 1027.001 D1 | Each rebound through FFI or VM natives; `dart:io` and `package:http` kept out, since they bypass the grant path (LLP 1016) |
| Ambient time and randomness refused | 1027.000 | The same for `DateTime.now()`, `Stopwatch` and unseeded `Random()`, which in the VM are natives (`DateTime_currentTimeMicros` in `vm_shared/lib/date_patch.dart`) rather than globals a prelude can shadow; how an embedder would intercept them is not investigated |
| Constants evaluated through the engine at bake; types generated from the plan; `tsc --strict` | 1027 D5 | A Dart VM in the bake; generated Dart types; `dart analyze` |
| The browser running the bake's script | 1027 D6 | A dart2js or dart2wasm loader, with F5's shims |
| A `module` card of bytecode and its version | 1027 D7 | AOT snapshots per target, never to iOS; or dynamic-module bytecode pinned to the SDK build |
| Two languages at parity | 1027.001 | Three |
| "Logic below the data seam is TypeScript by default or Rust" | `rules/DEFERRED.md` §Authoring models | An amended sentence, and a take (§Moving something off this list) |

Confidence: medium — design reasoning over the corpus; nothing built.

## 4. Confidence, gathered

| Claim | Basis |
|---|---|
| 20/20 answers on every target; how whole doubles print (F1) | measured; reproducible from §6 |
| `List.sort` is not stable (F1) | `dart:core` doc comment, 3.12.2 `lib/core/list.dart:485` |
| Speeds and ratios (F2, F5) | measured on this Mac under load; minimum over alternated rounds for Hermes and AOT, single runs for the JIT engines |
| Module and snapshot sizes (F3) | measured |
| The Dart VM's 1.30 MB (F3) | attributed from Flutter 3.44.2's iOS release dSYM; code only; medium |
| No distributed `libdart`; embedders build the VM from source (F3) | dart-lang/sdk#63611 comments, read through `gh` 2026-09-14 |
| Dynamic modules exist, are experimental, off by default, add-only, pinned to the build (F4) | the `pkg/dynamic_modules` README (`main`, read 2026-09-14); the `interpreter.cc` guard; this SDK's `dart2bytecode` and the refusal the stock VM printed; flutter's `mac_ios_engine_ddm.json` |
| Shorebird's interpreter at about 100× AOT (F4) | shorebird.dev, "How we built code push", 2024-05-17 |
| wasmi has neither GC nor exception handling (F4) | the wasmi README, read 2026-09-14 |
| wasmtime 47: GC and exceptions on by default, Pulley supports both, legacy exceptions refused (F4) | wasmtime `release-47.0.0` RELEASES.md, `code_translator.rs`, `docs/stability-*.md`; wasmtime#12251 |
| dart2wasm 3.13 emits only legacy exceptions; the switch agreed for about November (F4) | dart-lang/sdk `3.13.0` `pkg/dart2wasm/lib/code_generator.dart`; #54394, comment of 2026-08-27 |
| dart2wasm's imports (F4) | measured on the module; the standalone count from `3.13.0`'s `standalone/embedder.dart` |
| Web number semantics (F5) | dart.dev's number representation page; observed in F1 |
| Browser floors (F5) | webassembly.org `features.json`, read 2026-09-14 |
| Flutter's wasm browser support (F5) | docs.flutter.dev, "Support for WebAssembly", read 2026-09-14 |
| dart2js reads `Math.random` and scheduler globals (F5) | the compiled output, lines cited |
| Agent evidence (F7) | Octoverse 2025; Stack Overflow 2025; AutoCodeBench's HTML tables and §2.2.5; the abstract of arXiv 2602.17955; dart-lang/ai's skills README; the Dart 3.13.0 CHANGELOG |
| The integration bill (F8) | reasoning over 1027, 1027.000, 1027.001 and 1029.000; the `DateTime` native from the SDK's `date_patch.dart`; unbuilt |
| Anything on a phone | not measured |

Gathered by the research passes but **not re-checked here, and therefore
not used above**: McEval's per-language scores, The Stack's per-language
sizes, npm and pub.dev package counts, Shorebird's engine size and link
percentages, the GC status of WAMR, wasm3 and WasmKit, Flutter's iOS 26
debug-JIT changes, and benchmark-suite ratios of Dart AOT to Rust.

## 5. What this feeds, and what would reopen it

It feeds nothing decided. LLP 1027's ruling stands on its own reasons;
this is the record that Dart was measured against them.

For the record, what Dart would bring: compiled computation 1.6–3.9×
faster than Hermes without a JIT (F2); sound null safety and
runtime-checked types; records, patterns and exhaustive switches; isolates
as shared-nothing threads, which LLP 1027.002 would otherwise have to
build; and a VM whose iOS release path Flutter exercises every day.

What would reopen it, each with the measurement it should bring:

1. **Dart Dynamic Modules in stock AOT runtimes** — a Flutter release
   engine without a custom build — with published size and speed. Then:
   that engine's size over F3's, and this module interpreted on F2's
   harness against Hermes, on a phone.
2. **dart2wasm emitting exnref** (planned for about November 2026) **with
   a stable standalone import set.** Then: the module under the wasmtime
   runtime with Pulley and a collector, on a phone (the rig of 1032 F7),
   its imports implemented in Rust, against Hermes.
3. **wasmi gaining GC and exception handling**, which would let
   1029.000's iOS interpreter run the same module.
4. **A product reason to meet Flutter developers in their language.**
   dart2js on Hermes is the experiment that needs no runtime change (F4),
   at F2's and F3's factors.

Not reasons on their own: AOT speed, since Rust holds the hot slot (1027
D8, 1029.000); and Dart's hot reload, since exact2 restarts with carry
(F6).

## 6. Appendix — the probe, verbatim

A scratch directory outside the repo (the session's scratchpad,
`probe-dart/`). `build.sh` produces every artifact, the sizes, the
attribution and the comparison; `rounds.sh` produces the timings; the
tools and versions are §2's. `module.dart`, `hello.dart` and
`async_probe.dart` are written by `build.sh`.

### `build.sh`

```sh
#!/bin/bash
# LLP 1040's probe: every artifact, its size, and the 20-case comparison.
# Run from this directory. The timings are rounds.sh; the first run of a
# freshly linked AOT binary may wait in dyld while macOS evaluates it — run
# it once and let it start before timing.
set -e
EXACT=~/projects/exact2
H=~/projects/ibex/tools/hermes-vanilla
FLUTTER=/opt/homebrew/share/flutter/bin/cache
S=$FLUTTER/dart-sdk
E=$FLUTTER/artifacts/engine
CASES=$(cat cases.txt)

# module.dart: the module alone, every source reachable, no benchmark.
cat > module.dart <<DART
// The module alone: every source reachable, no benchmark.
import 'caltrain.dart';

const casesText = r'''
$CASES
''';

void main() {
  for (final line in casesText.trim().split('\n')) {
    final tab = line.indexOf('\t');
    print(answer(line.substring(0, tab), line.substring(tab + 1)));
  }
}
DART
echo "void main() { print('hi'); }" > hello.dart
dart analyze caltrain.dart bench.dart module.dart board_probe.dart

# TypeScript twin -> one script -> Hermes bytecode (bundle includes the harness).
bun build $EXACT/js/tests/fixtures/caltrain.ts --format=iife --outfile ts_twin.js
cat prelude.js ts_twin.js harness.js > ts_bundle.js
cat prelude.js ts_twin.js harness.js > ts_node.js
$H/hermesc-macos-arm64 -O -emit-binary -out ts_bundle.hbc ts_bundle.js

# Dart, every target.
dart compile exe bench.dart -o bench_aot
dart compile exe board_probe.dart -o board_probe
dart compile aot-snapshot module.dart -o module.aot
dart compile aot-snapshot hello.dart -o hello.aot
dart compile kernel module.dart -o module.dill
for prog in module hello bench; do dart compile js -O2 $prog.dart -o ${prog}_js.js; done
dart compile js -O4 module.dart -o module_js_o4.js
for f in module_js hello_js bench_js module_js_o4; do $H/hermesc-macos-arm64 -O -emit-binary -out $f.hbc $f.js; done
for prog in module hello bench; do dart compile wasm -O2 $prog.dart -o ${prog}_wasm.wasm; done
for prog in hello module; do
  $S/bin/dartaotruntime $S/bin/snapshots/dart2bytecode.dart.snapshot \
    --platform $S/lib/_internal/vm_platform.dill -o $prog.dbc $prog.dart
done
cat > async_probe.dart <<'DART'
Future<int> twice(int x) async { await Future<void>.value(); return x * 2; }
void main() async { print('async ${await twice(21)}'); }
DART
dart compile js -O2 async_probe.dart -o async_probe.js

# iOS arm64: Flutter's frontend server (AOT, TFA, product) -> gen_snapshot's Mach-O dylib.
mkdir -p ios
for prog in hello module; do
  $S/bin/dartaotruntime $E/darwin-x64/frontend_server_aot.dart.snapshot \
    --sdk-root $E/common/flutter_patched_sdk_product/ --target=flutter --aot --tfa \
    --no-print-incremental-dependencies -Ddart.vm.product=true -Ddart.vm.profile=false \
    --output-dill ios/$prog.dill $prog.dart
  $E/ios-release/gen_snapshot_arm64 --deterministic --snapshot_kind=app-aot-macho-dylib \
    --macho=ios/$prog.App --strip ios/$prog.dill
done

# The Dart VM's share of Flutter's iOS release engine, from its dSYM.
D=$E/ios-release/Flutter.xcframework/ios-arm64
otool -l $D/Flutter.framework/Flutter | grep -A3 'sectname __text'
xcrun nm -n --defined-only --demangle $D/dSYMs/Flutter.framework.dSYM/Contents/Resources/DWARF/Flutter > flutter_ios_syms.txt
python3 attribute.py

# Sizes.
for f in ts_twin.js ts_bundle.hbc hello.aot module.aot hello_js.js module_js.js module_js_o4.js \
         module_js.hbc hello_wasm.wasm module_wasm.wasm module_wasm.mjs hello.dbc module.dbc \
         ios/hello.App ios/module.App; do
  printf '%-18s %9d B  gzip -9 %8d B\n' $f $(stat -f %z $f) $(gzip -9c $f | wc -c)
done
ls -l $S/bin/dartaotruntime bench_aot | awk '{print $5, $9}'

# What the wasm module imports, and whether the stock VM loads bytecode.
node imports.mjs module_wasm.wasm
$S/bin/dartvm --interpreter module.dbc || true

# The 20 cases on every target, held to the TypeScript twin on Hermes.
$H/hermes ts_bundle.hbc | grep -E '^(case|log)' > cases_ts_hermes.txt
dart run module.dart > cases_dart_jit.txt
$H/hermes module_js.hbc > cases_dart2js_hermes.txt
node run_wasm.mjs module_wasm > cases_dart2wasm_node.txt 2>/dev/null
node compare.mjs cases_ts_hermes.txt '[["dart-jit (native VM)","cases_dart_jit.txt",false],["dart2js on Hermes","cases_dart2js_hermes.txt",false],["dart2wasm on V8","cases_dart2wasm_node.txt",false]]'
```

### `rounds.sh`

```sh
#!/bin/sh
# LLP 1040's timings. Alternates the engines so load changes hit each alike;
# waits out the first-exec evaluation of a fresh AOT binary first.
H=~/projects/ibex/tools/hermes-vanilla
while pgrep -f '^\./bench_aot' > /dev/null; do sleep 2; done
for r in 1 2 3; do
  echo "== round $r load $(sysctl -n vm.loadavg)"
  echo "-- ts-hermes";      $H/hermes ts_bundle.hbc | grep bench
  echo "-- dart-aot";       ./bench_aot | grep bench
  echo "-- dart2js-hermes"; $H/hermes bench_js.hbc | grep bench
done
echo "== references (JIT)"
echo "-- dart-jit";           dart run bench.dart | grep bench
echo "-- dart2wasm-node";     node run_wasm.mjs bench_wasm 2>&1 | grep -E '^(bench|compile)'
echo "-- ts-node";            node ts_node.js | grep bench
echo "-- dart2js-node";       node bench_js.js | grep bench
echo "-- dart2js-bun(JSC)";   bun bench_js.js | grep bench
echo "-- ts-bun(JSC)";        bun ts_node.js | grep bench
echo "-- dart2wasm-bun(JSC)"; bun run_wasm.mjs bench_wasm 2>&1 | grep -E '^(bench|compile)'
```

### `cases.txt`

```text
defaultLocation	[]
stations	[{"lat":37.3947,"lon":-122.0763}]
nearest	[{"lat":37.3947,"lon":-122.0763},3]
nearest	[{"lat":37.7,"lon":-122.4},9]
station	["paloalto",{"lat":37.3947,"lon":-122.0763}]
board	["mv","north",1787918400000]
board	["mv","south",1787900400000]
board	["sf","north",1787918400000]
board	["sj","north",1787875200000]
search	["Palo",{"lat":37.3947,"lon":-122.0763}]
search	["san",{"lat":37.3947,"lon":-122.0763}]
search	["",{"lat":37.3947,"lon":-122.0763}]
nearest	[{"lat":37.3947,"lon":-122.0763},10]
nearest	[{"lat":37.3947,"lon":-122.0763},1.5]
stations	[]
stations	[{"lat":91,"lon":0}]
station	["nowhere",{"lat":37.3947,"lon":-122.0763}]
board	["mv","east",1787918400000]
board	[7,"north",1787918400000]
bogus	[]
```

### `caltrain.dart`

```dart
// Caltrain's data source in Dart: a port of exact2's executor fixture
// js/tests/fixtures/caltrain.ts, source by source and message by message.
// The seam is the same shape exact-js calls: answer(source, argsJson) with
// JSON both ways; a refusal is {"error": {kind, message}}.

import 'dart:convert';
import 'dart:math' as math;

typedef Location = ({double lat, double lon});

class DataError implements Exception {
  DataError(this.kind, this.message);
  final String kind;
  final String message;
}

class Row {
  const Row(this.id, this.name, this.zone, this.lat, this.lon, this.offsetMin);
  final String id;
  final String name;
  final int zone;
  final double lat;
  final double lon;
  final int offsetMin;
}

const stations = <Row>[
  Row('sf', 'San Francisco', 1, 37.7765, -122.3947, 0),
  Row('22nd', '22nd Street', 1, 37.7573, -122.392, 5),
  Row('millbrae', 'Millbrae', 2, 37.6003, -122.3868, 20),
  Row('sanmateo', 'San Mateo', 2, 37.568, -122.324, 27),
  Row('redwood', 'Redwood City', 3, 37.4855, -122.231, 38),
  Row('paloalto', 'Palo Alto', 3, 37.4436, -122.1647, 45),
  Row('mv', 'Mountain View', 4, 37.3947, -122.0763, 52),
  Row('sunnyvale', 'Sunnyvale', 4, 37.3784, -122.0312, 57),
  Row('sj', 'San Jose Diridon', 4, 37.3297, -121.9023, 70),
];

const dayStartMs = 1787875200000;
const Location defaultLocation = (lat: 37.3947, lon: -122.0763);
const services = <(String, int, double)>[
  ('Local', 5, 1.0),
  ('Limited', 25, 0.8),
  ('Express', 45, 0.6),
];

final logs = <String>[];

Row? station(String id) {
  for (final s in stations) {
    if (s.id == id) return s;
  }
  return null;
}

/// Great-circle distance in meters (haversine).
double distanceM(Location a, double bLat, double bLon) {
  const r = 6371000;
  final lat1 = a.lat * (math.pi / 180);
  final lon1 = a.lon * (math.pi / 180);
  final lat2 = bLat * (math.pi / 180);
  final lon2 = bLon * (math.pi / 180);
  final dlat = lat2 - lat1;
  final dlon = lon2 - lon1;
  final s1 = math.sin(dlat / 2);
  final s2 = math.sin(dlon / 2);
  final h = s1 * s1 + math.cos(lat1) * math.cos(lat2) * s2 * s2;
  return 2 * r * math.asin(math.sqrt(h));
}

Map<String, Object> stationValue(Row s, Location location) => {
  'id': s.id,
  'name': s.name,
  'zone': s.zone,
  'distance': distanceM(location, s.lat, s.lon).round(),
};

Map<String, Object> locationValue(Location l) => {'lat': l.lat, 'lon': l.lon};

// JavaScript's and Rust's sorts are stable; Dart's List.sort is not
// guaranteed to be, so ties fall back to the original index.
int _stable(double d, int i, int j) => d < 0 ? -1 : (d > 0 ? 1 : i - j);

// A class, not a record: `List<(double, int, Map)>` plus a sort measured
// ~6x slower in AOT (121 us vs 21 us for `board`'s departures).
class _Keyed<T> {
  _Keyed(this.key, this.index, this.value);
  final double key;
  final int index;
  final T value;
}

int _byKey(_Keyed a, _Keyed b) => _stable(a.key - b.key, a.index, b.index);

List<Map<String, Object>> departures(Row s, String direction) {
  final north = direction == 'north';
  final terminus = north ? stations.first : stations.last;
  if (terminus.id == s.id) return [];
  final sign = north ? -1 : 1;
  final out = <_Keyed<Map<String, Object>>>[];
  for (var si = 0; si < services.length; si++) {
    final (service, every, speed) = services[si];
    var minute = 5 * 60 + si * 7;
    var n = 0;
    while (minute < 23 * 60) {
      final at = minute + sign * s.offsetMin * speed;
      if (at >= 0 && at < 24 * 60) {
        final train = 100 + si * 100 + n * 2 + (north ? 1 : 0);
        final headsign = north ? 'San Francisco' : 'San Jose';
        out.add(_Keyed(
          at,
          out.length,
          {
            'id': '${s.id}-$direction-$train',
            'train': train,
            'service': service,
            'headsign': headsign,
            'at': dayStartMs + at * 60000,
          },
        ));
      }
      minute += every * 6;
      n += 1;
    }
  }
  out.sort(_byKey);
  return [for (final d in out) d.value];
}

// --- argument checks, the crate's, message for message ---------------------

void arity(List<Object?> args, int expected) {
  if (args.length != expected) {
    throw DataError('BadArguments', 'expected $expected arguments, got ${args.length}');
  }
}

Location loc(List<Object?> args, int i) {
  final v = i < args.length ? args[i] : null;
  if (v is Map && v.length == 2) {
    final lat = v['lat'], lon = v['lon'];
    if (lat is num && lon is num && lat.isFinite && lon.isFinite &&
        lat >= -90 && lat <= 90 && lon >= -180 && lon <= 180) {
      return (lat: lat.toDouble(), lon: lon.toDouble());
    }
  }
  throw DataError('BadArguments', 'location');
}

String text(List<Object?> args, int i) {
  final v = i < args.length ? args[i] : null;
  if (v is String) return v;
  throw DataError('BadArguments', 'argument $i');
}

num finiteNumber(List<Object?> args, int i) {
  final v = i < args.length ? args[i] : null;
  if (v is num && v.isFinite) return v;
  throw DataError('BadArguments', 'argument $i');
}

// --- the sources -------------------------------------------------------------

Object? query(String source, List<Object?> args) {
  switch (source) {
    case 'defaultLocation':
      arity(args, 0);
      logs.add('defaultLocation ${jsonEncode(locationValue(defaultLocation))}');
      return locationValue(defaultLocation);
    case 'stations':
      arity(args, 1);
      final location = loc(args, 0);
      return [for (final s in stations) stationValue(s, location)];
    case 'nearest':
      arity(args, 2);
      final location = loc(args, 0);
      final count = finiteNumber(args, 1);
      if (count % 1 != 0 || count < 0 || count > stations.length) {
        throw DataError('BadArguments', 'count');
      }
      final all = [for (var i = 0; i < stations.length; i++) _Keyed(0.0, i, stations[i])];
      all.sort((a, b) => _stable(
          distanceM(location, a.value.lat, a.value.lon) - distanceM(location, b.value.lat, b.value.lon),
          a.index,
          b.index));
      return [for (final k in all.take(count.toInt())) stationValue(k.value, location)];
    case 'station':
      arity(args, 2);
      final id = text(args, 0);
      final location = loc(args, 1);
      final s = station(id);
      if (s == null) throw DataError('Unavailable', 'station $id');
      return stationValue(s, location);
    case 'board':
      arity(args, 3);
      final id = text(args, 0);
      final direction = text(args, 1);
      final nowMs = finiteNumber(args, 2);
      if (direction != 'north' && direction != 'south') {
        throw DataError('BadArguments', 'direction');
      }
      final s = station(id);
      if (s == null) throw DataError('Unavailable', 'station $id');
      return [for (final d in departures(s, direction)) if ((d['at'] as num) >= nowMs) d];
    case 'search':
      arity(args, 2);
      final q = text(args, 0).toLowerCase();
      final location = loc(args, 1);
      return [
        for (final s in stations)
          if (s.name.toLowerCase().contains(q)) stationValue(s, location)
      ];
    default:
      throw DataError('UnknownSource', source);
  }
}

// --- the seam ------------------------------------------------------------------

const appId = 'com.exact.caltrain';
const grants = '';

String answer(String source, String argsJson) {
  final args = jsonDecode(argsJson) as List<Object?>;
  try {
    return jsonEncode({'value': query(source, args)});
  } on DataError catch (e) {
    return jsonEncode({
      'error': {'kind': e.kind, 'message': e.message}
    });
  }
}
```

### `bench.dart`

```dart
// Drives caltrain.dart through the seam: the 20 cases, then per-call timings
// (JSON both ways, and compute only), the same algorithm as harness.js.
import 'dart:convert';
import 'caltrain.dart';

const casesText = r'''
defaultLocation	[]
stations	[{"lat":37.3947,"lon":-122.0763}]
nearest	[{"lat":37.3947,"lon":-122.0763},3]
nearest	[{"lat":37.7,"lon":-122.4},9]
station	["paloalto",{"lat":37.3947,"lon":-122.0763}]
board	["mv","north",1787918400000]
board	["mv","south",1787900400000]
board	["sf","north",1787918400000]
board	["sj","north",1787875200000]
search	["Palo",{"lat":37.3947,"lon":-122.0763}]
search	["san",{"lat":37.3947,"lon":-122.0763}]
search	["",{"lat":37.3947,"lon":-122.0763}]
nearest	[{"lat":37.3947,"lon":-122.0763},10]
nearest	[{"lat":37.3947,"lon":-122.0763},1.5]
stations	[]
stations	[{"lat":91,"lon":0}]
station	["nowhere",{"lat":37.3947,"lon":-122.0763}]
board	["mv","east",1787918400000]
board	[7,"north",1787918400000]
bogus	[]
''';

const benchCalls = [
  ['defaultLocation', '[]'],
  ['station', '["paloalto",{"lat":37.3947,"lon":-122.0763}]'],
  ['search', '["Palo",{"lat":37.3947,"lon":-122.0763}]'],
  ['nearest', '[{"lat":37.3947,"lon":-122.0763},3]'],
  ['board', '["mv","north",1787918400000]'],
];

(double, double) measure(void Function() call) {
  final sw = Stopwatch()..start();
  while (sw.elapsedMilliseconds < 300) {
    call();
    logs.clear();
  }
  final samples = <double>[];
  for (var k = 0; k < 41; k++) {
    var count = 0;
    sw..reset()..start();
    do {
      for (var i = 0; i < 16; i++) call();
      count += 16;
    } while (sw.elapsedMicroseconds < 20000);
    samples.add(sw.elapsedMicroseconds / count);
    logs.clear();
  }
  samples.sort();
  return (samples[0], samples[20]);
}

void main() {
  final lines = casesText.trim().split('\n');
  for (var i = 0; i < lines.length; i++) {
    final tab = lines[i].indexOf('\t');
    print('case $i ${answer(lines[i].substring(0, tab), lines[i].substring(tab + 1))}');
  }
  print('log ${logs.first}');
  logs.clear();
  for (final c in benchCalls) {
    final seam = measure(() => answer(c[0], c[1]));
    final args = jsonDecode(c[1]) as List<Object?>;
    final compute = measure(() => query(c[0], args));
    print('bench ${c[0]} seam_min=${seam.$1.toStringAsFixed(3)} seam_p50=${seam.$2.toStringAsFixed(3)} '
        'compute_min=${compute.$1.toStringAsFixed(3)} compute_p50=${compute.$2.toStringAsFixed(3)}');
  }
}
```

### `board_probe.dart`

```dart
import 'caltrain.dart';

class _Dep {
  _Dep(this.at, this.i, this.v);
  final double at;
  final int i;
  final Map<String, Object> v;
}

List<Map<String, Object>> departuresClass(Row s, String direction, {bool sort = true}) {
  final north = direction == 'north';
  final terminus = north ? stations.first : stations.last;
  if (terminus.id == s.id) return [];
  final sign = north ? -1 : 1;
  final out = <_Dep>[];
  for (var si = 0; si < services.length; si++) {
    final (service, every, speed) = services[si];
    var minute = 5 * 60 + si * 7;
    var n = 0;
    while (minute < 23 * 60) {
      final at = minute + sign * s.offsetMin * speed;
      if (at >= 0 && at < 24 * 60) {
        final train = 100 + si * 100 + n * 2 + (north ? 1 : 0);
        final headsign = north ? 'San Francisco' : 'San Jose';
        out.add(_Dep(at, out.length, {
          'id': '${s.id}-$direction-$train',
          'train': train,
          'service': service,
          'headsign': headsign,
          'at': dayStartMs + at * 60000,
        }));
      }
      minute += every * 6;
      n += 1;
    }
  }
  if (sort) {
    out.sort((a, b) {
      final d = a.at - b.at;
      return d < 0 ? -1 : (d > 0 ? 1 : a.i - b.i);
    });
  }
  return [for (final d in out) d.v];
}

double perCall(void Function() f) {
  final sw = Stopwatch()..start();
  while (sw.elapsedMilliseconds < 300) f();
  final samples = <double>[];
  for (var k = 0; k < 21; k++) {
    var count = 0;
    sw..reset()..start();
    while (sw.elapsedMicroseconds < 25000) {
      for (var i = 0; i < 16; i++) f();
      count += 16;
    }
    samples.add(sw.elapsedMicroseconds / count);
  }
  samples.sort();
  return samples[10];
}

void main() {
  final mv = stations[6];
  print('records+sort  ${perCall(() => departures(mv, 'north')).toStringAsFixed(3)} us');
  print('class+sort    ${perCall(() => departuresClass(mv, 'north')).toStringAsFixed(3)} us');
  print('class nosort  ${perCall(() => departuresClass(mv, 'north', sort: false)).toStringAsFixed(3)} us');
}
```

### `prelude.js`

```js
var __print = typeof print === "function" ? print : console.log.bind(console);
var __logs = [];
globalThis.console = { log: function () { var a = []; for (var i = 0; i < arguments.length; i++) { var x = arguments[i]; a.push(typeof x === "string" ? x : JSON.stringify(x)); } __logs.push(a.join(" ")); } };
```

### `harness.js`

```js
var casesText = "defaultLocation\t[]\nstations\t[{\"lat\":37.3947,\"lon\":-122.0763}]\nnearest\t[{\"lat\":37.3947,\"lon\":-122.0763},3]\nnearest\t[{\"lat\":37.7,\"lon\":-122.4},9]\nstation\t[\"paloalto\",{\"lat\":37.3947,\"lon\":-122.0763}]\nboard\t[\"mv\",\"north\",1787918400000]\nboard\t[\"mv\",\"south\",1787900400000]\nboard\t[\"sf\",\"north\",1787918400000]\nboard\t[\"sj\",\"north\",1787875200000]\nsearch\t[\"Palo\",{\"lat\":37.3947,\"lon\":-122.0763}]\nsearch\t[\"san\",{\"lat\":37.3947,\"lon\":-122.0763}]\nsearch\t[\"\",{\"lat\":37.3947,\"lon\":-122.0763}]\nnearest\t[{\"lat\":37.3947,\"lon\":-122.0763},10]\nnearest\t[{\"lat\":37.3947,\"lon\":-122.0763},1.5]\nstations\t[]\nstations\t[{\"lat\":91,\"lon\":0}]\nstation\t[\"nowhere\",{\"lat\":37.3947,\"lon\":-122.0763}]\nboard\t[\"mv\",\"east\",1787918400000]\nboard\t[7,\"north\",1787918400000]\nbogus\t[]\n";
var benchCalls = [
  ["defaultLocation", "[]"],
  ["station", '["paloalto",{"lat":37.3947,"lon":-122.0763}]'],
  ["search", '["Palo",{"lat":37.3947,"lon":-122.0763}]'],
  ["nearest", '[{"lat":37.3947,"lon":-122.0763},3]'],
  ["board", '["mv","north",1787918400000]'],
];
var nowMs = typeof performance !== "undefined" ? function () { return performance.now(); } : function () { return Date.now(); };
function seam(source, argsJson) {
  var args = JSON.parse(argsJson);
  try { return JSON.stringify({ value: exact.answer(source, args) }); }
  catch (e) { if (e && e.kind) return JSON.stringify({ error: { kind: e.kind, message: e.message } }); throw e; }
}
function measure(call) {
  var t0 = nowMs();
  while (nowMs() - t0 < 300) { call(); __logs.length = 0; }
  var samples = [];
  for (var k = 0; k < 41; k++) {
    var count = 0, dt;
    t0 = nowMs();
    do { for (var i = 0; i < 16; i++) call(); count += 16; dt = nowMs() - t0; } while (dt < 20);
    samples.push(dt * 1000 / count);
    __logs.length = 0;
  }
  samples.sort(function (a, b) { return a - b; });
  return [samples[0], samples[20]];
}
var lines = casesText.trim().split("\n");
for (var i = 0; i < lines.length; i++) {
  var tab = lines[i].indexOf("\t");
  __print("case " + i + " " + seam(lines[i].slice(0, tab), lines[i].slice(tab + 1)));
}
__print("log " + __logs[0]);
__logs.length = 0;
benchCalls.forEach(function (c) {
  var s = measure(function () { return seam(c[0], c[1]); });
  var parsed = JSON.parse(c[1]);
  var q = measure(function () { return exact.answer(c[0], parsed); });
  __print("bench " + c[0] + " seam_min=" + s[0].toFixed(3) + " seam_p50=" + s[1].toFixed(3) + " compute_min=" + q[0].toFixed(3) + " compute_p50=" + q[1].toFixed(3));
});
```

### `run_wasm.mjs`

```js
import { readFileSync } from "node:fs";
const name = process.argv[2];
const loader = await import(`./${name}.mjs`);
const bytes = readFileSync(`./${name}.wasm`);
const t0 = performance.now();
const app = await loader.compile(bytes);
const t1 = performance.now();
const inst = await app.instantiate({});
const t2 = performance.now();
console.error(`compile ${(t1 - t0).toFixed(2)} ms, instantiate ${(t2 - t1).toFixed(2)} ms`);
inst.invokeMain();
```

### `imports.mjs`

```js
import { readFileSync } from "node:fs";
const bytes = readFileSync(process.argv[2]);
const m = await WebAssembly.compile(bytes, { builtins: ["js-string"] });
const imps = WebAssembly.Module.imports(m), exps = WebAssembly.Module.exports(m);
const byMod = {};
for (const i of imps) (byMod[i.module] ??= []).push(`${i.name}:${i.kind}`);
for (const [k, v] of Object.entries(byMod)) console.log(k, v.length, v.slice(0, 12).join(" "));
console.log("exports", exps.length);
```

### `compare.mjs`

```js
// Holds each engine's 20 answers to the TypeScript twin's on Hermes (the twin
// is held byte-identical to the Rust crate by js/tests/caltrain.rs). Numbers
// compare as parsed f64s (Dart native prints 1787918880000 as "…0000.0");
// the executor's marshaling parses JSON numbers the same way.
import { readFileSync } from "node:fs";
const parse = (f, prefix) => readFileSync(f, "utf8").split("\n")
  .filter((l) => prefix ? l.startsWith("case ") : l.startsWith("{"))
  .map((l) => JSON.parse(prefix ? l.slice(l.indexOf(" ", 5) + 1) : l));
const same = (a, b) => {
  if (typeof a === "number" || typeof b === "number") return Object.is(a, b);
  if (a === null || b === null || typeof a !== "object") return a === b;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a), kb = Object.keys(b);
  return ka.length === kb.length && ka.every((k, i) => k === kb[i] && same(a[k], b[k]));
};
const ref = parse(process.argv[2], true);
for (const [name, file, prefixed] of JSON.parse(process.argv[3])) {
  const got = parse(file, prefixed);
  const bad = ref.map((r, i) => same(r, got[i]) ? null : i).filter((i) => i !== null);
  const text = readFileSync(file, "utf8");
  console.log(`${name.padEnd(22)} ${got.length} answers, ${ref.length - bad.length}/${ref.length} equal${bad.length ? " — differ: " + bad.join(",") : ""}; "at" printed with .0: ${/"at":\d+\.0[,}]/.test(text)}`);
}
```

### `attribute.py`

```python
# Attributes the Flutter iOS release engine's __text to components by symbol
# (address gap to the next symbol), from its dSYM's demangled symbol table.
import re, sys, collections
START, SIZE = 0x4000, 0x6a5eb8
END = START + SIZE
rows = []
for line in open('flutter_ios_syms.txt'):
    parts = line.rstrip('\n').split(' ', 2)
    if len(parts) < 3: continue
    addr = int(parts[0], 16)
    if START <= addr < END: rows.append((addr, parts[2]))
rows.sort()
def cls(n):
    if 'dart::bin::' in n or 'dart::bin' in n: return 'dart:io embedder (dart::bin)'
    if 'dart::' in n or n.startswith(('_Dart_', 'Dart_')) or 'DRT_' in n or 'DLRT_' in n: return 'Dart VM (dart::)'
    if 'double_conversion' in n: return 'Dart VM (dart::)'
    if 'tonic::' in n: return 'tonic (Flutter<->Dart bindings)'
    if 'bssl::' in n or re.match(r'_(SSL|EVP|BN|bn|ERR|CRYPTO|X509|x509|ASN1|asn1|EC|ec|RSA|rsa|aes|AES|sha|SHA|OPENSSL|CBB|CBS|HMAC|PEM|PKCS|pkcs|RAND|DH|DSA|dsa|EVP|HKDF|X25519|x25519|ED25519|chacha|gcm|md5|MD5|poly1305|ChaCha|OBJ|BIO|bio|d2i|i2d|ECDSA|ecdsa|p256|fiat|CRYPTO|ec_|dh_|siphash|SPAKE|spake|kyber|KYBER|MLKEM|mlkem|MLDSA|mldsa|slhdsa|SLHDSA|TRUST|trust|v2i|i2v|NCONF|CONF|conf|GENERAL|DIST|AUTHORITY|ACCESS|POLICY|NAME|X509V3|x509v3|lh_|sk_|OPENSSL_|boringssl|bcm_|BCM_)', n): return 'BoringSSL'
    if 'icu_' in n or re.match(r'_(u|ubrk|ucase|uchar|ucol|uloc|ures|udata|umutex|uhash|uprv|ustr|utext|utf8|utrie|ulist|uset|unorm|uscript|ubidi|ucptrie|umtx|uenum|ucnv)', n): return 'ICU'
    if n.startswith(('OT::', 'AAT::', 'CFF::', 'graph::')) or re.match(r'_hb_|hb_', n): return 'HarfBuzz'
    if 'impeller::' in n: return 'Impeller'
    if re.search(r'\bSk|skia|skgpu|sktext|skif|skcms|SkSL|skcpu|skvx|sksl|skottie|neon::|sse::|hsw::|skx::|portable::|lowp::|SkOpt', n): return 'Skia'
    if 'txt::' in n or 'minikin' in n: return 'text (txt)'
    if 'flutter::' in n or 'fml::' in n or '[Flutter' in n or 'Flutter' in n or 'InternalFlutterSwift' in n: return 'Flutter engine'
    if re.match(r'_(png|jpeg|jpg|WebP|VP8|WebP|jinit|jcopy|jpeg_|WebPD|VP8L|SharpYuv|wuffs|gif)', n) or 'wuffs' in n: return 'image codecs'
    if re.match(r'_(deflate|inflate|crc32|adler32|zlib|compress|uncompress|gz|zcalloc|zcfree|_tr_|Cr_z)', n) or 'Cr_z_' in n: return 'zlib'
    if 'rapidjson::' in n or 'absl::' in n: return 'other libs'
    if n.startswith('std::') or n.startswith('__cxxabiv1') or n.startswith('operator ') or n.startswith('_ZN'): return 'libc++/std (unattributed)'
    return 'other/unattributed'
tot = collections.Counter()
for i, (a, n) in enumerate(rows):
    nxt = rows[i + 1][0] if i + 1 < len(rows) else END
    tot[cls(n)] += nxt - a
first = rows[0][0] - START
text_total = SIZE
print(f"__text {text_total/1e6:.2f} MB, symbols in range {len(rows)}, leading gap {first} B")
for k, v in tot.most_common():
    print(f"{k:34s} {v/1e6:6.2f} MB  {100*v/text_total:5.1f}%")
```

### Output, 2026-09-14

The timings were produced by the commands of `rounds.sh` run as three
loops the same hour; `rounds.sh` gathers them into one script. In the
first file the `ts-hermes` lines came from a TypeScript bundle built
before the harness took its final form (a calibrated p50, printed as
`seam_us`); they are superseded by the second file, whose bundle has
the harness above. F2 and F5 take minimums across these files.

`build.sh`, from the attribution on:

```text
__text 6.97 MB, symbols in range 26610, leading gap 0 B
Skia                                 1.36 MB   19.5%
Dart VM (dart::)                     1.30 MB   18.6%
Flutter engine                       1.02 MB   14.6%
Impeller                             0.91 MB   13.1%
other/unattributed                   0.63 MB    9.1%
BoringSSL                            0.44 MB    6.3%
HarfBuzz                             0.31 MB    4.5%
ICU                                  0.30 MB    4.3%
dart:io embedder (dart::bin)         0.23 MB    3.3%
libc++/std (unattributed)            0.20 MB    2.8%
tonic (Flutter<->Dart bindings)      0.13 MB    1.9%
image codecs                         0.06 MB    0.9%
other libs                           0.05 MB    0.7%
zlib                                 0.02 MB    0.3%
text (txt)                           0.01 MB    0.2%
ts_twin.js              5916 B  gzip -9     1888 B
ts_bundle.hbc          12185 B  gzip -9     6757 B
hello.aot             970464 B  gzip -9   331514 B
module.aot           1046992 B  gzip -9   364206 B
hello_js.js             5157 B  gzip -9     1844 B
module_js.js           62616 B  gzip -9    20955 B
module_js_o4.js        60222 B  gzip -9    20219 B
module_js.hbc         113489 B  gzip -9    58368 B
hello_wasm.wasm         3115 B  gzip -9     1754 B
module_wasm.wasm       59129 B  gzip -9    24264 B
module_wasm.mjs        11419 B  gzip -9     3192 B
hello.dbc                374 B  gzip -9      278 B
module.dbc              8553 B  gzip -9     5092 B
ios/hello.App         925712 B  gzip -9   320802 B
ios/module.App        963696 B  gzip -9   337124 B
4760064 /opt/homebrew/share/flutter/bin/cache/dart-sdk/bin/dartaotruntime
5814656 bench_aot
dart2wasm 26 _317:function _315:function _316:function _29:function _30:function _185:function _191:function _192:function _178:function _179:function _193:function _212:function
Math 3 sin:function cos:function asin:function
 177 ):global  :global (:global north:global , :global :global NaN:global 
:global defaultLocation:global stations:global nearest:global station:global
WebAssembly 1 JSTag:tag
exports 4
Dart_LoadScriptFromBytecode: Cannot load bytecode as dynamic modules are disabled.
dart-jit (native VM)   20 answers, 20/20 equal; "at" printed with .0: true
dart2js on Hermes      20 answers, 20/20 equal; "at" printed with .0: false
dart2wasm on V8        20 answers, 20/20 equal; "at" printed with .0: true
```

Compile times, warm, one run each (`dart compile`, before `build.sh` renamed the outputs):

```text
dart compile exe bench.dart -o bench_aot                     1.07s rc=0
dart compile aot-snapshot module.dart -o module.aot          0.96s rc=0
dart compile aot-snapshot hello.dart -o hello.aot            0.70s rc=0
dart compile kernel module.dart -o module.dill               0.27s rc=0
dart compile js -O2 module.dart -o module_js_o2.js           0.57s rc=0
dart compile js -O4 module.dart -o module_js_o4.js           0.48s rc=0
dart compile js -O2 hello.dart -o hello_js_o2.js             0.32s rc=0
dart compile js -O2 bench.dart -o bench_js.js                0.49s rc=0
dart compile wasm -O2 module.dart -o module_o2.wasm          0.91s rc=0
dart compile wasm -O2 hello.dart -o hello_o2.wasm            0.49s rc=0
dart compile wasm -O2 bench.dart -o bench_wasm.wasm          0.78s rc=0
dart compile exe module.dart -o module_aot                   0.74s rc=0
```

`board_probe`:

```text
$ ./board_probe            # AOT
records+sort  120.928 us
class+sort    21.179 us
class nosort  12.611 us
$ dart run board_probe.dart # JIT
records+sort  124.615 us
class+sort    17.052 us
class nosort  14.031 us
```

Rounds, first file (Hermes and AOT alternated; then the JIT references):

```text
== round 1 load { 11.71 17.20 21.36 }
-- ts-hermes
bench defaultLocation seam_us=0.793 compute_us=0.458
bench station seam_us=1.221 compute_us=0.534
bench search seam_us=2.441 compute_us=1.465
bench nearest seam_us=7.568 compute_us=7.080
bench board seam_us=30.273 compute_us=22.461
-- dart-aot
bench defaultLocation seam_min=0.978 seam_p50=1.089 compute_min=0.477 compute_p50=0.535
bench station seam_min=0.994 seam_p50=1.126 compute_min=0.155 compute_p50=0.165
bench search seam_min=1.352 seam_p50=1.514 compute_min=0.374 compute_p50=0.429
bench nearest seam_min=3.341 seam_p50=3.679 compute_min=1.558 compute_p50=1.766
bench board seam_min=31.805 seam_p50=35.188 compute_min=11.680 compute_p50=14.137
-- dart2js-hermes
bench defaultLocation seam_min=11.062 seam_p50=12.136 compute_min=5.435 compute_p50=6.127
bench station seam_min=10.504 seam_p50=11.905 compute_min=2.413 compute_p50=2.610
bench search seam_min=12.500 seam_p50=14.881 compute_min=3.655 compute_p50=4.098
bench nearest seam_min=43.103 seam_p50=50.000 compute_min=22.727 compute_p50=26.042
bench board seam_min=359.375 seam_p50=416.667 compute_min=196.429 compute_p50=229.167
== round 2 load { 12.96 16.86 21.06 }
-- ts-hermes
bench defaultLocation seam_us=0.671 compute_us=0.366
bench station seam_us=1.129 compute_us=0.458
bench search seam_us=2.136 compute_us=1.404
bench nearest seam_us=7.324 compute_us=6.348
bench board seam_us=27.344 compute_us=20.020
-- dart-aot
bench defaultLocation seam_min=0.869 seam_p50=0.970 compute_min=0.425 compute_p50=0.491
bench station seam_min=0.938 seam_p50=1.049 compute_min=0.137 compute_p50=0.149
bench search seam_min=1.274 seam_p50=1.377 compute_min=0.352 compute_p50=0.404
bench nearest seam_min=3.248 seam_p50=3.607 compute_min=1.624 compute_p50=1.741
bench board seam_min=31.410 seam_p50=34.196 compute_min=11.945 compute_p50=13.092
-- dart2js-hermes
bench defaultLocation seam_min=9.690 seam_p50=11.364 compute_min=5.297 compute_p50=6.158
bench station seam_min=10.163 seam_p50=12.019 compute_min=2.525 compute_p50=2.867
bench search seam_min=12.626 seam_p50=14.881 compute_min=4.325 compute_p50=6.345
bench nearest seam_min=41.667 seam_p50=54.348 compute_min=21.186 compute_p50=25.510
bench board seam_min=343.750 seam_p50=390.625 compute_min=178.571 compute_p50=208.333
== round 3 load { 18.26 17.69 21.21 }
-- ts-hermes
bench defaultLocation seam_us=0.671 compute_us=0.336
bench station seam_us=1.129 compute_us=0.488
bench search seam_us=2.075 compute_us=1.404
bench nearest seam_us=7.813 compute_us=6.836
bench board seam_us=28.320 compute_us=21.484
-- dart-aot
bench defaultLocation seam_min=0.968 seam_p50=1.073 compute_min=0.388 compute_p50=0.478
bench station seam_min=0.962 seam_p50=1.203 compute_min=0.151 compute_p50=0.166
bench search seam_min=1.327 seam_p50=1.514 compute_min=0.421 compute_p50=0.458
bench nearest seam_min=3.458 seam_p50=3.938 compute_min=1.734 compute_p50=2.082
bench board seam_min=35.427 seam_p50=41.147 compute_min=12.827 compute_p50=15.384
-- dart2js-hermes
bench defaultLocation seam_min=11.261 seam_p50=14.368 compute_min=5.981 compute_p50=6.684
bench station seam_min=11.574 seam_p50=15.060 compute_min=2.535 compute_p50=2.907
bench search seam_min=12.755 seam_p50=15.060 compute_min=3.655 compute_p50=4.296
bench nearest seam_min=41.667 seam_p50=54.348 compute_min=25.000 compute_p50=28.409
bench board seam_min=375.000 seam_p50=437.500 compute_min=208.333 compute_p50=229.167
== references (JIT)
-- dart-jit
bench defaultLocation seam_min=0.960 seam_p50=1.153 compute_min=0.536 compute_p50=0.619
bench station seam_min=1.137 seam_p50=1.268 compute_min=0.183 compute_p50=0.207
bench search seam_min=1.468 seam_p50=1.794 compute_min=0.540 compute_p50=0.601
bench nearest seam_min=3.614 seam_p50=4.190 compute_min=1.830 compute_p50=2.008
bench board seam_min=34.667 seam_p50=41.091 compute_min=15.036 compute_p50=18.251
-- dart2wasm-node
compile 0.54 ms, instantiate 1.09 ms
bench defaultLocation seam_min=0.909 seam_p50=1.003 compute_min=0.264 compute_p50=0.336
bench station seam_min=1.096 seam_p50=1.234 compute_min=0.211 compute_p50=0.232
bench search seam_min=1.481 seam_p50=1.638 compute_min=0.596 compute_p50=0.649
bench nearest seam_min=3.513 seam_p50=3.901 compute_min=1.892 compute_p50=2.365
bench board seam_min=19.536 seam_p50=22.559 compute_min=10.428 compute_p50=11.350
-- ts-node
bench defaultLocation seam_min=0.337 seam_p50=0.392 compute_min=0.175 compute_p50=0.208
bench station seam_min=0.348 seam_p50=0.412 compute_min=0.033 compute_p50=0.039
bench search seam_min=0.686 seam_p50=0.771 compute_min=0.229 compute_p50=0.271
bench nearest seam_min=1.268 seam_p50=1.415 compute_min=0.672 compute_p50=0.776
bench board seam_min=7.220 seam_p50=8.474 compute_min=2.668 compute_p50=3.277
```

Rounds, second file (TypeScript on Hermes rebuilt with the final harness, alternated with AOT):

```text
== round 1 load { 15.57 16.95 20.61 }
-- ts-hermes
bench defaultLocation seam_min=0.623 seam_p50=0.685 compute_min=0.304 compute_p50=0.350
bench station seam_min=0.955 seam_p50=1.126 compute_min=0.410 compute_p50=0.488
bench search seam_min=1.825 seam_p50=2.306 compute_min=1.356 compute_p50=1.510
bench nearest seam_min=7.764 seam_p50=9.259 compute_min=6.281 compute_p50=8.681
bench board seam_min=27.778 seam_p50=32.051 compute_min=21.930 compute_p50=25.000
-- dart-aot
bench defaultLocation seam_min=1.164 seam_p50=1.306 compute_min=0.567 compute_p50=0.628
bench station seam_min=1.285 seam_p50=1.471 compute_min=0.183 compute_p50=0.205
bench search seam_min=1.451 seam_p50=1.607 compute_min=0.433 compute_p50=0.478
bench nearest seam_min=3.742 seam_p50=4.254 compute_min=1.843 compute_p50=2.071
bench board seam_min=33.173 seam_p50=36.980 compute_min=13.306 compute_p50=15.266
== round 2 load { 17.04 17.22 20.62 }
-- ts-hermes
bench defaultLocation seam_min=0.585 seam_p50=0.774 compute_min=0.377 compute_p50=0.422
bench station seam_min=1.110 seam_p50=1.236 compute_min=0.556 compute_p50=0.594
bench search seam_min=2.367 seam_p50=2.593 compute_min=1.530 compute_p50=1.741
bench nearest seam_min=8.446 seam_p50=8.993 compute_min=6.545 compute_p50=8.013
bench board seam_min=27.778 seam_p50=30.488 compute_min=19.531 compute_p50=21.186
-- dart-aot
bench defaultLocation seam_min=0.977 seam_p50=1.051 compute_min=0.459 compute_p50=0.514
bench station seam_min=1.096 seam_p50=1.250 compute_min=0.156 compute_p50=0.168
bench search seam_min=1.285 seam_p50=1.623 compute_min=0.439 compute_p50=0.479
bench nearest seam_min=3.998 seam_p50=4.406 compute_min=1.607 compute_p50=2.173
bench board seam_min=36.516 seam_p50=39.805 compute_min=15.223 compute_p50=16.429
== round 3 load { 16.92 17.18 20.51 }
-- ts-hermes
bench defaultLocation seam_min=0.861 seam_p50=0.952 compute_min=0.403 compute_p50=0.455
bench station seam_min=1.204 seam_p50=1.326 compute_min=0.516 compute_p50=0.568
bench search seam_min=2.193 seam_p50=2.632 compute_min=1.432 compute_p50=1.656
bench nearest seam_min=7.102 seam_p50=8.865 compute_min=6.010 compute_p50=7.267
bench board seam_min=24.510 seam_p50=29.070 compute_min=18.939 compute_p50=20.492
-- dart-aot
bench defaultLocation seam_min=0.911 seam_p50=1.015 compute_min=0.428 compute_p50=0.511
bench station seam_min=0.993 seam_p50=1.115 compute_min=0.138 compute_p50=0.156
bench search seam_min=1.369 seam_p50=1.489 compute_min=0.410 compute_p50=0.445
bench nearest seam_min=3.638 seam_p50=4.186 compute_min=1.710 compute_p50=1.823
bench board seam_min=34.502 seam_p50=38.148 compute_min=12.521 compute_p50=13.870
```

The web engines (`web_refs.txt`; its last block was cut by an over-broad filter and rerun below):

```text
-- dart2js-node
bench defaultLocation seam_min=0.777 seam_p50=0.946 compute_min=0.411 compute_p50=0.493
bench station seam_min=0.956 seam_p50=1.353 compute_min=0.328 compute_p50=0.362
bench search seam_min=1.472 seam_p50=1.987 compute_min=0.424 compute_p50=0.498
bench nearest seam_min=3.541 seam_p50=3.870 compute_min=1.903 compute_p50=2.101
bench board seam_min=31.250 seam_p50=36.765 compute_min=23.585 compute_p50=26.042
-- dart2js-bun(JSC)
bench defaultLocation seam_min=0.979 seam_p50=1.160 compute_min=0.363 compute_p50=0.508
bench station seam_min=0.951 seam_p50=1.055 compute_min=0.186 compute_p50=0.217
bench search seam_min=1.346 seam_p50=1.497 compute_min=0.517 compute_p50=0.575
bench nearest seam_min=3.482 seam_p50=4.045 compute_min=2.097 compute_p50=2.376
bench board seam_min=23.585 seam_p50=27.778 compute_min=10.081 compute_p50=10.776
-- ts-bun(JSC)
bench defaultLocation seam_min=0.227 seam_p50=0.254 compute_min=0.121 compute_p50=0.135
bench station seam_min=0.219 seam_p50=0.248 compute_min=0.030 compute_p50=0.036
bench search seam_min=0.506 seam_p50=0.645 compute_min=0.308 compute_p50=0.337
bench nearest seam_min=0.998 seam_p50=1.126 compute_min=0.854 compute_p50=0.968
bench board seam_min=4.388 seam_p50=4.937 compute_min=1.659 compute_p50=1.817
-- dart2wasm-bun(JSC)
compile 2.23 ms, instantiate 0.61 ms
case 12 {"error":{"kind":"BadArguments","message":"count"}}
case 13 {"error":{"kind":"BadArguments","message":"count"}}
case 14 {"error":{"kind":"BadArguments","message":"expected 1 arguments, got 0"}}
case 15 {"error":{"kind":"BadArguments","message":"location"}}
case 16 {"error":{"kind":"Unavailable","message":"station nowhere"}}
case 17 {"error":{"kind":"BadArguments","message":"direction"}}
case 18 {"error":{"kind":"BadArguments","message":"argument 0"}}
```

```text
$ bun run_wasm.mjs bench_wasm   # rerun: web_refs.txt's filter cut these lines
compile 1.85 ms, instantiate 0.78 ms
bench defaultLocation seam_min=1.780 seam_p50=2.198 compute_min=0.339 compute_p50=0.405
bench station seam_min=2.507 seam_p50=3.014 compute_min=0.227 compute_p50=0.271
bench search seam_min=3.788 seam_p50=4.243 compute_min=1.172 compute_p50=1.373
bench nearest seam_min=7.452 seam_p50=9.061 compute_min=4.257 compute_p50=4.835
bench board seam_min=40.808 seam_p50=51.333 compute_min=17.041 compute_p50=19.193
```
