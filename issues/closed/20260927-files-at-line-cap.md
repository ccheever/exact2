# Seven sources sit at the 1,500-line cap and are being compressed onto single lines to stay under it

**Status:** Closed
**Resolution:** The recorded 22 module splits already landed; current staged caps scan validates source sizes. Remaining unrelated platform failures belong to their own issues.
**Systems:** `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift`, `scripts/agent.mjs`, `host/web/glue.js`, `host/linux/src/content_region/presenter_tests.rs`, `kernel/build.rs`, `kernel/src/kernel.rs`, `host/linux/src/text/residency_tests.rs`
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** rules/RULES.md (the file cap), `bun scripts/caps.mjs`

At `c74615a3` these files are at 1,499, 1,499, 1,499, 1,499, 1,498, 1,496 and 1,496 lines. Code is being squeezed rather than split: `host/web/glue.js` has 300-character one-liners (for example `:307`, `:748`, `:805`), `host/apple/Sources/ExactKit/Bridge.swift:231-232`, and commit `0eeb76a2` "agent.mjs back under the cap". The next feature that touches any of them fails `caps` or adds more one-liners, which defeats the cap's purpose (files a reader can hold).

**Fix:** split each along its seams now. `glue.js` has clear candidates: the request path, the agent seek, and batch apply.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: split 22 crowded sources along module seams; required checks pass and boot stays at 2 modules; supplemental checks retain known macOS raster, Chrome Keychain, and game compile failures (details below).

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max. Verification: confirmed with `wc -l`.

## Resolution

Re-measured at `a3ec5bea` using caps' newline counting: 21 sources were within
50 lines of 1,500. `NodeViewIOS.swift` was just outside that band (1,448), so it
was split too. `kernel/src/kernel.rs` (1,416) and
`host/linux/src/content_region/presenter_tests.rs` (1,014) already had room and
were left alone. No presence-resize source was changed.

| Source | Before | After | Seam |
| --- | ---: | ---: | --- |
| `kernel/build.rs` | 1,499 | 1,337 | Schema validation → `build/validate.rs` |
| `kernel/src/txn.rs` | 1,465 | 1,122 | Transaction tests → `txn/tests.rs` |
| `kernel/src/style.rs` | 1,453 | 1,369 | Finite-value and touch-action test modules |
| `runner/src/instance.rs` | 1,473 | 1,159 | Region instances → `instance/region.rs` |
| `runner/tests/it/now_screen.rs` | 1,463 | 1,168 | Retained action-binding tests |
| `gpu/src/lib.rs` | 1,456 | 1,209 | Child texture composition → `children.rs` |
| `game/engine/src/sim.rs` | 1,488 | 1,334 | Simulation save/restore → `sim/snapshot.rs` |
| `host/linux/src/paint.rs` | 1,462 | 1,175 | Paragraph paint tests |
| `host/linux/src/presenter.rs` | 1,494 | 1,245 | Update delivery → `presenter/delivery.rs` |
| `host/linux/src/presenter/display_frame/tests.rs` | 1,458 | 1,009 | Follow-end growth tests |
| `host/linux/src/surfaces.rs` | 1,496 | 1,021 | Surface ABI tests |
| `host/linux/src/text/ink_tests.rs` | 1,452 | 840 | Cached placement tests |
| `host/linux/src/text/residency_tests.rs` | 1,497 | 1,098 | Trim comparison tests |
| `host/web/src/host.rs` | 1,466 | 1,374 | Font catalog/decode cache → `host/fonts.rs` |
| `host/web/tests/it/host.rs` | 1,456 | 1,275 | CSS projection tests |
| `host/web/glue.js` | 1,473 | 1,427 | Guest-frame outline/input → existing `navigation.js` |
| `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift` | 1,448 | 1,389 | Plain and scroll containers → `ScrollViewIOS.swift` |
| `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift` | 1,487 | 1,351 | `ChainingScrollView.swift` |
| `host/apple/Sources/ExactKit/Mac/PresenterMac.swift` | 1,487 | 1,373 | `PageScrollView.swift` |
| `host/apple/tests/ExactKitTests/TextGeometryTests.swift` | 1,491 | 1,279 | Borrowed measurement tests |
| `scripts/agent.mjs` | 1,460 | 1,266 | Source-map decoration and transcripts → `agent-inspect.mjs` |
| `scripts/deploy.mjs` | 1,464 | 1,336 | Signing → `deploy-signing.mjs` |

Compressed native creation, tint sizing, image retirement, UIKit editing, and
driver dispatch code were expanded. Guest-frame helpers use the already-loaded
navigation module: ordinary agent calls stay synchronous, and the first-pixel
graph remains exactly two modules. No new command entrypoints, checks, schema declarations,
or generated sources were added.

Tests added: none; this is a behavior-preserving source split. Existing test
bodies moved with their modules. The agent test double now supplies the current
host's `presence` object, which its state-read path already required at the base.

## Verification

The size finding was reproduced by measuring the base; no behavioral regression
test was added for a file-size/refactoring ticket. All measurements above use the
same newline count as caps (one more than `wc -l` for a final newline).

- `cargo build --all-targets --keep-going`: exit 0;
  `Finished dev profile [unoptimized + debuginfo] target(s) in 42.23s`.
- `cargo test --lib --bins --tests --no-fail-fast`: exit 0;
  1,758 passed, 0 failed, 8 ignored across 71 test binaries.
- `cargo clippy --all-targets --keep-going -- -D warnings`: exit 0;
  `Finished dev profile [unoptimized + debuginfo] target(s) in 12.41s`.
- `cargo fmt --all -- --check`: exit 0. The separate game package's fmt
  check also passed.
- `git add -A && bun scripts/caps.mjs`:
  `source files: 1655 scanned, 0 skipped as generated, 409 skipped as vendored`;
  `All budgets within cap. 6 categories inspected.`
- `bun scripts/boot.mjs`:
  `modules reachable before first pixel: 2 (host/web/glue.js, host/web/navigation.js)`;
  allowed import paths only.
- `cargo test -p exact-web -p exact-linux -p exact-gpu --lib --tests --no-fail-fast`:
  794 passed, 0 failed, 6 ignored across 10 test binaries. All-target Clippy for
  those three crates also passed with warnings denied.
- `bun test host/web/agent.test.mjs`: `36 pass`, `0 fail`, `2397 expect() calls`.
- `bun test scripts/deploy.test.mjs`: `16/16 passed`.
- `bun host/apple/build.mjs --test --ios`:
  `Executed 55 tests, with 0 failures (0 unexpected)`; `** TEST SUCCEEDED **`.
- `bun host/apple/build.mjs --test`:
  `Executed 450 tests, with 1 failure (0 unexpected)`. The only failure is the
  already-recorded `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
  (`1` instead of `240`; see `QUEUE.md`). All 53 text geometry tests, including
  the moved borrowed-measurement tests, passed.
- `bun host/web/build.mjs`: passed. `bun scripts/smoke.mjs web` completed the
  full app and host-fixture drive, including guest-frame interaction and the
  pinned transcript: `web smoke: 1 failure(s) in 76.2 s`. Its only failure was
  Chrome's Keychain `errSecInteractionNotAllowed` / encryption diagnostics,
  already recorded in `QUEUE.md`; no functional assertion failed.
- `cargo test --manifest-path game/Cargo.toml -p exact-game --lib --no-fail-fast`:
  blocked by existing E0599 at `engine/src/values.rs:111`, where
  `values.as_ref().write(w)` calls `write` on a slice with no such method.
  Re-running with the original `sim.rs` at `a3ec5bea` produced the same error.
  Recorded in `QUEUE.md`; no value-encoding change belongs to this refactor.

Nothing awaits a design ruling. The three unrelated verification failures above
remain outside this ticket's scope.
