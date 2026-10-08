---
name: 20261005-x20-rich-text-editing
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-policy
blocks: [20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-terminal-integrations, 20261005-thread-commands-and-keys]
upstream_url: https://github.com/ccheever/exact2/issues/125
reproduced_on: null
rest_upstream_url: [https://github.com/ccheever/exact2/issues/275, https://github.com/ccheever/exact2/issues/276]
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

## Fix built (2026-10-06)

Phase 1 built on exact2 `origin/main`, branch `daehyeon/fw-x20-textarea-editing` (worktree `~/orca/workspaces/exact2/t3-fw-x20`), commits `5467c273a`, `7fe3de309`, `6c9829b11` (two review rounds). Not pushed.
- On `textarea` and `input`, with the DOM's names: `selectionchange=action` (`Selection {start, end, text}`, UTF-16, reported after the edit, coalesced); commands `setSelectionRange(id, start, end)` and `setRangeText(id, text, start, end)` (fires `input`, one undo step on native, caret after the text, the DOM's offset rules, `maxlength` ignored); a cancelable `beforeinput=action` (`InputEvent {inputType, data}`; `preventDefault()` cancels, e.g. `insertFromPaste`; `historyUndo`/`historyRedo` on macOS). Also fixed: ⌘Z in a macOS textarea now reports the undone value to the app. Each macOS `input` has its own field editor while it is being edited.
- Not built (phase 2, needs Charlie's ruling under DEFERRED's "rich value type"): atomic inline chips.
- Known: iOS has no history events (UIKit's undo calls no delegate); a password field's paste arrives as `insertText`; on the web a `setRangeText` past `maxlength` has no undo step; AppKit groups a typed edit and a command in one run-loop pass into one undo step.
- Evidence: the five checks; macOS XCTests (694); the editing conformance pair against Chrome (21/21 steps equal); a scratch app (caret, mention at the caret, refused paste, one undo, maxlength) passes on the web JS target, the wasm web host and macOS; full macOS smoke as main (Caltrain 3/3); two independent reviews, their findings fixed. iOS compiles; not run on a simulator.
- Merge note: its event ABI kind 39 collides with X22's `Resize` (also 39); renumber one at merge.

## Merged upstream; partly fixed (2026-10-07, adopt-main-fixes-r4)

[#125](https://github.com/ccheever/exact2/issues/125) was closed by main #209 (`78e53a813`), in the feature
branch since main `463acda68` ([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)):
on macOS and iOS an `input`'s or `textarea`'s ⌘V, ⌘C and ⌘X fire `paste`, `copy` and `cut` at the
nearest handler before the field's own edit, and `preventDefault()` cancels it as in Chrome; the web hosts
honour that `preventDefault()` too. Nothing else of #125 is on `463acda68`: no `selectionchange` on a field
(still `lower-attr-tag`), no `beforeinput`, no `setRangeText`, no undo grouping, no atomic ranges
(`docs/contract-grammar.md`, "Form controls"). The phase-1 branch above is not merged.

Adoption: none possible. The composer is the native `NSTextView` (`T3Composer*.swift`) because chips are
atomic ranges and the menus replace a range in one undo step, the parts still missing. Its paste handling
(a large paste folds into `pasted-text.txt`, files and images become attachments, ⇧⌘V pastes plain text,
⌘V with nothing editable focused goes to the composer) therefore stays in Swift: #209's `paste` reaches a
Contract field, and the composer is not one. No Contract `input` or `textarea` in the clone needs a paste
handler: the reference's `onPaste` sites are the composer, the terminal's own page, a no-op in
`SettingsFontPreviews` and the window-level redirect to the composer.

## Rest filed upstream (2026-10-08)

Upstream (the rest): https://github.com/ccheever/exact2/issues/275 (#275, [Feature] Text fields: selectionchange on caret moves, setRangeText and a cancelable beforeinput (rest of #125)); https://github.com/ccheever/exact2/issues/276 (#276, [Policy] Atomic inline ranges (chips) over a text field's plain-string value (rest of #125)). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) before filing: after `setSelectionRange`, ArrowLeft reports nothing (no `selectionchange` on a field), `selectionchange` on a `textarea` is `lower-attr-tag`, `beforeinput` is `lower-unknown-attr`, `setRangeText` and `undo` are `type-unknown-command`, `contenteditable` is refused. #276 is blocked by `rules/DEFERRED.md:155-161` ("no rich value type") and has one "Decision needed" comment. Searched open and closed issues and PRs: no duplicate.

## Decided upstream (2026-10-08)

[Charlie on #276](https://github.com/ccheever/exact2/issues/276#issuecomment-6055589942) (closed, not planned): "Keep atomic chips out of plain Contract fields." [Charlie on #275](https://github.com/ccheever/exact2/issues/275#issuecomment-6055583546): "Add field
selectionchange and beforeinput; pin range-edit semantics first."
- **Declared difference (permanent), #276:** the composer's atomic chips stay in the native composer
  (`T3Composer*.swift`, styled ranges over `[label](t3-context://…)`).
- Waits for main fix of [#275](https://github.com/ccheever/exact2/issues/275) for plain Contract fields. It does not retire the native composer: the chips,
  the trigger and menu keys and the history recall live in the same view.
