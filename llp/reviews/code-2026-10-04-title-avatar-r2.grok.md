# Code review: the title view's avatar at its authored size, round 2 (363a31d75 (on f8b2de4da)), 2026-10-04 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `f8b2de4da`.
- **Method:** one brief (sha256 `7c6fa736b4948ed7a93a0858c75688907079c4d48f7f7ec8b0d69268a5d0b5dc`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the analysis, findings and verdict, unedited (the tool's opening progress sentences are omitted).
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken: any filled, fixed-size box qualifies, so initials beside a status dot keep their size. 2 taken (expect). 3 DEFERRED: the protected native-region projection does not write the key; no title avatar lives in such a region today.

---

**Finding 1 (reparent) is resolved.** `faceBoxSize` is computed from that view's own fill, point width, and point height. Moving the box into or out of a header does not change it, so `SetChildren` no longer has to refresh descendants. `kernel/src/txn.rs:790` still touches the box when its own child list changes, and `kernel/src/txn.rs:693` and `:711` touch it when its style changes.

**Finding 2 (eight-ancestor cap) is resolved.** The walk is gone. The test's avatar sits under the row plus eight columns, so the old loop stopped on the outermost column and never saw the header. That case now gets `40x40`.

**Finding 3 (tests) is partial.** The boot batch is parsed and the prop is checked on the avatar's own id. The unfilled box has none. Turning the width into `auto` emits a clear. Still open: a filled box outside any header, a second child, a lost fill, a `when` swap, a 40-to-32 update, and the deferred UIKit hide/show. Those are finding 2 below.

Clearing does work when the box is updated. A second child, a child destroyed without a replacement, and a `when` that changes the child count all `SetChildren` the box (`kernel/src/txn.rs:779` and `:790`, destroy at `:579`). `update` then omits the key and the existing diff clears it (`host/apple/src/paragraph.rs:341`). Losing the fill, or a width or height that becomes `auto` or a percent, is a `SetStyle` or `ClearStyle` on that same view, and the `Dimension::Points` match fails, so the same clear runs. Percent takes that same arm as the `auto` case the test covers. A `when` that swaps the one child for another child leaves the count at 1 and the points unchanged, so the mirror compares equal and the prop stays `40x40`.

The wire cost is one create-time field. `string_map` writes `"faceBoxSize":"40x40"` (21 bytes with the comma) into that view's create props. `Avatar` in `apps/messages/shapes.contract:118` matches the predicate, so each mounted list avatar pays that once. Later diffs compare the mirror `BTreeMap` (`host/apple/src/paragraph.rs:335`). An unchanged `WxH` is not put in `set` or `clear`, so scrolling does not re-send it. A size or qualification change is one `props` op on that view. Width and height are already omitted from style JSON, so this props op is the size signal, not a second copy of the style.

The only reader is `BadgeFace.init(box:authored:)` when `authored` is true (`host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:66`). Bar items call `BadgeFace(button)` with the default `authored: false` and stay at 36. The key is not a kernel prop, so the agent, web, and Linux trees do not see it. `ChromeIndex` does not index it. On Mac it sits unused in the props dictionary.

1. **Should-fix — A multi-child face still becomes the avatar, then loses its size.** `host/apple/src/paragraph.rs:43`, `host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:142`. Round 1 sent the points for any fixed view inside the header. The new gate requires exactly one kernel child. `BadgeFace` does not: it adopts any filled box that contains a text or a symbol, and the title walk then `continue`s (`NavigationTitleIOS.swift:66`) instead of looking deeper. Initials plus a status dot, or a `display: none` sibling, is still the avatar and now draws at 36. The §9.10 example and the chat fixture have one text child, so they stay correct.

   **Fix:** Drop the `children().len() != 1` check and keep the fill and point-size checks. `BadgeFace` already decides which box is a face. List avatars that already have one child stay as they are. A fixed filled card with several children gains the same 21-byte create field; that is the cost of not sizing a real avatar at 36.

2. **Nit — The test does not lock "the box alone," and a missing op passes as absence.** `host/apple/tests/it/header_box.rs:48`. The avatar is still under a header, so an ancestor walk with a higher cap would still pass. `unwrap_or_default()` at line 49 turns "no props op for `empty`" into a successful absence check.

   **Fix:** Add a filled 40×40 one-child box outside the header and assert `faceBoxSize` on that id. Assert the second-child clear, the clear when the background becomes `#00000000`, a `when` swap of the single text that leaves `40x40` with no clear, and a 40-to-32 update that sets `32x32`. `expect` the empty box's props before checking the key is missing. The UIKit hide/show case can stay deferred.

3. **Nit — The native-region projection never writes the key, and can clear it.** `host/apple/src/paragraph.rs:254`, `host/apple/src/content_region/host.rs:398`. `create` and `update` return before `face_box_size` for a protected id. The candidate rebuilds props with `props_for` only. Promotion diffs that map against `Host.mirror` and clears `faceBoxSize` if it was set. A title avatar in that region falls back to 36.

   **Fix:** Run the same helper when building the candidate props in `content_region/host.rs`, and insert `faceBoxSize` there too.

Verdict: LAND WITH FIXES
