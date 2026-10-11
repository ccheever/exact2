---
name: 20261007-x49-progress-value-accessibility
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-provider-sign-in-and-install]
upstream_url: https://github.com/ccheever/exact2/issues/279
reproduced_on: 0365ad1a4 (main)
---

# X49: a progress value for assistive technology (`progress`, `aria-valuenow`)

Moved to main `issues/20261009-aria-range-accessibility-values.md` (2026-10-09); tracked there.

## Summary

T3 Code shows the Antigravity runtime download as a native `<progress value max>` labelled
"Antigravity download". Contract has no determinate `progress` and carries no `aria-valuenow`,
`aria-valuemin`, `aria-valuemax` or `aria-valuetext`, so the clone can draw the bar but cannot hand
its value to VoiceOver or to the agent tree.

## Why it arose

- **T3 Code.** `ProviderSetupSection.tsx:213-224` (1e2ecbd975): while the runtime downloads and its size is
  known, `<progress aria-label="Antigravity download" value={downloadedBytes} max={totalBytes}>` sits under the
  status text "Downloading 12.3 MB of 45.6 MB.". A screen reader announces a progress indicator with its percentage.
- **Where the clone hit it.** [provider-sign-in-and-install](../../tasks/closed/20261005-provider-sign-in-and-install.md)
  (2026-10-07), `providers-setup.contract` `ProviderRuntimeRow`. Nonblocking: the status text carries the same numbers.

## Clone workaround

The bar is drawn as a track and a fill whose width is the percentage (`provider-install.ts` `progressPercent`), with
`role="progressbar"`, `aria-label="Antigravity download"` and `aria-description="<n>%"`. The visible bar and the status
text match the reference; VoiceOver reads the percentage as a description, not as the control's value, and the agent
tree shows the label only. Once main carries ARIA range values, `ProviderRuntimeRow` swaps `aria-description` for
`aria-valuenow`, `aria-valuemin`, `aria-valuemax` and `aria-valuetext`; the open
[provider-sign-in-verification-followup](../../tasks/closed/20261008-provider-sign-in-verification-followup.md) holds that row.

## Evidence and history

- Local draft (2026-10-07), reproduced with `contract vocab` on the feature branch's framework (main `7fa3fa5b7`):
  `progress`, `meter`, `aria-valuenow` and `aria-valuetext` are not carried; `role="progressbar"` is accepted
  without a value.
- Filed as [#279](https://github.com/ccheever/exact2/issues/279) on 2026-10-08, reproduced on main `0365ad1a4`:
  `progress value=30 max=100` is refused (`lower-attr-tag`); `aria-valuenow`, `aria-valuemin` and `aria-valuemax`
  are `lower-unknown-attr`. The macOS exposure of a drawn `role="progressbar"` box is #278 (X55).
- Main `d82c12252` (in the feature branch with [adopt-main-fixes-r6](../../tasks/closed/20261008-adopt-main-fixes-r6.md)'s
  merge of `e200397ec`) adds an indeterminate `progress` only (role `progressbar`, `aria-busy`; `value` and `max`
  refused). Nothing adopted: the Antigravity bar needs the value.
- Decision on #279 (2026-10-08): ARIA range values first, then determinate progress.
