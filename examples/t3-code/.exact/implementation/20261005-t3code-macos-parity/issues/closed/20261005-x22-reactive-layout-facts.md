---
name: 20261005-x22-reactive-layout-facts
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-browser-surface, 20261005-composer-fidelity, 20261005-diff-review-engine, 20261005-floating-device-player, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-links-previews-and-routing, 20261005-settings-scoped-controls-and-theme-editor, 20261005-shiki-residuals]
upstream_url: https://github.com/ccheever/exact2/issues/127
reproduced_on: 4c893fef6
---

# X22: Reactive layout facts (size, position, text width, row visibility)

Moved to main `issues/20261009-intersection-visibility-event-design.md` (2026-10-09); tracked there.

## Summary

T3 Code reads layout facts all over its UI: a composer height that the transcript must reserve, a menu that follows its anchor while a panel slides, a minimap that follows the scroll area, and sidebar rows that
know when they are near the viewport. The reference gets these from `ResizeObserver`, `getBoundingClientRect` and `IntersectionObserver`. In exact2 a Contract view cannot read another node's size or position
reactively, and `frame()` works only inside actions. The clone measures them with four native hooks.

## Why it arose

### The T3 Code behavior
Counts in `apps/web/src`, excluding tests, on 2026-10-05: 25 `new ResizeObserver` in 21 files, 8 `new IntersectionObserver` in 7 files, 121 `getBoundingClientRect` in 42 files, 17 `matchMedia`
in 12 files, 13 container-query matches in CSS and TSX. The user-visible cases that the clone also has:
- **Composer overlay reservation.** The composer stack floats over the transcript; its height is published and the transcript keeps that much room at its end so the last row can scroll clear
  (`components/ChatView.tsx:6770-6790`, `ResizeObserver` at `:6783`, re-measured when the scroll-to-bottom button shows).
- **Composer menus.** The slash and mention menus take a position from the anchor and update on window resize, on scroll, and on the resize of the anchor's ancestors, "because the composer is centered and
  capped at a max width, so opening a side panel slides it sideways without ever resizing it" (`components/chat/ChatComposer.tsx:1025-1040`). Resting controls compact from measured label widths
  (`chat/restingComposerControlsMeasurement.ts`; `ChatComposer.tsx:1194`).
- **Thread details card.** It measures its content height at each density (`chat/ThreadDetailsCard.tsx:85-100`) and picks the density that fits.
- **Timeline minimap and fades.** Viewport and content widths decide the minimap gutter and the hit strip (`chat/MessagesTimeline.tsx:1130-1145`, scroll fades at `:3304`).
- **Sidebar.** The brand probe width sets the sidebar minimum (`components/sidebar/SidebarChrome.tsx:88`); rows subscribe only while near the viewport, with an overscan margin
  (`components/Sidebar.logic.ts:85-100`, `IntersectionObserver`).
- **Others.** Right-panel tab-strip scroll state (`RightPanelTabs.tsx:1023`), preview panel inline size (`hooks/usePreviewPanelInlineSize.ts:90`), usage chart tooltip (`usage/UsageProviderChart.tsx:286`),
  citation chip placement (`chat/AssistantCitationSource.tsx:306`), element width (`hooks/useElementWidth.ts:17`).

### Where the clone hits it
Tickets that needed a fact: `20261005-composer-fidelity` (host width for label compaction), `20261005-floating-device-player` (container size, composer height, details card rect), `20261005-diff-review-engine`
(Cite button and chip popover at the selection), `20261005-pr-handoffs-and-quick-actions` (header fold from a scroll offset), `20261005-pr-links-previews-and-routing` (the hover card anchor),
`20261005-settings-scoped-controls-and-theme-editor`. Each row was nonblocking because a hook exists.

## Clone workaround

Four hooks, all measuring natively and reporting to TypeScript (mc-orch tree, 2026-10-05):
- `t3-frame` (13 hook attributes in 4 files: `app-main.contract`, `composer-controls.contract`, `composer.contract`, `r6-device.contract`): `modules/apple/T3ComposerFrames.swift` measures `[x, top, width, height]` in
  window space for the composer stack, card, banners, shoulder row and the device area. Reader: `r4-composer-overlay.ts` (the reservation).
- `t3-anchor` (21 attributes in 7 files): `T3Composer.swift:37` reports `[x, width]` of probe boxes drawn only to be measured (`r5-composer.contract:15,49`, `r6-polish.contract:18`); readers `r5-composer-measure.ts`,
  `r6-polish-measure.ts` ("Null until the box has been laid out"); `R5ComposerScroll.swift:6` counts scroll extents.
- `t3-measure` (1 attribute, `markdown.contract:803`): `R8KeysMeasure.swift` reports the drawn position of a node inside a scrolled transcript at the moment a menu opens ("Contract's `frame(id)` is the layout's
  position, which a scrolled transcript leaves behind").
- `t3-turn` (2 attributes, `timeline.contract:117,307`): `R9Input.swift:40` and `T3TimelineTurns.swift:5` tell which user turns are on screen, for the minimap.

Differences from the reference: measured widths are `null` until laid out; extra hidden probe nodes exist only to be measured; a native hook per fact adds code; facts arrive after layout through a change
notification. #127's decision (2026-10-08) made the measuring hooks (`t3-frame`, the measuring part of `t3-anchor`, `t3-measure`) and the theme editor's window-sized `resize=` tracker a permanent declared
difference; anchoring went to #112 (X17), and row visibility is the part main still tracks.

## Evidence and history

- Filed 2026-10-06 as [#127](https://github.com/ccheever/exact2/issues/127), reproduced on exact2 `4c893fef6` before filing.
- A local fix was built on 2026-10-06 (branch `daehyeon/fw-x22-resize-event`, worktree `~/orca/workspaces/exact2/t3-fw-x22`, commits `dcac295f9` and `fc4b14b88`, not pushed): a `resize=action` event
  delivering a `ResizeEvent {width, height}`, checked against Chrome on the web JS target, the wasm web host and macOS. Superseded on 2026-10-08 by main's element `resize` event (`5949b2b64`, in the branch)
  and #127's decision; not pursued (the branch is kept).
