# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 3, narrow re-check (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `88ec858564`.
- **Method:** one brief (sha256 `5f05f1a09db2db2324ad6f6ddfdced9d85e273d4c50adab5d0ebb98210b0d194`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed. The full Mac and iOS suites ran on the final tip.

## Brief

Narrow re-check of round 3's fixes (host change, macOS and iOS: Apple's plain text fields take CSS `direction` as each paragraph's explicit base; LLP 1001 §1). HEAD 88ec858564; round 3 reviewed 43de3d399b. Read `git diff 43de3d399b..HEAD` (one commit), with context from `git diff d0910f93df..HEAD` as needed.

Round 3 findings, and the fixes:
- **Astra P2: background undo trap.** The FieldEditor undo observer now returns before MainActor.assumeIsolated unless it runs on the main thread. Test: testAnUndoOffTheMainThreadIsPassedOver posts a DidUndoChange from a global queue. Honest note: on 43de3d399b this test did not trap in this harness, so it is a guard, not a reproduction.
- **Astra P2: a cancel whose input handler moves the focus restyled the other field's editor with the first field's direction.** markedTextChanged now styles the editor for its current owner (FieldEditor.owner, or self only if the field still has that editor), and FieldEditor.unmarkText re-reads self.owner after settlingComposition. Test: testACancelThatMovesTheFocusStylesTheEditorForItsNewField, red on 43de3d399b.
- **Astra P2: an appearance:none input's undo kept the restored physical alignment.** applyBaseDirection now always sets an input editor's side (fieldAlignment), as styleFieldEditor and syncFieldSide do; a text area's only where native or said. The undo test now covers an appearance:none input and asserts the side. Honest note: in this harness that assertion also passed on 43de3d399b, so it doesn't reproduce the stale `.left` Astra described; the fix makes the editor's side unconditional.
- **Grok P2 / Astra P2: iOS stand-in fields (search bar, alert prompt).** (a) applyFieldDirection now sets each field's textAlignment to the node's computed one (textAlignmentCode, CSS start), the stand-in's too, and puts the same alignment in the paragraph. (b) A direction that came while a stand-in composed now applies from the stand-ins' own delegates: HeaderSearch.searchBar(_:textDidChange:) and textFieldDidChangeSelection, and PromptField.typed() and textFieldDidChangeSelection. The stand-in test asserts the side (red on 43de3d399b: alignment natural) and the marked/unmarked sequence. Its composition end calls applyFieldDirection by hand, as those delegates do; UIKit's real search bar and alert wiring isn't driven in a unit test.
Full suites on 43de3d399b: Mac 305/0 and iOS 0 failed. On this tip they are running. Caps is green.

Check only:
(a) Are these fixes correct and complete?
(b) Does forcing textAlignment on UISearchTextField or an alert's UITextField change their look beyond the side (an LTR document's search field: UIKit natural vs our left, where natural is left in an LTR locale but right in an RTL locale under an `ltr` page)?
(c) Any new double report, event, or selection move?
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll re-check only the round-3 fixes against `43de3d399b..HEAD`, using the wider diff for context, and I won't change any files.The tip is one commit past the round-3 review. I'll read that diff and the surrounding field-direction code before judging the four fixes.The commit is small. Next I’ll read the field-editor and stand-in paths those lines sit in, including who calls them.The four round-3 fixes are correct and complete. Nothing in them adds a second input, select, or caret move, and forcing `textAlignment` on a search or alert field only moves the text and the placeholder.

**(a)** Each fix lands on the path that was wrong.

The undo observer returns before `MainActor.assumeIsolated` unless it is already on the main thread (`TextAreaMac.swift:717`). The manager check stays inside the isolated block. A text view’s undo is posted on the main thread; another object’s is not this editor’s, and touching the view there would be illegal anyway. The off-main test is only a guard, as noted, and the guard itself is the right one.

A cancel restyles whichever field owns the shared editor after the `input` handler runs. `markedTextChanged` reads `FieldEditor.owner` after `typedText`, and falls back to this node only while its field still has that editor (`FieldEditingMac.swift:127-129`). `unmarkText` re-reads `self.owner` after `settlingComposition` (`TextAreaMac.swift:759-762`). A direct `unmarkText` and a cancel through `setMarkedText` are both covered. A text area is not the shared editor, so it still takes `applyBaseDirection` on its own view.

`applyBaseDirection` now sets an input editor’s side whenever the view is not the text area (`TextAreaMac.swift:364`). A text area still gets a side only when it is native or `text-align` is said, which is the same rule as `styleTextArea`. The undo observer calls this after the restore, so a restored physical alignment is overwritten even though this harness already showed `.right` on `43de3d399b`.

On iOS, `applyFieldDirection` writes `textAlignmentCode` (CSS `start`: 0 left, 1 center, 2 right, 3 justify, matching `NSTextAlignment` on iOS) onto the field and the stand-in, and the same value into the paragraph (`NativeFieldsIOS.swift:124-131`). While `markedTextRange` is set it still skips both. The search bar and the alert prompt apply that from their own delegates on text change and on selection change (`NavigationBarIOS.swift:437`, `NavigationBarIOS.swift:450`, `PromptFieldIOS.swift:123`, `PromptFieldIOS.swift:165`), which is the stand-in equivalent of `TextField.unmarkText`. The owned field already did this from `fieldChanged` and `unmarkText`.

**(b)** `textAlignment` aligns the text and the placeholder inside the text rect. It does not move a search field’s glyph or clear button, and it does not change the capsule, font, color, or insets. Those follow the bar’s layout direction, which this commit does not set. An alert field has no glyph; the same property only changes the text and the placeholder. In an LTR locale, UIKit’s `natural` and our `left` are the same place. In an RTL locale under an `ltr` page, `left` is CSS `start` and `natural` would be the right. That difference is the side.

**(c)** No new report or selection move. `typedText` still runs once, and a second call with the same string returns. `styleFieldEditor` and `applyBaseDirection` change attributes only. The iOS hooks call `applyFieldDirection` after the existing `typedText` and `fieldSelections.changed`. When the side already matches, both writes are skipped. When it does not, `changed` still ignores an unchanged range.

VERDICT: APPROVE
