# macOS: progressbar, status, alert and modal dialog roles are not exposed to accessibility

**Status:** Open
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/278

## Current scope

Fix admitted role mappings and implicit live announcements first; amend LLP 1080.003 D3 for modal subtree projection alongside #282. Check actual AppKit tree/VoiceOver; ARIA modality is not keyboard containment.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

ARIA roles that Contract already carries tell a screen reader what a node is, and the web host hands them to Chrome's accessibility tree:

- `role="progressbar"` on a drawn bar;
- `role="status"` and `role="alert"` (live regions, polite and assertive by their roles);
- `role="dialog"` with `aria-modal`.

On macOS the host maps only buttons, links, checkboxes, radios, switches, images, groups and radio groups to AppKit roles (`NodeView.updateRoleAccessibility`, `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:354-370`), with headings and text through the text path. These nodes become plain static text, or nothing:

- VoiceOver cannot find a drawn progress bar;
- a status or an alert is read as ordinary text;
- a modal dialog does not hide the page behind it from VoiceOver.

### Current and expected behavior

- **Current (macOS, `tree --ax`):**
  - a `box role="progressbar" aria-label="Download"` is absent;
  - `text role="status" aria-live="polite"` and `text role="alert"` are `text`;
  - a `column role="dialog" aria-modal=true aria-label="Setup"` is absent as a dialog, and its children and the page's controls are all exposed side by side.
  - The `progress` element itself is exposed (`progressbar "Working" [busy]`).
- **Current (web, `tree --ax`):** `progressbar "Download"`, `status`, `alert`, and `dialog "Setup" [modal]`, with the header line `modal #11 by dialog:modal`.
- **Expected:** macOS exposes them as the web does:
  - `progressbar` as `NSAccessibility.Role.progressIndicator` with its name (and its value, see #279);
  - `status` and `alert` as live regions announced when their text changes: polite for `status`, assertive for `alert`, with the roles' implicit `aria-live`;
  - a modal `dialog`/`alertdialog` as a dialog group whose `aria-modal` hides its siblings from VoiceOver, reported as `modal` by `tree --ax`.

Notes:
- Main already announces text changes on a node with an explicit `aria-live` (`Accessibility.swift:279-287`, `.announcementRequested`). The role still reads as plain text, and `role="alert"` without `aria-live` is not announced (from reading the source; VoiceOver was not run).
- LLP 1080.003 D3 deferred `aria-modal` on macOS and describes a correct version: return the modal view from the nearest ancestor AppKit flattens, and report it as `modal`.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`:

```text
component A11y
  state stage = "Downloading"
  state failed = false
  action advance
    stage = "Unpacking"
  action fail
    failed = true
  view
    main testId="root" padding=24
      column gap=12
        progress aria-label="Working" testId="spinner"
        box role="progressbar" aria-label="Download" testId="bar" width=200 height=8 background-color="#dddddd"
          box width=60 height=8 background-color="#3366cc"
        text stage role="status" aria-live="polite" testId="stage"
        button press=advance testId="advance"
          text "Advance"
        button press=fail testId="fail"
          text "Fail"
        when failed
          text "Setup failed" role="alert" testId="error"
      column role="dialog" aria-modal=true aria-label="Setup" testId="dlg" margin-top=24 padding=12 border-width=1 border-style="solid" border-color="#999999"
        text "Setting up"
        button testId="dlg-ok"
          text "OK"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| macOS accessibility tree | `bun exact.mjs mac`; `bun exact.mjs agent macos "tap fail" "tap advance" "tree --ax"` | macOS 26.6.2, Apple Silicon | main `0365ad1a4` | 8 elements, in order: `AXScrollArea`; `progressbar "Working" [busy]`; `text "Unpacking"`; `button "Advance"`; `button "Fail"`; `text "Setup failed"`; `text "Setting up"`; `button "OK"`. No `Download` bar, no status, alert or dialog, no modal | as the web's row | `tree --ax` |
| Web accessibility tree | `bun exact.mjs agent web "tap fail" "tap advance" "tree --ax"` | Chrome 154 (the agent's) | same | `progressbar "Download"`; `status` holding "Unpacking"; `alert` holding "Setup failed"; `dialog "Setup" [modal]`; header `modal #11 by dialog:modal` | (the reference) | `tree --ax` |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- On macOS, `tree --ax` for the repro lists:
  - the `Download` progress bar with its label;
  - the stage line as a status (live, polite);
  - the error as an alert (live, assertive);
  - the dialog as a dialog, with `modal` reported and the page's controls outside it hidden.
- A VoiceOver spot check announces the stage change and the error once each.
- Unchanged: iOS (which already honours `aria-modal`), the web and the agent's Contract `tree`.

### Constraints and related work

- Workaround: none in Contract. An app's native module or access hatch could set AppKit roles on its own views only.
- Not tested: VoiceOver itself (the evidence is the platform's exposed tree as the agent reads it).
- Related: #279 (a progress value), LLP 1080.002 (`tree --ax`), LLP 1080.003 D3 (`aria-modal` on macOS deferred), LLP 1069.001 (indeterminate `progress`).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:20Z

**Decision: Complete admitted role mappings, implicit live announcements and macOS modal accessibility.**

Keep open for a correctness fix.

A declared progressbar, status, alert or dialog should not disappear or become undifferentiated text. macOS aria-modal is explicitly deferred by LLP 1080.003 D3, so that part requires its scoped amendment.

Do roles and announcements first, then the modal subtree projection alongside #282. Check AppKit's actual tree and a VoiceOver spot check; ARIA modality alone does not imply keyboard trapping.
