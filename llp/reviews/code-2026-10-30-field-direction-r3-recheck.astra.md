# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 3, narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `88ec858564`.
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

No actionable findings in `43de3d399b..88ec858564`.

- **(a)** The fixes look correct on the reviewed paths: the undo guard precedes actor isolation, composition settlement re-reads the editor’s owner, and input alignment is now unconditional. See [FieldEditingMac.swift:129](/tmp/iosfix2-rv-fd4-astra/host/apple/Sources/ExactKit/Mac/FieldEditingMac.swift:129) and [TextAreaMac.swift:717](/tmp/iosfix2-rv-fd4-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:717).
- **(b)** At [NativeFieldsIOS.swift:126](/tmp/iosfix2-rv-fd4-astra/host/apple/Sources/ExactKit/IOS/NativeFieldsIOS.swift:126), I found no evidence of appearance changes beyond text and placeholder placement. Both are affected by Apple’s [`textAlignment`](https://developer.apple.com/documentation/uikit/uitextfield/textalignment). Physical left under an `ltr` page, including in an RTL locale, matches the specified CSS `start` behavior.
- **(c)** No new duplicate report, event dispatch, or logical selection move identified. Attribute updates are guarded, and existing text/selection reporting deduplicates unchanged values.

Runtime qualification remains limited: [FieldDirectionIOSTests.swift:176](/tmp/iosfix2-rv-fd4-astra/host/apple/tests/ExactKitTests/FieldDirectionIOSTests.swift:176) bypasses actual stand-in delegate delivery. Real UIKit IME callbacks and visual appearance remain unverified here. I did not rerun suites in this read-only checkout.

VERDICT: APPROVE
