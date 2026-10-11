---
name: 20261011-device-player-compact-and-rail-keys
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-device-player-compact-and-rail-keys
pr_url: null
verified_commit: null
---

# The details card's compact and essential rows, its popover beside the player, and the device rail menus' keys

## Outcome

The user decided on 2026-10-11 that every item below is built now, matching the reference T3 Code `1e2ecbd975`. They
are the remaining differences of [floating-device-player](closed/20261005-floating-device-player.md) ("the card has one
density"; "when no placement is left the inline card hides instead of becoming the popover") and of
[audit-wave-followups-4](closed/20261010-audit-wave-followups-4.md) ("Found, not changed": the device rail's menus draw
their rows beside their `KeyMenu`).

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| DP-1 | The thread details card has one density: its full content, scrolled when a floating player folds it (`thread-details-card-layout.ts` ports `resolveThreadDetailsCardDensity` with its tests, unused) | `ThreadDetailsCard` measures its content per density and folds it: `full` (everything), `compact` (no Run on, no workspace row, no Automations, no Lineage, Version Control unseparated, the Git action group hidden unless an action runs or just finished), `essential` (compact without Open in) |
| DP-2 | With the inline card open and the floating player resized into it so that no 160 pt placement is left, the card hides; the header button keeps toggling the hidden inline card | `ThreadDetailsCard` turns into the header's popover (`mode = placement ? "inline" : "popover"`); the header toggles the popover; an inline placement coming back closes it |
| DR-1 | The device rail's menus (Text size, Rotate, More) draw their rows beside their `KeyMenu`, so a key at a focused row (one the pointer rests on) reaches no menu | Base UI `Menu`: on a focused row ↓ / ↑ move the highlight and wrap, Home / End go to the ends, a letter to the next row starting with it, Enter / Space press it, Escape closes the menu and gives the focus back to the trigger |

## Steps

1. Reference first: the source below; CDP on the reference lane for the card's three densities and its popover beside a
   floated player, and the rail's menus by keys.
2. DP-1: the canvas source (`chat-canvas-view.ts`) measures the card's full and compact heights from `t3-frame` frames
   (the content, and the parts only `full` shows), keeps them per thread and presentation as the reference's
   `measurements`, and answers the density (`resolveThreadDetailsCardDensity`); the card (`shell-details.contract`)
   shows each density's rows; `R4GitRows` hides the Git action group when compact.
3. DP-2: the canvas answers whether the card is inline or a popover and counts the switches; the window's toggle opens
   the popover in popover mode (also when the player took the inline place), and a switch back to inline closes it.
4. DR-1: the rail menus' rows go inside their `KeyMenu` (`r6-device.contract`); the shared rule's test stops naming the
   rail as the exception.
5. Bun tests; one agent drive on a lane with a fixture device hub (no real devices); before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| DP-1 | reference comparison; Bun tests of the density and of the rows each density shows; agent drive: the player resized under the card at 1280×840 and a short window | before / after / reference images |
| DP-2 | reference comparison; Bun tests; agent drive: the player resized into the card until no placement is left, the header button opens the popover, the player shrunk back closes it | before / after / reference images |
| DR-1 | reference comparison; Bun tests of the shared rule; agent drive: a rail menu row focused by the pointer, ↓ / End / Escape | before / after text record and image |
| DP-1, DP-2, DR-1 under real pointer and keys | real-input steps for the coordinator's session | — |

## Reference

- `apps/web/src/components/chat/ThreadDetailsCard.tsx` (placement, mode, per-density measurement keyed by thread and
  preferred width, `cardBottom` from the full height), `threadDetailsCardLayout.ts`, `ThreadDetailsPanel.tsx` (the rows
  per density), `components/GitActionsControl.tsx` (`compact`: the action group only while an action runs or after
  it), `components/BranchToolbar.tsx` (panel sections), `components/ChatView.tsx` (`threadPanelPresentation`,
  `toggleThreadPanel`), `rightPanelStore.ts` (`ThreadPanelVisibility`: `inlineOpen`, `popoverOpen`).
- `apps/web/src/components/device/DeviceControlsRail.tsx` (three Base UI `Menu`s: Rotate on Android, Device text size,
  More device actions), `components/ui/menu.tsx` (Base UI Menu: `useListNavigation` with `loopFocus`, typeahead).
