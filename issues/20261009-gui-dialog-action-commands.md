# `showModal(id)` and `close(id)` from an action on macOS and the web

**Status:** Open
**Systems:** Contract, GUI hosts, menus
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/282

## Current scope

Carry showModal(id)/close(id) on GUI hosts through invoker modal semantics, including inertness, focus, Tab, Escape and close events. Bare close() remains quit. Coordinate issues/20261009-ios-confirmation-shape.md for action-driven iOS dialogs; its alert-shape design remains separate.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

A modal dialog that an app opens from its own logic needs the focus behaviour of the web's `HTMLDialogElement.showModal()`, not only one opened by an invoker button. Examples are a confirmation a menu item asks for, a form a keyboard shortcut opens, or a prompt a server event raises. That behaviour is:

- the focus moves into the dialog;
- Tab and Shift+Tab stay inside it;
- Escape closes it;
- closing it gives the focus back.

LLP 1101.001 P5 names the API, `showModal(id)` and `close(id)` as host commands by the dialog's id. The compiler accepts `showModal` (`contract/syntax/src/lib.rs:115-118`), but only the terminal host carries it (`host/terminal/src/host.rs:247`).

### Current and expected behavior

- **Current, invoker form:** `button commandfor="im" command="show-modal"` opens `dialog id="im"` as a modal on macOS and the web. The focus moves to its first control, Tab cycles inside it, and Escape closes it and returns the focus to the invoker.
- **Current, from an action (macOS):** `showModal("sm")` logs `app: exact: unknown command showModal`. The dialog stays closed and the focus stays on the button that ran the action.
- **Current, from an action (web):** the journal logs `refused: showModal is not a command this runtime carries`.
- **Current, by hand:** an overlay drawn as `column role="dialog" aria-modal=true` with two buttons keeps no Tab inside on either host. Tab from its last button goes to the page's first button (`focus.logical` 22 → 3 on macOS and on the web). LLP 1080.003 §4 leaves keyboard containment out, pointing to `<dialog>` with `showModal`.
- **Expected:** `showModal(id)` from an action opens the dialog on macOS and the web with the invoker form's behaviour. `close(id)` closes it, gives back the focus and fires its `close` handler.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`:

```text
component X53Modal
  action openModal
    showModal("sm")
  view
    main testId="root" padding=24
      column gap=12
        button press=openModal testId="open-sm"
          text "showModal from an action"
        button commandfor="im" command="show-modal" testId="open-im"
          text "Invoker show-modal"
        button testId="outside"
          text "Outside"
      dialog id="sm" testId="sm"
        button testId="sm-first"
          text "First in sm"
        button testId="sm-last"
          text "Last in sm"
      dialog id="im" testId="im"
        button testId="im-first"
          text "First in im"
        button commandfor="im" command="close" testId="im-close"
          text "Close im"
```

View ids: open-sm 3, open-im 5, im-first 15, im-close 17.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| `showModal` from an action, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos "tap open-sm" "clock settle" logs` | macOS 26.6.2, Apple Silicon | main `0365ad1a4` | `t=0 command showModal("sm")`, `app: exact: unknown command showModal`; `focus.logical` stays 3 | `sm` open, focus on `sm-first` | the drive's `logs`, `state` |
| `showModal` from an action, web | `bun exact.mjs agent web "tap open-sm" "clock settle" logs` | Chrome 154 (the agent's) | same | `refused: showModal is not a command this runtime carries` | same | the drive's `logs` |
| Invoker form, macOS | `agent macos --json "tap open-im" "clock settle" state "type im-first key Tab" state "type im-close key Tab" state "type im-close key Escape" "clock settle" state` | macOS | same | focus 15 → 17 → 15, then 5 after Escape | (the behaviour asked for) | `state` replies |
| Invoker form, web | same ops with `agent web` | Chrome | same | focus 15 → 17 → 15, then 5 after Escape | (the behaviour asked for) | `state` replies |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- On macOS and the web, `tap open-sm` opens `sm` as a modal with the focus on `sm-first`.
- Tab from `sm-last` returns to `sm-first`, and Shift+Tab from `sm-first` goes to `sm-last`.
- Escape, or `close("sm")` from an action, closes it and returns the focus to `open-sm`, and the dialog's `close` handler runs.
- The page behind is inert while the dialog is open, as with the invoker form.
- `showModal` of an id that is not a `dialog`, or that is already open, is refused or ignored as HTML's `showModal()` is, with a journal line.
- Linux follows, or refuses with a named journal line.

### Constraints and related work

- Workaround: per-dialog `key` handlers on the first and last controls to wrap Tab, an explicit `focus()` into the dialog and back to the opener, and Escape bound with `aria-keyshortcuts`. Every dialog repeats it, and a control outside the dialog that is not inert (a toast) is still a stop.
- The API is LLP 1101.001 P5's, as built on the terminal host. Bare `close()` stays quit.
- Not tested: iOS.
- Related: LLP 1021 (dialogs), LLP 1080.003 §4 (keyboard containment not in `aria-modal`).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:21Z

**Decision: Carry showModal(id) and close(id) on GUI hosts.**

Keep open for a correctness fix.

The compiler already accepts these terminal-host commands, and GUI invoker dialogs already have the needed modal behavior. Complete the host dispatch rather than asking apps to build their own focus loops.

Share the invoker path for inert background, initial focus, Tab containment, Escape, close events and focus restoration. Preserve bare close() as quit.
