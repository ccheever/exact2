# Code review: the title view's avatar at its authored size (c62beb1e5), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `c62beb1e5`.
- **Method:** one brief (sha256 `d2905b58ba6cb51d0c48a8a66fcda4ff9f52cb7698ee549fa278f4592225af8f`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** Findings 1 and 2 are taken by removing the ancestry: `faceBoxSize` is read from the box alone (a filled, fixed-size box with one child), so moves and depth need no recomputation. Finding 3 is taken in part: the test parses the batch, checks the avatar's own props, none on an unfilled box, and the clear when the width stops being points. DEFERRED: a UIKit hidden-to-visible avatar transition test.

---

Reviewed both commit messages and `HEAD~2..HEAD` statically. No files changed or tests run.

1. **Should-fix — Reparenting does not refresh ancestry-dependent metadata.** [paragraph.rs:327](/tmp/x12-ra/host/apple/src/paragraph.rs:327), [paragraph.rs:385](/tmp/x12-ra/host/apple/src/paragraph.rs:385). A retained 40-point box moved into a header can keep no `headerBoxSize`, so the title still draws it at 36. Moving it out can leave the prop behind. `SetChildren` touches the parents; it touches moved descendants only when inherited styles change ([txn.rs:1073](/tmp/x12-ra/kernel/src/txn.rs:1073)). Queuing their layout does not regenerate these props. This also affects wrapping existing children in a newly created header.

   **Fix:** Recompute and diff `headerBoxSize` for affected retained subtrees after topology changes, including detached subtrees, using final kernel ancestry. Add regressions for moves into/out of headers, moving a wrapper, and a header introduced in a later commit.

2. **Should-fix — The eight-ancestor cutoff silently loses supported avatars’ sizes.** [paragraph.rs:49](/tmp/x12-ra/host/apple/src/paragraph.rs:49). Title discovery recursively accepts deeper descendants without this limit ([NavigationTitleIOS.swift:31](/tmp/x12-ra/host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:31)). With the header nine ancestors away, a valid authored 40-point avatar therefore falls back to 36. Adding an otherwise harmless wrapper changes its native size.

   **Fix:** Remove the arbitrary cutoff, or eliminate the ancestry dependency by projecting fixed box dimensions directly. Add a fixture crossing the eight/nine-ancestor boundary.

3. **Nit — The tests do not establish update/removal correctness.** [header_box.rs:28](/tmp/x12-ra/host/apple/tests/it/header_box.rs:28). Counting one matching prop anywhere and separately finding the avatar’s ID would also pass if the prop were attached to the wrong node. The UIKit addition covers initial size and dark-mode bounds, but not resizing or hiding/restoring the avatar.

   **Fix:** Parse the batch and assert the avatar’s exact props and their absence on `elsewhere`. Add size-only updates, fixed-to-auto clearing, class/viewport-driven changes, and UIKit visible → hidden → visible transitions.

The remaining checks look sound statically:

- **Cost:** Only Views with two point dimensions walk ancestors, with at most eight ancestor lookups and string allocation on a match. This is bounded additional work; measurable impact cannot be established without profiling.
- **Creation and styles:** Same-batch child-before-header creation is safe because projection reads the final kernel tree. Class and viewport-dependent size changes emit style operations and reach `update`; its existing prop diff clears metadata when dimensions cease qualifying.
- **Isolation:** Metadata stays in the Apple presenter projection. macOS receives unused metadata; web/Linux and the kernel-backed agent tree do not receive it.
- **Equality and constraints:** `size` participates in synthesized `BadgeFace` equality and `source`. Fixed image constraints override the dark image’s intrinsic size; zero constants are compatible with a hidden arranged subview. Bar items retain their default 36-point path and do not use these title-view constraints.

Verdict: LAND WITH FIXES
