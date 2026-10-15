---
name: 20261015-fullscreen-title-row
plan: 20261005-t3code-macos-parity
implementation: complete
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-fullscreen-title-row
pr_url: null
verified_commit: null
---

# The title row in real full screen

## Outcome

Goal: in real macOS full screen the title row (breadcrumb, sidebar toggle, panel toggles) stays at
the top of the window with no empty titlebar band over it, and without the traffic lights' inset, as
`AppSidebarLayout.tsx` drops `--workspace-controls-left` in full screen (reference pin
`1e2ecbd975`). The system titlebar may reveal at the top edge. Windowed stays as it is. Found in
realinput-1015 (`20261014-timeline-and-window-hooks.md`, step 32). The change is in and its
AppKit test passes; the real full-screen check did not run (the Mac's screen was locked).

## Cause

`modules/apple/T3WindowChrome.swift:21` (base `bdc7f8683`) puts the empty 52-pt unified
`NSToolbar` of `titleRow` (`:36-46`) on the window and keeps it there in every mode. Windowed,
the toolbar is transparent over the full-size content and only centers the traffic lights. In
full screen AppKit draws the toolbar as an opaque bar in its own titlebar window
(realinput-1015: "0,30 1352x848 plus a 52-pt titlebar window"). That bar sat over the top 52 pt of
the content, which is the title row. The content did not move down: the Contract's title row
was under the bar and still in the accessibility tree. The display mode was not the cause: the
host reads it from the window's style mask (`DisplayPreferences.displayMode`), and PR #584 only
replaced the fact; the deleted `T3FullScreen.swift` never changed the chrome.

## Change

- `modules/apple/T3WindowChrome.swift`: `attach(_:)` takes the window set-up out of
  `install(_:)`. On `willEnterFullScreen` the toolbar leaves the window. On `didExitFullScreen`
  the same toolbar comes back, then the traffic lights move to their inset again. A window that is
  already full screen when the chrome attaches gets no toolbar.
- `macos/tests/window-chrome/main.swift` (new AppKit binary, README recipe): posts the window's
  full-screen notifications and checks that the toolbar leaves and comes back, that the traffic
  lights keep their inset, that `destroy()` in full screen puts nothing back, and that a second
  attach keeps one toolbar. On the base chrome (with an `attach` shim) the full-screen case fails.

Framework gap (not fixed here): the app has no `NSWindowDelegate` hook, so it cannot use
`window(_:willUseFullScreenPresentationOptions:)` (`.autoHideToolbar`) or hear a failed entry
(AppKit posts no notification for `windowDidFailToEnterFullScreen`). After a failed entry the
title row has no toolbar until the next full-screen exit.

## Verification

| Check | Result |
| --- | --- |
| `window-chrome` AppKit binary | 3/3 pass; base chrome: 1 of 3 fails |
| Affected AppKit binaries (`composer`, `menus`, `r8-pointer`, which also compile the chrome) | composer 78/78, menus 47/47, r8-pointer 2/2 |
| Full app Bun suite (336 tracked `.test.ts` files) | 5239 pass, 1 skip, 0 fail |
| Strict `tsc` on `app.ts` | pass |
| `contract build examples/t3-code/app.contract` | pass (warnings only) |
| `cargo test -p t3-code-macos --lib --no-fail-fast` | 49 pass |
| `caps.mjs`, `cargo fmt --check`, `cargo check --workspace --all-targets --keep-going` | pass |
| Bundle `EXACT_APP_DIR=… build.mjs t3-code-macos --bundle` | built |
| Independent review of the diff | no blocking defects |
| Real input (`orca computer`) | NOT RUN: see below |

Real input, 2026-10-15: the realinput lock was taken, and a lane copy of this bundle
(`com.exact.t3code.macos.fsfix-1`, port 16917, homes copied from the ri1015 lane) launched
through LaunchServices. `orca computer` refused every read (`permission_denied`: no accessibility
window). The session's screen was locked (`CGSSessionScreenIsLocked` true; the front process was
`loginwindow`) and stayed locked for 30 minutes. The lane was quit, its server exited, the port
was free and the lock was removed. One `screencapture` of the lane window (windowed, inactive) shows
the title row as before (local `target/fullscreen-fix/pr-images/02-after-windowed-title-row-fix-build.png`).

| Real-input row | Result |
| --- | --- |
| Windowed title row | Pixels only (screen capture): breadcrumb, sidebar toggle and panel toggles drawn; no input |
| Enter real full screen: title row at the top, no band | NOT RUN (screen locked) |
| Breadcrumb or toggle clicked in full screen | NOT RUN |
| Hover at the top edge | NOT RUN |
| Exit full screen: windowed title row again | NOT RUN |

## Next action

Run the one real-input check with the Mac unlocked: lane copy of this branch's bundle, enter
full screen by View › Enter Full Screen, check the title row at the top with no band, click the
sidebar toggle and read back its state, hover the top edge, exit, and check the windowed title row.
Also look in that drive for a host-added window toolbar in full screen (the review's note:
`ToolbarMac.attach` accepts a window with no toolbar). If it passes, set `verification: verified`
and remove the "Real full screen" row from `STATUS.md`.
