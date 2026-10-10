# macOS: date, time and select inputs are not Tab stops

**Status:** Open
**Systems:** host/apple macOS, controls, focus
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/280

## Current scope

Put date/time/select in default forward/backward Tab order with platform keys/change events. Verify real input and disabled/tabindex=-1 exclusions.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

On the web, `<input type="date">`, `<input type="time">` and `<select>` are sequential focus stops: Tab reaches each of them in tree order (Chrome stops in each of a date or time input's fields). On macOS, Tab skips all three. A keyboard user cannot reach a date, time or select control in an Exact app on macOS without the pointer, for example in a form dialog.

### Current and expected behavior

- **Current (macOS):** the key-view loop skips `input type="date"`, `input type="time"` and `select`. Focus goes from the control before them straight to the next text field or button, and Shift+Tab skips them the same way.
- **Current (web):** each of the three is a Tab stop in tree order.
- **Expected:** on macOS each of the three is a Tab stop in tree order, as on the web, and once focused it takes the keys its AppKit control takes.

Hypothesis, from reading the source: `Presenter.tabbable` (`host/apple/Sources/ExactKit/Mac/PresenterMac.swift:1259-1265`) admits fields, text areas, native modules with a focus target, buttons, toggles and pressables, then `canBecomeKeyView`. A `Control` node of kind date, time or select matches none of them. A possible change is to admit the `Control` kinds that are not radios, in tree order, as the web host does.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`, with this `app.contract` (`app.ts` answers no sources):

```text
component X52Tab
  state day = "2026-10-08"
  state at = "09:30"
  state unit = "minutes"
  state note = ""
  action setDay(v: string)
    day = v
  action setAt(v: string)
    at = v
  action setUnit(v: string)
    unit = v
  action setNote(v: string)
    note = v
  view
    main testId="root" padding=24
      row gap=8 testId="fields"
        button testId="f-before"
          text "Before"
        input type="date" value=day change=setDay aria-label="Date" testId="f-date"
        input type="time" value=at change=setAt aria-label="Time" testId="f-time"
        select value=unit change=setUnit aria-label="Unit" testId="f-unit"
          option "Minutes" value="minutes"
          option "Hours" value="hours"
        input value=note input=setNote aria-label="Text" testId="f-text"
        button testId="f-after"
          text "After"
```

View ids from `tree`: f-before 3, f-date 5, f-time 6, f-unit 7, f-text 10, f-after 11.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Tab forward, macOS | `bun exact.mjs mac`, then `bun exact.mjs agent macos --json "type f-before key Space" state` followed by `"type root key Tab" state` four times | macOS 26.6.2, Apple Silicon, development build | main `0365ad1a4` | `focus.logical`: 3 → 10 → 11 → 3 (date, time and select skipped) | 3 → 5 → 6 → 7 → 10 → 11 | the drive's `state` replies |
| Shift+Tab, macOS | `"type f-after key Space" state`, then `"type root key Shift+Tab" state` three times | same | same | 11 → 10 → 3 | 11 → 10 → 7 → 6 → 5 → 3 | the drive's `state` replies |
| Tab forward, web | `bun exact.mjs agent web --json "type f-before key Space" state`, then `"type root key Tab" state` twelve times | Chrome 154 (the agent's) | same | 3 → 5 (×4, the date's fields) → 6 (×4, the time's fields) → 7 → 10 → 11 → 3 | (this is the reference) | the drive's `state` replies |

The relevant files are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- On macOS, the repro's Tab walk reads f-before → f-date → f-time → f-unit → f-text → f-after, and Shift+Tab walks the reverse.
- A focused date, time or select control on macOS takes its platform keys (arrows step the value; Space or the arrows open the select's menu), and `change` fires as on a pointer edit.
- A disabled or `tabindex=-1` control stays out of the order, as on the web.
- The web is unchanged.

### Constraints and related work

- Workaround: an explicit `tabindex=0` on the control. With it, the macOS walk reads 3 → 5 → 10, so the date input becomes a stop. Whether it then takes its platform keys was not checked. The workaround asks every app to mark controls that are stops by kind on the web.
- Not tested: iPadOS with a hardware keyboard, and real (non-agent) Tab key presses. The agent delivers the key through the platform's key path.
- Related: #179 (a focused custom pressable's ring on macOS, fixed by #189).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:18Z

**Decision: Put date, time and select controls in the default Tab order.**

Keep open for a correctness fix.

These admitted controls should be reachable without every app adding tabindex=0.

Drive forward/backward Tab with real input; verify disabled/tabindex=-1 exclusions and that focused controls accept their platform keys.
