---
name: 20261005-x27-window-chrome
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-desktop-shell-details]
upstream_url: https://github.com/ccheever/exact2/issues/113
reproduced_on: null
---

# X27: Title-row height and traffic-light inset, frame restore after the final style, full-screen state fact

## Summary

T3 Code's desktop window has a hidden-inset title bar with its traffic lights placed to fit a 52-point title row, comes back at the size and place it was left, and drops the space reserved for the traffic lights when it goes full screen. exact2's `viewport-fit=cover` gives full-size content but no title-row or traffic-light setting, restores the frame before the final window style, and gives the page no full-screen fact. The clone covers the first two with its own Swift (an empty toolbar and its own frame record) and cannot do the third from the page. What is needed is declarative window chrome, a frame restore that runs after the final style, and a full-screen fact in the page.

## Why this issue arose

### The T3 Code behavior

- **Title bar.** macOS windows use `titleBarStyle: "hiddenInset"` with `trafficLightPosition` `x: 16` and `y: 52 / 2 − 7 = 19` (`apps/desktop/src/window/DesktopWindow.ts:37–46` constants, `:256–265`). The web layout is 52 CSS px tall at the top (`--workspace-topbar-height`, `apps/web/src/index.css`). The position is recomputed with the page zoom (`DesktopWindow.ts:45–46`). Other platforms use a title-bar overlay with its own colors.
- **Space for the traffic lights.** The page reserves `var(--desktop-window-controls-inset, 90px)` at the left of the title row (`apps/web/src/components/AppSidebarLayout.tsx:60,259`). The preload script sets `--desktop-window-controls-inset` to `90 / zoom` px because native buttons do not scale with page zoom (`apps/desktop/src/preload.ts:30–47`).
- **Full screen.** The page asks the desktop bridge for the full-screen state and subscribes to changes (`getWindowFullscreenState`, `onWindowFullscreenStateChange`; `packages/contracts/src/ipc.ts:1241`; `AppSidebarLayout.tsx:262–281`). In full screen the inset is dropped, because the traffic lights are not in the window; it comes back on exit. Without this the sidebar header keeps a 90 px empty gap in full screen.
- **Frame.** The window's bounds are saved on resize, move, maximize and unmaximize (`DesktopWindow.ts:670–678`) and on close (`:678`). A full-screen, maximized or minimized window saves its normal bounds (`readPersistableBounds`, `:430–440`); bounds are normalized (`DesktopAppSettings.normalizeMainWindowBounds`) and the maximized flag is restored at reveal (`:818–830`). Tests: `window/DesktopWindow.test.ts`, `electron/ElectronWindow.test.ts`.

### What exact2 does today

From `EXACT2-GAPS.md` (written by earlier sessions from framework source at `c1522fdac`, checked against `main` `d2cb661eb`; citations as given there): table row X27 "Window chrome: title-row height and traffic-light inset; frame restore after final style"; and "`viewport-fit=cover` has no title-row or traffic-light setting (`ExactMac/main.swift:209-223`); the frame autosave is restored before the final style (`:310-313`)."

Bundled library (`20261005-platforms-v3`): window lifecycle and resize are verification priorities on macOS (`platforms.md`); window chrome options and a full-screen fact are **not covered: unknown**. Whether the page can read a full-screen state some other way (for example a `display-mode` media feature) is to confirm at `issue-open`. What the clone's lane drives showed: the page facts the agent reports are `online`, `root-font-size`, `visibility-state` and `can-share` (receipt in `target/t3-ui-parity/lanes/r12-threads/drive-receipts.ndjson`); no full-screen fact appears there.

### Where the clone hits it

- `modules/apple/T3WindowChrome.swift` (header and `install`): the 52-point title row uses the window's real traffic lights. It attaches an empty unified `NSToolbar`, which makes AppKit center the buttons at y = 26 (equal to the reference's 19 + 7), sets a transparent titlebar and a hidden title. Contract's `viewport-fit=cover` supplies the full-size content.
- `modules/apple/R8PointerWindowFrame.swift` (`R8WindowFrame`, defaults key `t3.window.frame`): header text says the host restores its frame autosave and only then turns on full-size content; AppKit keeps the content rect when that style is inserted, "so every launch lost the 32 pt titlebar (818 → 786 → 754, the top edge moving down)". The app therefore keeps its own frame record and puts it back once the window is in its final style. A saved frame is used only if it is finite, at least 200×200 and has a 100×60 piece of its titlebar strip on a screen. Agent runs never read or write it. The file header names this as lane r8-pointer item D14; the handoff checklist item 17 asks for a manual relaunch-twice check.
- Full screen: the module observes `didExitFullScreenNotification` (`T3WindowChrome.swift` line 31) but publishes no fact to TypeScript. The left inset stays `padding-left=90` in `settings-core.contract` and in `r12-sidebar-width.ts` (`MACOS_TRAFFIC_LIGHTS_INSET`), so in full screen the clone keeps the 90 pt gap that the reference drops (`20261005-desktop-shell-details`, scope item 6).
- To confirm: the traffic lights' x position against the reference's 16, whether the inset follows the clone's zoom, and whether a maximized window is restored as maximized. This plan did not measure them.

## Why it must be resolved

Parity goal: the window is the first thing a person sees. The full-screen gap is a visible difference; the frame workaround is an app copy of host behavior that every launch depends on and that only a real-input session can check (agent windows are placed by the driver). `20261005-desktop-shell-details` waits on the full-screen fact for its full-screen rows and carries the module-side workaround for the rest. Cost of keeping the workarounds: an empty toolbar used only to move the traffic lights, an app-owned frame record that can disagree with the host's autosave, and a hard-coded 90 that will drift if the host's chrome changes.

## Requested support

Stated the web way, on the macOS host first:

- **Window chrome.** The web standard for a custom title area is Window Controls Overlay: a manifest `display_override: ["window-controls-overlay"]` and the CSS `env(titlebar-area-x)`, `env(titlebar-area-y)`, `env(titlebar-area-width)`, `env(titlebar-area-height)` environment variables (the reference's own CSS already uses `env(titlebar-area-height)` for its Windows overlay). Map that to macOS: `env(titlebar-area-x)` is the width reserved for the traffic lights, `titlebar-area-height` the title-row height, and an `app.json` field sets the row height and traffic-light placement (`hiddenInset` equivalent). Alternative A: that standard mapping. Alternative B: a plain `app.json` `window.titleBar` block with `style`, `trafficLightPosition` and `height`, and a page fact for the inset.
- **Frame restore.** Restore the frame autosave after the final style is applied (a host fix), so the app needs no frame record. Optionally persist the maximized flag.
- **Full-screen fact.** The CSS media feature `(display-mode: fullscreen)` (or the DOM `fullscreenchange` plus `document.fullscreenElement` for the window) reflecting the native window's full-screen state, available to Contract as a viewport fact and in agent `state`.

## How to reproduce

To confirm on the pinned `main` at `issue-open`:

1. Frame: minimal app with `viewport-fit=cover`; launch, resize the window, quit, relaunch twice. Expected: same frame. Actual (per `EXACT2-GAPS.md` and the round-8 finding): the top edge moves down by the 32 pt titlebar on each launch.
2. Chrome: minimal app; place a 52-point row and read where the traffic lights sit. Expected: a way to set the row height and read the reserved width. Actual: none.
3. Full screen: clone with `T3WindowChrome` unchanged; press ⌃⌘F (or the View menu item). Expected (reference): the left padding of the sidebar header drops from 90 to the small default and returns on exit. Actual: it stays at 90. Evidence: `layout` of the header before and after, and a screenshot pair against the desktop oracle (`20261005-desktop-oracle-and-trace`).

## Acceptance for the fix

- A minimal app can read a full-screen fact; `state` shows it flip on enter and exit (agent key or menu).
- Relaunching twice leaves the frame unchanged, with no app-side frame record (AppKit test of the restore order plus an attended relaunch check on a lane build with `T3_LOCAL_HOME` and `T3_LOCAL_PORT`).
- The title-row height and traffic-light inset are set from `app.json` or the Contract; `layout` of the row shows the declared height.
- The clone's sidebar minimum width and header padding equal the oracle's in windowed and full-screen states (pixel pairs at 1280×840 and 840×620, light and dark).

## App adoption after resolution

Delete `R8PointerWindowFrame.swift` and its `t3.window.frame` default; replace the toolbar trick in `T3WindowChrome.swift` with the declared chrome (keep appearance handling); make `settings-core.contract` and `r12-sidebar-width.ts` read the inset and the full-screen fact instead of 90; reopen the full-screen rows of `20261005-desktop-shell-details` as ones that must pass. `issue-close` verifies the relaunch and full-screen rows.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

Re-checked 2026-10-07 on main `cff90b364` (task [20261007-adopt-main-fixes-r3](../tasks/closed/20261007-adopt-main-fixes-r3.md)): still missing: no title-row height or traffic-light inset setting, and no window full-screen fact (`bc6bc35f4` adds `requestFullscreen`/`fullscreenchange` for a `video` element only). `T3WindowChrome.swift` and `T3FullScreen.swift` stay.

## Re-checked on main `261dd4e10` (2026-10-07, adopt-main-fixes-r5)

[#113](https://github.com/ccheever/exact2/issues/113) is closed (2026-10-06) with only the frame restore (main #164,
adopted earlier). Still missing on `261dd4e10`: `env()` accepts only `safe-area-inset-*` and `viewport-segment-*`
(no `titlebar-area-*`), `host.macos.window` has no title-bar setting, `exactViewport()` has no `displayMode` and
`exactPage()` no full-screen field (its fields are `visibilityState`, `onLine`, `canShare`, `canOpenFiles`,
`hasFocus`). `T3WindowChrome.swift` and `T3FullScreen.swift` stay. No open upstream issue tracks the rest of #113;
filing one is the user's decision ([adopt-main-fixes-r5](../tasks/closed/20261007-adopt-main-fixes-r5.md)).
