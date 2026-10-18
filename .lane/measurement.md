# D2 cold-build measurements

2026-10-18, lane/1132.A2-cold-build. Cold measurements complete; cache and edit measurements follow.

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
.lane/measure/probe.py bun /tmp/exact-1132-a2-cold/exact.mjs web-build`.
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
python3 .lane/measure/probe.py bun /tmp/exact-1132-a2-cold/proof.mjs linux --build-only
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
**184.7 s (34.4% of the end-to-end build)**. This crosses D2's quarter gate.
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

| Phase, seconds | Web, load 152–334 | Native `gpu-dev`, load 125–254 | Native release, load 71–222 |
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
so the cache gate is met without counting parallel registry work twice or
assigning unclassified time to it. A registry-only generation trial follows.
The initial profile table was completed before the cache trial. The release
leg spends much more of its chain in path crates and cross-crate LTO.

Dependency-timing audit:

```sh
python3 .lane/measure/audit.py /tmp/exact-1132-a2-cold/target/d2-linux \
  /tmp/exact-1132-a2-cold/target target
```

All six HTML reports passed. A negative control makes one predecessor finish
one second after its dependent starts; the audit rejects it in every report.
