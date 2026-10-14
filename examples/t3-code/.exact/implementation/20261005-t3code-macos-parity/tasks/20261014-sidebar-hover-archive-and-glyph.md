---
name: 20261014-sidebar-hover-archive-and-glyph
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-sidebar-hover-archive-and-glyph'
pr_url: null
verified_commit: null
---

# The sidebar's hover after a control's tip, the tip on a press, the focus after Archive, the archived delete confirm and the branch pull request glyph

## Outcome

The user asked on 2026-10-14 to fix everything fixable in `realinput-1011c`'s known differences (STATUS "Known
differences", table 3 "Found, not in scope"); the coordinator's triage (fix wave, clone task
`sidebar-hover-archive-and-glyph`) gave this task these rows. The user decided on 2026-10-11 that each is built to match
the reference T3 Code.

| Id | Clone (`realinput-1011c`, the bundle of `1f813789b`) | Reference (check it first) |
| --- | --- | --- |
| SW-2 | B5: after the pointer passes over a row's Snooze or Settle and back into the row, the row's hover card closes for good and the row loses its hover (no surface, no actions) while the pointer stays in it ([pair](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/SW-2-inside-end-pair.png)) | The card closes for one frame pair (83 ms) while the control's tooltip is up and reopens; at the end the row is hovered with its card open |
| SW-3a | B5: a press on Settle (no movement) leaves the "Settle thread" tooltip up ([pair](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/SW-3-pressed-pair.png)) | The tooltip is gone from the press on (Base UI tooltips close on press); the row stays hovered, no card |
| R11-X63 | B14 step 3: after "Archive thread" from a row's menu, the focus is in the composer, so ⌘Z undoes the composer's text and the thread stays archived ([pair](https://raw.githubusercontent.com/ccheever/exact2/dae1a9619a430b2ecdea34ac21af9c34cdd37a1d/realinput-1011c/b14-r11/pairs/R11-X63-thread-undo-pair.png)) | After the archive the focus is the page (AXWebArea, no editable), not the composer; ⌘Z unarchives the thread. The row it right-clicked is a focusable `div role="button" tabIndex={0}` |
| G4-14b | B2 step 5: Settings › Archive's delete confirm reads 'Delete thread "Archive fixture B"?'; each row's Unarchive button is named "Unarchive Archive fixture B" ([notes](https://raw.githubusercontent.com/ccheever/exact2/7f34eae2b1671e15926020b44cadef91b08146d6/realinput-1011c/b2-panels-browser/G4-14-crash-summary.txt)) | 'Delete thread "this thread"?' (the live thread store holds no archived thread, so `confirmAndDeleteThread`'s title falls back); the button's name is its text, "Unarchive" |
| B12-PR-GLYPH | B12: the "Studio branch" thread (branch `feature/release-notes`, the lane repository's pull request #113) draws no pull request glyph, in the default sidebar or the legacy one ([notes](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/00-b12-notes.txt)) | "PR #113 - Open: Document the release steps" on that row |

## Steps

1. Reference first: the source (below) for each row; the realinput-1011c reference captures for SW-2, SW-3, R11-X63,
   G4-14 and B12.
2. SW-2: find why the row's hover is not given back when a control's tip ends inside the row (the window's
   `sidebarHover` and the root's half; main #327 made hover the hit's ancestor path, so the row keeps the host's hover
   under its controls and hears no new enter); give it back, with its card, and a test in the sidebar hover suite.
3. SW-3a: a pointerdown on Settle or Snooze closes the tip (and the card) at once; the row stays hovered.
4. R11-X63: a pick from a row's menu lets go of the composer's focus, so the focus is not in the composer after the
   archive, as in the reference, and ⌘Z reaches the app's archive undo.
5. G4-14b: the confirm's title and the button's name as the reference.
6. B12: trace the glyph's pull request match on the GitHub lane's data and port the reference's fallback.
7. Tests; one agent drive, before and after, on this worktree's builds; real-input steps for the final session.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| SW-2 | reference source and capture; Bun test (the window's and the root's hover actions run with Contract's action semantics); agent drive (`tap … hover` over the card, the Settle, the title) | before / after / reference images |
| SW-3a | reference; Bun test; agent drive (`tap settle-… down`, held) | before / after / reference images |
| R11-X63 | reference; Bun test (the root's `sidebarRun` lets go of the composer on a menu pick); agent drive (focus, then ⌘Z) | before / after images, state |
| G4-14b | reference; Bun test (the confirm's title, the button's name); agent drive (the button's name in the tree) | text before / after |
| B12-PR-GLYPH | the GitHub lane's real data (the thread's `branchPullRequest`, the server's `pullRequests.summary`); Bun test on that data; agent drive with the GitHub lane paired | before / after / reference images |
| SW-2, SW-3, R11-X63 step 3, G4-14, B12 under real input | real-input steps for the final session | — |

## Reference

- SW-2, SW-3a: `apps/web/src/components/Sidebar.tsx` (T3 Code `1e2ecbd975`, MIT): the row is the trigger of the
  details `Tooltip` (`detailsTooltip`, `SidebarThreadTooltip`, disabled while the snooze menu is open); each control is
  its own `Tooltip` (Settle, Snooze `SnoozeMenuButton`, Discard draft, Un-settle, Wake); `handleActionPointerDown` starts
  the section sweep; `components/ui/tooltip.tsx` (Base UI): a press closes the tooltip; the group shows one at a time.
- R11-X63: `Sidebar.tsx` the row card `div role="button" tabIndex={0}` with `onContextMenu={handleContextMenu}`;
  `hooks/useThreadActions.ts` `archiveThread` with `ThreadUndo`; the app's ⌘Z (`thread.undo`) with no editable focus.
- G4-14b: `components/settings/SettingsPanels.tsx` `ArchivedThreadsPanel` (the Unarchive `Button` with the ArchiveX
  icon and the text "Unarchive", no `aria-label`; `handleArchivedThreadContextMenu`), `hooks/useThreadActions.ts`
  `confirmAndDeleteThread` (`resolveThreadTarget` → `readThreadShell`, `?? "this thread"`).
- B12-PR-GLYPH: `components/ThreadStatusIndicators.tsx` `useLinkedThreadPullRequest` (no current link in
  `pullRequests`: `(!supportsLinks ? linkedPullRequest : null) ?? branchPullRequest`, read with
  `linkedPullRequestDetailAtom`, the server's `pullRequests.summary`), `prStatusIndicator`, `Sidebar.tsx` and
  `LegacySidebar.tsx` (`prBadge`, the legacy glyph and `handlePrClick`'s `prStatus.url`);
  `packages/client-runtime/src/state/pullRequests.ts` `createLinkedPullRequestSummaryAtomFamily` (stale after 60 s,
  refreshed every 60 s), `pullRequestDetailToVcsStatus`; `apps/web/src/components/Sidebar.logic.ts`
  `useSidebarRowSubscriptionLease` (a row near the viewport).

## Next action

Build every row, prove it, run the checks, open the draft PR.
