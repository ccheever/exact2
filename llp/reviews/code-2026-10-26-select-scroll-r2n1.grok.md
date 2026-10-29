# Code review: Apple pass #2: an outline brings the selection's own copy into view, narrow re-check 1, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `07363f4990`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**CLEAN.** `cssVisibilityHidden` is the node's computed, inherited `visibility`, so an ancestor's `hidden` is already on the node the reveal predicate reads.

`CssVisibility.swift:13` is `style["visibility"]?.string == "hidden"`, and an absent key is the initial `visible`. That dictionary is the computed style the host sends, not the authored row. `visibility` is inherited (`kernel/tables/schema.json`), and `style_json_for` resolves it: a child that does not set it under a `hidden` parent is sent `"hidden"`, a child that sets `visible` stays `"visible"`, and the initial `visible` is omitted (`host/apple/src/style.rs:975-984`, `style/flow_tests.rs` `a_box_inherits_visibility`). An inherited change touches those descendants and `update` re-sends the full style, which `applyStyle` replaces. A descendant that computes `visible` still paints and stays a reveal target. One that inherits `hidden` does not.

The predicate matches that. iOS passes `shown($0) && !$0.cssVisibilityHidden` (`InspectIOS.swift:104`); Mac passes `!$0.isHiddenOrHasHiddenAncestor && !$0.cssVisibilityHidden` (`InspectMac.swift:245`). `display: none` still goes through `isHidden` / `isHiddenOrHasHiddenAncestor`. Outline marks and `place()` are untouched, so a `visibility: hidden` box is still outlined where it sits. `Inspect.revealing` still returns nil for a held copy that is not shown, and does not fall through to another copy.

The Mac test writes `style["visibility"] = .string("hidden")` on the held glyph, leaves `isHidden` false, and expects the scroll offset to stay put (`InspectRevealMacTests.swift:131-135`), which is the path `cssVisibilityHidden` reads. It then clears the key before the no-selection case.
