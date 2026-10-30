# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `43de3d399b`.
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

- **P2 — Unrelated background undo can crash the app.** [TextAreaMac.swift:715](/tmp/iosfix2-rv-fd3-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:715): the observers receive every undo manager’s notifications with `queue: nil`, then enter `MainActor.assumeIsolated` **before** checking manager identity. A background undo therefore traps before being rejected. A Foundation probe confirmed off-main notification delivery; [Apple documents the fatal isolation check](https://developer.apple.com/documentation/swift/mainactor/assumeisolated(_:file:line:)). Arrange safe delivery before accessing the editor.

- **P2 — Composition cancellation can restyle another field’s editor.** [FieldEditingMac.swift:126](/tmp/iosfix2-rv-fd3-astra/host/apple/Sources/ExactKit/Mac/FieldEditingMac.swift:126): `typedText` synchronously invokes the app before this call. If cancelling composition in RTL input A causes its input handler to focus LTR input B, the shared editor now belongs to B, but A calls `styleFieldEditor` on it and overwrites B’s direction and alignment. Revalidate ownership after callbacks; the captured-owner call in `FieldEditor.unmarkText` needs the same protection. The composition test never changes focus from its input callback.

- **P2 — Undo still restores stale alignment in bare Mac inputs.** [TextAreaMac.swift:363](/tmp/iosfix2-rv-fd3-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:363): with `appearance: none` and unsaid `text-align`, the repair leaves alignment untouched. Keep that appearance through this sequence: delete LTR text, change direction to RTL, undo. AppKit restores the old physical `.left` paragraph alignment; the observer repairs its base to RTL but leaves it left-aligned instead of `start`. The undo test covers native appearance only and does not assert restored alignment.

- **P2 — UIKit stand-in fields still lose direction changes during composition.** [NativeFieldsIOS.swift:124](/tmp/iosfix2-rv-fd3-astra/host/apple/Sources/ExactKit/IOS/NativeFieldsIOS.swift:124): marked `headerField` instances are skipped, but search-bar and alert-prompt fields are UIKit’s classes, so they never execute `TextField.unmarkText`. Their edit delegates also do not call `applyFieldDirection`. A direction change during composition followed by a commit that changes no text leaves the old base until another style application. The stand-in test checks direct assignment without composition or the real delegate wiring.

Native suites were not rerun: this read-only checkout has no build artifacts. Only the Foundation notification behavior was verified at runtime.

VERDICT: CHANGES
