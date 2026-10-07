---
name: 20261005-browser-surface
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Browser surface: tabs, navigation, annotate, capture, device toolbar, profiles and agent automation

## Outcome

The right panel has the reference's Browser surface. A person opens a Browser tab from the "+" menu (shortcut B, with a profile submenu), types a URL or picks a recent URL or a discovered local server, and
browses with Back, Forward, Refresh/Stop, zoom, appearance and a device toolbar. They can annotate elements, regions and drawings and attach the result to the composer, take a screenshot, record the tab, open DevTools for the
page, float the tab over the chat, open a separate preview window, use profiles (Default, Incognito and up to 24 named ones), import cookies from another browser, and clear a profile's cookies and cache. Agents drive the tab
through the 14 `preview_*` tools. Links in chat and terminal open in a Browser tab when "Open links in" is "T3 Code". The tab menu's Mute item and the audible indicator work. Every state in the reference exists: empty, loading, load failed,
unreachable, crashed, reconnecting, recording, annotating.

This ticket is blocked. It starts when issue X1 is decided. For every path: Charlie waives or amends the DEFERRED browser-shell rule and a path is chosen. Path A (Chromium) also needs the framework support merged upstream; path B (WebKit in an app-module view) needs no exact2 build change, so it can start right after the decision. If the user closes X1, this ticket closes.

## Scope and exclusions

Included (each item is a group of acceptance rows below):

1. **Surface and tabs.** The Browser row in the "+" menu (`available`, shortcut B) with the profile submenu; one tab per URL session; tab title and favicon (captured, `FaviconCapture.ts`); audible and muted indicator; the Mute / Unmute
   item in the tab menu (the slot that `20261005-right-panel-tab-menu` keeps); closing a tab closes its session (`closePreviewSession`); crash recovery (`webviewCrashRecovery`); tab lifetime across panel hide and thread switch (`desktopTabLifetime`).
2. **Chrome row.** Back, Forward, Refresh/Stop (tooltip "Loading…" while loading), the URL field ("Search or enter URL", `normalizePreviewUrl` rules: bare loopback hosts get `http://`, bare public hosts `https://`, others validated),
   "Open in system browser", Annotate, Capture screenshot (Shift-click records), Float preview over chat, the More menu (`PreviewChromeRow.tsx:121-320`, `PreviewMoreMenu.tsx:111-228`).
3. **More menu.** Hard reload, Open DevTools, Open or Close separate preview window, Show or Hide device toolbar, Appearance (System, Light, Dark), Zoom out / in / reset row, and the "Profile: <name>" group with Clear cookies and Clear cache.
4. **Navigation states.** `Idle`, `Loading`, `Success`, `LoadFailed(code, description)` (`preview.ts:146-163`), the loading bar, the unreachable page ("Checking your connection", "Confirming the dev server is running",
   "Checking the proxy and the firewall", `PreviewUnreachable.tsx:38-40`), and the empty state: "No preview yet" with "Type a URL above, or run a dev script. Browser-ready localhost servers will show up here automatically.",
   "Recently used" (at most 8, each removable) and "Local servers" ("Select a live local server to open it in this browser tab.") (`PreviewEmptyState.tsx:25-110`).
5. **History and discovery.** `browserHistoryStore` (50 entries per project, 20 projects, URL up to 2,048 characters, title up to 512; `browserHistoryStore.ts:11-16`) and the server's discovered local servers
   (`subscribeDiscoveredLocalServers`, `apps/server/src/preview/PortScanner.ts`; `useDiscoveredLocalServers`). Target resolution for loopback and environment ports (`browserTargetResolver.ts`).
6. **Viewport and zoom.** Fill, freeform or one of the 17 device presets (`packages/shared/src/previewViewport.ts:20-143`), 240 to 3,840 px, area at most 3,840×2,160, resize handles, rotate, the device toolbar
   (`BrowserDeviceToolbar.tsx`, `BrowserViewportResizeHandles.tsx`, `browserViewportLayout.ts`); zoom ladder 25%, 33%, 50%, 67%, 75%, 80%, 90%, 100%, 110%, 125%, 150%, 175%, 200%, 250%, 300%, 400%, 500% (`preview.ts:127-134`);
   appearance preference sent to the page. Keys: ⇧⌘J toggles the surface, and while the preview has focus ⌘R refresh, ⌘L focus the URL field, ⌘= / ⌘+ zoom in, ⌘- zoom out, ⌘0 reset (`packages/shared/src/keybindings.ts:33-39`; context `previewFocus`).
7. **Annotate.** Tools select, marquee, draw, erase (`PickPreload.ts:119`, shortcut hints such as "Draw freehand (D)" at `:949`), Escape cancels (`:1332-1341`), elements with component names where the page exposes them, regions, strokes and style changes,
   a cropped screenshot; the result is a `PreviewAnnotationPayload` added to the composer (`composerDraftStore.ts:676` `addPreviewAnnotation`) as a context chip (kind `preview-annotation`; the clone already renders sent chips, `r4-timeline-chips.ts:111`).
8. **Screenshot and recording.** Screenshot width at most 1,280 px; recording at 30 or 60 fps (setting), optional key-press and mouse-press overlays with password fields excluded (`RecordingInput.ts`, `RecordingCursor.ts`), the compositor, one recording at a time
   (`BrowserRecordingConflictError`), the encoded file uploaded once as an attachment (`browserRecordingUpload.ts`, errors too large, deadline expired, transfer), toasts "Screenshot saved" (Copy image, Copy path) and "Recording saved" (Reveal, Copy path)
   (`PreviewView.tsx:340-575`), artifacts pruned after N days (`settings.ts:1187-1190`); downloads that an agent starts go to the artifact directory (`Manager.ts:3570-3584`).
9. **Picture in picture.** The separate preview window (480×320, minimum 240×160, about 12 frames per second, JPEG quality 80; `Manager.ts:144-157`) and the floating mini player's browser side: "Float preview over chat", the "Auto-show floating preview"
   setting, "Reconnecting preview…" (`ThreadPreviewMiniPlayer.tsx:103-198`). The player layout and the device side belong to `20261005-floating-device-player`; this ticket adds the browser source to it.
10. **Profiles and cookie import.** Built-in Default (persistent) and Incognito (in memory), up to 24 user profiles (`BROWSER_PROFILE_MAX_COUNT`, names up to 48 characters; `browserProfile.ts:19-60`), each with its own storage; the default profile for new tabs;
    Settings › Integrations › Browser (nine rows: profiles, default viewport, zoom, appearance, recording rate, key presses, mouse presses, "Open links in", "Auto-show floating preview"; clone: `settings-source-control.contract:429-447`, `settings-catalog.ts:80-89`,
    keys `settings.ts:314-357`). Cookie import wizard for Chrome, Edge, Brave, Vivaldi, Opera, Arc, Helium, Firefox and Safari with the six unavailable reasons and their copy (`browserImport.ts:31-60,142-154`, `BrowserImportWizard.tsx`): "Quit <browser> to import",
    "Let T3 Code read <browser>'s cookies" (Full Disk Access), "Import from <browser>", "Importing cookies", "Couldn't import from <browser>".
11. **Per-page DevTools** ("Open DevTools" in the More menu).
12. **Agent automation.** The desktop is a `previewAutomation` host: `previewAutomation.connect` (stream of requests), `.respond`, `.focusHost` (`rpc.ts:419-421`), serving `preview_open`, `_navigate`, `_status`, `_snapshot`, `_click`, `_type`, `_press`, `_scroll`, `_evaluate`,
    `_wait_for`, `_resize`, `_set_appearance`, `_recording_start`, `_recording_stop` (`apps/server/src/mcp/toolkits/preview/tools.ts:53-234`); host wait budget (`previewAutomationHostBudget`), open readiness, request consumer, target resolution, the agent cursor overlay.
    Also the preview RPCs `preview.open`, `.navigate`, `.resize`, `.refresh`, `.close`, `.list`, `.reportStatus` and `subscribePreviewEvents` (`rpc.ts:411-421,1429`).
13. **Links.** "Open links in" ("Your default browser" or "T3 Code"; `IntegrationsSettings.tsx:562-565`): chat Markdown links, pull request and check links (`useOpenLink.ts`), terminal links (`openTerminalLinkInPreview.ts`) and script `previewUrl`s. ⌘ or Ctrl-click always uses the system browser; only http(s)
    URLs open in the app; a failed in-app open falls back to the system browser (`browserLinkTarget.ts:25-34`).
14. **Security posture.** Guests are sandboxed with no Node access and a partition per profile (`WebviewPreferences.ts:1-49`); permission requests allow only clipboard-read, clipboard-sanitized-write, notifications and geolocation, and deny the rest
    (`BrowserSession.ts:31-40,206-211`); pop-ups and window-open handling (`Manager.ts:2088-2098`); the page keeps the engine's native user agent (`BrowserSession.ts:198-205`).

Excluded: other platforms, headless servers (the no-host error stays: `previewAutomation.ts:725-727`), T3 Connect / Clerk sign-in, telemetry, the update feed, WSL.

**Planned split.** This ticket is a whole product. At its `prepare`, split it into separate
PRs from the updated integration branch (no stacks), keeping this name for the first:
(1) engine view, tabs and chrome; (2) navigation, history, zoom and the device toolbar;
(3) Annotate, screenshots, recording and picture-in-picture; (4) profiles, cookie import and
clearing; (5) the `previewAutomation.*` host and links from chat and terminal ("Open links
in", Mute in the tab menu). Name the others `20261005-browser-surface-<part>`.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/desktop/src/preview/*` (`Manager.ts` 5,245 lines, `PickPreload.ts`, `BrowserSession.ts`, `WebviewPreferences.ts`, `FaviconCapture.ts`, `PreviewKeyboard.ts`, `RecordingInput.ts`, `RecordingCursor.ts`, `BrowserImport/*`);
`apps/web/src/browser/*`, `apps/web/src/components/preview/*`, `apps/web/src/previewStateStore.ts`, `browserHistoryStore.ts`, `browserFaviconStore.ts`, `previewMiniPlayerStore.ts`; `packages/contracts/src/{preview,previewAutomation,browserProfile,browserImport}.ts`;
`apps/server/src/mcp/toolkits/preview/*`; settings keys `settings.ts:314-357`.
The ticket is blocked on a framework decision, so the engine-specific parts are written for both paths of [X1](../issues/20261005-x01-chromium-cdp-browser-surface.md): rows marked **P** are completed at `prepare` from the decision. The facts about exact2 come from `EXACT2-GAPS.md` X1
(written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): CDP exists only in Chromium; WebKit has no equivalent for network events, trusted input dispatch, the full accessibility tree or screencast; `rules/DEFERRED.md:380-384` and
LLP 1020 §5 refuse the built-in `iframe` as a browser shell. Everything else about exact2 for this ticket is to confirm at `issue-open` or `prepare`.
Clone state (mc-orch tree, 2026-10-05): the Browser row is unavailable (`shell.ts:152`); the nine Settings rows are inert placeholders drawn at 64% opacity (`settings-source-control.contract:429-447`); the six `preview.*` commands are in the keybinding list (`keybinding-settings.ts:106-107`)
and run nothing; sent `preview-annotation` chips render; the mini player floats device sessions only; WKWebView precedents are `modules/apple/R6MediaPreview.swift` and `T3TimelineMermaid.swift`.
Library revision: `20261005-platforms-v3`. Selected topics: layout-and-interaction, accessibility (labels, focus, reduced motion), design (complete states), motion, testing-and-debugging. Native web views, engines, permission handling and agent automation hosts are unknown in the library.
Consumer framework revision: a `main` pin that contains the X1 support. Toolchain: Xcode 27.0, Bun 1.4.2 (pinned bun path first on `PATH`), plus the engine's build requirements if path A.
Line numbers are from the reference at `1e2ecbd975` and the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves clone code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-proxy.mjs`, `trace-diff.mjs`, `drive.mjs`, `lane-backend.sh`.
Rules for lane runs: attended rows build the lane app with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; lane ports are 16000 to 16999; never use port 3773, `~/.t3` or the `t3code` URL scheme.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| resolved framework issue | [X1 embedded browser engine](../issues/20261005-x01-chromium-cdp-browser-surface.md) | none yet (local draft) | Charlie\'s decision on the DEFERRED browser-shell rule and a chosen path; for path A also the support merged upstream (path B needs no exact2 change) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle and trace tools for the Browser surface) | pending |
| scheduling preference | After `20261005-right-panel-tab-menu` (Mute slot), `20261005-floating-device-player` (player layout), `20261005-terminal-integrations` (link routing hook) | none | Not prerequisites | — |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X1](../issues/20261005-x01-chromium-cdp-browser-surface.md) | Embedded browser engine and Charlie's waiver | `EXACT2-GAPS.md` X1 | blocking | Wait for the decision; then complete the **P** rows |
| [X8](../issues/closed/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | Page clicks, drags, annotate drawing and resize handles on the guest are attended; handles in Contract are agent-driven |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket | LLP 1016.000 receive-only | nonblocking (workaround: Swift transport) | Add the preview RPCs and the `previewAutomation.*` stream to `T3Transport.swift` |
| [X22](../issues/20261005-x22-reactive-layout-facts.md) | Slot rect of the guest view | `t3-frame` hooks | nonblocking (workaround exists) | Report the slot's rect to the native view |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | Key facts and the `previewFocus` context | native key monitors | nonblocking | The guest swallows keys; forward ⌘R, ⌘L, ⌘= ⌘- ⌘0 as `PreviewKeyboard.ts` does |
| [X26](../issues/20261005-x26-app-menu-control.md) | Native context menus | `T3ContextMenu.swift` | nonblocking | Tab menu rows reuse the native menu |

## Implementation notes

- **Path-independent layers first.** Pure logic with ported tests: URL normalization and target resolution, history store, viewport layout and actions, zoom ladder, link-target decision, profile list, import copy, recording scope and compositor decisions, automation budget and readiness,
  the empty-state logic. State in TypeScript modules (`browser-state.ts`, `browser-history.ts`, `browser-viewport.ts`, `browser-links.ts`, `browser-profiles.ts`); views in `browser-surface.contract` and `browser-chrome.contract`; the engine behind a small app-module interface
  (`T3BrowserEngine`: load, navigate, history, zoom, appearance, profile storage, evaluate, snapshot, input, screenshot, frames, devtools, audio state) so the path decision changes one module.
- **Engine module.** Path A: CEF view and helper apps as the framework build provides. Path B: `WKWebView` in `modules/apple/T3Browser*.swift` (precedent `R6MediaPreview.swift`), with an injected script for console, network and page automation, a snapshot-based screenshot, and the ARIA snapshot
  from the vendored script that `EXACT2-GAPS.md` X1 names. The deviation table in X1 lists what path B cannot match; each row there must either be closed by a framework fix or accepted by the user before this ticket claims parity.
- **Agent automation host.** `previewAutomation.connect` is a request stream; the host answers each request with `previewAutomation.respond` within the host wait budget (`resolveHostWaitBudgetMs`: the request timeout minus min(1,500 ms, 20%)). Readiness rules: `previewAutomationOpenReadiness`.
  The agent cursor overlay shows who controls the tab (`agentBrowserCursorLogic`).
- **Annotate.** The overlay and the payload are engine-side code (`PickPreload.ts` has more than 1,300 lines). Port the payload schema and the composer attach first; the overlay is injected into the guest; component-name resolution needs the page's React hook and is best effort.
- **Security.** Keep the reference's posture (item 14). Path B has no equivalent of an engine-level sandbox flag: record what the platform gives (to confirm at `prepare`).
- **States.** Loading: loading bar and "Loading…" tooltip. Empty: the empty state. Error: LoadFailed page, unreachable page, crash recovery, recording and screenshot failures with toasts, import failures. Disabled: buttons without history; Annotate and Capture when no tab; Float when
  no thread. Hover and keyboard focus: ring on every chrome button; the URL field selects all on focus. Escape: cancels annotation, closes the More menu, leaves the URL field (restoring the page URL). Reduced motion: the loading bar and the recording dot stop pulsing; the player's drag
  keeps its result. Every icon-only button keeps its `aria-label` (list in item 2).
- **Hot files.** Settings rows replace the inert `BrowserDefaults` rows in place; the launcher row's `available` flag and reason come from engine availability (`shell.ts:152`).

## Acceptance and reproduction

All rows run on macOS at 1280×840 and 840×620, light and dark, with a lane backend (`target/t3-ui-parity/lane-backend.sh`, isolated HOME, ports 16000 to 16999). Pages come from a local fixture server (static pages for load, redirect, 404, slow load, unreachable port, download, popup, form, audio, a React page
for component names). Oracle pairs use `target/t3-ui-parity/electron-oracle.mjs` on the same scenes. The `trace-proxy.mjs` and `trace-diff.mjs` tools compare RPC frames.

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` | The ported tests pass (list below) | macOS | log |
| Open and tabs | Thread; fixture server | Agent: "+" menu › Browser (B) with each profile; open two tabs; switch; close one; hide and show the panel | Tab titles and favicons follow the pages; sessions persist across hide, thread switch and relaunch as the oracle's; closing ends the session | macOS | `--json` tree/state, pairs |
| Chrome row | Page with history | Back, Forward, Refresh, Stop during a slow load, URL entry (bare host, `localhost:5173`, full URL, invalid), Open in system browser | States, tooltips ("Loading…") and normalization equal the oracle; invalid input shows the reference error; system browser opens the URL | macOS | `--json`, shots |
| Empty, loading, failed, unreachable | No tabs; then slow, 404, closed port, bad certificate | Open each | "No preview yet" with Recently used and Local servers (started fixture servers appear within the poll time); the loading bar; LoadFailed with code and description; the unreachable checklist | macOS | pairs |
| More menu | Open tab | Each item; Appearance radios; zoom row (buttons keep the menu open); Profile group | Items, order, labels, disabled states and results equal the oracle; zoom ladder steps match; cookies and cache clear for the profile only | macOS | `--json`, pairs |
| Device toolbar | Open tab | Show toolbar; each of the 17 presets; freeform 240 to 3,840; area limit; rotate; resize handles `(attended session)` | Sizes, clamps and the aspect lock equal the oracle and `previewViewport.test.ts:37` | macOS | `--json` layout, shots |
| Keys | Preview focused | ⌘R, ⌘L, ⌘= ⌘+ ⌘- ⌘0, ⇧⌘J; also with the guest page holding focus | Each does the reference action; the guest does not swallow them; ⇧⌘J toggles the surface | macOS | `--json` press, state |
| Annotate | Fixture page | Annotate (tooltip "Annotate elements, regions, and drawings"); select, marquee, draw, erase; Esc; send | Payload equals the oracle's for the same page (elements, regions, strokes, screenshot crop); chip in the composer; Esc cancels; drawing and marquee drags `(attended session)` | macOS | trace diff, shots |
| Screenshot | Open tab | Click Capture | Image up to 1,280 px wide; toast "Screenshot saved" with Copy image, Copy path; clipboard holds the image or the path | macOS | clipboard read, shot |
| Recording | Open tab; frame rate 30 then 60; overlays on/off; password field page | Shift-click Capture; interact; Stop | Pulsing dot; file saved and uploaded once as an attachment; toast "Recording saved" with Reveal and Copy path; overlays show keys and presses, never password input; a second tab cannot record at the same time; oversize and deadline errors show the reference messages | macOS (Screen Recording prompt if path B) | file check, trace |
| Separate window and player | Open tab | More › Open separate preview window; Float preview over chat; drag; thread switch; Auto-show setting; agent `preview_open` with and without `show` | Window size 480×320, min 240×160, about 12 fps; the player follows the oracle's layout rules; "Reconnecting preview…" when the guest drops; the setting decides auto-show | macOS | shots, state |
| DevTools | Open tab | More › Open DevTools | The page's developer tools open (path A: Chromium DevTools; path B: **P**) | macOS | shot |
| Profiles | Settings › Integrations › Browser | Add, rename, delete a profile; 24 limit; Incognito; default profile; open tabs under two profiles with the same site | Cookie jars are separate; Incognito clears on quit; the limit message equals the oracle's | macOS | state, shots |
| Cookie import | Fixture browser stores (copies of the reference's import fixtures) | Run the wizard for each source state: not installed, keychain approval, keychain missing, Full Disk Access, browser running, success, failure | Copy and steps equal the oracle's; success imports the fixture cookies into a new or existing profile; no real browser data is read in the lane run | macOS | shots, state |
| Agent tools | Lane backend; MCP client | Each of the 14 `preview_*` tools, including failure cases and timeouts | Results equal the oracle host's (trace diff); with the Browser disabled, the no-host error text equals `previewAutomation.ts:725-727` | macOS | trace diff |
| Links | Setting "T3 Code" then "Your default browser" | Click chat, PR, check and terminal links; ⌘-click; a `mailto:` link; a failing open | Tab opens beside the thread or the system browser opens, as `resolveLinkTarget` decides; fallback on failure; history records the visit | macOS | `--json`, `open` log |
| Permissions and popups | Fixture page asking for camera, geolocation, clipboard and a popup | Trigger each | Allowed set and denials equal the reference; popup handling equals `previewWindowOpenAction` | macOS | log |
| Mute | Page with audio | Tab menu › Mute / Unmute | Indicator and item follow the audible state (the three `tabMuteMenuItem` tests of `RightPanelTabs.test.tsx:253-283` ported) | macOS | state |
| Crash and recovery | Kill the page process (engine specific **P**) | Wait | Recovery matches `webviewCrashRecovery` | macOS | log |
| Real input `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch, real trackpad | Browse a real site; select text; scroll; pinch zoom if the reference allows it; drag in annotate; resize handles; drag the player; Korean 2-Set typing in the URL field and the page; ⌘ keys | Matches the oracle; each step recorded | macOS | notes, video |
| States | Each surface above | Keyboard focus ring on every chrome button; Escape in each mode; reduced motion on | Focus visible; Escape cancels one thing; no pulsing with reduced motion | macOS | `--json`, shots |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Tests to port (`bun:test`, original names; counts by `it(` at line start on 2026-10-05; DOM-bound tests such as `PreviewView.test.tsx` and `HostedBrowserWebview.test.tsx` become agent rows):
`previewStateStore.test.ts` (26), `browserHistoryStore.test.ts` (33), `browserFaviconStore.test.ts` (15), `browserFaviconLogic.test.ts` (4), `browser/browserDefaults` (4), `browserLinkTarget` (6), `browserTargetResolver` (19), `browserViewportActions` (4),
`browserViewportLayout` (12), `BrowserDeviceToolbar` (2), `browserSurfaceStore` (9), `desktopTabLifetime` (5), `previewRuntimeTabId` (3), `webviewCrashRecovery` (2), `browserRecording` (28), `browserRecordingScope` (5), `recordingCompositor` (7),
`components/preview/openTerminalLinkInPreview` (7), `closePreviewSession` (2), `openPreviewSession` (4), `addBrowserSurface` (2), `previewEmptyStateLogic` (3), `useDiscoveredLocalServers` (11), `previewViewportReadiness` (3), `previewViewportRollback` (2),
`previewClickFocus` (8), `previewAutomationHostBudget` (10), `previewAutomationOpenReadiness` (13), `previewAutomationRequestConsumer` (11), `previewAutomationTarget` (4), `agentBrowserCursorLogic` (3), `packages/contracts` `preview.test.ts` (23) and `browserProfile.test.ts` (9),
`packages/shared` `previewViewport.test.ts`, `apps/desktop` `PickedElementPayload` (11), `PreviewKeyboard` (12), `AnnotationKeyboard` (2), `RecordingInput` (5), `FaviconCapture` (22) and `BrowserImport/*` (108; the reader tests need the engine and fixtures: port the pure parts, run the rest as lane rows).
Record each substitution in the file header. The server-side MCP tests (`tools.test.ts` 2, `handlers.test.ts` 5) stay with the server.

Task-owned source paths: `shell.ts`, `settings-source-control.contract`, `settings-catalog.ts`, `keybinding-settings.ts`, new `browser-*.ts` and `browser-*.contract` with tests, `modules/apple/T3Browser*.swift`, `macos/tests/browser/**`, `r4-surfaces-panel.ts`, `r4-surfaces.contract`, `AGENT-HANDOFF.md`.
Required environment: lane backend with the preview fixture server and the fixture browser stores, oracle build, Xcode 27.0, Bun 1.4.2, the X1 support at the pinned `main`; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`.

## Progress

Blocked on X1 (Charlie's decision and the chosen path). No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |

## Next action

Blocked until X1 (`../issues/20261005-x01-chromium-cdp-browser-surface.md`) is resolved or decided; then `prepare`, or close this ticket if the decision is to close.
