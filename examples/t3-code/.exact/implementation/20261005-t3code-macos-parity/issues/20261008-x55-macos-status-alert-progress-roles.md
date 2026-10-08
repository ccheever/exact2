---
name: 20261008-x55-macos-status-alert-progress-roles
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-portable-app-download]
upstream_url: https://github.com/ccheever/exact2/issues/278
reproduced_on: 0365ad1a4 (main)
---

# X55: the macOS host exposes no live region, alert, dialog or progress bar to accessibility

## Summary

Contract carries `role="status"`, `aria-live="polite"`, `role="alert"`, `role="dialog"`/`"alertdialog"`
with `aria-modal`, and `role="progressbar"` with `aria-label` (the agent `tree` prints them as
`accessibilityRole`, `accessibilityLive`, `accessibilityModal`, `accessibilityLabel`). The macOS host's
`NodeView.updateRoleAccessibility` maps only buttons, links, checkboxes, radios, switches, images and
groups (headings and text through `updateTextAccessibility`), so on macOS these nodes are plain static
text or nothing: VoiceOver does not announce a status change or an alert, does not find the progress
bar, and the agent's `tree --ax` lists none of them. Related to X49 (the bar's value).

## Why this issue arose

The first-launch view (20261005-portable-app-download item 3, `first-launch.contract`) asks for a
progress bar labelled "Setting up T3 Code", a stage line that is a polite live region, an error that
is an alert, and a modal dialog. `bun scripts/agent.mjs macos … "tree --ax first-launch"` on the lane
build (2026-10-08): the elements are the heading ("Setting up T3 Code…", AXHeading) and the stage line
(AXStaticText); the progress bar is absent; `modal.present` is false; in the failure state the error
is AXStaticText and only the two buttons are interactive. The Contract `tree` of the same nodes has
`accessibilityRole: progressbar`, `accessibilityLive: polite`, `accessibilityRole: status`/`alert`
and `accessibilityModal: true`.

## Requested support

Map them on macOS as the web's accessibility tree does: `progressbar` to `NSAccessibility.Role.progressIndicator`
with its label (and X49's value), `status` and `aria-live` to a live region
(`NSAccessibility.post(element:notification: .announcementRequested)` or AXLiveRegion attributes)
announced when its text changes, `alert` to an announced live region of assertive priority, and a
modal `dialog`/`alertdialog` to an `AXGroup` with the dialog subrole whose `aria-modal` the agent's
`modal` reports.

## How to reproduce

Build the clone (`bun host/apple/build.mjs t3-code-macos --bundle`), then with an empty lane home:
`EXACT_MAC_BIN="<bundle>/Contents/MacOS/T3 Code (Exact)" T3_LOCAL_HOME=<empty> T3_LOCAL_PORT=16351
T3_LOCAL_UNPACK_DELAY_MS=6500 bun scripts/agent.mjs macos --json "clock +1500 real" "tree first-launch" "tree --ax first-launch"`.

## Acceptance for the fix

`tree --ax first-launch` lists the progress bar with its label, the stage line as a live region,
the failure's error as an alert and `modal.present: true`, and a VoiceOver spot check announces a
stage change and the failure.

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/278 (#278, [Bug] macOS: progressbar, status, alert and modal dialog roles are not exposed to accessibility). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) with a minimal public-API app before filing. One-file app (an indeterminate `progress`, a `box role="progressbar"`, `text role="status" aria-live="polite"`, `text role="alert"`, `column role="dialog" aria-modal=true`): macOS `tree --ax` lists the `progress` element (`progressbar "Working" [busy]`), plain `text` for the status and the alert, and no drawn bar, dialog or modal; the web lists `progressbar "Download"`, `status`, `alert` and `dialog "Setup" [modal]`. On main an explicit `aria-live` posts `.announcementRequested` on macOS (`Accessibility.swift:279-287`); the roles are still unmapped (`NodeViewMac.swift:354-370`), and LLP 1080.003 D3 deferred `aria-modal` on macOS. Searched: live region, role alert, aria-modal macOS, progressbar: no duplicate. Local record only on this branch (copied from the closed #260 branch).

Next: issue-close once #278 lands: re-run `tree --ax first-launch`.

## Decided upstream (2026-10-08): waits for main fix of #278

[Charlie on #278](https://github.com/ccheever/exact2/issues/278#issuecomment-6055581241): "Complete admitted role mappings, implicit live announcements and macOS modal accessibility."
- Waits for main fix of [#278](https://github.com/ccheever/exact2/issues/278), then an adoption round. The clone's `role="status"`/`"alert"` nodes, the drawn progress bar and its dialogs then reach VoiceOver; there is no workaround to remove.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): approved correctness with a scoped amendment (roles first; `aria-modal` needs LLP 1080.003's amendment).
