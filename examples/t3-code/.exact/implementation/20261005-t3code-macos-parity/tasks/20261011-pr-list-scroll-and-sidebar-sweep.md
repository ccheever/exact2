---
name: 20261011-pr-list-scroll-and-sidebar-sweep
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-list-scroll-and-sidebar-sweep
pr_url: null
verified_commit: null
---

# The Pull Requests list's place across a window refocus, and the sidebar sweep's hover

## Outcome

The user decided on 2026-10-11 that these two framework gaps recorded but never filed are built now, matching the
reference T3 Code: reproduce each in the clone first, fix it in the clone if the clone causes it, and if it is a real
framework gap, write a minimal one-file repro for the coordinator to file on main (keeping or adding a clone workaround).

| Id | Clone (recorded) | Reference (check it first) |
| --- | --- | --- |
| PS-1 (X58) | The Pull Requests list goes back to its top when the window is focused again and the list re-reads (pr-list-live-refresh's investigation A and attempt 4); a re-read with no focus change and the minute tick keep the place | `_chat.pull-requests.tsx`: the list reads again on the window's `focus` (`useLiveRefresh`); React keeps the rows (`answered ?? carried`), so the reader stays where they were |
| PS-2 (X13) | During a row-action sweep the hover card or a control's tooltip open at the press stays until the release (the host sends no hover events during a pan); Escape cancels through an invisible window shortcut (r12); after a cancelled sweep the hover is not where the pointer is | `Sidebar.tsx` `startActionSweep` / `Sidebar.pointer.ts` `SidebarPointerSensor`: past 6 px the list takes `**:pointer-events-none`, so the card or tooltip goes as the sweep takes over; Escape (document `keydown`, capture) cancels; a cancelled release applies nothing and its click is swallowed; the rows take the pointer again |

## Steps

1. Reference first: the sweep driven with trusted mouse and key events over CDP on the reference lane (`ref-app.sh`,
   five fixture threads in the lane's T3 home); the list's live refresh read from source.
2. PS-1: reproduce on the feature tip's build (`t3-code-evidence-base`) with the GitHub lane (sandbox
   `daehyeonmun2021/playground`, 99 open pull requests) paired through Settings › Connections: wheel down, the window
   refocused (`prefer has-focus`), with and without a list read due; find what moves the list (the scroll node's id
   and offset in `layout`, the runner's trace in `logs`).
3. PS-2: the same sweep on the clone's agent driver (`tap … down`, `tap move`, `type … key Escape`, `tap up`), before
   and after, with the card open at the press and with the focus already on the pressed button.
4. Fix the clone's side; a framework cause gets a one-file repro in the report and a row in `EXACT2-GAPS.md`.
5. Tests; one agent drive of the after build; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| PS-1 | reproduce, then the cause; agent drive (window refocus with a list read due, the scroll node's id and offset) | before / after text record |
| PS-1 under a real refocus | real-input steps | — |
| PS-2a: the card or tooltip goes as the sweep takes over | reference (CDP, trusted events); agent drive with the focus already on the pressed button; Bun test | before / after / reference images |
| PS-2b: Escape cancels the sweep, the release applies nothing | reference; agent drive; Bun test | before / after / reference images |
| PS-2c: after a cancelled sweep the row under the pointer is hovered, through the release, no card until the pointer moves | reference; agent drive; Bun test | before / after / reference images |
| PS-2 under a real trackpad drag and real Escape | real-input steps | — |

## Reference

- PS-1: `apps/web/src/routes/_chat.pull-requests.tsx` (`useLiveRefresh(() => refreshList(true))`, `listData =
  answered ?? carried`, the list's `scrollRef`), `apps/web/src/hooks/useLiveRefresh.ts`.
- PS-2: `apps/web/src/components/Sidebar.tsx` `SIDEBAR_DRAG_DISTANCE = 6`, `startActionSweep`
  (`setActionSweep`, `**:pointer-events-none` on the list while `actionSweep !== null`), the row's
  `handleActionPointerDown`, the Settle button's `Tooltip` (`TooltipPopup` "Settle thread") and the row's
  `ThreadHoverCardPopup` under one `TooltipProvider` (`delay={150} closeDelay={0}`);
  `apps/web/src/components/Sidebar.pointer.ts` `SidebarPointerSensor` (`keydown` Escape → `cancel`, `finish`, the
  swallowed release click).
