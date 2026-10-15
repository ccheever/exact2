---
name: 20261015-fullscreen-title-row
plan: 20261005-t3code-macos-parity
implementation: complete
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-fullscreen-title-row
pr_url: https://github.com/ccheever/exact2/pull/594
verified_commit: c3f3b793d
---

# The title row in real full screen

## Outcome

Goal: in real macOS full screen the title row (breadcrumb, sidebar toggle, panel toggles) stays at
the top of the window with no empty titlebar band over it, and without the traffic lights' inset, as
`AppSidebarLayout.tsx` drops `--workspace-controls-left` in full screen (reference pin
`1e2ecbd975`). The system titlebar may reveal at the top edge. Windowed stays as it is. Found in
realinput-1015 (`20261014-timeline-and-window-hooks.md`, step 32). Done: the real full-screen
check passed on 2026-10-15 (below); only the top-edge hover was not run (the tool cannot move the
pointer).

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
| Real input (`orca computer`) | PASS (second session, below); hover not run |

Real input, first session (2026-10-15 09:10): not run. The Mac's screen was locked
(`CGSSessionScreenIsLocked` true, front process `loginwindow`) for 30 minutes, and `orca computer`
refused every read (`permission_denied`). The lane was quit and the lock removed.

Real input, second session (2026-10-15 12:20, screen unlocked): realinput lock taken; a fresh lane
copy of this branch's bundle (code of `c3f3b793d`; `com.exact.t3code.macos.fsfix-1`, port 16917,
homes copied from the ri1015 lane) launched through LaunchServices; real keys and pointer through
`orca computer`, targeted by pid. Evidence is local in `target/fullscreen-fix/` (`ev/`, `pr-images/`).

| Real-input row | Result | Evidence (`ev/`) |
| --- | --- | --- |
| Windowed title row | PASS: traffic lights, sidebar toggle, breadcrumb and panel toggles drawn; window 36,38 1280×840; the AX tree has the window's `toolbar` | `b-windowed` |
| Enter real full screen (real ⌃⌘F) | PASS: window 0,30 1352×848; `list-windows` shows one window (realinput-1015 saw a second, 52-pt titlebar window) | `c-ctrlcmdf`, `d-fs` |
| Title row at the top, no band | PASS: breadcrumb, sidebar toggle and panel toggles at the top edge, sidebar toggle without the traffic lights' inset | `d-fs` |
| No toolbar in full screen (the review's note on `ToolbarMac.attach`) | PASS: the full-screen AX tree has no `toolbar` element | `d-fs.tree.txt` |
| Click a title-row toggle in full screen | PASS: a real pointer click on the sidebar toggle hid the sidebar (the tree lost "Search threads"); a second click brought it back. Seen: an AX element press on "Toggle thread details panel" changed nothing (still expanded), not looked into | `h-after-sidebar`, `j-sidebar-back`, `f-after-click` |
| Hover at the top edge | NOT RUN: `orca computer` has no pointer-move action | — |
| Exit full screen (real ⌃⌘F) | PASS: window 36,38 1280×840 again; the `toolbar` is back in the AX tree; the traffic-light crop (top 52 pt × 90 pt) is pixel-identical to the windowed capture before entry | `k-exit`, `l-windowed` |
| Light appearance | NOT RUN (dark only) | — |

Cleanup: the lane quit by AppleScript `quit`; no lane process stayed; port 16917 free; lock removed.

## Next action

Merged as [#594](https://github.com/ccheever/exact2/pull/594), feature commit `1dcd641a5` (squash). Open: the top-edge hover in full screen (needs a pointer-move tool) and why an
accessibility press on "Toggle thread details panel" did nothing in that session.
