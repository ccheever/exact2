---
name: 20261007-x49-progress-value-accessibility
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-provider-sign-in-and-install]
upstream_url: https://github.com/ccheever/exact2/issues/279
reproduced_on: 0365ad1a4 (main)
---

# X49: a progress value for assistive technology (`progress`, `aria-valuenow`)

**Status (reclassified 2026-10-08):** Bucket 4, approved, no fix in progress: #279 (ARIA range values first); no PR. The provider follow-up's X49 row waits.

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

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/279 (#279, [Feature] A progress value for assistive technology: determinate `progress` (`value`, `max`) or `aria-valuenow`). Reproduced on main `0365ad1a4` (relevant files unchanged on main `e200397ec`) with a minimal public-API app before filing. `progress value=30 max=100` is refused (`lower-attr-tag`: Exact does not draw the determinate bar yet); `aria-valuenow`, `aria-valuemin` and `aria-valuemax` are `lower-unknown-attr`. Main now has the indeterminate `progress` (LLP 1069.001, amended 2026-10-07), which macOS exposes as `progressbar [busy]` with no value. Searched: progress value, aria-valuenow, progressbar (issues and PRs): no duplicate. The macOS exposure of a drawn `role="progressbar"` box is #278 (X55).

Next: issue-close once #279 lands: replace the drawn bar's `aria-description` in `ProviderRuntimeRow` with the element or attributes.

## Main's indeterminate `progress` (2026-10-08, adopt-main-fixes-r6)

Upstream as [#279](https://github.com/ccheever/exact2/issues/279) (filed 2026-10-08). Main `d82c12252` (LLP 1069.001,
amended 2026-10-07; in the feature branch with adopt-main-fixes-r6's merge of `e200397ec`) adds `progress`: the
platform's activity indicator, role `progressbar` and `aria-busy`. It is indeterminate only: `contract build` refuses
`value` ("a `value` makes HTML's determinate progress bar, which Exact does not draw yet") and `max`, and
`contract vocab aria-valuenow` still answers "ARIA's, and Contract does not carry it yet". The Antigravity bar needs the
value, so nothing is adopted: `ProviderRuntimeRow` keeps its drawn track and fill with `aria-description`.

## Decided upstream (2026-10-08): waits for main fix of #279

[Charlie on #279](https://github.com/ccheever/exact2/issues/279#issuecomment-6055583378): "Add ARIA range values first; then determinate progress."
- Waits for main fix of [#279](https://github.com/ccheever/exact2/issues/279), then an adoption round. On adoption, `ProviderRuntimeRow` swaps `aria-description="<n>%"` for `aria-valuenow`, `aria-valuemin`, `aria-valuemax` and `aria-valuetext`.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): approved feature, medium (ARIA values first, coordinated with #278).
