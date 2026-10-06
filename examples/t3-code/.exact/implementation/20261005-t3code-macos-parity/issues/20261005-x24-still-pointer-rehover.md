---
name: 20261005-x24-still-pointer-rehover
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap
blocks: [20261005-diff-review-engine, 20261005-floating-device-player, 20261005-legacy-sidebar, 20261005-pr-code-tab, 20261005-upstream-timeline-and-markdown]
upstream_url: null
reproduced_on: null
---

# X24: Hover state follows layout changes under a stationary pointer

## Summary
In T3 Code (Chromium), when a list changes under a resting mouse pointer — a thread is settled
and the next row slides up, ⌘Z brings it back, a diff slice loads, a pane resizes — the element
now under the pointer becomes hovered without the mouse moving, so its hover actions, tooltips
and hover cards appear. In exact2 hover comes only from pointer enter/exit when the pointer moves.
The clone sends a synthetic "mouse entered" to the right view after the thread list re-renders,
but only for that one list and only in real (not agent) runs. exact2 should re-hit-test the last
known pointer position after layout and scroll, as a browser does.

## Why this issue arose

### The T3 Code behavior
- **Sidebar.** Settling a row (button, sweep or keyboard) removes it from its section; the row
  that slides under the pointer shows its hover-only actions and can open its hover card; ⌘Z
  within five seconds restores the row ("Settled 1 thread, ⌘Z to undo") and the restored row is
  hovered again if the pointer still rests there. The reference has no code for this: it is browser
  behavior (Chromium sends a fake mouse move after layout and scroll; to confirm in a conformance
  run at `issue-open`). Rows reveal controls with CSS hover (`group-hover/sidebar-row`,
  `apps/web/src/components/Sidebar.tsx:1633`) and tooltips/hover cards open on pointer enter (Base UI).
- **Other places the same rule matters.** The hover-revealed gutter "+" in Changes and Code when slices
  load (`diffs/AnnotatableCodeView.tsx` `enableGutterUtility`); the PR list row quick actions
  after a refresh; the floating device player's controls after it moves (plan tickets below); a
  tooltip that A17 closes on scroll must not reopen under a still pointer until the pointer moves
  (`ui/tooltip.tsx` `TooltipScrollDismissArea`; ticket row "stays closed under a still pointer").

### What exact2 does today
- `EXACT2-GAPS.md` summary row X24: "Hover re-hit-test under a still pointer after layout | Row
  under the pointer after ⌘Z / list change | host | `t3-rehover` hook". Detail: "Hover comes only
  from NSTrackingArea enter/exit when the pointer moves (`Mac/NodeViewMac.swift:321-354`)."
  (framework source at `c1522fdac`, checked against `main` `d2cb661eb`).
- Bundled library (`20261005-platforms-v3`): not covered; **unknown**. Not reproduced.
- Observed in the clone: the AppKit test below passes; the real-input check ("the re-hover after a
  real ⌘Z") is "never run; agent pass" (`AGENT-HANDOFF.md:196`, R-list; README "Known" notes).
  In agent runs the passes are not scheduled at all (`R10Connect.swift:129` `guard !agent`), so
  no agent drive can show the effect.

### Where the clone hits it
`modules/apple/R10Connect.swift:15-17,125-156` and `sidebar.contract:110` (`hook="t3-rehover"` on
the thread list). After the hooked list re-renders it runs passes at 30, 200 and 450 ms
(`schedulePasses`), hit-tests the pointer (`NSEvent.mouseLocation`), walks up to the innermost view
that has an `.mouseEnteredAndExited` tracking area, and calls `mouseEntered` on it when it differs
from the last one. Test: `macos/tests/r10-connect/main.swift:112` `testAStillPointerHoversTheRowThatSlidesUnderIt`.
Differences a user can see: only the sidebar list is covered (not diff slices, the PR list, the
floating player or the timeline); it never sends the matching exit, so a previously hovered view
relies on AppKit's own exit; the three fixed delays can miss a slower relayout; and nothing runs
while a button is down or another window covers the pointer (`mayHover`).

## Why it must be resolved
The goal is a complete clone; hover actions appearing under a still pointer after Settle or ⌘Z
is a daily sidebar interaction, and the same gap will recur in each new list-like surface. Plan
rows that carry it: the A17 scroll drive in `20261005-upstream-timeline-and-markdown`, the
project list in `20261005-legacy-sidebar`, the pill after the lane moves in
`20261005-floating-device-player`, the gutter "+" in `20261005-pr-code-tab` and
`20261005-diff-review-engine`, and the real-input rows of `20261005-round12-wrapup`. Keeping the
hook means one more `t3-rehover` per list, timers tied to layout speed, and no agent-checkable
evidence.

## Requested support
The web way: after layout, scroll or style changes that move the node under the last pointer
position, the user agent updates the hover target (Chromium dispatches a synthetic mouse move;
`:hover` and `pointerenter/leave` follow).
- **A (preferred).** On macOS the host re-hit-tests the last known pointer position after a
  frame that changed geometry (also after scroll), and sends enter/exit to the nodes whose hover
  changed. Coalesce to once per frame; skip while a button is down or another window owns the
  pointer.
- **B.** A node attribute (`hover-track` or the `t3-rehover` idea made generic) that opts a
  subtree in; cheaper, but every app must remember it.
Trade-off: A matches the web and needs no app code; it costs one hit-test per changed frame.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`". Minimal app: three rows with `hover=`
highlighting; a `button` below with `press` that removes row 1. Rest the pointer on the
boundary between row 1 and row 2, press the button via keyboard (Return) or an agent `tap`, so
the pointer never moves. Expected (Chrome page with `:hover` styles): row 2 highlights within
about 100 ms. Actual: no change until the pointer moves. Clone scenario: five fixture threads,
hover card 2, press Settle by keyboard, then ⌘Z (`AGENT-HANDOFF.md` real-input R-list); needs a real
pointer (attended session) because the agent's pointer is its own (X8).

## Acceptance for the fix
- AppKit test with a stubbed pointer location: a geometry change under it produces an enter on
  the new view and an exit on the old one, without any monitor installed.
- Conformance case against Chrome for the same DOM and timing window.
- Agent: after `tap` on the removal button, `state` shows row 2's hover id set (the hover state is
  readable); attended session: Settle then ⌘Z with a resting trackpad shows the hover each time.

## App adoption after resolution
Remove `hook="t3-rehover"` from `sidebar.contract:110`, the rehover half of `R10Connect.swift`
(keep wake and select-on-open), and the test `testAStillPointerHoversTheRowThatSlidesUnderIt`;
drop the README sentence about `t3-rehover`. Re-run the Settle/⌘Z, A17 and gutter rows named
above. `issue-close` verifies that no rehover hook remains.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's
approval; publication only after approval).
