# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 3 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `43de3d399b`.
- **Method:** one brief (sha256 `e9a2ecd5d3139cf63b3a487f53f4f3da1a1a6ceb3bab333c4af95a1105468141`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 88ec858564; the focus-move and stand-in side tests fail before the fix, the off-main-thread and bare-input undo tests are guards that do not reproduce here

## Brief

Code review, round 3 of 3 (host change, macOS and iOS: Apple's plain text fields take CSS `direction` as each paragraph's explicit base, and the Mac's unsaid text-align becomes start; LLP 1001 §1). Tip 43de3d399b is HEAD of this worktree; merge-base d0910f93df. Review the whole change fresh with `git diff d0910f93df..HEAD`, and round 2's fixes with `git diff b02b4ed8a6..HEAD`.

Round 2 findings (Astra and Grok, both CHANGES), and the fixes, each with a test that fails on b02b4ed8a6's product code:
- **Astra P2: a Mac composition cancelled with setMarkedText("") dropped the pending direction.** markedTextChanged now reapplies it once nothing is marked: applyBaseDirection for a text area, styleFieldEditor for an input. The composition test now also cancels, on the textarea and the input.
- **Astra P2: the undo repair missed Mac inputs.** For the field editor, AppKit restores the characters, posts the change, and then the same undo group restores the old paragraph attributes. So the session's FieldEditor now observes NSUndoManagerDidUndoChange and NSUndoManagerDidRedoChange for its own undo manager and reapplies the base. controlTextDidChange also reapplies it, which covers the secure editor's text changes. The undo test now covers the input too and checks no extra undo step.
- **Astra P2: iOS search-bar and alert-prompt fields (headerField) never took the direction.** applyFieldDirection now covers field and headerField, and runs when NavigationBar and PromptField attach their field, and on each style. Test: testTheFieldStandingInForAnInputTakesItsDirection.
- **Grok P2: an iOS UITextField commit that changes no text sends no editingChanged.** TextField.unmarkText now applies the direction. The test no longer calls fieldChanged by hand.
- **Grok P2: Mac password composition.** No change for the composition itself: macOS turns on secure input for a password field, which shuts out input methods, so no composition reaches that editor. Any text change there now reapplies through controlTextDidChange.
- **Grok P3: defaultParagraphStyle was copied before its alignment was set.** Fixed. Test: an empty rtl textarea's default paragraph is right-aligned and rtl.

Full suites on b02b4ed8a6: Mac 305/0 and iOS 0 failed. On this tip they are running. Caps is green. Web parity: host/web/tests/field-direction.test.mjs passes. No gate or baseline has an rtl plain field.

Check hard, final round:
- the FieldEditor undo observers (lifetime, a field editor shared across fields, an owner change, an undo in another field sharing the window's undo manager, work done on every undo in the app);
- re-entrancy of styleFieldEditor from markedTextChanged;
- headerField styling against UIKit's search bar and UIAlertController (could forcing the paragraph alignment change their look?);
- any double report, lost event, selection or caret move;
- whether the tests prove what they claim.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the full change from the merge-base and the round-2 fixes separately, then check the undo observers, re-entrancy, header-field styling, and whether the tests actually prove the claims.The tip is `43de3d399b` on merge-base `d0910f93df`. I'll read the full change and the round-2 delta, then the field-editor and header-field code those diffs touch.The undo observer and the search-field paragraph style are the two places a pass can still hide a real bug. I'll check those against AppKit and the field-editing call paths.The Mac undo and composition fixes hold. Two iOS holes remain on the field that stands in for an input, so this should not land as-is.

## Findings

### 1. P2 — A search bar or alert prompt never takes the field's side

`host/apple/Sources/ExactKit/IOS/NativeFieldsIOS.swift:128`

`applyFieldDirection` writes `paragraph.alignment = f.textAlignment` for both the page field and `headerField`. The page field's `textAlignment` was just set from CSS in `applyFieldStyle` (`NativeFieldsIOS.swift:107`). Nothing sets `headerField.textAlignment`. A `UISearchTextField` and an alert's `UITextField` keep UIKit's own alignment (left, or natural, which in an LTR locale is the left). The paragraph is pinned to that, not to `start`.

Under `direction: rtl` the stand-in gets a right-to-left base and stays left-aligned. Mac's lent toolbar and alert fields do take the side (`syncFieldSide` / `styleLentFieldSide`). The new test only reads `baseWritingDirection` on a plain `UITextField` (`FieldDirectionIOSTests.swift:165`), so it passes with the side wrong.

Forcing that alignment does not restyle the search icon, the clear button, or the alert chrome. The value written is the control's own `textAlignment`, and the paragraph style is copied first, so line height stays. The base-direction write reorders bidirectional text inside the existing text rect. That part is the feature. The miss is that CSS `start` never reaches the stand-in.

### 2. P2 — A direction change while that stand-in is composing is dropped

`host/apple/Sources/ExactKit/IOS/NativeFieldsIOS.swift:124`

The loop skips a field whose `markedTextRange` is set, and the comment at line 122 says the end of the composition applies it through `unmarkText` and `fieldChanged`. Those run only for the page `TextField`:

- `TextAreaIOS.swift:59` overrides `unmarkText` on `TextField`, the view `makeField` creates.
- `FieldEditingIOS.swift:72` runs from that view's `editingChanged`.

The search bar and the alert prompt are UIKit's fields. `HeaderSearch.searchBar(_:textDidChange:)` (`NavigationBarIOS.swift:434`) and `PromptField.typed()` (`PromptFieldIOS.swift:159`) report the text and do not call `applyFieldDirection`. Neither field is a `TextField`, so a commit that changes no text has no `unmarkText` hook either. Attach applies the direction once (`NavigationBarIOS.swift:372`, `PromptFieldIOS.swift:92`). A style that arrives while the stand-in is marked updates the hidden page field and leaves the visible one on the old base for the rest of that editing session. The new test never marks the stand-in, so it does not catch this.

## Checked, and holding

Field-editor undo, measured against AppKit here: the session editor's undo manager is not the window's. Ending an edit drops it (`undoManager` is nil off-window). The next field gets a new manager with an empty stack, so one input's undo does not replay into another field that shares the editor, and a text area's private manager does not match. `owner` is read when the notification fires, from the editor's current delegate. The tokens are held by the editor, the editor is captured weakly, and `deinit` removes them. `applyBaseDirection` from `NSUndoManagerDidUndoChange` does not post a text change, does not open an undo step, and does not move the selection. A secure field's editor is not this `FieldEditor`; its undo does post the change, with the old paragraph already restored, and the reapply in `controlTextDidChange` sticks. `MainActor.assumeIsolated` succeeds for that main-thread undo.

`styleFieldEditor` from `markedTextChanged` does not re-enter. On the field editor (`isRichText == false`), `setAlignment` and `textStorage.addAttribute` call neither `shouldChangeText` nor `didChangeText`, so there is no second `input` and no `beforeinput`. `typedText` also ignores a repeat of the same string.

The Mac tests measure the thing they name: the undo test undoes through the manager the observer watches and checks `canUndo`; the cancel test ends in `setMarkedText("")` with nothing marked; the empty `rtl` textarea reads `defaultParagraphStyle` after alignment is stored.

One latent crash, not hit by any undo in this tree: the observer is registered with `object: nil` (`TextAreaMac.swift:715`), and `assumeIsolated` runs before the manager check (`TextAreaMac.swift:717`). An undo posted off the main thread aborts the process (`dispatch_assert_queue_fail`). Every `UndoManager` in this host is created on the UI side, so this is not a current failure.

VERDICT: CHANGES
