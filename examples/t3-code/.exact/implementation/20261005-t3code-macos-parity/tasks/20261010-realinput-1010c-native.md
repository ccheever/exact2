---
name: 20261010-realinput-1010c-native
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010c-native
pr_url: https://github.com/ccheever/exact2/pull/398
verified_commit: null
---

# Failures of the attended real-input session of 2026-10-10 (realinput-1010c): native and app-level rows

## Outcome

The attended session (`realinput-1010c`, normal launches of lane copies, real keys and pointer, the user present for
sign-ins, notifications and the Korean input source) passed most rows. These steps failed under real input, although
their agent-mode and unit checks passed. This task fixes each as the reference does and writes exact real-input steps
for the next session. (#349's own failures are in its fix round, not here.)

## Findings

| Id | From | Reference (T3 Code `1e2ecbd975`) | Clone under real input | Evidence |
| --- | --- | --- | --- | --- |
| RC-1 | #373 RI-2 | A click on the permission helper's "T3 Code" row reveals the app in Finder and Finder comes to the front; the helper hides. | Finder opens a window with the bundle selected but stays behind System Settings; the helper stays docked; T3 Code does not come forward either (2 of 2, base build `ec6f9bf1c`). #373 added the reference's activation order; it is not enough. | [B2](https://raw.githubusercontent.com/ccheever/exact2/35cd1bf5e87832d50bc53de69f0a225ce3852ede/realinput-1010c/B2-ri2-helper-row-click.png) |
| RC-7 | every lane copy | (check) The reference raises no Documents-folder permission prompt at launch. | Lanes 5-9 and 11-13 raised "would like to access files in your Documents folder" at launch, with HOME set to the lane home, so something resolves the real `~/Documents` (the account's home, not `HOME`). Find the access; remove it if the reference does not make it. | [E-observation](https://raw.githubusercontent.com/ccheever/exact2/ce8a494e3bf39535de879b08948f805908ee14db/realinput-1010c/E-observation-documents-prompt.png) |
| RC-9 | B (check) | (check) T3 Code's Quit shortcut behavior by default (Settings › General › quit behavior). | One ⌘Q does not quit: the clone's default is Hold; two quick presses quit. Compare with the reference's default; change only if it differs. | [B1 ⌘Q](https://raw.githubusercontent.com/ccheever/exact2/ddf1b0150989510e1372d7459664a4066df89f42/realinput-1010c/B1-cmd-q-menu.png) |

## Scope and exclusions

Included: RC-1, RC-7 and RC-9 (RC-7 and RC-9 start as checks). The UI rows are in
[realinput-1010c-fixes](20261010-realinput-1010c-fixes.md).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RC-1 | an AppKit test of the activation path; exact real-input steps for the next session (stays open until it runs) | text (front app read-back) |
| RC-7 | find the access (log file opens or `fs_usage` on a lane launch); compare with the reference lane; remove it if the reference does not make it | text |
| RC-9 | reference comparison of the quit-behavior default; change only if it differs | text |

## Cause and fix

- RC-1: the helper was a non-activating `NSPanel` (`[.borderless, .nonactivatingPanel]`), so the window server never
  activated T3 Code on a click in it. #373's `T3FinderReveal` then asked to activate T3 Code itself (`NSApp.activate`)
  before yielding to Finder; under real input on macOS 26 that request from an inactive app was refused (T3 Code stayed
  inactive), so the yield and Finder's activation came from an inactive app and Finder's window opened behind System
  Settings, with Settings still frontmost and the helper still docked. The reference's helper is an ordinary focusable
  `BrowserWindow` (`MacPermissionHelper.ts:100-117`, shown with `showInactive()`): a click in it is a window-server
  activation, which no app refuses, and `shell.showItemInFolder` (`:145,148`) then runs from the active app. The panel
  is now `[.borderless]` (no `.nonactivatingPanel`), so a click in it activates T3 Code; showing it still never
  activates (`orderFrontRegardless`, the reference's `showInactive`). That the window server does activate T3 Code on
  the click is real-input batch step 1; agent mode cannot read it. `T3FinderReveal` asks to activate T3 Code only when
  it is not active (a VoiceOver press, which is no click), then yields to Finder, asks for the reveal and asks Finder
  to activate. A press on the panel now activates T3 Code too, as in the reference; the panel is then key, so `sync`
  keeps it shown while System Settings is not frontmost (the reference's `!frontmost && !isFocused`), and it hides
  once Finder or another app is front. This removes the previous record's undeclared difference "the panel does not
  activate T3 Code" (snapshot-permission-helper).
- RC-1, the first click (a declared difference, review round 1; Decision needed in #398): the reference's
  `BrowserWindow` never sets `acceptFirstMouse`, whose macOS default is false (Electron 44.4.2 `electron.d.ts:3824-3828`,
  "Whether clicking an inactive window will also click through to the web contents"). So in the reference the first
  click on the helper while System Settings is front only activates T3 Code and focuses the helper; the `click` or
  `dragstart` that sends `finder` or `drag` needs a second press. The clone's row and close button return
  `acceptsFirstMouse` true (unchanged since #359), so the activating click also reveals (or starts the drag, or
  closes). That is what RC-1 (and RI-2 before it) asks for, but it differs from the reference: one press here, two
  there. It is not an Exact limit, so it is not in `EXACT2-GAPS.md`. Batch step 3 checks the reference's first click,
  and the user decides whether the clone keeps one press or matches the reference (`acceptsFirstMouse` false on the
  row and the close button).
- RC-7: no change; the access is not the app's. The TCC log of the session attributes every Documents request of a
  lane copy to a child of the embedded T3 server, never to the app: the real `codex` CLI (26 requests), `claude` (6),
  Xcode's `git` (5). Reproduced from a shell with the pinned server started as the clone starts it in a lane (cwd = home
  = lane home) and codex run under a sandbox that denies `~/Documents`: codex's app-server (the server's provider probe,
  `cwd: process.cwd()`) reads `~/Documents/work/0.projects/exact2/.git/worktrees/<worktree>`. Every Orca worktree's
  `.git` file points there, and the lanes live inside the `t3-code` worktree (`target/t3-ui-parity/lanes/`), so a lane
  copy that finds a provider CLI walks up into that gitdir, and each lane copy's new bundle id gets its own prompt. The
  reference lane (`ref-app.sh`, the same sandboxed codex) makes the same access (`.git/worktrees/t3-code`); a lane home
  outside any repository makes none. The reference starts its backend in `HOME` when packaged
  (`DesktopEnvironment.ts:220`); the clone starts it in `NSHomeDirectory()`, which the lane launch pins to the same
  directory (`CFFIXED_USER_HOME`). For the next session: keep lane homes outside any checkout, or point each provider
  CLI's binary path at an absent file in the lane's `settings.json`.
- RC-9: no change; the clone matches the reference. `DEFAULT_QUIT_CONFIRMATION_MODE` is `"hold"`
  (`packages/contracts/src/settings.ts:228`) and `QuitHold.ts:169-171` accepts two presses in every mode; the clone's
  default is `'hold'` (`settings-core.ts:64`) and `T3QuitHold` has the same 1.2 s hold and 0.5 s double press.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RC-1 | implemented, not verified (real input). Before: the panel was `.nonactivatingPanel`, so a click left T3 Code inactive and its own activation request was refused (B2). After: the panel has no `.nonactivatingPanel`, so a click in it is expected to activate T3 Code; the row takes that activating click (`acceptsFirstMouse`, unchanged), and the reveal runs from the active app (yield, reveal, activate Finder). AppKit `snapshot` test: 57 permission helper checks pass. One press reveals here; the reference's first press only activates (Electron's default `acceptFirstMouse` false): a declared difference, Decision needed in #398. Open until real-input batch steps 1-3 run. | [rc1-activation-readback-v2.txt](https://raw.githubusercontent.com/ccheever/exact2/d8fe5cb5d4234b779eace8fa6642e5f9a602fe94/realinput-1010c-native/rc1-activation-readback-v2.txt), [rc1-appkit-snapshot-v2.txt](https://raw.githubusercontent.com/ccheever/exact2/39d82f66c6e6a40414827b56726f45812915021c/realinput-1010c-native/rc1-appkit-snapshot-v2.txt) |
| RC-7 | pass by check, no change: the accessor is codex (and claude, git) run by the T3 server in a lane home inside a git worktree whose gitdir is in `~/Documents`; the reference lane makes the same access; a lane outside any repository makes none. | [rc7-documents-access.txt](https://raw.githubusercontent.com/ccheever/exact2/55bf6a49ba06d3ccf74bfcafcfc22780c2b4e096/realinput-1010c-native/rc7-documents-access.txt) |
| RC-9 | pass by reference comparison, no change: both default to Hold, and Hold quits on a 1.2 s hold or two presses within 0.5 s. | [rc9-quit-default.txt](https://raw.githubusercontent.com/ccheever/exact2/93df05a1efd938c9ab5d81423ace87e4158fbc92/realinput-1010c-native/rc9-quit-default.txt) |

## Real-input batch steps

Launch a lane copy of this branch's bundle normally (LaunchServices, `open -n --env …`, its own bundle id, so it is its
own TCC responsible process), with the lane home outside any git checkout (RC-7: under `target/` of a worktree, provider
CLIs read the worktree's gitdir in `~/Documents` and raise a Documents prompt for the copy). Read the front app with
`lsappinfo info -only name "$(lsappinfo front)"`.

1. **RC-1, the helper row's click.** Settings › SnapShots › Set up › Allow (Screen Recording): System Settings opens on
   Privacy & Security › Screen Recording and the helper docks in its content column. Click the helper's "T3 Code" row
   once (no drag). Within about a second: the front app is Finder, a Finder window shows the lane app bundle selected,
   and the helper is not on screen. T3 Code's main window stays where it was (a click activation brings forward only the
   clicked panel). Click System Settings: Settings is front and the helper is back at its docked place. If Finder stays
   behind, record the front app right after the press (T3 Code means the click activated it; System Settings means it
   did not) and the helper's state. (One press reveals here; the reference's first press only activates: step 3.)
2. **RC-1, the drag after the change.** With the helper docked and System Settings front: press on the "T3 Code" row
   (T3 Code becomes the front app, as in the reference, and the drag starts on this first press, unlike the reference:
   step 3; the helper stays docked), drag a short way into the Screen
   Recording list and back onto the helper, release there: the drag image slides back, nothing is added or revealed,
   and the helper is still docked. Click System Settings: the helper stays docked. Whether to drop T3 Code into the list
   (a real grant) is the user's call; if dropped, the helper closes within a second once the grant is detected.
3. **RC-1, the reference's first click.** Start the reference lane (`A/ref-app.sh <lane> <base>`) and open the same
   setup there (SnapShots › Set up › Allow (Screen Recording); `DesktopSnapShot.ts:1311`): System Settings opens and
   the reference's helper docks in it. With System Settings front, click the helper's "T3 Code" row once (no drag).
   Record the front app right after, whether a Finder window with the bundle selected came up, and whether the helper is
   still docked; then click the row a second time and record the same. Expected from Electron's documented default
   (`acceptFirstMouse` false): the first click only makes the reference app front (no Finder; the helper stays docked
   and focused), and the second reveals and brings Finder front. If so, the clone's one-press reveal (step 1) is the
   declared difference under "Cause and fix" (Decision needed in #398); if the reference's first click already reveals,
   the two match and the difference is withdrawn. Stop it with `A/ref-stop.sh <lane>`.

## Tests

- AppKit `macos/tests/snapshot`: the permission helper checks (57) now check that the panel is an activating window (no
  `.nonactivatingPanel`; this fails on the base build's style mask), that the activating click reaches the row
  (`acceptsFirstMouse`, the declared difference from the reference's first click), that a mouse click on the row
  reveals, and the reveal's calls from a click-activated app (yield, select, activate Finder) and from an inactive one
  (activate T3 Code first). Public AppKit API only: review round 1 dropped a key-value read of AppKit's private
  `preventsActivation`, which would raise and stop the whole binary on an AppKit without it.

Checks (review round 1, code head `5aba0e1fb`: the fix merged with `feat(example)/t3-code` `7195f10b6`; all exit 0):
bun test 4355 pass / 1 skip / 0 fail (294 files); strict tsc; contract build (`app.contract` 1341 lines); `cargo test -p
t3-code-macos --lib` 17 pass; AppKit `snapshot` (57 permission helper checks,
[rc1-appkit-snapshot-v2.txt](https://raw.githubusercontent.com/ccheever/exact2/39d82f66c6e6a40414827b56726f45812915021c/realinput-1010c-native/rc1-appkit-snapshot-v2.txt));
caps; the five checks (cargo test 3675 pass, 0 fail). Round 1 changed only the test, comments and this record, so the
bundle build and the live drive were not repeated. First round (head `c326c090d`): the same checks and the bundle build
passed; live drive (agent mode, once): the app launches and connects
([live-main.png](https://raw.githubusercontent.com/ccheever/exact2/c7bb0e0a07a16e2a37cd9eacaea994ee82cf81ce/realinput-1010c-native/live-main.png)).

## Next action

Coordinator: review draft PR [#398](https://github.com/ccheever/exact2/pull/398); real-input batch steps 1-3 close RC-1 (front app
read-back; step 3 is the reference's first click). Decision needed (in #398): keep the one-press reveal from System
Settings that RC-1 asks for (a declared difference) or match the reference's documented first click, which only
activates (`acceptsFirstMouse` false on the row and the close button: two presses).
