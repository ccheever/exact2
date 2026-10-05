---
name: 20261005-round12-wrapup
plan: 20261005-t3code-macos-parity
implementation: planned
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
(`~/Documents/work/0.projects/exact2-worktrees/mc-orch-e88043b25805/examples/macos/t3-code/`).
This ticket cannot run from a fresh clone. The user chose this order (decision 2026-10-05
#1: wrap up round 12 in mc-orch, then copy). Its delivery is the import commit of the next
ticket. Before any edit, archive the tree (for example
`tar -czf <scratch>/t3-code-r12-before.tgz` of the app directory) so that no work can be lost.

## Scope and exclusions

Included (IDs from [research](../research.md) §Round 12):

| ID | Gap | Clone files | Reference |
| --- | --- | --- | --- |
| T1 | A no-project draft switches machine: Run-on lists each connected machine with a "No project" folder; picking one calls `projects.ensureScratch`; wait up to 10 s; Send shows "Preparing machine"; failure toast "Could not switch machine" + error. The last drive ended on the server's `ScratchFolderError`. | `r12-threads-scratch.ts`, `r4-git-env.ts:44-56`, `composer-controls-view.ts:114` | `apps/web/src/components/ChatView.tsx:2662-2730,4348-4420`; `useScratchProject.ts:14-22,65-78`; `packages/client-runtime/src/state/projectCommands.ts:56-62,124-150`; test `projectCommands.test.ts` |
| T3 | The open thread's device session floats on load as a mini player (not in sheet layouts ≤ 980 pt; hidden while the panel shows the same device). Drag/resize stay in `20261005-floating-device-player`. | `r12-threads-device.ts`, `r6-media-device.ts`, `r6-device.contract` | `ChatView.tsx:5285-5338`; `ChatView.logic.ts:99`; tests `previewMiniPlayerStore.test.ts`, `ChatView.logic.test.ts` |
| T4 | Right-panel tab strip at 840: overflowing tabs scroll; "Scroll tabs left/right"; active tab scrolls into view; persistence across relaunch. | `r12-threads-tabs.ts`, `r12-threads.contract`, `r4-surfaces.contract:93-94` | `RightPanelTabs.tsx:195,804-830,1015,1317-1346`; `RightPanelTabs.test.tsx` |
| T5 | A non-Git draft shows the strip with only a "Run on" chip (also when `hostsRestingComposerControls`). Written but never driven; no test. | `r12-threads-strip.ts`, `composer-controls-branch.ts:107`, `composer-controls.contract:726-728`, `composer-shapes.contract:89` | `BranchToolbar.logic.ts:67-79`; `BranchToolbar.tsx:603-612`; `ChatView.tsx:4185-4200`; `BranchToolbar.logic.test.ts:427-470` |
| F1 | Wheel scroll, thread switch, return: the position lands 64 pt off (2 of 4 real-wheel trials). The settle-tail fix is in `R9Input.swift` but its last edit is newer than its 13/0 test run. | `modules/apple/R9Input.swift`, `apple/tests/r9-input/main.swift:208-268` | `timelineScrollAnchoring.ts:164-175`; `ChatView.tsx:6552`; `timelineScrollAnchoring.test.tsx:274` |
| F2 | Files editor: a click below the last line, then typing, goes to the composer. Fix `focusFileEditor` is unbuilt. Confirm the reference caret position first. | `modules/apple/T3PanelsNative.swift`, `r4-surfaces-files.contract:206` | `components/files/FilePreviewPanel.tsx:16,599` (`@pierre/diffs/editor`) |
| F3 | First launch stores `onboardingCompletedAt` as 1970 (launch-relative `Date.now()` outside the agent). Store wall-clock ISO time. | `pages-welcome.ts:147,247`, `r8-pointer-clock.ts:8-12`, `sidebar-state.ts:102`, `app.contract:144`, reader `r9-connect-onboarding.ts` | `onboarding/firstRun.ts:13`; `FirstRunGate.tsx:157`; `contracts/settings.ts:403` |
| F4 | False "Some requests are slow" toast (shown twice after A off and B on). The reference times each request (15 s; 120 s for update methods) and clears on reply. | `shell-slow.ts`, `r3-protocol-reader.ts:47-69`, test `shell-r2.test.ts:33-84` | `rpc/requestLatencyState.ts:77-112`; `connection/platform.ts:644-657`; `SlowRpcRequestToastCoordinator.tsx:55` |
| F5 | `t3.server.origin` still names a removed backend. Remove must forget the pairing, credentials and cached threads as the reference does; the key must not outlive the environment. | `modules/apple/T3Transport.swift:101,116-119,137-152,219,829`; `T3Fleet.swift:59` | `ConnectionsSettings.tsx:2600-2640` |

Already done (no work, verify only): T2 "Worktree ready" card hides after the turn starts
(`r12-threads-worktree.ts`, `timeline-worktree.ts`).

Excluded: anything else from the upstream range (`20261005-upstream-timeline-and-markdown`,
`20261005-upstream-ui-sync`), the move and rebase.

## Context and guidance

Parent specification: [spec](../spec.md). Paths above: clone paths are relative to the
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

Checked sources and time: {{at prepare}}. Known framework gaps (X13 hover during a pan, X23
scroll restore) affect F1 and T4; the clone keeps its workarounds.

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
| Whole tree | — | `bun test examples/macos/t3-code`; strict `tsc`; `contract build`; `cargo test -p macos-t3-code-apple --lib`; all AppKit binaries; `lane-build.mjs` | All green; counts recorded | macOS | logs |

Task-owned source paths: the mc-orch app tree (`examples/macos/t3-code/**`).
Required environment: mc-orch worktree, pinned Bun and Hermes, lane backends; no T3 Code
(Nightly) running during drives.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` (archive the tree first), then `implement` in the mc-orch worktree.
