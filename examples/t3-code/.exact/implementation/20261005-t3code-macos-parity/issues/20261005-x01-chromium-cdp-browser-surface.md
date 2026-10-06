---
name: 20261005-x01-chromium-cdp-browser-surface
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-policy + framework-gap
blocks: [20261005-browser-surface, 20261005-right-panel-tab-menu, 20261005-t3-connect-sign-in]
upstream_url: null
reproduced_on: null
---

# X1: An embedded browser engine that an app can drive (Browser surface)

## Summary

T3 Code's desktop app has a full Browser surface in the right panel: Chromium tabs, an agent-driven page, annotation,
screenshots and recordings, device emulation, profiles and per-page DevTools. exact2 has no embedded browser engine that
an app can use as a browser shell, and its rules refuse the built-in `iframe` as one. The clone leaves the surface out and
opens links in the system browser. The surface needs either a Chromium path (framework build changes) or a WebKit path (app
module), and in both cases a decision from Charlie on the DEFERRED browser-shell rule.

## Why this issue arose

### The T3 Code behavior
The Browser surface is a desktop-only part of the right panel (reference `1e2ecbd975`).

- **Tabs and chrome.** The "+" menu has a Browser row (shortcut B) with a profile submenu; a tab is a `<webview>` guest
  (`apps/web/src/browser/HostedBrowserWebview.tsx`, `BrowserSurfaceSlot.tsx`). The chrome row has Back, Forward,
  Refresh/Stop, a URL field ("Search or enter URL"), "Open in system browser", "Annotate preview" / "Cancel annotation (Esc)",
  "Capture screenshot" (Shift-click records, "Stop recording" with a pulsing dot), "Float preview over chat", and a More menu
  (`components/preview/PreviewChromeRow.tsx:121-320`, `PreviewMoreMenu.tsx`). More: Hard reload, Open DevTools, Open separate preview
  window, Device toolbar, Appearance (System/Light/Dark), Zoom out/in/reset, and a "Profile: <name>" group with Clear cookies and Clear cache.
- **States.** Navigation is Idle, Loading, Success or LoadFailed (code, description) (`packages/contracts/src/preview.ts:146-163`); a loading
  bar; the floating player shows "Reconnecting preview…" when its guest is missing (`ThreadPreviewMiniPlayer.tsx:191`); an unreachable page lists "Checking your connection / Confirming the dev server is
  running / Checking the proxy and the firewall" (`PreviewUnreachable.tsx`); the empty state is "No preview yet" with Recently used (8) and
  discovered Local servers (`PreviewEmptyState.tsx`, server `apps/server/src/preview/PortScanner.ts`); tab favicons are captured
  (`apps/desktop/src/preview/FaviconCapture.ts`); tabs show audible or muted state and a Mute/Unmute menu item.
- **Viewport and zoom.** Fill, freeform or one of 17 device presets, 240–3840 px, area at most 3840×2160, resize handles, rotate; zoom ladder of 17
  steps from 25% to 500% with ⌘= ⌘- ⌘0; ⌘R refresh and ⌘L focus URL while the preview has focus; ⌘⇧J toggles the surface
  (`preview.ts:28-135`, `packages/shared/src/keybindings.ts:33-39`, `BrowserDeviceToolbar.tsx`).
- **Annotate.** An in-page overlay (select, marquee, draw, erase tools; elements, regions, strokes, style changes, a cropped screenshot) returns a
  `PreviewAnnotationPayload` that is attached to the composer or sent (`apps/desktop/src/preview/PickPreload.ts:119,949,1332-1341`, `ipc.ts:826-1010`).
- **Screenshot and recording.** Screenshots (width at most 1280) and screencast recordings (30 or 60 fps; optional key and mouse press overlay, password
  fields excluded) are saved as artifacts with toasts "Screenshot saved" (Copy image, Copy path) and "Recording saved" (Reveal, Copy path)
  (`PreviewView.tsx:340-575`, `Manager.ts:141-157`, `settings.ts:314-357`); artifacts are pruned after N days (`settings.ts:1189`).
- **Picture in picture.** A separate native window mirrors the tab (480×320, minimum 240×160, about 12 frames per second, JPEG quality 80), and the tab can
  float over the chat as the mini player (`Manager.ts:144-157`, `ThreadPreviewMiniPlayer.tsx:103-198`).
- **Profiles and import.** Built-in Default and Incognito plus up to 24 named profiles, each its own partition (`browserProfile.ts`,
  `BrowserSession.ts:12`); cookie import from Chrome, Edge, Brave, Vivaldi, Opera, Arc, Helium, Firefox and Safari with six unavailable reasons
  (not installed, needs keychain approval, keychain item missing, needs Full Disk Access, browser running, unsupported platform)
  (`packages/contracts/src/browserImport.ts`, `apps/desktop/src/preview/BrowserImport/*`, 24 tests).
- **Agent automation.** The server exposes 14 MCP tools (`preview_open`, `_navigate`, `_status`, `_snapshot`, `_click`, `_type`, `_press`, `_scroll`,
  `_evaluate`, `_wait_for`, `_resize`, `_set_appearance`, `_recording_start`, `_recording_stop`) that reach the desktop through `previewAutomation.connect`,
  `.respond` and `.focusHost` (`rpc.ts:419-421`, `apps/server/src/mcp/toolkits/preview/*`). The desktop is the host; a cursor overlay shows who controls the tab.
  Without a host the tool fails with: "No preview automation host is available … Preview tools run in a T3 Code desktop app that is open and connected
  to this environment" (`previewAutomation.ts:725-727`).
- **Links.** "Open links in" ("Your default browser" by default, or "T3 Code"; `IntegrationsSettings.tsx:562-565`, `browserLinkTarget.ts:25-34`) routes chat Markdown links, pull request and check links, terminal links and
  script `previewUrl`s; ⌘ or Ctrl-click always uses the system browser (`browserLinkTarget.ts`, `useOpenLink.ts`, `openTerminalLinkInPreview.ts`).
- **Size.** The desktop side is `Manager.ts` (5,245 lines) over Electron `<webview>` and `webContents.debugger`; the CDP domains it calls are Runtime,
  Network, Log, Page (`createIsolatedWorld`, screencast), Emulation, Input (mouse and key dispatch), Accessibility (`getFullAXTree`), DOM and Target.
  Reference tests (recounted on 2026-10-05 by `it(`/`test(` at line start): 117 in `apps/web/src/browser`, 150 in `apps/web/src/components/preview`, 81 in `apps/desktop/src/preview`, 108 in `apps/desktop/src/preview/BrowserImport`, 7 for the MCP preview tools, and the stores `previewStateStore` (26), `browserHistoryStore` (33), `browserFaviconStore` (15), `browserFaviconLogic` (4).

### What exact2 does today
- `EXACT2-GAPS.md` X1 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): "Chromium (CEF) needs a dynamic framework plus
  GPU/renderer/plugin helper apps inside the bundle. The exact2 Apple build cannot embed either." (citing `host/apple/build.mjs:1021-1045`: module
  dependencies link only as static libraries or framework slices from `modules/apple/*.xcframework`; `host/apple/build.mjs:1229-1244`: the macOS bundle copies only
  exact's binaries and `assets/`, with no `Contents/Frameworks` and no helper apps.) "CDP exists only in Chromium. WebKit has no equivalent for Network events,
  trusted input dispatch, the full accessibility tree, or screencast."
- Same file: `rules/DEFERRED.md:380-384` and LLP 1020 §5: the built-in `iframe` refuses `top` topology ("never, absent a product that is a browser shell"), navigation
  policy, popups, the controller ops, and permissions. "Recording and PiP need screen capture. ScreenCaptureKit asks for Screen Recording even for the app's own
  windows (`host/apple/Sources/ExactKit/Mac/AgentMac.swift:814`)."
- Library (`20261005-platforms-v3`, capabilities): "Unlisted APIs, Android parity, arbitrary native integrations — Unknown in this library." Not covered.
- Observed in the clone (mc-orch tree, 2026-10-05): the app already hosts WKWebViews for rendered HTML and Mermaid (`modules/apple/R6MediaPreview.swift`,
  `T3TimelineMermaid.swift`), so a WebKit view in an app module works today; nothing there is a browser shell.

### Where the clone hits it
- Right panel: the Browser row is always unavailable (`shell.ts:152`, "Only available in the desktop app."). Nine Browser rows in Settings › Integrations are disabled
  placeholders (`settings-source-control.contract:432,440-447`: profiles, default viewport, zoom, appearance, recording rate, key and mouse press overlays, "Open links in",
  "Auto-show floating preview"). The keybinding list has the six `preview.*` commands (`keybinding-settings.ts:106-107`) but nothing runs them.
- Links open in the system browser. The mini player floats device sessions only (`r12-threads-device.ts`). Sent `preview-annotation` chips render
  (`r4-timeline-chips.ts:109`) but none can be created.
- Agents on a thread in this client: `preview_*` tools find no host and return the no-host error above (reference source; to confirm on a lane run at `issue-open`).
- The terminal link target, the Mute item and the browser side of the player are carried as differences by the tickets in `blocks`.

## Why it must be resolved
The goal is a complete clone of the desktop app, and the Browser surface is one of its larger features: the reference's own agents use it to check the pages they build.
Until it exists, a T3 Code user loses annotation, screenshots, recordings, profiles, cookie import and every `preview_*` agent tool, and sees nine disabled settings rows.
Waiting tickets: `20261005-browser-surface` (the whole feature), and rows in `20261005-terminal-integrations` (link target), `20261005-right-panel-tab-menu` (Mute),
`20261005-floating-device-player` (browser source). Keeping the workaround costs little code but leaves a permanent, visible gap. The decision needed from Charlie: waive or
amend the DEFERRED browser-shell rule (`rules/DEFERRED.md:380-384`, LLP 1020 §5) for an app that is itself a browser shell, and choose path A or B (or close the feature by decision).

## Requested support
The web way: a browsing-context element with `src`, navigation events, history, zoom, per-profile storage, DevTools, script injection and a trusted input path. The macOS host first.

| Capability (reference mechanism) | Path A: Chromium (CEF or similar) | Path B: WebKit (`WKWebView` in an app module) |
| --- | --- | --- |
| Engine and rendering | Same engine as the reference; Chromium rendering | WebKit rendering (a visible difference from the reference) |
| Build | Needs a dynamic framework and helper apps in the bundle: copy, sign, `@rpath` (`host/apple/build.mjs:1021-1045,1229-1244`); a DEFERRED waiver | No exact2 build change (EXACT2-GAPS: "no exact2 change required"); the waiver question for an app-module shell is to confirm with Charlie |
| Page automation (`webContents.debugger`, CDP) | CDP is available | No CDP: injected script and native events instead |
| Console and network capture (`Runtime`, `Network`, `Log`) | CDP events | Injected script; no subresource status (EXACT2-GAPS) |
| Input (`Input.dispatchMouseEvent`/`KeyEvent`) | Trusted CDP input | Synthetic or NSEvent input instead of CDP (EXACT2-GAPS) |
| Accessibility snapshot (`Accessibility.getFullAXTree`) | CDP | ARIA snapshot from a vendored Playwright script (EXACT2-GAPS) |
| Recording and PiP (screencast) | `Page.screencastFrame` | `takeSnapshot` polling, or ScreenCaptureKit with a Screen Recording prompt (EXACT2-GAPS) |
| DevTools (`openDevTools`) | Chromium DevTools | Safari Web Inspector; it cannot be opened from code (EXACT2-GAPS) |
| Profiles, mute, zoom, annotate overlay (`persist:` partitions, `setAudioMuted`, `react-grab` preload) | Partitions and preload as in Electron | Each needs a WebKit mechanism: to confirm at `issue-open` |
| Cookie import | Same readers (main-process work, app side) | Same readers; the session write API differs: to confirm |
| Size of the work | Framework build work plus the app | App module only; X8 would help the agent test it |

Also useful on either path: X8 (agent pointer input in native views). Alternative C: keep the feature closed by user decision (then this issue closes).

## How to reproduce
To confirm on the pinned `main` at `issue-open`.
1. Launch the lane app on a fixture backend (isolated `T3_LOCAL_HOME`, port 16xxx). Open a thread, open the right panel "+" menu: the Browser row is disabled.
2. Open Settings › Integrations › Browser: the nine rows are disabled. Expected (reference): working controls and a Browser tab.
3. Start a thread whose agent calls `preview_open`: expected (reference) a tab opens; actual (clone) the tool returns the no-host error.
4. Minimal framework check: a sample app that asks for a Contract element showing a URL with navigation events; today the only candidates are the iframe (refused as a shell) and an app-module native view.

## Acceptance for the fix
- Path A: a sample bundle contains the framework and helper apps, is signed, launches, and shows a page; `bun host/apple/build.mjs … --bundle` documents the layout; an AppKit test loads a page, reads
  navigation events and zoom. Path B: an app-module view loads a page and reports Idle/Loading/Success/LoadFailed, back/forward state, zoom and a per-profile data store; the agent can
  `tree`/`state` it.
- The DEFERRED decision is recorded (waiver text, scope) and `rules/DEFERRED.md` is updated by its owner.
- For the app: `20261005-browser-surface` acceptance rows can run (its table).

## App adoption after resolution
Unblock `20261005-browser-surface` (`prepare`, then implement). Remove the disabled placeholders (`shell.ts:152`, `settings-source-control.contract:432-447`) as the feature lands; wire the six
`preview.*` keybindings; route links by "Open links in"; add the browser source to the mini player (`20261005-floating-device-player`); show Mute in the tab menu (`20261005-right-panel-tab-menu`);
connect as a `previewAutomation` host. `issue-close` checks that the agent `preview_*` tools succeed against the clone and that the settings rows work.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).
