# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `b02b4ed8a6`.
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

1. **P2 — Cancelling a Mac composition still drops a pending direction change.** [TextAreaMac.swift:233](/tmp/iosfix2-rv-fd2-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:233)  
   Start composing, change `direction`, then cancel through `setMarkedText("", …)`. That path reaches `markedTextChanged`, which applies held values and Markdown props but never reapplies plain-field direction. AppKit posts no text-change notification for this cancellation. Both textarea and input retain the old base. The new test only calls `unmarkText`, which [accepts the marked text](https://developer.apple.com/documentation/appkit/nstextinputclient/unmarktext%28%29), so it does not cover cancellation.

2. **P2 — The undo repair misses Mac single-line inputs.** [TextAreaMac.swift:563](/tmp/iosfix2-rv-fd2-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:563)  
   Delete text from an `ltr` input, change its direction to `rtl`, then undo while focused. Restored text can bring back its old paragraph attributes, but this repair runs only for `textArea`. Neither `controlTextDidChange` nor the field editor’s undo completion reapplies direction. The added undo regression exclusively constructs a textarea. Apply the repair after input undo/redo too, and extend that regression to inputs.

3. **P2 — iOS search-bar and alert fields never receive the direction.** [NativeFieldsIOS.swift:123](/tmp/iosfix2-rv-fd2-astra/host/apple/Sources/ExactKit/IOS/NativeFieldsIOS.swift:123)  
   `applyFieldDirection` targets only `node.field`. Navigation search and alert prompts display separate controls stored in `headerField`; their attachment, styling and editing callbacks never apply the new paragraph base. Consequently, an explicitly directed input still uses UIKit’s automatic base when presented there. The iOS tests inspect ordinary page fields, including page search inputs, rather than these projected controls.

Reviewed both diffs; no files changed. Storage-only probes found inexpensive uniform-text scans, with cost increasing with attribute fragmentation. Native suites and UIKit caret/event behavior were not rerun in this read-only checkout.

VERDICT: CHANGES
