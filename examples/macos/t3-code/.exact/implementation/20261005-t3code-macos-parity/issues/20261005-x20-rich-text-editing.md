---
name: 20261005-x20-rich-text-editing
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-policy
blocks: [20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-terminal-integrations, 20261005-thread-commands-and-keys]
upstream_url: null
reproduced_on: null
---

# X20: No way to build an editor with inline atomic chips, caret/selection access, range replacement, paste interception and undo grouping

## Summary

The T3 Code composer is a Tiptap/ProseMirror editor: chips for files, skills and citations are single units in
the text, menus replace a typed trigger with a chip, history recall and list continuation rewrite ranges,
large pastes become attachments, and undo steps are grouped by rule. Exact2 has no rich value type, no caret
or selection events on `textarea`, no paste/cut/copy events on macOS, and no undo control. The clone therefore
re-implements the editor in about 2,400 lines of Swift on top of `NSTextView`, instead of reusing T3 Code's
TypeScript logic. Needed: a small set of editing primitives (and a decision on whether a rich value type is
allowed).

## Why this issue arose

### The T3 Code behavior
All references at `1e2ecbd975` under `apps/web/src/`.
- **Atomic inline chips.** `@file`, `$skill`, citations and context references are inline nodes with
  `contentEditable={false}` (`components/ComposerPromptEditorTiptap.tsx:235,396`); a selection across a chip
  highlights it (`composer-chip-range-selected`, `:544`). That the caret crosses a chip in one step and
  Backspace removes it whole is the behavior of a non-editable inline node, to confirm in the oracle (the
  clone's editor header lists "atomic chips"). A chip at a paste boundary is completed with a space (`:1034-1056`).
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
- **Rich text.** With the setting on, Markdown styling and Cmd-B bold.
- Reference tests worth porting: `composer-undo-grouping.test.ts`, `composer-list-continuation.test.ts`,
  `composer-editor-mentions.test.ts`, `composer-rich-text-doc.test.ts`, `composer-rich-text.test.ts`,
  `composer-logic.test.ts` (59 tests). Together the editor is 1,498 lines plus about 1,160 lines of logic.

### What exact2 does today
- GAPS X20 (EXACT2-GAPS.md, earlier sessions, framework source at exact2 `c1522fdac`, checked against `main`
  `d2cb661eb`): "DEFERRED says "no rich value type" (`rules/DEFERRED.md:160`). LLP 1045's editor styles Markdown
  only. A textarea has no caret/selection events (`selectionchange` is on `text` only), and a macOS textarea
  fires no copy/cut/paste (`docs/contract-grammar.md:681-695`). Main added `KeyboardEvent` with
  `preventDefault()` (`8a0afbeab`, `f35b3eafc`), so Enter-to-send and menu keys no longer need native code."
  The summary row: "Rich-text editing with atomic inline nodes; caret/selection read and range replace; paste
  interception; undo groups" / "framework feature (DEFERRED "no rich value type")".
- Bundled library: not covered: unknown.
- Observed in the clone tree (mc-orch, 2026-10-05): the composer is Exact's ordinary `textarea`
  (`composer.contract:88`, hook `t3-composer`) whose `NSTextView` gets a proxy delegate
  (`modules/apple/T3ComposerEditor.swift:1-60`).

### Where the clone hits it
- `modules/apple/T3Composer*.swift` is 2,413 lines. The header of `T3ComposerEditor.swift` lists what it
  re-implements: "trigger detection for the command menus, the menus' keys, list continuation, Tab indent,
  prompt-history recall, the plan toggle chord, atomic chips and undo grouping" (with a 1.0 s undo delay),
  plus large-paste folding (`composer-editor-files.ts`). Chips are styled ranges over the plain string
  `[label](t3-context://v1/<kind>/<id>)` (`composer-editor-menu.ts:244-247`).
- Tests: AppKit binaries `composer` (45), `composer-files` (4), `intent` (4), `attach` (3) cover it; none can
  run T3 Code's own tests, and none is shared with the TypeScript layer.
- What a user sees: the same behavior where the Swift matches; every difference between the Swift and the
  reference editor is a defect that exists only in the clone, found only by driving it.
- What it costs: the plan's logic-reuse rule (port T3 Code's modules with their tests) cannot be applied to
  the largest piece of client logic; each upstream change to the editor (the Ultrathink prefix, list
  continuation fixes, mention completion) is rewritten and re-verified in Swift. A related limit: the caret
  position is not visible to TypeScript, so the ⌥↑ key of `20261005-thread-commands-and-keys` needs a native
  report.

## Why it must be resolved

The goal is to clone T3 Code with its own logic reused. The composer is the one surface where that is not
possible today, and it is the surface users touch most. `20261005-composer-fidelity` changes the prompt
(Ultrathink prefix and ring, queue edit with attachments) and `20261005-diff-review-engine` needs the same kind
of editor for line comments and Cite (quoted text as a chip). Both extend native Swift rather than reuse TS. As
a policy question, `rules/DEFERRED.md:160` ("no rich value type") must be read or waived: the decision needed
from Charlie is whether editing primitives over the plain-string value (option A) are consistent with it, or
whether a rich value type (option B) is acceptable. Cost of keeping the workaround: a permanent second
implementation and no automated parity with T3 Code's editor tests.

## Requested support

The web way: a `textarea` exposes `selectionStart/End`, `setRangeText`, `select`/`selectionchange`, cancelable
`beforeinput` (with `inputType`, including `historyUndo`) and `paste`/`cut`/`copy` with `clipboardData`; a
`contenteditable` surface adds `Selection`/`Range` and non-editable inline nodes. On the macOS host:
- **A (recommended).** Primitives over the existing plain-string value, with no new value type:
  1. caret and selection `start/end` as an event and a readable fact on `textarea` (not only `text`);
  2. `setRangeText(text, start, end)` and `setSelectionRange` as actions, as one undoable edit;
  3. cancelable `beforeinput`, `paste`, `cut` and `copy` events carrying `clipboardData` and `inputType`;
  4. undo control: close/open an undo group, and observe `historyUndo`/`historyRedo`;
  5. **atomic ranges**: the app declares `[start, end)` ranges of the string with a style (icon, label,
     colours); the host draws each as one unit, moves the caret over it in one step, deletes it as a unit, and
     keeps the string unchanged. (No exact web equivalent; closest are `contenteditable=false` inline nodes and
     the CSS Custom Highlight API for styling.)
  Composition (IME marked text) must not count as committed text until it ends.
- **B.** A rich value type with inline nodes (ProseMirror-like). Larger, and needs the DEFERRED waiver.
Trade-off: A is a set of small additions and keeps the app's draft a string (which is what the server stores);
B models the editor exactly but changes the language and the draft store.

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Minimal app: a `textarea` with value `see [a.ts](t3-context://v1/file/x) now` and a pill for the link. Today
   there is no way to (a) read the caret, (b) intercept a paste, (c) replace a range, (d) group undo, (e) make
   the link an atomic unit; with primitives A the same app is a few dozen lines. Expected: all five.
2. Clone: the cost, not a failing step: open `T3ComposerEditor.swift` next to
   `composer-undo-grouping.ts`; there is no TypeScript module to port the reference's tests against.

## Acceptance for the fix
- Minimal app (agent `state` + AppKit test): caret events for keyboard and pointer moves, `setRangeText` with
  undo, a cancelled paste, an undo group boundary, an atomic range that the caret skips and Backspace removes;
  IME composition reports no commit until it ends.
- Clone: port the reference tests `composer-undo-grouping.test.ts`, `composer-list-continuation.test.ts` and
  `composer-editor-mentions.test.ts` to `bun:test` against TypeScript editor logic that drives the primitives;
  the AppKit `composer` binary passes with its renderer role only; the ⌥↑ rule reads the caret in TypeScript.
- Pixel pairs and typing checks of the composer are unchanged against the oracle at 1280×840 and 840×620.

## App adoption after resolution
- Add a follow-up ticket at `issue-close` that ports the editor logic to TypeScript with the reference tests
  and shrinks `T3Composer*.swift` to rendering; reopen `20261005-composer-fidelity` and
  `20261005-diff-review-engine` rows that carry native workarounds.
- Remove the proxy delegate and the duplicated trigger, list, undo and paste code; keep platform-specific
  drawing. `issue-close` verifies the ported tests, the binary and the typing check.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval, and ask for
Charlie's reading of `rules/DEFERRED.md:160`; publication only after approval).
