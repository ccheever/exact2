# Cold game builds (LLP 1132 D2)

Run from the SDK root with pinned Bun, Python 3.9+ and rustup on PATH after
`bun scripts/exact.mjs setup`. The probe creates private wrapper symlinks under
`/tmp`, preserves Cargo's jobserver descriptors, and removes the wrappers when
it exits. Its rustc-wrapper path stays stable across cold/reuse runs. It records
wall times, start/end loads, `--timings` HTML, rustc phase timers and sampled
`wasm-opt` processes. Instrumentation sets `RUSTC_BOOTSTRAP=1` and changes the
compiler configuration; compare like configurations only.

These are the commands in `.lane/measurement.md`, with the committed probe path.
Use a new scaffold path and log directory for each repetition; never reuse a
compiled target for a cold sample. Both Cargo directories must start empty.

```sh
bun scripts/exact.mjs new /tmp/exact-1132-a2-cold --game --assets
D2_LOG=$PWD/.lane/measure/cold-web \
  python3 game/bench/cold-build/probe.py bun /tmp/exact-1132-a2-cold/exact.mjs web-build
D2_LOG=$PWD/.lane/measure/cold-linux \
CARGO_TARGET_DIR=/private/tmp/exact-1132-a2-cold/target/d2-linux \
CARGO_BUILD_BUILD_DIR=/private/tmp/exact-1132-a2-cold/target/d2-linux \
EXACT_GAME_PROOF_PROFILE=gpu-dev \
  python3 game/bench/cold-build/probe.py bun /tmp/exact-1132-a2-cold/proof.mjs linux --build-only
D2_LOG=$PWD/.lane/measure/cold-release \
CARGO_TARGET_DIR=/private/tmp/exact-1132-a2-cold/target/d2-release \
CARGO_BUILD_BUILD_DIR=/private/tmp/exact-1132-a2-cold/target/d2-release \
EXACT_GAME_PROOF_PROFILE=release \
  python3 game/bench/cold-build/probe.py bun /tmp/exact-1132-a2-cold/proof.mjs linux --build-only
```

`/private/tmp` is macOS's realpath of `/tmp`; use `/tmp` on Linux. A first
native proof may refuse newly generated directories: finish scaffold
preparation before measuring, and record a refusal instead of treating a
partial build as a completed cold sample. Do not change tracked SDK inputs
during a proof.

Audit timing dependencies, including a deliberately late predecessor, then
print the phase partition and path-dependant subtraction:

```sh
python3 game/bench/cold-build/audit.py /tmp/exact-1132-a2-cold/target/d2-linux \
  --phases .lane/measure/cold-linux --lock /tmp/exact-1132-a2-cold/.shells/Cargo.lock
```

Use the same command for the web and release log/target pairs (include the SDK
`target/` reports for the web's editor/text-flow work, selecting only that run's
reports). Pass `--game-prefix <scaffold-name>` when renaming the scaffold. The
JSON separates scheduling gaps, crate phases, link/LTO, sampled wasm-opt and
remaining wall time; add `cargo/scheduling` to `other` for the printed table's
Other row. Concurrent unit-seconds are never added as elapsed time. Eligibility
excludes registry packages tainted by any path/git dependency, by lock name.

For retained lane evidence, the exact recomputation command is:

```sh
python3 game/bench/cold-build/audit.py .lane/measure/retained-timings/2/d2-linux \
  --phases .lane/measure/cold-linux --lock game/app/shells.lock
```

The retained HTML and JSONL are untracked evidence filed by the orchestrator.
The cache is built but unwired. Its admission requires three cold runs per leg
under load 5, with eligible `gpu-dev` registry work above a quarter of total
wall time. Once admitted, repeat on fresh targets seeded from one generation;
set `D2_STOP_ON_REBUILD` to a JSON array of copied fingerprint hashes. The probe
exits 86 on any rustc invocation for a copied completed fingerprint. Cargo may
wrap that exit as 101; either is a failed warm. Count build-script run
fingerprints separately, since a rustc observer does not see their execution.

The observer regression is `bun test game/bench/cold-build/probe.test.mjs`.
It copies a completed Cargo target, changes source, and requires the observer
to stop Cargo's real rebuild. Snapshot only completed copied hashes before
starting Cargo: Cargo can clear a fingerprint before rustc starts.
