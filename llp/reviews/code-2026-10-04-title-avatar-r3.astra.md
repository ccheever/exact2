# Code review: the title view's avatar at its authored size, round 3 (final) (5e549f132), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at the round-2 artifacts commit.
- **Method:** one brief (sha256 `5d15797dd7c367e5750a391c1975307273b2a7ed86a39f39e4965b83be773120`), shared with grok. Round 3 of 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** Landed after one more fix, with no further round: 1 taken, so a childless box is never a face and stays a flat leaf. 2 taken: positive cases for a two-child face outside a header and a `currentcolor` fill, plus a negative for a childless dot. DEFERRED: explicit-colour to currentcolor and point-size update transitions.

---

Round-2’s correctness fixes are present: `currentcolor` resolves through inherited text color, multi-child avatars qualify, the child-vector allocation is gone, and missing create props now fail the test. UIKit transition coverage and the protected-region omission remain acceptably deferred for the documented scope. Regression coverage is still partial.

Reviewed statically; no files modified or tests run.

1. **Should-fix — Newly qualifying leaves lose iOS layer flattening.** [paragraph.rs:43](/tmp/x12-ra/host/apple/src/paragraph.rs:43)

   Removing the child gate also gives `faceBoxSize` to childless decorative boxes. [FlatLeavesIOS.swift:76](/tmp/x12-ra/host/apple/Sources/ExactKit/IOS/FlatLeavesIOS.swift:76) allows only `dataset` props, so these boxes now become full `NodeView`s. They also lose adjacent-leaf coalescing into one shape layer.

   This affects existing content: Caltrain’s [station dots](/tmp/x12-ra/apps/caltrain/app.contract:301) gain nine props when its nine-station map is mounted. More generally, **N otherwise eligible filled, fixed-size leaves now require N full views**. The cost exceeds the roughly 22 bytes for each `"faceBoxSize":"40x40",` field. Unchanged values remain suppressed by the mirror diff, but that does not preserve flattening.

   **Fix:** Exclude childless boxes with the allocation-free `kernel.arena().children(node.key.index).is_empty()` check. Keep every nonempty child count eligible; a childless View cannot satisfy `BadgeFace`’s text/symbol requirement. Add a regression proving decorative leaves stay flat while two-child avatars retain their size.

2. **Nit — The regression test still does not exercise either round-3 correctness fix.** [header_box.rs:32](/tmp/x12-ra/host/apple/tests/it/header_box.rs:32)

   The positive fixture remains a one-child box with an explicit color under a header. Reintroducing either round-2 defect would still pass these assertions. The transparent and unstyled cases are useful negatives, but neither exercises `currentcolor`: an omitted declaration defaults to transparent; Rust `None` is the `currentcolor` sentinel.

   **Fix:** Add node-specific positive assertions for a two-child filled box outside a header and a `background-color="currentcolor"` box with inherited color. Exercise explicit-color → currentcolor and a point-size update. UIKit resize/hide/restore coverage can retain its recorded deferral.

Verdict: LAND WITH FIXES
