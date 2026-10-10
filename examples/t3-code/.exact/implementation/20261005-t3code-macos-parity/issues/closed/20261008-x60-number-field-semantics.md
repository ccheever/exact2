---
name: 20261008-x60-number-field-semantics
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/301
reproduced_on: febb2c5fb (main; a one-file app, macOS and the web) and 592657b3a (feat(example)/t3-code-provisional-decisions-parity; the feature branch's framework, main 1f19b2400)
---

# X60: `input type="number"` on macOS is a plain text field (no ArrowUp/ArrowDown stepping, no character filter)

Moved to main `issues/20261009-macos-number-field-behavior.md` (2026-10-09); tracked there.

## Summary

On the macOS host, an `input type="number"` with `min`, `max` and `step` behaves as a text field:
ArrowUp and ArrowDown leave its value unchanged, and any character can be typed. In Chrome,
ArrowUp/ArrowDown step the value within `min`/`max`, and the field refuses characters a number cannot
hold.

## Why it arose

### The T3 Code behavior
Settings › Connections › Tailscale HTTPS › "Set up Tailscale HTTPS?" has an "HTTPS port" field that is
`<Input type="number" inputMode="numeric" min={1} max={65_535} step={1}>` (`ConnectionsSettings.tsx`,
reference `1e2ecbd975`); its spin buttons are hidden (`input.tsx`: `[appearance:textfield]`), so it looks
like a text field, but ArrowUp/ArrowDown step it and letters cannot be typed into it.

### Where the clone hits it
`connections-network-dialogs.contract` `TailscaleSetupDialog`, task
[provisional-decisions-parity](../../tasks/closed/20261008-provisional-decisions-parity.md). Driven with the
agent on the clone's lane build (2026-10-08): with `type="number" min=1 max=65535 step=1` on the field,
`type tailscale-port 0443` then `type tailscale-port key ArrowUp` leaves `value="0443"` (the view tree),
where Chrome shows `444`.

## Clone workaround

The dialog's key handler steps the value itself (`tsStep`: ArrowUp `floor(n) + 1`, ArrowDown `ceil(n) - 1`,
clamped to 1…65535, an empty or unreadable value becomes 1), and validity is the reference's `/^\d+$/` on
the trimmed text. Not worked around: the character filter (a letter typed into the clone's field shows,
with the "Enter a port from 1 to 65535." error under it; the reference's field never shows it). On
adoption, remove `tsStep` and the ArrowUp/ArrowDown branch of `TailscaleSetupDialog.submitKey`.

## Evidence and history

- Reproduced on the clone's lane build (above) on `592657b3a` (the feature branch's framework, main
  `1f19b2400`).
- Filed as [#301](https://github.com/ccheever/exact2/issues/301) ([Bug] macOS: `input type="number"` is a
  plain text field (ArrowUp/ArrowDown do not step, letters are accepted)) on 2026-10-08, reproduced on main
  `febb2c5fb` with a one-file app (`type="number" min=1 max=65535 step=1`, value "443"), on macOS and on
  the web. macOS: ArrowUp leaves "443", `key a` gives "a443", `type port 4a4` gives "4a4", an emptied
  field stays empty on ArrowUp, and `tree --ax` shows the field as a `textbox`. The web: "444", "444",
  "44", "1", the bounds hold, and the role is `spinbutton`. Evidence:
  [x60-arrowup.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-arrowup.png),
  [x60-letters.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-letters.png),
  transcript [x60-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-ops.txt).
- Accepted upstream on 2026-10-08 as a correctness fix ("Implement number-field behavior and its
  adjustable accessible role").
