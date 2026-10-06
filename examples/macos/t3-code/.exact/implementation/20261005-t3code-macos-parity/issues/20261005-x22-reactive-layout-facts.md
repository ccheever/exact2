---
name: 20261005-x22-reactive-layout-facts
plan: 20261005-t3code-macos-parity
status: fix-built
kind: framework-gap
blocks: [20261005-browser-surface, 20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-floating-device-player, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-links-previews-and-routing, 20261005-settings-scoped-controls-and-theme-editor, 20261005-shiki-residuals]
upstream_url: null
reproduced_on: null
---

# X22: Reactive layout facts (size, position, text width, row visibility)

## Summary

T3 Code reads layout facts all over its UI: a composer height that the transcript must reserve, a menu that follows its anchor while a panel slides, a minimap that follows the scroll area, and sidebar rows that
know when they are near the viewport. The reference gets these from `ResizeObserver`, `getBoundingClientRect` and `IntersectionObserver`. In exact2 a Contract view cannot read another node's size or position
reactively, and `frame()` works only inside actions. The clone measures them with four native hooks. A reactive fact in Contract (or CSS forms that make the fact unneeded) would remove those hooks.

## Why this issue arose

### The T3 Code behavior
Counts in `apps/web/src`, excluding tests, on 2026-10-05: 25 `new ResizeObserver` in 21 files, 8 `new IntersectionObserver` in 7 files, 121 `getBoundingClientRect` in 42 files, 17 `matchMedia`
in 12 files, 13 container-query matches in CSS and TSX. The user-visible cases that the clone also has:
- **Composer overlay reservation.** The composer stack floats over the transcript; its height is published and the transcript keeps that much room at its end so the last row can scroll clear
  (`components/ChatView.tsx:6770-6790`, `ResizeObserver` at `:6783`, re-measured when the scroll-to-bottom button shows). States: resting, expanded, a banner stack on top.
- **Composer menus.** The slash and mention menus take a position from the anchor and update on window resize, on scroll, and on the resize of the anchor's ancestors, "because the composer is centered and
  capped at a max width, so opening a side panel slides it sideways without ever resizing it" (`components/chat/ChatComposer.tsx:1025-1040`). Resting controls compact from measured label widths
  (`chat/restingComposerControlsMeasurement.ts`; `ChatComposer.tsx:1194`).
- **Thread details card.** It measures its content height at each density (`chat/ThreadDetailsCard.tsx:85-100`) and picks the density that fits.
- **Timeline minimap and fades.** Viewport and content widths decide the minimap gutter and the hit strip (`chat/MessagesTimeline.tsx:1130-1145`, scroll fades at `:3304`).
- **Sidebar.** The brand probe width sets the sidebar minimum (`components/sidebar/SidebarChrome.tsx:88`); rows subscribe only while near the viewport, with an overscan margin
  (`components/Sidebar.logic.ts:85-100`, `IntersectionObserver`).
- **Others.** Right-panel tab-strip scroll state (`RightPanelTabs.tsx:1023`), preview panel inline size (`hooks/usePreviewPanelInlineSize.ts:90`), usage chart tooltip (`usage/UsageProviderChart.tsx:286`),
  citation chip placement (`chat/AssistantCitationSource.tsx:306`), element width (`hooks/useElementWidth.ts:17`).
Each fact updates on window resize, content change and animation frames; the value before first layout is "unknown", and the UI must not flash a wrong state.

### What exact2 does today
- `EXACT2-GAPS.md` X22 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): "`frame()` now reads viewport space (`e73605835`), but works only in actions and is not reactive
  (`docs/contract-for-humans.md:1150-1156`)." The table names the need: "Reactive layout facts (size/position, text width) and row visibility".
- Library (`20261005-platforms-v3`): the agent's `layout <testId>` reports node identity, coordinate spaces, scroll and clip chains and visibility, but it is a test observation (testing-and-debugging.md:46).
  The app-readable host facts the library lists are in `exactViewport()` (`prefersReducedMotion`, `prefersReducedTransparency`, `prefersColorScheme`; accessibility.md:36, design.md:63). A reactive
  per-node size or position for app logic is not covered: unknown in this library.
- Observed in the clone (clone code, mc-orch tree, 2026-10-05): `modules/apple/R8KeysMeasure.swift:4-8` says "Contract's `frame(id)` is the layout's position, which a scrolled transcript leaves behind; a popup
  anchored to a node inside the transcript (a table's Copy menu) asks for the drawn position at the moment it opens."

### Where the clone hits it
Four hooks, all measuring natively and reporting to TypeScript:
- `t3-frame` (13 hook attributes in 4 files: `app-main.contract`, `composer-controls.contract`, `composer.contract`, `r6-device.contract`): `modules/apple/T3ComposerFrames.swift` (76 lines) measures `[x, top, width, height]` in
  window space for the composer stack, card, banners, shoulder row and the device area. Readers: `r4-composer-overlay.ts` (85 lines, the reservation).
- `t3-anchor` (21 attributes in 7 files): `T3Composer.swift:37` reports `[x, width]` of probe boxes drawn only to be measured (`r5-composer.contract:15,49`, `r6-polish.contract:18`); readers `r5-composer-measure.ts` (25 lines),
  `r6-polish-measure.ts` (38 lines, "Null until the box has been laid out"); `R5ComposerScroll.swift:6` counts scroll extents.
- `t3-measure` (1 attribute, `markdown.contract:803`): `R8KeysMeasure.swift` reports the drawn position of a node inside a scrolled transcript at the moment a menu opens.
- `t3-turn` (2 attributes, `timeline.contract:117,307`): `R9Input.swift:40` and `T3TimelineTurns.swift:5` tell which user turns are on screen, for the minimap.
- Differences from the reference: measured widths are `null` until laid out; extra hidden probe nodes exist only to be measured; a native hook per fact adds code; and facts arrive after layout
  through a change notification instead of as part of the same layout pass. Whether a user sees a one-frame lag is to confirm on a lane run.

## Why it must be resolved

The goal is a complete clone, and these facts carry some of the app's most visible polish: the composer that never covers the last message, menus that stay attached to their controls, and a timeline that tracks the reader.
Waiting tickets: `20261005-composer-fidelity` (host width for label compaction), `20261005-floating-device-player` (container size, composer height, details card rect), `20261005-diff-review-engine` (Cite button and chip popover
at the selection), `20261005-pr-handoffs-and-quick-actions` (header fold from a scroll offset), `20261005-pr-links-previews-and-routing` (the hover card anchor), `20261005-settings-scoped-controls-and-theme-editor`.
Each ticket's row is nonblocking because a hook exists, but every new fact is another hook, another Swift class, another probe node and another TypeScript reader. The maintenance cost grows with each ticket.

## Requested support

The web way, in the order in which the web solves them:
1. **CSS forms that remove the need.** Container queries (`@container`, `container-type: inline-size`) for compacting labels and densities; anchor positioning (`anchor-name`, `position-anchor`, `position-area`,
   `anchor()`) for menus and cards (the popover side of this exists on `main`: EXACT2-GAPS lists `position-area`, `2c6b551ba`); `position: sticky` is X32.
2. **A reactive fact for the rest**, the equivalent of `ResizeObserver` and `IntersectionObserver`: a Contract value such as `size(testId)` and `rect(testId, space)` that re-evaluates on layout (including during animation),
   a `visible(testId, margin)` value for row visibility, and a text-width measure for a string at a style (the equivalent of `measureText`). The fact is `unknown` until laid out and says so in the type.
3. **`frame()` in derives**, not only in actions, with a stated coordinate space and drawn-versus-layout position (the case in `R8KeysMeasure.swift`).
macOS first; the web host already has the DOM forms. A and B differ in size: step 1 is the web-standard path and removes more hooks; step 2 is the general path. Both can be done in order.

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Minimal app: a column whose width follows the window, a sibling that shows that width as text through a `derive`, and a menu that must sit under a button inside a scrolled list.
2. Resize the window. Expected (web): the text follows. Actual: the value cannot be read in a `derive`; `frame()` in an action returns it once.
3. Clone scenario: lane build, a draft thread; open the model picker; open the right panel while the menu is open. The reference menu follows the composer while the panel animates (read from `ChatComposer.tsx:1025-1040`);
   check what the clone does with `layout` before and after.
4. Row visibility: a long list; read which rows are in view. The web host has `IntersectionObserver`; exact2 has no equivalent for app logic.

## Acceptance for the fix

- A conformance case against Chrome: a reactive size and position equal `getBoundingClientRect` for the same node at three window sizes and during a 200 ms transition (sampled by `clock +N`); a text width equals
  `measureText` for the same font within 0.5 px; a visibility fact flips at the margin during scroll; a container query and an anchored menu give the same boxes as Chrome.
- Agent run: `state` shows the facts, and `layout` agrees with them.
- An AppKit test covers the macOS host's change delivery without a frame of lag.

## App adoption after resolution

Remove the hooks that only measure: `t3-frame` (`T3ComposerFrames.swift`), the measuring part of `t3-anchor` and the probe nodes, `t3-measure` (`R8KeysMeasure.swift`) and, with X23, `t3-turn`. Replace
`r4-composer-overlay.ts`, `r5-composer-measure.ts` and `r6-polish-measure.ts` readers with the new values. Rows that must pass after the change: the composer reservation rows of `20261005-composer-fidelity`, the menu-follows-panel
check, the minimap rows, the player container rows, and oracle pairs at 1280×840 and 840×620. `issue-close` checks that no measuring hook remains except where a native reason is written down.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Fix built (2026-10-06)

Built on exact2 `origin/main`, branch `daehyeon/fw-x22-resize-event` (worktree `~/orca/workspaces/exact2/t3-fw-x22`), commits `dcac295f9` and `fc4b14b88` (review fixes). Not pushed.
- `resize=action` (an action or action prop; a quoted string stays CSS's `resize` row) delivers a `ResizeEvent {width, height}`, the border box in CSS px at Chrome's 1/64 px, following ResizeObserver: first after the layout that follows creation, then on change, last size per frame, Chrome's depth rule for loops. Native hosts deliver inside the layout and commit in the same batch (no frame of lag), bounded per commit.
- Not built: a visibility (IntersectionObserver) fact; geometry in derives stays refused (LLP 1051.000 D2). Row visibility: `scroll` plus `frame(id)` in an action.
- Evidence: the five checks; compiler/runner tests (12), Apple host tests (5), a Linux presenter test; conformance against Chrome (Caltrain's dist); a scratch app passes on the web JS target, the wasm web host and macOS, including after `resize 600x800`; one independent review (a loop that never stopped on native, among six) fixed.
- Before main: LLP 1051.000 §7 (proposed) needs Charlie's ruling and a DEFERRED take or waiver. Merge note: its event ABI kind 39 collides with X20's `BeforeInput` (also 39); renumber one at merge.
