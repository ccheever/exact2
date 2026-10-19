# Lane 1132.A2

## D2 — 2026-10-18, round 1

**Orchestrator decision:** keep the registry cache built but unwired. Loaded
cold builds took 1,388.7 s web, 537.0 s native `gpu-dev`, and 902.1 s native
release. The eligible native `gpu-dev` registry dependency chain was
151.7331 s of 537.0323 s (28.254%). This single run at load 124–260 does not
satisfy D2's three runs under load 5. No profile uses the cache, and setup
has no warmup flag or primer. Quiet measurement decides whether to wire it.

The dormant `game/app/crates.mjs` accepts Cargo metadata's resolved
`target_directory` and `build_directory`; registry intermediates go into
the latter, including when only `CARGO_TARGET_DIR` overrides products.
It relocates both directory prefixes and their realpath spellings in
dep-info. Host and triple profile directories seed independently; skipped
populated profiles remain private while older coherent cached units survive.
Keys include the explicit strip setting and `RUSTUP_TOOLCHAIN`, alongside
channel, profile, triple, compiler and flags. A rebuilt copied unit is a
failed warm. Path/git crates and all their transitive dependants stay out.
Cache files are outside proof inputs; simulation, saves and clocks are untouched.

Bun regressions exercise resolved Cargo directories, populated host/target
profiles, versioned dependencies, a same-name path/registry pair, and dep-info
realpath aliases. The real `buildBake` orchestration is exercised up to its
pre-build boundary with a cache spy: web, release and native `gpu-dev` do
not open a generation. Setup refuses the removed flag before provisioning.
The fresh native build-only run and checks are recorded in report.md.

`game/bench/cold-build/{probe.py,audit.py,README.md}` contains the rerunnable
measurement tools and exact commands. The audit reproduces the 32.99 s
path-dependant subtraction from the 184.7231 s registry chain and checks a
late-predecessor negative control. Retained timing HTML stays untracked
under `.lane/measure/retained-timings/` for the orchestrator to file with the
reviews. Loaded transfer trials remain historical evidence, not admission. A real
Cargo invalidation control exposed a gap in the earlier fingerprint observer;
the corrected probe checks pre-build copied hashes and now stops the rebuild.
The earlier zero-rebuild counts require rechecking before cache admission.

The measured Rust-edit watcher already selects `gpu-dev`, runs bindgen and
skips wasm-opt; its warm restore took 4.800 s at load 107–109. LLP 1127.001
D1 owns the dev loop's remaining performance work. Linux here is the headless
host compiled for this Mac's native triple. Three quiet runs remain owed.

## As built

D2 measured the cold web, native `gpu-dev` and native release legs under
heavy load. Its registry cache is built and unwired: the single loaded
eligible chain of 151.7331 / 537.0323 s (28.254%) does not meet the required
three-run load-under-5 oracle. That gate decides whether `buildBake` and
setup gain registry reuse. Directory placement, profile seeding, key inputs
and realpath relocation are corrected and tested in the dormant module;
measurement commands and the eligibility subtraction are reproducible.
The Rust-edit producer already uses `gpu-dev` (warm 4.800 s under load
107–109); this finding is handed to LLP 1127.001 D1.
