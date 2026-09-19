# FOLD full verification and receiving hosts — 2026-09-20

Commands, exit codes, elapsed times and free disk for each command are retained in
`~/lanes/gamenext/scratch/fold/full-results.json`; raw logs are in `logs/full-*`.
Corrections and final reruns follow below. All builds keep inherited debug=0 and
incremental=0, `EXACT_UPDATE_TRUST=development`, the pinned Bun 1.3.12 and a
shared build cache in this clone. TMPDIR is under the authorized FOLD scratch.
Generated app shells alone resolve with `--locked --offline`; their source locks
all retain ureq 3.4.2. The root workspace has no game member or dependency.

## Final command results

The full sweep ran 61 commands; final correction checks and pristine addenda
are recorded separately in `final-results.json`, and diagnostic reruns in
`diagnostic-results.json`. A final discovered app/tooling repair is recorded in
`repair-results.json`. Exit failures remain visible in those records.

| Scope | Passed | Failed | Ignored / skipped | Meaning |
|---|---:|---:|---:|---|
| Game Cargo workspace | 704 | 18 | 24 | All failures need an adapter; 3 are new engine cases. New 200k diagnostic passed separately. |
| Nine ordinary app workspaces | 43 | 3 | 1 | Only the three named pixel tests refuse the adapter. Includes cubes logic. |
| Root app-tooling Bun | 29 | 0 | 2 | Pristine also passes 29; isolated imports and ordinary no-lock bake repaired without weakening assertions. |
| Affected core | 538 | 3 | 1 | Native registry load failures reproduced pristine; actual wasm host/glue transaction fixtures execute and pass. |
| Fixed Lanterns workspace | 4 | 0 | 1 | Historical construction, full three-mode invariant, moved primitive feed and clip negative control. Timing diagnostic also passed separately. |
| Game Bun | 137 | 3 | 1 | Chrome reuse, focused browser buttons, and generated-game browser proof cannot launch Chrome. Generated game's Linux assertions pass. |
| Host web Bun fixtures | 143 | 5 | 2 | Four xcrun failures and Caltrain Chrome ENOENT. The two conditional wasm skips execute through the Rust core suite. |
| Established Linux proofs | 28 executions | 0 | 0 | Seven normal runs plus continuous/Save/FreshGame paranoid modes. No pin changes after the recorder-only reconciliation. |

Core selection: `exact-kernel`, `exact-plan`, `exact-runner`, `exact-linux`,
`exact-web`, `exact-motion`, `exact-gpu`, `contract-syntax`, `contract-types`,
`contract-analyze`, `contract-lower`, `contract`. All selected core clippy checks
pass. Game workspace and all ten consumers pass clippy `-D warnings` and fmt. Root build, test and clippy were
executed and stop at TypeScript app bakes requiring the absent lean Hermes
executor (Weatherlight / Messages / Fieldnotes in these runs). Root fmt passes.
No green root build or root test result is claimed.

Six additional real Linux collectors recorded live and fixed Lanterns in all
three modes. Their assertion failure arrays are empty, and production
`agreePins` accepts identical complete inventories and runtime game identities:
live `lanterns`, three tick keys/two saves; fixed `lanterns-evidence`, 1,210 ticks
and 1,210 full saves per mode. Fixed comparisons cover every full-save byte.
Collector command exits are **1 / UNVERIFIED**, because the first shared pins do
not exist. Both Linux-only first-baseline commands refuse publication as intended;
both committed pin files remain empty. Only scratch candidates were written.

The two engine E10 profile proofs in game Bun actually build Beacons and skinned
with `gpu-dev` and release and compare observations; they pass. No profile or
pin policy was relaxed. Tests using optional adapter early returns remain included
in the numeric passes but certify no GPU execution.

## Measurements and bounds

Release 200k interleaved/churn initializer diagnostic: base 11,672,475 bytes,
new initializer 23,872,519 bytes; preparation 1.013007 s, merge 2.372347 s;
peak RSS 992,380 kB. All original value/identity/report omission controls pass.
The added complete typed restore diagnostic (diary 007) measures a
53,158,370-byte save, 327.796 ms malformed refusal with **zero** setup calls,
4.121289 s valid restore plus merge, and 1,333,528 kB peak RSS. Its 200,000
value checks prevent empty or unchanged results from passing.

Fixed I3 release, 120 ticks: whole simulation (animation included) median
229,879 ns, primitive feed 23,025 ns, peak RSS 16,896 kB;
final hash `0xe616dc56c02e8795`. Work and refusal bounds are unchanged: decode
and projection limits in diary 007, primitive feed 1M slots, fixed proof ten
probes and 120 continuations each. These are local measurements, not speedups.

All six existing release physics diagnostics were rerun and pass. At 20k
statics / 100 bodies: step 88.988 µs, 1k rays 591.466 µs, 1k overlaps
692.248 µs (medians). Single-static teleport + checked queries: 3,723.230 µs
median, 3,730.669 ms total for 1,000 edits. One/eight controller batches:
1.207/374.461 µs; interleaved pages with one controller: batch 2.529 µs,
step 742.913 µs, checked query pair 750.394 µs. Movement-only one/eight:
median 18.868/397.086 µs, p95 45.708/31,981.157 µs. Changing scene extrema:
median/p95 4,569.413/4,900.835 µs versus ordinary 17.897/18.187 µs.
1024² terrain: first ray 8,004.736 µs, first controller 7,160.991 µs;
RSS world/query/controller 12,352/14,824/20,052 KiB. Canonical-fallback tails
remain visible; no improvement is inferred from run-to-run differences.

The unchanged `bun game/bench/size.mjs fold-final --app beacons` builds the
production web artifacts, then refuses missing Twiggy. Binaryen is also absent.
The unoptimized GPU module is 2,323,714 bytes / 515,440 gzip, SHA256
`42309289bb00ba430502fa843ce79d12eb2b17ced17ac10e379f10d50b45533f`;
preopt 4,856,999 bytes. `WebAssembly.compile` accepts the bytes. No current
optimized size, function attribution or GPU boot result is claimed. Receipt:
`scratch/fold/primitive-size.json`; all logs remain in the authorized scratch.

Caps pass: 736 source files scanned, no source over 1,500 lines. Boot passes:
two reachable JS modules, one wasm reference, 88,723 JS bytes, 3,298-byte page.
These are source-graph counts, not browser startup or GPU timing evidence.

## Receiving-host matrix (replaces diary 005's matrix)

Use the repository-pinned Bun 1.3.12 (package.json), captured locks and
`EXACT_UPDATE_TRUST=development`. Record an obtained adapter; a test that returns
early without one certifies no GPU execution. The seven established games are
`asset-fixture`, `beacons`, `greybox`, `particles-fixture`, `placement-fixture`,
`skinned-fixture`, and `sprites-fixture`.

1. Native GPU: run
   `cargo test --manifest-path game/Cargo.toml --workspace --no-fail-fast`,
   `bun game/app/shells.mjs --test`, and
   `cargo test --manifest-path game/verification/lanterns/.shells/Cargo.toml --workspace --locked --offline --no-fail-fast`
   (prepare the fixed shell first with
   `bun game/app/shells.mjs game/verification/lanterns`). Explicitly run
   `cargo test --manifest-path game/Cargo.toml -p exact-game-render restoring_fox_uploads_zero_asset_bytes_after_ready -- --nocapture`
   without `--ignored`, and
   `cargo test --manifest-path game/Cargo.toml -p exact-game-render surface_lifecycle -- --ignored --nocapture`.
   The strict Fox assets are renderer-owned; the acceptance is unchanged.
   The ordinary suite now also includes
   `quad_tests::r14_unprepared_particle_draw_names_the_refusal`,
   `quads::retained_tests::r14_native_children_reserve_order_before_frame`, and
   `renderer::e10_tests::full_glow_blooms_without_clipping_the_lit_pixel_to_white`.
   Run `cargo test -p exact-gpu --no-fail-fast` for the three native registry
   failures. Inspect real pixels, including non-white emissive Glow.
2. Chrome/WebGPU: for each established game run
   `bun game/prove.mjs <game> --repin --hosts linux,web`.
   Require this folded branch's hashes and continuation saves (the two Glow
   consumers changed through the recorder); only host/provenance should change.
   Run first baselines with
   `bun game/prove.mjs lanterns --hosts linux,web` and
   `bun game/prove.mjs game/verification/lanterns --hosts linux,web`, without
   `--repin`. Require complete three-mode inventories and all 1,210 fixed
   tick/full-save observations per mode; fixed pins must identify
   `lanterns-evidence`, not its directory basename. Run
   `bun game/bench/cubes/proof.mjs web` separately; it has no Linux proof/pins.
   Run `cd game && bun test` with CHROME set to an installed browser, including
   carrier isolation and focused-button keyboard/hover assertions. Verify that
   Beacons and skinned `gpu-dev` and release observations agree.
3. macOS and iOS simulator: after the baselines exist, run
   `bun game/prove.mjs <game> --hosts macos,ios --compare-saves` for all seven,
   live Lanterns and `game/verification/lanterns`. Run
   `swift test --package-path host/apple`, `bun scripts/smoke.mjs host`, and
   `bun scripts/smoke.mjs host-ios` with built embedding fixtures. Exercise
   play/restart/save/load after HUD commit, restore with held controls, editor
   focus, multiple canvases, complete/partial/failed detach ACK, late textures
   and presentation after device recovery. Recheck Beacons victory focus and
   Greybox focused-button Space/Enter ownership. Simulator input does not prove
   physical-touch timing; no physical iPhone result is claimed.
4. Browser and Apple pixels: inspect perspective/orthographic/integer scaling,
   parents, sprites, transparency/depth, mirrored/skinned geometry, affine
   sockets, missing assets and stale poses through placement/sprites/skinned
   proofs. Geometric visibility samples do not certify raster visibility.
5. Browser and Apple live capture: capture a running Lanterns won/lost instance
   before restore, with held controls and fractional clock phase. Replay prefix
   zero and the full stream; continue and check the first controlled seek.
   Refuse malformed captures, wrong artifacts, early live phase and incomplete
   ownership ACK. Headless success does not establish live scheduling.
6. Real WebGPU: lose the device during asset delivery and staged authored reload;
   exercise commit and rollback. Check recovered Fox pixels, child placement,
   controls, loaded/requested artifact identities and real readiness. Exercise
   explicit plan+world Carry while predecessor assets are still loading.
7. A producer with lean Hermes, Binaryen, Twiggy, Chrome and Apple SDK: rerun the
   root five checks (`cargo build --workspace`, `cargo test --workspace`,
   `cargo clippy --workspace --all-targets -- -D warnings` plus
   `cargo fmt --all -- --check`, `bun scripts/caps.mjs`, `bun scripts/boot.mjs`),
   `bun test ./host/web/tests`, and
   `bun game/bench/size.mjs fold-final --app beacons`. Build Caltrain web first
   (`bun host/web/build.mjs caltrain-web`) so its placement test actually runs.
   The four Apple placement tests require xcrun. Caltrain's historical 60-second
   Chromium timeout was not reproduced here: both branches currently fail with
   missing Chrome after successful bakes. Its old underlying cause is unresolved.
   Verify optimized byte attribution and launch primitive Beacons and the starter.

Trials remain opt-in, not a gate: no agent trials were dispatched. Keep diary
005's disclosed unchanged direct-crate baseline failure and six unsupported
rendering controls. Fixed I3 still lacks simultaneous apex/moving-crate/active
blend coverage; the EXPHYS deferred-bit negative-control debt remains. Nothing
here erases those coverage limits or claims a speedup.

## Delivery state

The merge is `d39daaf`; recorder-only pin reconciliation `1f0a536`;
end-to-end unfavorable restore `2e0e44d`; final capture/app-tooling repair
`18b523e`. Environmental proof is a separate commit `6f446d9` and diary 008;
this full verification note is committed separately. All detected merge defects
are repaired with original assertions retained. No hardware red is reported as a
passing gate, and no historical timeout is relabeled as proven environmental.
Pristine tracked status stayed empty; its worktree and all separate Cargo outputs
were removed. The final filesystem has approximately 88 GiB free, above the
25 GiB stop floor. No pushes, remote service commands or agents were used.
