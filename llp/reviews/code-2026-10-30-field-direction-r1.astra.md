# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `17af0e693d`.
- **Method:** one brief (sha256 `9b21b99453a512240eda84dab1b619ec0303377914839f13c6dd16ddd68a7680`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes.
- **Disposition:** fixed in b02b4ed8a6, each with a test that fails before the fix

## Brief

Code review, round 1 of 3 (host change, macOS and iOS). Branch tip 17af0e693d (HEAD of this worktree); merge-base d0910f93df. Review `git diff d0910f93df..HEAD`: 5 commits, the red tests first.

Approved plan (theme2, 2026-10-30 10:45Z): ~/x2theme/iosfix2-apple-field-direction-plan.md, if readable. The QUEUE item: Apple's plain text fields took each paragraph's base direction from its first strong character, whatever CSS `direction` said, and the Mac's unsaid text-align was left even under rtl. Apple text (TextEngine.setBaseDirection, 0cb3cfbda0) and the Markdown editor (LLP 1045 D11) already take the CSS direction as an explicit base.

The change:
- **Base direction.** Mac and iOS plain fields set the node's direction as each paragraph's EXPLICIT base (.leftToRight or .rightToLeft, never natural):
  - Mac: Field, SecureField, SearchField, the session's FieldEditor, TextArea;
  - iOS: UITextField's defaultTextAttributes and TextAreaIOS.applyLineHeight's paragraph;
  - a Markdown editor's text once its markup goes (detach).
- **Alignment.** The Mac's unsaid text-align becomes start (fieldAlignment default, plus a "left" case that was missing). A Mac textarea of appearance:none now applies an explicit text-align (it was ignored), and otherwise stays natural, which now follows the explicit base.
- **Placeholder.** A plain Mac TextArea's drawn placeholder takes the base and side.
- **Run-time change.** A direction change re-applies to the text and the typing attributes without moving the selection.
  - On the Mac, NSTextField's and its cell's alignment setters end an editing session once the node's layout direction has changed, so syncFieldSide() sets the field's alignment and base only when no editor has it: in styleNativeField, and one run-loop turn after editing ends.
  - The editor takes both meanwhile (styleFieldEditor -> applyBaseDirection, editor.alignment).
- **Composition.** Marked text is untouched.
- **Docs.** LLP 1001 §1's declaration is narrowed to Linux (and LLP 1045's matching line); the QUEUE line goes.

Theme2's conditions:
(1) Web parity: host/web/tests/field-direction.test.mjs builds the same kinds of field for the web host, in both directions, with unsaid and explicit text-align; computed direction, unicode-bidi normal, no dir=auto, and a screenshot of where "Hi" draws. It passes, so the definition is the web's.
(2) Red-first tests: FieldDirectionMacTests (6) and FieldDirectionIOSTests (6). The Mac and iOS base, detach and run-time-change tests fail on main. iOS alignment and the iOS placeholder were already right on main (guards).
(3) Gates or baselines with an rtl plain field: none. The only rtl field in any contract is markdown-stress's atom-rtl-editor, a Markdown editor, unchanged.
(4) Full Mac and iOS suites and caps on the tip: caps green; the suites are running.

Check hard:
- applyBaseDirection's attribute enumeration (ranges with no paragraph style, preserving other paragraph attributes, typing and default styles, no edit events, undo stack untouched?);
- the Mac field-editor and NSTextField interplay (any path where editing still ends, or the selection moves; the deferred syncFieldSide after the view or session goes; a field remade);
- the SecureField editor, the search field, toolbar and alert lent fields (pageField paths skip styleNativeField: OK?);
- iOS: defaultTextAttributes replacing font or colour or kerning, a UITextField being edited, tvOS (NativeFieldsIOS also compiles for tvOS);
- the Markdown detach paths on both platforms;
- the appearance:none text-align change on the Mac (is it the web's behaviour; any regression);
- whether the tests prove what they claim (the placeholder pixel checks, glyph order).
Do NOT edit files. Give findings with file:line and severity. End with one line: VERDICT: APPROVE or VERDICT: CHANGES.

## Review

1. **P2 — Undo can restore the previous CSS direction.** [TextAreaMac.swift:520](/tmp/iosfix2-rv-fd1-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:520): delete all text under `ltr`, switch to `rtl`, then Undo. AppKit restores the deleted text’s old paragraph attributes; the plain-text change callbacks never reapply the current direction. An in-memory TextKit 2 probe confirmed restored text had an LTR base while typing attributes remained RTL. Reconcile direction after undo/redo without adding undo entries.

2. **P2 — Direction changes during composition are dropped.** [TextAreaMac.swift:513](/tmp/iosfix2-rv-fd1-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:513), [TextAreaIOS.swift:557](/tmp/iosfix2-rv-fd1-astra/host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:557): both style paths return while text is marked. After composition ends, callbacks restore Markdown/decorations but never restyle a plain textarea’s paragraphs or typing attributes. Changing direction during composition therefore leaves the old base until another style update. The deferred direction needs applying on commit and cancellation.

3. **P2 — The iOS input update writes into active composition.** [NativeFieldsIOS.swift:120](/tmp/iosfix2-rv-fd1-astra/host/apple/Sources/ExactKit/IOS/NativeFieldsIOS.swift:120): unlike the textarea path, this new write has no `markedTextRange` guard. UIKit’s SDK specifies that `defaultTextAttributes` applies attributes to the full text range, so changing direction rewrites the marked paragraph too. Defer this update during composition and apply it afterward. The runtime input test never focuses or composes in the field.

4. **P2 — Lent Mac fields miss direction updates.** [NativeFieldsMac.swift:82](/tmp/iosfix2-rv-fd1-astra/host/apple/Sources/ExactKit/Mac/NativeFieldsMac.swift:82): `pageField != nil` bypasses this synchronization and `styleFieldEditor`; `applyStyle` also excludes lent fields. Toolbar search fields initially retain their platform base until focused, and direction changes while editing a toolbar or alert field never reach its editor. Give direction synchronization a path independent of page chrome styling.

5. **P2 — Mac Markdown detach loses explicit alignment.** [TextAreaMac.swift:471](/tmp/iosfix2-rv-fd1-astra/host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:471): `detach` restores `plainAttributes` with natural alignment, and `applyBaseDirection` changes only the base. A props-only `markup="none"` update on an RTL field with `text-align: left` therefore produces right-aligned text. Reapply the plain field’s alignment as well; the current detach test covers only LTR with unsaid alignment.

6. **P2 — Mac glyph assertions switch away from the production layout engine.** [FieldDirectionMacTests.swift:55](/tmp/iosfix2-rv-fd1-astra/host/apple/tests/ExactKitTests/FieldDirectionMacTests.swift:55): accessing `layoutManager` permanently converts a TextKit 2 textarea to TextKit 1. The alignment assertions do this too. An AppKit probe confirmed the conversion, so these checks do not establish glyph order/alignment for the shipped textarea. Measure through TextKit 2 or rendered pixels.

No files changed. `git diff --check` passed. Native suites were not rerun in this read-only session; the probes exercised AppKit independently.

VERDICT: CHANGES.
