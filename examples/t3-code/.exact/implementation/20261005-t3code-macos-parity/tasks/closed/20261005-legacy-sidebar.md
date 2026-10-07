---
name: 20261005-legacy-sidebar
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: feat(example)/t3-code-legacy-sidebar
pr_url: https://github.com/ccheever/exact2/pull/143
verified_commit: null
---

# The "Sidebar (legacy)" switch shows per-project thread trees

## Outcome

Turning on General › "Sidebar (legacy)" ("Restore per-project thread trees instead of the default
flat sidebar.") replaces the default sidebar with the reference's legacy sidebar: a list of
projects, each expandable into its own thread list with a preview count, "Show more" and
"Show less", sort options, drag-to-reorder projects, and the legacy context menus. Turning it off
returns to the default sidebar. The new-thread shortcut creates a thread at once in this mode.

## Scope and exclusions

Reference behavior (`LegacySidebar.tsx`, 3 825 lines):

1. **Layout.** Chrome header, then "Projects" content, then chrome footer. Each project is a row
   (click toggles expansion; a status indicator summarises its threads; a **New thread** button
   with the shortcut label, shown on hover as far as the source tells (check in the oracle); a drag
   handle in Manual order) followed by its thread list. A collapsed
   project still shows the active thread. Expansion and project order persist.
2. **Thread list.** Threads of a project, archived ones removed, sorted by the thread sort order.
   Shows the first N (preview count, 1–15, default 6). If more exist: **Show more** (with the compact
   status of the hidden threads) expands; **Show less** collapses. An expanded project with none
   shows "No threads yet".
3. **Thread row.** Status pill (resolved from run state and last visit), title (double-click renames
   inline; Enter commits, Escape cancels; failure "Failed to rename thread", empty "Thread title cannot
   be empty"), PR status icon with tooltip, terminal-running icon, remote-environment icon, relative
   time, ⌘-held jump label, Archive button (with the setting on: first press shows **Confirm**), click
   selects, ⌘/⇧ click builds a selection, files can be dropped onto a row.
4. **Sidebar options** (button `aria-label` "Sidebar options", tooltip right): groups "Sort projects"
   (Last user message / Created at / Manual), "Sort threads" (Last user message / Created at), and
   "Visible threads" with a number field (`aria-label` "Visible thread count", buttons "Decrease visible
   thread count" / "Increase visible thread count").
5. **Menus.** Project: Rename, Group into…, Copy Path, Project settings, Remove (destructive; a
   non-empty project asks "Project is not empty": "Delete all threads in this project before removing
   it." with **Delete anyway**; plain remove says "This removes only this project entry.").
   Thread: "New thread on <branch>" (when it has a branch), Rename thread, Mark unread, Copy Path, Copy
   Thread ID, Project settings, Delete (destructive). Several selected: Mark unread, Archive, Delete.
   There are no pin, settle or snooze actions in this sidebar (no such strings in the file).
6. **Keys.** `chat.new` (⌘N, ⇧⌘O) creates a thread in the current project at once, where the default
   sidebar opens "New thread in…" for more than one project (`routes/_chat.tsx:121-130`); thread
   previous/next/jump follow this sidebar's visible order.

Excluded: the desktop-update pill and ARM-build warning in the footer (T3 update feed is out of scope);
pin/settle/snooze and the Working shelf (default sidebar only); the default sidebar itself.

## Context and guidance

Parent specification: [spec](../../spec.md). Reference (T3 Code `1e2ecbd975`):
`apps/web/src/components/LegacySidebar.tsx:221-232,287-360,360-1008,1009-1158,1158-2634,2718-2840,2917-3138,3139-3825`,
`apps/web/src/components/AppSidebarLayout.tsx:331-336`, `apps/web/src/components/Sidebar.logic.ts`
(`sortProjectsForSidebar`, `orderItemsByPreferredIds`, `resolveThreadStatusPill`,
`resolveProjectStatusIndicator`, `buildMultiSelectThreadContextMenuItems`, `isTrailingDoubleClick`,
`shouldClearThreadSelectionOnMouseDown`), `apps/web/src/lib/threadSort.ts`,
`apps/web/src/sidebarProjectGrouping.ts`, `packages/contracts/src/settings.ts:52-78`
(sort orders; preview count 1–15, default 6).
Port with their names and tests whatever the default sidebar's port lacks (check first:
`sidebar-model.ts`, `sidebar-state.ts`, `r4-polish-palette-projects.ts` already hold the project sort and
grouping): `sortThreads` and its test, the `Sidebar.logic.test.ts` cases for the functions above, and the
`sidebarProjectGrouping` cases the clone has not ported.
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction (keyed virtualized
lists: one `each` with a keyed flow root; bounded scroll; offscreen row targeting; keyboard focus),
components (rows are keyed child components; state outside removed branches), state-and-data
(persist expansion and order, awaited), accessibility (row labels, keyboard), design (all states),
motion (list changes are animated in the reference; reduced motion), testing-and-debugging.
Clone evidence (mc-orch tree, 2026-10-05): the switch is stored and unused (`settings-core.ts:54,406`,
`settings-appearance-look.ts:23,58`); project order consumed by `sidebar-view.ts:79`; project
groups and rename/remove dialogs exist (`client.ts projectGroups`, `projects-view.ts`); thread rows
and menus are the default sidebar's (`sidebar-row.contract:56`, `sidebar-commands.ts`); the "New
thread in…" picker is `app.contract:1081-1093`. The thread sort order and preview count settings are
not read anywhere.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it
by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23);
"oracle" below is `target/t3-ui-parity/electron-oracle.mjs`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | 20261005-clone-on-exact2-main | pending | Merged | pending |
| merged task PR | 20261005-desktop-oracle-and-trace | pending | Merged | pending |
| scheduling preference | 20261005-main-fix-adoption | pending | Merged first (sidebar, popover, tooltip, pointer-events) | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| decision | U22: build the full "Sidebar (legacy)" (about 3,800 reference lines) or defer it | none | The user confirms building it in full (the spec says every reference feature) before `prepare`. | pending |

## Issue assessment at preparation

Checked sources and time: {{at prepare}}; draft records only.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X9](../../issues/20261005-x09-root-component-across-files.md) | `app.contract` cap; a second sidebar is large | 1,327 of 1,500 lines | nonblocking until the cap, then blocking | Put the whole legacy sidebar in new `.contract` files with child state; add one `when` in the root |
| [X13](../../issues/20261005-x13-hover-keys-during-pan.md) | Hover and keys during a pan (project drag) | `EXACT2-GAPS.md` X13 | nonblocking (workaround: the sweep code in `sidebar-drop.ts`; Escape cancel differs) | Declare |
| [X24](../../issues/closed/20261005-x24-still-pointer-rehover.md) | Hover under a still pointer after the list changes | X24 | nonblocking (workaround: `t3-rehover` hook) | none Update 2026-10-07 (adopt-main-fixes-shell): fixed on main #174; `t3-rehover` removed from the legacy project list. |
| [X17](../../issues/20261005-x17-popover-position-try.md) | Sidebar options menu flips | X17 | nonblocking (declared visible difference) | Declare |
| [X26](../../issues/20261005-x26-app-menu-control.md) | Native menus at a point | X26 | nonblocking (workaround: `T3ContextMenu.swift`) | none |

## Implementation notes

- New files with the `legacy-sidebar-` prefix: model (`legacy-sidebar-model.ts`: projects, thread
  lists, preview count, pinned collapsed thread, hidden status), view (`legacy-sidebar.contract`),
  commands (options menu, expansion, project and thread menus), tests. Reuse the default sidebar's row
  view for the thread row where the reference rows are the same; add only the legacy differences
  (archive confirm, no pin/settle/snooze, jump labels).
- `chat.new` branch: read the switch in `sidebarNewThread` (`app.contract:1081`); in legacy mode skip
  the picker.
- Persist expansion state and the two new client settings in the same store as the other client settings;
  read `sidebarThreadSortOrder` and `sidebarThreadPreviewCount` from the server-synced client settings as the
  reference does.
- Desktop-only footer items are not drawn.

## Acceptance and reproduction

Every row, attended or not, runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773; see
`20261005-embedded-server-runtime`).

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Switch | Lane backend with 3 projects (one grouped across two machines), 9 threads incl. a running thread, a PR-linked thread, a remote-machine thread | Turn the switch on and off | Tree appears and disappears; default sidebar state intact after returning | macOS 1280×840 and 840×620, light and dark | pixel pairs vs oracle with `legacySidebarEnabled`; `--json` tree |
| Thread lists | Same | Expand and collapse; Show more / Show less; set preview count to 1, 3, 15 | Counts, hidden-status dot, "No threads yet", active thread pinned in a collapsed project | macOS | screenshots; `layout`; ports of `sortThreads` and `Sidebar.logic.test.ts` cases |
| Sort and options | Same | Open Sidebar options; change each group; use the number field and its buttons | Order changes; value clamps to 1–15; labels exact | macOS | pixel pair; client settings in the server config |
| Rows | Same | Double-click rename (Enter, Escape, empty, server refusal); Archive with the confirm setting on and off | States and error toasts as above | macOS | agent drive; trace of the rename and archive commands |
| Menus | Same | Right-click project, thread, multi-selection (attended session) | Items, order, destructive style; "Project is not empty" dialog and Delete anyway | macOS; real pointer | native-menu capture vs oracle `(attended session)`; trace of removals |
| Reorder | Manual order | Drag a project (attended session) | New order persists after relaunch | macOS; real pointer | session notes; `t3-code.json` |
| New thread key | Two projects | ⌘N in legacy mode and in default mode | Legacy: draft opens in the current project; default: "New thread in…" picker | macOS | `--json` drive; AppKit `r8-keys` |
| Jump keys | Same | ⌘1–9, ⇧⌘[ / ⇧⌘] | Follow the visible legacy order | macOS | screenshots of jump labels; oracle comparison |
| Options menu and dialog keyboard | Same; a non-empty project | Open Sidebar options with the keyboard, change a radio, press Escape; remove the project to open "Project is not empty"; Tab, Enter, Escape; set prefers-reduced-motion | Focus enters the menu or dialog and returns to its trigger; Escape closes; Enter on Delete anyway removes; menus and the dialog appear without movement under reduced motion | macOS | `tree --ax`; film (`over 300 every 30`) in both modes; `(attended session)` for real keys |
| Keyboard and focus | Same | Tab and arrow through rows; Enter; Menu key | Visible focus; every icon button has its `aria-label` | macOS | `tree --ax` |
| Clone checks | `git add -A` | Usual list, `bun scripts/caps.mjs`, five checks | Green; the default sidebar cells are unchanged; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs; matrix |

States covered: loading (before the shell arrives: no rows), empty (no projects: Add project; no threads),
error (rename, archive, delete, remove failures), disabled (Archive on a running thread), hover (New thread,
Archive), keyboard focus, drag, reduced motion (list expand/collapse and reorder without movement).
Task-owned source paths: new `legacy-sidebar-*.ts`, `legacy-sidebar*.contract` (+ tests), `sidebar-view.ts`
hunks, `app.contract` hunk, `settings-core.ts` (two settings), `modules/apple/T3Sidebar.swift` hunks if
drag needs them.
Required environment: Xcode 27.0, pinned Bun 1.4.2, oracle desktop build with the legacy switch on, lane
backends with the fixture above.

## Progress

Implemented on `feat(example)/t3-code-legacy-sidebar` (2026-10-06); verification: unverified (no independent
review; attended rows not run).

- `legacy-sidebar-model.ts`: ports of `sortThreads`/`getThreadSortTimestamp`/`getLatestThreadForProject`,
  `orderItemsByPreferredIds`, `sortProjectsForSidebar`, `resolveThreadStatusPill`, `resolveProjectStatusIndicator`,
  `buildMultiSelectThreadContextMenuItems`, `isTrailingDoubleClick`, `resolveAdjacentThreadId`, `reorderProjects`,
  `resolveProjectExpanded`, `formatRelativeTimeLabel`, and the legacy thread and project menus.
- `legacy-sidebar-view.ts`: groups across this environment and the fleet's background environments
  (`logicalKey`, `groupLabel`), Sort projects / Sort threads, preview count, Show more / Show less with the hidden
  status, the open thread under a collapsed project, "No threads yet", jump labels and the visible order that
  `thread.jump.N` and `thread.previous`/`next` follow (no wrap, as the reference). Snapshot field `legacy`.
- `legacy-sidebar-commands.ts`: expansion (persisted in `t3-code.json` `sidebar.projectExpanded`), Show more/less,
  Sidebar options (the two new client settings `sidebarThreadSortOrder`, `sidebarThreadPreviewCount` 1-15, in
  `settings-core.ts`), Manual reorder (persisted `sidebar.projectOrder`, also read by the palette's project order),
  the thread menu, the multi-selection menu (Mark unread / Archive behind "Archive N threads?" / Delete), the project
  menu (Rename, Group into..., Copy Path, Project settings, Remove; a group's members as submenus), "Project is not
  empty" with Delete anyway, the removal confirm, Rename project and Project grouping dialogs, the project New thread
  button (a member menu for a group), inline Archive with Confirm.
- `legacy-sidebar.contract` (+ `-shapes`): the chrome header and footer, Search trigger, Projects header with Sidebar
  options and Add project, the project rows (a virtualized list with `reorderdrop`; grips in Manual), thread rows.
- `chat.new` (⌘N, ⇧⌘O) creates in the current project at once while the switch is on (`sidebar-commands.ts`
  `new-thread-click`); ⇧-click ranges over the row's project list.

Remaining differences (not built or framework):
- Context menus, real pointer drag and hover reveal are agent-driven only; real input unverified (attended).
- No PR click-through to the right panel (the icon and its label show; a press opens the thread); no terminal-running
  icon, discovered-port button or file drop onto rows (terminal, preview and `file_handlers` are not in this clone).
- Thread and project tooltips are in-row `Tip`s, clipped by the list's scroll box; the options menu anchors below its
  trigger (X17); the Archive button shows on hover only (no focus-within reveal, so it is not keyboard reachable).
- List expand/collapse and reorder are not animated (the reference's auto-animate); nothing moves under reduced motion.
- "New thread on <branch>" opens a plain draft in the project (as the default sidebar's menu does), not one on the branch.
- Project actions on a background environment's member are disabled except Copy Path; the stale-row dedupe of
  `environmentGrouping.test.ts` is not ported.
- "Project is not empty"'s Delete anyway uses the default toast button tone, not the destructive one.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 | `2dac5bd6b` (3 commits on `d78ac86ff`) | `bun test examples/t3-code` 1241/0 (base 1200; +41 in `legacy-sidebar.test.ts`); strict tsc clean; `contract build` 2178 slots, 42 resources, 48380 nodes; `cargo test -p t3-code-macos --lib` 10/0; caps green; bundle `EXACT_APP_DIR=… host/apple/build.mjs t3-code-macos` exit 0 | Lane drive (macOS 1280×840, lane backend 127.0.0.1:16160, seeded 3 projects / 9 threads), below | Attended rows; second-environment fixture; relaunch persistence shown by `t3-code.json`, not a relaunch (agent sessions get fresh data dirs) |

Lane drive records (`target/lane/drive/record.txt`, trimmed; `<lane>` is the lane's project root):

```
[main] connected: legacy.enabled=false sidebar=true
[main] legacy-sidebar=true default sidebar=false            (switch on)
run 1: rows=beta-1,alpha-8,…,alpha-3  show-more: 1 empty: 1; after Show more: rows=9 show-less=true; after Show less: rows=7
[main] options open: true radios=legacy-sort-projects-{updated_at,created_at,manual},legacy-sort-threads-{updated_at,created_at}
[main] preview 6-3=3: rows=4 (1 beta + 3 alpha)
[main] sort threads created_at: rows=beta-1,alpha-8,alpha-7,alpha-1
[main] collapsed alpha with alpha-8 open: rows=beta-1,alpha-8
[main] dblclick beta-1: rename field=true; after Enter: thread-title-beta-1 text "Renamed by legacy sidebar", field gone
[main] hover alpha-1: archive button=true; archived alpha-1: still listed=false
[main] ⌘N legacy: paletteOpen=false palettePage="" threadId="" projectId="beta"
[main] manual grips: 3; beta collapsed: rows=alpha-8,alpha-7
[main] switch off: legacy-sidebar=false default sidebar=true
[main] ⌘N default: paletteOpen=true palettePage="new-thread-in"
[drag] before: alpha,beta,gamma grips=3
[drag] during: {"item":"<env>:<lane>/gamma","from":"legacy-projects","before":"<env>:<lane>/alpha","phase":"active"}
[drag] after: gamma,alpha,beta
t3-code.json after the main drive: clientSettings.legacySidebarEnabled true, sidebarProjectSortOrder manual,
sidebarThreadSortOrder created_at, sidebarThreadPreviewCount 3, sidebar.projectExpanded {…/beta: false, …/alpha: true}
```

Not run: context menus and native menu captures (real pointer), the oracle pixel pairs, dark mode and 840×620,
`tree --ax`, reduced-motion films, a running / PR-linked / remote-machine thread live (unit tests only).

## Next action

Review the PR; the attended rows (menus, real drag, hover) and the oracle comparison remain for `verify`.
Decision U22 (build in full) was taken as "build" by this wave's coordinator; confirm with the user.
