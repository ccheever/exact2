---
name: 20261005-round12-wrapup
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: c1522fdac (untracked app tree in the mc-orch worktree; no PR)
branch: null
pr_url: null
verified_commit: null
---

# Round 12 finished on the old base

## Outcome

The newest clone tree (round 11 + the stopped round 12) passes every clone check on its own
base `c1522fdac`, and the round-12 items either work as the reference does or are recorded
as open with evidence. The tree is then ready for `20261005-clone-on-exact2-main` to import.

**Portability exception.** This tree exists only as untracked files in the mc-orch worktree
(`~/Documents/work/0.projects/exact2-worktrees/mc-orch-e88043b25805/examples/t3-code/`).
This ticket cannot run from a fresh clone. The user chose this order (decision 2026-10-05
#1: wrap up round 12 in mc-orch, then copy). Its delivery is the import commit of the next
ticket. Before any edit, archive the tree (for example
`tar -czf <scratch>/t3-code-r12-before.tgz` of the app directory) so that no work can be lost.

## Scope and exclusions

Included (IDs from [research](../../research.md) §Round 12):

| ID | Gap | Clone files | Reference |
| --- | --- | --- | --- |
| T1 | A no-project draft switches machine: Run-on lists each connected machine with a "No project" folder; picking one calls `projects.ensureScratch`; wait up to 10 s; Send shows "Preparing machine"; failure toast "Could not switch machine" + error. The last drive ended on the server's `ScratchFolderError`. | `r12-threads-scratch.ts`, `r4-git-env.ts:44-56`, `composer-controls-view.ts:114` | `apps/web/src/components/ChatView.tsx:2662-2730,4348-4420`; `useScratchProject.ts:14-22,65-78`; `packages/client-runtime/src/state/projectCommands.ts:56-62,124-150`; test `projectCommands.test.ts` |
| T3 | The open thread's device session floats on load as a mini player (not in sheet layouts ≤ 980 pt; hidden while the panel shows the same device). Drag/resize stay in `20261005-floating-device-player`. | `r12-threads-device.ts`, `r6-media-device.ts`, `r6-device.contract` | `ChatView.tsx:5285-5338`; `ChatView.logic.ts:99`; tests `previewMiniPlayerStore.test.ts`, `ChatView.logic.test.ts` |
| T4 | Right-panel tab strip at 840: overflowing tabs scroll; "Scroll tabs left/right"; active tab scrolls into view; persistence across relaunch. | `r12-threads-tabs.ts`, `r12-threads.contract`, `r4-surfaces.contract:93-94` | `RightPanelTabs.tsx:195,804-830,1015,1317-1346`; `RightPanelTabs.test.tsx` |
| T5 | A non-Git draft shows the strip with only a "Run on" chip (also when `hostsRestingComposerControls`). Written but never driven; no test. | `r12-threads-strip.ts`, `composer-controls-branch.ts:107`, `composer-controls.contract:726-728`, `composer-shapes.contract:89` | `BranchToolbar.logic.ts:67-79`; `BranchToolbar.tsx:603-612`; `ChatView.tsx:4185-4200`; `BranchToolbar.logic.test.ts:427-470` |
| F1 | Wheel scroll, thread switch, return: the position lands 64 pt off (2 of 4 real-wheel trials). The settle-tail fix is in `R9Input.swift` but its last edit is newer than its 13/0 test run. | `modules/apple/R9Input.swift`, `macos/tests/r9-input/main.swift:208-268` | `timelineScrollAnchoring.ts:164-175`; `ChatView.tsx:6552`; `timelineScrollAnchoring.test.tsx:274` |
| F2 | Files editor: a click below the last line, then typing, goes to the composer. Fix `focusFileEditor` is unbuilt. Confirm the reference caret position first. | `modules/apple/T3PanelsNative.swift`, `r4-surfaces-files.contract:206` | `components/files/FilePreviewPanel.tsx:16,599` (`@pierre/diffs/editor`) |
| F3 | First launch stores `onboardingCompletedAt` as 1970 (launch-relative `Date.now()` outside the agent). Store wall-clock ISO time. | `pages-welcome.ts:147,247`, `r8-pointer-clock.ts:8-12`, `sidebar-state.ts:102`, `app.contract:144`, reader `r9-connect-onboarding.ts` | `onboarding/firstRun.ts:13`; `FirstRunGate.tsx:157`; `contracts/settings.ts:403` |
| F4 | False "Some requests are slow" toast (shown twice after A off and B on). The reference times each request (15 s; 120 s for update methods) and clears on reply. | `shell-slow.ts`, `r3-protocol-reader.ts:47-69`, test `shell-r2.test.ts:33-84` | `rpc/requestLatencyState.ts:77-112`; `connection/platform.ts:644-657`; `SlowRpcRequestToastCoordinator.tsx:55` |
| F5 | `t3.server.origin` still names a removed backend. Remove must forget the pairing, credentials and cached threads as the reference does; the key must not outlive the environment. | `modules/apple/T3Transport.swift:101,116-119,137-152,219,829`; `T3Fleet.swift:59` | `ConnectionsSettings.tsx:2600-2640` |

Already done (no work, verify only): T2 "Worktree ready" card hides after the turn starts
(`r12-threads-worktree.ts`, `timeline-worktree.ts`).

Excluded: anything else from the upstream range (`20261005-upstream-timeline-and-markdown`,
`20261005-upstream-ui-sync`), the move and rebase.

## Context and guidance

Parent specification: [spec](../../spec.md). Paths above: clone paths are relative to the
mc-orch app directory; reference paths are relative to `apps/web/src/` unless they start
with `packages/` or `apps/`, at T3 Code `1e2ecbd975` (the fixture runtime is `f870c419fc`).
The round-12 task text and the real-input failure descriptions are copied into the next
section; nothing outside this plan directory and the mc-orch worktree is needed to read them.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (persistence must
be awaited on macOS; first-frame defaults are not proof of runtime storage), layout-and-
interaction (route boxes do not scroll; `navigationScroll`; offscreen row targeting),
testing-and-debugging (one session per observation; preserve failed attempts).
Lane tools: mc-orch `target/t3-ui-parity/` as documented in `AGENT-HANDOFF.md` "Lane tooling";
export `T3_RUNTIME=$PWD/target/t3-ref/runtime-f870c41` for every lane backend.

## Round-12 task text

Copied from the round-12 workflow that was stopped on 2026-10-05 at 11:48 KST (lanes
`r12-sidebar` and `r12-render` finished; `r12-threads` did not). IDs T1–T5 and F1–F5 are the
ones in the table above.

**Lane `r12-threads`, task as given:** "(1) No project drafts can switch machine (reference
c47f4263f9 and 845ddd9354 'Could not switch machine' toast) and 'not reached this device'
waits up to 10 s like the reference; (2) the live 'Worktree ready' card must not stay above a
retried thread; (3) a thread's open device session auto-shows as the floating player on load
(reference ChatView autoShowFloatingPreview); (4) relaunch persistence verified and fixed at
840 (sheet presentation) as well as 1280." Item 1 is T1; item 2 is T2 (done); item 3 is T3;
item 4 is T4. T5 (the non-Git strip) was added by the lane while it tested T1. The lane's
rules: verify by running (tests for logic, native PNG and tree evidence paired with reference
PNGs); at most 3 fix rounds per item, then record the item as open with a kind (in-app,
framework, physical-input, os-grant, live-credentials, fixture-cannot-produce).

**Real-input failures (the `realinput2` session, a real-keyboard-and-pointer pass on a lane
copy of the app), as handed to the fix lane `fixr`:**

1. Scroll restore is off by about 64 pt after a real wheel scroll, a thread switch and a return
   (2 of 4 trials). `R9Input.swift` `sample()` and `record()` kept only guarded samples during
   the wheel, and nothing recorded the final position when the thread was left. Wanted: record
   the last settled position on leave (thread owner change) and after wheel momentum ends,
   keep the guards against programmatic jumps, and add AppKit tests that simulate a wheel with
   a momentum tail and then an owner switch.
2. Files editor: a click in the empty area below the last line lands on the `file-lines` view,
   focus falls back to the composer, and typing goes into the draft. In the session's
   screenshot the composer held the typed text (Hangul after "draft two") and the open
   `README.md` showed none of it. Wanted: a press anywhere in the editor body focuses the editor
   text view with the caret at the end (the reference behavior; confirm it, see F2), with an
   AppKit or agent check.
3. `onboardingCompletedAt` was saved as `1970-01-01T00:01:37Z` on a real first launch:
   `Date.now()` is launch-relative in a real (non-agent) run. An earlier real-input session
   (round 8) had saved `1970-01-01T00:01:58.501Z`; a reader-side guard
   (`r9-connect-onboarding.ts`) hides old values but the writer still stores the wrong time.
   Wanted: find where wall time comes from (`r8-pointer-clock.ts` `epochNow()`,
   `sidebar-state.ts` `clock()`, `wallTime.epochAtZero` in `app.contract`) and make every
   persisted wall timestamp true epoch time in agent and normal runs, with a unit test for the
   normal-run clock shape.
4. After switching environment A off and B on, a false "Some requests are slow" toast
   ("1 request waiting longer than 15s") appeared twice. Requests to a switched-off
   environment must not count as slow (`shell-slow.ts`, the client call plumbing and
   `r3-protocol-reader.ts`, where an unanswered trace ends only after a status read).
5. Removing environment B left the persisted `t3.server.origin` pointing at B. On removal the
   saved origin must be reset or repointed as the reference does (the key lives in
   `T3Transport.swift`, not `T3Fleet.swift`).

**State when the round stopped:** `bun test` 1025 pass, 0 fail, strict `tsc` clean and
`contract build` OK (1865 slots) at about 11:47; the `r12-threads` lane build finished at
11:48:46. The AppKit `r9-input` binary reported 13 tests, 0 failures at about 11:45:32, and
`R9Input.swift` was saved at 11:46:20, after that run. `T3PanelsNative.swift` (`focusFileEditor`)
was saved at 11:48:53, after the last build, and was never built. The non-Git strip files were
saved at 11:46:52 and never driven. F3, F4 and F5 had no edit. The last T1 drive (11:43) ended
with the toast "Could not switch machine: Failed to create the folder for threads without a
project." (the server's `ScratchFolderError` on lane backend B).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | Decision 2026-10-05 #1 (wrap up in mc-orch first) | none | Recorded | `spec.md` |

## Issue assessment at preparation

Checked sources and time: 2026-10-05 23:25 KST. Plan `issues/` (X1–X45; no `issues/closed/`),
prior attempts (none) and reviews (none) for this ticket; GitHub `ccheever/exact2` PRs
searched for "t3-code" (none). No remote issue was searched: the plan's issues are local drafts
and none is published yet. This ticket runs on the old base `c1522fdac`, whose mc-orch tree
already carries the uncommitted framework edit `js/src/parking.rs` + `js/src/lib.rs`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X23](../../issues/20261005-x23-scroll-restore-offsets.md) | Scroll restore by key (F1 wheel, thread switch) | `EXACT2-GAPS.md` X23; clone `R9Input.swift` settle tail | nonblocking (workaround exists; the real-wheel check stays attended) | keep the workaround; record the attended result in X23 |
| [X13](../../issues/closed/20261005-x13-hover-keys-during-pan.md) | Hover and keys during a pan (sweep rows) | `EXACT2-GAPS.md` X13 | nonblocking (not in this ticket's scope) | none |
| [X14](../../issues/closed/20261005-x14-parked-native-reply.md) | Native replies after a let-go answer | mc-orch tree carries the framework edit at `c1522fdac` | nonblocking here (the old base already includes the edit); matters in `20261005-clone-on-exact2-main` | none |
| [X8](../../issues/closed/20261005-x08-agent-pointer-native-views.md) | Pointer input into native views (F2 Files editor click) | `EXACT2-GAPS.md` X8 | nonblocking (AppKit test + attended row) | none |

## Implementation notes

- F3, F4 and F5 are independent; do them first.
- T1 and T5 need a working second lane backend; first find why lane B's scratch folder
  creation failed (`ScratchFolderError`), since a fixture fault can hide a client fault.
- Port the reference tests named above where they are pure logic (`bun:test`, `now` argument
  instead of fake timers, original test names).

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| T1 switch machine | Lane backends A and B (isolated), a no-project draft | Run on ▸ machine B; send | Draft moves; Send shows "Preparing machine" then sends on B. Failure path (stop B): toast "Could not switch machine" + error | macOS 1280×840 | agent transcript, shots, oracle shot |
| T3/T4 | Device fixture; 840×620 window | Load a thread with a device session; open many right-panel tabs; relaunch | Player floats (not in sheet layout); strip scrolls, buttons work, active tab in view, tabs restored | macOS 840×620 and 1280×840 | shots beside oracle |
| T5 | Non-Git project draft | Open the draft | Strip with only "Run on" | macOS both sizes | shot pair; new test |
| F1 | Long thread A, thread B | Agent wheel on A, switch to B, back to A (real wheel in session S1) | Same row and offset as before leaving | macOS | `layout` before/after; AppKit r9-input green |
| F2 | Files editor with a short file | Click below the last line; type | Text goes into the editor at the reference caret position | macOS | AppKit test; real-input S1 |
| F3 | Fresh data folder, normal (non-agent) launch | Complete onboarding; read `t3-code.json` | Wall-clock ISO date | macOS | file excerpt |
| F4 | Backends A and B | A off, B on; normal requests | No slow toast unless a request exceeds 15 s; toast clears on reply | macOS | transcript; ported latency test |
| F5 | Backends A and B paired | Remove B; relaunch | `status` origin is not B; no B credentials or cached threads | macOS | `state`, Keychain item absent (`security find-generic-password` by service/account only) |
| Whole tree | — | `bun test examples/t3-code`; strict `tsc`; `contract build`; `cargo test -p t3-code-macos --lib`; all AppKit binaries; `lane-build.mjs` | All green; counts recorded | macOS | logs |

Task-owned source paths: the mc-orch app tree (`examples/t3-code/**`).
Required environment: mc-orch worktree, pinned Bun and Hermes, lane backends; no T3 Code
(Nightly) running during drives.

## Progress

Prepared 2026-10-05 23:25 KST (details below). Implemented 2026-10-06 by four parallel lanes in
the mc-orch worktree plus coordinator hunks; **ready for `verify`**.

Decisions applied during implementation (user, 2026-10-05/06): the exact2 repo itself is the
framework reference (U1); real-input rows run through `orca computer` (U14); no pixel-perfect
loops — behavior first, one oracle comparison per surface.

### Changes (paths relative to `examples/t3-code/`, mc-orch tree)
| Item | Result | Files |
| --- | --- | --- |
| F3 onboarding / persisted wall time | Fixed: writers store wall-clock ISO; `${now()}` (ms since launch) replaced by `${wallTime.epochAtZero + now()}` in the welcome Finish/Import and prices-save commands; nothing is stored before the host reports the date. Old pre-2026-02-07 stamps still read as unset (differs from the reference for a client with no saved environment; recorded). | `r8-pointer-clock.ts`, `pages-welcome.ts`, `sidebar-state.ts` (comments), `r9-connect-onboarding.ts` (comment), `app.contract` (2 lines), `r13-store.test.ts` (32) |
| F5 removed backend origin | Fixed: native `forgotten()` clears `t3.server.origin` after Remove / Forget and repairs a stale key; the client drops the focused environment's address, selection and cached threads after Remove (coordinator hunk). | `modules/apple/T3Transport.swift`, `macos/tests/transport/main.swift` (+5), `client.ts`, `client.test.ts` (+2) |
| F4 false slow toast | Fixed: reference latency state ported (`now` argument, no timers); a request ends on its own reply, on an environment/generation/state change, or at the transport's 31 s deadline; answered-but-let-go requests end on the next shell read (coordinator hunk in `shell.ts`). | `shell-slow.ts`, `r3-protocol-reader.ts`, `shell.ts`, `request-latency.test.ts`, `shell-slow.test.ts` (23), `shell-r2.test.ts` |
| T5 non-Git strip | Done: ported `shouldShowEnvironmentIndicator` / `shouldShowComposerContextStrip`; strip shows only "Run on" at 1280 and 840; started threads show a static machine row. | `r12-threads-strip.ts`, `composer-controls-branch.ts`, `composer-controls.contract`, `composer-shapes.contract` |
| T1 switch machine | Done: success (draft moves to B, Send shows "Preparing machine", sends on B) and failure (toast "Could not switch machine" + error, draft stays). Root cause of the old failure was the fixture (a `scratch` file; base dir inside a git worktree). Needed `native.watch('t3.notify')` in `client.ts` (coordinator hunk). | `r12-threads-scratch.ts`, `r4-git-env.ts`, `client.ts`, `r13-threads.test.ts` (17), lane tool `tools/backend-s.sh` |
| T3 device float on load | Done: reference rule (first snapshot is the baseline; a thread open before the first chunk takes the empty state) ported. | `r12-threads-device.ts`, `r13-panels.test.ts` (8), `r12-threads.test.ts` (hunk) |
| T4 tab strip at 840 | Done: active tab scrolled into view with 24 pt padding; edge fades; a new tab waits to be measured. Positions match the reference (165 vs 164, 222 vs 222); tabs restored after relaunch. | `r4-surfaces.contract` |
| F2 Files editor click | Fixed in code: the editor always takes focus when it mounts; the caret goes to the end for a press below the text (reference measured in headless Chrome). AppKit test reproduces the old failure and passes. | `modules/apple/T3PanelsNative.swift`, `macos/tests/r5-panels/main.swift` (+3) |
| F1 wheel + thread switch | No change needed: `R9Input.swift` reviewed; r9-input 13/0; agent drive restores the same rows/offsets. | — |
| T2 Worktree card | Re-checked: hides after retry. | — |

### Development checks (coordinator, 2026-10-06, integrated tree)
- `bun test examples/t3-code`: 1147 pass, 0 fail (113 files).
- Strict `tsc`: clean. `contract build`: OK (1865 slots, 41 resources, 41819 nodes).
- `cargo test -p t3-code-macos --lib`: 7/0.
- AppKit binaries (`lanes/r13-integrate/tools/swift-all.sh`, mermaid against an isolated backend
  on 16190): all pass (r5-panels 8, r9-input 13, transport and fleet suites 0 failures, mermaid
  PASS) **except `snapshot`**, which stops at "actual current layout translates printable
  letter" because the Mac's current input source is Korean 2-Set; no SnapShot file changed in
  this round (archive diff). Rerun under an ASCII layout in `verify`; the dependency on the live
  layout is a test-environment fault (related to issue X15).
- `lane-build.mjs r13-integrate`: OK (44 s).

### Not yet done (for `verify`)
- **F1 real wheel and F2 real click** (`orca computer`): not run. Blockers seen: (1) the screen
  locked after 300 s idle — fixed for runs with `caffeinate -d -u`; (2) another registered
  build of the same bundle id (`lanes/r10-connect/T3 Code.app`) was relaunched by an unknown
  bundle-id launch about 7 s after a lane app started and took focus; it was quit (user
  approval) and unregistered from LaunchServices, but other registered copies remain;
  (3) `orca computer` keyboard and coordinate clicks returned `window_not_focused` for the Exact
  window even when it was frontmost, and `type-text` did not reach the pairing field
  (accessibility presses and `set-value` work; `set-value` did not enable Pair). Pairing for
  these runs needs another path (seed the pairing by script as the F4 lane did).
- **F4 held request** (15–31 s → one toast that clears on reply): unit tests only; the frozen
  backend makes the app drop its connection first. The let-go case passed in a real launch.
- **F3 Finish-path normal launch**: blocked by the same focus problem; the saved-environment
  path passed in a normal launch (`2026-10-05T15:25:48.202Z`).

### Findings for other tickets
- `composer-controls-branch.ts` `load()` re-sends `vcs.refreshStatus` on every re-ask (18 in
  flight seen) → `20261005-client-activity-reporting` / composer work.
- `composer-editor.ts:313` stamps the stash with `new Date(n || 0)` (possible 1970 stamp).
- Lane tooling (for `20261005-clone-on-exact2-main` and U23 `tools/`): base dirs must not be
  inside a git worktree (`tools/backend-s.sh`); address lane apps by path or pid, never by
  bundle id; keep the display awake during real-input runs; unregister old lane builds.
- An `orca computer` focus limitation with Exact windows (`window_not_focused`) needs a look
  before real-input rows scale up.

Evidence: `target/t3-ui-parity/lanes/{r13-store,r13-slow,r13-threads,r13-panels,r13-integrate}/`
in the mc-orch worktree (local; not portable).

Preparation record (2026-10-05 23:25 KST):
- Checkout (environment note): mc-orch worktree
  `~/Documents/work/0.projects/exact2-worktrees/mc-orch-e88043b25805`, branch
  `mc/orch-e88043b25805` at `c1522fdac`; the app tree is untracked there (no task branch, no PR —
  the ticket's portability exception).
- Archive before any edit: `target/preserve/t3-code-r12-before-20261005.tgz` in that worktree,
  sha256 `4898542ccd21cd9a8b8344db01ee0e625983632ac9b88aee848a866ad13c0cbe` (591 entries: the
  app tree plus `js/src/parking.rs` and `js/src/lib.rs`).
- Dependency evidence: decision 2026-10-05 #1 is recorded in `spec.md` (Confirmed requirements).
- Environment: Xcode 27.0; pinned Bun 1.4.2 (`~/.bun-1.4.2/bin`); Hermes
  `hermes-6badada76212` in the root checkout's `target/t3-tools/`; oracle runtime
  `target/t3-ref/runtime-f870c41` present; lane tools in `target/t3-ui-parity/`; no lane
  process running; disk 978 GiB free.
- Limitations: T3 Code (Nightly) is running for the user. Agent drives use isolated lane ports
  and homes, so they do not touch it; the attended rows (F1 real wheel, F2 real click, F3 normal
  launch) need the user to quit it first. No computer-use tools in this session, so attended
  rows wait for an attended session.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 20261006-r13-lanes | mc-orch tree after the four lanes + coordinator hunks (archive diff: 23 changed, 5 new files) | dev checks above: pass except `snapshot` (input-source environment); real-input F1/F2/F3-finish and F4 held request not run | mc-orch `target/t3-ui-parity/lanes/r13-*` | real-input focus (`window_not_focused`); snapshot test needs ASCII layout |

## Next action

None. Superseded on 2026-10-06: the round-12 tree was imported into `feat(example)/t3-code` (base PR #99), and every later task builds on it. The rows recorded unverified above (`snapshot`, physical F1–F3 Finish, the live F4 held request) were not re-run under this ticket.
