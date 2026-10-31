# Code review: Apple's plain text fields take CSS direction as their base (LLP 1001 §1), round 1 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `17af0e693d`.
- **Method:** one brief (sha256 `9b21b99453a512240eda84dab1b619ec0303377914839f13c6dd16ddd68a7680`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
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

I'll review the host change against the approved plan and the diff from the merge base, starting with the red tests and the direction/alignment paths you called out. No files will be edited.This is a committed-range review of `d0910f93df..HEAD`. I'll confirm the worktree, load the reviewer brief, and collect that diff before launching the review.The range matches `origin/main` (`d0910f93df..17af0e693d`, 11 files). The plan file is not at the path given, so the review will use the diff and the conditions in the request.The implementation diff is small enough to trace call by call. Next I’ll read the field-editor, detach, and test paths against the conditions in the request.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open on two paths that can change the verdict: whether the Mac field editor's alignment setter emits an edit, and whether an iOS field rewrite moves the caret while editing. I'll read those call sites before judging.The remaining verdict hinges on the field-editor alignment setter and the iOS attribute write while a field is editing. I'll read those paths in the tip.No findings. The Mac and iOS plain-field change matches the web definition: an explicit paragraph base (`.leftToRight` or `.rightToLeft`), an unsaid alignment of start, and a direction change that restyles text and typing attributes without ending the Mac editing session.

**Base direction.** `applyBaseDirection` (`TextAreaMac.swift:356`) copies an existing paragraph style and writes only `baseWritingDirection`, so a run with no style gets a fresh one and other paragraph attributes stay. It updates storage, typing attributes, and `defaultParagraphStyle`, and it goes through `textStorage` `beginEditing`/`endEditing` rather than `shouldChangeText`, so it does not post an edit or push undo. That is the same enumeration `TextEngine.setBaseDirection` already uses (`Text.swift:885`). The field editor is included (`(tv as? TextArea)?.markup == nil` is true for a non-area). Marked text skips the write (`NativeFieldsMac.swift:133`, `TextAreaMac.swift:491`).

**Mac editing.** `syncFieldSide` (`NativeFieldsMac.swift:95`) writes the field's alignment and base only when `currentEditor()` is nil. While editing, `styleFieldEditor` (`NativeFieldsMac.swift:125`) sets the editor. `NSTextView.setAlignment(_:range:)` does not record undo unless the caller brackets it with `shouldChangeText`, and this call does not. `typedText` (`FieldComposition.swift:35`) returns when the string is unchanged, so an alignment write is not an `input`. The deferred block (`FieldEditingMac.swift:45`) is `[weak self]` and no-ops if a new editor has the field. A remake ends editing, then `styleNativeField` styles the new editor if one is current.

**Lent fields.** Toolbar and alert fields skip `styleNativeField` on purpose (`NativeFieldsMac.swift:69`, `NodeViewMac.swift:1222`): AppKit draws and places them. Begin-editing still runs `styleFieldEditor`, so a focused search or alert field gets the base. The secure editor is AppKit's, and the same path covers it.

**iOS and tvOS.** The paragraph write (`NativeFieldsIOS.swift:116`) is a subscript write-back of `defaultTextAttributes`, same as kern, after font and `textColor` are set, and only when the base or alignment differs. `fieldBaseDirection` sits outside the tvOS-excluded search chrome. A textarea direction change goes through `addAttributes` (`TextAreaIOS.swift:382`), which keeps the selection; the input test checks the attribute, not the caret.

**Detach and alignment.** Both detach paths stamp the field base after `markup` is cleared (`TextAreaMac.swift:471`, `TextAreaIOS.swift:513`). `plainAttributes` carry line height and no base, so the explicit base replaces `.natural` and keeps the line height. An unsaid Mac alignment is start, including the new `"left"` case (`NativeFieldsMac.swift:106`). An `appearance: none` textarea applies a said `text-align` and otherwise stays natural on top of the explicit base (`TextAreaMac.swift:517`), which is the web's start. `field-direction.test.mjs` covers `bare-rtl` and `bare-rtl-left`.

**Tests.** Mac glyph checks use a Latin-opening string under `rtl` and an Arabic-opening string under `ltr`, and the second paragraph opens in Latin (`FieldDirectionMacTests.swift:97`). The iOS second-paragraph attribute opens in Arabic, so that one line does not separate an explicit base from first-strong; the first paragraph's glyph order does, and `applyLineHeight` stamps one style across the storage. Placeholder scans sit on the first line (Mac: top of a flipped cache; iOS: the guard that already passed on main). They prove side, which is all an LTR placeholder can prove.

VERDICT: APPROVE
