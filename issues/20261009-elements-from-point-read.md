# `elementsFromPoint(x, y)`: every node at a point, so an inspect layer can take the click and still name what it covers (theme editor Inspect)

**Status:** Open
**Systems:** Contract, runner geometry, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/321

## Current scope

Design the action-only elementsFromPoint structural read, preserving front-to-back own-id results and geometry restrictions. Coordinate #281 for ordinary overlay click consumption. Pin own-id versus nearest-ancestor-id behavior before the original first-entry comparison; this is not a devtools or pixel-readback feature.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

T3 Code's theme editor has an **Inspect** button ("Inspect app colors"). Everything below is at `1e2ecbd975`, in `apps/web/src/components/settings/ThemeEditorPanel.tsx:612-724, 1176-1210` and `themeInspector.ts`.

- **Arm.** While inspect is armed, the panel says "Select an element · Esc to cancel".
- **Hover.** Hovering any element outside the editor outlines it, with the theme role that paints it as a label.
- **Click.** A click selects that role in the editor and disarms. The click itself never reaches the app: the panel listens on the document in the capture phase and calls `preventDefault()` and `stopPropagation()` on `pointerdown` and `click`.
- **Uses.** The editor then shows "N uses".

The lookup is structural. No pixels are read.
- From the hit element up through its ancestors, it takes the first one whose paint depends on a theme token.
- It reads the Tailwind utility class names first (`inspectThemeRoleFromUtilitiesAtElement`, `themeInspector.ts:389-403`). If none matches, it compares `getComputedStyle` paint before and after swapping a token for a sentinel colour (`inspectThemeRoleAtElement`, `:453-501`).
- `getBoundingClientRect` places the outline.

Exact already has most of this:
- `elementFromPoint(x, y)` names the front-most node at a point by its nearest `id` (LLP 1094 D10, `docs/contract-grammar.md:545`);
- `PointerEvent` carries `clientX`/`clientY`;
- `frame(id)` gives the box for an outline;
- a Contract app writes every paint itself, so it can carry the role in the node's `id` (`id="paint-surface-card"`). That plays the part the reference's class names play.

The hover half works today.

**What cannot be built is the click.**
- A Contract pointer event goes only to the innermost node that hears it, and "None of them takes anything from `press`" (`docs/contract-grammar.md:813-822`).
- `preventDefault()` in a `pointerdown` does not cancel the press under it, as in DOM.
- There is no capture phase for pointer events (#140 chose capture-phase handling for keys only).

So the only way to keep the inspect click away from the app's controls is a layer over the app that takes the press. `elementFromPoint` then names the layer, never what it covers.

The smallest primitive that closes this is DOM's plural: **`elementsFromPoint(x, y) -> list<string>`**.
- It returns every box at the viewport point, front to back, each named by its own `id` (boxes without one are skipped).
- It tests the same boxes and skips the same ones (`pointer-events: none`, hidden) as `elementFromPoint`, through the same transforms and scrolls.
- It is an action-only read.

The inspect layer then skips its own id and reads the first role-named id under it. On the web the runtime already answers `elementFromPoint` with `document.elementsFromPoint` (LLP 1094 §6, the note of 2026-10-04 on D11). Natively, the runner's walk (`runner/src/geometry.rs`) would collect hits instead of stopping at the first.

### Current and expected behavior

Measured on both hosts with the app below (record linked under the table):

1. **No layer, hover over Save:** `hit: paint-primary-save` on macOS and the web. The lookup works.
2. **No layer, click Save:** the hit is right, but the app's own `press` also runs (`presses` 0 → 1 on both hosts).
3. **Same, with `preventDefault()` in the root's `pointerdown`:** the press still runs (`presses` 1 → 2 on both).
4. **Layer over the card, hover over Save:** `hit: paint-surface-card` on both hosts, the layer's nearest id, never `paint-primary-save`.
5. **Layer, click over Save:** on the web the layer takes the click (`presses` stays 2). On macOS it also pressed Save under the layer (`presses` 2 → 3). That is the pass-through of #281, here with a plain positioned box rather than a popover.

**Expected:** in step 4, `elementsFromPoint(e.clientX, e.clientY)` in the layer's handler returns `["paint-primary-save", "paint-surface-card"]` on every host. A click on the layer selects the role and presses nothing, as the reference's capture-phase listeners ensure.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`, with no data sources:

```text
component X68app
  state inspecting = false
  state layer = false
  state cancel = false
  state hit = "-"
  state presses = 0
  action arm(withLayer: bool)
    inspecting = true
    layer = withLayer
  action stop
    inspecting = false
    layer = false
  action toggleCancel
    cancel = not cancel
  action look(e: PointerEvent)
    if inspecting
      hit = match elementFromPoint(e.clientX, e.clientY) { case some(id) => id, case none => "none" }
  action pick(e: PointerEvent)
    if inspecting
      hit = match elementFromPoint(e.clientX, e.clientY) { case some(id) => id, case none => "none" }
      if cancel
        preventDefault()
  action pressPrimary
    presses = presses + 1
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="#ffffff" color="#111111" pointermove=look pointerdown=pick
      row gap=8
        button press=arm(false) testId="inspect"
          text "Inspect"
        button press=arm(true) testId="inspect-layer"
          text "Inspect (layer)"
        button press=stop testId="stop"
          text "Stop"
        button press=toggleCancel testId="cancel"
          text "preventDefault"
      text `hit: ${hit} · presses: ${presses}` white-space="nowrap" testId="out"
      text `inspecting: ${inspecting} · layer: ${layer} · preventDefault: ${cancel}` white-space="nowrap" testId="mode"
      column id="paint-surface-card" testId="card" width=280 padding=8 background-color="#eef2ff" border-width=1 border-style="solid" border-color="#c7d2fe" position="relative"
        button id="paint-primary-save" press=pressPrimary testId="primary" background-color="#2563eb" color="#ffffff" padding=8
          text "Save"
        when layer
          box testId="layer" position="absolute" left=0 top=0 width="100%" height="100%" pointermove=look pointerdown=pick
```

Ops (the layer covers exactly the card, so its centre is over Save): `"tap inspect" "tap primary hover" "clock settle" "tree out" "tap primary" "clock settle" "tree out" "tap cancel" "clock settle" "tap primary" "clock settle" "tree out" "tap cancel" "tap stop" "tap inspect-layer" "clock settle" "tap layer hover" "clock settle" "tree out" "tap layer clicks 1" "clock settle" "tree out"`.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Inspect without and with a layer, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 700x300 <ops>` | macOS 26.6.2, Apple Silicon | main `b896050d7` | `out` after each step: `paint-primary-save · 0`, `paint-primary-save · 1`, `paint-primary-save · 2`, `paint-surface-card · 2`, `paint-surface-card · 3` | the layer reads `paint-primary-save` through `elementsFromPoint`; presses stay 0 while inspecting through the layer | [record](https://raw.githubusercontent.com/ccheever/exact2/911685b6825363eeb0cb30e117c7fd761c0e8e14/file-x48-x68/x68-record.txt) |
| Same, web | `bun exact.mjs agent web --size 700x300 <ops>` | Chrome 154 (the agent's) | same | `paint-primary-save · 0`, `· 1`, `· 2`, `paint-surface-card · 2`, `paint-surface-card · 2` | same | same, [end-state screenshots](https://raw.githubusercontent.com/ccheever/exact2/d47bb6ad52f193bb2b368f7dbe15ee558fb5a059/file-x48-x68/x68-macos-vs-web.png) |
| `elementsFromPoint` | `contract build` of a `pointerdown` action that reads `elementsFromPoint(e.clientX, e.clientY)` | any (compiler) | same | `` [type-unknown-function] `elementsFromPoint` is not in the stdlib roster … did you mean `elementFromPoint`? `` | `list<string>` | — |

### Acceptance criteria

- In the repro, with the layer's handlers changed to read `elementsFromPoint(e.clientX, e.clientY)` and skip the layer:
  - hover and click over Save give `paint-primary-save` on macOS and the web;
  - `presses` does not move while the layer is up. On macOS this also needs #281's fix to hold for a plain positioned box.
- The list is front to back, by each box's own `id`, and agrees with `elementFromPoint` on the first entry that has an id.
- It applies the same skips, transforms and scroll offsets as `elementFromPoint`. Outside an action it is refused with `type-geometry-outside-action`.
- Not requested here:
  - the editor's "N uses" count and the spotlight of every node of a role, which would need the frames of all mounted nodes with a given id;
  - a paint or style read.

### Constraints and related work

- **Not #116.** #116 is about an any-type file `input`, and about image decoding and pixel readback **in data modules**, on file bytes, "not window pixels". Its decision keeps codecs and readback deferred and proposes only the any-type file-input exception. Inspect reads no pixels at all: it needs a hit test that can see under a layer, plus the app's own role names. Nothing in #116's bounded scope, nor in its deferred part, provides that.
- **Not #101.** #101 is a developer-tools inspector for the app's own UI. Its decision rules out "a new inspector window or inspect command". This request is an app-facing stdlib read like `elementFromPoint`, used by an app feature.
- Larger alternatives, not proposed:
  - (a) capture-phase pointer handlers that can cancel the press: the reference's exact shape, and the pointer sibling of #140. It changes event dispatch on every host.
  - (b) a paint-provenance read, i.e. which state value a node's colour comes from, the analogue of the reference's token probe. Contract apps author every paint, so tagging roles in ids is enough.
- Workaround today: no Inspect. Hover-only inspection works without a layer, but its click also presses the control under it.
- Related: #281 (macOS pass-through under a covering node), LLP 1094 D10 (`elementFromPoint`), LLP 1051.000 D1 (`frame`).
