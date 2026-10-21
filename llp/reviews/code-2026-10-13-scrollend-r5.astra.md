# Code review: DOM's `scrollend` round 5, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `ba7babde117f6c9b577945d8975cfe5e8d11b1a71b9aea3c2a9d5325889faa4a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 6's code.

---

- **MINOR — A no-op boot scroll swallows the first real `scrollend`.** [host/web-js/rt.js:240](/Users/admin/x2p2/apps/exact2-comp/host/web-js/rt.js:240) sets `$bootScrollEnd` before knowing whether the offset moves. For example, restoring `scrollTop=200` while content is empty clamps to zero and produces no event. Once content loads, an end-only handler loses the first real scroll’s end at [rt.js:932](/Users/admin/x2p2/apps/exact2-comp/host/web-js/rt.js:932). Nothing clears the flag during movement without an authored `scroll` handler. Reproduced by executing HEAD’s drain and listeners in memory with a clamping scroller. Suppression must track actual boot movement and work independently of `scroll=`. [CSSOM specifies no end event for unchanged offsets.](https://drafts.csswg.org/cssom-view/#scrolling)

The rebase checks out: HEAD’s parent matches local `origin/main`, ABI kind 51 is consistent, and the patched NativeFields plan digest matches. Both round-4 fixes are present; the JS smooth-restore case passes the listener probe. No further user-visible defect established in native ordering, cancellation, reuse, or swipe-row handling.

Cached binaries passed the two Contract tests, JS emitter test, and three Mac tests. These were not fresh builds; iOS was not rerun. Mac momentum remains unverified: the tests post notifications themselves, and [Apple’s documentation](https://developer.apple.com/documentation/appkit/nsscrollview/didendlivescrollnotification) does not explicitly settle that guarantee.

The proposed bsky5 pattern is supported: set `flinging` on scroll, clear it and snap on `scrollend`, removing `calm`/`snap` restarts. Other per-scroll state writes still need attention to eliminate presenter passes.

**NOT READY**
