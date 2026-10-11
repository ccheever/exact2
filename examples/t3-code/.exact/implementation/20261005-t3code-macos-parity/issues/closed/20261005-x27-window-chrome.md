---
name: 20261005-x27-window-chrome
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-desktop-shell-details]
upstream_url: https://github.com/ccheever/exact2/issues/113
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/267
---

# X27: Title-row height and traffic-light inset, frame restore after the final style, full-screen state fact

Moved to main `issues/20261009-titlebar-env-display-mode.md` (2026-10-09); tracked there.

## Summary

T3 Code's desktop window has a hidden-inset title bar with its traffic lights placed to fit a 52-point title row, comes back at the size and place it was left, and drops the space reserved for the traffic lights when it goes full screen. exact2's `viewport-fit=cover` gave full-size content but no title-row or traffic-light setting, restored the frame before the final window style, and gave the page no full-screen fact. Main #164 fixed the frame restore; the title row and the full-screen fact are what main still tracks.

## Why it arose

### The T3 Code behavior

- **Title bar.** macOS windows use `titleBarStyle: "hiddenInset"` with `trafficLightPosition` `x: 16` and `y: 52 / 2 − 7 = 19` (`apps/desktop/src/window/DesktopWindow.ts:37–46`, `:256–265`). The web layout is 52 CSS px tall at the top (`--workspace-topbar-height`, `apps/web/src/index.css`).
- **Space for the traffic lights.** The page reserves `var(--desktop-window-controls-inset, 90px)` at the left of the title row (`apps/web/src/components/AppSidebarLayout.tsx:60,259`); the preload script sets it to `90 / zoom` px (`apps/desktop/src/preload.ts:30–47`).
- **Full screen.** The page asks the desktop bridge for the full-screen state and subscribes to changes (`getWindowFullscreenState`, `onWindowFullscreenStateChange`; `packages/contracts/src/ipc.ts:1241`; `AppSidebarLayout.tsx:262–281`). In full screen the inset is dropped, because the traffic lights are not in the window.
- **Frame.** The window's bounds are saved on resize, move, maximize, unmaximize and close (`DesktopWindow.ts:670–678`); a full-screen, maximized or minimized window saves its normal bounds (`:430–440`), and the maximized flag is restored at reveal (`:818–830`).

### Where the clone hits it

`20261005-desktop-shell-details` (scope item 6 and its full-screen rows). The host's `viewport-fit=cover` had no title-row or traffic-light setting (`ExactMac/main.swift:209-223`), and the frame autosave was restored before the final style (`:310-313`), so every launch lost the 32 pt titlebar (818 → 786 → 754, the top edge moving down).

## Clone workaround

- `modules/apple/T3WindowChrome.swift`: the 52-point title row uses the window's real traffic lights. It attaches an empty unified `NSToolbar`, which makes AppKit center the buttons at y = 26 (equal to the reference's 19 + 7), sets a transparent titlebar and a hidden title. Contract's `viewport-fit=cover` supplies the full-size content.
- `modules/apple/T3FullScreen.swift` (desktop-shell-details): the window's full-screen state as a native fact; `NSWindow`'s did-enter and did-exit notifications publish `t3.status`, whose presentation carries `fullScreen`, so the title rows drop the 90 pt inset as the reference does.
- `R8PointerWindowFrame.swift` (the app's own frame record, defaults key `t3.window.frame`, lane r8-pointer item D14) was removed when main #164 landed (below).
- When main has the fact and `env()` slice (#267's decision, 2026-10-08), `T3FullScreen.swift` and the hard-coded 90 pt inset go; `T3WindowChrome.swift` (the title-row height) waits for the manifest field.

## Evidence and history

- Filed 2026-10-06 as [#113](https://github.com/ccheever/exact2/issues/113).
- 2026-10-07 (adopt-main-fixes-shell, PR #181): main #164 restores the frame after the final style; `R8PointerWindowFrame.swift` deleted. #113 was closed with only that part.
- 2026-10-07, re-checked on main `cff90b364` ([20261007-adopt-main-fixes-r3](../../tasks/closed/20261007-adopt-main-fixes-r3.md)) and `261dd4e10` ([adopt-main-fixes-r5](../../tasks/closed/20261007-adopt-main-fixes-r5.md)): `env()` accepts only `safe-area-inset-*` and `viewport-segment-*`, `host.macos.window` has no title-bar setting, `exactViewport()` has no `displayMode` and `exactPage()` no full-screen field (`bc6bc35f4` adds `requestFullscreen`/`fullscreenchange` for a `video` element only). `T3WindowChrome.swift` and `T3FullScreen.swift` stay.
- 2026-10-08: the rest filed as [#267](https://github.com/ccheever/exact2/issues/267), reproduced on main `0365ad1a4`: macOS `layout` gives `safe-area 32 0 0 0` against a 52 pt title row and the traffic lights stay centred at about y = 16 pt; `env(titlebar-area-*)` is `lower-attr-value`; `exactPage().fullscreen` and `exactViewport().displayMode` are refused at bake.
