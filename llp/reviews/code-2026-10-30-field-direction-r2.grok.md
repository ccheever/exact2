# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `b02b4ed8a6`.
- **Method:** one brief (sha256 `f950c255ee81885d4211dfa5744d35d4003ffac63d74750b7ee6b2cdb38fbe87`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in 43de3d399b, each with a test that fails before the fix (the Mac password composition answered: secure input shuts out input methods)

## Brief

Code review, round 2 of 3 (host change, macOS and iOS: Apple's plain text fields take CSS `direction` as each paragraph's explicit base, and the Mac's unsaid text-align becomes start). Tip b02b4ed8a6 is HEAD of this worktree; merge-base d0910f93df. Review the whole change with `git diff d0910f93df..HEAD`, and round 1's fixes with `git diff 17af0e693d..HEAD`.

Round 1: Grok APPROVE; Astra CHANGES with six P2 findings, all fixed, each with a test that fails on 17af0e693d's product code:
1. **Undo restoring old paragraph attributes.** applyBaseDirection now runs after textDidChange (Mac) and textViewDidChange (iOS). It writes attributes only, adds no undo step and skips marked text. Test: testAnUndoAfterADirectionChangeTakesTheDirectionNow, which also checks the undo stack has no extra step.
2. **A direction change during composition was dropped.** On the Mac it's applied at TextArea.unmarkText and at FieldEditor.unmarkText (via styleFieldEditor); on iOS at TextArea.unmarkText; and on both at textDidChange after a commit. Tests: testADirectionThatComesWhileComposingAppliesAtItsEnd, Mac (textarea, input) and iOS.
3. **The iOS input wrote defaultTextAttributes into a composition.** Now applyFieldDirection() is guarded by markedTextRange and runs again at fieldChanged (editingChanged). In the iOS test, fieldChanged is called by hand after unmarkText, standing in for UIKit's editing-changed.
4. **Lent Mac fields (toolbar search, alert prompt).** syncFieldSide runs at lending, and styleLentFieldSide (from applyStyle) covers the field and its editor. Test: testAFieldLentToTheToolbarTakesTheDirection.
5. **A Mac detach lost a said alignment.** applyBaseDirection now also sets the side when the field is native or the author said one. Test: testAMarkdownEditorWhoseMarkupGoesKeepsASaidSide.
6. **The Mac tests read layoutManager, which switches a text area to TextKit 1.** They now measure with firstRect(forCharacterRange:actualRange:), and one assertion checks the text area still has a textLayoutManager.

Theme2's conditions (approval 10:45Z):
(1) web parity: host/web/tests/field-direction.test.mjs passes;
(2) red-first tests, Mac and iOS;
(3) no gate or baseline has an rtl plain field (only markdown-stress's Markdown editor, unchanged);
(4) full suites and caps: caps green. On 17af0e693d the full iOS suite was 0 failed; the Mac suite had 1 failed, FrameAdvanceMacTests, a known load flake. The suites on this tip are running.

Check hard:
- the cost of applyBaseDirection on every textDidChange for a large textarea (it enumerates paragraph styles and writes only where they differ);
- any edit or event the hooks could emit (an input event, selectionchange, undo coalescing broken by attribute writes during typing);
- styleFieldEditor from FieldEditor.unmarkText (any re-entrancy with settlingComposition);
- styleLentFieldSide versus AppKit's own toolbar styling;
- the iOS hooks during UITextView typing (typingAttributes churn, a caret jump);
- whether the tests prove what they claim.
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

I'll review the full host change from the merge-base and the round-1 fixes on top of it, without editing anything. First I'm reading the review guidance and the two diffs.The tip is `b02b4ed8a6` on merge-base `d0910f93df`, with round 1 in the last commit. I'll read both diffs and the surrounding field-direction code before judging the hard cases.The diffs are large enough that the interesting paths are the direction hooks and their tests. I'll read those in full and trace what each call can emit.[P2] Apply an iOS input's direction in `unmarkText`, once the mark is gone — `host/apple/Sources/ExactKit/IOS/FieldEditingIOS.swift:72`

A direction that arrives while a `UITextField` is composing is applied only from `fieldChanged`. That runs for `.editingChanged`. A commit that does not change the text posts no change: the text areas apply the direction in `unmarkText` for that reason (`host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:236`), and `TextField` (`host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:55`) does not override `unmarkText`. After a confirm-as-is, `defaultTextAttributes` stay on the old base until some later edit sends `.editingChanged`. `testADirectionThatComesWhileComposingAppliesAtItsEnd` calls `fieldChanged()` itself (`host/apple/tests/ExactKitTests/FieldDirectionIOSTests.swift:153`), so it passes without UIKit sending that event. `FieldCompositionIOSTests` already records that a field does not send `.editingChanged` for `unmarkText`.

[P2] Reapply the base when a Mac password field's composition ends — `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:748`

The composition hook is `FieldEditor.unmarkText` calling `styleFieldEditor`. A password field's editor is AppKit's `NSSecureTextView` (`SecureField` at `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:758`), and `controlTextDidChange` (`host/apple/Sources/ExactKit/Mac/FieldEditingMac.swift:65`) does not apply the base. `styleFieldEditor` also skips while text is marked. A `direction` that arrives during a password composition stays the old paragraph base for the rest of that editing session. `syncFieldSide` corrects the field only after editing ends. The composition test covers plain `input` and `textarea` only.

[P3] Set the said alignment before assigning `defaultParagraphStyle` — `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:520`

`defaultParagraphStyle` copies the style at the assignment. `paragraph.alignment` is set on the next line, so the copy keeps `.natural` (raw value 4) while `baseWritingDirection` is already the field's. The text storage and `typingAttributes` get the mutated style afterward, and `applyBaseDirection` repairs the default on a later text change. Until then an empty textarea's default style does not carry a native or said alignment.

The per-keystroke walk is cheap in the steady state. On an `NSTextView`, the first fix writes and the next keystrokes write nothing; `beginEditing`/`endEditing` with no edit does not post another text change. Typing "abc" with that fix on each change undoes as one step. A 20,000-paragraph storage whose styles never coalesce is about 5 ms per no-op walk; a plain field's identical paragraph style is one run. `styleFieldEditor` after `unmarkText` posts no second `NSTextDidChange` or `NSControlTextDidChange`, and it does not register an undo. It runs after `settlingComposition` returns. Assigning an `NSSearchField` to an `NSSearchToolbarItem` leaves `alignment` and `baseWritingDirection` as set. The iOS textarea hook is attribute writes guarded by the marked range, same early-out as the Mac. No gate or baseline plain field is `rtl`; the only `direction="rtl"` field in app sources is `apps/markdown-stress`'s Markdown editor.

VERDICT: CHANGES
