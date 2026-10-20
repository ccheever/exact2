# Code review: LLP 1121 stage 9 G3, a pointer's :hover look on iOS, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the iOS hover-look commit.
- **Method:** one brief (sha256 `bec521430f16d0e42179248119da196a3fb3c214fc01d3474076f56606912bab`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (the same, plus removal re-entering relook).
- **Disposition:** fixed in round 1's commit.

---

Nothing here is blocking. A node that already has a `:hover` look gets one recognizer, shows the look on `.began`, and a hold on that node lets go on a real enter or leave. Four defects sit beside that path.

1. **should-fix** — A hover hold on a node with no `:hover` look never sees the pointer. `HeldState.swift:169`, `LookHoverIOS.swift:19`.
   `hold` seeds `over` from `pressPaint.hovered`, and `lookHovered` runs only from the look recognizer, which `lookHover` installs only when the style contains a hover look. Node 9 in the fixture (a press, no looks) can hold `hover`, but nothing ever calls `lookHovered` for it, so the pointer entering or leaving leaves the hold in place. The same strand happens if a hold starts on a node whose look is removed later: teardown drops the only observer. The Mac seeds `over` from a live hit test and from the full hover path, including a node with no look (`HoverMac.swift:56`).
   **Fix:** Track the pointer for the held node even when it has no look (a recognizer owned by the hold, or a window hit-test). Seed `over` from that. Let go only when that bit flips. Remove the extra recognizer when the hold ends if the style still does not want one.

2. **should-fix** — Removing the look recognizer is reported as the pointer leaving. `LookHoverIOS.swift:29`.
   Hover is held, the pointer is over the node (`over == true`, recognizer in `.began`), and a style op drops the `:hover` look. `lookHover(false)` calls `removeGestureRecognizer`. UIKit moves a recognizing recognizer to `.cancelled` and sends the action, so `lookHovering` calls `lookHovered(self, false)` and the hold ends with `"input"` while the pointer has not moved. That callback also runs `lookState` → `relook` → `applyStyle` re-entrantly inside `presented`, before the caller’s `applyStyle`. If a future UIKit stops sending the action, `pressPaint.hovered` is still forced false and no recognizer remains to see the real leave, so the hold sticks instead.
   **Fix:** Clear the target (or ignore callbacks) before removal, and do not call `lookHovered` from teardown. Keep the hold’s observer until the hold ends, as in finding 1.

3. **should-fix** — A `:hover` look makes a collection row unrecyclable. `NodePoolIOS.swift:332` (recognizer added at `LookHoverIOS.swift:27`).
   `recyclable` accepts a gesture only when it is an idle `PointerRecognizer` or a resting context-menu recognizer. `UIHoverGestureRecognizer` is neither, including while it sits in `.possible` with no touches. `park` fails, and `PresenterIOS.swift:955` destroys the row. Every virtualized card whose style has a hover look is created and destroyed instead of reused. The handler recognizer had this shape already; a look is on ordinary cards.
   **Fix:** Treat an idle `lookHoverRecognizer` (and the handler `hoverRecognizer`) as pool-safe. On `rebind`, force the recognizer back to `.possible` and clear `hovered`, so a row parked under the pointer does not come back already hovered.

4. **should-fix** — The new test never changes the recognizer’s state, and the tvOS test is not compiled. `HeldStateTests.swift:735`.
   `testIOSHoverLookAndHold` calls `HeldState.lookHovered` and `lookState` itself. Deleting the body of `lookHovering` leaves it green, including the assertion that the pointer showed the look. It never checks a leave, a second `true` while already over, or a hold on view 9. `testTVOSHoldsNoHover` is the `#else` of `#if os(iOS)` inside a file wrapped in `#if os(iOS) || os(macOS)` (`HeldStateTests.swift:1`), so that function is dead.
   **Fix:** Drive `.began`, `.changed`, `.ended`, and `.cancelled` through `lookHovering` (a recognizer subclass or a test hook). Assert leave, already-over, and a hold on the node with no look. Put the tvOS assertion in a test the tvOS target compiles.

Checked and sound: install and removal run only when the presence of the look flips, so a style op or a paint frame that still has the look does not churn recognizers, and the `look()` check in `presented` is cheap. `.changed` is the pointer moving inside the node, and ignoring it matches a binary look. A disabled node still latches `hovered`, and `looked` plus `propsChanged` → `relook` keeps the paint off until it is enabled again. Authored `inert` and `display: none` leave hit-testing (`isUserInteractionEnabled`, `isHidden`), so a new hover does not begin. `shouldRecognizeSimultaneouslyWith` returning true is what keeps a recognizer that sits in `.began` for the whole hover from blocking scroll pans, long-press, context menus, drag and reorder, and `UIPointerInteraction`. The handler recognizers still exclude each other, because their own delegates do not return true, so the innermost `hover` handler still fires once. tvOS stays unsupported (`#if os(tvOS)` in `hold`, recognizer compiled only for `os(iOS)`). Catalyst is `os(iOS)` and correctly gets this recognizer. `LookHover` is `@MainActor` and UIKit calls the delegate and the action on the main thread; under the package’s Swift 5 mode that is the same pattern as the other UIKit callbacks that touch `HeldState`.
