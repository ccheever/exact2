# Code review: macOS pointerdown before focus in a text area or an input (LLP 1005), the control-click test's rewrite, a quick check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `5fbe7ac83c`.
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

Yes. The [replacement test](/tmp/iosfix2-rv-pd6-astra/host/apple/tests/ExactKitTests/EditorPointerOrderMacTests.swift:394) covers the relevant part of condition (1): Control and Control+Shift produce no early down, no `early`, and no hold across all three editor types. Each unmodified positive control confirms the target can reach the early path, preventing a vacuous pass.

Coverage lost: sending control-clicks through NSApp and checking downstream down/up balance, duplicate suppression, and focus-before-down ordering. The old test exercised real menus but did not assert that one opened.

That narrowing is acceptable here. Only this test differs from `497de689b9`; the monitor still calls `pressEditorFirst` and returns the original event, leaving subsequent handling unchanged. Password-field and combined-modifier coverage increase.

Review only; I did not rerun the suites or edit files. The two 302/0 runs are your reported evidence.

VERDICT: APPROVE
