# macOS: `input type="number"` is a plain text field (ArrowUp/ArrowDown do not step, letters are accepted)

**Status:** Open
**Systems:** host/apple macOS, controls, accessibility
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/301

## Current scope

Implement approved stepping, numeric editing and adjustable accessibility. Compare Chrome for bounds/step, empty/partial values, paste, fractions and exponents; preserve IME rather than digits-only filtering.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

HTML's number state (`<input type="number">`, HTML §4.10.5.1.12) is a field for a number. The web host renders Contract's `input type="number"` as that element, and Chrome:

- steps the value with ArrowUp and ArrowDown (`stepUp()`/`stepDown()` by `step`, kept within `min` and `max`; an empty field steps from 0, then into `min`…`max`);
- refuses characters that cannot form a number as they are typed or inserted;
- exposes the field as a `spinbutton`.

Contract accepts the form with numeric `min`, `max` and `step` (`docs/contract-for-agents.md`, "A number field's (`input type="number"`, written so) `min`, `max` and `step` take numbers"; LLP 1102 §3.12). On macOS it is an ordinary text field:

- the arrows do not step;
- any character can be typed;
- the platform sees a plain text box.

The Apple host has no number-field code: `grep -rn stepUp host/apple/Sources` finds nothing, the macOS field distinguishes only `type="password"` (`NodeViewMac.swift:897`), and iOS maps `number` only to a keyboard (`NodeViewIOS.swift:1116`). An app that wants a port or a quantity field has to re-implement stepping in a key handler on every field, and it still cannot keep letters out.

### Current and expected behavior

- **Current (macOS):** a `type="number" min=1 max=65535 step=1` field at "443", focused:
  - ArrowUp leaves "443" (the agent's reply: `"key":"ArrowUp","value":"443"`);
  - `key a` gives "a443";
  - `type port 4a4` gives "4a4";
  - an emptied field stays empty on ArrowUp;
  - `tree --ax` shows `textbox "Port" value="443"`.
- **Current (web):**
  - ArrowUp gives "444";
  - `key a` leaves "444";
  - `type port 4a4` gives "44";
  - an emptied field becomes "1" on ArrowUp;
  - "65535" stays on ArrowUp and "1" stays on ArrowDown;
  - `tree --ax` shows `spinbutton "Port" value="443"`.
- **Expected:** on macOS:
  - ArrowUp and ArrowDown step by `step` within `min`/`max`, as `stepUp`/`stepDown` do, with one `input` event per step;
  - characters a number cannot hold are refused, as Chrome refuses them (digits, a sign, `.`, `e`/`E` reach the field);
  - the accessibility tree names the field as an adjustable number (the web's `spinbutton`; AppKit's incrementor), or the issue records why not.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>` (its `app.ts` exports an empty `sources`):

```text
component X60
  state text = "443"
  action edit(value: string)
    text = value
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="light-dark(#ffffff, #111111)" color="light-dark(#111111, #eeeeee)" font-size=14
      input type="number" min=1 max=65535 step=1 value=text input=edit aria-label="Port" testId="port" width=200
      text `value: ${text}` testId="echo"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Step, refuse a letter, bounds, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 420x200 "tap port" "type port key ArrowUp" tree "type port key a" tree "type port 4a4" tree "screenshot x60-macos.png" "type port 65535" "type port key ArrowUp" tree "type port 1" "type port key ArrowDown" tree` | macOS 26.6.2 (25G83), Apple Silicon | main `febb2c5fb` | "443" after ArrowUp; "a443" after `key a`; "4a4" after `type port 4a4`; 65535 and 1 unchanged (nothing steps) | "444"; "444"; "44"; 65535; 1 | images 1 and 2, transcript |
| Same, web | `bun exact.mjs agent web --size 420x200` with the same operations | Chrome 154 (the agent's) | same | "444"; "444"; "44"; "65535"; "1" | (the reference) | images 1 and 2, transcript |
| Empty field, ArrowUp | `bun exact.mjs agent <macos\|web> --size 420x200 "tap port" "type port key Backspace" "type port key Backspace" "type port key Backspace" tree "type port key ArrowUp" tree` | as above | same | macOS "" then ""; web "" then "1" | "1" (`min`) | text above |
| Accessibility tree | `bun exact.mjs agent macos --size 420x200 "tree --ax"` | as above | same | `textbox "Port" value="443"` (web: `spinbutton "Port" value="443"`) | an adjustable number | transcript |

Image 1: ArrowUp on "443".

![X60: ArrowUp, web 444 vs macOS 443](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-arrowup.png)

Image 2: `type port 4a4`.

![X60: typed 4a4, web 44 vs macOS 4a4](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-letters.png)

Transcript (the contract, the commands and the agent's replies on both hosts): https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x60-ops.txt

Seen first in an app's "HTTPS port" field (`type="number" inputmode="numeric" min=1 max=65535 step=1`, spin buttons hidden), which steps with the arrows and refuses letters in its reference (a browser).

### Acceptance criteria

- On macOS, for the repro: ArrowUp on "443" gives "444"; ArrowDown on "1" stays "1"; ArrowUp on "65535" stays "65535"; ArrowUp on an empty field gives "1"; each step fires `input`.
- `key a` and `type port 4a4` leave no letter in the field ("444" and "44").
- `tree --ax` on macOS names the field as an adjustable number.
- The web is unchanged. iOS: at least the character filter, with its decimal pad unchanged.

### Constraints and related work

- Workaround: a `key` handler on each field that steps the value itself and clamps it, and validation of the text in `change`. Nothing in Contract can refuse a typed character before it shows (#275: no cancelable `beforeinput`).
- Not tested: iOS, Linux, a fractional `step`, `step="any"`, a locale decimal comma.
- Related: #275 (a cancelable `beforeinput`), LLP 1102 §3.12 (numeric `min`/`max`/`step` on a number field), LLP 1104 (number fields among the native field types).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:17Z

**Decision: Implement number-field behavior and its adjustable accessible role.**

Keep open for a correctness fix.

Contract already accepts number/min/max/step; plain text editing on macOS does not meet that promise.

Use Chrome as the oracle for stepping, bounds, empty and partial values, fractional steps, paste and exponents. Preserve IME and numeric-keyboard behavior; avoid a digits-only filter.
