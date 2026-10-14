---
name: 20261014-legacy-sidebar-parity
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-legacy-sidebar-parity
pr_url: https://github.com/ccheever/exact2/pull/557
verified_commit: null
---

# Complete the seven remaining legacy-sidebar interactions

## Outcome

Complete `legacy-sidebar-parity`, follow-up 2 of the eight-task fix wave, after
`dialog-focus-return-and-traps` actually merges into `feat(example)/t3-code`.
Deliver all seven rows below in one application PR. These are the original
`realinput-1011c`/fix-wave row IDs, not the reused IDs in the older legacy task.

| Row | Original failure and required result |
| --- | --- |
| LS-4 | A real double-click shows a thread rename field while the composer keeps focus. Rename must focus the field and select its complete title. Start rename after the thread switch's composer focus, or suppress that competing focus while rename starts. The next typed character replaces the title. |
| LS-6 | Project settings must open Settings › Projects scoped to `project=<key>`. The Remove confirmation must reuse G2-12's two-way Tab trap. Show `Environment: <name>` in the applicable project dialogs and use `Group into…` with U+2026. |
| LS-8 | Shift-click adds the anchor-to-target range to the existing Command-click selection, as reference `threadSelectionStore.ts` does. Command-click, Command-click, Shift-click must select three threads in the three-row fixture, rather than replace the earlier selection and leave two. |
| LSF-3 | A file dropped on a background environment's thread row must switch to that environment, open that thread, and add the file chip. The reported failure showed the drop ring and then did nothing. This is drop 4; preserve already-passing drops 1, 2, 3, and 5. |
| LSF-4b | The inline archive Confirm takes focus when it appears. Preserve its existing within-row and leave behavior. |
| LS-7 | Mark unread on a thread with no completed run stays silent, as the reference does; it must not show the reported error toast. |
| LEG-TIP | A legacy tooltip closes when its owning row leaves the rendered list, including archival elsewhere or by a keyboard command, without depending on a pointer-leave event. The original proposed `includes(legacyRowIds, hoverId)` describes the intended ownership gate, not the current variable names or syntax. |

Required proof remains: meaningful Bun tests for each row, a Contract build, one
bounded before/after agent drive, and the real-input steps for LS-4, LS-6, LS-8,
LSF-3 drop 4, LSF-4b, and LS-7. Keep agent-driven native proof and actual input
qualification separate. Do not claim that the older task's passes cover these failures.

Prepared on 2026-10-14 at feature base `1025adf7888a4064eeace609fa91bc48b2938238`.
PR #553 merged at `6a7a3cad0cb5f66fb7b5d677d21f8e130356e5d7`, 05:48:59 UTC,
and is an ancestor of this checkout. The application changes and model regressions are
implemented on prepared HEAD `4b062d4356acac5b4220a09bb9d65d5c86440ca7`. Verification
is in progress; physical input qualification is open under the user’s no-screen direction.

## Scope and exclusions

All application paths below are relative to `examples/t3-code/`. Keep changes within
the application: the legacy view/model/commands/fleet/drop files, affected shared
sidebar selection and navigation paths, root hover visibility wiring, existing native
row-drop integration if reproduction requires it, and existing tests. Change shared
code only as needed for these seven interactions and protect its default-sidebar users.

The target is the existing AppKit macOS app, Apple Silicon, minimum macOS 14. The
coordinator's available host is macOS 26.6.2 build 25G83 with Xcode 27 and Bun 1.4.2;
record the actual implementation environment rather than treating this as a macOS 14
runtime qualification. Preserve light/dark appearance and default/minimum layouts.
No repeated pixel-polish matrix is required for this behavior task.

The preceding dialog task owns the shared focus lifecycle, `LegacyDialogFrame`, and
both legacy `SettingsConfirm` callers. Consume its merged implementation. Do not
reimplement final-focus ownership or revert its trap/busy behavior. Inline thread
rename and inline archive Confirm are this task's initial-focus cases.

Excluded: framework/compiler/runner/host edits; new focus/drop hooks, fixture servers,
debug endpoints, scripts or test interpreters; menu-key/popover-command retirement;
generic DOM emulation; a broad Settings › Projects fleet rewrite; other follow-up
tasks; provider sends, real GitHub writes, user-data deletion, and unrelated pixel
work. The existing declared fleet-settings difference must remain visible until
explicitly resolved. If it prevents LS-6's requested scoped route, surface that
dependency instead of marking LS-6 complete or silently widening this task.

## Context and guidance

Repository: `https://github.com/ccheever/exact2`; integration branch:
`feat(example)/t3-code`. Plan records are repository-relative under
`examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/`:

- `spec.md` and `plan.md`: parent scope and acceptance. Older `daehyeon/t3-code`
  branch names are historical.
- `reviews/20261014-first-wave-resume.md`: actual first-wave merge readback and
  retained runtime/input limitations.
- `tasks/closed/20261014-adopt-main-fixes-r14.md`: main adoption and current D1/D2
  syntax; delivery merged through PR #549.
- `tasks/closed/20261005-legacy-sidebar.md` and
  `tasks/closed/20261011-legacy-sidebar-full.md`: prior implementations and attempts.
  The latter is PR #469, including its later real-input failures.
- `tasks/20261014-sidebar-hover-archive-and-glyph.md` and
  `tasks/20261014-branch-notice-and-navigation.md`: overlapping first-wave repairs.
- `tasks/20261014-dialog-focus-return-and-traps.md`: PR #553 merged. Its bounded
  repair is available; its broader native qualification remains open.
- Application `STATUS.md` and `EXACT2-GAPS.md`: remaining input qualifications and
  declared application limits; reconcile their prose with actual source and evidence.

The original `fix-wave-plan.json` entry's full seven-row scope and proof are reproduced
above. It is not necessary to obtain that machine-local scratch file or planner chat.
Be explicit when citing IDs: PR #469's LS-4 means archive/tab behavior, LS-7 means
tooltips, and LS-8 means list motion; this ticket's LS-4 means rename, LS-7 means
silent Mark unread, and LS-8 means additive selection.

Reference repository: `https://github.com/pingdotgg/t3code`, read-only commit
`1e2ecbd9758830669684b494d4398f626b0576e0`. Relevant paths:

- `apps/web/src/components/LegacySidebar.tsx`: row rename, inline Confirm, project
  menu, per-environment actions, and dropped-file navigation.
- `apps/web/src/threadSelectionStore.ts`: `toggleThread` and `rangeSelectTo`.
- `apps/web/src/sidebarPendingFileDropStore.ts`,
  `apps/web/src/components/chat/workspaceFileDrop.ts`, and the consuming effects in
  `ChatView.tsx`/`ChatComposer.tsx`: queued files and ownership after navigation.
- `apps/web/src/hooks/useThreadActions.ts`: `useMarkThreadUnread`, including
  `reportFailure: false` on the server command.
- `apps/web/src/routes/projects.$projectKey.tsx`,
  `apps/web/src/sidebarProjectGrouping.ts`, and
  `packages/client-runtime/src/state/projectGrouping.ts`: scoped settings route and
  logical/physical project identity.

Source inspection pin: `a218ee78fec42b1a4590b199297f30667876de67`. Framework main
pin: `178ae7237edaf7b3b07aa419f0d32b98b7e5ee80`, included through merged round 14.
These identify preparation evidence, not the eventual prepared base or verified commit.

Use the Exact library `20261005-platforms-v3` for foundations, state/data,
component lifetime, accessibility, macOS and testing. It predates this runtime.
Current `rules/RULES.md`, `rules/DEFERRED.md`, `AGENTS.md`,
`docs/contract-grammar.md`, `docs/recipes/keyboard-and-focus.md`, and
`docs/testing.md` govern syntax and checks. D1/D2 is applied: view `if`/`for`,
receiver methods, `.length`, and `T[]`; do not restore retired view `when`/`each`
or free-function `includes` spellings.

## Dependencies

| Dependency | Kind | Resolution evidence / remaining gate |
| --- | --- | --- |
| First-wave repairs | merged task PRs | All eleven merged into the feature branch at checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`, per the committed first-wave readback. This checkpoint is an ancestor of the inspection pin. Relevant #545 merge is `44c4cf8a5d220d83e8ea8de58cf052066117ad0b`; #547 merge is `123d3113141d5cf36df3f4ba1617bf3d9adb033c`. Both independently read back as MERGED into the feature branch on 2026-10-14. |
| Main adoption round 14 | merged task PR | #549 is MERGED into `feat(example)/t3-code` at `3710af842bc54521c298124156989950b6f2e68c`, 2026-10-14 03:08:53 UTC. That merge and main `178ae7237` are ancestors of the inspection pin. This gate is satisfied; do not copy the earlier dialog draft's stale pending-merge state. |
| Dialog focus return and traps, especially G2-12 | merged task PR | PR #553 is MERGED into the feature branch at `6a7a3cad0cb5f66fb7b5d677d21f8e130356e5d7`, 2026-10-14 05:48:59 UTC. Prepared base `1025adf7888a4064eeace609fa91bc48b2938238` contains it. Merged helpers are recorded below; this dependency is satisfied. |
| Existing legacy implementation | merged task PR | #469 merged 2026-10-12 at `5863748f1a46c1c8255263009da83cf6fd5e7bee`; included at the inspection pin. This follow-up addresses recorded failures; it is not a duplicate implementation of the closed task. |
| Public focus and field selection | resolved framework issues | Bubbling focus `1bcf7436f` and field selection `48755525c` are ancestors. Existing `DialogOpenFocus` and `FieldOpenSelect` are public-API consumers; no new native focus hook is needed. |
| Authorized seven-row application scope | recorded decision | The 2026-10-14 fix-wave continuation names this follow-up and the original seven rows. App-only work and the instruction against repeated pixel testing remain in force. |

Fresh GitHub head-branch and title searches on 2026-10-14 found no existing
legacy-sidebar-parity PR. The coordinator created this Orca worktree from the actual
remote feature head after verifying #553 merged. It is not a dependent stack.

Issue assessment, based on the plan's open/closed records, the root closed issue
records, prior attempts, application gaps, and available GitHub readback on 2026-10-14:

| Issue/reference | Capability and target | Evidence/revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| Dialog task / G2-12 | Two-way modal traversal and return, AppKit | Shared lifecycle and both legacy SettingsConfirm callers merged through #553 at `6a7a3cad0cb5f66fb7b5d677d21f8e130356e5d7` | nonblocking | Consume its helpers and requalify legacy callers; retain its separate unverified native states. |
| #283 / X54 | Bubbling subtree focus | `issues/closed/20261009-bubbling-focus-events.md`, plan `issues/closed/20261008-x54-focus-within-subtree.md`, ancestor `1bcf7436f` | nonblocking | Use `FocusEvent.target`/`relatedTarget`; protect within-row transitions. |
| #275 / existing plain-field selection | Rename select-all | `issues/closed/20261009-field-selection-beforeinput-range-edit.md`, ancestor `48755525c`; existing FieldOpenSelect | nonblocking | Use an own `id`, `focus` and `setSelectionRange`; prove actual selection after the open-thread race. |
| #282 / X53 | GUI dialogs and focus | Root `issues/closed/20261009-gui-dialog-action-commands.md` and plan X53 resolution; current authored dialogs still have app guards | nonblocking | Keep the authored UI and use the preceding task's trap, not a native-sheet migration. |
| #490 / X79 | Ordinary button Tab stops, AppKit | Root `issues/closed/20261010-macos-tab-from-a-focused-box.md`; Keyboard Navigation off intentionally changes native traversal | nonblocking | Record the setting, explicitly establish initial focus, and verify required modal traversal. Do not globally enable a preference to mask a failure. |
| #519 / prior LSF-6 crash | Release after node removal | `issues/closed/20261013-macos-a-click-after-a-node.md`; fix `b49c56029` included via round 12 and is an ancestor | nonblocking | Preserve the fix and exercise safe Cancel paths. A recurrence needs new failure evidence, not repetition of the old crash claim. |
| X13 / X62 and first-wave hover work | Row hover and top-level tooltip presentation | Closed plan hover records and current root controller; first-wave #541 and #545 included | nonblocking | Keep the root controller and clipping-safe layer. Add ownership validity without retiring this layer or losing outside-press forwarding. |
| Native row-drop/composer attachment boundary | Background environment file handoff | Existing `t3-row-drop` hatch, `T3RowDrop.swift`, TS queue consumer, prior AppKit tests and failing real-input drop 4 | nonblocking | Reproduce and fix the app's routing/lifetime path. A generic file picker improvement does not replace row drop. Escalate a demonstrated framework blocker if found. |
| Declared fleet Settings › Projects difference | Logical project scope across environments | `EXACT2-GAPS.md`, "Legacy sidebar: Project settings on a background environment's project"; clone currently focuses that environment first | nonblocking for the bounded route repair | Preserve the declared boundary. Verify the selected project, not just page opening. If LS-6 cannot meet its scoped target within this boundary, record a blocking dependency for the coordinator. |

No new runtime failure or framework blocker was reproduced during preparation.
Closed issue evidence establishes available capability; it does not certify these
seven application interactions. A newly required capability with unknown impact blocks
implementation readiness until investigated and recorded in the plan's issue assessment.

## Implementation notes

### Current ownership and public APIs

`legacy-sidebar.contract` owns LegacyThread, LegacyRenameField, LegacyProject,
LegacyDialogFrame and tooltip triggers. `legacy-sidebar-view.ts` projects groups,
visible rows and dialog text. `legacy-sidebar-model.ts` supplies menus and ordering.
`legacy-sidebar-commands.ts` routes menu actions; `legacy-sidebar-fleet.ts` routes
background writes. `sidebar-commands.ts` owns shared selection and local Mark unread.
`legacy-sidebar-drop.ts`, `client-ops-threads.ts`, `settings-b-fleet.ts`, and
`modules/apple/T3RowDrop.swift` bridge row drop to the eventual composer owner.

`app.contract` owns the shared hover controller and clock; `app-window.contract`
receives that state and renders `HoverLayer`. `hover-layer.contract` presents it.
At the inspection pin the root is 1,498 lines and T3Window 804; source files must stay
within 1,500 lines. The first-wave root placement removed 134 unrelated window-property
captures and reduced the compiled plan. Do not move the controller back into T3Window
or forward a new broad closure that restores that regression.

Use `focus(id)` and `setSelectionRange(id, 0, title.length)` for the plain rename field.
The range is UTF-16; include a non-ASCII/emoji title in proof. `testId` does not create an
own `id`. `focusin`/`focusout` bubble with string `target` and optional `relatedTarget`.
There is no public DOM `activeElement`, `closest`, or `:enabled` API to invent.
Existing `FieldOpenSelect` focuses/selects once on first nonzero layout and resets when
closed; reread its merged implementation after the dialog task.

The dialog owner reports shared lifecycle state in T3Window, final focus outside the
mounted popup, bubbling focus to track inside targets, two-way traversal from both
SettingsConfirm/AppConfirm buttons, busy refusal, and an explicit all-disabled popup
fallback. These are ownership requirements, not frozen signatures. Reconcile the
actual merged signatures for `LegacyDialogFrame`, both legacy SettingsConfirm callers,
initial-focus helpers and final-focus hooks before editing.

### LS-4 and LSF-4b: initial focus under real event ordering

LegacyRenameField currently has `autofocus=true` and `testId=rename-${t.id}`, but no
own `id` or select-all helper. Row `press` opens a thread; `dblclick` starts rename;
the open-thread completion can request composer focus. Trace both native presses,
the double-click, asynchronous completion, and first layout. Do not add an arbitrary
delay. Preserve plain-click composer focus, editing the already-open row, Enter/blur
commit, Escape cancellation, and the nested PR/port double-click handlers.

Inline archive Confirm likewise has only autofocus/testId. Give it a stable focus
target and use the merged once-per-opening focus pattern. Its `confirmShown` depends
on `confirming`, `pointerLeft`, `focusLeft`, running state and rename state. Moving
within the row must retain Confirm; leaving the row ends it. Do not repeatedly steal
focus on unrelated snapshots or keep it alive after its owner disappears.

### LS-6: existing navigation and environment projection

The route is already implemented: `projectSettings` stores
`sidebarSession.navigate={kind:'project-settings', projectId:...}` and returns
`sidebar:navigate`; `sidebar-view.ts` projects it; the root's command completion
opens Settings, sets `settingsRoute='projects'`, and supplies the selected project.
The older "opens nothing" observation does not establish a missing handler. Reproduce
the actual command/snapshot ordering and project identity before selecting a fix.

The reference route redirects to Settings › Projects using the logical project key.
The clone currently maps background member IDs by switching environments first and
uses the focused environment's settings groups. Cover local, merged local/background,
and background-only projects, retaining the declared fleet-page limit. Do not use an
unqualified raw ID to select a homonymous project in another environment.

The follow-up source lookup confirmed that `app.ts` already converts the legacy
member ID to `client.projectGroups().key`, and `settingsScopeOf` in
`settings-core-view.ts` performs the same conversion. Existing PG-8 coverage in
`settings-diagnostics-scope.test.ts` checks this mapping. Raw member IDs at the menu
boundary are intentional. Reproduce the command/snapshot ordering before changing
that boundary or treating project-key translation as the missing implementation.

`dialogView` already emits `Environment: ...` when its state contains a label. Focused
members from `legacyGroups` currently have `environmentLabel:''`; background members
take their configuration label. Correct the missing identity/label propagation for
the applicable dialogs instead of duplicating static text. Rename and remove are
already separate projected copy paths. `Group into...` is currently ASCII in the menu
model; change the displayed copy to the requested U+2026. The pinned reference source
also contains an ASCII spelling, so cite the accepted native observation/spec for this
copy requirement rather than claiming the source already uses U+2026.

### LS-8: set union and anchor semantics

`sidebarSelecting` currently replaces selection for Shift and always sets the anchor
on a Command toggle. Match reference `threadSelectionStore.ts`: add the ordered range
to a deduplicated existing set; preserve its anchor after a valid Shift; with no valid
anchor/target in the current project list, add the clicked target and make it the
anchor. Command-toggle-on sets the anchor; toggle-off retains the previous anchor.
Keep the legacy project's visible ordering. Protect the shared default-sidebar path.

The background `select-thread` branch can enter `focusFleetThread` before the ordinary
selection helper. Include this path in the behavioral inventory and ensure modifier
selection does not inadvertently navigate an environment. Do not flatten all projects
into a new range-selection scope.

### LSF-3: queued files survive the environment transition

The native row hatch rings file drags, queues under the row's key, and clicks a row
only when it is not already open. It skips directories and stages supported files
through the existing composer attachment path. `openThreadDropKeys` currently permits
the raw thread ID and `fleet:<environment>:<thread>` alias. `takeRowDrops` consumes
matching keys, clears abandoned navigation queues, and adds with `snapshotOwner`.

Reproduce the full transition: queued under the background key, environment connection
not ready, destination thread ready, correct owner consumes exactly once. Inspect
`focusFleetThread`'s connection generation/adoption and the intermediate refreshes;
the current raw/fleet alias unit test does not prove this sequence. A homonymous raw
thread ID in another environment must not receive or clear this drop. Failed or
superseded navigation must clear its files so a later visit cannot attach stale data.
Preserve cancellation propagation and avoid replacing the native hatch without evidence.

### LS-7 and LEG-TIP: scoped silence and valid tooltip owners

Local/background Mark unread dispatch to the environment when visited tracking exists;
the legacy menu wraps failures in `attempt('Failed to mark thread unread', ...)`.
The reference server mutation uses `reportFailure:false`; its local fallback updates
a watermark only with a completed timestamp. Silence does not necessarily mean "never
dispatch." Match that behavior for a no-completed-run fixture and keep genuine
completed-run unread state and cancellation semantics. Do not silence unrelated writes.

Every legacy tooltip currently shares `HoverTip.key='legacy-tip'`. Trigger identities
are in values such as `legacy-tip-title-${id}-bubble`; project headers also have tips.
There are no `legacyRowIds` or `hoverId` variables at this base. Model the owner and
check it against current rendered legacy thread/project owners. Do not compare the
generic key to a thread list or clear every shared tooltip when any legacy row changes.
Cover removed rows, collapsed/hidden rows, project headers, switching sidebar modes,
and still-valid owners through reorder. Owner removal may deliver no leave event.
Keep delayed-open state from resurfacing an invalid tooltip.

Preserve #545's nested `sidebarPressDown` forwarding to root `outsidePressDown`, row
hover restoration, held-press dismissal, archive undo focus and PR-summary caching.
Preserve #547's branch/navigation changes. No changes to their known remaining input
qualifications are implied by this ticket.

## Acceptance and reproduction

The dialog merge/base gate is satisfied. Use existing isolated fixtures and the
task's fresh bundle. Do not use the normal T3 home,
port 3773, or the `t3code` URL scheme. Record actual fixture reset and pairing steps,
source SHA, framework SHA, toolchain, OS/build and Keyboard Navigation setting. Existing
lane setup is documented in `examples/t3-code/tools/github-lane/README.md` (from the
repository root) and the prior legacy task. Reuse the coordinator's existing isolated
lane and its `T3_GITHUB_LANE`/`T3_GITHUB_LANE_SHARED` paths, then pair through Settings ›
Connections with `bun examples/t3-code/tools/github-lane/lane.mjs pair primary`.
Do not run the GitHub lane seed/probe commands or create a replacement server for this
repair. Existing local-only fixture builders may initialize task-owned copies. If the controlled
fixture is unavailable, record that prerequisite. Pairing credentials stay out of
committed evidence. Do not send provider messages or make GitHub mutations.

Fixture: at least three ordered local project threads A/B/C, one completed-run thread,
one with no completed run, a background environment with its own thread/project, and
homonymous thread IDs in the controlled test model. Enable Settings › General › Legacy
features › Sidebar (legacy). Toggle Archive confirmation for its dedicated case. Use
a harmless text file, PNG and folder from the existing temporary fixture; preserve
composer draft and right-panel state. Real-input archival uses disposable fixture data
with undo; project/thread removal tests cancel destructive confirms.

### Meaningful regression coverage

Extend `legacy-sidebar-full.test.ts` and `legacy-sidebar.test.ts`; use existing
`contract-action-fixture.ts` for actual action/derive behavior and the existing client/
native test doubles for navigation and queued drops. Relevant regressions are in
`sidebar.test.ts`, `sidebar-hover-archive-and-glyph.test.ts`,
`pr-list-scroll-and-sidebar-sweep.test.ts`, `hover-layer.test.ts`,
`dialog-trap.test.ts`, and `pr-list-place-and-thread-picker.test.ts`.
Source inventory assertions remain wiring checks, not focus/navigation proof.

| Row | Required behavioral assertions |
| --- | --- |
| LS-4 | Both open-row and different-row double-click sequences end with the rename field focused and range `[0,title.length]`; a later open-thread result cannot retake focus. Typing replaces the complete ASCII/Unicode title. Plain click still opens/focuses the composer; PR/port double-click does not start rename. Enter, Escape and blur remain correct. |
| LS-6 | The real menu result opens Projects and selects the correct target for local, merged and background-only fixtures. Applicable rename/remove copy names the environment and menu copy uses U+2026. Both Tab directions from each confirm button remain inside, including busy/all-disabled behavior inherited from G2-12; dismissal has one final-focus owner. |
| LS-8 | A Command-click A, Command-click C, Shift-click B sequence selects A/B/C. Add forward/reverse ranges, existing out-of-range selection, no/missing anchor, another project's target, duplicate-free repeated Shift, toggle-off anchor stability and plain-click clearing. Protect default sidebar and background modifier handling. |
| LSF-3 | Pending background drop survives connection transition, attaches to the intended environment/thread exactly once when ready, and does not land on an identically named/raw-ID thread. Multiple files retain order. Failed/superseded navigation cleans pending files. Existing open-row, local-row, image, folder and panel-preservation cases stay green. |
| LSF-4b | Showing Confirm focuses its actual mounted ID once; unrelated updates do not steal focus; pointer/focus movement within the row preserves it; leaving/removing/running/renaming closes it; reopen focuses it again. A check for `autofocus=true` alone is insufficient. |
| LS-7 | No-completed-run local and background fixtures produce no error toast or fabricated unread watermark. Exercise visited-tracking server response and local fallback. A completed run still becomes unread; unrelated failures and cancellation retain their proper behavior. |
| LEG-TIP | A visible or delayed legacy tip cannot survive owner removal without leave. Archive elsewhere/key, collapse/hide, project removal and mode switch dismiss invalid owners. Reorder retains a valid owner; an unrelated default-sidebar/usage/composer tip is not cleared. |

Use the existing compiled-plan/AppKit patterns if action-model tests cannot observe
the command-completion ordering. `macos/src/composer_accessibility_tests.rs` shows a
real compiled plan/host fixture; do not create a second test framework. If Swift row
drop changes, run `macos/tests/row-drop/main.swift` through the app README's existing
module-test recipe with generated ExactDataKeys and `ExactNativeModule.swift`.
Its five AppKit cases prove drag destination/queue/staging mechanics, not Finder's
end-to-end background-environment handoff.

Focused commands from the repository root, with the pinned Bun on PATH:

```sh
bun install --frozen-lockfile
bun test ./examples/t3-code/legacy-sidebar.test.ts ./examples/t3-code/legacy-sidebar-full.test.ts ./examples/t3-code/sidebar.test.ts ./examples/t3-code/sidebar-hover-archive-and-glyph.test.ts ./examples/t3-code/hover-layer.test.ts ./examples/t3-code/dialog-trap.test.ts --timeout 60000
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/t3-legacy-sidebar.plan
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
```

Run changed related tests and the full app suite for final verification. Required
repository checks use current instructions: default-member build, test, Clippy plus
fmt, caps, boot, and the workspace check required by `rules/RULES.md`. App Rust tests
are outside default members; run `cargo test -p t3-code-macos --lib` when touched.
The coordinator may supply integration checks only when their source identity covers
the change. A Cargo build does not compile/launch the app's Swift module. Do not repeat
passing broad checks without a new change or unresolved concern.

### Bounded native and real-input pass

Coordinate the Mac input slot. Use `scripts/agent.mjs macos` against the task's current
bundle; record `tree`, `tree --ax`, relevant client state, focus/selection and errors.
For authored-clock proof use `clock settle`; for OS-input proof preserve the live wall
clock and observe readiness. `s.tap()` advances/freezes the clock even in live mode;
use a raw existing operation only for explicitly labelled setup. Obey stale-build refusal.
Capture one before/after drive with a per-row assertion summary. Then qualify these
real-input steps on the same final source; an agent command named dblclick is not by
itself proof of the originally failing physical double-click sequence.

1. LS-4: with the composer focused, physically double-click another thread title;
   confirm field focus/select-all, type a harmless replacement, and cancel. Repeat on
   the already-open row and with an emoji title. Confirm no draft text changed.
2. LS-6: pick Project settings from local and background project menus; record the
   selected project and environment. Inspect `Environment: <name>` and `Group into…`.
   Open Remove, cycle Tab/Shift+Tab from both buttons, then cancel; no deletion.
3. LS-8: Command-click A, Command-click C, Shift-click B; record all three selected.
   Toggle one off and extend again to expose anchor regressions.
4. LSF-3: drag the text file from Finder onto the background environment's thread row.
   Observe ring, environment switch, destination thread and file chip. Retain the
   existing drop regressions: text onto an unopened local row; PNG onto the open row;
   folder alone does nothing; dropping onto the open row preserves its right panel.
5. LSF-4b: show inline archive Confirm and record native focus immediately. Move
   pointer/focus within the row, then away; reopen and use keyboard activation only
   on disposable fixture data with undo. Check the draft survives archive/undo.
6. LS-7: use Mark unread on the fixture with no completed run, locally and in the
   background environment. Record the absence of an error toast after completion;
   do not generate a provider run to create this fixture.
7. LEG-TIP: show a legacy title/PR tooltip and remove its owning visible row via a
   fixture update or keyboard archive while leaving the pointer still. The tip must
   disappear and not return after its delay. Spot-check a header tip and a valid
   reordered row, then the first-wave default-sidebar hover/held-press cases.

Screenshots support visible state; AX/focus, queued-file ownership, menu results and
selection assertions establish behavior. Record unavailable real-input cases as open,
never as inferred passes. No web/iOS/Linux or macOS 14 runtime parity is claimed.

## Progress

| Stage | State |
| --- | --- |
| Original scope | All seven fix-wave rows reproduced and reconciled with prior attempts. |
| Source/reference inspection | Complete at integration `a218ee78f` and reference `1e2ecbd975`. |
| First-wave and round-14 merge gates | Satisfied at the inspected base; relevant PRs independently read back. |
| Dialog merge and interface reconciliation | Satisfied through #553; merged helper ownership checked on the prepared base. |
| Portable ticket | Promoted to the plan task directory with the actual dependency and prepared base. |
| Implementation checkout / branch / PR | Orca checkout `t3-code-legacy-sidebar-parity`, branch in frontmatter, base `1025adf7888a4064eeace609fa91bc48b2938238`. No PR yet. |
| Code changes | All seven application repairs implemented; runtime and test source fingerprint recorded in evidence. |
| Verification | Full Bun 5,073 pass / 1 skip / 0 fail; strict TypeScript and Contract build pass. All 29 app Rust tests, the fresh native build and bounded rename agent before/after pass. Repository checks pass; physical input remains open. Physical input remains open. |
| Independent implementation review | Source approved at fingerprint `50c11104961f002747f3d7b9dc7f5be64b2571e2a9c010219b25fd497fc7158f`; independent focused suite 396 pass, 0 fail. Native qualification is excluded from that approval. |

## Attempts and evidence

Preparation on 2026-10-14 inspected the current rules/deferred list, parent plan/spec,
reference pin, both old legacy tickets, original seven-row specification, current
source and public API, first-wave sidebar/branch repair records, main adoption,
existing tests/native recipes, open/closed issue records and declared limits.

Git status was clean at `a218ee78fec42b1a4590b199297f30667876de67` before writing this
ignored draft. Read-only remote state confirmed the same feature head during initial
preparation; PR #469, #545, #547 and #549 were read back as merged, and their inclusion
was checked. First-wave checkpoint, main, focus/selection fixes, and releaseHold fix
are ancestors. No fetch, branch creation or remote mutation occurred in this stage.

Prior evidence must be retained, not promoted to current passes:

- PR #469's old agent/model results later disagreed with `realinput-1011c` for rename,
  inline Confirm, local project menus and background drop 4. Its closed task explicitly
  records those failures and the old releaseHold crash.
- [Original LSF-3 drop log](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/LSF-3-drops.txt)
  was read during preparation: the background target did not open; its nominal next
  open-row drop was corrected to the actually open local row.
- First-wave #545 evidence is under
  `evidence/20261014-sidebar-hover-archive-and-glyph/20261014-resume/`: native hover,
  held-press and PR #113 passed after the async repair; real input/Archive qualification
  remains separate. Preserve its failed plan-only rebake and successful full fallback.
- #547's task and `evidence/20261014-branch-notice-and-navigation/` protect existing
  branch/navigation behavior. Its transparent captures were not visual proof.

No runtime root cause is claimed solely from inspection. Future attempts must record
the exact prepared/final SHA and source fingerprint, fixture/reset, commands,
assertions, result, artifact paths and unresolved limits. Keep failed attempts and the
repository's three-round repair limit; another worker does not reset the count.

### Prepared merged interfaces and fixture availability

The coordinator reread these merged interfaces at the prepared base:

- `dialog-trap.contract`: `DialogOpenFocus(target)` focuses once on a nonzero layout;
  `FieldOpenSelect(target, length)` then selects `[0,length]`. Both reset on zero layout.
  `FocusGuard(guardId, targets, popup)` filters explicit `data-focus-stop="enabled"`
  members and falls back to the popup when no target remains.
- `app-window.contract`: T3Window owns `dialogFocusId`, frames/lineage, menu popup and
  invoker capture. It provides `dialogEntered(owner,event)` and
  `dialogFallback(popup,targets,key,event)` to descendants. `DialogFinalFocus(open,owner)`
  outside the conditional popup reports opening/closing to that single owner.
- `LegacyDialogFrame(title,description,label,first,busy,cancel,save)` injects those
  actions, declares its popup as `data-dialog-focus="open"`, and marks only enabled
  controls. Both legacy `SettingsConfirm` callers retain their distinct IDs.
- `SettingsConfirm(title,description,confirmLabel,destructive,busy,error,dialogId,cancel,confirm)`
  explicitly cycles both Tab directions between Cancel and Confirm, or to the popup
  while busy. Its shared implementation remains owned by #553.

Local environment notes, not portable task identities:

- Checkout: `/Users/daehyeonmun/orca/workspaces/exact2/t3-code-legacy-sidebar-parity`.
  Use Bun `/Users/daehyeonmun/.bun-1.4.2/bin/bun` through PATH and this checkout's own target.
- Existing audit root: sibling `t3-code/target/t3-audit`. Its
  `lanes/ri1012-b12/clone-t3-home` contains four disposable local legacy fixture threads;
  all currently have zero runs. Its `seed/t3-home` is an available clean configuration.
- Existing historical fixture source, read-only:
  `/private/tmp/claude-501/-Users-daehyeonmun-orca-workspaces-exact2-t3-code/ff0894f2-cfb5-44b9-94c8-a8504a994fef/scratchpad`.
  `v1011c/b12/github-lane/servers/primary/t3home` contains the background thread fixture;
  `v1011c/b12/drop/{note.txt,shot.png,only-folder}` supplies drop assets.
  The coordinator's read-only lookup checked both SQLite databases with `quick_check`.
- Reuse existing local fixture tools against new task-owned homes and repo copies only:
  pinned audit `t3` runtime `project add`, `legacy-sidebar-full/seed-threads.py` for an
  offline owned DB, and `v1011c/tools/t3api-1012.mjs` for a local thread without a run.
  Use the repository's existing `tools/github-lane/lane.mjs` only with the owned lane.
  Do not execute historical hardcoded launch/stop wrappers or mutate their files.
- No original provider session may be resumed or edited. Do not copy active database
  WAL files unsafely. Do not use a historical PID file to stop a process. Historical
  fixtures are local-only dependencies and must be labelled that way in published proof.
- The macOS desktop slot is assigned to this task once implementation begins. User
  authorization covers Orca OS-input tests. Receipts identify these as synthetic input;
  do not call them human input. Use `EXACT_AGENT=live`, passed explicitly to the existing
  agent `open()` extra environment, and `EXACT_DEVICE=real` for native control behavior.
  Global Keyboard Navigation is off and must remain unchanged. The screen locked during
  preparation; the user then directed continued work without screen input. OS input is
  paused. Existing agent API checks continue with `EXACT_AGENT=1`; no unlock is assumed.

The task-owned copies were initialized and paired. A baseline agent-dispatched double-click
reproduced the rename field mounting while composer focus remained. Full raw snapshots
and pairing material stay local; the selected packet is sanitized.
Completed-run behavior has model coverage available; do not generate a provider run
merely to satisfy a fixture. The declared background Projects-page limit remains.

### Implementation and verification — 2026-10-14

The selected packet is [evidence/20261014-legacy-sidebar-parity](../evidence/20261014-legacy-sidebar-parity/README.md).
The [published packet](https://github.com/ccheever/exact2/blob/741e9780b1bbcbfff1cf3d0905d75eea4246c4f1/legacy-sidebar-parity/20261014/README.md) and [native capture](https://raw.githubusercontent.com/ccheever/exact2/741e9780b1bbcbfff1cf3d0905d75eea4246c4f1/legacy-sidebar-parity/20261014/rename.png) are pinned to evidence commit
`741e9780b1bbcbfff1cf3d0905d75eea4246c4f1` on `ccheever/exact2`’s `t3-code-evidence` branch.
The packet maps each original row to the implementation and its proof level, records the
source fingerprint and independent review, and preserves the bounded baseline result.
Raw logs and private fixtures are under `target/legacy-sidebar/` in the implementation
checkout; pairing credentials and full client snapshots are not published.

The new client regressions failed before their fixes. Review additionally reproduced
two asynchronous drop races: destination cleanup lost during a deferred clear, exposed
by a later reopen, and an image fallback chip written into the newly opened draft after
a deferred editor reply. Both were fixed and the regressions now pass. Project Settings
now carries its member ID in the command reply; this proves removal of the snapshot
dependency, not the original native menu failure’s cause. Existing shared dialog traps,
final focus, raw-member project mapping and default-sidebar behavior are retained.

One implementation repair round also corrected a Contract stale-read diagnostic and
obsolete hover wiring assertions; failed logs remain local. Full app Bun: 5,073 pass,
1 skip, 0 fail, 120,214 assertions across 335 files. Strict TypeScript, Contract build and
format checks pass. Independent source review: 396 pass, 0 fail, 2,449 assertions.
Both compiled-plan regressions pass after adding the test fixture’s missing mutation
callback settlement step; all 29 app Rust tests pass. The fresh native bundle builds.
The coordinator’s default-member build/test/Clippy/boot, workspace, formatting, caps and
Contract source sweep pass (94 roots, 195 imports). The clean native before/after uses
the same raw agent operations and short clock advances, with empty app error strings.
Before, rename mounts while composer focus remains; afterward the field owns native
FieldEditor focus. One native agent key replaces the complete title, and Escape restores
it. Different-row double-click also opens/focuses rename. The screenshot and exact
readbacks are in the packet; they do not qualify physical input. All recorded task
app/controller/backend PIDs are stopped and ports 16566/16567 have no listeners.

The earlier live setup revealed that the attempted preference flags did not isolate
saved environments. Only this task’s exact catalog entry and Keychain account were
removed; the coordinator independently verified the restored preferences against the
prelaunch snapshot and absence of the account. `AGENT-HANDOFF.md` records that finding.
Later checks use the module’s in-memory credentials under `EXACT_AGENT=1`. No provider
run, GitHub mutation, normal user-project change or OS-global input was used.

### Publication and merge readback

PR [#557](https://github.com/ccheever/exact2/pull/557) merged into
`feat(example)/t3-code` on 2026-10-14 at 07:06:56 UTC, squash commit
`4520b0124b36cf24481a973f59a414ae8753dab1`. Its published head was
`6f40c58f82736f355712a0ddae7507113d1da38c`, containing implementation
`dbd2cb3970c89306672371ee4f21064df9ee466e` and the feature branch's documentation-only
dialog cleanup record. Both remote Socket checks passed. The coordinator read back
the merged state and integration head, then matched all 23 runtime/test file hashes
against fingerprint `50c11104961f002747f3d7b9dc7f5be64b2571e2a9c010219b25fd497fc7158f`.
The [published proof](https://github.com/ccheever/exact2/blob/741e9780b1bbcbfff1cf3d0905d75eea4246c4f1/legacy-sidebar-parity/20261014/README.md)
includes the native capture and bounded before/after readbacks. Delivery is merged;
full task verification remains open.

### Real-input session realinput-1015, 2026-10-15

Bundle of `db3c4540f` (code `aa4fd19c3`) in a lane copy (`com.exact.t3code.macos.ri1015-1`) launched through LaunchServices (a normal launch, not agent mode), port 16915, isolated homes from audit lane `ri1012-b10` plus the real-GitHub lane's sandbox (read only). Real keys and pointer through `orca computer`; the input source was `com.apple.keylayout.2SetHangul` (read only, not changed). While a painted popover or dialog is open the window's accessibility tree collapses and `orca` refuses input, so those keys went as System Events key codes to the lane (front process checked first). Evidence is local in the session worktree's `target/realinput-1015/` (`notes.txt`, `ev/`). 32 steps in all: 27 passed, 2 failed, 3 not verified.

| Row | Result | Evidence (`target/realinput-1015/ev/`) |
| --- | --- | --- |
| LS-4 (steps 14, 15) | PASS: with the composer focused, a real double-click on another row and on the open row opens rename with the whole title selected (AX range 1..30); typing replaces it, Escape restores. The emoji-title variant was not run (no fixture) | `s14-d`, `s14-e`, `s14-f`, `s15-b` |
| LS-6, local project (step 17) | PASS: the project menu's Project settings opens Settings with Project scope: work; Remove shows "Environment: Daehyeon's MacBook Pro" and traps Tab both ways; Cancel removes nothing. Background-environment half UNVERIFIED (no second environment) | `s17-a-small.png`, `s17-d`, `s17-e` |
| LS-8 (step 16) | PASS: ⌘-click A, ⌘-click C, ⇧-click B selects three; ⌘ toggle off then ⇧-click extends from the new anchor | `s16-c-crop.png`, `s16-e-crop.png` |
| LSF-3 drop 4 | UNVERIFIED: no background environment, and a Finder drag would drive another app | — |
| LSF-4b (step 19) | PASS: with Archive confirmation on, a real click on the inline Archive shows Confirm with the native focus on it; Tab away reverts it. An accessibility-path press archives at once (known difference) | `s19-n`, `s19-o`, `s19-q` |
| LS-7, local (step 18) | PASS: Mark unread on the no-run thread shows no toast. Background half UNVERIFIED | `s18-c-crop.png`, `s18-f-small.png` |
| LEG-TIP (step 20) | UNVERIFIED: the tip shows at rest, but no keyboard path removed its row with the pointer still | `s20-b-crop.png` |

## Next action

After realinput-1015 (2026-10-15): Keep `verification: unverified`. realinput-1015 passed LS-4, LS-8, LSF-4b and the local halves of LS-6 and LS-7 by real input; LSF-3 drop 4, the background-environment halves and LEG-TIP's row removal stay open for a session with a second environment. No new task; the new differences seen are in `STATUS.md` "Known differences".

Continue `connections-network-settings` from the actual merged feature branch.
Resume the seven physical-input qualifications only when screen input is authorized
and available. Keep this task active and unverified until those checks are actually
run. The squash merge preserves the tested implementation and does not close its
remaining acceptance items.

The user has already authorized continuing this fix wave through publication and
merge of solvable application repairs. The coordinator owns those external actions.
Keep full verification open for any specifically unavailable native qualification;
merging a bounded repair does not by itself close the ticket.
