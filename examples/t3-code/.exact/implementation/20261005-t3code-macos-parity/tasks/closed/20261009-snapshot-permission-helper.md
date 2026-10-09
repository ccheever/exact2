---
name: 20261009-snapshot-permission-helper
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-snapshot-permission-helper
pr_url: https://github.com/ccheever/exact2/pull/359
verified_commit: 4c88d6a850572a5d7be3e83cb962050774506488
---

# The macOS permission helper window beside System Settings

## Outcome

When the app opens System Settings › Privacy for Screen Recording or Accessibility, it shows the reference's small helper
panel docked under the System Settings window. The panel says "Set up <permission>", shows "↑ Drag T3 Code into the list
above" with a T3 Code button you can drag (a click reveals the app in Finder), and has a close button. It follows the
Settings window and closes when the grant is detected.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)), from source (the screen was
locked). Reference: T3 Code `1e2ecbd975`. Clone: `c603c22d6`.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PG-9 | A packaged macOS build: after it opens Privacy › Screen Recording or Accessibility (SnapShot setup, `requestPermissions`, `openSystemSettings`), a "Set up <permission>" panel docks under the System Settings window: "↑ Drag T3 Code into the list above", a draggable T3 Code button (a click reveals it in Finder) and a close button. It tracks the Settings window and closes when the grant is detected. | Only opens the `x-apple.systempreferences` URL: no helper panel, no drag source, no grant polling (`modules/apple/T3SnapShot.swift:274-300`). | Settings › SnapShots › enable without Screen Recording granted; watch beside System Settings. | `target/t3-ref/src-1e2ecbd975/apps/desktop/src/permissions/MacPermissionHelper.ts` (source comparison) |

## Scope and exclusions

Included: the helper panel, its drag source and Finder reveal, docking to the System Settings window, and closing on the
grant, for Screen Recording and Accessibility.

Excluded:
- Changing the user's real privacy grants. A row that needs a grant or a revoke is attended: the user does it in person.
- Framework code. The auditor's reading: an AppKit `NSPanel` in the clone's native module needs no framework support.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/desktop/src`): `permissions/MacPermissionHelper.ts`,
`permissions/MacSettingsWindow.ts`, `permissions/MacPermissions.ts:21-40`; `snapShot/DesktopSnapShot.ts:1305-1325, 1399`;
`ipc/methods/window.ts:314-330`.

Clone (`examples/t3-code`): `modules/apple/T3SnapShot.swift:274-300` (opens the System Settings URL). Keep the
permission messages, which already match ("… then restart T3 Code.").

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state). The agent's
`screenshot … window` captures only the app window, so this task's pairs are real screen captures taken while the
screen is unlocked.

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PG-9 | Enable SnapShots without Screen Recording: System Settings opens on Screen Recording and the helper docks under it with the drag button and close. A click on the button reveals the app in Finder. The close button closes it. | `pg9-helper-docked.png` | needs_real_input (System Settings and a real screen) |
| PG-9 | Grant detection: after the user grants the permission, the helper closes by itself. Same for Accessibility. | `pg9-helper-closes.png` | needs_real_input, attended (the user grants and later revokes in person) |
| PG-9 | Unit or XCTest: the panel's placement follows a given Settings window frame. | text: test output | XCTest |

### Results (draft PR [#359](https://github.com/ccheever/exact2/pull/359), head `109c5a78c`)

| Row | Result | Proof |
| --- | --- | --- |
| PG-9 docked helper | Built; the real-screen pair is open (real-input batch, steps 1-2). The AppKit render of the panel matches the reference's `helperHtml` rendered in Chromium (light, dark, hover). The AppKit test covers docking, following, hiding when covered or unreadable, reveal, ×, Escape, Settings closing and one helper at a time | [panel image](https://raw.githubusercontent.com/ccheever/exact2/b6cc3c64fd12f227975f7532e5541237bc4a19d5/snapshot-permission-helper/pg9-helper-panel.png), [tests](https://raw.githubusercontent.com/ccheever/exact2/b75ed9979ddfc7aaa2d4b4d4a4513f45fd2e745d/snapshot-permission-helper/pg9-tests.txt) |
| PG-9 grant closes it | Built; the attended grant is open (real-input batch, steps 3-4). The AppKit test covers a grant seen by the 1 s poll: it closes the helper and returns to the owner window | [tests](https://raw.githubusercontent.com/ccheever/exact2/b75ed9979ddfc7aaa2d4b4d4a4513f45fd2e745d/snapshot-permission-helper/pg9-tests.txt) |
| PG-9 placement XCTest | Pass: the reference test numbers ((367,100,723,719) → (599,663,475,140); (-800,200,…) → (-568,763,…); the content-column bounds on both displays) and the docked AppKit frame | [tests](https://raw.githubusercontent.com/ccheever/exact2/b75ed9979ddfc7aaa2d4b4d4a4513f45fd2e745d/snapshot-permission-helper/pg9-tests.txt) |
| Live agent drive (no regression) | Pass: Settings › SnapShots › Set up snapshots is unchanged before and after. Agent mode never prompts, opens Settings or docks a helper, and the agent build holds both grants on this Mac | [pair](https://raw.githubusercontent.com/ccheever/exact2/48ee01d5a855a10e4c6d35a43f1477fe796d6d31/snapshot-permission-helper/pg9-setup-unchanged.png) |

### Review round (2026-10-10, code head `ba427ae29`)

An independent review found two should-fix problems. Both are fixed, in one round.

| Problem | Fix | Proof |
| --- | --- | --- |
| After a drag, AppKit sends the row no mouseUp, so `pressed`/`dragging` stayed set: a refused or elsewhere drop left the closed hand over the row and no arrow outside it. Also unverified: `hovering` stayed true when the panel was ordered out under the pointer (Finder reveal) | `T3PermissionHelperAppRow` implements `draggingSession(_:endedAt:operation:)`. It calls `dragEnded(at:)`, which releases the press and sets the open hand over the row or the arrow outside it, like the reference's `#app:active` ending. `T3PermissionPanel.orderOut`/`orderFrontRegardless` call `T3PermissionHelperView.syncHover()`, which reads the hover from the pointer (it is never on while the panel is hidden). AppKit test: 5 new checks (hidden under the pointer drops ×; back under it shows ×; back elsewhere hides it; a press grabs; a drag end releases the press and reveals nothing) | [tests](https://raw.githubusercontent.com/ccheever/exact2/4e7c35637bb4edae7091c9089a0d8a209b483c4d/snapshot-permission-helper/pg9-review-tests.txt) |
| No automated test covered, outside agent mode, which helper setup and `snapshotRequestPermissions` choose | `T3SnapShot` now reaches the grant probes, prompts, Settings opener and focused window through `T3SnapshotPermissionSystem`, and the helper through `showHelper` (defaults: the real system and `T3PermissionHelper`). The new `macos/tests/snapshot/permission-requests.swift` (16 checks, fake grants, no TCC) checks the reference `requestPermissions`/`setup` and their tests: Screen Recording before Accessibility; the Accessibility pane only with app text on and Screen Recording granted; nothing with the grants present; each Allow docks its own helper, which polls its own grant. A copy without `include &&` fails the test | [tests](https://raw.githubusercontent.com/ccheever/exact2/4e7c35637bb4edae7091c9089a0d8a209b483c4d/snapshot-permission-helper/pg9-review-tests.txt) |
| Live agent drive (no regression) | The bundle boots and Settings › SnapShots renders. The "Enable snapshots" tap landed on the lane's "Nightly needs the beta mobile app" toast, which covers the switch, so the setup dialog was not reopened (the retry was spent). Agent mode's setup guard did not change, and the AppKit test still covers it | [checks](https://raw.githubusercontent.com/ccheever/exact2/abbaa767cc04c2fd1dbe603b3ca8c302124c6182/snapshot-permission-helper/checks-review.txt) |

### Built

- `modules/apple/T3PermissionHelper.swift`: `T3PermissionHelper` (reference `MacPermissionHelper`). The panel is `T3PermissionPanel`, a borderless, transparent, floating, non-activating `NSPanel`. The card is the reference page: header, the T3 Code row as an `NSDraggingSource` with the bundle's file URL (copy or link only), and × shown on hover or keyboard focus. Also here: `T3SettingsWindow` (the JXA poll's window choice, `settingsHelperBounds`), `T3SettingsWindowWatcher` (0.5 s while Settings is frontmost, 1 s otherwise, changes only) and `appBundle` (`macAppBundlePath`).
  The row releases its press when a drag ends (`dragEnded(at:)`), and the panel reads its hover from the pointer when it is ordered out or in (`syncHover`).
- `T3SnapShot.swift`: Allow Screen Recording and Allow Accessibility show the helper. The new `snapshotRequestPermissions` op (reference `requestPermissions`) does nothing in agent mode. Grants, prompts, the Settings opener and the helper are reached through the `permissions` (`T3SnapshotPermissionSystem`) and `showHelper` seams.
- `client-ops-snapshot.ts`: Continue requests permissions after the test capture (`enableForSetup`). Include app text on, while SnapShots is on, requests Accessibility first (`saveIncludeAccessibility`).
- Not declared differences (no framework limit): the panel does not activate T3 Code; the helper also shows in development bundles, which are real app bundles with their own privacy identity (the reference's development executable is generic Electron).

## Tests

- AppKit `macos/tests/snapshot/permission-helper.swift`: 53 checks (48, plus 5 from the review round: hover after ordering out or in, and the drag end). `permission-requests.swift`: 16 checks, which helper setup and `snapshotRequestPermissions` dock outside agent mode, with fake grants. `feedback.swift` adds 1: isolated `snapshotRequestPermissions` never prompts. `T3_PERMISSION_HELPER_EVIDENCE=<dir>` renders the panel.
- Bun `snapshot-settings.test.ts`: Continue's order is test capture, then request permissions, then configure; a refused test does not request; Include app text requests only when turned on with SnapShots on.
- Final head `109c5a78c`, all exit 0: `bun test examples/t3-code` (3,649 pass, 1 skip), strict tsc, contract build (5,736 slots), `cargo test -p t3-code-macos --lib` (13), AppKit snapshot, caps, and the five checks (cargo test 3,521 passed, 0 failed, 34 ignored) ([checks](https://raw.githubusercontent.com/ccheever/exact2/5e301c22cb61782f644c374cdd7544a19a8d7227/snapshot-permission-helper/checks.txt)). `app.contract` is unchanged at 1,230 lines.
- Review round, code head `ba427ae29` (only Swift and the AppKit tests changed), all exit 0: AppKit snapshot, `cargo test -p t3-code-macos --lib` (13), caps, the bundle build, and the five checks (cargo test 3,521 passed, 0 failed, 34 ignored). The Bun tests, tsc and contract build were not re-run, because none of the files they read changed ([checks](https://raw.githubusercontent.com/ccheever/exact2/abbaa767cc04c2fd1dbe603b3ca8c302124c6182/snapshot-permission-helper/checks-review.txt)).

## Real-input batch steps

Screen unlocked, the user present (every step needs a missing grant: the user revokes it in person and grants it again
at the end). The agent build already holds both grants on this Mac, so first remove them for "T3 Code (Exact)" in System
Settings › Privacy & Security › Screen Recording and › Accessibility (select it, "−"), by hand.

Build and launch the branch outside agent mode (agent mode refuses every prompt, so it never docks a helper), on the
lane's home and port:

```sh
cd /Users/daehyeonmun/orca/workspaces/exact2/t3-code-snapshot-permission-helper
EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle
A=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit; L=$A/lanes/snapshot-permission-helper
T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16942 T3CODE_TELEMETRY_ENABLED=false \
  T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 \
  "target/clients/b1c5badc07a0869b05c49b3e/com.exact.t3code.macos/macos/T3 Code (Exact).app/Contents/MacOS/T3 Code (Exact)" &
```

(the client folder is the one the build prints as "local client"). For the before side, the same launch from
`/Users/daehyeonmun/orca/workspaces/exact2/t3-code-evidence-base` (its `target/clients/0d02a3550eba6edce4b12b59/…`).
Take each capture with `screencapture -x <png>` (whole screen) and compose before | after.

1. **Docked helper (`pg9-helper-docked.png`).** Settings › SnapShots › turn on "Enable snapshots"; in "Set up snapshots",
   Screen Recording › Allow. Read back: macOS may show its own Screen Recording prompt first (answer "Open System
   Settings"); System Settings opens on Privacy › Screen Recording, and within a second a panel docks inside its
   content column, 16 pt above the bottom edge: "↑ Drag T3 Code into the list above" and a "T3 Code" row with the app
   icon. Capture. Before (base build): only System Settings opens, nothing docks.
2. **Follows, hides, reveals, closes.** Drag the System Settings window by its title bar: the panel follows within
   0.5 s. Click Finder in the Dock: the panel hides; click System Settings: it shows again. Move the pointer over the
   panel: × shows at its top right. Click the "T3 Code" row: Finder opens with "T3 Code (Exact).app" selected (the
   panel hides while Finder is front). Move the pointer away from where the panel was, click System Settings: the panel
   is back with × hidden. Drag the "T3 Code" row about 40 pt and release it on the panel's own header (a refused
   drop: the drag image slides back, nothing is added or revealed). Move the pointer over the row: the open hand
   (not the closed hand). Move it off the panel: the arrow. Click × : the panel closes and T3 Code comes to the
   front. Allow again, click the panel once, press Escape: it closes. Allow again and quit System Settings (⌘Q): the
   panel closes and T3 Code comes to the front.
3. **Drag into the list, grant detected (`pg9-helper-closes.png`, attended).** Allow again; drag the "T3 Code" row
   onto the Screen Recording list. Read back: T3 Code (Exact) is added (macOS may ask for the user's password and offer
   "Quit & Reopen": choose "Later"); within a second the helper closes by itself and T3 Code comes to the front.
   Capture before and after the drop. If macOS does not report the new grant until relaunch, the helper stays until
   Settings closes; record that (the reference reads the same API).
4. **Accessibility.** Back in Set up snapshots, Accessibility › Allow: the macOS Accessibility prompt shows; System
   Settings opens on Privacy › Accessibility with the "Set up Accessibility" helper docked. Drag the row into the
   list, switch it on: the helper closes within a second. The same helper appears when "Include app text" is turned
   on while SnapShots is on and Accessibility is missing.
5. Leave both grants as the user wants them; quit the lane app (⌘Q) and kill only the pid started above.

## Next action

Coordinator: review draft PR [#359](https://github.com/ccheever/exact2/pull/359) (the review round's two should-fix problems are fixed at `ba427ae29`), then run the real-input batch steps above with the user present. Step 2 now also checks the hover after a Finder reveal and the cursor after a refused drag. They close the docked-helper and grant rows with `pg9-helper-docked.png` and `pg9-helper-closes.png`.

## Delivery

Merged by the coordinator on 2026-10-10 as `4c88d6a85` (#359, squash) after an independent review and its repair round. Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
