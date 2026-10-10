# `outline: none` on `input` and `textarea`, to remove the focus ring Exact draws on a bare field

**Status:** Open
**Systems:** kernel schema, Contract, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/302

## Current scope

Implement approved bounded outline:none suppression on input/textarea across native and bare/painted hosts. Keep default rings, Tab and accessibility when absent. Coordinate #283; no full outline vocabulary is required.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

On the web an author takes the browser's focus ring off a field with CSS `outline: none` (`outline-style: none`, `outline-width: 0`). The usual reason is that the field draws its own focus look, often on a wrapper (Tailwind's `outline-none` with a `focus-within:` ring on the card around a composer).

LLP 1104 r9 D6 keeps Exact's ring on every bare field and textarea, "what a browser does after `all: unset` with a focus restore (CSS UI 4 §7.2.2)":

- the web host restores the UA outline with `:focus-visible` (`host/web/index.html:52-54`, `outline: revert`);
- macOS draws a 2 pt `keyboardFocusIndicatorColor` ring (`FieldEditingMac.swift:47-67`, `showFieldFocus`: the box painter's border for a field, a layer named `exact.fieldFocus` above a textarea's scroll view);
- Linux paints `field_ring`.

A browser still lets the author remove that ring. Contract has no `outline`:

- `contract vocab outline` answers "`outline` is not a tag or an attribute";
- `outline="none"` on a `textarea` is refused with `[lower-unknown-attr]`.

Before `5b2b77339` a literal `appearance="none"` left the macOS ring off (r4's `fieldStyle` mark). Since D6 r9, nothing does. An app that draws its own focus look therefore shows two: its own and Exact's.

### Current and expected behavior

- **Current (macOS, window pixels):**
  - each of the three fields in the repro shows a 2 pt blue ring when focused: a bare `textarea`, a `textarea appearance="none"` and a bare `input`;
  - the ring sits inside the app's own rounded card;
  - nothing in Contract removes it.
- **Current (web):** the same three show Chrome's focus outline (`:focus-visible`, which a text field matches on any focus). Nothing in Contract removes it either.
- **Expected:**
  - `outline="none"` (or `outline-style="none"` / `outline-width=0`) is accepted on `input` and `textarea`;
  - every host's ring honours it: the web host's restore, the macOS ring, Linux's `field_ring`;
  - a field without it keeps the ring as now.
  - A whole `outline` vocabulary (colour, offset, width) is not needed for this request. `outline: none` is the case apps hit.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>` (its `app.ts` exports an empty `sources`):

```text
component RingProbe
  state text = "A bare textarea"
  state other = "appearance none"
  state line = "A bare input"
  action write(value: string)
    text = value
  action note(value: string)
    other = value
  action edit(value: string)
    line = value
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=16 background-color="#ffffff" color="#111111" font-size=14
      column padding=12 border-radius=16 border-width=1 border-style="solid" border-color="#d4d4d8"
        textarea value=text input=write testId="bare-textarea" height=48 padding=0 border-width=0 background-color="#00000000"
      column padding=12 border-radius=16 border-width=1 border-style="solid" border-color="#d4d4d8"
        textarea value=other input=note testId="none-textarea" appearance="none" height=48 padding=0 border-width=0 background-color="#00000000"
      column padding=12 border-radius=16 border-width=1 border-style="solid" border-color="#d4d4d8"
        input value=line input=edit testId="bare-input" padding=0 border-width=0 background-color="#00000000"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Focus each field, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 420x320 tree "tap bare-textarea" "type bare-textarea key ArrowRight" "screenshot bare.png window" "tap none-textarea" "type none-textarea key ArrowRight" "screenshot none.png window" "tap bare-input" "type bare-input key ArrowRight" "screenshot input.png window" tree` | macOS 26.6.2 (25G83), Apple Silicon | main `febb2c5fb` | a 2 pt blue ring inside the card on each focused field, `appearance="none"` included | a way to draw no ring (`outline="none"`) | images 1-3 (right), transcript |
| Same, web | `bun exact.mjs agent web --size 420x320` with the same operations (no `window`) | Chrome 154 (the agent's) | same | Chrome's focus outline on each focused field | the same way to draw none | images 1-3 (left) |
| The vocabulary | `bun scripts/exact.mjs contract vocab outline`; `contract build` of `textarea value=t input=w outline="none" testId="x"` | any | same | "`outline` is not a tag or an attribute"; ``[lower-unknown-attr] `textarea` has no attribute `outline` `` | accepted on `input` and `textarea` | transcript |

Image 1: the bare textarea focused.

![X61: bare textarea focused, web vs macOS](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-bare.png)

Image 2: the `appearance="none"` textarea focused.

![X61: appearance=none textarea focused, web vs macOS](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-none.png)

Image 3: the bare input focused.

![X61: bare input focused, web vs macOS](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-input.png)

Transcript (the contract, the commands, the agent's replies and the compiler's refusal): https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x61-ops.txt

Seen first in an app whose reference removes the ring with `outline-none` and draws focus on the card around its composer and its settings preview field. The composer is focused most of the time the app is used, so the extra ring is the most visible difference from the reference on its main screen.

### Acceptance criteria

- `contract build` accepts `outline="none"` on `input` and `textarea`.
- The repro, with `outline="none"` on all three fields, shows no ring when each is focused:
  - on macOS (`screenshot … window`);
  - on the web in Chrome;
  - on Linux.
- Without `outline="none"`, every field keeps its ring as today, Tab still reaches each field, and the accessibility tree is unchanged.

### Constraints and related work

- Accessibility: as on the web, removing the ring is the author's choice and the author's duty to replace (WCAG 2.4.7). An app that removes it needs to know when an ancestor holds the focus to draw its own look; see #283 (`focusin`/`focusout` or `:focus-within`).
- Workaround: none. `appearance="none"` no longer removes the ring on macOS (D6 r9), and a field drawn without the ring would need a native module or an access hatch on every host.
- Not tested: Linux, iOS (D6: an iOS field shows the caret, no ring), a bare `button` (which the same row would reasonably cover).
- Related: LLP 1104 D6, `5b2b77339` (macOS: Exact draws the ring for a bare field and textarea), #283, #179 (closed: a ring for a custom pressable).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:27Z

**Decision: Add the requested outline:none suppression.**

Keep open with the bounded scope below.

Keep the default ring and let an author replacing it suppress it with the CSS name. A full outline color/offset vocabulary is unnecessary for this request.

Cover native and bare input/textarea paths plus painted hosts. Pair with #283 for an accessible custom wrapper focus look; Tab and the accessibility tree must remain intact.
