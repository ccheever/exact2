---
name: 20261007-x49-progress-value-accessibility
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-provider-sign-in-and-install]
upstream_url: null
reproduced_on: 7d3a9d654 (feature branch on main 7fa3fa5b7's framework)
---

# X49: a progress value for assistive technology (`progress`, `aria-valuenow`)

## Summary

T3 Code shows the Antigravity runtime download as a native `<progress value max>` labelled
"Antigravity download". Contract has no `progress` (or `meter`) element and carries no
`aria-valuenow`, `aria-valuemin`, `aria-valuemax` or `aria-valuetext`, so an app can draw the bar
but cannot hand its value to VoiceOver or to the agent tree. The requested support is the HTML
`progress` element (or the ARIA value attributes on any node) on every host.

## Why this issue arose

### The T3 Code behavior
- `ProviderSetupSection.tsx:213-224` (1e2ecbd975): while the runtime downloads and its size is
  known, `<progress aria-label="Antigravity download" value={downloadedBytes} max={totalBytes}>`
  sits under the status text "Downloading 12.3 MB of 45.6 MB.". A screen reader announces a
  progress indicator with its percentage.

### What exact2 does today
- `cargo run -p contract -- vocab progress` (and `meter`): "`progress` is not a tag or an attribute".
- `vocab aria-valuenow` / `aria-valuetext`: "ARIA's, and Contract does not carry it yet; Contract
  carries aria-busy, aria-checked, … aria-selected" (no value attributes).
- `role` is a free string, so `role="progressbar"` is accepted, but without a value.

### Where the clone hits it
`providers-setup.contract` `ProviderRuntimeRow`: the bar is drawn as a track and a fill whose width
is the percentage (`provider-install.ts` `progressPercent`), with `role="progressbar"`,
`aria-label="Antigravity download"` and `aria-description="<n>%"` as the workaround. The visible
bar and the status text match the reference; VoiceOver reads the percentage as a description,
not as the control's value, and the agent tree shows the label only.

## Why it must be resolved

The goal is the reference's behavior, accessibility included. A progress value is a standard
control state (HTML `progress`, ARIA `progressbar`), and every long-running install or upload in
an app needs it. Nonblocking: the status text carries the same numbers.

## Requested support

The HTML `progress` element (`value`, `max`, indeterminate without `value`), or `aria-valuenow`,
`aria-valuemin`, `aria-valuemax` and `aria-valuetext` on any node, mapped to the platform's value
(NSAccessibility `AXValue` on a progress indicator, UIAccessibility `accessibilityValue`, the
web's attributes), and printed by the agent `tree`.

## How to reproduce

`cargo run -q -p contract -- vocab progress` and `vocab aria-valuenow` on the pinned main; a
one-node app `box role="progressbar" aria-valuenow=30` is refused by `contract build`.

## Acceptance for the fix

- `contract build` accepts `progress value=… max=…` (or the ARIA value attributes).
- `bun scripts/agent.mjs macos tree` prints the value; VoiceOver reads "30 percent".

## App adoption after resolution

Replace the drawn bar's `aria-description` with the element or attributes in
`ProviderRuntimeRow`; keep the drawn track and fill only if the native element cannot be styled.

## Status and next action

Local draft (2026-10-07, provider-sign-in-and-install). Reproduced with `contract vocab` on the
feature branch's framework (main `7fa3fa5b7`). Upstream searched by title for "progress",
"aria", "value", "meter", "range", "slider" (`gh issue list --state all`): no match. Not
published: publication needs the user's approval (`issue-open`).
