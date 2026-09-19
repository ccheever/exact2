# FOLD full verification and receiving hosts — 2026-09-20

Commands, exit codes, elapsed times and free disk for each command are retained in
`~/lanes/gamenext/scratch/fold/full-results.json`; raw logs are in `logs/full-*`.
Corrections and final reruns follow below. All builds keep inherited debug=0 and
incremental=0, `EXACT_UPDATE_TRUST=development`, the pinned Bun 1.3.12 and a
shared build cache in this clone. TMPDIR is under the authorized FOLD scratch.
Generated app shells alone resolve with `--locked --offline`; their source locks
all retain ureq 3.4.2. The root workspace has no game member or dependency.

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
