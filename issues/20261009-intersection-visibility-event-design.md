# Layout facts beyond size: visibility, live position, container/anchor CSS

**Status:** Open
**Systems:** Contract, GUI hosts, layout events
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/127

## Current scope

Charlie's issue-fix ruling 5 (2026-10-12) admits `intersect=action` with
`{isIntersecting, intersectionRatio}`, `intersect-margin`, the nearest scroller
as root, threshold 0, and mount + enter/leave delivery. Consumer: T3 sidebar
subscriptions and timeline minimap. Anchors coordinate with #112; container
queries, derive-time frame and general live geometry remain deferred.

## First slice, 2026-10-12

The Contract compiler, typed event/ABI, vocabulary and both web targets now carry
the admitted shape (LLP 1005 "Intersection visibility"). The margin is a literal
one-to-four px/% rootMargin. Both web executors use one lazy browser adapter;
removal disconnects it and a changed nearest scroller starts a new observation.
The admission and incomplete-host deviation are recorded in DEFERRED and LLP 1001.

**What remains (issue stays Open):** see the c1 continuation below for the
shared native implementation, its qualification limits and pending drives.

Regression coverage: `geometry::intersect_compiles_and_dispatches_its_entry_after_authored_arguments`
(the baseline failed with `lower-unknown-attr`), the Chrome oracle test in
`host/web/tests/intersect.test.mjs`, and `host/web-js/conformance/intersect.contract`
with its scroll, removal and remount drive. Verification results are reported by
the lane; the existence of a fixture is not a claim that a host passed it.

## c1 continuation, 2026-10-12

The five first-slice failures are repaired and each passed by name: both type
choice diagnostics, the VS Code event vocabulary, the schema digest snapshot,
and the reserved `data-intersectmargin` host attribute.

AppKit/UIKit and Linux now share `Runner::intersect_due` over the kernel's
post-layout geometry and the scroll offsets already reported for `frame()`.
The native hooks deliver mount and threshold-zero enter/leave, including edge
contact and zero-area targets. They use the nearest scroll container (including
`overflow: hidden`), root margins, committed 2D transforms and intervening CSS
path bounds. Generation-checked samples are marked before dispatch; a removed
or recycled target cannot receive an earlier queued entry. No Swift visibility
calculator or new ABI entry point was added. Apple wakes its existing executor
when an offset changes; Linux samples in its existing geometry turn.

The shared Rust regression first failed with no initial entry. Its first
implementation exposed default CSS border widths incorrectly shrinking the
root even for `border-style: none`; subtracting only used borders fixed that.
The Apple host regression then exposed implicit `<scroll>`/`<list>` overflow:
root selection now consults the same semantic default as kernel layout.
Both shared tests pass with `cargo test -p contract --test intersect`.
They cover scrolling, mount/remount, stale-entry rejection, root changes,
viewport edges, zero area, display:none, transforms and intervening clips.
The Apple host and Linux presenter regression each pass via
`cargo test -p exact-apple -p exact-linux --lib native_intersect --no-fail-fast`.
They and `IntersectionMacTests` / `IntersectionIOSTests` use
`scripts/fixtures/intersect.contract`, with no authored scroll handler.

The Chrome adapter test passes. Its new numeric assertion establishes that
Chrome's percentage root margins use root height for vertical sides and root
width for horizontal sides. Native now follows that observed behavior;
LLP 1001 records the discrepancy from the W3C draft's all-width rule.

The filtered AppKit run passed one test in `ExactKitTests.IntersectionMacTests`
with real native scrolling: `ONLY_CLASSES=ExactKitTests.IntersectionMacTests bun host/apple/build.mjs --test`. The temporary class filter in `xctest.mjs`
was restored afterward.

**Remaining qualification:** run `IntersectionIOSTests` on a lane-owned simulator
and the full macOS/iOS/Linux app drives. No simulator was booted in c1.
Qualify/fix inline text targets, sticky/fixed and animated presentation geometry,
rotated-root ratios and SVG clip references. Windows and Android inherit the
Linux presenter path but have not been exercised; terminal does not call the
sampler. The shared sampler uses committed transform rows, as `frame()`
does; it does not observe animation samples. The issue stays open.

The conformance `--build` run is deliberately skipped under the machine's
current load, as instructed for c1. No full web-executor parity is claimed.
Scoped Clippy (`contract`, `contract-lower`, `exact-kernel`, `exact-runner`,
`exact-apple`, `exact-linux`, all targets), formatting, caps and issue format
passed. The semantics quick check was started, then stopped during compilation
after confirming Lake is absent from both PATH and `~/.elan/bin/lake`.
The remaining check results are recorded in the lane's final report. Prepared
tests are not a claim that a native presenter passed.

## Review fix round 1, 2026-10-12

All four review findings reproduced before the fix:

- Apple sampled mount before the presenter applied `scrollTop=150`. The
  AppKit fixture reported `2:true:1` (a false entry followed by a true entry),
  instead of the browser's single `1:true:1`. Sampling now runs at the start
  of the next executor pump, after presentation, before new pump commits.
  `native_intersect_waits_for_presented_authored_scroll` and the authored-scroll
  addition to `IntersectionMacTests` cover this boundary.
- Native batches used hash-map iteration rather than registration order.
  `native_intersect_entries_follow_registration_order` failed for 32 mounted
  rows. The runner now orders existing observations by their registration,
  fresh observations by monotonic mount ids, and reobserves changed roots at
  the end. Both native hosts consume that order.
- `200PX` compiled but native geometry treated it as zero.
  `native_intersect_margin_units_are_case_insensitive` failed on that spelling;
  the compiler now normalizes units once for all hosts. Mixed-case and
  four-sided forms are covered too.
- Linux delivered ratio `0.4` before a resize handler removed an 80px spacer,
  then never updated it to `1` because the target remained intersecting.
  `native_intersect_samples_after_resize_handlers` reproduced it. Every Linux
  geometry turn now completes the resize loop before intersection sampling,
  with nested sampling suppressed while resize callbacks run.

The Chrome test `intersection initial entries follow scroll, resize and
registration order` supplies the expected results, including uppercase units
and registration order after a DOM move. The issue remains Open for the
qualification listed above; this round does not claim the remaining geometry
cases or full app drives.

The focused runner/compiler and Apple/Linux regressions, Chrome adapter tests,
and filtered AppKit `IntersectionMacTests` passed. Scoped Clippy passed for
`contract`, `contract-lower`, `exact-runner`, `exact-apple` and `exact-linux`.
The related `resize` filter passed 18 Apple/Linux tests; the compiler integration
`intersect` filter passed two. Formatting, staged caps and issue format passed.
`cargo run -p contract-difftest -- quick` reached the driver and exited 2:
`lake build: No such file or directory` (Lean is not installed). The temporary
`ONLY_CLASSES` filter was restored after AppKit. No simulator was booted in
this review round; UIKit and the broader qualification above remain open.

## Review fix round 2, 2026-10-12

Reviewer A's malformed-margin finding reproduced: the compiler accepted
`1.px`, and Chrome threw `SyntaxError` both for that target and for a valid
target registered afterward. `intersect_margin_requires_css_number_syntax`
first failed on the accepted literal. The browser regression
`invalid margins cannot block later intersection targets` first failed on
both constructor errors; it also checks Chrome's refusal of eight malformed
forms and acceptance of seven signed, fractional and exponent forms.

The compiler now validates CSS number syntax before float parsing. The shared
web adapter retires failed registrations and reports the error without
interrupting other targets. Its regression covers a newly invalid existing
target and explicit re-registration after correcting a margin, too.
Reviewer B reported no finding. This round does not change native sampling;
the issue remains Open for the native qualification above.

Verification: `cargo test -p contract -p contract-lower -p exact-web --lib
--bins --tests --no-fail-fast` passed 1,433 tests (13 ignored), and the focused
`cargo test -p contract --test intersect --no-fail-fast` passed all five.
`bun test host/web/tests/intersect.test.mjs` passed all three Chrome tests.
`cargo clippy -p contract -p contract-lower -p exact-web --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, staged caps and issue format passed.
`cargo run -p contract-difftest -- quick` exited 2 because `lake build` could
not start: Lean is not installed. No native presenter or full app drive was
rerun for this parser/adapter-only correction.

## Rebase qualification, 2026-10-12

The orchestrator reports `IntersectionIOSTests` passed on an iOS 27 simulator
before this rebase (lane head `776debde4`). This is the UIKit presenter result
from the orchestrator's run, not a new simulator run by this continuation.
The earlier pending UIKit class run is therefore satisfied. Full app drives
and the geometry/carrier qualifications listed above remain open.

Rebased onto `origin/main` at `57ab536e3`. Main's `keycapture` keeps host ABI
kind 44; `intersect` now uses 45 in the web bridge, browser dispatcher, shared
decoder and compiler regression. Both event names remain in the plan format,
vocabulary and editor grammar. The runtime copies and test stand-ins retain
main's font-family, querySelectorAll and contextAt support alongside observers.
The handler array length and schema digest snapshot are updated for the union.

The digest test first printed `11178576454759414984`; the snapshot now records
`0x9b22480f187afcc8`, and the full kernel test run passed it. The first caps run
found `host/apple/src/host.rs` three lines over the limit after merging main;
shortened comments and removal of a misplaced doc line restored the cap.

Post-rebase verification:

- `cargo build --all-targets --keep-going` — passed.
- `cargo test -p exact-linux --lib --tests native_intersect` — both targeted tests passed.
- `cargo test -p exact-web-js --bins --no-fail-fast` — 41 passed, including the Chrome paint corpus.
- `ONLY_CLASSES=ExactKitTests.IntersectionMacTests bun host/apple/build.mjs --test` — one AppKit class, one test, zero failures. The temporary `ONLY_CLASSES` filter was restored; no test-runner change is committed.
- `cargo clippy --all-targets --keep-going -- -D warnings` — passed.
- `cargo run -p contract-difftest -- quick` — exited 2: `lake build: No such file or directory (os error 2)`; Lean is still unavailable.
- `cargo test -p exact-kernel -p contract -p contract-lower -p exact-runner -p exact-web -p exact-apple --lib --bins --tests --no-fail-fast` — 2,995 passed, 32 ignored.
- `bun test host/web/tests/intersect.test.mjs host/web/tests/js-runtime.test.mjs host/web/tests/js-link-press.test.mjs host/web/tests/js-storage-scope.test.mjs host/web/tests/form-chrome.test.mjs` — 52 passed.
- `cargo fmt --all -- --check`, `git add -A && bun scripts/caps.mjs`, `bun scripts/issue.mjs check`, and `bun scripts/boot.mjs` — passed.

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
