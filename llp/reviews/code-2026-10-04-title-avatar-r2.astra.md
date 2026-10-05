# Code review: the title view's avatar at its authored size, round 2 (363a31d75 (on f8b2de4da)), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `f8b2de4da`.
- **Method:** one brief (sha256 `7c6fa736b4948ed7a93a0858c75688907079c4d48f7f7ec8b0d69268a5d0b5dc`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken: no background-color resolves to the text colour (currentcolor) before the alpha test. 2 taken differently: the child-count gate is removed (see grok 1), so no children are read. 3 in part: each box's create props are required (`expect`), and a transparent and an unstyled box are covered. DEFERRED: UIKit resize and hide/restore transitions.

---

Round-1 findings **1 and 2 are resolved**: sizing no longer depends on ancestry or depth. **Finding 3 is partly resolved**: assertions now identify the correct node and check fixed-to-auto clearing, but other transitions remain uncovered.

Reviewed statically; no files changed or tests run.

1. **Should-fix — `currentcolor` backgrounds lose authored sizing.** [paragraph.rs:46](/tmp/x12-ra/host/apple/src/paragraph.rs:46). `background_color == None` represents `currentcolor`, not an absent fill. The Apple style projection resolves it against computed `color` ([style.rs:755](/tmp/x12-ra/host/apple/src/style.rs:755)), but `is_some_and` rejects it here. A filled 40×40 title avatar using `background-color="currentcolor"` therefore receives no `faceBoxSize` and renders at the 36-point fallback. Switching from an explicit color to equivalent `currentcolor` also incorrectly clears its size.

   **Fix:** Resolve the background with `node.style.background_color.unwrap_or_else(|| node.text_color())` before checking both appearances’ alpha. Add initial and explicit-color → currentcolor regressions, including inherited color.

2. **Should-fix — Checking child count copies every child.** [paragraph.rs:43](/tmp/x12-ra/host/apple/src/paragraph.rs:43). `NodeRef::children()` allocates a vector and translates every child slot into an ID ([kernel.rs:181](/tmp/x12-ra/kernel/src/kernel.rs:181)). This happens for every created or updated View, before checking fill or dimensions. A touched column containing thousands of rows incurs an additional full child-list allocation even when it cannot qualify; individual avatars also incur avoidable allocations.

   **Fix:** Use the existing borrowed slice: `kernel.arena().children(node.key.index).len()`. This makes the count check constant-time without allocation.

3. **Nit — Transition coverage remains incomplete.** [header_box.rs:49](/tmp/x12-ra/host/apple/tests/it/header_box.rs:49), [NavigationBasicsIOSTests.swift:78](/tmp/x12-ra/host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:78). The Rust test covers boot, deep nesting, an unfilled box, and width becoming auto. It does not establish child-count changes, fill removal, percent dimensions, restoration, size-only changes, or a single-child `when` swap. The UIKit test still covers initial sizing and dark mode, without resizing or hiding/restoring the avatar. Also, `unwrap_or_default()` lets the negative assertion pass if the expected create operation is missing.

   **Fix:** Require the unfilled box’s create props with `expect`; extend the fixture through those transitions and assert node-specific sets/clears, including no redundant size update for a one-for-one swap. Add UIKit size-change and visible → hidden → visible assertions.

The other requested checks look sound:

- **Batch and mirror cost:** `"faceBoxSize":"40x40"` adds about **22 bytes per matching create**, including a separating comma—roughly 22 KB for 1,000 materialized avatars. Each retains another entry in the Rust mirror and Swift props dictionary. Updates rebuild and compare that entry, but unchanged values are **not retransmitted** ([paragraph.rs:335](/tmp/x12-ra/host/apple/src/paragraph.rs:335)). There is no new whole-app diff pass. Timing impact was not measured.
- **Clearing and swaps:** Style changes and child additions/removals touch the box; the host recomputes from the final kernel tree and clears missing props. Second-child, fill-loss, auto, and percent transitions therefore clear correctly. A one-for-one `when` swap preserves qualifying dimensions.
- **Consumers:** The only direct reader is `BadgeFace` with `authored: true` ([NavigationBarIOS.swift:155](/tmp/x12-ra/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:155)). Bar items retain their 36-point path. The prop’s presence alone does not classify ordinary list boxes as title avatars.

Verdict: LAND WITH FIXES
