---
name: 20261005-x17-popover-position-try
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-composer-fidelity, 20261005-legacy-sidebar, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-links-previews-and-routing, 20261005-pr-writing-and-metadata, 20261005-provider-settings-upkeep, 20261005-provider-sign-in-and-install, 20261005-server-update-banner, 20261005-upstream-ui-sync, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/112
reproduced_on: 4c893fef6
---

# X17: Popover anchoring with every side/align area and automatic flip and shift (`position-area`, `position-try`)

Moved to main `issues/20261009-popover-css-flip-fallbacks.md` (2026-10-09); tracked there.

## Summary
T3 Code positions tooltips, hover cards, menus and popovers with Base UI's positioner: a side
(top/bottom/left/right/inline-start/inline-end), an alignment (start/center/end), an offset, and
automatic collision handling that flips the side and shifts the alignment against the window's
5 px edge, exposing the available width to the popup. exact2 places a popover above or below its
invoker. The clone computes, per call site, where Base UI would land and hard-codes side,
alignment and a transparent "tail" that pushes an end-aligned menu back inside the window.

## Why it arose

### The T3 Code behavior
- **Positioner defaults.** `apps/web/src/components/ui/tooltip.tsx:82-132`: side `top`, align
  `center`, `sideOffset` 4, positioner `max-w-(--available-width)`; popup scales from 98 % and
  fades (`data-starting-style:scale-98 opacity-0`). `ui/preview-card.tsx:9-40`: side `top`, align
  `start`, `sideOffset` 6. `ui/menu.tsx:20-65`: `sideOffset` 4, align `center`, submenus side
  `inline-end` with `alignOffset` −5 (:303-321). None of these set collision props, so Base UI's
  defaults apply (flip the side, shift the alignment, padding about 5 px).
- **Visible results.** A tooltip on a control near the top edge appears below it; a menu anchored
  at the right edge slides left so it stays inside; a hover card taller than the space above flips
  below. The popup's transform origin follows the side. The PR hover card opens after 350 ms
  and closes after 120 ms (`PullRequestLinkPreview.tsx:110-113`).
- Users see this on: thread hover cards, sidebar and composer tooltips, the PR hover card and
  checks/stack popovers, model and provider menus, the usage tooltip, update and sign-in
  popovers (plan tickets listed in `blocks`).

### Where the clone hit it
- `shell-tip.contract:1-40` (`Tip`, `TipBubble`): "`side` is where the reference lands after
  collision flipping; `align` where it lands after shifting"; `inset` "moves an end- (or start-)
  aligned bubble past the trigger's edge, as Base UI's 5pt collision shift lands it against the
  window edge"; bubbles are 360 pt rows because a 320 pt card shifted far in must not wrap early.
  Each call site passes the answer it measured in the oracle.
- `r6-pr-logic.ts:208-232` `prCard(…, room, row)`: computes `flipped` (`width > room - 5`) and
  `clamped` from text-width tables (`width12`) because "the reference portals the card; here it
  is drawn inside the details card, which clips". `r6-pr.test.ts` "the card flips to end-aligned where it would cross the window".
- `pages-prs.contract:161-166`: "An open popover sits at its invoker's bottom-left, clamped to the
  window, so an end-aligned menu carries a transparent tail as wide as the gap from its trigger's
  right edge to the window's" (`tail=(viewportWidth - filtersRight)`, with hand-computed
  `filtersRight`/`providerRight`).

User-visible differences: a card or menu placed by a fixed answer is wrong at any window width,
zoom or anchor position the oracle was not measured at (the 840×620 minimum is the main
exposure); a hover card that would not fit above does not flip unless its call site computes it.

## Clone workaround

Fixed placement per call site (`TipBubble` side/align/inset tables, `prCard`'s `flipped`/`clamped`/`maxWidth`, the `tail`
arithmetic in `pages-prs.contract`); hover layers (tooltips, hover cards, the Usage popovers) place themselves with
`hoverFlip` on the hover layer of `fix-hover-cards` (#307). No new per-site arithmetic is added meanwhile.
Once main flips invoker popovers (`popovertarget`: the menus, the Check out, More and stack menus, the update details
popover, the machine list), the clone replaces their arithmetic with `position-area`/`position-try` and re-drives the
placements that lean on the macOS clamp (the `tail`, the margins under `position-area="bottom span-right"`).

## Evidence and history
- Filed as [#112](https://github.com/ccheever/exact2/issues/112) on 2026-10-06, reproduced on exact2 `4c893fef6` before filing.
- `AGENT-HANDOFF.md` lists "tooltip offsets; Send tooltip shifted; flipped hover card overhang" among the clone's declared differences.
- Decided on 2026-10-08: CSS `position-area` and flip fallbacks for invoker popovers first; state-driven layers, `anchor-size()`
  and `position-visibility` are outside that first slice, so they keep the app's own placement.
