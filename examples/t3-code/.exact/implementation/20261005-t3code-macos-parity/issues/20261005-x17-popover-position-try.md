---
name: 20261005-x17-popover-position-try
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-composer-fidelity, 20261005-legacy-sidebar, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-links-previews-and-routing, 20261005-pr-writing-and-metadata, 20261005-provider-settings-upkeep, 20261005-provider-sign-in-and-install, 20261005-server-update-banner, 20261005-upstream-ui-sync, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/112
reproduced_on: 4c893fef6
---

# X17: Popover anchoring with every side/align area and automatic flip and shift (`position-area`, `position-try`)

## Summary
T3 Code positions tooltips, hover cards, menus and popovers with Base UI's positioner: a side
(top/bottom/left/right/inline-start/inline-end), an alignment (start/center/end), an offset, and
automatic collision handling that flips the side and shifts the alignment against the window's
5 px edge, exposing the available width to the popup. exact2 places a popover above or below its
invoker. The clone computes, per call site, where Base UI would land and hard-codes side,
alignment and a transparent "tail" that pushes an end-aligned menu back inside the window. exact2
needs the CSS Anchor Positioning vocabulary (all `position-area` values, `position-try` fallbacks)
on macOS so a popover lands where the web lands for any window size and any anchor position.

## Why this issue arose

### The T3 Code behavior
- **Positioner defaults.** `apps/web/src/components/ui/tooltip.tsx:82-132`: side `top`, align
  `center`, `sideOffset` 4, positioner `max-w-(--available-width)`; popup scales from 98 % and
  fades (`data-starting-style:scale-98 opacity-0`). `ui/preview-card.tsx:9-40`: side `top`, align
  `start`, `sideOffset` 6. `ui/menu.tsx:20-65`: `sideOffset` 4, align `center`, submenus side
  `inline-end` with `alignOffset` −5 (:303-321). None of these set collision props, so Base UI's
  defaults apply (flip the side, shift the alignment, padding about 5 px — to confirm in Base UI's
  docs at `issue-open`; the clone's comments say "the window's 5pt collision edge").
- **Visible results.** A tooltip on a control near the top edge appears below it; a menu anchored
  at the right edge slides left so it stays inside; a hover card taller than the space above flips
  below. The popup's transform origin follows the side. The PR hover card opens after 350 ms
  and closes after 120 ms (`PullRequestLinkPreview.tsx:110-113`); the clone's `shell-tip.contract:1`
  records a 600 ms open delay for tooltips; tooltips close on scroll (A17), Escape and trigger leave.
- Users see this on: thread hover cards, sidebar and composer tooltips, the PR hover card and
  checks/stack popovers, model and provider menus, the usage tooltip, update and sign-in
  popovers (plan tickets listed in `blocks`).

### What exact2 does today
- `EXACT2-GAPS.md` summary row X17: "Popover side areas and `position-try` flips | Hover cards and
  tooltips that flip near edges | contract/host | fixed placement". Its "Already fixed on exact2
  main" list: "Popover placement above/below the invoker with `position-area` (`2c6b551ba`)."
  (from framework source at `c1522fdac`, checked against `main` `d2cb661eb`).
- Bundled library (`20261005-platforms-v3`): not covered; **unknown**. Not reproduced.
- Observed in the clone: the clone's own comments and tests record the workaround and its
  limits (below). `AGENT-HANDOFF.md` lists "tooltip offsets; Send tooltip shifted; flipped hover
  card overhang; dialog and popover shadows faint or missing" among declared differences.

### Where the clone hits it
- `shell-tip.contract:1-40` (`Tip`, `TipBubble`): "`side` is where the reference lands after
  collision flipping; `align` where it lands after shifting"; `inset` "moves an end- (or start-)
  aligned bubble past the trigger's edge, as Base UI's 5pt collision shift lands it against the
  window edge"; bubbles are 360 pt rows because a 320 pt card shifted far in must not wrap early.
  Each call site passes the answer it measured in the oracle.
- `r6-pr-logic.ts:208-232` `prCard(…, room, row)`: computes `flipped` (`width > room - 5`) and
  `clamped` from text-width tables (`width12`) because "the reference portals the card; here it
  is drawn inside the details card, which clips" (hover card on the thread details PR row).
  `r6-pr.test.ts` "the card flips to end-aligned where it would cross the window".
- `pages-prs.contract:161-166`: "An open popover sits at its invoker's bottom-left, clamped to the
  window, so an end-aligned menu carries a transparent tail as wide as the gap from its trigger's
  right edge to the window's" (`tail=(viewportWidth - filtersRight)`, with hand-computed
  `filtersRight`/`providerRight`).
User-visible differences: a card or menu placed by a fixed answer is wrong at any window width,
zoom or anchor position the oracle was not measured at (the 840×620 minimum is the main
exposure); popovers drawn inside a clipping parent are cut where the web portals them;
a hover card that would not fit above does not flip unless its call site computes it.

## Why it must be resolved
Complete parity means popups land where the web puts them at every size, including the 840×620
minimum and a resized or full-screen window. The workaround is per-call-site arithmetic with
text-width tables: it breaks when fonts, text or the interface font size (`20261005-interface-font-size`)
change and has to be re-derived for each new popover. Plan rows that carry the declared
difference: popover and tooltip rows in `20261005-pr-handoffs-and-quick-actions`,
`20261005-pr-header-actions-and-stacks`, `20261005-pr-writing-and-metadata`,
`20261005-pr-links-previews-and-routing`, `20261005-composer-fidelity`,
`20261005-legacy-sidebar`, `20261005-upstream-ui-sync` (A16 reads the popover's placement near the
window edge at 840 pt) and the five settings/usage/provider/server tickets. Impact: nonblocking
today; leaving it means every pixel pair near an edge needs a written cause.

## Requested support
The web way (CSS Anchor Positioning), macOS host first:
- `position-anchor` + `position-area` with all values (the nine areas plus `inline-start/end`
  spans), `anchor-size()`, and `position-try-fallbacks: flip-block, flip-inline` /
  `position-try-order` / `@position-try`, so a popover chooses a fallback when it would overflow
  the window; `position-visibility: anchors-visible` (hide when the anchor scrolls away).
- A way to read the available size for `max-width` (Base UI's `--available-width`): the
  `max-width: anchor-size()`-style functions or an `env`-like value for the free space.
- Popovers escape ancestor clipping (top layer), as `popover` does on the web.
Alternative B: expose the anchor and window rectangles reactively (X22) and let apps compute the
fallback; this keeps today's per-site arithmetic and only removes the measuring. Trade-off: A
fixes every popover at once and matches Chrome; B is cheaper for the framework but leaves the
duplication in apps.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: a `button` with
`popovertarget` at the top-right corner and a 320×120 `popover` using
`position-area: bottom span-left` and `position-try-fallbacks: flip-block`. Steps: resize the
window so the button is within 40 px of the right edge, then of the bottom edge. Expected
(Chrome): the popover shifts or flips to stay inside the window. Actual: placed by the fixed
area, overflowing. Clone scenario: thread details PR row hover card (`r6-pr.contract`) at 840×620
with the details card narrow; compare with the oracle's card.

## Acceptance for the fix
- Conformance case against Chrome: the same anchor/popover pair at four window sizes lands at
  matching rectangles (±1 px) for side, align and fallback.
- Agent: `layout <popover testId>` at 1280×840 and 840×620 shows the flipped/shifted frame;
  screenshot pairs with the oracle for hover card, menu, tooltip near an edge.
- Clone: the `prCard` flip test, the `pages-prs.contract` tail and the `Tip`/`TipBubble` insets
  are removed and the pairs still match.

## App adoption after resolution
Replace `TipBubble` side/align/inset tables, `prCard`'s `flipped`/`clamped`/`maxWidth`, and the
`tail` arithmetic in `pages-prs.contract` with `position-area`/`position-try`; delete the
`width12` tables if nothing else uses them; update `r6-pr.test.ts`. Declared near-edge
differences in the tickets above are deleted. `issue-close` verifies the edge pairs at 840×620
and the removal of the arithmetic.

## Status and next action
Published 2026-10-06 as [#112](https://github.com/ccheever/exact2/issues/112) (reproduced on exact2 `4c893fef6` before filing). Decided upstream on 2026-10-08: see the last section.

## Decided upstream (2026-10-08): waits for main fix of #112 (the next core investment)

[Charlie on #112](https://github.com/ccheever/exact2/issues/112#issuecomment-6055584392): "Choose CSS position-area and flip fallbacks for invoker popovers. … Start with invoker popovers and
flip-block/inline/start … Prefer CSS fallback order to an unconditional macOS-only clamp."
- Waits for main fix of [#112](https://github.com/ccheever/exact2/issues/112), then an adoption round: invoker popovers (`popovertarget`: the menus, the Check
  out, More and stack menus, the update details popover, the machine list) will flip near an edge.
- **Narrowed:** the first slice does not cover state-driven layers (tooltips, hover cards, the Usage popovers),
  `anchor-size()` or `position-visibility`. Those keep the app's own placement (the hover layer of
  `fix-hover-cards`, #307).
- **Different design:** placements that lean on the macOS clamp (the `tail` in `pages-prs.contract`, the margins
  under `position-area="bottom span-right"`) move if CSS fallback order replaces the clamp; re-drive them at
  adoption. No new per-site arithmetic meanwhile.
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): selected next core investment; larger design (amend LLP 1021's no-flip line first).
