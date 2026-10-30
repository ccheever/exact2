# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), the control-click test's rewrite, a quick check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `5fbe7ac83c`.
- **Method:** one brief (sha256 `41ae2832205095219dd7c9acbc28e63c11008635d663e7bc21c167deea06ec6f`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Quick check, test-only. HEAD 5fbe7ac83c changes one test in host/apple/tests/ExactKitTests/EditorPointerOrderMacTests.swift after the final re-check of 497de689b9, which both reviewers approved. See `git show HEAD`.
Why: the old testAControlClickGoesItsOwnWay sent control-clicks through NSApp and opened real contextual NSMenus (cancelled when tracking began). The Mac suite runs classes in parallel processes, and ContextMenuMacTests failed twice in full runs (an XCTUnwrap of the menu's on-screen frame came back nil) once this class was in the suite. It passed alone 2/2. That test measures on-screen menu windows.
The new testAControlClickTakesNoEarlyDown asks Presenter.pressEditorFirst directly. A control-click (and control-shift) on a field, a text area and a password field produces no early down, no `early` and no hold. The same press unmodified does produce the early down, so the control key alone decides.
Since then, two full Mac suites on 5fbe7ac83c were green (302/0).
Check: does the new test still prove condition (1)'s part that this change touches (no early path for a control-click; everything after it is today's code path, unchanged)? Is anything lost? Do NOT edit files. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll look at the HEAD commit and the condition it is supposed to still prove, without changing any files.The diff only swaps that one test. Next I’ll read condition (1) in the parent change and how the new assertion actually reaches the monitor.The gate is inside `pressEditorFirst`. I’ll check whether a control-click that misses it still falls through the old editor path, and what the old assertions were actually locking.The new test still proves the part of condition (1) this change touches. A control-click does not enter the early path, so the delivery after that decision stays the one already there.

`pressEditorFirst` returns on `.control` before it stores `mouseChain.early` or calls `pointerPressed`. The window monitor then returns that same event, with no control-click branch of its own. `tracksPointer` sends a down of its own only when `early` is not this event, and it does that from `mouseDown`, after `NSWindow` has focused the editor. That is the report a press had before the early down existed. The test shows `early` stays nil, so a control-click is on that path.

The test locks the decision itself. On a field, a text area, and a password field, a control-click and a control-shift click leave no `down`, no `early`, and no hold. The same event with no modifiers reports `down` and only that, so the control flag is what decides.

The rewrite no longer sends the click through `NSApp`, no longer opens a menu, and no longer checks the resulting log for one balanced press with `down` after `focus`. Those checks allowed an empty log. They locked an early down that leaked past a finished click, which is the same leak the new assertions reject at the function that would create it. They did not require the menu or the editor's later report to appear. A double report of one event cannot happen once `early` is unset: the early call is skipped, and `tracksPointer` emits its single down in the other branch.

VERDICT: APPROVE
