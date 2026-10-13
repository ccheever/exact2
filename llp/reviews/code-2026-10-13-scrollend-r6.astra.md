# Code review: DOM's `scrollend` round 6, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `afefd66bf9e9d7d4c8555dc96ca67bc240d715024af54e88867d9b3fea76075d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** The one MINOR (both reviewers: a negative no-op boot restore on the JS target) taken in the landing commit, as the reviewers suggested, without a seventh round.

---

- **MINOR — [host/web-js/rt.js:240](/Users/admin/x2p2/apps/exact2-comp/host/web-js/rt.js:240): negative no-op boot restores still swallow the first real end.** On an ordinary scroller starting at zero, `scrollTop=-1` clamps to zero, but `at < 0` sets `$bootScrollEnd`. No boot event clears it. A `scrollend`-only handler consequently misses the first user scroll’s end. With both handlers, a single-step scroll loses both events. I reproduced this using HEAD’s actual functions and a clamping test port. Round 5 fixes the positive case only; suppression must account for both bounds and the scrolling direction. [CSSOM clamping rules](https://drafts.csswg.org/cssom-view-1/#scroll-an-element)

The iOS tracking fix looks correct by inspection, including a drag starting during the quiet period. I found no further concrete issue in dispatch, final-scroll ordering, pooling/incarnation checks, or swipe-row initialization. The appended event’s digest matches all three patched fixtures.

AppKit momentum remains unverified with real input. Apple documents that live-scroll completion waits for scroll animations, but the added test posts notifications itself. [Apple release notes](https://developer.apple.com/library/archive/releasenotes/AppKit/RN-AppKitOlderNotes/)

**bsky5’s proposed pattern is served:** set `flinging` on scroll, then clear it and snap on `scrollend`, removing the timer-key writes. Other state writes retained in `scroll` still incur their existing work.

Validation: both cached Contract tests and the cached emitter test passed; in-memory checks confirmed the boot defect and disposal guard. Native suites and the unrelated iOS failures were not rerun. Worktree unchanged.

**NOT READY**
