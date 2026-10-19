# D2 cold-build measurements

2026-10-18, lane/1132.A2-cold-build. Cold and registry-reuse phase measurements; setup and live-edit results follow below.

Machine: Mac17,6, 18 logical CPUs, 128 GiB RAM; macOS host triple
`aarch64-apple-darwin`. Bun 1.4.2, stable rustc 1.97.0
(2d8144b78), web nightly 2026-08-21. The Linux headless host is built
for this machine's native triple, as the proof runner does.

The machine is heavily loaded. These are loaded measurements, not the RFC's
three runs under load 5. Repeat if load falls below 10 during this lane.

Method: start with `bun scripts/exact.mjs new /tmp/exact-1132-a2-cold
--game --assets`. Each measured target and its Cargo build directory start
empty. Registry source downloads and installed SDK tools are already present.
Cargo receives `--timings`; a rustc wrapper records `-Ztime-passes` (with
`RUSTC_BOOTSTRAP=1`) and each unit's wall duration and load averages. It
preserves Cargo's jobserver descriptors. The link column uses the parent `LLVM_fatlto` / `LLVM_thinlto` timers
only when rustc enables cross-crate LTO, plus `run_linker`. Local
ThinLTO/code generation stays with the crate, including in `gpu-dev`. The recorded timer names make this distinction explicit.
`wasm-opt` processes are sampled every 200 ms; their durations have that
sampling uncertainty at each end.

A pilot that closed jobserver descriptors was discarded and its target was
moved aside before measurement. It contributes no numbers below.

`exact new` also starts a Contract-compiler prebuild in the SDK target; its
reported PID was 80505. That prebuild completed before the measured web build
left its initial lint. It is separate from the game's cold target.

Native command selection: `bun exact.mjs linux` currently delegates to
`scripts/build-linux.mjs`, which selects `host-dev` and builds the host only.
The `gpu-dev` measurement therefore uses the proof runner's Linux build-only
leg, which builds both the GPU artifact and the host. The release row uses
the same leg with `EXACT_GAME_PROOF_PROFILE=release`. Each has both
`CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` pointing to its own empty
subdirectory under the game's excluded `target/`.

## Cold web

Command: `D2_LOG=$PWD/.lane/measure/cold-web python3
game/bench/cold-build/probe.py bun /tmp/exact-1132-a2-cold/exact.mjs web-build`.
The probe adds `--timings` to Cargo builds and records rustc timers. Exit 0.
Elapsed: **1,388.7 s**, load averages before **197.49 / 120.21 / 76.52**,
after **151.99 / 205.98 / 198.44** (1 / 5 / 15 minutes). Observed one-minute
load reached **333.60**. The filesystem-tool build is in the SDK target;
the editor and text-flow builds also compile into that target.

Summed compiler-unit work below is **unit-seconds, not additive wall time**.
Each number was measured under the web run's load range above. Link/LTO time
is subtracted from the crate columns. Build-script execution is additional
and visible in Cargo's HTML; it is included in the dependency-chain table
that follows when it blocks a root artifact.

| Phase | Compiled units | Unit-seconds | One-minute load context |
|---|---:|---:|---|
| Registry crates | 339 | 1,006.1 | 152–334 |
| `exact-game*`, `exact-gpu*` path crates | 14 | 256.1 | 152–334 |
| Game logic and generated GPU/web shells | 7 | 114.7 | 152–334 |
| Core and vendored path crates | 68 | 1,027.5 | 152–334 |
| Rust standard library | 37 | 343.1 | 152–334 |
| LTO/link-timed units | 95 | 201.9 | 152–334 |
| `wasm-opt` (sampled wall time) | 6 observed processes | about 22.2 | 152–334 |

The `wasm-opt` sum is approximate: processes shorter than a sampling interval
can be missed, and polling includes process-list and scheduler latency.

The Cargo HTMLs expose the units each metadata/final artifact unblocked.
Following those edges backward from the final artifact partitions each Cargo
invocation's elapsed dependency chain without adding parallel crate times.
Registry time in the GPU and host chains is **236.8 s**; the editor/text-flow
chains add **10.6 s**. The initial 70.0 s lint and remaining orchestration are
reported separately rather than assigned to an invented compiler phase.
The GPU artifact alone is not the whole cold web build.

## Cold native `gpu-dev`

Command (from this worktree):

```sh
D2_LOG=$PWD/.lane/measure/cold-linux \
CARGO_TARGET_DIR=/private/tmp/exact-1132-a2-cold/target/d2-linux \
CARGO_BUILD_BUILD_DIR=/private/tmp/exact-1132-a2-cold/target/d2-linux \
EXACT_GAME_PROOF_PROFILE=gpu-dev \
python3 game/bench/cold-build/probe.py bun /tmp/exact-1132-a2-cold/proof.mjs linux --build-only
```

Exit 0, elapsed **537.0 s**. Load before **139.15 / 200.46 / 196.60**,
after **237.44 / 216.67 / 202.25**. Output:

```text
Finished `gpu-dev` profile [optimized] target(s) in 5m 51s
PASS all recorded children exited
PROOF UNVERIFIED exact-1132-a2-cold linux: 0 failures; 536.864 s
```

The GPU Cargo invocation took 179.2 s; the host invocation took 351.7 s.
Across their nonoverlapping dependency chains, registry crates account for
**184.7 s (34.4% of the end-to-end build)** before excluding path-dependants.
This loaded run does not satisfy D2's three quiet-run oracle.
The source classifier matches package **name and version**, preserving
underscores, so a vendored version and a registry version of `wgpu-hal` do
not collapse into one category. Local ThinLTO in `gpu-dev` is crate code
generation, not cross-crate linking.

## Cold phase table and cache gate

Nonoverlapping wall seconds on Cargo's completed dependency chains. The
remaining wall time includes linting, tooling, scheduling and orchestration;
it is not silently assigned to registry compilation. The table adds to each
end-to-end total. Every number in a build column shares the load context
in its heading. `Game/shells` includes generated host/GPU wrappers; the
unit-work table above shows compilation done concurrently as well.

| Phase, seconds | Web, load 152–334 | Native `gpu-dev`, load 124–260 | Native release, load 71–222 |
|---|---:|---:|---:|
| Registry | 247.4 | 184.7 | 98.1 |
| Engine path crates | 58.9 | 32.1 | 61.2 |
| Game/shells | 109.3 | 17.0 | 46.5 |
| Core/vendor path crates | 401.6 | 265.1 | 374.1 |
| Standard library | 249.9 | 0 | 0 |
| Cross-crate LTO and linker | 186.0 | 1.1 | 227.0 |
| `wasm-opt`, sampled | about 22.2 | 0 | 0 |
| Other, including initial lint | 113.4 | 37.0 | 95.2 |
| **Total** | **1,388.7** | **537.0** | **902.1** |

The release command is the native command above with `D2_LOG` ending in
`cold-release`, both Cargo directories ending in `d2-release`, and
`EXACT_GAME_PROOF_PROFILE=release`. Exit 0; load before
**221.72 / 214.00 / 201.47**, after **71.49 / 95.87 / 136.29**:

```text
Finished `release` profile [optimized] target(s) in 9m 29s
PASS all recorded children exited
PROOF UNVERIFIED exact-1132-a2-cold linux: 0 failures; 901.848 s
```

The native `gpu-dev` registry chain alone is **34.4%** of its complete build,
without counting parallel registry work twice or assigning unclassified time
to it. This is loaded evidence, not cache admission. A registry-only generation trial follows.
The initial profile table was completed before the cache trial. The release
leg spends much more of its chain in path crates and cross-crate LTO.

Dependency-timing audit:

```sh
python3 game/bench/cold-build/audit.py /tmp/exact-1132-a2-cold/target/d2-linux \
  /tmp/exact-1132-a2-cold/target target
```

All six HTML reports passed. A negative control makes one predecessor finish
one second after its dependent starts; the audit rejects it in every report.

## Registry eligibility and reuse

The cache excludes every path/git package and any registry package depending
on one, transitively. That reduces the native `gpu-dev` registry dependency
chain to **151.7 s, 28.3% of 537.0 s** in this single loaded run. The release eligible chain is **62.6 s, 6.9% of 902.1 s**. The entire
web registry chain is only **17.8%** of its cold build. The orchestrator's round-1 decision leaves the cache built and unwired for all
profiles, including setup. Three cold `gpu-dev` runs under load 5 must put
the eligible registry chain over a quarter before any integration is enabled.

The subtraction is checkable from the two native GPU/host HTML chains:

| Native `gpu-dev` registry chain | Seconds |
|---|---:|
| All registry segments | 184.723101083 |
| Exclude `wgpu-core-deps-apple` (depends on path `wgpu-hal`) | −0.130000000 |
| Exclude `wgpu-core` (transitive path-dependant) | −24.220000000 |
| Exclude `wgpu` (transitive path-dependant) | −8.640000000 |
| **Eligible registry segments** | **151.733101083** |
| End-to-end build | 537.032320976 |
| **Eligible fraction** | **28.253998%** |

The vendored `wgpu-hal` segment was already classified as Core/vendor path;
it is not subtracted twice. The shipped SDK lock gives the same exclusions
as the measured generated shell lock. Recompute with:

```sh
python3 game/bench/cold-build/audit.py .lane/measure/retained-timings/2/d2-linux \
  --phases .lane/measure/cold-linux --lock game/app/shells.lock
```

The round-1 audit reproduced the table and passed its late-predecessor
negative control on both reports. The executable probe and audit are committed
under `game/bench/cold-build/`; its README gives fresh-target commands for all
three legs. The following reuse results describe the earlier integration
trial; that integration and its setup primer have now been removed.

A fresh generated game took a coherent generation, then built with the same
Cargo timings and rustc timer wrapper. The wrapper exits 86 if Cargo invokes
rustc for a copied unit whose completed fingerprint is present. It did not
fire. A preceding transfer trial also built successfully with the source
target directory moved away; reuse does not depend on that target remaining
available. Changes to the compiler instrumentation create a separate key.

| Phase, seconds | Web control, load 65–116 | Native `gpu-dev` reuse, load 134–226 | Release experiment, load 70–188 |
|---|---:|---:|---:|
| Registry | 144.3 | 49.7 | 0.0 |
| Engine path crates | 30.0 | 44.0 | 103.7 |
| Game/shells | 55.4 | 13.5 | 54.9 |
| Core/vendor path crates | 109.9 | 288.4 | 372.8 |
| Standard library | 62.5 | 0 | 0 |
| Cross-crate LTO and linker | 87.3 | 1.1 | 242.1 |
| `wasm-opt`, sampled | about 10.5 | 0 | 0 |
| Other | 70.1 | 21.1 | 74.4 |
| **Total** | **570.0** | **417.8** | **847.9** |

Rounding can change a displayed sum by 0.1 s. The release experiment took a
registry generation solely to test its acceptance; that profile is **not
cached by the shipped build**. Its path/LTO bottleneck leaves no registry
work on the final dependency chain even though registry units still compile
in parallel. This is why compiler work and elapsed dependency chains are
reported separately.

| Compiler work, unit-seconds | Native `gpu-dev` reuse, load 134–226 | Release experiment, load 70–188 |
|---|---:|---:|
| Registry | 301.9 | 318.2 |
| Engine | 292.4 | 225.3 |
| Game/shells | 26.5 | 61.2 |
| Core/vendor | 1,419.6 | 913.4 |
| LTO/link | 4.4 | 246.1 |

`gpu-dev`: **392 copied fingerprints, zero rebuilt despite reuse**; registry
compiler invocations fall from **379 to 34**. The remaining registry units
are path-dependent `vello`, `wgpu`, `wgpu-core`, `wgpu-core-deps-apple`,
`wgpu-hal`, and the SDK helper's separate debug-profile dependencies:
`base64`, `block-buffer`, `cfg-if`, `cpufeatures`, `crypto-common`, `digest`,
`generic-array`, `itoa`, `libc`, `memchr`, `serde_core`, `serde_json`, `sha2`,
`typenum`, `version_check`, `zmij`. Some have multiple versions or compiler
units. Path dependants intentionally do not enter a generation.

Release experiment: **422 copied fingerprints, zero rebuilt despite reuse**;
registry compiler invocations fall from **444 to 69**. Neither number is a
claim that all units in a generation are needed by every build.

Reuse commands use the cold native command with the fresh game's
`proof.mjs`, its empty private target/build directory, and
`D2_STOP_ON_REBUILD=$PWD/.lane/measure/product-gpu-dev-hashes.json` (or
`product-release-hashes.json`). Outputs:

```text
game/cache: ... starts with the 392 registry units this machine has compiled
PASS all recorded children exited
PROOF UNVERIFIED integrated-game linux: 0 failures; 417.412 s

PASS all recorded children exited
PROOF UNVERIFIED integrated-game linux: 0 failures; 847.388 s
```

Reuse load before/after (1 / 5 / 15 minutes): `gpu-dev`
**192.09 / 154.84 / 153.01 → 136.05 / 150.53 / 153.51**; release experiment
**133.71 / 149.56 / 153.13 → 77.73 / 109.78 / 126.40**.

The web control runs `web-build` again with the original app's entire target
moved aside, so the game target is fresh but the SDK's editor/text-flow leaf
artifacts are resident. It copies no registry generation. Load is
**98.13 / 111.81 / 126.50 → 65.60 / 90.71 / 109.88**. Its shorter total is
not attributed to the native cache. The cold/control GPU wasm files have
identical SHA-256:
`e373aa7da20c744b9406f5c93f8e704d1f16fd7919a08afd27eac76d9db7bcdd`.
The game's web remap flags include its directory; two generated games have
different keys. Key isolation is an actual negative control, not a copied
unit that Cargo rejected.

An earlier transfer trial's end-to-end proof was invalidated by a tracked
measurement-note commit during its run. Its successful Cargo transfer is
not counted as an end-to-end proof; the stable-source run above is the
recorded result.

The rejection guard itself has a direct negative control: a completed fake
fingerprint and a rustc invocation for the same hash. It exits 86 before
running rustc:

```text
86 D2 CACHE REJECTED: Cargo invoked rustc for copied fingerprint 0123456789abcdef
```

This is a guard fixture, not a real Cargo rejection tolerated during reuse.
Round 1 added a real Cargo negative control. It found that Cargo may clear
the completed fingerprint before invoking rustc, so the earlier observer can
miss a rebuild. The committed probe instead refuses any invocation whose
hash was in the pre-build copied-completed snapshot. Its regression copies
a completed tiny Cargo target, changes source, and sees observer exit 86
(Cargo exit 101). The historical zero-rebuild claims above are not conclusive;
repeat acceptance counts with this observer before enabling the cache.
Timing HTMLs remain at `.lane/measure/retained-timings/` in this worktree,
including the native cold reports under `2/d2-linux/cargo-timings/`. They
are untracked lane evidence; the orchestrator files them with the reviews.
Unit JSONL, summaries and logs remain under `.lane/measure/`. Retiring targets reclaims disk space and does
not alter the recorded runs.

## Earlier setup trial and live Rust edits

The setup option and primer below were removed in round 1; this section
records the earlier experiment, not the current CLI.

`PATH="$HOME/.cargo/bin:$PATH" bun scripts/exact.mjs setup --game` exits 0:
`SDK tools ready.` It built the asset-capable primer's GPU and native host
with `gpu-dev`, retained its registry generation, and removed the primer.
The full setup took **910.4 s**, load **112.27 / 86.22 / 99.56 →
136.43 / 298.13 / 241.95**. This includes pinned Hermes installation and
readiness checks, not just compilation; required verification and dev
startup ran concurrently. It is not a build speed comparison.

The actual loop command was `bun game/dev.mjs /tmp/exact-1132-a2-cold
--port 8953`. After the watcher announced itself, the edit probe added an
inline comment to `const HZ: u32 = 120;`, waited for a swap, then restored
the original line and waited again. Code:
`host/web/dev.mjs:1297` selects `gpu-dev` when the workspace defines it;
the producer runs Cargo then wasm-bindgen, without wasm-opt.

| Live edit | Producer time | Cargo | Bindgen | One-minute load range |
|---|---:|---:|---:|---|
| First use of wasm `gpu-dev` | 212.326 s | 210.180 s | 2.146 s | 106.20–136.43 |
| Warm one-line restore | 4.800 s | 2.606 s | 2.194 s | 106.63–109.30 |

First edit load before/after (1 / 5 / 15 minutes):
**136.43 / 298.13 / 241.95 → 109.30 / 201.68 / 212.40**. Warm edit:
**109.30 / 201.68 / 212.40 → 106.63 / 199.60 / 211.60**. Both overlap the
required repository checks; these are loaded edit measurements, not latency
budgets. Output:

```text
gpu: 1 source file(s) changed; building exact-1132-a2-cold-gpu (gpu-dev)
gpu: rebuilt in 212326 ms (cargo 210180 ms, bindgen 2146 ms); swap pushed
gpu: 1 source file(s) changed; building exact-1132-a2-cold-gpu (gpu-dev)
gpu: rebuilt in 4800 ms (cargo 2606 ms, bindgen 2194 ms); swap pushed
```

**Handoff to LLP 1127.001 D1:** the current Rust-edit GPU producer selects
`gpu-dev` correctly. F3's `Finished web profile` is not the profile this
watcher used after either measured edit. The initial shipping host remains
`web`; startup builds of native producers are separate from edit timings.
The original Rust file was restored. The web agent drove Play, held W for
500 ms, and settled; the player reached z=-1.7333331 at tick 229. The
1280×720 screenshot was inspected: capsule, two beacons, grid and controls
rendered. See report.md for the exact drive and output.

## Normal namespace acceptance after setup

A second public scaffold command succeeded:
`bun scripts/exact.mjs new /tmp/exact-1132-a2-after-setup --game --assets`.
Without any compiler wrapper or bootstrap flag, its first native build
printed `starts with the 392 registry units this machine has compiled`.
The setup primer had already been deleted. The 345 copied compiler-unit
`invoked.timestamp` files and 47 copied build-script run fingerprints
retained their times (allowing the copy's sub-millisecond rounding).
The instrumented integration target's 47 build-script run fingerprints
also retained their times. Release timing reports show no nonzero-duration
eligible build-script run. Together with the rustc guard, this covers
compiler units and build-script execution separately.

The first fresh native proof deliberately refused generated directory
creation after compiling its GPU leg: exit 1, **104.5 s**, load
**75.14 / 165.87 / 197.48 → 100.48 / 145.38 / 185.63**. The existing E10
fresh/stable/cached test in `game/proof.test.mjs` expects this refusal.
The stable rerun exited 0, **192.2 s**, load
**104.35 / 139.63 / 181.00 → 98.19 / 122.17 / 165.39**:

```text
PASS all recorded children exited
PROOF UNVERIFIED exact-1132-a2-after-setup linux: 0 failures; 192.167 s
```

These are acceptance runs, not a replacement phase-table benchmark: the
first attempt stopped between GPU and host, and other verification was
active. A full native gameplay proof passed every assertion, including both
beacons, victory, replay, pause/restart and focus. Its default final exit
was 1 because this game has no baseline: that is the existing verdict
contract, owned by D4. `EXACT_PROOF_COMPARE=0` alone also exits 1 for a game with no pins;
collection additionally requires `EXACT_PROOF_REPIN`. No baseline was
written or accepted. The default run's load was **90.40 / 118.46 / 162.81 → 88.21 / 117.54 / 162.22**:


```text
PASS victory waits for every beacon (2/2)
PASS Play again clears victory and the lit count
PASS Restart leaves movement running
PASS all recorded children exited
PROOF UNVERIFIED exact-1132-a2-after-setup linux: 0 failures; 3.538 s
UNVERIFIED: no pins
```

It is a new game with no baseline, so this proves native loading and the
scripted gameplay, not comparison to saved pins. The stable build rerun
is the existing proof policy, owned by D3; the cache does not weaken it.
