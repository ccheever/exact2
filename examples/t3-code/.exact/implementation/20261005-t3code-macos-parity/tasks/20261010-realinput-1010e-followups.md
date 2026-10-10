---
name: 20261010-realinput-1010e-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010e-followups
pr_url: https://github.com/ccheever/exact2/pull/406
verified_commit: null
---

# Findings of the real-input session realinput-1010e

## Outcome

The session `realinput-1010e` ran the real-input steps of #398 (realinput-1010c-native) and #399 (realinput-1010c-fixes)
on the bundle of `a1ade42f9`, plus #308's retry. Passed: RC-1 step 1, RC-2, RC-3, RC-4 (b)–(d), RC-6, RC-8. This task
takes what did not pass, and two new observations. Each row is compared with the reference first; a row that matches it
closes as such.

## Findings

| Id | From | Clone under real input | Evidence |
| --- | --- | --- | --- |
| RE-1 | #398 RC-1 step 3 | Not run: the reference shows its permission helper only when packaged (`app.isPackaged`), and the audit's reference runs from source. The open question is whether the reference's first click on its helper (System Settings front) only activates it (Electron's documented `acceptFirstMouse` default, false) or also clicks through. | [E1](https://raw.githubusercontent.com/ccheever/exact2/5113cc7d755271906d63b7efc8c8472eacbbd96d/realinput-1010e/E1-398-rc1-step3-reference.txt) |
| RE-2 | #398 RC-1 step 2 | A drag from the helper's row starts on the first press, but T3 Code becomes the front app only on release. After the next click on System Settings the helper closes and does not come back; the record expects it to stay docked (after step 1's click it did come back). | [notes](https://raw.githubusercontent.com/ccheever/exact2/d2a861e62ae350a3dffff37583c00397edf95c72/realinput-1010e/E3-398-notes.txt), [input log](https://raw.githubusercontent.com/ccheever/exact2/255928f3810222129fdb0f65f89944bf99d44aea/realinput-1010e/E3-398-rc1-r9-input.log), [after drag](https://raw.githubusercontent.com/ccheever/exact2/3c0dc690cb41698843e4f6317d39e542dce78d4e/realinput-1010e/E3-398-rc1-drag-after-redacted.png) |
| RE-3 | #399 RC-4 (a) | With the caret after "now" (position 20), a click on the skill chip moves the caret to 0, before the chip. After Escape closes the details no caret is painted, and typing gives `x$frontend-design now` (2 of 2). | [typed](https://raw.githubusercontent.com/ccheever/exact2/7a248636ea42d3fecaf35fdcfe546194628c52f8/realinput-1010e/E4-RC4a-2-FAIL-typed-x-lands-before-chip.png), [no caret](https://raw.githubusercontent.com/ccheever/exact2/88d59b512246d6a67f2d2ad00e26df0aceca3ce7/realinput-1010e/E4-RC4a-2-FAIL-no-caret-after-now.png), [input log](https://raw.githubusercontent.com/ccheever/exact2/dddf6ac3548562932718caa1adbe9b38e29bbfa0/realinput-1010e/E4-RC4-r9-input.log) |
| RE-4 | #399 RC-6 (new) | After Cancel by mouse in the browser import wizard, the focus goes nowhere instead of back to Add profile. | [notes](https://raw.githubusercontent.com/ccheever/exact2/a18ab23126a27ce4804478282e641fb8734d3f02/realinput-1010e/E4-399-notes.txt) |
| RE-5 | #308 | PR #168's Code tab in the GitHub lane answered "The server returned HTTP 503." in three sessions (1010d, 1010e), so the gutter drag never ran. | [E2](https://raw.githubusercontent.com/ccheever/exact2/addf3a13effcf856f8ce10eec7fbd867fff1e6f2/realinput-1010e/E2-308-503.png) |
| RE-6 | #399 RC-5 | On a fresh Usage view the unpriced popover stays open on slow steps, a quick move and rests across the gap, but entering from the (i)'s left edge closes it as soon as the pointer leaves the (i). Twice earlier, a bad state: any 1–2 pt move on the (i) closed it for about 250 ms, and later hover stopped opening it until a click and Escape. Not reproduced on three tries. The session pressed ⌥⌘T for a trace, which is not the binding, so no trace was written. | [bad state](https://raw.githubusercontent.com/ccheever/exact2/5eeb9e3df58a0cada2dc728cd6f24f03438d06f1/realinput-1010e/E4-RC5-bad-state-flicker-on-2pt-move-within-i.png), [closes on first step](https://raw.githubusercontent.com/ccheever/exact2/0457d3dfcc1099f87db9802703ca17a711eb602c/realinput-1010e/E4-RC5a-bad-state-closes-first-step.png) |

## Steps

- RE-1: settle the reference's first click without packaging all of T3 Code if a smaller probe answers it. Option A: a
  minimal Electron app on the reference's own Electron binary (`target/t3-ref/src-1e2ecbd975`'s `node_modules/electron`)
  that shows a `BrowserWindow` the way the reference's helper does (`showInactive`, the same options) beside another
  front app, and logs whether a single click reaches the page. Option B: run the reference packaged (its app folder in a
  copy of `Electron.app/Contents/Resources/app`, so `app.isPackaged` is true) in an audit lane. The real click itself
  is a real-input row; write its exact steps. If the reference's first click only activates, return `acceptsFirstMouse`
  false on the clone's helper row and close button (match the original; the user's rule since #398).
- RE-2: read the reference helper's show and hide rules (the docking condition, `!frontmost && !isFocused`, and what a
  drag does) and compare them with the clone's after a drag session ends. Fix where the clone differs. Real-input steps.
- RE-3: compare with the reference's composer (a click on a skill chip: where the caret goes, and after its popover
  closes). Fix the clone's caret on chip click and after Escape. A unit or agent test where agent input reaches it.
- RE-4: compare the reference's wizard (a Base UI Dialog returns the focus to its trigger on close). Fix if different.
- RE-5: find which request answers 503 (the lane server's log) and whether the reference answers the same in the same
  lane config. A lane setup problem is fixed in the lane's setup notes for the next session; a clone bug is fixed here.
- RE-6: read the hover code for the bad state (timers, the ~250 ms close, a stuck hover flag) and the left-edge entry.
  Fix what the code shows. The next session's trace step uses `kill -USR1 <pid>` or the development build's ⌥⇧T
  (CLAUDE.md), not ⌥⌘T.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RE-1..RE-6 | reference comparison first; a unit or AppKit test of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Cause and fix

- RE-1: no change; the clone matches the reference, and #398's declared difference is withdrawn. A probe settled it
  without packaging and without real input (option A of the steps): the reference's own Electron 44.4.2 runs a
  `BrowserWindow` with `MacPermissionHelper.ts:100-117`'s options and a page shaped like `helperHtml`, shown with
  `showInactive()`, and an N-API addon asks AppKit inside that process what a click on the "T3 Code" row would ask:
  `[hitView acceptsFirstMouse:]`. Answer: true (the page view, `RenderWidgetHostViewCocoa`, at window level 3), and false
  for the same window at the normal level. Electron's `acceptFirstMouse` stays false, but Chromium accepts every press in
  a window above the normal level (`render_widget_host_view_cocoa.mm` `acceptsMouseEventsOption`: `kAlways`, "Always-on-top
  windows ... accepts all mouse events even if the window or the application is inactive"), and the helper is
  `alwaysOnTop`. So the reference's first click on the row activates T3 Code and reaches the page (a click reveals, a press
  drags), as the clone's row and close button do (`acceptsFirstMouse` true). The comments and the AppKit check say so now.
- RE-2: the reference's show and hide rules are the clone's (`syncPosition`: hide unless Settings is frontmost or the
  helper is focused; on blur and on each changed reading; close when Settings closes or the grant lands). The first-press
  drag matches too (RE-1). One difference: the reference polls Settings in its own process (one `osascript`), so a drag
  from the helper or an open menu never holds a reading back; the clone's watcher timer ran in the default run-loop mode
  only, so AppKit's event-tracking loop (a drag, a menu) paused it. It runs in the common modes now. What made the
  helper stay away in realinput-1010e is not visible from the code (its r9 log shows a second press on the row released in
  the list with the panel no longer key); with `R9_INPUT_LOG` set the clone now writes each Settings reading, show, hide
  (and why), finish (and why), the panel's key changes and the drag's start and end to that log (`<time> helper ...`), so
  the next session can tell a hidden helper from a closed one.
- RE-3: in the reference the chip is a selectable atom: a press makes a ProseMirror NodeSelection of it, the popover takes
  the focus, Escape gives the focus to the chip's button with the chip still selected (no caret), and typing replaces the
  chip: `$frontend-design now`, the caret after "now", the chip, Escape, "x" gives "x now" (CDP). (STATUS expected the caret
  to stay after "now"; the reference does not do that.) The clone let the press reach the text view, which put a caret at
  the chip's edge (position 0) and "x" went in front of it. Now `T3ComposerChipPress` takes a press on a skill chip:
  it gives the chip's editor the focus and selects the chip's whole source, and the text view does not see the press (a
  composition still open is left to the text view). The text view's highlight covers the pill painted below the text,
  so an overlay above the text paints the chip that is the selection again, tinted (the reference's selected chip,
  Highlight at 30%).
- RE-4: the reference returns the focus to Add profile when the wizard is cancelled by the pointer (CDP: `BUTTON "Add
  profile"`, no ring); the clone left it nowhere. `app.contract` `importWizardFocus`: when the wizard's open state goes from
  true to false, `focus("browser-profiles-add")` (the button has that id now), so every way out (Cancel, Escape, Close,
  the X) ends there.
- RE-5: a clone bug, not the lane. In realinput-1010d (lane `ri1010d-5`; 1010e's lane is gone, same setup) the lane copy
  ran its embedded server (This machine, focused, no GitHub project) and was paired with the GitHub lane's server, where
  #168 is listed. The detail and activity went to the lane server (`T3Client.rpc`'s router), but the Code tab's diff
  (`POST /api/pull-requests/diff`) and its Viewed read went to the focused embedded server, which answered
  `PullRequestUnavailableError` (provider-unsupported) with HTTP 503. The lane server answers the same diff with 200. The
  reference sends the diff to the pull request's environment (`pullRequests.ts`: an environment query over that
  environment's prepared connection) and routes `filesViewed`, `setFilesViewed` and `diffFileContents`; paired the same way
  over CDP it shows #168's files, its diff answered by the lane server. Now `CodeContext` carries the listing environment
  (the panel's selection); `prDiff` goes to that server's transport (`environmentCall`), `filesViewed` and
  `diffFileContents` through the pull request router from it (shared reads, as before: `environmentRequest` `share`), and
  `setFilesViewed` is routed from it as the reference's write is (review of #406; it went unrouted to the focused server
  before this task, then to the listing server's transport): `PullRequestRouter.dispatch` takes the identity probes and
  the reference's candidate order, a server that cannot take it before it leaves passing it to the next, and its `send`
  puts the write on the chosen server detached (composer-replies.ts, usage-replies.ts), so no answer waits on it, as
  before; the readers' `filesViewedOnly` invalidations (the reference's `finish`) run after the host accepted it, in the
  panel's next answer (`afterWrite`), before the read that follows. No server that can take it now: the batch waits for
  the next flush. One declared difference (EXACT2-GAPS, X19 #124): a refusal that comes back in the reply (an alternate's
  guard that saw another account after its probe) fails the batch where the reference passes it to the next server. The
  503's text: `T3RemoteAuth.failureMessage` reads a pull request error's tag as the
  RPC path does, so a 503 says "Change requests cannot be browsed for this project's host yet." (or the CLI's requirement)
  instead of "The server returned HTTP 503.". #308's gutter drag can be retried.
- RE-6: no clone cause found; nothing changed in the hover. The left-edge entry matches the reference: over CDP the
  reference also closes as the pointer leaves the (i) sideways (70 ms and 450 ms detours) and does not reopen in the popup;
  1-2 px moves on the (i) keep it open there. The clone's Contract machine (`hoverTipAt`, `enterSeg`/`enterPop`) closes a
  shown popover only on a leave that no enter of the trigger or the card follows within 50 ms, an outside press, Escape or a
  surface change, and reopens after the 300 ms open delay, which is the ~250 ms seen; a small move inside the (i) can close
  it only if the host delivers a leave without an enter, which is the macOS host's hover (X62, main issue
  `20261009-macos-subtree-hover.md`, #322, open main PR #327). The trace step the record named does not work on macOS: only
  the Linux host handles `SIGUSR1` (`kill -USR1` ends a macOS app), and the clone's menu bar removed the host's Develop menu
  and its Save Trace (⌥⌘T), so the session's ⌥⌘T reached the view as a key. A development build keeps Save Trace now, as a
  hidden View item with ⌥⌘T (`R8KeysMenus.arrange`), so the next session can take the trace.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RE-1 | pass by reference comparison, no change: the reference's first click on its always-on-top helper reaches the page (probe inside its Electron: `acceptsFirstMouse:` true at level 3, false at the normal level), as the clone's does; the #398 declared difference is withdrawn. AppKit snapshot check updated. The real click on the probe is real-input step 1 (confirmation only) | [RE1-reference-first-click.txt](https://raw.githubusercontent.com/ccheever/exact2/c1e15cb1c2cd14c7469548fb7f300f464d0fa1b0/realinput-1010e-followups/RE1-reference-first-click.txt), [probe main.js](https://raw.githubusercontent.com/ccheever/exact2/a3ac6ae326ebf85d486cb2a0a583080377beac35/realinput-1010e-followups/re1-probe-main.js.txt), [probe.m](https://raw.githubusercontent.com/ccheever/exact2/037a4cf07a3e2fc1acb8ce32d0a266c170116db2/realinput-1010e-followups/re1-probe.m.txt) |
| RE-2 | implemented, not verified (real input): the watcher keeps polling during a drag (AppKit check fails with the default-mode timer, passes now); the helper's transitions go to `R9_INPUT_LOG`. Open until step 2 runs | [RE2-helper-after-drag.txt](https://raw.githubusercontent.com/ccheever/exact2/4374fe3f137af2e8be4d2ba94a20a3d5e2a4799d/realinput-1010e-followups/RE2-helper-after-drag.txt) |
| RE-3 | pass (AppKit test, agent drive): before, the press left the caret and "x" did not replace the chip (`$frontend-design nowx` in the drive); after, the press selects the chip and Escape then "x" gives `x now`, as the reference. The selected chip is painted above the text view's highlight (AppKit render; the live drives ran before that commit, see the attempts). Real keys: step 3 | [RE3 image](https://raw.githubusercontent.com/ccheever/exact2/4095f2b9848f6a1dae2313a2345dc46e9b830b7c/realinput-1010e-followups/RE3-typed-x.png), [RE3-chip-press.txt](https://raw.githubusercontent.com/ccheever/exact2/085ecad0823a4e99b30f2a9fdb110a39e75b6d32/realinput-1010e-followups/RE3-chip-press.txt), [selected chip, AppKit render](https://raw.githubusercontent.com/ccheever/exact2/9aa5eac2965249628d887d1972b5175ce9608f02/realinput-1010e-followups/RE3-selected-chip-appkit.png) |
| RE-4 | pass (agent drive, reference): after Cancel by the pointer the focus is Add profile (node 2172), before it was none, as the reference's `BUTTON "Add profile"`. Real pointer and ring: step 4 | [RE4-wizard-focus.txt](https://raw.githubusercontent.com/ccheever/exact2/13d05b19133c9ab11bf2a49fc8fb544556e7fc17/realinput-1010e-followups/RE4-wizard-focus.txt) |
| RE-5 | pass (agent drive with the embedded server focused and the GitHub lane paired, reference over CDP the same way): before "The server returned HTTP 503." (the embedded server's 503), after #168's files from the lane server, as the reference. Review of #406: the Viewed write routed as the reference's (tests: the feature tip's sources 1 pass / 5 fail, the PR's previous head 5 / 1, now all pass; not driven live). #308's gutter drag and a Viewed tick: step 5 | [RE5-review-viewed-write.txt](https://raw.githubusercontent.com/ccheever/exact2/4ce356226d75eb2188e5996bb81690c1a2f81d5e/realinput-1010e-followups/RE5-review-viewed-write.txt), [RE5 image](https://raw.githubusercontent.com/ccheever/exact2/1b97747eaa6402a2c0e0209977bab0f7feed826d/realinput-1010e-followups/RE5-code-tab.png), [RE5-code-tab-503.txt](https://raw.githubusercontent.com/ccheever/exact2/6d92fa93ad475ad0b5d7800024a46b6f4677a2e0/realinput-1010e-followups/RE5-code-tab-503.txt) |
| RE-6 | no clone cause; the left-edge entry matches the reference (CDP); the bad state needs the host's hover events: step 6 takes a trace with ⌥⌘T (kept in development builds; AppKit check). Open until step 6 runs | [RE6-unpriced-hover.txt](https://raw.githubusercontent.com/ccheever/exact2/bafd3cf43a5d19492c4ce8f91176fa7f87ff28e4/realinput-1010e-followups/RE6-unpriced-hover.txt) |

## Real-input batch steps

Launch this branch's bundle normally as a lane copy (`open -n --env …`, its own bundle id), with the lane home outside any
git checkout (RC-7) and `R9_INPUT_LOG=$L/logs/r9-input.log`; read the front app with
`lsappinfo info -only name "$(lsappinfo front)"`. A=`/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit`.

1. **RE-1, the reference's first click (confirmation).** The probe needs no packaging:
   `env -i HOME=$A/lanes/realinput-1010e-followups/home USER=$USER TMPDIR=$TMPDIR PATH=/usr/bin:/bin "$A/ref/T3 Code (Alpha).app/Contents/MacOS/Electron" $A/lanes/realinput-1010e-followups/re1-probe --hold 40000 > $L/logs/re1-probe.log 2>&1`
   (record its pid). A 560×140 helper-shaped panel appears at the bottom right, always on top, the probe not active. Click
   System Settings, then click the panel's "T3 Code" row once. Expected in the log, from that one click: `app
   did-become-active`, `page mousedown on #app`, `page click on #app` (the reference reveals on its first click, as the
   clone does). Click System Settings again, then press on the row, drag a short way and back, release: note the order of
   `page dragstart` / `startDrag returned` and `app did-become-active` (the reference's side of RE-2's "front only on
   release"). The probe quits by itself after the hold.
2. **RE-2, the helper after a drag.** Settings › SnapShots › Set up › Allow (Screen Recording): the helper docks. (a) Click
   the row once: Finder front, the bundle selected, the helper hidden; click System Settings: the helper is back (passed in
   1010e). (b) Press on the row, drag a short way into the list and back onto the helper, release (do not drop into the
   list). Click System Settings' sidebar (outside the helper). Expected: within about a second the helper is docked again
   (the reference hides it on that click and shows it again with the next reading; so does the clone). Attach the r9 log:
   its `helper` lines give each reading, `hide: <why>`, `show at …` and `finish: <why>`; if the helper stays away, the last
   ones say whether it was hidden (and which reading never came) or closed (and why).
3. **RE-3, the chip.** In the composer type `$frontend-design now`, click after "now", then click the "Frontend Design"
   chip: the details open and the chip is selected (tinted, no caret). Escape: the details close, the chip stays selected.
   Type `x`: the prompt reads `x now` with the caret after x (the reference: `x now`).
4. **RE-4, the wizard's Cancel.** Settings › Integrations › Browser profiles: click Add profile, click Chrome; in the wizard
   click Cancel: Accessibility Inspector's focused element is "Add profile" (no ring after a click). Then by keys: Add
   profile focused, ↓ ↓ to Chrome, Return, Escape: the wizard closes and the ring is on Add profile.
5. **RE-5 and #308's gutter drag.** The lane copy with its embedded server and the GitHub lane's server paired (Settings ›
   Connections › Add environment, a fresh `bun lane.mjs pair primary` link): Pull Requests › #168 › Code: the files load
   (no "The server returned HTTP 503."). Tick one file's Viewed box: the count goes up; after about a second press the
   panel's Refresh: the box stays ticked (the write reached GitHub through the listing server). Untick it again so the
   lane's state is unchanged. Then #308's step 2 (the gutter drag;
   `tasks/closed/20261005-pr-code-tab.md`).
6. **RE-6, the trace.** Usage › Cost › 30 days, the unpriced (i): repeat 1010e's moves; when the bad state shows (a 1-2 pt
   move closes it, or hover stops opening it), press ⌥⌘T at once (development build; the hidden View › Save Trace): an
   alert names the trace file. Read it with `bun scripts/agent.mjs trace <file>` and keep it with the session's evidence
   (each `hover in/out view N`: the (i) and the popup's hover box). Never `kill -USR1` the app: macOS ends it.

## Tests

- `realinput-1010e-followups.test.ts` (new): RE-4's focus return in `app.contract` and the Add profile id; RE-5 through the
  panel's resource, a focused server that refuses and a paired one that holds the project (before: the tab ends in
  `error`; after: the diff and the Viewed read on the paired server, nothing on the focused one). Added in the review of
  #406: a hidden range's contents read through the paired server; a Viewed tick sent detached to it (`usage-reply:` key,
  read again once the reply landed); a tick held while it is not connected (`requeue`), then sent to it; and, with GitHub
  shared "read and act" on both and a remote listing server, the tick written through the local server with the same
  account (`expectedAccountId`), the readers told only after the reply. With the feature tip's sources 1 pass / 5 fail;
  with the PR's previous head 5 / 1 (the write went unrouted); mutation checks in the evidence text.
- `pages-pr-routing.test.ts` "a routed write sent detached (dispatch, afterWrite)" (review of #406): `dispatch` picks the
  server the awaited write picks, with the guard, and `afterWrite` sends the invalidations the awaited write sends; a
  server that cannot take it before it leaves passes it to the next; with nothing to route to, the refusal is the caller's.
- AppKit `macos/tests/composer` `chippress.swift` `testAPressSelectsTheChipSoTypingAfterEscapeReplacesIt` (5 failures with
  the tip's `T3ComposerChipPress.swift`, 57 tests pass now).
- AppKit `macos/tests/snapshot` `permission-helper.swift`: a reading reported while the run loop runs in `.eventTracking`
  (fails with the default-mode timer), and the first click reaching the row and the close button as the reference's
  always-on-top page (58 checks).
- AppKit `macos/tests/transport` `remote-scopes.swift`: a pull request error's 503 and 502 bodies give the reference's
  sentences (62 tests).
- AppKit `macos/tests/r8-keys`: the host's Save Trace stays a hidden ⌥⌘T in View and the key performs it (6 tests).

Checks on the code head `ab9a40699` (the branch merged with `feat(example)/t3-code` `efa8059f6`; all exit 0): `bun test
examples/t3-code` 4375 pass / 1 skip / 0 fail (298 files); strict tsc; `contract build` of `app.contract` (1353 lines);
`cargo test -p t3-code-macos --lib` 17 pass; AppKit composer 57, snapshot (58 permission helper checks), transport 62,
r8-keys 6, menus 46, all 0 failures; caps; the five checks (cargo test 3679 pass, 0 fail, 34 ignored). The bundle was built
for the live drives (`427c80fb1`, `bf86e902a`).

Checks after the review fixes, on the code head `4823a79dc` (TypeScript only; `feat(example)/t3-code` unchanged since
`8d69a4329`, all exit 0): `bun test examples/t3-code --timeout 60000` 4381 pass / 1 skip / 0 fail (298 files); strict tsc;
`contract build` of `app.contract` (1353 lines, unchanged); caps; the five checks (cargo test 3679 pass, 0 fail, 34
ignored). No Rust or Swift changed, so `cargo test -p t3-code-macos` and the AppKit binaries were not run again; no bundle
or live drive (the task's one drive and its retry were used).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975` on lane `realinput-1010e-followups` (backend 16640, CDP 16641) | RE-3 chip (CDP), RE-4 wizard Cancel, RE-5 paired with the GitHub lane, RE-6 hover moves; RE-1 probe in its Electron | the texts above |
| Before drive | feature tip `4c13b440f`, built in worktree `t3-code-realinput-1010e-before` (the shared evidence base `c03d7e908` predates #398, #399 and #400) | RE-3 `$frontend-design nowx`, RE-4 focus none, RE-5 HTTP 503 | images and texts |
| After drive 1 | this branch (`427c80fb1`) | RE-3 `x now`, but the pressed chip showed as a grey box: the text view's highlight covers the pill painted below the text; RE-4 Add profile; RE-5 files | [the grey box](https://raw.githubusercontent.com/ccheever/exact2/b3b8effd6accf2e9e188d5b5e7341e7e45d99de6/realinput-1010e-followups/RE3-after-drive1-chip-under-highlight.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/43084892f7dcd2e3eb7fb07577a28b0623f6c136/realinput-1010e-followups/drive-record.txt) |
| After drive 2 (the retry) | `bf86e902a` (a clear highlight while a chip is the selection) | RE-3 `x now`, RE-4 Add profile (node 2172), RE-5 files: the images and texts above. The grey box stayed: an inactive window's highlight (agent mode) ignores `selectedTextAttributes` | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/43084892f7dcd2e3eb7fb07577a28b0623f6c136/realinput-1010e-followups/drive-record.txt), [drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/c553a8468910e723484408d938838327a8688aa4/realinput-1010e-followups/drive.sh.txt) |
| AppKit render | `33cfc9071` (an overlay above the text paints the selected chip) | the pressed chip shows, tinted; not driven live again (one retry) | [selected chip, AppKit render](https://raw.githubusercontent.com/ccheever/exact2/9aa5eac2965249628d887d1972b5175ce9608f02/realinput-1010e-followups/RE3-selected-chip-appkit.png) |
| Review fixes (round 1) | `4823a79dc` (the Viewed write routed: `dispatch`, `afterWrite`; RE-5 tests) | tests: the feature tip's sources 1 pass / 5 fail, the PR's previous head 5 / 1, now all pass; mutation checks fail the matching tests; not driven live | [RE5-review-viewed-write.txt](https://raw.githubusercontent.com/ccheever/exact2/4ce356226d75eb2188e5996bb81690c1a2f81d5e/realinput-1010e-followups/RE5-review-viewed-write.txt) |

## Not done / not verified

- RE-1 (confirmation), RE-2, RE-3, RE-4, RE-5's #308 drag and RE-6 under real input: open until the batch steps above run
  (agent mode cannot be the key window, move the real pointer, drag into System Settings or show a focus ring).
- The routed Viewed write (review of #406) was not driven live: the task's one drive and its retry ran before it. The
  RE-5 lane's default sharing ("off") sends it to the listing server's transport, as the live drive's build did; real-input
  step 5 now ticks a file there.
- **Follow-up for the coordinator to file** (an observation, not a finding of this record; no task, queue line or gap
  tracks it yet): the import wizard's initial focus. When the wizard opens on Configure, the reference focuses its first
  tile ("Personal / 5 cookies", CDP in RE4-wizard-focus.txt): `BrowserImportWizard.tsx:145-146` is a Base UI `Dialog` /
  `DialogPopup` with no `initialFocus` and no `autoFocus`, so the first tabbable element of the popup takes it, and the
  close X comes after the step (`ui/dialog.tsx` `DialogPopup`). The clone focuses Import (`browser-profiles.contract`
  `BiButton(buttonId="browser-import-run", … first=true)`, `autofocus=first`). Its other steps choose a button too
  (`first=true`: "I’ve quit it", Cancel on Full Disk Access, Done, Close), and `autofocus` applies at each step, where Base
  UI focuses only when the dialog opens; each step needs the reference compared (CDP) before it changes.

## Next action

Coordinator: review draft PR [#406](https://github.com/ccheever/exact2/pull/406) (review fixes on it: the Viewed write
routed, RE-5's tests, RE-4's node); file the wizard initial-focus follow-up above; the real-input batch steps 1-6 close
RE-1 to RE-6, then #308's step 2.
