---
name: 20261005-x43-tristate-switch-mixed
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: [20261005-settings-scoped-controls-and-theme-editor]
upstream_url: null
reproduced_on: null
---

# X43: A tri-state (`mixed`) accessibility value on a switch

## Summary

When the selected settings targets disagree, T3 Code draws a switch in a "mixed" state and exposes `aria-checked="mixed"` so assistive technology reads "mixed" rather than "on" or "off". Exact2 documents `aria-label`, `aria-expanded`, `aria-pressed`, `aria-live`, `aria-hidden`, `inert` and `autofocus`; the clone's switches pass a boolean to `aria-checked`. Whether `aria-checked` accepts `mixed` (or a native checkbox exposes `indeterminate`) is unknown. The mixed look is drawn by the app in either case; the missing piece is the accessible state.

## Why this issue arose

### The T3 Code behavior
- `Switch` takes a `mixed` prop. Mixed draws the thumb centred on a muted track at 70 % opacity and sets `data-mixed`; it sets `aria-checked="mixed"` only when mixed so the real state stays readable otherwise ("Base UI copies every key we pass, even `undefined`, over its own aria-checked", `apps/web/src/components/ui/switch.tsx:12-46`, attribute at `:33`).
- `ScopedSwitch` turns it on when the selected targets (project checkouts, or environments) disagree on the setting keys, and a click turns the setting on for every target, "the macOS mixed-checkbox convention" (`components/settings/ScopedSwitch.tsx:7-19`, `useScopedSettings.ts:43-47`, `scopedSettingsAreMixed`, `scopedSettings.ts:132`).
- It is used for nine settings: `enableDeviceSupport`, `enableAgentDeviceAccess` (`IntegrationsSettings.tsx:772,836`), `autoResumeLimitedThreads`, `snoozeLimitedThreads`, `sidebarAutoSettleOnMerge`, `sidebarAutoSettleAfterDays`, `enableProviderUpdateChecks`, `continueThreadsAfterServerUpdate`, `newWorktreesStartFromOrigin` (`SettingsPanels.tsx:2339-3057`).
- Reference tests: `scopedSettings.test.ts:431,439` ("scoped settings mixed values"). No test checks the attribute.
- Not verified here: the ARIA specification's rule for `mixed` on `role="switch"`; the reference sets it on Base UI's switch root, and some platforms treat the value as a checkbox state. To confirm at `issue-open`.

### What exact2 does today
- Not in `EXACT2-GAPS.md`. The bundled library (`20261005-platforms-v3`, accessibility) lists the documented attributes above and says assistive-technology behavior is not certified by a tree snapshot; `aria-checked` is not listed: unknown. Its layout topic names native `input type="checkbox" switch`; a native switch has no mixed state, a native checkbox can (web `indeterminate`).
- Observed in the clone (mc-orch tree, 2026-10-05): every switch is a `button role="switch" aria-checked=<bool>` with app-drawn track and thumb (`settings-rows.contract:188-189`, `providers.contract:213`, `settings-kit.contract:89`). The clone detects the mixed condition for rows already (`serverState().mixed`, `settings-core.ts:216`, `inheritance: 'mixed'`) but draws and reports on or off (`settings-core.ts:267`, `checked: state.value === true`).

### Where the clone hits it
`20261005-settings-scoped-controls-and-theme-editor` adds the mixed look and the "press turns everything on" rule in app code. The accessible state would read "off" (or "on") for a mixed switch. A sighted user sees the same as the reference; a VoiceOver user hears a different state. No other difference.

## Why it must be resolved

The goal is behavior parity including accessibility (`spec.md` target matrix: `aria-label` and keyboard focus; VoiceOver spot checks where a ticket names them). A mixed switch that reads "on" misreports a setting that is on for some targets and off for others. The rows affected are the nine above (the same acceptance row "Mixed across environments/checkouts" in the settings ticket). Impact: nonblocking (the look and the write behavior match). Keeping the workaround costs a declared accessibility difference in `EXACT2-GAPS.md` and a VoiceOver check by a person. The issue ends when the attribute is accepted and adopted, or by a decision to accept the difference.

## Requested support

On the macOS host first, the web way: `aria-checked` accepting `true`, `false` and `mixed` on `role="switch"` and `role="checkbox"` nodes, mapped to the platform accessibility value (macOS `AXValue` 0/1/2 for a checkbox-like element). Alternative: `input type="checkbox"` with an `indeterminate` property bound to state, rendered as an NSButton checkbox with `allowsMixedState`; it would change the look, so the app would keep its own drawing and use only the semantics. Other hosts: iOS accessibility value "mixed"; web native.

## How to reproduce

To confirm on the pinned `main` at `issue-open`. One-node app: `button role="switch" aria-checked="mixed" testId="s"`. Run `bun exact.mjs contract build app.contract --json`, then `bun exact.mjs agent macos "tree --ax" ...` Reference result (Chrome): accessibility tree shows `checked: mixed`. Expected on exact2: a compile diagnostic (boolean expected) or a tree that shows on/off. Clone scenario: a project with two checkouts that differ for `newWorktreesStartFromOrigin`, Settings › General, inspect the row with `tree --ax`.

## Acceptance for the fix

- `contract build --json` returns `[]` for the three values on a switch and a checkbox.
- Agent: `tree --ax` shows the mixed state for a mixed switch, and on/off after the press that turns it on for every target.
- AppKit test: the node's accessibility value is mixed (2) and changes with state.
- A person confirms with VoiceOver that the row reads "mixed" (an attended row to add to `20261005-settings-scoped-controls-and-theme-editor` when the fix lands).

## App adoption after resolution

Bind `aria-checked` to the row's tri-state in `settings-rows.contract` (and the two Integrations rows in `source-control-view.ts`), delete the declared accessibility deviation from `EXACT2-GAPS.md` and from the Issue assessment of `20261005-settings-scoped-controls-and-theme-editor`. `issue-close` verifies: the mixed row reports mixed in `tree --ax`; the press resolves it; the attended VoiceOver row passes once added.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).
