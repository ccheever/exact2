# `aria-disabled`: a control that reads as disabled and still takes the focus

**Status:** Open
**Systems:** kernel schema, Contract, GUI hosts, accessibility
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/20261010-x70-aria-disabled-focusable.md

## Summary

On the web a control can be unavailable and still be a Tab stop: `aria-disabled="true"`. Tab reaches it,
assistive technology reports it as disabled ("dimmed" in VoiceOver), and the page ignores its press.

Contract has no `aria-disabled`. `contract vocab aria-disabled` says "Contract does not carry it yet", and
`aria-disabled="true"` on a `button` is refused (`lower-unknown-attr`). The only disabled state is
`disabled`, which works as HTML's: the control leaves the Tab order and refuses the focus
(`docs/contract-grammar.md:1201-1204`). So an app must choose between "disabled and skipped by Tab" and
"in the Tab order but reported as enabled".

## Why this arose

T3 Code's sidebar "Check for updates" control (`SidebarUpdatePill`, `1e2ecbd975`,
`apps/web/src/components/sidebar/SidebarUpdatePill.tsx:153-158, 304, 317`) is `aria-disabled` when no
update feed exists, drawn at 60 % opacity. Tab still reaches it and shows its focus ring; a press does
nothing. The Settings nav has the same control.

The clone (`sidebar-icons.contract` `SidebarUpdatePill`, merged in #368) draws it with `disabled=true`, so
Tab skips it. Its label, dimmed look, tooltip and no-op press match. The other option, an enabled button
that ignores its press, keeps the Tab stop but tells assistive technology the control works. The clone
declares the difference in `EXACT2-GAPS.md` ("Desktop update controls (no feed)").

## Reproduction

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` exports an empty `sources`.

```text
component X70app
  state log = "-"
  action note(name: string)
    log = name
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 display="flex" flex-direction="column" gap=12 background-color="#ffffff" color="#111111"
      text `last press: ${log}` testId="out"
      row gap=8 align-items="center"
        button press=note("first") testId="first"
          text "First"
        button disabled=true press=note("update") testId="update"
          text "Check for updates"
        button press=note("last") testId="last"
          text "Last"
```

Drive: `bun exact.mjs agent <macos|web> --json --size 520x160 "tap first" "type first key Tab" state "tap last" "type last key Shift+Tab" state "tap update" "tree out" "tree --ax"`.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| Tab from First, Shift+Tab from Last, macOS | macOS 26.6.2 (25G83), Apple Silicon | main `abc1eadff`; #327 head `14493d253` the same | focus First → Last, Last → First; "Check for updates" is never focused; ax: `disabled` | with `aria-disabled="true"` in place of `disabled=true`: First → Check for updates → Last, and back; ax: disabled and focusable | image (right), record |
| Same, web | Chrome 155 (the agent's) | main `abc1eadff` | the same as macOS | the same | image (middle), record |
| The attribute | `bun exact.mjs contract build` with `aria-disabled="true"` on the button | main and #327 alike | `` [lower-unknown-attr] `button` has no attribute `aria-disabled`; `aria-disabled` is ARIA's, and Contract does not carry it yet `` | accepted | record |
| The web's own behavior | the same three buttons as plain HTML, `<button aria-disabled="true">`, headless Chrome 155 over CDP | — | Tab: First → Check for updates → Last; Shift+Tab back through it; ax: `disabled=true focusable=true` (a `disabled` button: not focusable) | (the reference) | image (left), record |

![X70: Tab from First in Chrome (aria-disabled), Exact web and Exact macOS](https://raw.githubusercontent.com/ccheever/exact2/d4109d63e545054c04bda13fea11338245556553/fw-issues-20261010/x70-tab-chrome-web-macos.png)

Record (contract, the HTML page, every step's focus and accessibility tree):
https://raw.githubusercontent.com/ccheever/exact2/d4109d63e545054c04bda13fea11338245556553/fw-issues-20261010/x70-record.txt

## Constraints

- `kernel/tables/schema.json` is the one place a new row is declared; the refusal is
  `contract/lower/src/lint.rs:152` ("is ARIA's, and Contract does not carry it yet").
- `disabled` keeps HTML's meaning and must not change: a disabled control is never focusable
  (`docs/contract-grammar.md:1201-1204`; macOS `NavigationRules.focusRefusal`,
  `host/apple/Sources/ExactKit/NavigationRules.swift:181-183`), and a disabled node passes the pointer to an
  enabled ancestor (`llp/1005-plan-and-runner-v1.spec.md:490-491`).
- LLP 1115 (`llp/1115-write-the-web-ship-the-platform.principle.md:16`): what the author writes wins. Here
  the author writes ARIA's name for "unavailable but reachable".
- WAI-ARIA 1.2 `aria-disabled`: the element stays focusable and is reported as disabled; it does not stop
  events (the page ignores them).
- No DEFERRED rule refuses an ARIA state.

## Acceptance criteria

- `contract build` accepts `aria-disabled="true"` (and a bound bool) on `button` and on a focusable element
  with a `role`.
- In the repro with `aria-disabled="true"` in place of `disabled=true`, on macOS and the web: Tab from First
  reaches Check for updates, then Last; Shift+Tab walks back through it.
- `tree --ax` reports it disabled and focusable on both hosts (macOS `AXEnabled` false, as Chrome maps it).
- Without `aria-disabled`, nothing changes; `disabled=true` still leaves the Tab order.

## Decision needed

Whether a press on an `aria-disabled` control still runs its `press` action. ARIA's answer is yes (the
author ignores it), which keeps the web's behavior; a host that suppresses it would be friendlier to apps
but differ from the web.
