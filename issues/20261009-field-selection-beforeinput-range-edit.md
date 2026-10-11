# Text fields: selectionchange on caret moves, setRangeText and a cancelable beforeinput (rest of #125)

**Status:** Open
**Systems:** Contract, GUI hosts, text editing
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/275

## Current scope

Add field selectionchange/beforeinput and pin range-edit semantics first. Charlie measured setRangeText select/selectionchange without input or a new undo step: original acceptance is superseded. Preserve selectionMode, document deviations, and test IME/noncancelable edits.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #125, split out because #125 closed when its clipboard part landed (#209: a field's ⌘V, ⌘C and ⌘X fire `paste`, `copy` and `cut` first, cancelable). Three DOM editing primitives on `textarea` and `input` are still missing on every host:

- **`selectionchange` on a field.** A plain caret move (arrow keys, a click) is not reported. `select` fires only for a non-empty selection or `setSelectionRange`, and `input` only for an edit.
- **`setRangeText(id, text, start, end)`.** An app cannot replace a range of a field's value as one edit (one `input`, one undo step, the caret placed by the DOM's rules).
- **`beforeinput`.** An app cannot see an edit's `inputType` and `data` before it lands, or cancel it with `preventDefault()` (for example `insertFromPaste`, `historyUndo`).

An editor that turns a typed trigger (`@name`) into a reference, recalls history only when the caret is on the first line, or folds a large paste into an attachment needs all three. Without them such an editor is written natively per platform, and its logic and tests exist twice.

Atomic inline ranges, the other remainder of #125, are refused by `rules/DEFERRED.md` and are filed separately as a policy question.

### Current and expected behavior

Current, on main `0365ad1a4` (the relevant compiler and host files are unchanged on `e200397ec`):

- `selectionchange=` on a `textarea`: `lower-attr-tag` "`selectionchange` belongs to `text`: it reports the part of the reader's text selection inside one paragraph, not `textarea`".
- `beforeinput=` on a `textarea`: `lower-unknown-attr` "`textarea` has no attribute `beforeinput`".
- `setRangeText("editor", "[a.ts]", 4, 9)` in an action: `type-unknown-command` "`setRangeText` is not a host command".
- At run time, after `setSelectionRange("editor", 4, 9)`, two ArrowLeft presses report nothing (the app still holds `select 4-9`); typing `x` then reports `input 4-4`, so the caret had moved without the app knowing. Same on the web and macOS.

Expected (Chrome, by the DOM's names):

- `selectionchange` on a field fires on every caret or selection change, keyboard or pointer, with the field's `selectionStart`, `selectionEnd` and `selectionDirection` (the existing `InputEvent` fields).
- `setRangeText(id, text, start, end)` replaces the range, fires `input` once, places the caret after the text (or by the DOM's `selectMode`), and is one undo step.
- `beforeinput` hands `inputType` and `data`; `preventDefault()` cancels the edit; `historyUndo` and `historyRedo` arrive as input types.

Proposal (a suggestion, not a required design): the payloads reuse `InputEvent` with `inputType` and `data` added; `setRangeText` joins `setSelectionRange` as a host command by the field's `id`.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` answers no sources:

```contract
component TextareaEditing
  state body = "see @a.ts now"
  state caret = "none"
  state selects = 0
  action edited(v: string, e: InputEvent)
    body = v
    caret = `input ${e.selectionStart}-${e.selectionEnd}`
  action selected(e: InputEvent)
    selects = selects + 1
    caret = `select ${e.selectionStart}-${e.selectionEnd}`
  action pick
    setSelectionRange("editor", 4, 9)
  view
    main testId="root" padding=24
      textarea id="editor" testId="editor" value=body input=edited select=selected
      button press=pick testId="pick"
        text "Select @a.ts"
      text caret testId="caret"
      text `selects ${selects}` testId="counts"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Caret move | `bun exact.mjs agent <host> --json "tap pick" state "type editor key ArrowLeft" "type editor key ArrowLeft" state "type editor key x" state` | web (Chrome 154) and macOS 26.6.2 (`bun exact.mjs mac`) | `0365ad1a4` | `caret` stays `select 4-9` after both ArrowLeft presses; `x` gives `seex @a.ts now`, `input 4-4` | a `selectionchange` reporting 3-3 after the second press | agent `state` slots, both hosts identical |
| `selectionchange` on the field | add `action moved(s: Selection)` and `selectionchange=moved` on the `textarea`; `bun exact.mjs contract build app.contract --json` | compiler | `0365ad1a4` | `lower-attr-tag` (quoted above) | compiles; fires on a field | compiler output |
| `beforeinput` | add `beforeinput=selected` on the `textarea`; `contract build --json` | compiler | `0365ad1a4` | `lower-unknown-attr` | compiles; cancelable | compiler output |
| `setRangeText` | replace `pick`'s body with `setRangeText("editor", "[a.ts]", 4, 9)`; `contract build --json` | compiler | `0365ad1a4` | `type-unknown-command` | compiles; one `input`, one undo step | compiler output |

### Acceptance criteria

- In the app above, on the web and macOS: ArrowLeft, ArrowRight and a pointer click in the field each report the new offsets through `selectionchange`.
- `setRangeText("editor", "[a.ts]", 4, 9)` gives `see [a.ts] now` with one `input`; one ⌘Z (Ctrl+Z) restores the old value.
- `beforeinput` with `preventDefault()` on `insertFromPaste` leaves the value unchanged, with a real ⌘V on macOS and in Chrome; `historyUndo` is reported on ⌘Z.
- A conformance case against Chrome over the same steps for the three primitives.
- iOS and Linux: the same, or a stated limit (UIKit's undo calls no delegate, so `historyUndo` may not be observable there).

### Constraints and related work

- Related: #125 (closed by #209, which covers only the clipboard events), #132 (selection on `text`).
- Not tested: iOS, Linux, IME composition, a real keyboard on macOS (the agent delivers keys as platform events).
- A way to end the current undo group has no DOM equivalent; it is left out of this issue's acceptance and needs its own proposal if wanted.
- Workaround today: a native text view per platform with its own delegate, outside Contract and the agent's reach.

## Discussion at transfer

### ccheever — 2026-10-08T08:07:29Z

**Decision: Add field selectionchange and beforeinput; pin range-edit semantics first.**

Keep open with the bounded scope below.

The primitives are useful. Chrome 155 showed setRangeText emits select/selectionchange, no input, and preserves the existing selected range by default. It did not create the requested undo step in the focused-field probe.

Correct the acceptance before implementation. Preserve DOM selectionMode and observable events; if Exact deliberately adds input synchronization or an undoable edit, record that deviation explicitly. Test IME and noncancelable beforeinput cases.
