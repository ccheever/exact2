---
name: 20261011-legacy-sidebar-full
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-legacy-sidebar-full
pr_url: null
verified_commit: null
---

# The "Sidebar (legacy)" in full: PR links, ports, file drops, keyboard Archive, branch threads, background environments

## Outcome

Decision U22 (the user, 2026-10-11): the legacy sidebar is built in full, as the reference. The first build
([legacy-sidebar](closed/20261005-legacy-sidebar.md), PR #143) left six differences in its "Remaining differences"
list. This task builds each of them so the legacy sidebar does what `LegacySidebar.tsx` does.

| Id | Clone (feature tip `6db5cadea`) | Reference (check it first) |
| --- | --- | --- |
| LS-1 | A thread row's pull request glyph is an image: a press on it opens the thread (the row's press) | The glyph is a link (`handlePrClick`, `useOpenPrLink`): with a thread open it opens the pull request beside the clicked thread in the right panel and goes to that thread; with no thread open, on the Pull Requests page; a link no project reads follows "Open links in"; ⌘ or Ctrl opens the system browser |
| LS-2 | No discovered-port button (the terminal-running glyph was built later and matches `terminalStatusFromRunningIds`) | A Globe2 button "Open localhost:<port>" (tooltip "Open localhost:<port>" plus " (+N)") for a thread whose terminal serves a discovered local server: it goes to the thread and opens the port in a Browser tab beside it; a failure toasts "Unable to open preview" |
| LS-3 | Files dropped on a row do nothing | `makeWorkspaceFileDropHandlers` on the row: a file drag rings the row (`ring-1 ring-inset ring-primary/70`); the drop queues the files for that thread (`sidebarPendingFileDropStore`), goes to the thread, and its composer adds them as dropped files (images to the shelf, other files as chips) |
| LS-4 | Archive shows on hover only, so the keyboard never reaches it; the project row's New thread likewise | Archive and New thread stay in the tab order (opacity 0) and show on `group-focus-within` as on hover; the meta (time, jump label) and the remote badge hide the same way; with Archive confirmation on, the Confirm button takes the focus; Archive's "Archive" tooltip only without confirmation |
| LS-5 | "New thread on <branch>" opens a plain draft in the project | The draft carries the branch: `handleNewThread(projectRef, { branch, worktreePath, envMode: worktreePath ? "worktree" : "local", startFromOrigin: false })` |
| LS-6 | On a background environment's project, Rename, Group into..., Remove and New thread are disabled (New thread toasts "Open a thread on that environment first."), Project settings does nothing on a remote-only project; a background thread's menu, Archive and inline rename do nothing; stale duplicate project rows of one checkout are not merged (`environmentGrouping.test.ts`) | Every project and thread action targets the member's or thread's own environment (`projectEnvironment.update/delete`, `threadEnvironment.*`, `handleNewThread(scopeProjectRef(member.environmentId, …))`); Project settings opens Settings › Projects scoped to the logical project; `buildProjectGroups` keeps the freshest row of a physical checkout and its identity |

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
