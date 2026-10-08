---
name: 20261008-x60-number-field-semantics
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/301
reproduced_on: febb2c5fb (main; a one-file app, macOS and the web) and 592657b3a (feat(example)/t3-code-provisional-decisions-parity; the feature branch's framework, main 1f19b2400)
---

# X60: `input type="number"` on macOS is a plain text field (no ArrowUp/ArrowDown stepping, no character filter)

## Summary

On the macOS host, an `input type="number"` with `min`, `max` and `step` (which the compiler accepts,
`contract/lower/src/controls.rs`, LLP 1102 §3.12) behaves as a text field: ArrowUp and ArrowDown leave
its value unchanged, and any character can be typed. In Chrome, the web standard, ArrowUp/ArrowDown
step the value within `min`/`max` (`stepUp`/`stepDown`), and the field refuses characters a number cannot
hold (only digits, `.`, `+`, `-`, `e`, `E` reach it).

## Why this issue arose

### The T3 Code behavior
Settings › Connections › Tailscale HTTPS › "Set up Tailscale HTTPS?" has an "HTTPS port" field that is
`<Input type="number" inputMode="numeric" min={1} max={65_535} step={1}>` (`ConnectionsSettings.tsx`,
reference `1e2ecbd975`); its spin buttons are hidden (`input.tsx`: `[appearance:textfield]`), so it looks
like a text field, but ArrowUp/ArrowDown step it and letters cannot be typed into it.

### What exact2 does today
Driven with the agent on the clone's lane build (2026-10-08, `provisional-decisions-parity`): with
`type="number" min=1 max=65535 step=1` on the field, `type tailscale-port 0443` then
`type tailscale-port key ArrowUp` leaves `value="0443"` (the view tree), where Chrome shows `444`.
The macOS host has no number-field code (`grep -rn stepUp host/apple/Sources` finds nothing; iOS maps
`inputmode` to a keyboard type only, `NodeViewIOS.swift`).

### Where the clone hits it
`connections-network-dialogs.contract` `TailscaleSetupDialog`. Workaround in the clone: the dialog's key
handler steps the value itself (`tsStep`: ArrowUp `floor(n) + 1`, ArrowDown `ceil(n) - 1`, clamped to
1…65535, an empty or unreadable value becomes 1), and validity is the reference's `/^\d+$/` on the trimmed
text. Not worked around: the character filter (a letter typed into the clone's field shows, with the
"Enter a port from 1 to 65535." error under it; the reference's field never shows it).

## Why it must be resolved
The web is the standard (`CLAUDE.md`): a field written as a number field should behave as one on every
host, or an app has to re-implement stepping per field and still cannot filter input.

## Requested support
`input type="number"` on macOS (and iOS): ArrowUp/ArrowDown step by `step` within `min`/`max` as
HTMLInputElement `stepUp`/`stepDown` do, and characters that cannot form a number are refused.

## How to reproduce
One-file app: `input type="number" min=1 max=65535 step=1 value=text input=edit` with
`state text = "443"`. On macOS, focus it and press ArrowUp (agent: `type <id> key ArrowUp`): the value stays
`443` (expected `444`). Type `a`: it appears (expected: refused).

## Acceptance for the fix
ArrowUp on `443` gives `444`, ArrowDown on `1` stays `1`, ArrowUp on `65535` stays `65535`, an empty field
steps to `1`; `a` is refused; the same on the web target.

## App adoption after resolution
Remove `tsStep` and the ArrowUp/ArrowDown branch of `TailscaleSetupDialog.submitKey`.

## Status and next action
Draft, not published (the user files issues). No upstream issue found by title search
(`gh issue list --search "number input step ArrowUp"`, 2026-10-08).

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/301 (#301, [Bug] macOS: `input type="number"` is a plain text
field (ArrowUp/ArrowDown do not step, letters are accepted)). Reproduced on main `febb2c5fb` with a one-file app
(`type="number" min=1 max=65535 step=1`, value "443"), on macOS and on the web. macOS: ArrowUp leaves "443", `key a`
gives "a443", `type port 4a4` gives "4a4", an emptied field stays empty on ArrowUp, and `tree --ax` shows the field as a
`textbox`. The web: "444", "444", "44", "1", the bounds hold, and the role is `spinbutton`. So the issue covers three
things: stepping, the character filter and the accessibility role. Evidence: [x60-arrowup.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-arrowup.png),
[x60-letters.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-letters.png), transcript [x60-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-ops.txt).

Next: `issue-close` once #301 lands: remove `tsStep` and the ArrowUp/ArrowDown branch of
`TailscaleSetupDialog.submitKey`; a typed letter should then never show in the port field.
