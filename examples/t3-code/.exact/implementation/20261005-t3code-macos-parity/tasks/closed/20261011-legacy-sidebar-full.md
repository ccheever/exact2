---
name: 20261011-legacy-sidebar-full
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-legacy-sidebar-full
pr_url: https://github.com/ccheever/exact2/pull/469
verified_commit: 1f813789b
---

# The "Sidebar (legacy)" in full: PR links, ports, file drops, keyboard Archive, branch threads, background environments

## Outcome

Decision U22 (the user, 2026-10-11): the legacy sidebar is built in full, as the reference. The first build
([legacy-sidebar](20261005-legacy-sidebar.md), PR #143) left six differences in its "Remaining differences"
list. This task builds each of them so the legacy sidebar does what `LegacySidebar.tsx` does.

| Id | Clone (feature tip `6db5cadea`) | Reference (check it first) |
| --- | --- | --- |
| LS-1 | A thread row's pull request glyph is an image: a press on it opens the thread (the row's press) | The glyph is a link (`handlePrClick`, `useOpenPrLink`): with a thread's route open it opens the pull request beside the clicked thread in the right panel and goes to that thread; otherwise (a page, an unsent draft) on the Pull Requests page; a link no project reads follows "Open links in"; ⌘ or Ctrl opens the system browser |
| LS-2 | No discovered-port button (the terminal-running glyph was built later and matches `terminalStatusFromRunningIds`) | A Globe2 button "Open localhost:<port>" (tooltip "Open localhost:<port>" plus " (+N)") for a thread whose terminal serves a discovered local server: it goes to the thread and opens the port in a Browser tab beside it; a failure toasts "Unable to open preview" |
| LS-3 | Files dropped on a row do nothing | `makeWorkspaceFileDropHandlers` on the row: a file drag rings the row (`ring-1 ring-inset ring-primary/70`); the drop queues the files for that thread (`sidebarPendingFileDropStore`), goes to the thread, and its composer adds them as dropped files (images to the shelf, other files as chips) |
| LS-4 | Archive shows on hover only, so the keyboard never reaches it; the project row's New thread likewise | Archive and New thread stay in the tab order (opacity 0) and show on `group-focus-within` as on hover; the meta (time, jump label) and the remote badge hide the same way; with Archive confirmation on, the Confirm button takes the focus; Archive's "Archive" tooltip only without confirmation |
| LS-5 | "New thread on <branch>" opens a plain draft in the project | The draft carries the branch: `handleNewThread(projectRef, { branch, worktreePath, envMode: worktreePath ? "worktree" : "local", startFromOrigin: false })` |
| LS-6 | On a background environment's project, Rename, Group into..., Remove and New thread are disabled (New thread toasts "Open a thread on that environment first."), Project settings does nothing on a remote-only project; a background thread's menu, Archive and inline rename do nothing; stale duplicate project rows of one checkout are not merged (`environmentGrouping.test.ts`) | Every project and thread action targets the member's or thread's own environment (`projectEnvironment.update/delete`, `threadEnvironment.*`, `handleNewThread(scopeProjectRef(member.environmentId, …))`); Project settings opens Settings › Projects scoped to the logical project; `buildProjectGroups` keeps the freshest row of a physical checkout and its identity |
| LS-7 (found while building) | The row's title, worktree glyph and a collapsed project's status dot have no tooltip; the worktree's label names the whole path | Each has a TooltipPopup: the title in full, `Worktree: <last segment> (<branch>)` (ThreadWorktreeIndicator, formatWorktreePathForDisplay), the status label |
| LS-8 (found while building) | List changes do not move; "Project is not empty"'s Delete anyway is the default (blue) button | autoAnimate (`SIDEBAR_LIST_ANIMATION_OPTIONS`, 180 ms ease-out, none under reduced motion) on the project list and each thread list; the toast's `actionVariant: "destructive"` |

LS-7 and LS-8 were in the first build's "Remaining differences" too; U22 is the legacy sidebar in full.

## Steps

1. Reference first: `LegacySidebar.tsx` (the row, the project item, the menus), `lib/openPullRequestLink.ts`,
   `portDiscoveryState.ts`, `components/preview/openDiscoveredPort.ts`, `components/chat/workspaceFileDrop.ts`,
   `sidebarPendingFileDropStore.ts`, `hooks/useHandleNewThread.ts`, `packages/client-runtime/src/state/projectGrouping.ts`,
   `environmentGrouping.test.ts`; the reference app on the lane where the fixture allows.
2. LS-4 and LS-5 (view and draft context), LS-1 (the glyph as a button, the panel or the page), LS-2 (the
   sidebar's own discovery stream, the button), LS-6 (fleet writes, the dedupe), LS-3 (a native drop target on
   each row through an access hatch, the drop consumed when the thread opens).
3. Bun tests for each; an AppKit test for the drop target.
4. One agent drive (before on the evidence worktree, after on this branch, the reference by CDP).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| LS-1 | Bun tests (panel beside the thread, the page, an ordinary link, ⌘); agent drive: press the glyph of a PR-linked thread | before / after / reference images |
| LS-2 | Bun tests (the sidebar stream, the button's label and tooltip, the open); agent drive with a server listening in a thread's terminal | before / after / reference images |
| LS-3 | Bun tests (the drop consumed on open: images to the shelf, files as chips); AppKit test of the row's drop target; real drag in the batch | text before / after (AppKit test), real-input steps |
| LS-4 | Bun test of the view tree; agent drive: Tab from a row to Archive, Enter, Confirm focused | before / after / reference images |
| LS-5 | Bun test (the draft's branch, worktree and mode); agent drive: "New thread on <branch>" from the menu (the menu's choice by the agent's menu pick) | before / after images |
| LS-6 | Bun tests (fleet writes for each action, the dedupe cases of `environmentGrouping.test.ts`); agent drive with a background environment | before / after images |
| Real input | Menus, a real file drag, keys | real-input batch steps below |

## Reference

- `apps/web/src/components/LegacySidebar.tsx` 316-944 (SidebarThreadRow: `handlePrClick`, discovered ports,
  `fileDropHandlers`, the archive and meta classes), 1158-2840 (SidebarProjectItem: `handleThreadFileDrop`,
  `handleProjectButtonContextMenu`, `handleRemoveProject`, `createThreadForProjectMember`,
  `handleThreadContextMenu` "new-thread-on-branch", the header's New thread and remote badge), 3809
  (`openPullRequestsInRightPanel={routeThreadRef !== null}`).
- `apps/web/src/lib/openPullRequestLink.ts` (`useOpenChangeRequestLink`, `useOpenPrLink`,
  `shouldOpenPullRequestExternally`), `apps/web/src/portDiscoveryState.ts` (`useThreadDiscoveredPorts`),
  `apps/web/src/components/preview/openDiscoveredPort.ts`, `apps/web/src/components/chat/workspaceFileDrop.ts`,
  `apps/web/src/sidebarPendingFileDropStore.ts`, `apps/web/src/components/ChatView.tsx` 10481-10512,
  `apps/web/src/hooks/useHandleNewThread.ts`, `apps/web/src/routes/projects.$projectKey.tsx`,
  `packages/client-runtime/src/state/projectGrouping.ts` (`buildProjectGroups`,
  `shouldReplacePhysicalProjectWinner`, `selectProjectIdentitySource`), `apps/web/src/environmentGrouping.test.ts`
  184-325.

## Cause and fix

- **LS-1** (`legacy-sidebar-links.ts` `openRowPullRequest`, `legacy-sidebar.contract` LegacyThread). The glyph is a
  button nested in the row's button (its own press and double-click, so the row neither opens nor renames), labelled
  and tooltipped with the status. A press reads the click's modifiers: ⌘ or Ctrl opens the system browser. With a thread's
  route open (`routeThreadRef !== null`: a server thread, or a draft once sent, `resolveActiveThreadRouteRef`; an unsent
  draft is none, which the reference lane confirmed), `prTarget` over the row's own environment's projects and capabilities
  finds the project; the pull request tab opens in that thread's right panel (`openPullRequestIn` on its panel by key) and
  the thread opens (`goTo`: `openSelected`, or `focusFleetThread` for a background row; the result `sidebar:opened` takes
  openThread's effects in `app.contract`). On a page, the exact project across the environments that read pull requests
  (the focused first) answers `sidebar:pr-page:<selection>`, which opens the Pull Requests page on it. Otherwise
  `openLink` ("Open links in"), with the row's thread when beside one.
- **LS-2** (`legacy-sidebar-ports.ts`, `browser-targets.ts`). The sidebar keeps its own `subscribeDiscoveredLocalServers`
  stream (key `sidebar-discovered-servers`, no configured URLs) while the legacy sidebar shows rows (`watchRowPorts`,
  each refresh); the discovery state is keyed so the Browser empty state's stream is untouched. Each background
  environment the fleet keeps connected subscribes too (`fleetPortsPass`/`fleetPortsEvent` in `settings-b-fleet.ts`).
  A row's ports are the servers whose terminal is its thread; the Globe2 button "Open localhost:<port>" (tooltip with
  "(+N)") goes to the thread and opens the port's URL for that environment beside it (`openUrlInPreview`), recording the
  visit; a background row's opens once its environment is the focus and its thread open (`openPendingPort`).
- **LS-3** (`modules/apple/T3RowDrop.swift`, `legacy-sidebar-drop.ts`, `app.json`). Exact's `drop` takes only the types
  `file_handlers` declares, which would also make the app a Finder viewer of every file (a tell the reference has not), so
  the row's button carries an access hatch (`t3-row-drop`, `data-thread-key`) and the module lays a drop target over it:
  AppKit picks a drag destination by registered types (`_hitTest:dragTypes:`), not `hitTest`, so the target takes file
  drags while clicks, hovers and right clicks still reach the row. A drag rings the row (1 px, primary at 70 %); a drop
  queues the files under the row's key (folders let go, as the row's `addFolders` does nothing) and presses the row; once
  that thread is open, the refresh takes its queue (`rowDropTake`) and adds the files as the composer's Attach does
  (`addAttachedFiles`, split from `attachFiles`): images to the shelf, other files as chips at the caret only in that
  thread's own editor (`editorInsert` now refuses another owner), else at the end of its stored draft.
- **LS-4** (`legacy-sidebar.contract`). Archive (and the project's New thread) stay mounted at opacity 0 with no pointer
  and show on hover or while the focus is anywhere in the row (`within` from each control's focus and blur), as
  `group-focus-within`; the meta and the environment badge hide the same way. Confirm takes the focus (`autofocus`), and
  the focus leaving it ends the confirmation; Archive's "Archive" tooltip shows only without the confirmation.
- **LS-5** (`legacy-sidebar-links.ts` `openBranchDraft`). After the draft opens, its context is the thread's branch, its
  worktree and `worktree`/`local` mode (`patchDraftContext`); a background thread's draft takes the same context on its
  environment. The default sidebar's same menu row (Sidebar.tsx does the same) uses it too.
- **LS-6** (`legacy-sidebar-fleet.ts`, `legacy-sidebar-commands.ts`, `legacy-sidebar-view.ts`, `sidebar-commands.ts`,
  `worktree-cleanup.ts`). A background environment's requests go through its own transport (`envAccess`, as
  desktop-activation.ts's): project Rename and Remove (`projects.mutate`), "Project is not empty" counted there, Group
  into... (this device's override), New thread (its draft, the environment becoming the focus), the thread menu (mark
  unread, copy path and id, rename inline from the menu or a double-click, delete with its worktree question and the
  orphaned worktree's removal there), Archive with its undo, and the multi-selection's actions. Project settings opens
  Settings › Projects on the project, its environment becoming the focus first (that page reads the focused environment).
  `legacyGroups` ports buildProjectGroups' physical dedupe: rows of one checkout keep the freshest, grouped by the
  freshest identity among them, and the stale rows' threads join the winner.
- **LS-7**, **LS-8** (`legacy-sidebar.contract`, `legacy-sidebar-view.ts`, `toast.ts`, `shell.ts`, `shell-toast.contract`):
  the three tooltips (the title's in `LegacyTitleTip`, which grows with the row so the title still clamps); project items,
  thread lists and thread rows carry `-exact-layout-transition` and an exit fade (`lsRowOut`), and a row that arrives in a
  shown list fades in once (`lsRowIn` while its `enter` holds: the projection tick it first appears in; not the sidebar's
  first draw, and not a windowed row mounted again as it scrolls into view), 180 ms ease-out, none under reduced motion;
  the toast's action takes `actionVariant: 'destructive'`. The host hovers only the nearest node with a `hover`
  handler (`MouseChainMac` `hoverUnderPointer`), so the title's tooltip trigger kept a row unhovered when the pointer
  entered it on the title (the first after drive: Archive stayed hidden and the press at its place opened the thread);
  every tooltip trigger in a thread row or a project header now tells its row (`LegacyRowTip`, `LegacyTitleTip`'s
  `relay` into `innerHover`), as group-hover holds for the whole row.

### Review round 1 (2026-10-12, the independent review of PR #469)

| Finding | Cause | Fix |
| --- | --- | --- |
| B1 (blocking): the row that opened its thread kept showing Archive, its time hidden, after the pointer left (LS-4's focus-within) | A mouse press makes the row's button first responder; nothing in the clone then moved the focus. The reference's ChatView focuses the composer on every thread change (ChatView.tsx 6569-6577, a frame later, not while the terminal drawer is open) | `select-thread` (client-ops-threads.ts) answers `focus:composer` when the open thread changed (a background row's environment switch counts) and that thread's terminal drawer is closed (`composerTakesFocus`, legacy-sidebar-links.ts); `goTo` (the pull request glyph, the port) answers `sidebar:opened:focus`, which `app.contract` turns into `focus("composer")`. A drop presses the row, so it takes the same path. The default sidebar's row press and the other `select-thread` callers (a notification's Open thread) focus the composer too, as the reference's shared ChatView does |
| B2 (blocking): a row or a project header stayed hovered when a tooltip trigger went away under the pointer (the status box as a press expands, Archive as Confirm replaces it, the port, a thread starting to run) | A trigger reports leaving only through its own hover(false); an unmounted node's events are dropped (Presenter `send`) | LegacyThread and LegacyProject clear `inner` on their own hover(true), which the host sends once its hit-test after the layout finds the row under the pointer (MouseChainMac `hoverUnderPointer`, `followPointer` after a relayout). The same class held for the focus: a control that goes with the focus sends no blur, so LegacyThread now names the control holding it (`focused`: row, pr, port, archive, confirm, rename) and counts only one still drawn (`inside` is derived; the rename field reports its focus too) |
| S6 (Confirm's blur): the focus moving to another control of the row ended the confirmation; found with it: the pointer moving from the row onto one of its own triggers (the title spans the row) ended it too | `confirmLeft` and `hover(false)` ended it at once; the host moves the hover from the row to a trigger, and the focus between controls, by a leave and an enter at one instant | `pointerLeft` and `focusLeft` hold when the pointer or the focus left (`performanceNow()`); Confirm shows only while neither has (`confirmShown`), and an enter more than 16 ms after the leave ends the confirmation (handleMouseLeave; handleBlurCapture's check a frame later). Archive starts a confirmation afresh |
| Found while fixing B1 (Bun suite): a selected thread the shell does not hold | It shows the empty state, which has no composer; the reference's effect returns without an active thread | No `focus:composer` for it (`no-active-thread.test.ts` keeps its empty message) |
| Found while fixing S6: the terminal glyph's tooltip did not relay (listed under "Seen while building") | Crossing it left the row, which now ends a confirmation | The legacy row wraps the glyph (`TerminalGlyph`, split out of `TerminalProcessIndicator`, which is unchanged for its other users) in `LegacyRowTip` |
| S3: the toast's destructive action was `#dc2626` in both themes | | `light-dark(#fb2c36, #fb414a)` (bg-destructive, `--error`), the clone's other destructive buttons' token |
| S6 (drops, ports) | A drop always pressed its row; nothing let a queued drop go; `openRowPort` recorded its pending open before the focus switch | The row button carries `data-thread-open` (app.json `thread-open`); T3RowDrop presses only a row that is not the open thread's (`drop(_:_:open:press:)`). A pressed row whose thread fails to open clears its drops (`rowDropClear`, `clearRowDrops`), and a refresh that finds another thread open (a landed navigation: ready, a thread open) clears drops queued before it for a thread still not open (its press found the app busy, or another thread opened first). A background port's pending open goes when its focus switch fails |
| S5 (Project settings on a project only a background environment has) | The clone's Settings › Projects reads and writes the focused environment's groups only (clone scope, not an Exact limit) | Declared in EXACT2-GAPS.md ("Legacy sidebar: Project settings on a background environment's project") with a proposed follow-up task (`settings-projects-fleet`: the page over every environment, writes through each environment's transport); a row for STATUS.md "Known differences" |
| S4 (evidence and coverage) | | LS-8 text before/after (`ls8-record.txt`); Bun: LS-1 on a background environment's row (the panel by the thread's key, then the focus switch; the page's `environmentId`) |

### Review round 2 (2026-10-12, the re-review of PR #469)

| Finding | Cause | Fix |
| --- | --- | --- |
| B2, left open: the pointer on a non-top row's Archive (or a collapsed project's status) as the thread starts running from a new message: the row re-sorts to the top in that batch, the host hovers the row that slid under the pointer, and the moved row kept `inner` (painted hovered while running, then Archive shown and its time hidden after the run) | Round 1 relied on the host hovering the row itself again; it does that only for the row still under the pointer. The leave the host sends to the hovered trigger (Presenter `hover`: the old node's leave, then the new node's enter) is dropped for a destroyed node (`send`), and Archive's trigger was under `when not t.running` | Every tooltip trigger of a legacy row and header keeps its box while its trigger is not drawn: `LegacyRowTip(shown=…)` is `display: none` and the trigger's own `when` moved inside it (pull request, port, worktree, terminal, Archive under `not confirmShown and not t.running`, the collapsed status). A hidden node is still the presenter's, so its leave arrives with the host's hit-test after the layout wherever the row went, and `relay(false)` clears `inner`. The focus is unchanged: the controls themselves still unmount (no hidden first responder) |
| LS-7, found in round 2: the title, pull request, worktree, port, terminal and Archive tooltips of each project's first row showed a sliver | `TipBubble` was drawn beside the trigger inside the thread list, which clips (`overflow="hidden"`, the reference's own, for autoAnimate); the reference portals every TooltipPopup | `LegacyRowTip` and `LegacyTitleTip` hand the bubble to the window's hover layer (`hoverTipAt`, `legacyTip`: kind "tooltip", Base UI's 600 ms, `frame(tipId)` of the trigger), the clone's portal for tooltips in clipping areas (hover-layer.contract). All legacy tips share one key, so the row or header the pointer is next on closes one whose trigger went with no leave (a rename replaces the title); Archive's press and a right-click on a row or header close it first (Base UI's closeOnClick), as either can remove the row. The list keeps its clip |
| Double-click race: `action nested(name)` set `focused` on the pull request and port buttons' `dblclick`, so the first click's `sidebar:opened:focus` moving the focus to the composer before the second click's dblclick left the row focused (Archive shown, time hidden) | The body wrote state the focus and blur own | `nested` only calls `stopPropagation()` (handleRowDoubleClick returns for a nested control); `focused` stays with `within` |
| Base moved to `ca4f28e35` (#482); two fixtures exported `contractComponent` with different semantics | `contract-component-fixture.ts` ran statements one after another with writes visible at once; #482's `contract-action-fixture.ts` follows Contract's action semantics | Merged; `contract-component-fixture.ts` is gone. `contract-action-fixture.ts` takes call statements (action props, injected actions, host commands, read with the action's starting state) and `drawn(marker, state, scope)` (the `when` / `else when` / `else` arms around a view line); the legacy row tests run through it (`mount` in the test) |

## Tests

- `legacy-sidebar-full.test.ts` (new): LS-1 (panel beside a thread and the thread opened, in place, the Pull Requests page,
  ⌘, "Open links in" both ways), LS-2 (the stream's subscribe and release, labels, the button's open and its failure; a
  background environment's stream and its deferred open), LS-3 (the drop taken for the open thread: chips, shelf, refused
  types, the editor owner; nothing asked when nothing is queued), LS-4 and LS-8 (the view's structure), LS-5 (worktree
  and local context), LS-6 (the dedupe cases of environmentGrouping.test.ts; background Rename, Remove with "Project is not
  empty" and Delete anyway, New thread; a background thread's menu, rename, archive with undo, delete with the worktree
  question; double-click rename; Project settings), LS-7 (tooltips and the worktree label; every tooltip trigger in a row
  relays its hover to the row), LS-8 (motion attributes, the arrival flag by tick, the destructive tone).
- `legacy-sidebar.test.ts`: the project menu has no disabled row for a background member; the worktree label.
- `macos/tests/row-drop` (AppKit, 5): the target is AppKit's drag destination over the row while `hitTest` gives clicks
  to the row; a drag rings it, a drop reports the row's key and its files without folders; queued drops are taken per key
  in order and staged as Attach stages them; review: a drop presses only a row that is not the open thread's; a clear lets
  the named drops go.
- Review round 1 (`legacy-sidebar-full.test.ts`, "review: …", 17 tests): `select-thread`'s `focus:composer` (another
  thread; not the open row, a ⌘-click or an open terminal drawer) and `sidebar:opened:focus`; LegacyThread and
  LegacyProject run by `contract-component-fixture.ts` (the component's own actions and derives in Bun) through the host's
  event sequences: the opened row after the composer takes the focus, the header's status trigger unmounted, Archive's
  trigger replaced by Confirm, the pointer onto the title, the focus between controls and out, a focused control that
  goes (port, rename); LS-1 on a background environment's row; stale drops, a failed open's drops, a failed port focus.
  The pre-fix contract fails the hover and focus cases (`review-hover-focus-record.txt`). Since round 2 they run through
  `contract-action-fixture.ts` (Contract's action semantics), with the host's leave modelled per trigger.
- Review round 2 (`legacy-sidebar-full.test.ts`, "review 2: …", 6 tests; `r12-sidebar.test.ts` reads `openMenu`): the
  pointer on Archive as the thread runs and re-sorts (the host's leave reaches the trigger only while `drawn`), a
  collapsed project's status as the projects re-sort, every trigger's box kept with `shown` and its `when` inside, the
  hover layer's tip (key, kind, text, side, align, the trigger's frame; none without a label), the row's and header's
  hover, Archive's press and both right-clicks closing a legacy tip, and the double-click race. Round 1's contract
  fails all six (`r2-review-record.txt`).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| LS-1 | pass (agent drive, Bun): a provider-less pull request link (Archive me open) opens the link and the thread stays (`opened-urls.txt`: `https://git.example.invalid/acme/app/pull/7`); with Studio branch open, Review changelog's glyph goes to Review changelog with #132 in its right panel, as the reference; on an unsent draft it opens the Pull Requests page on #132, as the reference; before, each press opened the row's thread. ⌘-click: Bun only (real-input step 5). Review: Bun covers a background environment's row (the panel by the thread's key, then the focus switch; the page's `environmentId`) | [link](https://raw.githubusercontent.com/ccheever/exact2/3bf40709025d06777f52d8dbf02a7c67f12472dc/legacy-sidebar-full/ls1-link.png), [beside](https://raw.githubusercontent.com/ccheever/exact2/164e2a9914885fbe0edb8d70c8ae33d82412d5ce/legacy-sidebar-full/ls1-beside.png), [page](https://raw.githubusercontent.com/ccheever/exact2/bbdcc3c93fe9b414f42dc10d9bff2b2d29dd44b1/legacy-sidebar-full/ls1-page.png) |
| LS-2 | pass (agent drive, Bun): a server on 16565 in Dev server's terminal gives the row the globe ("Open localhost:16565"); its press opens the thread with a Browser tab on `localhost:16565`; a background environment's Dev server row shows its own globe from that environment's stream | [port](https://raw.githubusercontent.com/ccheever/exact2/316aa3d91a31edf8d7593b5f3cde92fbbf286d58/legacy-sidebar-full/ls2-port.png), [rows](https://raw.githubusercontent.com/ccheever/exact2/c019a38a9db68cd97159f70c372bf4fd6604858b/legacy-sidebar-full/ls2-row.png) |
| LS-3 | in-app path pass (AppKit test, Bun): before, no drag destination over a row; after, the row's target takes file drags (clicks still reach the row), rings, reports the row's key and files, and the open thread's composer takes them. Review: a drop on the open thread's row only queues; a drop whose thread did not open goes (AppKit 5 run, Bun). A real Finder drag is real-input step 1 | [ls3-record.txt](https://raw.githubusercontent.com/ccheever/exact2/55cb0b24a59d392720c2a558c9f56cd6952fd061/legacy-sidebar-full/ls3-record.txt), [ls3-review-record.txt](https://raw.githubusercontent.com/ccheever/exact2/6f9b851a673eeeedbf986b54dc3788aec8b2743b/legacy-sidebar-full/ls3-review-record.txt) |
| LS-4 | pass (agent drive, Bun). Review 2: the double-click race on the pull request or port leaves the row unfocused (Bun; round 1's contract fails it, [record](https://raw.githubusercontent.com/ccheever/exact2/745aaae6116bce943d3acb7eb58d033d5064f0ca/legacy-sidebar-full/r2-review-record.txt)). Tab from the Archive me row, with no pointer on it, shows its Archive with the focus, as the reference; before, Tab skipped it. Review (after-review drive on `05b9c5547`'s app code): a press that opens Dev server leaves the focus in the composer (`tree`: `composer` focused) and, the pointer on Branch work, the open row shows "just now", as the reference (before the fix it showed Archive); Archive confirmation on: Confirm shows and has the focus, stays with the pointer on the row's title, and goes with the pointer on another row, which leaves the row unhovered with its time; Tab still shows Archive. Real keys: step 2 | [tab](https://raw.githubusercontent.com/ccheever/exact2/078995f3735ee7fc99dab7603d2ca774345c4033/legacy-sidebar-full/ls4-tab.png), [open row](https://raw.githubusercontent.com/ccheever/exact2/9e985d11247c6b77ecf2669b0d562cd055701d04/legacy-sidebar-full/review-open-row.png), [confirm](https://raw.githubusercontent.com/ccheever/exact2/507f5f4628ccb46417d783fb1f3cb4353088b9f2/legacy-sidebar-full/review-confirm.png), [hover-focus record](https://raw.githubusercontent.com/ccheever/exact2/b89c291949f59e42d4e33a5e697e28b9cfa27f02/legacy-sidebar-full/review-hover-focus-record.txt) |
| LS-5 | in-app path pass (text before/after of the menu's command, Bun): "New thread on feature/audit" on a worktree thread opens the draft with `{envMode: "worktree", branch: "feature/audit", worktreePath: …}`; before, a plain draft. The native menu pick is real-input step 3 | [ls56-menu-record.txt](https://raw.githubusercontent.com/ccheever/exact2/0fec515747a8be59585538c1543541a3815b7f7f/legacy-sidebar-full/ls56-menu-record.txt) |
| LS-6 | pass (agent drive, Bun, text record): Archive on a background environment's row archives it there with the undo notice; New thread on a background project opens its draft on that environment, as the reference (before: "Open a thread on that environment first."); the background project's menu has Rename, Group into... and Remove enabled and acting there, a background thread's menu shows and acts there (text record). Native menu picks: step 4 | [archive](https://raw.githubusercontent.com/ccheever/exact2/fb1ac9bd0f10fcc5ce38f2519cc9af15f032d084/legacy-sidebar-full/ls6-archive.png), [new thread](https://raw.githubusercontent.com/ccheever/exact2/467d6e450790475dbdfa16b83b2e7686bee15aed/legacy-sidebar-full/ls6-new-thread.png), [ls56-menu-record.txt](https://raw.githubusercontent.com/ccheever/exact2/0fec515747a8be59585538c1543541a3815b7f7f/legacy-sidebar-full/ls56-menu-record.txt) |
| LS-7 | pass (agent drive, Bun). Review 2: the first row's title, pull request and Archive tooltips draw whole above it, over the project header, as the reference's ([title](https://raw.githubusercontent.com/ccheever/exact2/460b25b52e55111e3eaa2d747ffdb94e5ea41ac1/legacy-sidebar-full/r2-first-row-title-tip.png), [pull request, Archive](https://raw.githubusercontent.com/ccheever/exact2/57c927b56cc69fe58ad8678bdbc38bc17e4e6600/legacy-sidebar-full/r2-first-row-pr-archive-tips.png)); the pointer moving to another row closes the tip and lets the row go; Confirm still ends as the pointer leaves ([pointer](https://raw.githubusercontent.com/ccheever/exact2/61493782ba129310621c10f9b4248874d04c2ab1/legacy-sidebar-full/r2-pointer-and-confirm.png)); a trigger that goes as its row re-sorts lets the row or header go (Bun, [record](https://raw.githubusercontent.com/ccheever/exact2/745aaae6116bce943d3acb7eb58d033d5064f0ca/legacy-sidebar-full/r2-review-record.txt)). Round 1: the title's tooltip on hover (the background Dev server row); the worktree and status tooltips, Bun. The first after drive found the title's trigger took the row's hover (below); fixed, and the second drive's Archive press is the proof. Review: a trigger that goes under the pointer no longer leaves its row or header hovered (Bun, the pre-fix contract fails it) | [title](https://raw.githubusercontent.com/ccheever/exact2/c9d6cd5b761e6a168b09c063196fc76dd2801d6e/legacy-sidebar-full/ls7-title.png), [hover-focus record](https://raw.githubusercontent.com/ccheever/exact2/b89c291949f59e42d4e33a5e697e28b9cfa27f02/legacy-sidebar-full/review-hover-focus-record.txt) |
| LS-8 | in-app path pass (Bun: motion attributes, the arrival flag by tick, none under reduced motion, the destructive "Delete anyway" in bg-destructive `light-dark(#fb2c36, #fb414a)`; text before/after: the toast's tone and the motion attributes, the toast itself needs the native menu); the motion under a real archive and the red Delete anyway are real-input steps 7 and 4 | [ls8-record.txt](https://raw.githubusercontent.com/ccheever/exact2/8423a36833ae49ce2573a5f6fb298a0c62771d95/legacy-sidebar-full/ls8-record.txt) |

Review round 2 drive (agent mode, 1280×840, fresh storages): before on the evidence worktree (`6db5cadea`), after on
this branch's bundle of `8949d0dba` (`r2-before/`, `r2-after/` t0-t6), the reference by CDP (`r2-ref-*.png`); `tree`
after each tip shows the layer's `legacy-tip-title-lsf-pr-bubble` and `legacy-tip-archive-lsf-pr-bubble`, none once the
pointer moved, and `thread-archive-confirm-lsf-pr` until the pointer left. The right-click close (`openMenu`) came after
the drive: Bun and the contract build only.

Drives (agent mode, 1280×840, `screenshot … window`): before on the evidence worktree (`6db5cadea`), after on this
branch's bundle (`aef1a2d16`'s app code), the reference by CDP on its lane; the GitHub lane's primary server (16564) paired
as a second environment through Settings › Connections. The first after drive (`fe17e8b12`) found the title tooltip's
hover handler kept the row unhovered when the pointer entered on the title (the host hovers only the nearest node with a
`hover`): the agent's Archive press at Archive's place opened the thread instead. Fixed in `aef1a2d16` (every tooltip
trigger in a row tells the row, `LegacyRowTip`, `LegacyTitleTip`'s `relay`), with a Bun wiring test; the second drive
(the brief's one retry) took every after image.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout); Settings ›
General › Legacy features › Sidebar (legacy) on. A project with a git checkout and a few threads (one on a branch, one
in a worktree), the GitHub lane's server paired as a second environment (Settings › Connections › Add environment,
`bun lane.mjs pair primary`), with a thread linked to a pull request. Never send a message.

1. **LS-3, a file.** From Finder drag a text file onto a legacy thread row that is not open: while over the row it shows
   a thin blue ring; drop: that thread opens and its composer shows the file's chip. Drag a PNG onto the open thread's
   row: it joins the composer's image shelf. Drag a folder alone onto a row: no ring stays, nothing opens. Drag a file
   onto a background environment's row: that environment becomes the focus, the thread opens with the chip. Drag a file
   onto the open thread's row: the chip joins its composer and the right panel stays as it was (no second press).
2. **LS-4, keys.** Click a thread row, press Tab: the row's Archive button shows with the focus ring (no pointer over the
   row); Enter archives it (Archive confirmation off). Turn Archive confirmation on (Settings › General): Tab to Archive,
   Enter: Confirm shows and has the focus; Enter archives. Again, then Tab away: the Confirm goes. Tab from a project row
   reaches its New thread button, shown.
   Review: click another thread's row, then move the pointer off the sidebar: the composer has the caret and the opened
   row shows its time (no Archive). With Archive confirmation on, click a row's Archive: Confirm shows; move the pointer
   left along the row over its title: Confirm stays; move it to another row: Confirm goes and the row shows its time,
   unhovered. Confirm focused, ⇧Tab: Confirm stays; Tab out of the row: it goes. Collapse a project with a running or
   unread thread, click its status dot (it expands), move away: the header loses its hover and New thread hides.
   Review 2: rest the pointer on the first row's title, then its pull request glyph, then Archive: each tooltip shows
   whole above the row (over the project header). Right-click the title while its tooltip shows: the tooltip goes as the
   menu opens. Click a pull request glyph of a thread that is not open, then at once click it again (a double-click),
   move the pointer off the sidebar: that row shows its time, not Archive.
3. **LS-5, menu.** Right-click the branch thread › "New thread on <branch>": the draft's branch chip shows that branch
   (Current checkout); on the worktree thread the draft is in that worktree.
4. **LS-6, menus.** Right-click the background project: Rename, Group into..., Remove are enabled. Rename › the dialog
   says "Environment: <label>" › Save: the title changes. Remove on it with threads: "Project is not empty" with a red
   Delete anyway › the removal confirm › Confirm: it goes. Right-click a background thread: the legacy menu shows; Mark
   unread, Copy Path, Rename thread and Delete act on that environment. Project settings on a background-only project:
   that environment becomes the focus and Settings › Projects shows the project (a declared difference: the reference
   keeps the focus).
5. **LS-1, pointer.** With a thread open, click a pull request glyph of another thread: that thread opens with the pull
   request tab in the right panel; ⌘-click opens the system browser; from the Pull Requests page the click selects it there.
6. **LS-2, pointer.** Run a dev server in a thread's terminal (⌘J, `python3 -m http.server <lane port>`): its row shows the
   globe; hover: "Open localhost:<port>"; click: the thread opens with a Browser tab on that port.
7. **LS-8.** Archive a row: it fades and the rows below slide up (reduced motion is checked by the agent's `prefer`,
   never System Settings).

## Checks

Review round 2, on `35b170c8a` (the final app code; the commit after it changes only this record), after merging
`origin/feat(example)/t3-code` (`ca4f28e35`, no newer commit at the checks):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4759 pass, 1 skip, 0 fail (319 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,427 lines; 250 diagnostics, all warnings, the same set as the merged base's (none new in `legacy-sidebar.contract`) |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed (its first run, on `8949d0dba`, exited 101: "app sources changed during capture", my `openMenu` edit landing mid-bake; rerun clean) |
| AppKit `macos/tests/row-drop` | not rerun | no Swift changed in round 2 (round 1: 5 run, 0 failed) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap (on the record's commit) |
| `cargo build --all-targets --keep-going` | 0 | on `8949d0dba`; `35b170c8a` changes only the clone's contract and tests, which no default member builds |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (on `8949d0dba`, as above) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | on `8949d0dba` |
| `cargo fmt --all -- --check` | 0 | on `8949d0dba` |
| `bun scripts/boot.mjs` | 0 | on `8949d0dba` |

Review round 1, all on `731407007` (the final app code; the commit after it changes only this record), after merging
`origin/feat(example)/t3-code` (`8fa9446cc`, no newer commit at the checks):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4749 pass, 1 skip, 0 fail (318 files); the first run found `no-active-thread.test.ts` (a missing thread answered `focus:composer`), fixed in `731407007` |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,427 lines; 29,227,366 bytes; no `#dc2626` warning left |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed |
| AppKit `macos/tests/row-drop` (README recipe) | 0 | 5 run, 0 failed (`ls3-review-record.txt`); no Swift changed after it |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

Live drive (one, agent mode, 1280×840, the bundle of `05b9c5547`'s app code; `731407007` adds the missing-thread
guard and the terminal glyph's relay, both under Bun): `after-review/` r0-r6 on the lane, `tree` showing `composer`
focused after the row press and Confirm gone after the pointer left.

Round 0 (on `aef1a2d16`, `fe0e39c91` merged): all the same checks exit 0 but `cargo test` (101: #440's
`apps/color-gallery/app.contract:269` on the feature branch then, since fixed there).

## Not done / not verified

- Real-input steps 1-7: ran in `realinput-1011c` ("Real-input results" below).
- A legacy tooltip whose row goes with no press of its own (another device archives it, a key shortcut) while it
  shows, the pointer then resting on no legacy row, header or trigger (the empty space under the list): it stays until
  the pointer reaches one or the surface changes. A component has no unmount event in Contract to close it, and the
  tooltip lives in the window's hover layer (the reference's portal closes with its unmounted Tooltip root). Rare;
  Archive's press and the right-click menus, the paths a person takes, close it first.
- The B2 case itself (a thread starting to run) live: it needs a provider turn, which the lane never sends; Bun runs it.
- Project settings on a project only a background environment has: the clone makes that environment the focus first,
  then opens Settings › Projects on it; the reference opens Settings › Projects scoped to the logical project without
  changing the focus. The clone's Settings › Projects page reads and writes the focused environment only: a clone scope
  boundary, not an Exact limit, so it is declared in `EXACT2-GAPS.md` ("Legacy sidebar: Project settings on a background
  environment's project") and proposed to the coordinator as its own task, `settings-projects-fleet` (the page over the
  fleet's project groups, each write through its environment's transport, as `settings-scope-sources.ts` does), with a
  STATUS.md "Known differences" row until it lands (the coordinator's files).

## Seen while building, not a finding row

- The reference's tooltips also open on keyboard focus (Base UI); the clone's `Tip` opens on hover only, app-wide.
- Round 2: the legacy rows' tooltips are now the hover layer's `HoverText` (the look the layer gives every portaled
  TooltipPopup, ThreadAutomationsPanel's included) instead of `TipCard`: both port ui/tooltip.tsx's popup, `HoverText`
  at 16 px leading, `TipCard` at leading-snug. Base UI's closeOnClick on the pull request, port and title presses is not
  modelled (the tip stays while the pointer rests, as the clone's `Tip` does app-wide); Archive and the right-click do it.
- Fixed in review round 1: the terminal glyph's tooltip (`TerminalProcessIndicator`, shared with the default sidebar)
  did not relay its hover to the row, so a pointer entering a row on that 12 px glyph left Archive hidden, and crossing
  it ended a confirmation; the legacy row now wraps the glyph (`TerminalGlyph`, split out of the indicator) in its own
  `LegacyRowTip`.

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, LS-3 (file drops) | LSF-3 (B12) | Partial: drops 1, 2, 3 and 5 pass; drop 4 fails: a file dropped on a background environment's thread row shows the ring and does nothing (reference: that environment's thread opens with the chip) | [LSF-3-drops.txt](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/LSF-3-drops.txt) |
| 2, LS-4 (keys, reviews) | LSF-4b (B12) | Partial: steps 1, 2, 3, 6, 7 and 8 pass; the inline archive Confirm never takes the focus, so ⇧Tab and Tab cannot reach it (reference: Confirm focused); step 4 not reachable; step 5's dot observable in neither app | [LSF-4b-focus.txt](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/LSF-4b-focus.txt) |
| 2, LS-4 (Tab to Archive) | LS-11, LS-5 (B5) | Not reachable: Tab reaches no sidebar control with the Mac's Keyboard navigation off (the clone's Tab stops only at text fields), where the reference tabs to every control, Archive included; LS-5's pointer path passes | [LS-11-focus-clone.txt](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/LS-11-focus-clone.txt) |
| 3, LS-5 (New thread on a branch) | LS-7 (B5), LSF-5w (B12) | Pass (LS-7's Mark unread not verified: no completed run in the fixture) | [LSF-5w-draft-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/LSF-5w-draft-pair.png) |
| 4, LS-6 (background menus) | LSF-6 (B12) | Fail (crash): steps 1–2 pass; the clone crashed on the thread Delete confirm's mouse-up (`NodeView.releaseHold`, the releaseHold crash, which draft main PR #519 files as `issues/closed/20261013-macos-a-click-after-a-node.md` and fixes, not merged; the delete was never sent); steps 4–5 not run | [LSF-6-crash-summary.txt](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/LSF-6-crash-summary.txt) |
| 4, LS-6 (local menus) | LS-6 (B5) | Fail: Project settings opens nothing (reference: Settings › Projects); the remove confirms do not hold Tab; no "Environment:" line; "Group into..." against "Group into…" | [LS-6-project-settings-pair.png](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/LS-6-project-settings-pair.png) |
| 5, LS-1 | LSF-1 (B12) | Pass; recorded: the clone keeps the Pull Requests list's query when a glyph selects a pull request | [LSF-1-beside-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/LSF-1-beside-pair.png) |
| 6, LS-2 | LSF-2 (B12) | Pass; finding: a legacy row click on the Pull Requests page does not open the thread (clone; the reference not tried) | [LSF-2-browser-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/LSF-2-browser-pair.png) |
| 7, LS-8 (archive motion) | LS-5 (B5), LS-12 | Pass: an archive fades and the rows below slide up in about 185–190 ms (reference about 170–185 ms), filmed by real capture; LS-12's agent film is blank (not filmable) | [LS-5-archive-confirm-film-pair.png](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/LS-5-archive-confirm-film-pair.png) |

Not passed: LSF-3's drop 4, LSF-4b, LSF-6 (the crash) and LS-6. The other g3 rows over the legacy sidebar on the same bundle: LS-1 (prep), LS-2, LS-9 and LS-10 pass; LS-3 passes in part; LS-4 (a real double-click's rename field without the focus) and LS-8 (⌘-click, ⌘-click, ⇧-click selects 2, reference 3) fail. All are in `STATUS.md` "Known differences".

## Delivery

Merged on 2026-10-12 as `5863748f1` (#469, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`; LSF-3's drop 4, LSF-4b, LS-6 and the releaseHold crash (LSF-6; a framework bug, filed and fixed by draft main PR [#519](https://github.com/ccheever/exact2/pull/519), `issues/closed/20261013-macos-a-click-after-a-node.md`, not merged) are in `STATUS.md` "Known differences", with the Project settings row (the user's decision of 2026-10-11: no new tasks from real-input failures or build observations, so the proposed `settings-projects-fleet` task was not created).
