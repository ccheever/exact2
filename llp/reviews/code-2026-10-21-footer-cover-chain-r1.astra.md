# Code review: an absolute footer over a route's bottom-edge scroller rests on the tab bar (kernel cover chain), round 1 of 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `9aa0361b4a`.
- **Method:** one brief (sha256 `bd73c8bfd3fb75db9274cd1abeab4d25654cebe8a659a899b60d8a363dc1f9f7`), blind to the other reviewer; 3 rounds (kernel, theme2's plan approval) then a narrow check. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** changes (3 BLOCKER, 0 SHOULD-FIX, 0 NIT mentions).
- **Disposition:** taken (the round 1 commit).

- **BLOCKER — chain changes can leave route padding stale.** [kernel/src/kernel/cover.rs:266](kernel/src/kernel/cover.rs:266). The search depends on display, positioning, overflow and `Whole`, but ancestor invalidation covers children changes and bottom-Inset transitions only. In `nested()`, apply the scroller’s Inset, then change footer 7 from absolute to relative: the chain now ends at the footer, but the route retains zero bottom padding. `style_changed` restyles only the changed node; nested `Whole` toggles similarly restyle only their immediate parent. **Fix:** invalidate the nearest relevant `Edges` ancestor when these predicates change, considering both old and new states. Test these transitions with the Inset unchanged.

- **BLOCKER — restored covers undercount fitted sheets.** [NavigationCoversIOS.swift:124](host/apple/Sources/ExactKit/IOS/NavigationCoversIOS.swift:124), consumed by [layout.rs:522](host/apple/src/layout.rs:522). A `fit-content` sheet with an edge scroller now retains its bottom cover while its route contributes no bottom padding. `fit_content_height` also removes the scroller’s Inset from measurement, but `emit_fitted` still subtracts the full route cover. With a 34-point cover, it reports content height minus 34, producing an undersized sheet. **Fix:** subtract only the cover padding actually included in the fitted measurement. Add a host test combining route `Edges`, descendant `Inset` and `fit-content`.

- **BLOCKER — the downward walk crosses the boundary Q2’s invalidation respects.** [kernel/src/kernel/cover.rs:281](kernel/src/kernel/cover.rs:281). Using the new Q2 fixture, change sheet 2’s left padding after installing scroller 6’s Inset. Restyling the sheet descends through route 3’s `Edges`, finds the same Inset, and incorrectly removes the sheet’s 34-point bottom padding. The existing test passes because the outer sheet’s cached style survives. **Fix:** prevent an Inset from satisfying an `Edges` ancestor beyond its nearest covered ancestor; stop children-change propagation at that boundary too. Extend Q2 with an unrelated outer-sheet restyle and reversed cover-installation order.

Destruction and replacement reach `children_changed` through detach cleanup. The added ancestor search costs O(depth) per children change, stopping inside scrollers; performance remains unmeasured. I found no separate issue with `PinnedRoom`’s keyboard guard, tvOS or Mac.

The new kernel/iOS tests distinguish the main fix, but miss the cases above. The Linux test would also pass before this commit—it checks preserved behavior.

Source review only; tests were not run in this read-only worktree.
