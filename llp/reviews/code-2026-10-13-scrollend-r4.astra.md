# Code review: DOM's `scrollend` round 4, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `1a4edf9aaca8a8a0ac42fa1a7364acb8c7b96aacb51d4640b59a3d1983da18c2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the Grok CLI's progress lines before it dropped).
- **Verdict:** NOT READY.
- **Disposition:** Every finding taken in round 5's code (with the rebase onto main: kind 51).

---

**MAJOR — Smooth boot restoration drops the end after delivering scrolls.** [host/web-js/rt.js:926](/Users/admin/x2p2/apps/exact2-comp/host/web-js/rt.js:926)

The round-3 fix handles an instantaneous restore, but a nonzero boot offset with `scroll-behavior: smooth` produces multiple `scroll` events followed by one `scrollend`. `onScroll` clears `$bootScroll` after the first event, allowing subsequent movement through; `onScrollend` then discards the completion. This follows the browser’s [smooth-scroll semantics](https://drafts.csswg.org/cssom-view/#scrolling).

Replaying that sequence through the current binders delivered **four scroll callbacks, zero end callbacks, and left `flinging=true`**. An app using the proposed pattern keeps video loading held and misses its header snap until another scroll completes. Restore instantly, or suppress movement and completion consistently throughout restoration.

No further actionable findings. The iOS round-3 guard covers both active `offsetDrivers` and `startOwed`. The presenter ordering, cancellation/reuse guards, Mac observer replacement, swipe-row change, ABI wiring, and appended plan event look consistent; the format digest changes.

**bsky5’s ordinary feed-fling use is served on the implemented hosts:** it can replace the `scrollTick`-keyed `calm`/`snap` tasks with `scrollend`. Existing header-state updates still require work during scrolling. The smooth-restoration case above remains broken.

The two compiler/runner tests and prebuilt JS emission test passed. Native tests were not rerun. AppKit momentum completion remains unverified; the reported unrelated iOS suite failures remain unqualified against main. No files changed.

**NOT READY**
