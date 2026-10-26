# Code review: Apple pass #2: an outline brings the selection's own copy into view, round 1 of 2, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `1a34c46a46`.
- **Method:** one brief (sha256 `f18752bca9121cddec7b80b66ee369f6d1889ad10c21bd22c5fc3af6bf945129`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

No BLOCKER found.

- **SHOULD — [inspect-dev.js:501](host/web-js/inspect-dev.js:501), [dev.js:827](host/web/dev.js:827):** Both web paths still reveal a connected selection hidden with `visibility: hidden`. It remains eligible and receives `scrollIntoView`; its retained layout box permits scrolling under the [CSSOM algorithm](https://drafts.csswg.org/cssom-view/#dom-element-scrollintoview). Check visibility before revealing, preserving the no-fallback behavior. The removal-only test misses this case.

- **SHOULD — [tamber-select-reveal.test.mjs:62](scripts/tamber-select-reveal.test.mjs:62):** The fixed 400 ms sleep does not establish outline completion. Receiving `selection` or an HTTP acknowledgement precedes application of the outline. Under load, unchanged-scroll assertions can pass prematurely and positive assertions can fail spuriously. Wait for the particular outline to be applied before checking geometry.

- **NIT — [InspectRevealingTests.swift:22](host/apple/tests/ExactKitTests/InspectRevealingTests.swift:22):** The duplicate-node-ID case selects session `one`, whose matching copy appears first. Removing the session comparison would still pass every assertion. Select session `two` and expect `b9`; also hide/remove `b9` while `a9` remains shown and expect nil.

- **NIT — [tamber-select-reveal.test.mjs:107](scripts/tamber-select-reveal.test.mjs:107):** Accepting either card does not verify the promised first-copy fallback. Assert the first candidate in each page’s actual outline order.

Otherwise, Apple identity guards, iOS delegation, bar-placeholder handling, move-site inclusion, and the JS reload restore’s `el: null` fallback look correct.

Ran extracted web outline functions in memory. Full XCTest/browser suites were not run under the read-only filesystem restriction.
