# Layout facts beyond size: visibility, live position, container/anchor CSS

**Status:** Open
**Systems:** Contract, GUI hosts, layout events
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/127

## Current scope

Select bounded IntersectionObserver-shaped event payload/margin/consumer first. Anchors coordinate with #112; container queries, derive-time frame and general live geometry remain deferred. This is a visibility candidate, not the original umbrella.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
`resize=action` (the element resize event, `ResizeObserver`'s, commit 5949b2b64) works on web and macOS: a pane's width follows the window, and a text's rendered width can be read. What is still missing: row visibility (`IntersectionObserver`), a node's position outside an action, container queries, and anchor positioning beyond an invoker-anchored popover. A position read with `frame()` goes stale as soon as layout moves the node. Ask: add an intersection event and CSS container queries and anchor positioning, so apps do not measure with native hooks.

## Why it matters
A coding-agent desktop app keeps UI attached to moving layout. Examples:
- A slash-command menu stays under a centered composer that slides sideways when a side panel opens. The composer's size does not change, so a resize event never fires.
- A timeline minimap tracks which turns are on screen.
- Sidebar rows subscribe to live data only while near the viewport (an overscan margin).
- Controls compact their labels by the width of their container.
Today apps write a native hook per fact (measure in Swift, report to TypeScript), with hidden probe nodes and values that are `null` until laid out.

## Current behavior (exact2 4c893fef6)
Supported (same on web and macOS):
- `resize=fit` on a `column`: `"paneW": 372` at 420×900, `"paneW": 552` after `resize 600x800`.
- `resize=label` on a `text` with `align-self="flex-start"`: `"labelW": 170.03125`. This measures a rendered node, not a string at a style.
Missing:
- Position outside actions: `frame("anchor")` in a `derive` is refused (`type-geometry-outside-action`, LLP 1051.000 D2). Read in an action at 420 wide, `"anchorX": 135.59375`. After `resize 600x800` the button's box is `space viewport 189.59,42 386.41×18`, and `anchorX` is still 135.59.
- Visibility: no intersection event; `intersect=` is `lower-unknown-attr`, and the docs do not mention `IntersectionObserver`.
- Container queries: `` `column` has no attribute `container-type` ``.
- Anchor positioning: `anchor-name` and `position-anchor` are unknown attributes. `position-area` exists only on an invoker-anchored popover; the schema says "no anchor-name, position-anchor or position-try".

## Expected behavior
Web standards (Chrome), with **proposed** Contract forms:
- **Intersection event** (`IntersectionObserver`): e.g. `intersect=action` with a `rootMargin`-like attribute. Fires when the node enters or leaves its scroll container's viewport plus the margin, with `isIntersecting` and the ratio.
- **Container queries**: `container-type="inline-size"` on a box, and a way for descendants to pick styles by its width (for example a `class=` choice keyed to a container condition). The exact Contract form is a design question.
- **Anchor positioning**: `anchor-name` on a node; `position-anchor`, `position-area` and `anchor()` on an absolute or fixed box, so it follows the anchor through layout and animation with no app code.
- **Live position**, if the above do not cover a case: a resize-like event that also fires when the border box moves, delivered as `resize=` is (no frame of lag on native).
- Text width of an arbitrary string (`measureText`) only if a rendered probe with `resize=` is not enough.
- Hosts: web, macOS, iOS, Linux.

## Reproduction
Minimal app (`app.ts` has no sources):
```contract
component X22LayoutFacts
  state paneW = -1
  state labelW = -1
  state anchorX = -1
  action fit(w: number, h: number)
    paneW = w
  action label(w: number, h: number)
    labelW = w
  action where
    let g = frame("anchor")
    anchorX = g.x
  view
    main padding=24 display="flex" flex-direction="column"
      column testId="pane" resize=fit background-color="#eee"
        text "Model: Large reasoning" testId="label" align-self="flex-start" resize=label
      button "Where is the anchor?" press=where testId="where" id="anchor" margin-left="30%"
      text `pane ${paneW} label ${labelW} anchor ${anchorX}` testId="facts"
```
| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x22-layout-facts`; app above; `contract build --json` | compiler | `[]` | `[]` |
| 2 | `agent web state "tap where" "resize 600x800" state` | web (Chrome) | paneW 372 → 552; labelW 170.03 | same |
| 3 | `bun exact.mjs mac`; same drive with `agent macos` | macOS 26 | same as web | same |
| 4 | `agent web/macos "tap where" "resize 600x800" state "layout where"` | web, macOS 26 | anchorX 135.59; box x 189.59 | a way to follow x = 189.59 |
| 5 | add `derive … frame(…)`, `container-type`, `anchor-name`, `position-anchor`, `intersect=` one at a time | compiler | refused (see Current behavior) | accepted |

### Evidence
The files are attached at the end of this issue, in folded sections.
Quoted from `repro.log`:
```
web/macOS  "paneW": 372 -> "paneW": 552 ; "labelW": 170.03125
web        "anchorX": 135.59375 ; layout where: space viewport 189.59,42 386.41×18
macOS      "anchorX": 135.60000610351562 ; layout where: space viewport 189.6,42 386.4×18
compiler   type-geometry-outside-action ; `column` has no attribute `container-type` ; `button` has no attribute `anchor-name`
```
- `evidence/X22/repro.log`: commands, `state` slots and `layout` lines on both hosts, the compiler probes, the `position-area` vocabulary.
- `evidence/X22/app/`: the minimal app source.

## Acceptance criteria
- An intersection event on rows of a `scroll` fires at the margin while scrolling (agent drive with `tap <scroll> wheel`) on web and macOS, matching Chrome's `IntersectionObserver` order.
- A menu anchored with `anchor-name`/`position-anchor` stays under its anchor after `resize 600x800` and during a 200 ms layout transition (sampled with `clock +N`), within 1 px of Chrome's box.
- A container query switches a label style at a container width, matching Chrome at three window sizes.
- `contract vocab` lists the new names.

## Notes
- Size (`resize=action`) is done and verified here; it was the larger part of the original request.
- Workaround today: native hooks per fact, or a `scroll` event plus `frame()` in an action, re-read on every scroll and resize. Neither follows a node that moves without scrolling or resizing.
- LLP 1051.000 D2 forbids geometry in derives on purpose (layout feedback). Live position should come as an event or as CSS, not as a derive.
- `position: sticky` is a separate issue. Not tested: iOS, Linux.

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef65f1bf8d6e4be6df9265e2ae49790637 (2026-10-05), checkout exact2-repro, macOS 26.6.2
# 5949b2b64 (resize=action) is an ancestor: yes

$ bun exact.mjs contract build app.contract --json
[]

## 1. resize=action (supported): pane width and label width, at 420x900 and after resize 600x800
$ bun exact.mjs agent web state "tap where" "resize 600x800" state
  "slots": {
    "paneW": 372,
    "labelW": 170.03125,
    "anchorX": -1
  },
  "slots": {
    "paneW": 552,
    "labelW": 170.03125,
    "anchorX": 135.59375
  },
$ bun exact.mjs mac; bun exact.mjs agent macos state "tap where" "resize 600x800" state
  "slots": {
    "paneW": 372,
    "labelW": 170.03125,
    "anchorX": -1
  },
  "slots": {
    "paneW": 552,
    "labelW": 170.03125,
    "anchorX": 135.60000610351562
  },
# paneW follows the window (372 -> 552) on both hosts; labelW (a text's width) is 170.03 on both.

## 2. Position outside an action: anchorX was read by frame() when "where" was tapped at 420 wide;
##    after the resize the button moved, and nothing re-reads it.
$ bun exact.mjs agent web "tap where" "resize 600x800" state "layout where"
  "slots": {
    "paneW": 552,
    "labelW": 170.03125,
    "anchorX": 135.59375
  },
  space viewport 189.59,42 386.41×18 · scale 1
$ bun exact.mjs agent macos "tap where" "resize 600x800" state "layout where"
  "slots": {
    "paneW": 552,
    "labelW": 170.03125,
    "anchorX": 135.60000610351562
  },
  space viewport 189.6,42 386.4×18 · frame 189.60000610351562,42 386.3999938964844×18 (kernel, in the parent) · window 189.6,42 386.4×18 · screen 631.6,92 386.4×18 · scale 2
# anchorX stays 135.59 while the button's box is at x=189.59.

## 3. Compiler probes (app.contract with one change each)
derive anchorLive = frame("anchor").x:
[{"id":"type-geometry-outside-action","message":"`frame` reads layout, which only an action may do: a derive, a view or a `fn` that read it would feed layout back into the tree it lays out (LLP 1051.000 D2, LLP 1039 D5)"}]
container-type="inline-size" on the column:
[{"id":"lower-unknown-attr","message":"`column` has no attribute `container-type`"}]
anchor-name="--a" on the button:
[{"id":"lower-unknown-attr","message":"`button` has no attribute `anchor-name`"}]
position-anchor="--a" on an absolute text:
[{"id":"lower-unknown-attr","message":"`text` has no attribute `position-anchor`"}]
an IntersectionObserver-like event (intersect=fit):
[{"id":"lower-unknown-attr","message":"`column` has no attribute `intersect`"}]

## 4. Vocabulary
$ cargo run -q -p contract -- vocab position-area
position-area: style attribute
  rows position_area
  enum none|bottom span-right|bottom|bottom span-all|top span-right|top|top span-all|center, default none
# schema.json _comment: "CSS `position-area` on an invoker-anchored popover (its implicit anchor is the invoker; no anchor-name, position-anchor or position-try)"
$ grep -n "IntersectionObserver\|measureText" docs/*.md
(no output)
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
// X22: what app logic can learn about layout. Size: resize=action. The rest?
component X22LayoutFacts
  state paneW = -1
  state labelW = -1
  state anchorX = -1
  action fit(w: number, h: number)
    paneW = w
  action label(w: number, h: number)
    labelW = w
  action where
    let g = frame("anchor")
    anchorX = g.x
  view
    main padding=24 display="flex" flex-direction="column"
      column testId="pane" resize=fit background-color="#eee"
        text "Model: Large reasoning" testId="label" align-self="flex-start" resize=label
      button "Where is the anchor?" press=where testId="where" id="anchor" margin-left="30%"
      text `pane ${paneW} label ${labelW} anchor ${anchorX}` testId="facts"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X22 Layout Facts",
  "short_name": "X22 Layout Facts",
  "id": "com.example.x22-layout-facts",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x22-layout-facts",
    "name": "X22 Layout Facts"
  },
  "host": {
    "ios": {
      "minimumOS": "17.0",
      "deviceFamily": [
        "iphone",
        "ipad"
      ]
    },
    "macos": {
      "minimumOS": "14.0",
      "window": {
        "width": 900,
        "height": 700
      }
    },
    "web": {}
  },
  "deploy": {
    "store": {
      "web": "0",
      "macos": "0",
      "ios": "0",
      "linux": "0"
    }
  }
}
````

`app.test.contract`

````contract
test "facts"
  expect tree has "facts"
````

`app.ts`

````ts
import type { Answer } from './app.contract.d.ts';

export const appId = 'com.example.x22-layout-facts';
export const grants = '';
export const answer: Answer = () => { throw new Error('no sources'); };
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T07:49:38Z

## Decision needed
**[Design]**: an accepted LLP decides otherwise, or the API has to be chosen first. It needs a ruling before implementation. Re-checked on main `78286adc1` (2026-10-07). It still reproduces. (`type-geometry-outside-action` for `frame()` in a derive; `intersect`, `container-type`, `anchor-name`, `position-anchor` are unknown attributes).

**Blocked by:**
- LLP 1051.000 D2 (geometry only in actions).
- LLP 1039 (`llp/1039-viewport-facts.rfc.md:31,58`: "no container queries").
- LLP 1021 D2 (no `anchor-name`/`position-anchor`).

**Options, separable:**
- **1.** An intersection event (`IntersectionObserver`'s model; e.g. `intersect=action` with a margin). It is event-shaped, so it doesn't run into D2's layout-feedback concern.
- **2.** Anchor positioning (`anchor-name`, `position-anchor`, `anchor()`) on absolute or fixed boxes.
- **3.** Container queries.
- **4.** A live-position event.

**Recommendation:** split this issue. Admit 1 (medium). Decide 2 together with #112. Leave 3 refused unless a consumer outweighs LLP 1039's reasons. Leave 4 out if 1 and 2 land.

### ccheever — 2026-10-08T08:07:47Z

**Decision: Separate intersection visibility from anchor positioning; keep container queries and reactive geometry deferred.**

Keep open with the bounded scope below.

An IntersectionObserver-shaped event is a useful bounded candidate. Anchor placement belongs with #112. Derive-time frame reads or general live-position feedback can create layout loops.

Keep one umbrella with explicit sub-decisions until work starts. Select an event payload/margin and consumer before building visibility; keep container queries out under LLP 1039.
