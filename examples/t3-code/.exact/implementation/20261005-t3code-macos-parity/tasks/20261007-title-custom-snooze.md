---
name: 20261007-title-custom-snooze
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-title-custom-snooze-v2
pr_url: https://github.com/ccheever/exact2/pull/299
verified_commit: null
---

# Open Custom snooze from the thread title menu

## Outcome

Thread title > Snooze > Custom opens the existing Custom snooze dialog for that thread, as
the sidebar's Custom action does. Choosing a valid time snoozes the intended thread; Cancel
or Escape leaves its snooze state unchanged.

This began as a discovery record from the 2026-10-07 desktop comparison; the fix is in
"Implementation" below.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.

The missing route was repeated in the running Exact app on the same idle, unsnoozed scratch
thread, `40c97546-4d18-4d65-bb04-3f859f73bdcb`:

1. Open the thread, press its title (`thread-title`) and hover Snooze (`title-menu-snooze`).
2. Press Custom (`title-menu-snooze:custom`). The menu closes, but no Custom snooze dialog
   appears, including after `clock +500 real`.
3. Hover the same thread's sidebar row, press its snooze button, then Custom
   (`snooze-<thread-id>-custom`). The Custom snooze dialog opens with Date and Time fields.
4. Press Cancel (`snooze-cancel`). No snooze value was saved during this comparison.

The reference Electron app was also driven through **Thread title > Snooze > Custom**.
It opened the Custom snooze dialog with Date and time / Duration choices, Date, Time,
Cancel and Snooze. Pressing Cancel closed it without confirming a snooze.

Local evidence, relative to the checkout root (not committed):

- `target/desktop-audit/native/native-final-scratch-title-custom-no-dialog.{png,json}`
- `target/desktop-audit/native/native-final-scratch-title-custom-settled.{png,json}`
- `target/desktop-audit/native/native-final-sidebar-custom-snooze.{png,json}`
- `target/desktop-audit/native-actions.ndjson` records both routes and the cancellation.
- `target/desktop-audit/evidence/ref-panels-title-custom-snooze.{png,txt}`
- `target/desktop-audit/evidence/ref-panels-title-snooze-cancelled.{png,txt}`

The title-route captures contain neither `title-menu` nor `sidebar-snooze` after the click.
The sidebar capture contains `sidebar-snooze` (`Custom snooze`), `snooze-date`, `snooze-time`
and `snooze-cancel`. The reference capture contains `dialog "Custom snooze"`; the cancellation
capture no longer contains that dialog. The source path below confirms both reference entry
points use the same dialog.

## Cause and implementation guidance

The clone already constructs the title submenu's Custom entry: `shell.ts:198` emits
`op: "ui:custom-snooze"` with the thread id. `shell-panels.contract` forwards it to
`titleMenuPick`. That action in `app.contract:1430-1456` closes the title menu, handles
`ui:rename`, `ui:project-settings` and `ui:confirm`, but has no `ui:custom-snooze` branch.
Its generic dispatch explicitly excludes other `ui:` operations. Nothing opens the dialog.

The working sidebar route is `sidebar-row.contract:148` → `sidebarRun` in `app.contract:1152`
→ `sidebar:snooze:custom` → `sidebar-commands.ts:489` `openSnoozeDialog`. Route the title action
through this existing command/dialog behavior for its explicit target thread. Retain the
current command clock, capability checks, validation, cancellation and confirmation behavior.
Opening Custom must not immediately send a `thread.snooze` mutation.

Reference paths in `apps/web/src/`:

- `components/chat/ChatHeader.tsx:136` installs `useThreadActionMenu`; `openTitleMenuNow`
  invokes it at line 160.
- `hooks/useThreadActionMenu.ts:160-165` recognizes `snooze:custom`, awaits
  `requestCustomSnooze()`, returns on cancellation and snoozes only after a choice.
- `components/Sidebar.tsx:597` calls the same `requestCustomSnooze()`.
- `components/CustomSnoozeDialog.tsx:39` stores the request; its host renders the dialog at
  line 53. `routes/__root.tsx:239` mounts that host globally.

## Dependencies and deduplication

- [Thread commands and keys](closed/20261005-thread-commands-and-keys.md) owns worktree deletion
  and its listed shortcuts. It does not implement this title-menu action.
- [Minor UI fixes](closed/20261007-fix-minor-ui-issues.md) fixes title-menu settlement and
  snooze availability rules, not the Custom action route. Its `shell.test.ts` check establishes
  that the Custom label exists; it does not execute the Contract handler.
- [Reference test inventory](20261005-reference-logic-tests-done-areas.md) records tests and
  explicitly excludes fixing divergences. It does not track this observed interaction failure.
- No framework issue is needed for this finding: the same app already opens the dialog from
  the sidebar. This functional omission is separate from the shared visual-parity task.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Title route | On an eligible thread, open title > Snooze > Custom | Title menu closes and the existing Custom snooze dialog opens for that thread | Live native capture, alongside the reference route |
| Same behavior from both entry points | Open the dialog from title and sidebar on the same thread | Matching initial date/time, Duration mode, validation and controls | Paired native captures |
| Cancel and Escape | Open from the title and cancel by each method | Dialog closes; no thread.snooze request and no snooze-state change | Bounded drive and request/state readback |
| Confirm | Choose a valid future date/time, then a valid duration in a separate repeat | Only the intended thread receives the expected snoozedUntil value | Request/state readback |
| Unavailable actions | Use a running, ineligible or unsupported thread | Existing reference-compatible Snooze visibility/disabled rules remain | Targeted cases and UI capture |
| Keyboard and target | Open by keyboard, choose Custom and return; switch threads before reopening | Usable focus and Escape behavior; no dialog targets a previously open thread | Bounded live drive |

## Implementation (2026-10-08, `feat(example)/t3-code-title-custom-snooze-v2`)

Taken over from an earlier session's uncommitted attempt (kept, unchanged, on the local branch
`backup/t3-code-title-custom-snooze-wip-20261008`, `9697b1dd1` on `f90277989`). Its route through
`sidebarRun` and its submenu-press `activate` were kept and re-applied on `07dcef1ab`; its
`Host::boot` regression test (three failed fixture rounds) and its `use`-line merges were not.

- `app.contract` `titleMenuPick`: `ui:custom-snooze` is an `else if` arm of the action's one send:
  `sidebarRun("snooze:custom", target, "title")`, so the title's thread, the window's wall time,
  the busy guard and the sidebar's dialog are the row's. The dialog takes the focus at open
  (`sidebarDialogFocus`, #255); `focusSidebarDialog` notes a dialog the title opened
  (`titleDialog`), and the gated task `titleDialogClosed` gives the focus back to `thread-title`
  however the dialog closes, Snooze included (Base UI's finalFocus).
- `sidebar-commands.ts`: `snooze:custom` with the value "title" opens the dialog for that one
  thread (`from: 'title'`, never the sidebar selection). Its confirm is useThreadActionMenu's
  `snoozeThread`: `snooze()` (the "Snoozed" undo notice) without the sidebar's forward navigation
  (`useThreadActionMenu.ts:160-166`, `useThreadActions.ts:883-928`; only `Sidebar.tsx:4100-4128`
  navigates). `sidebar-view.ts` `dialogReturn` is `thread-title` for it, so Cancel, Close and
  Escape (`SidebarSnoozeDialog`'s `dismiss`) return there instead of the row (#255's report).
- `shell-panels.contract` `TitleMenu`/`MenuRow`: pressing a submenu row (click, Enter, Space)
  opens the submenu with the focus on its first enabled item, as the reference's menu fallback
  does (`contextMenuFallback.ts:435-470`, `openSubmenu(true)`); before, Enter on Snooze did
  nothing, so Custom was unreachable by keyboard. Escape or a press outside gives the focus back
  to the title when the title opened the menu (`titleMenuFromTitle`; a right-click leaves the focus
  where it was, as the fallback's cleanup restores what held it, lines 237-255). A disabled row hears
  no pointer (`MenuRow` `point`; the fallback gives it `pointer-events: none` and no listeners), so a
  disabled Snooze never opens its submenu. The rows' labels are start-aligned (a button centers its
  text; the fallback's rows do not).
- `sidebar-commands.ts` `snooze()` refuses a thread waiting on the user before the round trip, with
  the reference's message (`useThreadActions.ts:897-909`, ThreadSnoozeBlockedError), for every caller.

Tests (the title route's — the Rust title test and the first two Bun tests — fail on the base sources and pass here; the row and availability tests pass on both, as guards):
- `macos/src/title_snooze_tests.rs`: the compiled plan's root actions with a recording data source
  (no JS engine): `titleMenuPick("ui:custom-snooze", "thread-b", …)` closes the menu and asks
  `command("sidebar:snooze:custom", "thread-b", "title", <window wall time>)` once; a second pick
  waits for it. With the base `app.contract` it asks nothing (`left: []`). A second test pins the
  row's `sidebarRun("snooze:custom", …)` request.
- `sidebar.test.ts` "Custom snooze from the thread title menu": the title opens the same dialog as
  the row (mode, date, time, amount, unit) for its own thread, not the selection, with
  `dialogReturn: thread-title`, sending nothing; Cancel sends nothing; a past date keeps the
  dialog with its error; a date, then a duration, snooze only the title's thread with the expected
  `snoozedUntil`, the undo notice, and no navigation; the row's Custom still moves on; Custom is
  offered only where Snooze is (disabled while the thread waits on the user, hidden when snoozed
  or unsupported); a thread that came to wait on the user while the dialog was open is refused
  with the reference's toast and nothing is sent.

## Results

| Row | Result | Proof |
| --- | --- | --- |
| Title route | pass (agent): the menu closes, Custom snooze opens for the open thread, focus on "Date and time"; base: nothing opens, nothing focused | drive After 1 / Before 2; image 01; `title_custom_snooze_asks_the_sidebar_command_for_its_own_thread` |
| Same behavior from both entry points | pass: one dialog and one opener (`openSnoozeDialog`): an hour out, Date and time, 2 hours, same validation and controls | drive After 1 and 6 (both 14:22 at 13:22 window time); image 05; sidebar test 1 (snapshots equal) |
| Cancel and Escape | pass (agent): Escape, Cancel and Close each close it with the focus on the title; no `thread.snooze` receipt, no state change | drive After 2 and 5; image 03; sidebar test 1 |
| Confirm | pass (agent): tomorrow 10:30 → exactly one receipt, tcs-a `2026-10-09T01:30:00.000Z`; after switching threads, 3 hours → exactly one receipt for tcs-b only; the thread stays open, "Snoozed 1 thread," | drive After 3-4; image 04; sidebar test 2 |
| Unavailable actions | pass (unit): Snooze disabled while waiting on the user, Wake instead when snoozed, nothing without the capability (`shell.ts` unchanged); a disabled row hears no pointer, and `snooze()` refuses a waiting thread. Live capture of a disabled Snooze: blocked — needs a provider turn paused on a request (no signed-in provider lane; a login needs the real-input lock) | sidebar tests 3 and 4; `shell.test.ts` title-menu tests; review round 1 B1 |
| Keyboard and target | pass (agent keys): Enter on the title opens the menu, Enter on Snooze focuses "In 1 hour", Tab ×5 reaches Custom…, Enter opens the dialog for the newly open thread, Escape returns to the title; Escape in a menu the title opened returns to the title, in a right-click menu the focus stays put. Real keys and the ring: deferred to the real-input batch (lock held by the batch agent at the end of this session) | drive After 5 and Retry 1-3; image 02; steps below |
| Desktop oracle comparison | not run — user decision 2026-10-06; the reference paths above were read instead | — |

Evidence (`t3-code-evidence/title-custom-snooze/`):
[01 title Custom](https://raw.githubusercontent.com/ccheever/exact2/3317471028c8ce65da68bf85a2904f6078cf753c/title-custom-snooze/01-title-custom.png),
[02 Enter on Snooze](https://raw.githubusercontent.com/ccheever/exact2/cc6abab927b1743ba1be27026a3a36091c0742b1/title-custom-snooze/02-snooze-enter-focus.png),
[03 Escape → title](https://raw.githubusercontent.com/ccheever/exact2/b64d37e6eab590907ce27b2336c4e8f7068c531b/title-custom-snooze/03-escape-returns-to-title.png),
[04 confirmed](https://raw.githubusercontent.com/ccheever/exact2/db71a5515cd9bff6989d7654787f17e11ff2453d/title-custom-snooze/04-title-confirmed.png),
[05 sidebar Custom](https://raw.githubusercontent.com/ccheever/exact2/3ea66b021b28df508d073d0b41aa7da4b1371fa6/title-custom-snooze/05-sidebar-custom.png),
[agent-drives.txt](https://raw.githubusercontent.com/ccheever/exact2/d06d584735f6da214f42b184918bc327b8721ef9/title-custom-snooze/agent-drives.txt).

Not done or not verified, each with its blocker:
- Real keys (Enter, Tab, Escape) and the focus ring on the title: deferred to the real-input
  batch — the shared lock was held by the batch agent when this session ended. Steps below.
- A live capture of a disabled Snooze row: needs a thread waiting on the user, i.e. a real
  provider turn paused on a request; no signed-in provider lane exists (`provider-lane` has no
  READY file) and a login needs the real-input lock. Steps below (step 5).

Independent review: round 1 FAIL (B1: a disabled Snooze row could still open its submenu by hover,
and a Custom… there would now reach `thread.snooze`; non-blocking: Escape after a right-click moved
the focus to the title) → repaired by `point`, the `snooze()` guard and `titleMenuFromTitle`, and
re-driven on the final build (Retry section of `agent-drives.txt`); round 2 PASS with non-blocking
notes ([review.md](../evidence/20261007-title-custom-snooze/review.md)).

Found here, not in this task's scope:
- The title menu's preset snoozes (`chat:snooze`) dispatch without the "Snoozed" undo notice and
  without `snooze()`'s waiting-on-the-user guard; the reference's go through `snoozeThread` (with
  both), as Custom… now does.
- After a right-click-opened menu, Custom…'s dialog returns the focus to the title, not to what held
  it before (the reference's finalFocus); restoring an arbitrary earlier focus needs the host modal
  of [X53](../issues/20261008-x53-state-driven-modal-focus.md).
- The in-app title menu has no ↑/↓ between items (the reference's DOM fallback has none either;
  its desktop build uses the native menu, which does).
- A toast stays above the Custom snooze overlay (both entry points, unchanged).

## Real-input batch steps

One session under the real-input lock (owner note "title-custom-snooze: real keys"). Build this
branch's bundle: `EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos
--bundle`; copy it to `T3 Code (Lane TCS).app`, set `CFBundleName`/`CFBundleDisplayName` and the
bundle id `com.exact.t3code.lane.tcs`, `codesign --force --deep -s -`. Lane: start the staged
runtime's `t3 serve --host 127.0.0.1 --port 16552 --base-dir <lane>/t3-home --no-browser` with an
isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_* and `T3CODE_TELEMETRY_ENABLED=false` (record
its pid); seed a project and two idle threads (`target/title-snooze/seed.mjs` in this worktree
shows the RPCs); launch the app from the same environment, pair through the welcome's link
(paste-text), Continue, Continue, "Do not import projects". Read each step back with
`screencapture -x -o -l <window id>` (points = pixels ÷ 2).
1. Click the first thread, click its title, hover Snooze, click Custom…: the dialog opens with the
   ring on "Date and time". Press Escape: the ring is on the title. Read the server's
   `orchestration_command_receipts`: no `thread.snooze`.
2. Press Return (the title is focused): the menu opens. Tab to Snooze, Return: the ring is on
   "In 1 hour". Tab ×5 to Custom…, Return: the dialog. Escape: the ring on the title.
3. Reopen by keyboard, Escape in the menu: the ring on the title.
4. From the dialog opened by the title, choose Duration (click), Snooze: exactly one
   `thread.snooze` for that thread, the thread stays open, "Snoozed 1 thread, ⌘Z to undo".
5. With a signed-in provider lane (`provider-lane/READY-<provider>`), start a turn in
   approval-required mode that asks to run a shell command, so the thread waits on the user. Open
   its title menu: Snooze is dimmed; hover it: no submenu; click it: nothing. (Untried route without
   a provider: a queued turn start inside its 2-minute grace window also disables Snooze,
   `sidebar-model.ts` `hasQueuedTurnStart`.)
Stop the server by its pid and quit the app copy.

## Progress

2026-10-08: implemented and verified with agent drives against base `07dcef1ab` and this branch on
one lane (above); the real-key rows wait for the real-input batch.

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 0 (earlier session, 2026-10-08) | uncommitted on `f90277989` (backup `9697b1dd1`) | route and submenu press worked in its drives; its `Host::boot` test failed three rounds (`UnknownSource("snapshot")`, engine not loaded, `markdown` owner thread) | its local `target/title-snooze/` (not published) | superseded |
| 1 | branch tree before review (runner digest `76a010a3…`) | runner passed (`attempt1-report.json`); new tests fail on the base sources (2 Bun, 1 Rust) and pass here; five checks green; agent drives before/after; review round 1 FAIL (B1) | images 01-05, `agent-drives.txt`, `review.md` | B1 |
| 2 | branch tree after the repairs (runner digest `53a6e426…`) | runner passed (`attempt2-report.json`: 68 focused tests, 3041 clone tests with 1 skip, tsc, contract build, 13 Rust lib tests, caps); five checks green again (3383 pass / 0 fail / 33 ignored, 94 binaries); retry drive on the rebuilt app | `agent-drives.txt` Retry, `attempt2-report.json` | real-input batch; disabled-row capture (no provider lane) |
| 3 | `93ca3386f` merged with `feat(example)/t3-code` at `d3df2c426` (runner digest `da1f63f0…`, 998 paths) | runner passed (`attempt3-report.json`: 68 focused tests, 3053 clone tests with 1 skip, tsc, contract build 3847 slots / 4150 actions, 13 Rust lib tests, caps); five checks green (3383 pass / 0 fail / 33 ignored, 94 binaries) | `attempt3-report.json` | real-input batch; disabled-row capture (no provider lane) |
| 4 | merged again with `feat(example)/t3-code` at `c0475fbaa` (#294, #295; runner digest `906feb7b…`, 998 paths) | runner passed (`attempt4-report.json`: 68 focused tests, 3054 clone tests with 1 skip, tsc, contract build 3862 slots / 4166 actions, 13 Rust lib tests, caps); five checks green (3383 pass / 0 fail / 33 ignored, 94 binaries) | `attempt4-report.json` | real-input batch; disabled-row capture (no provider lane) |

## Next action

Run the real-input batch steps above; then mark the keyboard row's real-key half verified.
