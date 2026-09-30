# Paint-owner and presence bookkeeping is written twice (Apple, Linux) and has diverged; move it below the hosts

**Status:** Closed
**Resolution:** Fixed: native paint ownership, per-view appearance and layout observation now share kernel motion policy; reproduced Linux's first-report transition and verified the fix with kernel/host regressions, the five checks and UIKit tests. Unrelated host-check failures are recorded below. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** `host/apple/src/paint.rs`, `host/apple/src/presence.rs`, `host/linux/src/paint_motion.rs`, `host/linux/src/presence.rs`, runner or kernel `motion`
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1062, LLP 1063; Charlie accepted the recommendation 2026-09-27

Both Rust hosts implement the same policy: paint-owner adoption, boot adoption, the "stays `currentcolor`" rule, scheme re-targeting, `node_key`, layout observation and `observe_box`. They already differ: Apple resolves appearance per view (`paint.rs:48`), and Linux uses one `self.dark` (`paint_motion.rs:28`).

**Do:** move the policy into the runner or the kernel's motion module, so a host only presents values. Keep Apple's per-view appearance as the semantics, and have Linux follow it. Every future host (Android) then inherits one copy. Keep the crate order: each crate depends on strictly less than the one above it.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: native paint ownership, per-view appearance and layout observation now share kernel motion policy; reproduced Linux's first-report transition and verified the fix with kernel/host regressions, the five checks and UIKit tests. Unrelated host-check failures are recorded below.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

`kernel/src/motion/paint.rs` now owns adoption, destruction cleanup,
`currentcolor` bookkeeping and appearance retargeting. Both hosts use Apple's
first-report correction and subsequent per-view transitions. Linux carries the
resolved appearance into its painter and invalidates partial repainting when it
changes, so settled rows agree with the motion targets. Its presenter reports
the initial appearance at boot and restores it when replacing a session.

`kernel/src/motion/layout.rs` owns layout seeding, observation, retirement and
the presented offset/size calculation. `motion.rs` supplies the shared
generation-checked `node_key`. Apple retains its exit views and batch emission;
Linux retains its documented exit refusal and pixel presentation. No dependency,
feature, schema, script or press-feedback change was needed.

Five regressions were added beside the existing paint/presence tests: first
appearance correction on Linux (failed before the fix), per-view Linux pixels
through rejoining the session, shared appearance retargeting, destroyed-owner
slot reuse, and layout seeding/resize/hide/show/retirement. Existing Apple
`currentcolor`, per-view keyframe, windowed-layout and exit tests also pass.

Verification on 2026-09-27:

- Required Cargo build, tests, Clippy and formatting: `build=0 test=0 clippy=0 fmt=0`;
  1,753 tests passed, none failed, eight ignored across 71 binaries.
- Staged cap check: `All budgets within cap. 6 categories inspected.`
- Boot check: `modules reachable before first pixel: 2`; allowed import paths only.
- Apple Rust integration: `110 passed; 0 failed`.
- Linux Rust: `432 passed; 0 failed; 1 ignored` (library), `70 passed; 0 failed`
  (integration), `59 passed; 0 failed` (pinned paint/text).
- Host Clippy: passed for both Apple and Linux, all targets, warnings denied.
- `bun host/apple/build.mjs --ios --test`: `Executed 53 tests, with 0 failures`;
  `TEST SUCCEEDED`.
- `bun host/apple/build.mjs --test`: `Executed 446 tests, with 1 failure`;
  `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
  reports 1 instead of 240, also reproduced before the fix and already in QUEUE.
- `bun scripts/smoke.mjs macos`: five failures, identical before and after:
  two launch-fact assertions expect seed 0 instead of 1, scroll limit 667
  instead of 652, unavailable `screencapture`, and the deck tap focuses `null`.
  The motion, precedence and three app tests pass.
- `bun scripts/smoke.mjs linux`: only the two seed-0/seed-1 launch-fact
  assertions fail, identically on base `c1b01a13`; motion, precedence, canvas
  readback and three app tests pass.
- The broader Apple Rust library run has one environment failure:
  `abi::tests::fresh_preparation_reads_platform_secrets_and_defers_effects_until_commit`
  cannot write to Keychain (`User interaction is not allowed`), also reproduced
  on base `c1b01a13`; 138 other tests pass.

No design decision or motion work remains for Charlie; the host-check failures
above are outside this ticket's scope.
