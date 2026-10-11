# `focusin`/`focusout` (or `:focus-within`): an ancestor hears the focus enter its subtree

**Status:** Open
**Systems:** Contract, runner events, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/283

## Current scope

Implement approved bubbling focusin/focusout with target/relatedTarget information, without a new Tab stop. Correct the original acceptance: DOM focusout also fires between descendants. Cover programmatic/click/Tab focus and removal.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

A container sometimes needs to know that the keyboard focus entered or left its subtree. Two examples:

- A collapsed, clipped block expands when Tab reaches a link hidden below its fold, so the focus never lands on something out of sight.
- A row reveals its hover-only actions while any control inside it has the focus.

The web gives this with `focusin` and `focusout`, which bubble, and with `:focus-within`. A listener on a `<div>` hears them without the `<div>` joining the Tab order.

Contract carries `focus` and `blur` only, on the element that takes the focus. An element with a `focus` handler becomes a Tab stop of its own (`docs/contract-grammar.md` §Focus order), so a wrapper cannot listen without adding a stop.

### Current and expected behavior

- **Current:**
  - `contract vocab focusin` and `focusout` say "not a tag or an attribute".
  - `focus-within` is not an attribute either.
  - A `column focus=open` around a `link` is itself a Tab stop, and Tab onto the link does not run `open`.
- **Expected:** `focusin` and `focusout` as DOM's (they bubble, payload as DOM's or none), heard by an ancestor without making it focusable, on every host. Or a `:focus-within` fact a binding can read. Either covers both examples above.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`:

```text
component X54Focusin
  state opened = 0
  action open
    opened = opened + 1
  view
    main testId="root" padding=24
      column gap=12
        button testId="before"
          text "Before"
        column focus=open testId="wrapper" padding=8
          text `opened ${opened}` testId="count"
          link href="https://example.com" testId="inner"
            text "A link inside"
        button testId="after"
          text "After"
```

View ids: before 3, wrapper 5, inner (the link) 7, after 9.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Vocabulary | `bun exact.mjs contract vocab focusin`, `vocab focusout`, `vocab focus-within` | any | main `0365ad1a4` | "not a tag or an attribute" (all three) | an event an ancestor can hear | CLI output |
| Tab walk, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --json "type before key Space" state` then `"type root key Tab" state` three times; then `"type before key Space" "type root key Tab" "tree count" "type root key Tab" "tree count"` | macOS 26.6.2 | same | focus 3 → 5 → 7 → 9 (the wrapper is a stop); `opened 1` when the wrapper takes the focus, still `opened 1` when the link does | 3 → 7 → 9, with the ancestor's listener run when the link takes the focus | `state`, `tree` |
| Tab walk, web | the same ops with `agent web` | Chrome 154 (the agent's) | same | 3 → 5 → 7 → 9; `opened 1`, then `opened 1` | same as above | `state`, `tree` |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- `column focusin=open` (or the chosen form) around the link: Tab from `before` goes to the link, `opened` counts 1, and the column is not a stop.
- `focusout` runs when the focus leaves the subtree, and not when it moves between two of its descendants (or the payload says so, as DOM's `relatedTarget` does).
- The same on macOS, iOS with a hardware keyboard, the web and Linux.

### Constraints and related work

- Workaround: make the hidden content's controls reveal themselves with their own `focus` handlers, or put a separate "show more" stop before the content. Neither handles focus from a click or from `focus(id)`.
- Not tested: iOS and Linux.
- Related: #140 (keyboard capture phase, another event-delivery gap).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:27Z

**Decision: Add bubbling focusin/focusout with enough target information.**

Keep open with the bounded scope below.

An ancestor listener must not become a new Tab stop. Correct the acceptance: DOM focusout also fires when focus moves between descendants; a subtree-exit test uses relatedTarget, or a separate focus-within semantic.

Test movement into, within and out of a subtree, focus(), clicks, Tab and removal. Do not silently give a DOM event focus-within-only behavior.
