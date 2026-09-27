# Hold presence motion to one recorded timeline across hosts

**Status:** Fixed: one 17-step recorded timeline now compares presented surfaces, opacity and ghost lifetime on web/macOS/Linux; the sweep reproduces resize and Linux exit divergences below, and regression tests pass (iOS unavailable here).
**Systems:** `scripts/smoke.mjs` or the motion parity corpus, web/Apple/Linux
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063 (the 2026-09-27 ruling: no browser oracle, parity by a recorded timeline)

With the web running `layout-transition` and `exit-animation` by FLIP, Chrome can't serve as the oracle. Charlie ruled that parity is one recorded timeline compared across hosts.

**Do:** drive the same fixture on each host with the agent clock (`clock +N` steps through a move, a resize-during-move, a retarget, and an exit), record each moving node's presented box and opacity per step (`layout`/`state`), and compare hosts within a tolerance. Extend the existing motion parity cases (`host/web/src/parity.rs`) rather than adding a new tool.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

Implemented on `fix/review-presencetimeline`:

- `host/web/src/parity.rs` owns the Contract fixture and its relative-clock
  operations: move, size change during movement, retarget, viewport resize,
  another move, exit, and cleanup. The existing `parity` binary compiles it.
- `bun host/web/parity.mjs --presence` runs those operations through the agent
  on web, macOS and Linux. `--hosts` selects hosts. Every step records both
  moving nodes, including a missing node as `null`; comparisons check clocks,
  the operation sequence, surface boxes (0.1 point), opacity (0.005), and exits.
  All disagreements are reported, with a nonzero exit. Raw `layout`, `state`
  and logs remain in `target/presence-parity/<host>.json`; `comparison.json`
  lists the disagreements. No new script or blocking check was added.
- `host/web/tests/fixtures/presence-motion.json` is the shared macOS recording.
  It records what happened, not which host is correct. Explicitly replace it
  with `--record-presence <host>` after reviewing a changed timeline; normal
  runs never update it. Root-relative boxes remove only the native safe-area
  offset; discrepancies during the animation are retained.
- `state.presence` reads the presented surface and local opacity, including
  Apple/web exit ghosts that have left `tree`. Linux reads its painter's
  affine surface. The web driver now accepts the existing `tap` resize form
  through Chrome's viewport emulation. Its CDP transport moved to the
  existing `scripts/agent-launch.mjs` to leave `agent.mjs` below the line cap.

Reproduced first: the browser regression failed with
`TypeError: presence.live.observation is not a function`. It now reads a
65-point surface at 250 ms of its 1000 ms growth and the ghost's
0.6 opacity. Companion tests cover native surface/ghost inspection, fixture
compilation, and rejection of missing samples/ghosts, changed operations or
clocks, geometry/opacity drift, and non-finite values.

New findings from the same timeline (not repaired in this tooling ticket):

1. **A viewport resize has three different outcomes.** At 300 ms the card is
   at y=67.1, size 115.2 × 47.6, targeting y=50, size 180 × 80. The web snaps
   immediately to the target, as LLP 1063 D6 says. Linux still presents the
   old sample in the resize reply, then snaps on `clock +100`. macOS keeps
   animating: at 400 ms it is y=65.2, size 122.4 × 51.2. This affects the next
   retarget too. These are actual native presenter observations, not kernel
   target boxes. The fixture's fixed boxes avoid font-metric differences.
2. **Linux has no exit ghost.** At 1600/1700/1800 ms web and macOS retain the
   card with opacity 1/0.75/0.5; Linux has already removed it. Its logs say
   `exit-animation: refused on Linux`. This declared limitation is reported
   as a parity disagreement, not silently skipped. At 2001 ms every host has
   removed the ghost, and at 2601 ms the sibling has settled.
3. **iOS was not driven.** `xcrun simctl list devices booted` fails with
   `unable to find utility "simctl"`. Its observation is implemented alongside
   AppKit's, but remains unverified here. The existing iOS agent also refuses
   window resize; a device/simulator sweep needs a real supported resize path.

Verification (2026-09-27, worktree-local builds):

```text
cargo build --all-targets --keep-going: Finished `dev` profile in 1.42s
cargo test --lib --bins --tests --no-fail-fast: 1750 passed; 0 failed; 8 ignored (71 binaries)
cargo clippy --all-targets --keep-going -- -D warnings: Finished `dev` profile in 11.40s
cargo fmt --all -- --check: exit 0
bun scripts/caps.mjs: All budgets within cap. 6 categories inspected.
bun scripts/boot.mjs: modules reachable before first pixel: 2; wasm references: 8
bun test host/web/tests/presence.test.mjs host/web/tests/presence-loader.test.mjs: 12 pass; 0 fail
cargo test -p exact-web -p exact-linux --test it presence --no-fail-fast: 3 passed per host; 0 failed
cargo test -p exact-web --test it parity --no-fail-fast: 3 passed; 0 failed
cargo clippy -p exact-web -p exact-linux --all-targets --keep-going -- -D warnings: exit 0
bun host/apple/build.mjs --test: Executed 447 tests, with 1 failure
bun host/web/parity.mjs --presence: recorded 17 samples on each of web, macos, linux
presence web: 16 disagreements (box 0.1 pt, opacity 0.005)
presence macos: all 17 samples match (box 0.1 pt, opacity 0.005)
presence linux: 15 disagreements (box 0.1 pt, opacity 0.005)
presence parity: 31 failure(s); recordings in target/presence-parity
```

The one Apple-suite failure is outside presence:
`RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`,
line 157, expected 240 source identities but got 1. It repeated in both full
Swift test runs. All presence tests, including the new inspection test, pass.
The web, macOS and Linux app builds succeeded. No unrelated renderer fix was
attempted. Charlie has no outstanding design ruling for this ticket; the
resize discrepancies, Linux exit support and feasible iOS sweep remain follow-up
work, with the new comparison serving as their reproducer.
