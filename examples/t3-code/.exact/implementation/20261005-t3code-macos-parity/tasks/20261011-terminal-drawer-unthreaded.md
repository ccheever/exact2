---
name: 20261011-terminal-drawer-unthreaded
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-drawer-unthreaded
pr_url: null
verified_commit: null
---

# The terminal drawer for a draft and for a background environment's thread

## Outcome

The user decided on 2026-10-11 that the two terminal drawer items left in the "Not done / limits" list of
[terminal-drawer](closed/20261005-terminal-drawer.md) are built now, matching the reference T3 Code.

| Id | Clone (as recorded) | Reference (check it first) |
| --- | --- | --- |
| TD-1 | "drafts without a server thread have no drawer (the clone's drafts carry no thread id)" | A draft carries its thread id from the start (`draftThread.threadId`); ⌘J and the layout button open the drawer in the draft's project root (or its worktree), and the terminal is still that thread's after the first send |
| TD-2 | "a background (fleet) environment's thread has no drawer (sessions use the focused transport)" | Every environment is connected at once; a thread's terminal RPCs (`open`, `attach`, `write`, `resize`, `close`) go to the thread's own environment (`threadRef.environmentId`), and the drawers of up to ten hidden threads of any environment stay mounted with their sessions attached |

## Steps

1. Reference first: the drawer for a draft and for a thread of a paired (non-primary) environment, in the reference
   source and in the reference app on this lane.
2. Read what the clone does today on the base (`6db5cadea`): source, Bun tests, one before drive with a second
   environment paired.
3. Build what differs: terminal sessions follow their thread's environment (the focused transport or that
   environment's background transport), and the drawer's mounted threads cover every environment and draft, as the
   reference's `existingOpenTerminalThreadKeys`.
4. Tests (Bun, AppKit transport); one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| TD-1 | reference comparison; Bun test; agent drive: a draft of a project, ⌘J, a command in the drawer | before / after / reference images |
| TD-2 | reference comparison; Bun and AppKit tests; agent drive: a second environment paired, its thread's drawer opened, a command, a switch to a thread of This machine and back | before / after / reference images, and text (the sessions' state, the servers' terminal logs) |

## Reference

- `apps/web/src/components/ChatView.tsx` (T3 Code `1e2ecbd975`): `activeThread` is the server thread or the local
  draft (`buildLocalDraftThread(threadId, draftThread, …)`); `toggleTerminalVisibility` opens `term-1` for the active
  thread (draft or server) with `cwd: gitCwd ?? activeProject.workspaceRoot`, `worktreePath` and
  `projectScriptRuntimeEnv`, through `openTerminal({ environmentId, input })`; `existingOpenTerminalThreadKeys` is the open
  drawers' keys among `serverThreadKeys` (every environment's threads) and `draftThreadKeys`; the mounted
  `PersistentThreadTerminalDrawer`s (`reconcileMountedTerminalThreadIds`, `MAX_HIDDEN_MOUNTED_TERMINAL_THREADS`).
- `apps/web/src/components/ThreadTerminalDrawer.tsx`: `PersistentThreadTerminalDrawer` reads the draft
  (`useComposerDraftStore.getDraftThreadByRef`) for its project and worktree; `TerminalViewport` attaches with
  `useAttachedTerminalSession({ environmentId: threadRef.environmentId, … })` and writes / resizes through
  `terminalEnvironment.write` / `.resize` with that `environmentId`.
- `packages/client-runtime/src/state/terminal.ts`, `terminalSession.ts`: each command and the attach stream are
  addressed to an environment's own connection.
