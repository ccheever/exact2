# A hatch cannot run at a fixed period across a list's rows: `repeating-linear-gradient()`, length stops and `background-attachment: local` are refused

**Status:** Closed
**Resolution:** repeating-linear-gradient and px stops land on every host, held to Chrome (UIKit included); background-attachment: local stays deferred by ruling 9
**Systems:** kernel gradient, Contract background-image and background-attachment, every host
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-11
**Related:** LLP 1066 D2 (the grammar it refuses: `repeating-*`, length stop positions) and D7 (`local` refused); LLP 1077 ("Still refused: `repeating-*`"); `QUEUE.md` "The document window, what the studio pass left" (repeating gradients and length stops refused everywhere); https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/EXACT2-GAPS.md ("Split view's hatched empty side"); T3 PR #427

## Summary

CSS hatches a region with a repeating gradient: 45° stripes 2px wide every 8px are
`repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)`. Contract refuses that value
(`kernel/src/gradient.rs:573` `REFUSED`), refuses the same stripes written as a plain `linear-gradient()` with length
stop positions (`gradient.rs:928`: "a stop's position is a percentage; lengths are not implemented"), and refuses
`background-attachment: local` (schema bit 171 admits `scroll` and `fixed`).

So an author cannot paint one hatch at a fixed period under a run of rows of a virtualized list. Each row can draw its
own stripes (an SVG `pattern` builds and paints), but each row's pattern starts at its own top. Unless the period
divides every row's height, the stripes break at every row edge. A per-row phase would need each row's offset in the
list kept current as the rows above it mount, wrap and resize; `resize=fit` hears a box's own size, not its moves, so
with rows that wrap to different numbers of lines no phase stays right. In Chrome the same rows are hatched by one pattern on
the scroller with `background-attachment: local` (or by one element over the run when the rows are not virtualized):
the stripes run unbroken across rows of any height and move with them as the list scrolls.

## Why this arose

The T3 Code desktop app (`1e2ecbd975`, with @pierre/diffs 1.3.0-beta.10) hatches the empty side of an added or deleted
run in its split diff view: one `[data-content-buffer]` element over the run, `repeating-linear-gradient(-45deg,
transparent 0 4.242px, <buffer> 4.242px 5.656px)` in 8px tiles (`#e6e6e6` / `#1d1d1d` over `#fcfcfc` / `#0a0a0a`). The
clone draws its diff as a virtualized list with one item per line (a new file's split view is one run of the whole
file, so a run cannot be one item). It draws the stripes as an SVG pattern per row at a 10pt period, which divides the
20pt line, so the stripes stay unbroken across rows and wrapped lines, where the reference's period is 8 (T3 PR #427,
task files-gutter-parity FG-3; declared in the clone's `EXACT2-GAPS.md`).

## Reproduction

Main `2b5e4a7dc01c2328a3407ab3b8744a4a6fbbe6d6`, `bun scripts/exact.mjs contract build <file>` on each one-file
contract (all four and their output are in the record below):

```text
// A: CSS's own hatch, 45° stripes 2px wide every 8px (5.657px along the gradient line).
component Hatch
  view
    column width=320 height=200 background-color="#fcfcfc" background-image="repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)"
```

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| A: `repeating-linear-gradient()` as above | `contract build`, macOS 26.6.2 | main `2b5e4a7dc` | exit 1: `[lower-attr-value] … repeating-linear-gradient() is not implemented; a gradient paints once` | builds; every host paints Chrome's stripes | record |
| B: the same stripes as `linear-gradient(-45deg, transparent 0px 4.243px, #e6e6e6 4.243px 5.657px)` | `contract build` | main `2b5e4a7dc` | exit 1: `a stop's position is a percentage; lengths are not implemented` | builds and paints as Chrome does (no hatch: past the last stop the end colour fills the box) | record |
| C: a `list virtualized=true` with `background-attachment="local"` | `contract build` | main `2b5e4a7dc` | exit 1: `` `background-attachment="local"` is not a valid `background-attachment`: expected one of "scroll", "fixed" `` | builds; the list's background scrolls with its rows | record |
| D: today's way, an 8px SVG `pattern` in each 20px row of a virtualized list | `contract build` | main `2b5e4a7dc` | builds (8 nodes); each row's stripes start at the row's top, so they break at every row edge | (the limit this issue is about) | record |
| The oracle: A as each 20px row's own background, and as one background on the scroller with `background-attachment: local`, scrolled 13px, one row two lines tall | Chrome 155.0.8059.39, headless, 2x | — | per row: broken at every row edge; on the scroller: unbroken across every row, and it moves with them | (the reference) | image |

![Chrome: a per-row 8px hatch breaks at every 20px row edge; one hatch on the scroller with background-attachment: local runs across them](https://raw.githubusercontent.com/ccheever/exact2/22a6f95318afd6806d86b42fa1cfd231f1eb93dd/fw-issues-20261011m/hatch-chrome.png)

Record (the four contracts, the commands and output, and the Chrome page):
https://raw.githubusercontent.com/ccheever/exact2/fc72646d868267355a042910f991fd2c718231ee/fw-issues-20261011m/hatch-record.txt

Not run: painting on any host (each value fails the build first); Linux, Windows, iOS.

## Constraints

- LLP 1066 D2 refuses rather than approximates: "A host that paints something else than Chrome is a parity bug that
  nothing reports." So a repeating gradient, once admitted, is held to Chrome's pixels on every host, as LLP 1066's
  twelve-case page holds the others (LLP 1055.000 notes Core Graphics and Skia antialias a hard-edged repeating
  gradient differently).
- LLP 1066 D7 refused `local` "since it is the same as `scroll` for a box whose own content does not scroll under its
  background". That holds for a box that does not scroll; on a scroller (a `scroll`, a `list`) `local` is what CSS
  writes for a background that moves with the content.
- `background-attachment: fixed` with a repeating gradient would join the rows too, but its stripes stay still while
  the rows scroll under them, where the browser's (and the app's reference's) move with the rows.
- A hypothesis, not a requirement: a per-row form would do as well if a row could place its background in its list's
  content coordinates, but CSS has no such property; `local` on the scroller is CSS's own way.
- No open or closed issue on main covers repeating gradients, length stops or `local`, and neither do the GitHub
  issues (searched `gradient`, `repeating`, `stripe`, `hatch`, `background-attachment`).

## Acceptance criteria

- A builds, and on the web, macOS and iOS its stripes match Chrome's within LLP 1066's band (a repeating case and a
  length-stop case on the gradients parity page).
- B builds and paints as Chrome paints it.
- C builds, and a hatch on a virtualized list with `background-attachment: local` runs unbroken across rows 20px and
  40px tall and moves with them as the list scrolls, on the web and macOS, as in the image's right half.
- What stays refused (`url()`, `image-set()`, `cross-fade()`) keeps its message.

## Approved first slice (2026-10-12)

Charlie's issue-program ruling 9 admits `repeating-linear-gradient()` and length
stops held to Chrome, and defers `background-attachment: local`. Acceptance
criterion C remains deferred; this change does not make a virtualized list's
background move with its rows.

The gradients lane implements scenarios A and B with px length stops, including
mixed px/% lists, omitted positions and double stops. Native stop placement is
resolved against the painted box again after resizing; AppKit/UIKit share that
resolver and Linux uses it before painting. The GPU painter uses one period with
native repetition and compensates for Vello's integer-coordinate sampling to
sample CSS pixel centres. LLP 1066's dated amendment and LLP 1001 record the
remaining grammar and microscopic-period limits. The agent contract and
`contract vocab background-image` describe the admitted spelling.

Tests added:

- Kernel `repeating_hatch_and_length_stops_round_trip`, observed failing with the
  original repeating-gradient refusal before implementation.
- Kernel `length_fix_up_uses_the_painted_size_and_repetition_keeps_its_period`:
  mixed positions, resize, stable period and cycle boundaries; the zero-period
  expectation is corrected by review round 1 below.
- Apple Rust `lengths_cross_the_apple_seam_after_the_box_is_known`.
- Three stock-Chrome pixel cases on the shared Apple/Linux parity page and an
  Apple length-resize check.
- Web JS `gradients` conformance fixture: computed period, resize and mask.

The issue stays open. What remains:

- UIKit parity and resize tests need a working simulator. Three boot attempts
  stalled (iOS 26.4 migration and iOS 27 startup); no UIKit tests ran. Every device
  booted by this lane was shut down; its temporary device was deleted.
- `background-attachment: local` needs a later ruling admitting it.

The Chrome agent drive passes both appearance checks. Inspected captures show
width changing from 120 to 240px and hatch period from 8 to 12px, while the
mixed-stop background and repeating mask track the box. Its final state is
`size=240, period=12`.

Linux's third pixel run passes all 15 cases in both painters and both schemes;
the reported hatch is pixel-exact in CPU and GPU output. The macOS fixture app
loaded all 17 nodes, tapping `dark` changed state and native scheme, and the
390×555-point screenshot was inspected. This verifies the runtime bridge, not
the still-failing AppKit pixel band.

Verification limitations: the first broad Rust run and an isolated rerun failed
an existing image-layout test (`max-width=300`, observed 0×0 after its three-second
image wait, expected 300×150); the second broad run passed it. That second run
hit a 30-second navigation timeout in the existing web paint-order corpus; the
isolated web rerun passed. The full AppKit suite also failed context-menu,
reorder-group, text-selection and text-metrics assertions whose source this lane
does not change. `contract-difftest quick` could not run its oracle because
`lake` is missing; installing Lean via a remote installer is outside this lane's
no-remote-commands constraint.

### Verification commands

Commands below ran in this worktree. Logs and captures are under the ignored
`target/gradients-*` paths; no check of another worktree or workspace-wide Cargo
check was run.

| Command | Result |
|---|---|
| `bun install --frozen-lockfile` | Pass. |
| `cargo test -p exact-kernel --lib repeating_hatch_and_length_stops_round_trip -- --nocapture` | Expected failure before the implementation: repeating gradients refused. |
| `cargo test -p exact-kernel --lib gradient::tests -- --nocapture` | Pass, 15 tests. |
| `cargo test -p exact-apple --lib gradient -- --nocapture` | Pass, 5 tests. |
| `cargo test -p exact-kernel -p exact-apple -p exact-linux -p contract -p exact-web-js --lib --bins --tests --no-fail-fast` | Two runs failed: first image layout and GPU hatch; second GPU hatch and web paint navigation timeout. Kernel, Apple and Contract tests passed. Both runs preceded the final GPU sampling correction. |
| `target/debug/deps/it-a61745a9320b45d0 image::sampled_pixels_keep_natural_auto_axes_and_min_max_layout --exact --nocapture` | Failed image wait; subsequently passed in the second broad run. |
| `target/debug/deps/exact_web_js-c63e69cbeaa60eb9 paint_tests::server_client_and_adopted_paint_agree_on_the_chrome_corpus --exact --nocapture` | Pass on isolated rerun. |
| `target/debug/deps/pinned-aa9e6c0ed35bc96b gradients::every_gradient_case_matches_chrome_light_then_dark --exact --nocapture` | Pass after the third GPU fix; 15 cases, CPU/GPU, light/dark. |
| `cargo clippy -p exact-kernel -p exact-apple -p exact-linux -p contract -p exact-web-js --all-targets -- -D warnings` | Pass before the final GPU sampling correction; final Linux-only check follows. |
| `cargo run -p contract-difftest -- quick` | Blocked: Rust build passed, oracle failed because `lake` is unavailable. |
| `target/debug/contract build scripts/fixtures/gradients.contract -o target/gradients.plan` | Pass, 17 nodes. |
| `target/debug/contract vocab background-image` | Pass. |
| `bun host/apple/build.mjs --test` | Failed AppKit parity (2.7846/255) and the other assertions listed above. |
| `bun host/apple/build.mjs` and `bun host/apple/build.mjs caltrain-apple` | Pass; the latter refreshed the signed app after a stale-receipt drive refusal. |
| `bun scripts/agent.mjs macos --plan target/gradients.plan --size 390x555 tree "tap dark" state logs "screenshot target/gradients-macos.png"` | Pass; screenshot inspected. An earlier drive including `check both` stopped because macOS does not implement that operation. |
| `bun host/web-js/conform.mjs --synthetic --only gradients --build --strict --wasm-root target/gradients-wasm --out target/gradients-conformance` | Failed before comparison: missing WASM dist. |
| `EXACT_WEB_LINK=all EXACT_WEB_DIST=/Users/ccheever/projects/exact2-iss-gradients/target/gradients-wasm/caltrain bun host/web/build.mjs caltrain-web --wasm` | Canceled during the redundant Caltrain server-renderer build in favor of the direct JS fixture drive. |
| `cargo test -p exact-linux --lib --tests --no-fail-fast` | Pass after the final GPU correction: 737 unit, 103 integration, 78 pinned pixel and 1 additional test. |
| `cargo fmt --all -- --check` | Pass on the final source. |
| `git add -A && bun scripts/caps.mjs` | Pass. |
| `bun scripts/issue.mjs check` | Pass; issue remains open. |
| `cargo clippy -p exact-linux --all-targets -- -D warnings` | Pass on the final GPU source. |
| `bun host/web-js/build.mjs caltrain --plan target/gradients-conformance/synthetic-gradients.plan --out target/gradients-js --render none` | Canceled during the cold Caltrain data-module build; the self-contained fixture uses the existing `hello` host instead. |
| `bun host/web-js/build.mjs hello --plan target/gradients-conformance/synthetic-gradients.plan --out target/gradients-js --render none` | Pass, zero compiler warnings. Direct output lacks the complete-build receipt, so the first agent invocation refused it. |
| `EXACT_APP_DIR=/Users/ccheever/projects/exact2-iss-gradients/apps/hello EXACT_WEB_DIST=/Users/ccheever/projects/exact2-iss-gradients/target/gradients-js bun host/web/build.mjs hello-web` | Pass; produces the authenticated base build. |
| `EXACT_WEB_DIST=/Users/ccheever/projects/exact2-iss-gradients/target/gradients-js bun scripts/agent.mjs web --app hello --plan target/gradients-conformance/synthetic-gradients.plan --size 420x555 tree "check both" "screenshot target/gradients-web-before.png" "tap change" tree state logs "screenshot target/gradients-web.png"` | Pass; both appearances have no findings, state changes to width 240/period 12, both screenshots inspected. |

## c1 continuation (2026-10-12)

The remaining AppKit error is hard-stop position quantization inside Core
Graphics’ expanded axial ramp. A direct Core Graphics reproduction has exactly
the captured test’s 2.784571/255 mean error: 886 of 7,000 pixels differ, each
by 22, and every pixel is either the authored #e6e6e6 or background #fcfcfc.
The diff follows stripe boundaries throughout the box, with alternating
narrow and wide stripes. There is no blended edge colour, uniform phase
shift or pattern-tile seam. Drawing the continuous runs with separate gradient
endpoints preserves the geometry and colour space and makes that reproduction
pixel-exact against the Chrome fixture. The shared Swift painter now splits
axial ramps at internal hard stops.

Diagnostics and verification logs for this continuation are under
`target/gradients-c1-*`. The full wasm `conform --build` run is intentionally
skipped per the orchestrator’s instruction: its dist build does not finish
under the current machine load.

The focused AppKit class now passes all three tests. Its captured hatch is
pixel-exact in both appearances, within the unchanged 2/255 band; its on-screen
Retina capture downsampled to 1× is 2.13/255, within that test’s unchanged
3/255 band. All 15 gradient cases, the length resize and the border cases pass.
The first continuation run fixed captured pixels but exposed the on-screen
Core Animation ramp at 3.13295/255; repeating backgrounds and masks now use
the corrected drawn painter. The temporary class filter and pixel dump were
restored before committing. No additional permanent test was needed: the
first slice’s Chrome hatch regression failed before this fix and passes after it.

UIKit remains unverified. `simctl boot` for the previously shut-down iPhone 18
Pro Max (793D239B-0A6E-48CB-9C12-E87C729207EB) timed out after 60 seconds.
The first bounded shutdown also timed out; a later shutdown and `simctl list`
confirmed this device is Shutdown. No other simulator was booted or stopped.

### c1 verification commands

The test bundle below is
`target/clients/8acfb403880225d04837dabc/com.exact.caltrain/macos/aarch64-apple-darwin/embedded/development/tests/out/Products/Debug/ExactKitTests.xctest`.

| Command | Result |
|---|---|
| `xcrun xctest -XCTest ExactKitTests.BorderParityMacTests <bundle>` | Baseline failed: the screen capture helper force-unwrapped a missing window-server image, before reaching the gradient test. |
| `xcrun xctest -XCTest ExactKitTests.BorderParityMacTests/testEveryGradientCaseMatchesChrome <bundle>` | Baseline expected failure: hatch 2.784571/255 in both appearances. |
| `ONLY_CLASSES=ExactKitTests.BorderParityMacTests bun host/apple/build.mjs --test` | First fix run: captured gradients passed at 0/255; on-screen hatch failed at 3.13295/255. Second fix run: all 3 tests passed, including on-screen. Temporary filter restored afterward. |
| `cargo fmt --all -- --check` | Pass. No Rust source changed in c1. |
| `git add -A && bun scripts/caps.mjs` | Pass, all 7 cap categories. |
| `bun scripts/issue.mjs check` | Pass; issue remains open. |
| `xcrun simctl boot 793D239B-0A6E-48CB-9C12-E87C729207EB` | Timed out after 60 seconds; UIKit unverified. |
| `xcrun simctl shutdown 793D239B-0A6E-48CB-9C12-E87C729207EB` and `xcrun simctl list devices available -j` | First shutdown timed out; final shutdown reported already Shutdown and final list confirmed Shutdown. |
| `bun host/apple/build.mjs caltrain-apple` | Pass, signed development app rebuilt. |
| `bun scripts/agent.mjs macos --plan target/gradients.plan --size 390x555 tree "screenshot target/gradients-c1-macos-light.png" "tap dark" state logs "screenshot target/gradients-c1-macos.png"` | Pass: 17 nodes, native scheme changes to dark, both 390×555-point captures inspected. |

No new Rust, web or Linux source changed in c1; their earlier lane results are
recorded above. UIKit and the wasm conformance build were not run in c1 for
the reasons stated above. The issue stays open for UIKit qualification and the
deferred `background-attachment: local` scope.


## Review fix round 1 (2026-10-12)

Reviewer A found a real zero-period parity defect, but the proposed equal-weight
average is not what Chrome paints. Stock Chrome 155.0.8059.39 paints the last
stop in each requested case, in both appearances:

| Stops in `repeating-linear-gradient(to right, …)` | Chrome |
|---|---|
| `red 0px, blue 0px` | `#0000ff` |
| `red 0px, lime 0px, blue 0px` | `#0000ff` |
| `red 0px, lime 0px, blue 0px, white 0px` | `#ffffff` |

Every pixel of each 100×70 box has that colour. Thus neither the former
`#408040` nor the suggested `#555555` matches Chrome for the three-stop case.
The kernel now resolves a zero period to its last stop. Positive microscopic
periods keep their existing average and cutoff. The agent guide, LLP 1066 and
LLP 1001 describe the distinction.

The shared Chrome fixture adds `zero-two`, `zero-three` and `zero-four`, with
light/dark reference images and cases in both native parity tests. The kernel
adds `zero_period_repetition_matches_chromes_last_stop`. Before the fix, that
unit test failed and all three cases failed on Linux CPU/GPU and AppKit in both
appearances (mean errors 85, 127.67 and 141.67/255 respectively). After the fix, the new cases match Chrome pixel-for-pixel on Linux CPU/GPU
and in AppKit’s captured-gradient test, in both appearances; all 18 gradient
cases pass those tests. No parity band was changed. Logs are `target/gradients-r1-*`.

UIKit is not rerun in this targeted review round; its qualification remains
pending as recorded above. The issue stays open for that and the deferred
`background-attachment: local` scope.

### Round 1 verification commands

`bundle` below is
`target/clients/8acfb403880225d04837dabc/com.exact.caltrain/macos/aarch64-apple-darwin/embedded/development/tests/out/Products/Debug/ExactKitTests.xctest`.
The Apple build uses a temporary `macTests` class filter selecting only
`ExactKitTests.BorderParityMacTests`; that filter is restored and is not part
of the commit. No workspace-wide or full-host test suite ran in this round.

| Command | Result |
|---|---|
| `bun target/gradients-oracle.mjs` | Pass; stock Chrome 155.0.8059.39 captures all 18 cases in both schemes. Pixel inspection confirms blue, blue, white throughout the three new boxes. |
| `cargo test -p exact-kernel --lib zero_period_repetition_matches_chromes_last_stop -- --nocapture` | Expected failure before the fix. |
| `cargo test -p exact-kernel --lib gradient::tests -- --nocapture` | Pass after the fix: 16 tests. |
| `cargo test -p exact-linux --test pinned gradients::every_gradient_case_matches_chrome_light_then_dark -- --exact --nocapture` | Expected failure before the fix; pass after: all 18 cases, CPU/GPU, light/dark. New cases are pixel-exact. |
| `xcrun xctest -XCTest ExactKitTests.BorderParityMacTests/testEveryGradientCaseMatchesChrome "$bundle"` | Expected failure against the pre-fix archive; pass after rebuilding, all 18 cases in both appearances and length resize. New cases are pixel-exact. |
| `bun host/apple/build.mjs --test` (temporary class filter above) | Build passed; border test passed, then the on-screen test crashed at `BorderParityMacTests.swift:92` when the window-server capture returned nil, as in the c1 baseline. The captured-gradient test was run directly afterward. |
| `cargo clippy -p exact-kernel -p exact-linux --all-targets -- -D warnings` | Pass. |
| `cargo fmt --all -- --check` | Pass. |
| `target/debug/contract build scripts/fixtures/gradients.contract -o target/gradients-r1.plan` | Pass; existing fixture warnings about authored colours and pre-action reads. |
| `xcrun xctest -XCTest ExactKitTests.BorderParityMacTests/testEveryCaseMatchesChromeOnScreen "$bundle"` | Focused retry also crashed at the same nil-capture unwrap; on-screen parity remains unverified in this round. |
| `bun host/apple/build.mjs caltrain-apple` | Pass; signed development app rebuilt. |
| `bun scripts/agent.mjs macos --plan target/gradients-r1.plan --size 390x650 tree "screenshot target/gradients-r1-macos-light.png" "tap dark" state logs "screenshot target/gradients-r1-macos-dark.png"` | Pass; 20 nodes, scheme changes to dark, both captures inspected with blue/blue/white zero-period boxes. |

## 2026-10-12 UIKit verification (orchestrator)

`BorderParityIOSTests` ran on an iOS 27 simulator (iPhone Air) through
`build.mjs --test --ios` filtered to that class: 2 tests, 0 failures. With
AppKit, Linux and the web already held to Chrome, every host is qualified.
`background-attachment: local` stays deferred by ruling 9 (rules/DEFERRED.md).
