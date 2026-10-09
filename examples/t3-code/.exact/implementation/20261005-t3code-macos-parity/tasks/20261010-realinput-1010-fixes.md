---
name: 20261010-realinput-1010-fixes
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010-fixes
pr_url: https://github.com/ccheever/exact2/pull/373
verified_commit: null
---

# Two bugs the 2026-10-10 real-input batch found in merged audit fixes

## Outcome

The real-input batch `realinput-1010` (normal launches of lane copies, real keys and pointer) passed #361's and #357's
rows and most of #355's and #359's. Two steps failed. This task fixes both, as the reference does.

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| RI-1 (#355, PA-2) | On "Open a surface", a letter opens its surface (B, T, F, D, P, L, M), also after the panel was closed and reopened. | Real arrows and Return work, but after closing the Files tab and pressing ⌘⌥B again, a real F does nothing (`f`, `F`, a clean repeat). The launcher still had the keys: ↓ moved its highlight right after. | "Audit work thread"; ⌘⌥B; ↓×3 Return (Files opens); close the Files tab; ⌘⌥B; F. | [r1-pa2-letter-f.png](https://raw.githubusercontent.com/ccheever/exact2/ae3a0188f58b0c0983e0ffd0dc63aae82c684325/realinput-1010/r1-pa2-letter-f.png) |
| RI-2 (#359, PG-9) | A click on the helper's "T3 Code" row reveals the app in Finder, and Finder comes to the front (the helper then hides, as it does whenever System Settings is covered). | The click opens Finder's window with the app selected, but Finder stays behind System Settings and the helper does not hide (two clicks). | SnapShots › Set up › Allow (Screen Recording); click the helper's "T3 Code" row. | [r4-pg9-step2-row-click-reveal.png](https://raw.githubusercontent.com/ccheever/exact2/7ddcff1e7ff21eb8c634f1225254ce733388f2a4/realinput-1010/r4-pg9-step2-row-click-reveal.png), [readback](https://raw.githubusercontent.com/ccheever/exact2/f175842b4a97dca827ac518775eca74bff5a1b3e/realinput-1010/r4-pg9-step2-readback.txt) |
| RI-3 (#359, PG-9, check) | The reference's helper closes when its grant poll sees the grant; the setup row reads the same API. | In the attended run the user saw the Screen Recording helper close and T3 Code come front, while the setup row kept "Allow" until the end (the macOS relaunch case). Check that the helper's grant poll and the setup row use the same check, as the reference does; change nothing if the reference behaves the same. | Attended only: revoke, Allow, drag the row into Screen Recording; read the helper and the row. | [screen-recording read-back](https://raw.githubusercontent.com/ccheever/exact2/447a52027fee82452706d39f1568c8611bdc1938/realinput-1010/r5-pg9-attended-screen-recording.png) |

## Context and guidance

- RI-1: `R8KeysLauncher.consume` and the launcher's `ui("key", k)` (`shell-panels.contract`, `r8-keys*.ts`,
  `#355`'s focus-on-mount). The arrows reach the launcher after the reopen; check how a letter is consumed versus an
  arrow after a reopen (a stale focused-node id or a consumed-once flag). Reference: `RightPanelTabs.tsx` `handleKeyDown`.
- RI-2: `modules/apple/T3PermissionHelper.swift` (the row's click: `NSWorkspace.activateFileViewerSelecting`). The panel
  is non-activating; Finder may need an explicit activation (`NSRunningApplication` for `com.apple.finder`
  `activate()`), as the reference's `shell.showItemInFolder` brings Finder front. Reference:
  `apps/desktop/src/permissions/MacPermissionHelper.ts`.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RI-1 | Bun/AppKit test of the reopen path; agent drive (⌘⌥B, close, ⌘⌥B, F opens Files); real F in the next batch | before / after image |
| RI-2 | AppKit test that the click activates Finder; real click in the next batch (Finder front, helper hidden) | text (front app read-back) |
| RI-3 | code comparison with `MacPermissionHelper.ts`; unit test of the shared check | text |

## Cause and fix

- RI-1: the launcher's `key` handlers run only from Exact's own session monitor (`ExactViewMac` `shortcutMonitor` →
  `Presenter.routeKey`); a view's `keyDown` runs none. The composer's monitor (`T3Composer.handle`, ChatView's
  type-to-focus) took a launcher letter with `R8KeysLauncher.consume`, sent it to the launcher view's `keyDown` and
  returned nil. AppKit calls a window's local monitors in no fixed order (a two-monitor probe ran them [A, B] in 5 of 8
  launches and [B, A] in 3; the order also changes as monitors come and go), so whenever the composer's monitor ran
  first the letter never reached Exact's route: arrows still worked (the composer's monitor lets them pass), F did
  nothing. Agent keys skip the local monitors, which is why agent drives passed. Now `R8KeysLauncher.route` never
  consumes the letter: with the launcher focused the event goes on, unredirected, to Exact's route (`.pass`); with the
  focus elsewhere (not a typing context) the launcher takes the focus and the key is posted again at the head of the
  queue (`.taken`, once per key). The launcher's `key` action prevents and stops a letter of an available row, either
  case (`ShellSurface.letter`), as the reference's capture listener does (`RightPanelTabs.tsx:408-420`), so AppKit
  sees no unhandled key. Review round 1: `route` recognises its own re-posted copy before anything else; if the focus
  left the launcher before the copy came back (a rare race), the copy is dropped (`.dropped`, `T3Composer.editorKey`
  returns `Routed(event: nil)`) instead of falling through to type-to-focus, which typed the letter into the composer.
- RI-2: the row click ran only `NSWorkspace.activateFileViewerSelecting`. The helper's panel never activates T3 Code,
  and that reveal left Finder behind the active System Settings. In the reference the helper is an ordinary focusable
  window (`MacPermissionHelper.ts:100-117`): the click makes T3 Code the active app, and `shell.showItemInFolder`
  (`:145,148`) is then a reveal asked by the active app. `T3FinderReveal` now does the same in order: the click
  activates T3 Code (the same call the helper's finish already uses), T3 Code yields activation to Finder, asks for the
  reveal, and asks Finder to activate. Nothing waits for Finder's window. Showing the panel and dragging from it still
  never activate T3 Code. The helper's unchanged tracking hides it once Settings is covered. (The first version, without
  the self-activation, asked from an app that was not active; under macOS 14+ cooperative activation such requests may
  be ignored, so review round 1 adopted the reference's order before the batch.)
- RI-3: no behavior change. The reference's helper poll and setup row make the same calls
  (`MacPermissionHelper.ts:13-19` and `DesktopSnapShot.ts:334-339`: `getMediaAccessStatus("screen")`,
  `isTrustedAccessibilityClient(false)`), and so did the clone's (`CGPreflightScreenCaptureAccess()`,
  `AXIsProcessTrusted()` in `T3SnapshotPermissionSystem` for the poll and inline in `snapshotState` for the row).
  `snapshotState` and `configure` now read the same `permissions` value the helper polls, so a test holds them together.
  The attended run's "helper closed, row still Allow" is explained in [ri3-same-check.txt](https://raw.githubusercontent.com/ccheever/exact2/0cb3a0de8ef345673f70d248f7e9604350f12f5c/realinput-1010-fixes/ri3-same-check.txt):
  the row and the poll read one report, and the next helper (Accessibility) docks only from the Accessibility Allow,
  whose `show()` closes the open helper first.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RI-1 | pass by AppKit test and agent drive; real F open (next batch). The order test fails on the feature-branch tip in 3 of 4 cases (F with the composer's monitor first; D from the bare window in either order) and passes here. Agent drive on this branch: Timeline verification, Toggle right panel, F (Files), close Files, Toggle right panel, F: Files opens, as the reference. Review round 1: the re-posted copy's race (focus gone before it returns) typed the letter into the composer on `3be705a83`'s launcher; now dropped (test fails before, passes after). | [ri1-reposted-copy-race.txt](https://raw.githubusercontent.com/ccheever/exact2/89574e7af02504a385747fc8a5ac899b34c37d76/realinput-1010-fixes/ri1-reposted-copy-race.txt), [ri1-reopen-letter-f.png](https://raw.githubusercontent.com/ccheever/exact2/23638d7f46d19f1893f4936dc8ba1063f19af044/realinput-1010-fixes/ri1-reopen-letter-f.png), [ri1-appkit-order-test.txt](https://raw.githubusercontent.com/ccheever/exact2/425f24742171944931cbff9ba880a0194ae74433/realinput-1010-fixes/ri1-appkit-order-test.txt), [drive-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/76d9ce332bb683e8560ef097d9d77b39a606130f/realinput-1010-fixes/drive-ops.txt) |
| RI-2 | implemented, not verified. The AppKit test checks only the call order on fakes (activate T3 Code, yield to Finder, reveal, activate Finder); it does not show Finder coming front. Open until real-input batch step 2 (Finder front, helper hidden). | [ri2-finder-reveal-v2.txt](https://raw.githubusercontent.com/ccheever/exact2/268d4d722bdacd2418cc6343f2e7235654225c7a/realinput-1010-fixes/ri2-finder-reveal-v2.txt), [ri2-finder-reveal.txt](https://raw.githubusercontent.com/ccheever/exact2/00ab5660c23986919c539911ab56d5d3589d95f7/realinput-1010-fixes/ri2-finder-reveal.txt) (first version) |
| RI-3 | pass by code comparison: the reference and the clone each read one check for the poll and the row; nothing changes in behavior. Unit test: fake grants flip the poll and the row together. | [ri3-same-check.txt](https://raw.githubusercontent.com/ccheever/exact2/0cb3a0de8ef345673f70d248f7e9604350f12f5c/realinput-1010-fixes/ri3-same-check.txt) |

The evidence-base build (`950e8e2e5`) predates #355, so its launcher has no arrow keys; both clone drives open Files by
letter so the steps match.

## Real-input batch steps

Launch the lane copy normally (LaunchServices, its own TCC responsible process), as realinput-1010 did.

1. **RI-1, real letters after a reopen.** Select "Audit work thread". If the right panel shows tabs, close each (the
   panel closes with the last). Press ⌘⌥B: "Open a surface". Press ↓ three times, then Return: Files opens. Close the
   Files tab: the panel closes. Press ⌘⌥B, then F (no Shift): Files opens, with no system beep. Close it, ⌘⌥B,
   Shift-F: Files opens. Close it, ⌘⌥B, click once on empty transcript space (the focus leaves the launcher, not into
   a text field), press D: Diff opens. Close it, ⌘⌥B, click into the composer, type F: an "f" lands in the composer and
   no surface opens. Clear it. Then quit T3 Code (⌘Q), launch it again and repeat "⌘⌥B, F" once: Files opens (the
   monitors' order differs between launches).
2. **RI-2, the helper row's click.** Settings › SnapShots › Set up › Allow (Screen Recording): the helper docks in
   System Settings. Click the helper's "T3 Code" row once (no drag). Within about a second: the front app is Finder
   (`lsappinfo front`) with the app bundle selected, and the helper is not on screen (CGWindowList). Note whether
   T3 Code's main window came in front of System Settings (the reference leaves it behind; the click now activates
   T3 Code first, as the reference's click does, and a programmatic activation may bring the main window with it).
   Click System Settings: Settings is front and the helper is back at its docked place. If Finder stays behind, record
   the front app (T3 Code or System Settings) and the helper's state: the click already activates T3 Code before the
   reveal, so a failure here means the self-activation was refused or did not land before the reveal.
3. **RI-3.** No step (code comparison). Optional when the attended PG-9 run is repeated: note whether the Screen
   Recording helper disappears before or after the Accessibility row's Allow is pressed.

## Tests

- AppKit `macos/tests/r8-keys` (6 tests, 6 pass): `testTheLauncherTakesFocusAndItsLetters` now checks `route` (`.pass`
  with the launcher focused, nothing handed to the view's `keyDown`; `.taken` from the bare window with the key posted
  again once; `.dropped` for the copy when the focus left meanwhile, also into a text field; `.none` for an unlisted
  letter, a typing context, a chord, a hidden or removed launcher);
  `testALauncherLetterReachesExactsKeyRouteInEitherMonitorOrder` (new) presses F and D through the composer's real
  monitor beside a stand-in for Exact's route, in both orders, and the copy's race (focus gone before the copy returns:
  nothing reaches the composer).
- AppKit `macos/tests/snapshot`: the permission helper checks (54) gain the reveal's order (activate T3 Code, yield,
  select, activate Finder; order only);
  the permission request checks (20) gain the poll and the setup rows agreeing with fake grants.
- `shell.test.ts`: each surface's `letter` (`btfdplm`). `r4-surfaces.test.ts`: the launcher's letter branch
  (prevented, stopped, then `ui("key")`).

Checks: see the PR ("Checks").

## Review round 1 (2026-10-10)

An independent review found four should-fix items; all are addressed on this branch:

- RI-2 was marked pass on a call-order test of fakes: now "implemented, not verified" until batch step 2, and the click
  runs the reference's order (T3 Code active first, then the reveal), the fallback the batch step named.
- `T3FinderReveal`'s comment said Finder is activated "once its window is open"; nothing waits, and the comment says so.
- A re-posted launcher letter returning after the focus left the launcher fell through to type-to-focus; it is dropped.
- The README's AppKit inventory said "r8-keys 4" (now 6) and "snapshot 86 checks" (now 161).

## Next action

Run real-input batch steps 1 and 2; then the coordinator reviews and merges.
