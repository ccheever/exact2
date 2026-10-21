# Code review, round 2: aria-modal r2 (cc6a3efc1), 2026-10-03 (grok)

- **Family:** xAI. `grok-4.7`, reasoning effort xhigh, `--no-subagents`, read-only by instruction, `--cwd` a detached worktree at `bd1dacae6`.
- **Method:** one brief (sha256 `f8a3896d543c7f10e02909f7c75d08e8a70b8b661f0c1fa1a3806d59bd4c3036`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the findings and verdict, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):**
  - 1 (destroying the last modal never posts): taken, with the view id (the same fix as astra 1).
  - 2 (a sheet's stack over a root modal; a covered stack's dialog): taken. A visible sibling painted over the candidate, or an accessibility-hidden ancestor, disqualifies it. The test adds a covering sibling. Making the sheet's stack itself modal is not done: `ModalIOS` already hides the background.
  - 3 (stale schema and RFC text): taken.

---

1. **Should-fix** — Destroying the last modal never posts `.screenChanged`. `host/apple/Sources/ExactKit/Accessibility.swift:201`

`announcedModal` is a weak view. A conditional overlay closes with `destroy`: `PresenterIOS.swift:800` releases the last ExactKit retain and removes the view before `syncModal` runs, so the view is gone and the weak reference is already nil. `newest` is nil too, and `nil !== nil` is false, so the guard returns without posting. Siblings are exposed again because the view has left the hierarchy, and VoiceOver is not moved back. Exit, `display: none`, and a cleared prop keep the view alive, which is why those tests pass. The same identity check skips the post when a pooled row is rebound to a new modal id in the same batch (`NodePoolIOS.swift:512` clears the flag, then `syncModal` sees the same object).

Fix: remember `announcedModalID: UInt32?`. Post when that id differs from the chosen view's id, including when the chosen id is nil. Add a destroy of the only modal to `testAriaModalIsUIKitsModalViewWhileExposed` and assert the id is cleared.

2. **Should-fix** — A root `aria-modal` hides a navigation sheet presented over it. `host/apple/Sources/ExactKit/Accessibility.swift:188`

`syncModal` picks the frontmost modal sibling and ignores every other subview. `NavigationIOS.swift:334` inserts the presented stack with `root.addSubview(nav.view)`, and `NavigationIOS.swift:351` brings it to the front. On a root whose `container` is itself (Signal Clone: no scroll, clip box, material, or glass), that navigation view is a sibling of the overlay. With `zPosition` 0, the later navigation view paints on top, while `accessibilityViewIsModal` on the overlay still hides it. VoiceOver stays in the overlay the sheet is covering. `ModalIOS.swift:265` sets `accessibilityElementsHidden` on the previous navigation, and the exposure check only reads the modal view's own flag (`Accessibility.swift:189`), so a dialog still inside that covered stack stays modal too.

Fix: treat a candidate as exposed only when no sibling paints in front of it (same `zPosition`, then subview order) and no ancestor has `accessibilityElementsHidden`. While a sheet is up, set `accessibilityViewIsModal` on the presented navigation view and clear it when the sheet closes, so the sheet hides the page and the overlay. Cover a root overlay plus `navigationPresentation="modal"` in the iOS tree test.

3. **Nit** — The schema and the RFC still describe the r1 macOS behavior and the old web test. `kernel/tables/schema.json:336`, `llp/1080.003-authored-modality.rfc.md:41`

The schema comment says macOS keeps only the modal child's accessibility children. r2 removed that filter. The proof list still points at `host/web/tests/it/host.rs`; the test now lives in `host/web/tests/it/aria.rs`.

Fix: describe macOS as deferred (D3), and point the proof at `aria.rs`.

Round 1, as checked against this tree: macOS cells, scroll parents, paint order, and unreported `modal` are resolved by deleting the filter and recording D3. Exit, `display: none`, `false`, and clear drop the flag, and the iOS test covers those plus sibling order and the sibling coming back. D2 matches `container` and the tab host. The agent walk's `kids.first` is sound once only one sibling keeps the flag. `chrome.ids` keeps a prop set to `"false"` and drops a cleared one; `modalViews` clears a view that left the index. `syncModal` runs at the end of every iOS `apply`, including a style-only `display` change, on the main thread. `layer.zPosition` is `usedZIndex` (`NodeViewIOS.swift:1139`), set before the pass. The sibling test never sets a conflicting `z-index`.

Verdict: LAND WITH FIXES
