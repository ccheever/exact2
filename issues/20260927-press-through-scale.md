# The press composes through CSS `scale`, never `transform`, and is kept under reduced motion

**Status:** Fixed: web composes authored scale and press feedback without touching transform; Apple and Linux keep feedback under reduced motion. Verified by browser regressions, gallery drive, 754 Rust host tests, 53 UIKit tests and six macOS press tests; full root gates and the macOS suite have unrelated failures listed below.
**Systems:** Web (`host/web/index.html`, `host/web/src/css.rs`, `host/web/input-glue.js`), Apple (`PressFeedback.swift`), Linux, kernel schema bit 147
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1061 (the 2026-09-27 ruling), LLP 1055.000 §8 ruling 3 (`transform` owed), LLP 1001

The web shell's `[data-pressed] { transform: scale(var(--exact-press)); }` (`host/web/index.html:63`) writes CSS `transform`. It will replace an author's `transform` when the owed HTML row lands, and it already can on an SVG element that carries `transform`. Charlie ruled that the press composes through CSS's separate `scale` property, as Apple's host already does, and that it is kept under reduced motion.

**Do:**
- Emit the web press as `scale: calc(<row scale> * var(--exact-press))` from `css.rs`, or the equivalent, so it multiplies with the row's `scale` and leaves `transform` alone.
- Make the press hit box undo exactly that factor about `transform-origin` (`input-glue.js:65`, from the review's Grok #8).
- Remove the reduced-motion drop on every host (LLP 1061 D5), and update the schema comment on bit 147 (it still says "about its centre").
- Test: a pressable SVG element with an authored `transform`, and a box with a `scale` row, both keep their geometry while pressed. Test the press under `prefer prefers-reduced-motion reduce`.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

## Implementation and verification (2026-09-27)

- Web: `host/web/src/css.rs`, `src/host.rs`, `src/document.rs`, `index.html`,
  `input-glue.js`, `motion-glue.js`, and the input setup in `glue.js`.
  Two non-inherited CSS numbers hold the authored scale and the eased press
  factor; CSS `scale` multiplies them. Transitions, keyframes and springs
  address the authored number, so they can move during a press. The press
  never writes `transform`. Re-press hit testing neutralizes only the press
  effect during the synchronous measurement, preserving the actual origin,
  SVG transform and ancestor geometry.
- Apple: `PressFeedback.swift` no longer suppresses feedback under reduced
  motion; `DisplayPreferences.swift` documents that policy.
- Linux: `press.rs`, `host.rs`, the painter's presentation/SVG/box code and
  `presenter/contact.rs` now provide eased feedback for held contacts, folded
  into the painted scale. The unpressed hit box survives release/re-press;
  leaving/re-entering, cancellation and gesture takeover release correctly.
  This uses the existing presenter clock and its settle/frame scheduling.
- `kernel/tables/schema.json` bit 147 names `transform-origin` and keeps
  feedback under reduced motion. Updated LLP 1061's implementation account
  and the gallery's reduced-motion explanation.

Regressions first failed on the old implementation: the browser reported
`scale: 1` instead of `0.5`; the macOS reduced-motion test reported a factor
of `1` instead of `0.97`; Linux held its width at `150` instead of `75`.
Tests beside the existing ones now cover SVG transforms, authored scale,
CSS transitions and keyframes, non-centred re-press hit boxes, reduced motion,
Linux cancellation and both Apple presenters. Files: `host/web/tests/press.test.mjs`,
`host/web/tests/it/animation.rs`, `host/web/src/css.rs`,
`host/linux/src/presenter/events_tests.rs`, and both Apple
`PressFeedback{Mac,IOS}Tests.swift` files.

Commands and summary lines:

```text
bun test host/web/tests/press.test.mjs
  1 pass; 0 fail; 26 expect() calls
cargo test -p exact-web -p exact-linux --lib --tests --no-fail-fast
  test result: ok. 434 passed; 0 failed; 1 ignored
  test result: ok. 69 passed; 0 failed; 0 ignored
  test result: ok. 56 passed; 0 failed; 0 ignored
  test result: ok. 34 passed; 0 failed; 0 ignored
  test result: ok. 161 passed; 0 failed; 5 ignored
cargo clippy -p exact-web -p exact-linux --all-targets --keep-going -- -D warnings
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.90s
bun host/apple/build.mjs --ios --test
  Executed 53 tests, with 0 failures (0 unexpected)
  ** TEST SUCCEEDED **
bun host/apple/build.mjs --test
  PressFeedbackMacTests: Executed 6 tests, with 0 failures (0 unexpected)
  Executed 447 tests, with 1 failure (0 unexpected)
cargo fmt --all -- --check
  exit 0
git add -A && bun scripts/caps.mjs
  All budgets within cap. 6 categories inspected.
bun scripts/boot.mjs
  modules reachable before first pixel: 2; wasm references: 8
  Allowed import paths only.
```

Built `apps/interaction-gallery` with `EXACT_APP_DIR` and drove it using
`bun scripts/agent.mjs web --json`: `layout reset`, `tap reset down`,
`tap hold 150`, `layout reset`, `tap up`, then repeated after
`prefer prefers-reduced-motion reduce`. Reset's width was
`46.02 → 44.64 → 46.02` in both cases; its centre stayed fixed. The reduced
motion screenshot shows the new explanation and the held button.

The required root commands were all run: `cargo build --all-targets --keep-going`,
`cargo test --lib --bins --tests --no-fail-fast`, and
`cargo clippy --all-targets --keep-going -- -D warnings`. All three are blocked
by the existing calls in `contract/cli/tests/it/strings.rs:221,223`:
`error[E0061]: this method takes 3 arguments but 2 arguments were supplied`
(`Runner::set_place` needs its third argument). These calls are unchanged
from this branch's starting commit.

The full macOS suite's remaining failure, reproduced on the final run, is
`RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
at line 157: `XCTAssertEqual failed: ("1") is not equal to ("240")`.
No raster-loader files were changed. This ticket needs no further design
ruling; those unrelated verification failures remain for their owning lanes.
