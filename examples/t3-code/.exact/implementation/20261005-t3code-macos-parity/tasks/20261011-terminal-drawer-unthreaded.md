---
name: 20261011-terminal-drawer-unthreaded
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
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

## What the base does (6db5cadea)

Read and driven before building (agent mode, 1280×840; This machine is the clone's embedded server on 16402, B a
second T3 server of the same release on 16403 with its own environment id, its own project `work-b` and one thread
"Lane B terminal", paired through Settings › Connections › Add environment; B's shell prints `LANE_ENV=B`):

- **TD-1 was already built.** `dc9de3504` (2026-10-07, after the terminal-drawer record's "Not done" was written) gave a
  draft its reserved thread id (`r7-handoff-thread.ts` `ensureDraftThreadId`) and opened the drawer under it: on the
  base, ⌘J in the "work / New thread" draft opens `term-1` in the project folder and the first send launches the thread
  under the same id (Bun test "draft terminal reserves its send identity…"). What still differed from the reference:
  the draft's open drawer was unmounted as soon as another thread was shown (`existing` held only the focused shell's
  server threads; the reference's `existingThreadKeys` holds `draftThreadKeys` too), so its session lost its stream.
- **TD-2.** The clone opens a background environment's thread by moving the focus there (`focusFleetThread`), so B's
  thread does get a drawer and its shell runs on B (`env-B`). But every terminal session rode the focused transport
  (`T3TerminalSessions.of(transport)`): when the focus moved, the sessions of the environment just left re-attached on
  the next socket, which belongs to the other server. `terminal.attach` starts a shell when none runs, so each focus
  change started a stray shell on the wrong machine: in the before drive, B's server got a shell for This machine's
  draft thread (in This machine's `work` folder) and This machine's server got one for B's "Lane B terminal" thread
  (B's `work-b` folder); B's metadata stream listed 2 terminal sessions where B has one. The drawers of the environment left behind were
  unmounted (`mounted` 0 on Timeline verification), where the reference keeps them mounted with their sessions.

## Cause and fix

- `T3TerminalSessions.swift`: a session carries its environment and goes to the transport that reaches it now: the
  focused transport while that environment is the focus, otherwise that environment's background transport
  (`T3Fleet`, which now hands each transport it makes to `T3TerminalSessions.fleetObserver`). Each observed transport
  reports the environment its socket reached (`terminalConnection(true, environmentId)`, `T3Transport.swift`); a session
  whose environment has no connected transport waits for one. Attach, write and resize all go through that route, so
  a session is never attached, written or resized on another environment's server (the reference's terminal commands
  take `threadRef.environmentId`). A write with no route writes the transport's own "The server is not connected."
  into the terminal, as a failed write does.
- `terminal-drawer-view.ts` `existingThreadKeys`: ChatView's `existingThreadKeys` — the focused environment's threads,
  every synchronized background environment's threads (`fleet`), and every draft's reserved id. An environment whose
  thread list is not known at the moment (the focus before its shell loads, a switched-on environment still
  connecting after the focus left it) keeps the drawers it has mounted, as the reference keeps a reconnecting
  environment's threads; an environment switched off or removed, or a thread its synchronized shell no longer lists,
  loses them. The mounted threads' sessions stay retained natively (`terminalRetain`), each on its own environment.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| TD-1 | pass (agent drive, reference): before and after, Toggle terminal drawer in the draft "work / New thread" opens `term-1` in `projects/work` under the draft's reserved id, as the reference; after, the draft's open drawer also stays mounted (its session kept) while another thread is shown, as the reference's `draftThreadKeys` (`mounted` 2 after, 0 before, on Timeline verification) | [td1-draft-drawer.png](https://raw.githubusercontent.com/ccheever/exact2/5fbdec3a15e0b646aaa65662b93567b46e9276b3/terminal-drawer-unthreaded/td1-draft-drawer.png), [td-record.txt](https://raw.githubusercontent.com/ccheever/exact2/625a3318622eb5252d69913fef2a149120d2fb41/terminal-drawer-unthreaded/td-record.txt) |
| TD-2 | pass (agent drive, reference, Bun, AppKit): B's thread opens its drawer and runs on B (`env-B`) before and after, as the reference; before, each focus change started a stray shell on the other server (B ran one for This machine's draft, This machine one for B's thread) and the drawers left behind were unmounted; after, each server runs only its own threads' shells (the reference's servers likewise), the drawers of both environments stay mounted, and B's metadata lists its one session | [td2-background-environment.png](https://raw.githubusercontent.com/ccheever/exact2/468b84208e661b108302142e03da5f02d067281c/terminal-drawer-unthreaded/td2-background-environment.png), [td2-back-to-b.png](https://raw.githubusercontent.com/ccheever/exact2/2a55370620c16af591f7ea1ee2874b8a661dd935/terminal-drawer-unthreaded/td2-back-to-b.png), [td-record.txt](https://raw.githubusercontent.com/ccheever/exact2/625a3318622eb5252d69913fef2a149120d2fb41/terminal-drawer-unthreaded/td-record.txt) |

Drives (agent mode, 1280×840, `target/t3-audit/clone-drive.sh`, base port 16400, a fresh `--storage` per run): before on
the evidence worktree at `6db5cadea`, after on this branch's bundle (`9321f6518`'s app code); B restarted with no terminal
history before each run. The reference is T3 Code (Alpha) `1e2ecbd975` (Electron, `ref-app.sh`, backend 16400, CDP 16401)
with B paired through its own Settings › Connections. Same steps on all three: Toggle terminal drawer in the draft and
`echo draft-terminal $PWD`; open "Lane B terminal", Toggle terminal drawer, `echo env-$LANE_ENV`; open Timeline
verification; open "Lane B terminal" again. The screens match before and after (the drawer was already reachable both
ways); the difference is on the servers and in what stays mounted, recorded in `td-record.txt`. The clone's terminal is
dark in a light window under the agent (the terminal-drawer record's spike note), the reference's light; not this task.

## Real-input batch steps

None: both rows were driven end to end in agent mode (the drawer toggle, typing into the terminal and thread switches
are agent operations here). The coordinator's session may confirm them with real input if it wants: pair a second
server, open its thread, ⌘J, type a command, open a thread of This machine and come back; the second server's
`userdata/logs/terminals` must hold only its own thread's log.

## Tests

- `terminal-drawer-unthreaded.test.ts` (3, Bun): TD-1, ⌘J in a draft opens `term-1` in the project root under the
  draft's id, and the draft's drawer stays mounted and retained behind a server thread; TD-2, a background environment's
  thread keeps its drawer mounted and its session retained across a focus change (its environment switched on but not
  connected yet, then synchronized, then the focus back), each session keyed by its own environment; and the drawer goes
  when its thread is gone from its environment's synchronized shell or the environment is switched off. The same file
  against the base's `terminal-drawer-view.ts`: 0 pass, 3 fail (the draft and A's drawer unmounted).
- `macos/tests/transport` `TerminalStreamTests.testSessionsGoToTheirOwnEnvironmentsServerAcrossAFocusChange` (AppKit,
  two loopback servers as environments A and B): B's session is not attached or written on A's server and waits; B's
  background transport attaches, writes and resizes it on B; after the focus moves to B, A's session is never attached
  on B and attaches on A's background transport. A mutation that puts every session on the focused transport (the
  base's rule) fails it on 8 assertions. The transport tests that used `env-a` on the `fixture` loopback server now use
  `fixture`, and wait for the route before a resize.

## Not done / not verified

None.
