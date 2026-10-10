---
name: 20261005-x20-rich-text-editing
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-policy
blocks: [20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-terminal-integrations, 20261005-thread-commands-and-keys]
upstream_url: https://github.com/ccheever/exact2/issues/125
reproduced_on: null
rest_upstream_url: [https://github.com/ccheever/exact2/issues/275, https://github.com/ccheever/exact2/issues/276]
---

# X20: No way to build an editor with inline atomic chips, caret/selection access, range replacement, paste interception and undo grouping

Moved to main `issues/20261009-field-selection-beforeinput-range-edit.md` (2026-10-09); tracked there.

Main's file carries #275 (field `selectionchange`, `setRangeText`, `beforeinput`). The chips half, #276, was closed upstream
as not planned (2026-10-08): the native composer's atomic chips are a permanent declared difference.

## Summary

The T3 Code composer is a Tiptap/ProseMirror editor: chips for files, skills and citations are single units in
the text, menus replace a typed trigger with a chip, history recall and list continuation rewrite ranges,
large pastes become attachments, and undo steps are grouped by rule. Exact2 had no rich value type, no caret
or selection events on `textarea`, no paste/cut/copy events on macOS, and no undo control. The clone therefore
re-implements the editor in about 2,400 lines of Swift on top of `NSTextView`, instead of reusing T3 Code's
TypeScript logic.

## Why it arose

### The T3 Code behavior
All references at `1e2ecbd975` under `apps/web/src/`.
- **Atomic inline chips.** `@file`, `$skill`, citations and context references are inline nodes with
  `contentEditable={false}` (`components/ComposerPromptEditorTiptap.tsx:235,396`); a selection across a chip
  highlights it (`composer-chip-range-selected`, `:544`). A chip at a paste boundary is completed with a space (`:1034-1056`).
- **Menus that replace ranges.** Typing `@`, `$` or `/` opens a menu; choosing a row replaces the typed range
  with a chip (`composer-editor-mentions.ts`, `composer-logic.ts`, `expandCollapsedComposerCursor`).
- **Caret and selection reads.** `readSelectionRange` (`ComposerPromptEditorTiptap.tsx:1247`),
  `isCaretAtStart` (used by the Alt-ArrowUp "edit last queued message" key, `ChatView.tsx:7823`),
  `insertTextAtEnd` (type-to-focus, `ChatView.tsx:7608`), and the caret's visual line so prompt history claims
  ArrowUp/ArrowDown only on the first or last visual line (`:101,1451`).
- **Range replacement.** List continuation and Tab indent (`composer-list-continuation.ts`), history recall,
  autocomplete, the Ultrathink prefix rewrite and pastes all replace ranges in one transaction.
- **Paste interception.** `handlePaste` (`:1034`) cancels the default, imports clipboard text and inserts it;
  files and images go to attachments; a large paste folds into a `pasted-text.txt` attachment with the toast
  "Large paste attached as …" (`components/chat/ChatComposer.tsx:5799,5935`).
- **Undo groups.** A new step after 1,000 ms without changes and whenever the kind of edit switches between
  insert and delete; a paste or cut is its own step (`composer-undo-grouping.ts`, `COMPOSER_UNDO_GROUP_DELAY`).
- Reference tests: `composer-undo-grouping.test.ts`, `composer-list-continuation.test.ts`,
  `composer-editor-mentions.test.ts`, `composer-rich-text-doc.test.ts`, `composer-rich-text.test.ts`,
  `composer-logic.test.ts` (59 tests). Together the editor is 1,498 lines plus about 1,160 lines of logic.

### Where the clone hit it
The composer is the surface users touch most, and the one where the plan's logic-reuse rule (port T3 Code's modules with
their tests) cannot apply. `20261005-composer-fidelity` changes the prompt (Ultrathink prefix and ring, queue edit with
attachments) and `20261005-diff-review-engine` needs the same kind of editor for line comments and Cite (quoted text as a chip).
The caret position is not visible to TypeScript, so the ⌥↑ key of `20261005-thread-commands-and-keys` needs a native report.

## Clone workaround

The native composer: Exact's ordinary `textarea` (`composer.contract:88`, hook `t3-composer`) whose `NSTextView` gets a proxy
delegate (`modules/apple/T3ComposerEditor.swift`). `modules/apple/T3Composer*.swift` is 2,413 lines; the header of
`T3ComposerEditor.swift` lists what it re-implements: "trigger detection for the command menus, the menus' keys, list continuation, Tab indent,
prompt-history recall, the plan toggle chord, atomic chips and undo grouping" (with a 1.0 s undo delay), plus large-paste folding
(`composer-editor-files.ts`). Chips are styled ranges over the plain string `[label](t3-context://v1/<kind>/<id>)` (`composer-editor-menu.ts:244-247`).
Its paste handling (a large paste folds into `pasted-text.txt`, files and images become attachments, ⇧⌘V pastes plain text,
⌘V with nothing editable focused goes to the composer) stays in Swift. Tests: AppKit binaries `composer` (45), `composer-files` (4),
`intent` (4), `attach` (3). #275 on main would serve plain Contract fields; it does not retire the native composer, since the chips,
the trigger and menu keys and the history recall live in the same view.

## Evidence and history

- Filed as [#125](https://github.com/ccheever/exact2/issues/125) (2026-10-06).
- A local phase-1 fix was built on 2026-10-06 (branch `daehyeon/fw-x20-textarea-editing`, commits `5467c273a`, `7fe3de309`, `6c9829b11`, not pushed):
  `selectionchange`, `setSelectionRange`, `setRangeText` and a cancelable `beforeinput` on `textarea` and `input`; the editing conformance pair against Chrome
  passed 21/21 steps. Not pursued after the 2026-10-08 decisions.
- #125 was closed by main #209 (`78e53a813`), in the feature branch since main `463acda68`
  ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)): on macOS and iOS an `input`'s or `textarea`'s ⌘V, ⌘C and ⌘X fire `paste`,
  `copy` and `cut` before the field's own edit, and `preventDefault()` cancels it. Adoption: none possible; #209's `paste` reaches a Contract field, and the composer
  is not one. No Contract `input` or `textarea` in the clone needs a paste handler.
- The rest filed as [#275](https://github.com/ccheever/exact2/issues/275) ([Feature]) and [#276](https://github.com/ccheever/exact2/issues/276) ([Policy]) on 2026-10-08,
  reproduced on main `0365ad1a4` before filing: after `setSelectionRange`, ArrowLeft reports nothing; `selectionchange` on a `textarea` is `lower-attr-tag`,
  `beforeinput` is `lower-unknown-attr`, `setRangeText` and `undo` are `type-unknown-command`, `contenteditable` is refused.
