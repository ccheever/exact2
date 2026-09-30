# A viewport resize during a layout move: the web snaps, macOS keeps animating, Linux snaps a step late

**Status:** Closed
**Resolution:** Fixed: shared layout observation resets presentation in the resize frame, and Apple snaps every tracked box; Rust presence regressions and the web/macOS/Linux recordings verify immediate resize parity. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple host (`host/apple/src/presence.rs`, `host/apple/src/host.rs` resize, `Sources/ExactKit` window resize path), Linux host (`host/linux/src/presence.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063 D6 (a resize takes new boxes with no animation, on every host), `issues/closed/20260927-presence-cross-host-timeline.md` (the recorder that found it)

`bun host/web/parity.mjs --presence` records one 17-step timeline on each host. When the viewport is resized while a `layout-transition` move is running:
- **Web:** snaps at once (the 2026-09-27 fix). This is D6.
- **macOS:** keeps animating. `resize_inner` sets `presence.snap` (`host/apple/src/host.rs`), but the recorded boxes keep moving, so the window-resize path either doesn't reach it or the snap doesn't retire a move already in flight.
- **Linux:** snaps on the next clock step, not in the resize's own frame.

Linux also drops exit ghosts at once, but that is declared (LLP 1063 D8) and not in scope here.

**Fix:** on Apple and Linux, a resize retires every running `Property::Layout` animation and presents the new boxes in the same frame. The recorder's resize steps should then match on all three hosts. Sweep iOS when a simulator is available.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: shared layout observation resets presentation in the resize frame, and Apple snaps every tracked box; Rust presence regressions and the web/macOS/Linux recordings verify immediate resize parity.

Implemented on `fix/review-resize`:

- `kernel/src/motion/layout.rs` returns snapped nodes for immediate presentation
  reset. Linux's existing retirement path now clears its cached offset and scale
  during resize, without waiting for the next engine frame.
- `host/apple/src/layout.rs` and `presence.rs` observe all tracked layout boxes
  after resize, including boxes whose layout did not change, and emit their
  identity presentations in that batch.
- Regressions beside both hosts' presence tests cover running position and size
  transitions, fixed and fluid widths, the next tick, and a later animated move.
  The shared kernel test also requires the immediate reset. All three tests
  failed before the fix and pass after it.
- The shared `host/web/tests/fixtures/presence-motion.json` recording is refreshed
  from macOS after comparing its corrected timeline with web. Both resize samples
  now show the card at `(0, 50, 180, 80)` and sibling at `(0, 130, 50, 30)` on all
  three hosts; macOS and web agree at all 17 samples.

Verification (worktree-local builds):

```text
cargo build --all-targets --keep-going: Finished `dev` profile in 1m 08s
cargo test --lib --bins --tests --no-fail-fast: 1758 passed; 0 failed; 8 ignored (71 binaries)
cargo clippy --all-targets --keep-going -- -D warnings: Finished `dev` profile in 24.77s
cargo fmt --all -- --check: exit 0
bun scripts/caps.mjs: All budgets within cap. 6 categories inspected.
bun scripts/boot.mjs: modules reachable before first pixel: 2; wasm references: 8
cargo test -p exact-apple -p exact-linux -p exact-kernel --test it presence:: --no-fail-fast: Apple 10, Linux 4, kernel 9 passed; 0 failed
cargo clippy -p exact-apple -p exact-linux --all-targets --keep-going -- -D warnings: exit 0
bun test host/web/tests/presence.test.mjs host/web/tests/presence-loader.test.mjs: 12 pass; 0 fail
bun host/apple/build.mjs --test: PresenceMacTests 6 passed; full suite 450 tests, 1 unrelated failure
bun host/apple/build.mjs --test --ios --sim 028AE19D-05FD-4F0F-AA0B-536F93AABAA7: 55 passed; 0 failed; PresenceIOSTests 7 passed
bun host/web/parity.mjs --presence:
  presence web: all 17 samples match (box 0.1 pt, opacity 0.005)
  presence macos: all 17 samples match (box 0.1 pt, opacity 0.005)
  presence linux: 3 disagreements (the declared missing exit ghosts only)
  presence parity: 3 failure(s); recordings in target/presence-parity
```

The AppKit suite's sole failure is the existing
`RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps` assertion
at line 157 (1 source identity instead of 240), also reproduced and documented in
`20260927-presence-cross-host-timeline.md`. No raster-cache changes were made.

No design ruling is needed. Linux's three missing exit-ghost samples remain the
declared D8 limitation, outside this ticket. The iOS agent still refuses window
resize, so the full resize recording cannot run through that driver. The UIKit
presence tests passed on the iOS 27.0 iPhone Air simulator, including applying the
identity layout presentation that ends a move.
