# Popovers cannot flip near a window edge: no position-try-fallbacks, few position-area values

**Status:** Open
**Systems:** kernel placement, Contract, GUI hosts, menus
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/112

## Current scope

Preserve Charlie's selected next core investment: invoker-popover position-area and flip-block/inline/start fallback order. Amend LLP 1021 no-flip scope; resolve macOS-only clamp against CSS. Check all edges/scroll/resize against Chrome; coordinate #127 without admitting all anchor CSS.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

## Summary
A popover anchored to its invoker accepts only eight `position-area` values (`none`, `bottom span-right`, `bottom`, `bottom span-all`, `top span-right`, `top`, `top span-all`, `center`). The compiler refuses `position-try-fallbacks`, the `left`/`right`/`span-left`/logical areas, `position-anchor`, `anchor-name` and `position-visibility`. So a popover near the window's bottom-right corner cannot flip above-left of its button as Chrome does. On macOS it is clamped into the window and covers its own button; on the web it overflows the window. Ask: CSS Anchor Positioning's fallbacks (`position-try-fallbacks` with `flip-block`/`flip-inline`) and the remaining `position-area` values, on every host.

## Why it matters
A coding-agent desktop app shows tooltips, hover cards and menus next to controls everywhere in the window: a menu at the right edge, a tooltip at the top edge, a hover card in a narrow side panel. Web UI libraries flip and shift these against the window edge automatically. Without fallbacks the popup covers the control that opened it (macOS) or runs off-screen (web), and the result changes with window size. Apps today compute a side and alignment per call site from measured or estimated text widths and pad end-aligned menus with transparent "tails". That arithmetic breaks when text, fonts or the window size change and must be redone for every new popover.

## Current behavior (exact2 4c893fef6)
- `contract vocab position-area`: `enum none|bottom span-right|bottom|bottom span-all|top span-right|top|top span-all|center`. `kernel/tables/schema.json` (`position_area`): "no anchor-name, position-anchor or position-try … Clamped to the viewport; never flipped" (LLP 1021 D2, §5).
- The compiler refuses `position-area="bottom span-left"` (`lower-css-position-area`: "Other areas (left, right, a corner, span-left, logical keywords) are not implemented by the native top layers; a flip is `position-try`, also not implemented") and `position-try-fallbacks` (`lower-unknown-attr`).
- Window 420×600, an 80×32 button at `332,560`, a 320×120 popover (1 px border) with `position-area="bottom span-right"`:
  - macOS `layout pop`: `space viewport 98,478 322×122`: clamped into the window; it covers the button (`332,560 80×32`).
  - web `layout pop`: `space viewport 332,592 322×122`: 8 px inside the window, the rest past the right and bottom edges. Chrome gives the same for this CSS without fallbacks (`plain: 332,592`).
- So macOS and the web already disagree for the same app (the macOS clamp is a declared deviation).

## Expected behavior
Chrome 154, same geometry and CSS (`position: absolute`, `position-area: bottom span-right`):
- with `position-try-fallbacks: flip-block, flip-inline, flip-block flip-inline`: `90,438 322×122`: above the button and ending at its right edge, inside the window, not covering the button.
Proposals:
- **Contract (proposal):** admit `position-try-fallbacks` with `flip-block`, `flip-inline`, `flip-start` and their combinations, and the rest of `position-area` (`left`, `right`, corners, `span-left`, `span-top`/`span-bottom`, logical keywords), on invoker-anchored popovers.
- **All hosts (proposal):** choose the first fallback whose box fits the inset-modified containing block, as Chrome does; with no fallback that fits, keep the base position. macOS, iOS and Linux then match Chrome; the macOS-only clamp becomes either a declared default fallback or goes away (a decision for LLP 1021 §5).
- Later (proposal): `position-visibility: anchors-visible`, and an available-size value (`anchor-size()`) for `max-width`.

## Reproduction
Minimal app (`app.ts` has no sources; `probe.contract` adds the refused attributes):
```contract
component X17PopoverFlip
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=8 justify-content="flex-end" align-items="flex-end"
      button popovertarget="pop" testId="open" width=80 height=32
        text "Open"
      column id="pop" popover="auto" position="absolute" position-area="bottom span-right" width=320 height=120 margin=0 padding=0 border="1px solid #888" background-color="#ffffff" testId="pop"
        text "Popover 320×120"
```
| Step | Command / action | Host | Actual | Expected |
|---|---|---|---|---|
| 1 | `bun scripts/exact.mjs new target/repro/x17-popover-flip`, add files, `contract build --json` | any | `[]` | `[]` |
| 2 | `contract build probe.contract --json` (`position-area="bottom span-left" position-try-fallbacks="flip-block, flip-inline"`) | any | `lower-css-position-area`, `lower-unknown-attr` | accepted |
| 3 | `bun exact.mjs mac`; `agent macos "resize 420x600" "tap open" "clock settle" "layout open" "layout pop"` | macOS 26 | pop `98,478 322×122`, over the button | with fallbacks: `90,438 322×122` |
| 4 | `agent web "resize 420x600" "tap open" "clock settle" "layout pop"` | Chrome (JS target) | pop `332,592 322×122`, past the edges | with fallbacks: `90,438 322×122` |
| 5 | `bun .exact/chrome-anchor.mjs` (plain HTML, same CSS) | Chrome 154 | `plain: 332,592`, `flip: 332,592`, `both: 90,438` | reference |

### Evidence
The files are attached at the end of this issue, in folded sections.
- `evidence/X17/repro.log`: the vocab answers, the compiler diagnostics, both `layout` outputs (`space viewport 98,478 322×122` on macOS, `space viewport 332,592 322×122` on the web), the web popover's computed CSS (`"positionArea":"span-right bottom","position":"absolute","inset":"0px 0px 0px 0px"`), and Chrome's rectangles.
- `evidence/X17/app/`: app.contract, app.ts, app.json, probe.contract, chrome/anchor.html, chrome/chrome-anchor.mjs, chrome/web-open.mjs.

## Acceptance criteria
- `probe.contract`-style attributes compile.
- With `position-try-fallbacks: flip-block, flip-inline, flip-block flip-inline`, `layout pop` reads `90,438 322×122` (±1 px) on macOS and the web at 420×600; at a window where the base position fits, it keeps the base position.
- A web conformance case compares the hosts with Chrome at several window sizes and anchor corners (top edge → flips below; right edge → flips inline).

## Notes
- `flip-block, flip-inline` alone does not move the box in Chrome here (`flip: 332,592`): neither single flip fits, so the combined `flip-block flip-inline` option is needed.
- Not tested: iOS, Linux, `role="menu"` popovers (native `NSMenu`/`UIMenu` place themselves), popovers inside clipping ancestors.
- Policy: LLP 1021 §5 limits the set on purpose ("not implemented by the native top layers"); a decision on the fallback algorithm and on the macOS clamp is needed before implementation.

---

<details><summary>Evidence: <code>repro.log</code></summary>

````text
exact2 4c893fef6 (origin/main, 2026-10-05); macOS 26 (Darwin 25.6.0); Chrome/154 (playwright-core channel chrome)

$ bun scripts/exact.mjs new target/repro/x17-popover-flip   # then app.contract, app.ts as in app/
$ bun exact.mjs contract build app.contract --json
[]
$ bun exact.mjs contract vocab position-area
position-area: style attribute
  rows position_area
  enum none|bottom span-right|bottom|bottom span-all|top span-right|top|top span-all|center, default none
$ bun exact.mjs contract vocab position-try-fallbacks
position-try-fallbacks: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; a hyphenated tag (`map-view`) is a native module, admitted when app.json's `modules` declares it; its other attributes are the module's props
$ bun exact.mjs contract vocab position-anchor
position-anchor: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; a hyphenated tag (`map-view`) is a native module, admitted when app.json's `modules` declares it; its other attributes are the module's props
$ bun exact.mjs contract vocab anchor-name
anchor-name: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; a hyphenated tag (`map-view`) is a native module, admitted when app.json's `modules` declares it; its other attributes are the module's props
$ bun exact.mjs contract vocab position-visibility
position-visibility: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; a hyphenated tag (`map-view`) is a native module, admitted when app.json's `modules` declares it; its other attributes are the module's props

## probe.contract: app.contract with position-area="bottom span-left" position-try-fallbacks="flip-block, flip-inline"
$ bun exact.mjs contract build probe.contract --json
[{"col":58,"end_col":71,"file":"probe.contract","id":"lower-css-position-area","line":7,"message":"`position-area=\"bottom span-left\"`: exact2 places an invoker's popover in a subset of CSS `position-area`: none, bottom span-right, bottom, bottom span-all, top span-right, top, top span-all, center. Other areas (left, right, a corner, span-left, logical keywords) are not implemented by the native top layers; a flip is `position-try`, also not implemented","related":[]},{"col":91,"end_col":113,"file":"probe.contract","id":"lower-unknown-attr","line":7,"message":"`column` has no attribute `position-try-fallbacks`","related":[]}]

## macOS
$ bun exact.mjs mac
host/apple: target/repro/x17-popover-flip/target/clients/d67b77599a4520879f57822f/com.example.x17-popover-flip/macos/aarch64-apple-darwin/embedded/development/standalone/ExactMac (cargo 52.7 s, swift 4.3 s, arms 1.2 s, modules 0.0 s, package 0.4 s; signed 0F0D033A); GPU: no GPU crate; web arm: libexact_web.dylib
$ bun exact.mjs agent macos "resize 420x600" "tap open" "clock settle" "layout open" "layout pop" tree
{"windowFrame":[420,632],"contentLayout":[420,600],"resized":[420,600],"native":"NSWindow.setContentSize","delivery":"platform-window","toolbar":false,"contentView":[420,600],"backingScale":2,"paint":"displayIfNeeded; presentation unobserved","viewport":[420,600],"carrier":"macos","epoch":1,"incarnation":1,"clock":0}
{"delivery":"platform","at":[372,576],"tapped":2,"target":"open","carrier":"macos","mode":"agent","epoch":1,"incarnation":1,"clock":0}
viewport 420×600 · clock 0 ms
node #2 [open] Pressable · site 1 · epoch 1 · incarnation 1
  white_space = normal (initial)
  space viewport 332,560 80×32 · frame 332,560 80×32 (kernel, in the parent) · window 332,560 80×32 · screen 822,634 80×32 · scale 2
  visible clipped=false hidden=false inViewport=true inert=false
viewport 420×600 · clock 0 ms
node #4 [pop] View · site 3 · epoch 1 · incarnation 1
  position_area = bottom span-right (authored, own)
  white_space = normal (initial)
  space viewport 98,478 322×122 · frame 90,470 322×122 (kernel, in the parent) · window 98,478 322×122 · screen 588,552 322×122 · scale 2
  visible clipped=false hidden=false inViewport=true inert=false
View#1 [root]
  Pressable#2 [open] [focused]
    Text#3 "Open"
  View#4 [pop]
    Text#5 "Popover 320×120"

## web (exact2 JS target)
$ bun exact.mjs agent web "resize 420x600" "tap open" "clock settle" "layout open" "layout pop" tree
{"resized":[420,600],"viewport":[420,600],"delivery":"browser-viewport","carrier":"web","clock":0,"epoch":1,"incarnation":1}
{"at":[372,576],"tapped":2,"target":"open","delivery":"platform","carrier":"web","mode":"agent","clock":0,"epoch":1,"incarnation":1}
viewport 420×600 · clock 0 ms
node #2 [open] Pressable · site 1 · epoch 1 · incarnation 1
  white_space = normal (initial)
  space viewport 332,560 80×32 · scale 1
  visible clipped=false hidden=false inViewport=true inert=false
viewport 420×600 · clock 0 ms
node #4 [pop] View · site 3 · epoch 1 · incarnation 1
  position_area = span-right bottom (authored, own)
  white_space = normal (initial)
  space viewport 332,592 322×122 · scale 1
  visible clipped=false hidden=false inViewport=true inert=false
View#1 [root]
  Pressable#2 [open] [focused]
    Text#3 "Open"
  View#4 [pop]
    Text#5 "Popover 320×120"

## exact2 web target, the popover's own CSS (app/chrome/web-open.mjs)
$ EXACT_APP_DIR=$PWD bun .exact/web-open.mjs
{"open":true,"positionArea":"span-right bottom","position":"absolute","inset":"0px 0px 0px 0px","rect":"332,592 322x122"}

## Chrome, plain HTML (app/chrome/anchor.html): same geometry and the same authored CSS (position: absolute, margin 0, 320x120 + 1px border), popover shown with showPopover({source: button})
## plain: position-area: bottom span-right
## flip:  + position-try-fallbacks: flip-block, flip-inline
## both:  + position-try-fallbacks: flip-block, flip-inline, flip-block flip-inline
$ bun .exact/chrome-anchor.mjs
{"chrome":"Chrome/154.0.0.0","open":"332,560 80×32","plain":"332,592 322×122","flip":"332,592 322×122","both":"90,438 322×122"}
````
</details>

<details><summary>Evidence: minimal app sources (<code>app/</code>)</summary>

`app.contract`

````contract
// X17: a popover anchored below a button in the window's bottom-right corner.
component X17PopoverFlip
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=8 justify-content="flex-end" align-items="flex-end"
      button popovertarget="pop" testId="open" width=80 height=32
        text "Open"
      column id="pop" popover="auto" position="absolute" position-area="bottom span-right" width=320 height=120 margin=0 padding=0 border="1px solid #888" background-color="#ffffff" testId="pop"
        text "Popover 320×120"
````

`app.json`

````json
{
  "$schema": "../../../scripts/app.schema.json",
  "name": "X17 Popover Flip",
  "short_name": "X17 Popover Flip",
  "id": "com.example.x17-popover-flip",
  "start_url": "/",
  "display": "standalone",
  "app": {
    "id": "com.example.x17-popover-flip",
    "name": "X17 Popover Flip"
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

`app.ts`

````ts
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.x17-popover-flip';
export const grants = '';

const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
````

`chrome/anchor.html`

````html
<!doctype html>
<meta charset="utf-8">
<style>
  html, body { margin: 0; height: 100%; }
  #root { box-sizing: border-box; height: 100%; padding: 8px; display: flex; flex-direction: column; justify-content: flex-end; align-items: flex-end; }
  #open { width: 80px; height: 32px; }
  .pop { position: absolute; margin: 0; padding: 0; width: 320px; height: 120px; border: 1px solid #888; background: #fff; }
  #plain { position-area: bottom span-right; }
  #flip { position-area: bottom span-right; position-try-fallbacks: flip-block, flip-inline; }
  #both { position-area: bottom span-right; position-try-fallbacks: flip-block, flip-inline, flip-block flip-inline; }
</style>
<div id="root"><button id="open" popovertarget="flip">Open</button></div>
<div id="plain" class="pop" popover="manual">no fallbacks</div>
<div id="flip" class="pop" popover="manual">flip-block, flip-inline</div>
<div id="both" class="pop" popover="manual">flip-block, flip-inline, flip-block flip-inline</div>
````

`chrome/chrome-anchor.mjs`

````js
// Chrome's placement of the same anchor/popover pair, with and without position-try-fallbacks.
import pw from '../../../../node_modules/playwright-core/index.js';
const browser = await pw.chromium.launch({ channel: 'chrome' });
const page = await browser.newPage({ viewport: { width: 420, height: 600 } });
await page.goto('file://' + new URL('./chrome/anchor.html', import.meta.url).pathname);
const out = await page.evaluate(() => {
  const r = (e) => { const b = e.getBoundingClientRect(); return `${b.x},${b.y} ${b.width}×${b.height}`; };
  const open = document.getElementById('open');
  const res = { chrome: navigator.userAgent.match(/Chrome\/[\d.]+/)[0], open: r(open) };
  for (const id of ['plain', 'flip', 'both']) {
    const p = document.getElementById(id);
    p.showPopover({ source: open });
    res[id] = r(p);
    p.hidePopover();
  }
  return res;
});
console.log(JSON.stringify(out));
await browser.close();
````

`chrome/web-open.mjs`

````js
import { open } from '../../../../scripts/agent.mjs';
const s = await open({ host: 'web', app: 'x17-popover-flip', size: [420, 600] });
await s.tap('open'); await s.clock('settle');
console.log(await s.carrier.evaluate(`(() => { const p = document.querySelector('[data-testid=pop]'); const cs = getComputedStyle(p); const b = p.getBoundingClientRect();
  return JSON.stringify({ open: p.matches(':popover-open'), positionArea: cs.positionArea, position: cs.position, inset: [cs.top, cs.left, cs.bottom, cs.right].join(' '), rect: b.x + ',' + b.y + ' ' + b.width + 'x' + b.height }); })()`));
await s.close();
````

`probe.contract`

````contract
// X17: a popover anchored below a button in the window's bottom-right corner.
component X17PopoverFlip
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=8 justify-content="flex-end" align-items="flex-end"
      button popovertarget="pop" testId="open" width=80 height=32
        text "Open"
      column id="pop" popover="auto" position="absolute" position-area="bottom span-left" position-try-fallbacks="flip-block, flip-inline" width=320 height=120 margin=0 padding=0 border="1px solid #888" background-color="#ffffff" testId="pop"
        text "Popover 320×120"
````

</details>

## Discussion at transfer

### daehyeon-mun — 2026-10-07T07:49:33Z

## Decision needed
**[Design]**: an accepted LLP decides otherwise, or the API has to be chosen first. It needs a ruling before implementation. Re-checked on main `78286adc1` (2026-10-07). It still reproduces. (`position-area` has 8 values; `position-try-fallbacks` is `lower-unknown-attr`; web `layout pop` is `332,592 322×122`).

**Blocked by:** LLP 1021 D2/§5 (`llp/1021-menus.rfc.md:471,491,606`: "a flip is `position-try`, still refused"; the schema's "Clamped to the viewport; never flipped").

**Options:**
- Admit `position-try-fallbacks` (`flip-block`, `flip-inline`, `flip-start` and their combinations) and the remaining `position-area` values on invoker-anchored popovers. Every host takes the first fallback that fits, as Chrome does.
- Decide what happens to the macOS-only clamp: it either becomes a declared default fallback or goes away.

**Recommendation:** admit them, and replace the macOS clamp with CSS's behavior. CSS Anchor Positioning defines the algorithm exactly and Chrome is the oracle. Today macOS and the web already disagree for the same app. Decide this together with #127's anchor positioning.

**Cost:** medium to large: kernel placement, four hosts, and a conformance case at several window sizes and corners.

### ccheever — 2026-10-08T08:07:33Z

**Decision: Choose CSS position-area and flip fallbacks for invoker popovers.**

Selected as the next core investment by Charlie on 2026-10-08; keep open for design and implementation.

This removes repeated popup arithmetic and the current web/macOS disagreement. Prefer CSS fallback order to an unconditional macOS-only clamp.

Charlie selected this priority on 2026-10-08. Amend LLP 1021's no-flip scope before implementation. Start with invoker popovers and flip-block/inline/start; test all window edges, scrolling and resize. Coordinate anchors with #127 without admitting all of anchor CSS at once.
