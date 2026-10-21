# Code review: the iPad-only Swift tests and the PressFocus sheet flake, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `8e5ae24e67`.
- **Method:** one brief (sha256 `1f0abe24bfef606f8face63f0c88507c8319b7c4d898c7cb4b553b57e34b3faf`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**No blockers found. Three should-fix items remain.** Reviewed read-only; no tests run.

1. **should-fix — [NavigationTabsIOS.swift:471](/tmp/rv-ipad-2/host/apple/Sources/ExactKit/IOS/NavigationTabsIOS.swift:471): matching counts still do not identify tab chrome.**  
   A hatch-added collection beside the selected controller, with one section containing two items, satisfies the two-tab fixture’s predicate. If encountered first, it supplies inspection’s tab IDs and rectangles and receives the scroll override. This leaves part of round 1’s finding unresolved.

   **Fix:** establish association with the actual tab items, preferably through their identifiers, and reject ambiguous candidates. Add a matching-count sibling collection test; verify its content remains inspectable, its scroll flag survives, and the real tabs still resolve.

2. **should-fix — [ScrollsToTopIOS.swift:25](/tmp/rv-ipad-2/host/apple/Sources/ExactKit/IOS/ScrollsToTopIOS.swift:25): suppression is permanent and does not distinguish a floating strip from a scrollable sidebar.**  
   Even correctly identified tab content can be vertical. A sidebar or expanded list satisfying the same section predicate gets `scrollsToTop = false` outside inspection, with no restoration if the presentation changes. The SDK’s [scrollsToTop declaration](/Applications/Xcode-27.0.0.app/Contents/Developer/Platforms/iPhoneSimulator.platform/Developer/SDKs/iPhoneSimulator.sdk/System/Library/Frameworks/UIKit.framework/Headers/UIScrollView.h:264) explicitly limits the one-eligible-scroller rule to **iPhone**. The current flag-count test does not establish sidebar behavior.

   **Fix:** restrict this override to the verified horizontal floating strip, preserve/restore the prior value across presentation changes, and cover a scrollable sidebar or expanded list. This risk is identified from code, not reproduced here.

3. **should-fix — [AgentRevealIOS.swift:129](/tmp/rv-ipad-2/host/apple/Sources/ExactKit/IOS/AgentRevealIOS.swift:129): round 1’s floating-bar occlusion follow-up remains unchanged.**  
   `bars(over:)` appends only `tabs.tabBar`, which the window filter then removes on the floating path. On content extending beneath that bar—particularly without a navigation bar—`visiblePort` can consider an obscured target visible, so reveal leaves it covered.

   **Fix:** include the verified floating chrome in the cover calculation and test revealing a target whose centre starts beneath it. This predates the range, but was explicitly raised in round 1 and has no disposition in HEAD’s message.

The remaining round-1 changes hold: synchronous touch restoration precedes both delegate callbacks, addressing the overwrite and dropped-restore races; floating placement resolves the current cell; the complete scroll census is restored; and both PressFocus scenarios now control the timer. The trait correction and larger reveal fixture preserve their assertions.

Given the reported UIKit probe, limiting pushed-route disappearance checks to the insetting bar is justified. Home, Detail, and return visibility are now checked, and a missing floating bar fails explicitly; hiding it over Chat remains a documented product decision.
