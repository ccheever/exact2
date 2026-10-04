# Code review: aria-modal (f8c924003), 2026-10-03 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` a detached worktree at `f8c924003`.
- **Method:** one brief (sha256 `c8c7f32b67a8f605b0dc6450fb745e6fd744dd00891b32733b38502eb171e017`), shared with grok. Round 1, blind to the other review. The coordinator asked for it on Charlie's Signal Clone work. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):**
  - 1 (macOS cells and scroll parents), 4 (macOS `modal` unreported) and 5 (macOS paint order): taken by deferring macOS. The filter is removed, and LLP 1080.003 D3 records what a correct version needs.
  - 2 (an exiting modal): taken. `beginExit` clears the property, and `syncModal` skips accessibility-hidden views.
  - 3 (notifications track the prop rather than presentation): taken. `syncModal` runs after each batch, decides modality from exposure, and posts `.screenChanged` when the set changes, removal and `display` included. The test covers each case.

---

Static review only; no files changed or tests run.

1. **Should-fix — macOS can remove controls inside the modal.**  
   [NodeViewMac.swift:343](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:343) filters an already flattened accessibility tree using view ancestry. Native control cells are neither `NSView` nor `NSAccessibilityElement`, so `inside()` rejects them. AppKit explicitly exposes some controls through their cells. [Apple accessibility hierarchy documentation](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Accessibility/cocoaAXOverview/cocoaAXOverview.html).

   A scrolling parent has another failure: its accessibility child can be the `NSScrollView` containing the modal, which the descendant-only predicate also rejects. **Fix:** obtain the unignored accessibility subtree from the selected modal, preserving native participants and any necessary wrapper structure. Extend [AccessibilityTreeMacTests.swift:68](/tmp/x9-review/host/apple/tests/ExactKitTests/AccessibilityTreeMacTests.swift:68) with a text field, native checkbox, and scrolling parent; its current custom button misses these cases.

2. **Should-fix — an exiting modal continues suppressing live siblings.**  
   The selection at [NodeViewMac.swift:347](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:347) checks only the prop and `isHidden`. [PresenceMac.swift:27](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/PresenceMac.swift:27) retains the exiting view onscreen with its props, marks it accessibility-hidden, and moves it above its siblings. It therefore remains the selected modal throughout the exit, suppressing the content that should become accessible.

   iOS likewise retains `accessibilityViewIsModal` in [PresenceIOS.swift:26](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/PresenceIOS.swift:26); the agent’s sibling selector still chooses that view before its hidden subtree is skipped. **Fix:** retire effective modality when exit starts, and exclude accessibility-hidden or retired modal candidates. Test the tree during an exit, before destruction.

3. **Should-fix — iOS notifications track prop changes rather than presentation changes.**  
   [NodeViewIOS.swift:1020](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1020) posts when a hidden or unattached view receives `true`, but showing that same view through `display` changes posts nothing. Removing a conditionally mounted modal also bypasses this code. Initial creation calls it before attachment and child mounting in [PresenterIOS.swift:742](/tmp/x9-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:742).

   Consequently, [LLP 1080.003:25](/tmp/x9-review/llp/1080.003-authored-modality.rfc.md:25) overstates the guaranteed focus handoff. **Fix:** coalesce notifications after effective modal presentation changes and mounting completes, with an exposed focus target or `nil`. Cover mount, removal, visibility toggles, and unchanged-value application. The existing equality guard correctly prevents repeated identical prop writes from posting.

4. **Should-fix — the macOS agent never reports authored modality.**  
   [AgentAccessibility.swift:224](/tmp/x9-review/host/apple/Sources/ExactKit/AgentAccessibility.swift:224) sets `modal.present` only for an attached sheet. An authored modal filtered by this commit still reports `false`. Consequently, an exposed control outside the modal’s parent subtree cannot trigger `outside-modal`: [agent-ax.mjs:247](/tmp/x9-review/scripts/agent-ax.mjs:247) requires that metadata.

   **Fix:** make the collector recognize the effective authored modal and retain an identifiable modal boundary despite AppKit flattening. Test `modal` metadata and a non-sibling leak, rather than only checking which button IDs remain.

5. **Should-fix — “last child” does not mean “painted on top.”**  
   [NodeViewMac.swift:347](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:347) selects the last modal subview, while painting also uses [NodeViewMac.swift:756](/tmp/x9-review/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:756)’s `usedZIndex`. With two modal siblings, an earlier overlay with higher `z-index` is visible above the selected overlay, yet its accessibility subtree is discarded.

   **Fix:** select by effective paint order, using sibling order as the tie-breaker, and test conflicting sibling/z-index orders. Correct the corresponding claim in [LLP 1080.003:27](/tmp/x9-review/llp/1080.003-authored-modality.rfc.md:27).

The ordinary non-scrolling iOS root does place overlays beside the native tab container. Compiler bool validation, JS-target reuse of the web prop mapping, explicit `"false"` serialization, flat-leaf promotion, pool reset, and the unchanged schema digest look sound. The iOS XCTest verifies the property and the agent’s sibling-rule implementation; it does not independently prove native-bar exclusion or VoiceOver focus movement.

Verdict: LAND WITH FIXES
