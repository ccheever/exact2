---
name: 20261008-dialog-shortcut-focus
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-dialog-shortcut-focus
pr_url: null
verified_commit: null
---

# Dialog keyboard focus reaches invisible shortcut buttons

## Outcome

Keyboard navigation through a modal must not focus the app's zero-size shortcut dispatch
buttons. Preserve working keyboard shortcuts, Escape cancellation and focus return.

## Evidence and scope

Reported during PR #238's attended session on 2026-10-07/08:
[provider sign-in task](closed/20261005-provider-sign-in-and-install.md#attempts-and-evidence).
Tab from a dialog's last button reached `settings-shortcuts.contract`'s `keyboard-dispatch`
buttons. The original report describes this as app-wide and pre-existing. It has not been
reproduced again on `29dbc5dbf`; the root cause and framework/app ownership remain unconfirmed.
The provider dialog fix that makes Settings inert is already implemented and is distinct
from this remaining focus stop. Implemented 2026-10-08 on `feat(example)/t3-code-dialog-shortcut-focus`
(below).

Start with `settings-shortcuts.contract`, `keyboard-dispatch.ts`, `AppConfirm` in
`shell-panels.contract`, and the modal wiring in `app-settings.contract` / `app.contract`.
Keep the fix limited to focus eligibility and modal behavior. If a minimal reproduction
proves a framework limitation, record that evidence before classifying it as one.

## Reproduction and acceptance

1. Build the current macOS app with an isolated lane home. Open Settings > Providers and
   a fixture-backed Sign out or Remove confirmation; do not sign out a retained real account.
2. Use real Tab and Shift+Tab through the controls, including past the final button.
   Record focused elements and the accessibility tree. Repeat with another app dialog.
3. Verify no zero-size dispatcher or inert Settings control takes focus. Check that focus
   follows the modal's intended visible controls in both directions.
4. Escape must dismiss without sending the confirm RPC and return focus to the trigger.
   Return on Confirm must send exactly one intended operation.
5. With the dialog closed, verify representative keyboard shortcuts still work, including
   a configured shortcut and a shortcut while a text field is focused.

## Root cause

Reproduced on `da4f4512f` (agent drive 1, base): Tab from Add Environment's Close went to the
Nightly toast's buttons and then to `shortcut-settings.open` and `shortcut-navigation.back`
(`keyboard-dispatch`). Two causes, both in the clone:

- The dispatch buttons are ordinary 1×1 `button`s, and a 1×1 button is a sequential focus stop on
  the web (Chrome, checked on a scratch app: Tab visits a 1×1 button in a 0×0 `overflow: hidden`
  box) and therefore on every Exact host (macOS's key-view loop follows HTML's rule, LLP 1088 D7.3).
  `inert` and `aria-hidden` are not the issue; the framework behaves as the web does. The same holds
  for 19 other invisible chord and hatch buttons (composer keys, the palette's Escape, Usage's
  chords, Settings' "/", the sidebar's sweep Escape) and a surface tab's 0×0 rename-cancel anchor.
- The clone's dialogs are overlays without a focus trap. Where the page behind them is `inert`
  (Settings under a provider dialog), the loop wraps through whatever else is not inert: the
  dispatch buttons and a toast. Where it is not (main-window confirms, the sidebar's dialogs), Tab
  walks into the sidebar. Base UI traps the focus in every Dialog/AlertDialog, takes it at open
  (`initialFocus`) and gives it back at close (`finalFocus`).

Framework (checked on main `462308f9c`, nothing newer): exact2's only modal with host focus
containment is a `dialog` opened by an invoker button; `showModal(id)` from an action is carried by
the terminal host only (macOS logs "unknown command showModal", the web refuses it) and
`aria-modal` keeps no Tab inside (LLP 1080.003 §4) — [X51](../issues/20261008-x51-state-driven-modal-focus.md).
On macOS a date input, a time input and a `select` are no Tab stops at all (the web's are) —
[X50](../issues/20261008-x50-macos-form-controls-tab-order.md). The clone-side fix below is the
narrowest that matches the reference without them.

## What was built

- **No invisible Tab stops.** `tabindex=-1` on `ShortcutButton` and the queued-edit key
  (`settings-shortcuts.contract`) and on every other 1×1 chord or hatch button (composer,
  composer-editor, composer-controls, palette, pages-usage, settings-core, sidebar) and a surface
  tab's 0×0 rename-cancel anchor (`r4-surfaces.contract`). A negative
  `tabindex` leaves the chord, its menu item and an agent press as they were (`ShortcutsMac`
  matches `aria-keyshortcuts` on pressable buttons; checked on the scratch app with ⌘J on a
  `tabindex=-1` button while a dialog was open).
- **AppConfirm and SettingsConfirm** (every ConfirmDialogHost-style confirmation: thread title
  menu, terminal close, server update, provider Sign out/Remove, sidebar archive/unpin/delete,
  Settings' archive, project, theme and process confirms): Cancel has `autofocus`, and `key`
  handlers wrap Tab from Confirm to Cancel and Shift+Tab from Cancel to Confirm. AppConfirm's ids
  come from a `confirmId` per call site, so two mounted at once do not share them.
- **Custom snooze** (`SidebarSnoozeDialog`): Close moved after the footer (DialogPopup renders it
  last); the schedule type is ToggleGroup's roving stop (`tabindex` 0/-1, the first toggle until
  another takes the focus; ←/→ with wrap, Home, End); Shift+Tab from the toggle goes to Close and
  Tab from Close to the toggle; Cancel, Close and Escape give the focus back to the thread's row.
- **Add Environment** (Settings › Connections): Remote link takes the focus at open; Shift+Tab
  from it and Tab from Close wrap; closing returns the focus to "Add environment"
  (`manage-connection`) or the route's "Add route".
- **Focus at open and at close** where the opener keeps the focus: a root task
  (`sidebarDialogFocus`, `app.contract`) focuses a sidebar dialog's first stop however it opened
  (row menu, context menu, the title menu once routed there); the terminal's close confirm takes
  the focus from the terminal or from the close button that asked (`terminalCloseConfirm`, the
  dispatch's `closeTerminal`, `terminal.contract`'s `askClose`) and Cancel gives it back (existing); the title menu's confirms give it back to the title button
  (`confirmUi`); `dialogReturn` (snapshot, `sidebar-view.ts`) names a one-thread dialog's row.
- `app.contract` stays at 1,500 lines: adjacent `use` lines from one file are merged.

## Results

Lane (not committed): two builds on one fixture — base `da4f4512f` (`t3-code-evidence-base`) and this
branch — on `T3_LOCAL_HOME=target/lane/t3-home`, `T3_LOCAL_PORT=16610`, isolated `CODEX_HOME`,
`CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_TELEMETRY_ENABLED=false`; a lane project `work`; three threads
made with a version-only `claude` stand-in (`target/lane/bin/claude-stub`, so their turns fail; no
account used); the Antigravity runtime downloaded for real (316 MB) for its Remove confirmation.
Drives: `bun scripts/agent.mjs macos --json --size 1280x840` (agent mode: keys are delivered to the
window, the focus is read from the tree's `[focused]` and `tree --ax`). The transcript is
[agent-focus-drives.txt](../evidence/20261008-dialog-shortcut-focus/agent-focus-drives.txt); the
images are agent window screenshots with the `[focused]` element outlined (no AppKit ring is drawn in
agent screenshots, `docs/agent-pitfalls.md`).

| Row | Result | Proof |
| --- | --- | --- |
| 1. Settings › Providers confirmation (fixture-backed) | pass (agent): the Remove confirmation of a real downloaded runtime; no account involved | drive 2, image 02 |
| 2. Tab and Shift+Tab past the last control, focused elements and accessibility tree; another dialog | pass (agent) on six dialogs: Providers Remove, title-menu Delete, terminal close (⌘W and the trash button), Custom snooze, sidebar Delete, Add Environment; `tree --ax` marks `Cancel [focused]`. Real keys: deferred to the real-input batch — screen locked (user away) | drives 1–6, images 01–04 |
| 3. No dispatcher or inert Settings control takes focus; focus follows the visible controls both ways | pass (agent): every Tab and Shift+Tab stays in the dialog and wraps in the reference order (base reaches the toast, the sidebar and `shortcut-settings.open`/`shortcut-navigation.back`); Date, Time and Unit in Custom snooze are skipped by the host (X50) | drives 1–6 |
| 4. Escape sends nothing and returns focus to the trigger; Return on Confirm sends exactly one operation | pass (agent): Escape → trigger on all six (Remove button, title, terminal, row ×2, "Add environment"), no `thread.delete`/`thread.snooze` receipt and no `provider.install.remove` span from them; Return on Confirm → exactly one `thread.delete` receipt and exactly one `ws.rpc.provider.install.remove` span | drives 2–6, 8 |
| 5. Shortcuts with the dialog closed (a configured one, one with a text field focused), and with one open | pass (agent), identical on base and branch: ⌘K, ⌘1, ⌘, , composer + ⌘K; ⌘K over Custom snooze and over Add Environment; ⌘W reaches the hidden `shortcut-terminal.close` | drives 7, 9 |
| Tests | `dialog-focus.test.ts` (7) and the `dialogReturn` test in `sidebar.test.ts` fail on the base sources (all 8) and pass here; `bun test examples/t3-code` 2519 pass, 1 skip, 0 fail (base 2511, 1 skip) | runner report |
| Independent review | round 1 FAIL (B1: the Settings `inert` extension froze the page and lost focus return for the project and scheduled-task dialogs) → reverted; non-blocking 2–4 and 7 repaired, 5 recorded; round 2 below | [review.md](../evidence/20261008-dialog-shortcut-focus/review.md) |
| Real-keyboard before/after | deferred to the real-input batch — screen locked (user away) | steps below |

Not done or not verified, each with its blocker:
- Real-keyboard rows (2 with real keys, the focus ring itself): deferred to the real-input batch — screen
  locked (user away). Steps below.
- Date, Time and the Unit select in Custom snooze are no Tab stops on macOS: framework,
  [X50](../issues/20261008-x50-macos-form-controls-tab-order.md).
- Dialogs without their own trap no longer reach the dispatch buttons, but keep the rest of their
  base behavior: a showing toast is still a Tab stop from the provider wizard, the theme dialogs and
  Restore defaults (their page is inert), and the scheduled-task, project, SnapShot and
  connection-remove dialogs leave Settings interactive (making it inert under them lost the opener's
  focus and could freeze the page, review B1). AppConfirm opened by a composer image remove or a
  right-panel tab close keeps the focus on its trigger (no focus return exists for them). A sidebar
  dialog gives the focus back to its thread's row, not to whatever held it before (a ⌘-chord typed in
  the composer). All need the host's modal: framework, [X51](../issues/20261008-x51-state-driven-modal-focus.md);
  per-dialog traps are the stopgap used here.
- The dispatch buttons stay exposed to VoiceOver as 1×1 buttons (unchanged; a `tabindex` does not hide
  them, and the reference has no such elements). Recorded as a follow-up, not this task's focus path.

## Real-input batch steps

One session, under the real-input lock (owner note "dialog-shortcut-focus: real keys"). Builds: base
from `t3-code-evidence-base` (`da4f4512f`, take `.build-lock` around the build) and this branch, each
`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`; copy each bundle
to `T3 Code (Lane DSF base).app` / `T3 Code (Lane DSF after).app`, set `CFBundleName`,
`CFBundleDisplayName` and a lane bundle id (`com.exact.t3code.lane.dsf.{base,after}`), `codesign --force
--deep -s -`. Run one at a time from a shell with the lane environment of Results (a fresh lane home
needs the welcome once: Continue, Continue, "Do not import projects"; then add the project and threads
as below), and record the pid.

Fixture: a project at `target/lane/work` (git repo); threads "Snooze-me", "Delete-me" and "Reply OK"
(type in the composer, Send; the `claude` stand-in fails the turn); Antigravity added in Settings ›
Providers › Add provider and its runtime installed (Install, wait for "Installed.").

For each build (`screencapture -x -o -l <window id>` after each step; points = pixels ÷ 2):
1. ⌘, → click Connections → click "Add environment". Read: where the ring is at open (after: Remote link;
   base: none). Press Tab ×7 (after: ring cycles inside, ends on "Pairing code"; base: after Close the
   ring disappears — the toast's buttons, then the 1×1 Settings shortcut). Shift+Tab ×3. Escape → after:
   ring on "Add environment".
2. Providers › Antigravity: Tab to "Remove downloaded runtime" and press Return. Read the ring (after:
   Cancel). Tab ×3, Shift+Tab ×2 (after: Cancel ⇄ Confirm only; base: focus lost or the 1×1 buttons).
   Escape → after: ring on Remove; the runtime still reads "Installed.". Count
   `ws.rpc.provider.install.remove` in the lane's `server.trace.ndjson*`: unchanged.
3. Hover "Snooze-me" → its clock button → Custom… . Read the ring (after: "Date and time"). ←/→ move
   between the two toggles; Tab ×5 and Shift+Tab ×2 (after: toggle → Cancel → Snooze → Close → toggle;
   base: out to the toast and the sidebar). Escape → after: ring on the row; no `thread.snooze` receipt.
4. Open "Reply OK", click its title → Delete. Tab ×3, Shift+Tab, Escape → after: ring on the title; no
   `thread.delete` receipt.
5. Dialogs closed: ⌘K (palette opens), Escape; click the composer, ⌘K (palette); ⌘1 (first thread). With
   step 3's dialog open: ⌘K (palette opens over it).
Compose base/after pairs per step with `pair.py` (as images 01–04) and upload to
`t3-code-evidence/dialog-shortcut-focus/real-*.png`.

## Progress

2026-10-08: reproduced on `da4f4512f` with agent drives; scratch-app checks of the framework (1×1
buttons are Tab stops on the web and macOS, `tabindex=-1` keeps their chords, `showModal(id)` is not
carried on macOS or the web, date/time/select are no macOS Tab stops); fix, tests and drives; real keys
deferred (screen locked).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | branch tree before the review repairs | agent drives 1–9 (base and branch); review round 1 FAIL (B1) | `agent-focus-drives.txt` §1–9, `review.md` | B1 |
| 2 (2026-10-08) | branch tree after the repairs; runner fingerprint in `attempt1-report.json` (`source_unchanged: true`) | runner passed every check it runs: new tests 42 pass; `bun test examples/t3-code` 2519 pass / 1 skip / 0 fail; strict `tsc`; `contract build` (2631 slots, 2738 actions, 59420 nodes); `cargo test -p t3-code-macos --lib` 11 pass; caps. Five checks: build, test (3383 pass / 0 fail / 33 ignored, 94 binaries), clippy, fmt, caps, boot green. Drives §10 on the rebuilt app; review round 2 PASS | `agent-focus-drives.txt` §10, images 01–04, `attempt1-report.json`, `review.md` | real-input batch (screen locked); X50; X51 |

## Next action

Review the draft PR. The coordinator runs "Real-input batch steps" when the screen is unlocked.
