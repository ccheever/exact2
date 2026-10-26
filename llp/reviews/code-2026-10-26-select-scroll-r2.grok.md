# Code review: Apple pass #2: an outline brings the selection's own copy into view, round 2 of 2, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `de2130e9a8`.
- **Method:** one brief (sha256 `509697093ddb23934a2e9d15e2d4fc230f6cf360fad267b78ccc7e0fb3a46549`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**SHOULD** — Apple still scrolls a `visibility: hidden` copy. `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:103` and `host/apple/Sources/ExactKit/Mac/InspectMac.swift:244`.

`Inspect.revealing` treats `shown == false` as "shows nowhere" and then scrolls nothing. On both Apple hosts that flag is only `isHidden` (iOS also `alpha < 0.01`). `isHidden` is `display: none` or a host hide — `CssVisibility.swift` keeps the box for computed `visibility: hidden` and does not set `isHidden`. A held selection on that copy still gets `scrollRectToVisible` / `scrollToVisible`, so the feed moves to an empty box. The web half of this change does not: `checkVisibility({ visibilityProperty: true })` is false, and no scroll happens. The Mac session test only sets `isHidden` (`InspectRevealMacTests.swift:127`), so it stays green.

The reveal bit needs `cssVisibilityHidden` as well (the node's computed value, same as hit-testing). Leave `place` / `shown()` as they are so the dashed mark is still drawn on that box.

Checked and holding: the other session's same node id returns nil when its copy is not shown; with no selection the first shown copy is unchanged; web `?? true` runs only when `checkVisibility` is absent (then the first connected copy), and `visibilityProperty` is the option Chrome has shipped since 121, which is what the rest of these dev pages already call; the page test waits on `[data-exact-outline]`, which is appended in the same turn as the scroll.
