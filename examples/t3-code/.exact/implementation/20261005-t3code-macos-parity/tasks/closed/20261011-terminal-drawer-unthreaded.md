---
name: 20261011-terminal-drawer-unthreaded
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-drawer-unthreaded
pr_url: https://github.com/ccheever/exact2/pull/460
verified_commit: 1f813789b
---

# The terminal drawer for a draft and for a background environment's thread

## Outcome

The user decided on 2026-10-11 that the two terminal drawer items left in the "Not done / limits" list of
[terminal-drawer](20261005-terminal-drawer.md) are built now, matching the reference T3 Code.

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

## Review fixes (2026-10-12)

An independent review of PR #460 found one should-fix problem and no blocking one:

| Finding | Fix | Proof |
| --- | --- | --- |
| `T3TerminalSessions.swift`: `streamTransport` was set before the attach opened and never cleared when the stream ended (lost or failed) or `opened` returned "Disconnected", so `state.presentation.terminalSessions[].via` could say "focused" or "background" for a session that was not attached (diagnostics only; `attached` was right) | Cleared in the `ended` path, the failed `opened` path and `drop()`; `via` now names only the transport a stream is opening or open on | [via-review.txt](https://raw.githubusercontent.com/ccheever/exact2/45acb87176a74f7cec5ad764f9da087c5f7c57f3/terminal-drawer-unthreaded/via-review.txt): `TerminalStreamTests.testSessionsGoToTheirOwnEnvironmentsServerAcrossAFocusChange` asserts that after the focus moves to B, A's ended stream reports `via` "" and B's "focused". The pre-fix file fails it (`("Optional("focused")") is not equal to ("Optional("")")`), the fix passes it |

No UI changed (the field is agent-visible state only), so the images stand; the text before/after is the assertion above.

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
  on B and attaches on A's background transport, and A's ended stream reports no `via` (review fix). A mutation that
  puts every session on the focused transport (the base's rule) fails it on 12 assertions; the file before the review
  fix fails the `via` assertion. The transport tests that used `env-a` on the `fixture` loopback server now use
  `fixture`, and wait for the route before a resize.

## Not done / not verified

None.

## Checks

On the review-fix head (`b0a723335` merged with `origin/feat(example)/t3-code` at `fe0e39c91` in `f8b75bc3e`, plus this
record), 2026-10-12:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4664 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,398 lines (unchanged) |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed |
| AppKit `macos/tests/transport`, `terminal`, `fleet` (README recipe) | 0 | 63 (2 opt-in live skips) / 38 (1 opt-in skip) / 9 tests, 0 failures; run on `b0a723335` before the merge, whose inputs (`modules/apple`, `macos/tests`, `ExactNativeModule.swift`, `data-keys.mjs`, `app.json`) the merge did not change |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 101 | 3,744 passed, 1 failed: `contract` `button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` on `apps/color-gallery/app.contract:269` ("styled default button needs appearance=\"none\""). Not this task: the feature branch's tip has the same `apps/`, `contract/` and `.contract` files (this branch changes none), and main fixed it in `45940782c` ("a native button's accent-color asks for no migration"), which `feat(example)/t3-code` has not adopted yet |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once, before the live drive (`9321f6518`'s app code); not rebuilt for the review fix, which changes only the agent-visible `via` field that the AppKit test drives |

Earlier, on `db2f5c8bb` (first delivery): the same checks all exited 0 (Bun 4656 pass, cargo test 3,679 passed).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| The optional confirmation | TDU-1 (B11) | Pass: the second server's drawer comes back with only its own thread's log | [TDU-1-drawer-back.png](https://raw.githubusercontent.com/ccheever/exact2/dbf3b209457122ad13ae8928de9538c888000b5d/realinput-1011c/b11-studio/TDU-1-drawer-back.png) |

The session's rechecks over the terminal (G1-1 … G1-8, G2-05 step 5, G2-06's ⌘W half, DP-2, DP-3, DP-4, DP-8) ran on the same bundle: G1-6, G2-05, G2-06 and DP-8 pass; G1-1, G1-3, G1-4, G1-5, DP-2, DP-3 and DP-4 fail and G1-2, G1-7 and G1-8 pass in part, on Korean composition, the middle press, a second ⌘W, terminal links, the paste refusal's text, the Add to chat chip and the floating player's drag; none of them is this task's change (which environment a terminal session follows). They are in `STATUS.md` "Known differences".

## Delivery

Merged on 2026-10-12 as `8fa9446cc` (#460, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`; what stays open is listed in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
