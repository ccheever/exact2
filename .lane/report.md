# Lane 1132.A2 — D2

The original implementation record below is superseded by the Round 1 section.
Its cache-admission and setup claims describe the earlier trial.

2026-10-18. Branch `lane/1132.A2-cold-build`; no pushes.

## 1. What landed

- `c71794110`: document the initial web and Rust-edit profiles, the
  wasm-bindgen PATH prerequisite, and the native host/module build command.
- `0cc2889fd`: cold phase table before the cache decision.
- `91da0ae9d`: registry-only native `gpu-dev` generations, build integration,
  opt-in `setup --game` warmup, and Bun tests. Fix the isolated SDK fixture's
  copied module closure to include the cache and its existing missing imports.
- `88602c9c2`: keep the cache module a library and warm through the existing
  setup command, avoiding a main-module import cycle.
- `831bd6d6b`: reuse measurements, release limits, and the observed dev build
  shutdown footgun.
- `0fa59295e`: normal setup/cache acceptance, measured Rust-edit profile,
  dated As built note, and proof rerun guidance.

Cold elapsed times: web **1,388.7 s**, native `gpu-dev` **537.0 s**, native
release **902.1 s**. The eligible native `gpu-dev` registry dependency chain
is **151.7 s (28.3%)**, so it earns the cache. Web's complete registry chain
is 17.8%; release's eligible chain is 6.9%. Neither profile uses the cache.
The fresh-target integration trial reused **392 fingerprints without a
rebuild or rejection**, compiled 34 registry units rather than 379, and took
417.8 s under a different heavy load. This does not establish a speedup on
a quiet machine.

[measurement.md](measurement.md) contains the cold/reuse phase tables,
load averages, commands, compiler-unit work, controls, and acceptance counts.
[as-built.md](as-built.md) contains the dated D2 disposition and the RFC's
`As built` note for the orchestrator to port.

## 2. Oracle runs

The loaded phase-table runs all exited 0. Native build-only runs reported:

```text
PASS all recorded children exited
PROOF UNVERIFIED exact-1132-a2-cold linux: 0 failures; 536.864 s
PROOF UNVERIFIED exact-1132-a2-cold linux: 0 failures; 901.848 s
PROOF UNVERIFIED integrated-game linux: 0 failures; 417.412 s
```

UNVERIFIED is expected for build-only: it is compile evidence, not a claim
that a saved baseline passed. The negative timing control moves a
predecessor after its dependent; all six cold HTML reports reject it.
The cache tests exclude path/git packages and their dependants, reject a
populated target, isolate changed profiles/compiler flags, preserve dep-info
times, and retain two coherent generations without mixing independent
builds. A changed profile is a cache miss. The transfer wrapper would exit
86 on recompilation of a copied completed fingerprint; it never fired.

The cold and control web GPU wasm files have the same SHA-256:
`e373aa7da20c744b9406f5c93f8e704d1f16fd7919a08afd27eac76d9db7bcdd`.
The web control copies no registry generation and retains SDK leaf artifacts;
its shorter wall time is not attributed to the native cache.

Script verification:

```sh
bun test --timeout 60000 game/app/crates.test.mjs scripts/exact.test.mjs scripts/app-manifest.test.mjs
# 43 pass, 0 fail; 21 assertions
bun test --timeout 60000 scripts/app-bake.test.mjs
# 37 pass, 3 skip, 0 fail
bun test --timeout 60000 game/app/crates.test.mjs scripts/app-workspace.test.mjs
# 37 pass, 0 fail; 21 assertions
```

`bun scripts/exact.mjs setup --game` exited 0 and printed `SDK tools ready`.
The actual watcher used `gpu-dev` for both Rust edits: 212.326 s on the
first wasm profile build, then 4.800 s warm (2.606 Cargo, 2.194 bindgen).
Load ranges were 106–136 and 107–109 respectively; required verification
ran concurrently. No wasm-opt step runs in that producer. This finding is
handed to **LLP 1127.001 D1**; no dev-loop implementation is duplicated.

Live drive (exit 0):

```sh
EXACT_APP_DIR=/tmp/exact-1132-a2-cold PATH="$HOME/.cargo/bin:$PATH" \
  bun scripts/agent.mjs web --url http://127.0.0.1:8953 --size 1280x720 \
  tree "tap play" "type world key KeyW for 500" "clock settle" \
  "state world:player" "screenshot .lane/measure/dev-loop-final/play.png" logs
```

```text
tapped #8 "play"
clock 500 ms ... tick:60 ... changing:[player.Transform,camera.Transform]
clock 1908.334 ms ... tick:229 ... quiescent:true
player.Transform.position = [0,0.9,-1.7333331]
screenshot .../play.png · 1280×720
```

I inspected the screenshot: the capsule, both beacons, grid, HUD, and
controls rendered. An initial drive queried an undefined entity named
`clock`; the corrected player query above passed. The temporary Rust edit
was restored byte for byte. The recorded dev process subtree was stopped.

The public fresh consumer took setup's 392-unit generation after the primer
was deleted. Its first GPU build hit the existing generated-directory
refusal; the stable build-only rerun exited 0 with zero failures. An audit
found all 345 compiler-unit timestamps and 47 build-script run fingerprints
unchanged (sub-millisecond copy rounding allowed). The default full Linux proof passed every gameplay assertion but exited 1
because the new game has no baseline (the existing exit-code contract).
Disabling comparison alone also exited 1; collection additionally depends on
`EXACT_PROOF_REPIN`. No pins were written or accepted. The runtime output
was:

```text
PASS victory waits for every beacon (2/2)
PASS Play again clears victory and the lit count
PASS Restart leaves movement running
PASS all recorded children exited
PROOF UNVERIFIED exact-1132-a2-after-setup linux: 0 failures; 3.538 s
UNVERIFIED: no pins
```

Command: `EXACT_GAME_PROOF_PROFILE=gpu-dev bun
/tmp/exact-1132-a2-after-setup/proof.mjs linux`. The new game has no saved
baseline; this is runtime acceptance, not a baseline comparison.

All six root checks passed (the lint check includes Clippy and format):

```sh
cargo build --all-targets --keep-going
cargo test --tests --no-fail-fast
cargo clippy --all-targets --keep-going -- -D warnings
cargo fmt --all -- --check
bun scripts/caps.mjs
bun scripts/boot.mjs
cargo check --workspace --all-targets --keep-going
```

Root tests: **5,452 passed, 51 ignored, 0 failed** across 100 completed
binaries. Caps reported `All budgets within cap. 8 categories inspected.`
Boot reported `Allowed import paths only.` The full workspace check exited 0.
Raw outputs are in `.lane/verify/`; `root.json` records command, directory,
exit code, elapsed time and load averages for each Cargo/boot command.

Game workspace test command exited 0: **970 passed, 24 ignored, 0 failed**.

```sh
cargo test --manifest-path game/Cargo.toml --workspace --locked --lib --bins --tests --no-fail-fast
```

Game Clippy and format exited 0:

```sh
(cd game && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check)
```

The shell check exited 0 across all 14 game/bench fixture directories:

```sh
CARGO_TARGET_DIR="$PWD/target/game-validation" \
CARGO_BUILD_BUILD_DIR="$PWD/target/game-validation" \
  bun game/app/shells.mjs --test
```

Its private target is inside this worktree; no other checkout's target was
used. The log sums to **101 passed, 9 ignored, 0 failed**
including the fixtures' declared test/doc-test targets.

Beacons command: `bun game/prove.mjs beacons`.
The fresh attempt refused generated directory creation after its GPU build
(`.lane/verify/beacons.log`, exit 1). The unchanged stable rerun exited 0:

```text
| linux | 1 | 122.239 | equal | not requested | PASS |
PROOF PASS beacons
```

Stable run load (1 / 5 / 15 minutes): **58.67 / 92.15 / 109.35 →
56.99 / 81.10 / 102.74**. Its summary is `PASS` at
`/Users/ccheever/projects/exact2-wt/1132.A2-cold-build/game/games/beacons/artifacts/prove/run-2VuQIl/summary.json`.
No pins were changed. The first refusal is retained in the report; no
proof input check was weakened. All required checks are green on the final
source. Raw stable output/result: `.lane/verify/beacons-stable/`.

Completed verification incremental data, the standalone game test target,
and retired measurement targets were removed to relieve disk pressure.
Their command logs, numeric summaries and measurement timing HTMLs remain;
the normal setup cache and the post-setup consumer's working target remain.

## 3. Deviations

- The shared machine never approached the RFC's load-under-5 oracle. The
  requested loaded runs are recorded; three quiet runs per leg remain owed.
  No observed load below 10 triggered a repeat.
- Generic `bun exact.mjs linux` selects `host-dev` and builds only the host.
  The native `gpu-dev` measurement uses `proof.mjs linux --build-only`,
  which builds the GPU artifact and host as requested.
- Linux's headless host is compiled for this Mac's native triple,
  `aarch64-apple-darwin`; this is what the proof runner builds here.
- `setup --game` makes engine compilation explicit, preserving I13's optional
  engine. Ordinary setup continues provisioning tools and source dependencies.
- The release transfer was tested experimentally; it is not shipped because
  the release slice did not earn the quarter gate. Cache files never become
  proof inputs; no engine path artifact is cached.
- `docs/contributor-pitfalls.md` does not exist in this checkout.

## 4. Footguns

- `exact.mjs linux` does not build the game's GPU module; the README and
  Games pitfalls give the host/module command.
- wasm-bindgen must be on PATH even when already installed in `~/.cargo/bin`;
  the game README names the prerequisite.
- Stopping the dev wrapper during a build can leave its build child alive.
  The Games pitfalls and Queue name the issue. Only this lane's recorded
  subtree was terminated.
- Instrumenting rustc must preserve Cargo's jobserver descriptors. The pilot
  that closed them was discarded before measurement.
- A tracked SDK commit during a proof can invalidate its input receipt.
  The premature transfer trial is not counted as an end-to-end proof.
- A first native proof can deliberately refuse generated directory creation.
  The fresh consumer reproduced this path; `game/proof.test.mjs`'s E10
  fresh/stable/cached case expects that refusal. The Games pitfalls names
  the rerun; this lane does not change the proof input policy owned by D3.
- Web remap flags name the game directory. Sharing native registry artifacts
  does not justify sharing web artifacts across generated games.

## 5. Unverified

The quiet three-run phase oracle remains unverified because the shared
machine stayed loaded. No quiet performance claim is made. Native Linux
runs here qualify the headless host on the Mac's native target; they do not
qualify Linux KMS, window or input behavior on a Linux OS.


## Round 1

The orchestrator's decision is to land the cache built and unwired. Removed
`buildBake`'s cache import/take/keep and the `setup --game` flag and primer;
there is no retained primer to isolate (fix item 7 is moot). README and Queue
now require three cold `gpu-dev` runs under load 5 before wiring reuse.
The loaded 28.254% result is evidence, not admission.

The dormant module now takes Cargo metadata and places intermediates in its
resolved build directory, relocates both product/build prefixes and their
realpaths, seeds host/triple profiles independently, and keys strip plus the
toolchain override. Populated skipped profiles never enter a generation;
older coherent cached profiles are preserved. Tests cover the requested
versioned lock and same-name path/registry cases, actual Cargo metadata with
only `CARGO_TARGET_DIR` overridden, dep-info aliases, cache-free orchestration
for web/release/native development, and refusal of the removed setup flag.

Committed probe/audit commands are under `game/bench/cold-build/`. The audit
reproduced 184.723101083 − 0.13 − 24.22 − 8.64 = 151.733101083 s; dividing by
537.032320976 s gives 28.253998%. The subtraction table is in measurement.md.
All 22 retained timing HTMLs passed dependency-edge audits and their applicable
late-predecessor controls. The HTML stays in `.lane/measure/retained-timings/`
as untracked evidence for the orchestrator to file with the reviews.

The probe's new real-Cargo negative control caught a defect in the earlier
observer: Cargo can clear the completed fingerprint before invoking rustc.
The committed probe now checks the pre-build copied-hash snapshot directly.
`bun test game/bench/cold-build/probe.test.mjs` passed: the cold tiny fixture
compiled, copying its completed fingerprint then changing its source made
Cargo invoke rustc, and the observer exited 86 (Cargo/outer command 101).
This tests the rejection observer, not registry eligibility or a speedup.
The earlier historical guard's "zero rebuilt" claim is therefore limited;
a quiet admission trial must use the corrected observer and recheck counts.

The cache tests passed after fixing one failure in independent-profile
retention. The final standalone cache run passed all six tests and 39
assertions. The required full command, root and game checks, and fresh native
build results are appended after the runs complete.
