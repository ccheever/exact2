---
name: 20261005-browser-surface
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface
pr_url: null
verified_commit: null
---

# Browser surface part 1: engine view, tabs and chrome

## Outcome

The right panel has a Browser surface on a `WKWebView` inside the clone's own module. A person opens a Browser tab from
the "+" menu or the launcher (shortcut B; the "+" menu's profile submenu lists Default), types an address in the chrome
row's "Search or enter URL" field (a bare loopback host gets `http://`, a bare public host `https://`, anything else is
validated), and browses with Back, Forward and Refresh (named Stop while a page loads, with the loading bar and the
"Loading…" tooltip), Open in system browser and the More menu's Hard reload. A tab shows its page title (else its host,
else "Browser") and its favicon (captured from the page, else the public favicon service for a public host, else the
globe). An idle tab shows "No preview yet"; a page that fails to load shows "This site can't be reached" with the
host, the error's words, its name and Reload. A tab is a server preview session (`preview.open`, `.close`, `.list`,
`.reportStatus`): it keeps its page while the panel is hidden and while another thread is shown, closing it closes the
session, and a page whose WebKit process dies is reloaded as `webviewCrashRecovery` plans. The page keeps WebKit's
native user agent, the camera and the microphone are denied, a scripted pop-up opens a real window that keeps its
opener, and in a development build Safari's Web Inspector attaches to the page (`T3WebInspection`). Annotate, Capture
and Float preview are drawn disabled and marked "part 3"; the More menu's other rows are drawn disabled and marked with
their part.

This is part 1 of the planned split (below). The rest of the reference's Browser surface is in four records, each
planned to start after this one merges:
- [part 2, navigation](20261005-browser-surface-navigation.md): Recently used and Local servers, the unreachable page's
  Details, history and discovery, zoom, appearance, the device toolbar and the preview keys;
- [part 3, capture](20261005-browser-surface-capture.md): Annotate, screenshots, recording, downloads, the separate
  window and the floating player;
- [part 4, profiles](20261005-browser-surface-profiles.md): Incognito and named profiles, cookie import, Clear cookies
  and Clear cache;
- [part 5, automation](20261005-browser-surface-automation.md): the `previewAutomation.*` host, preview events, links
  from chat and terminal, and Mute.

Engine (user decision, 2026-10-08, after [#100](https://github.com/ccheever/exact2/issues/100) was closed upstream as not planned): **path B of
[X1](../issues/closed/20261005-x01-chromium-cdp-browser-surface.md)**, a `WKWebView` in the clone's module.
- No CDP, no Chromium and no per-page Chromium DevTools: Charlie refused path A ("Keep a browser shell outside core … Do not add Chromium/CEF or browser-control commands as incidental
  iframe work").
- Safari Web Inspector stands in for DevTools (development builds only, as #101 allows); it cannot be opened from code.
- Each row of X1's path-B deviation table that a part builds is a declared difference in `EXACT2-GAPS.md` ("Browser
  surface: declared differences (X1 path B)"); part 1's rows are there.
- This is clone-side work, not a framework wait: path B needs no exact2 change, and X1 is closed.

## Scope and exclusions

Part 1 (the planned split's "engine view, tabs and chrome"), by the original scope's numbering:

1. **Surface and tabs.** The Browser row in the "+" menu and the launcher (`available` on the desktop app, shortcut B;
   the "+" menu's profile submenu lists the profiles, Default only until part 4; the launcher's chevron shows only with
   more than one profile, `RightPanelTabs.tsx:495-560, 1246-1285`); one tab per server preview session
   (`addBrowserSurface`, `openPreviewSession`); tab title and favicon (`surfaceTitle`, `PreviewFavicon`, a captured icon
   after `FaviconCapture.ts`); closing a tab closes its session (`closePreviewSession`); crash recovery
   (`webviewCrashRecovery`); tab lifetime across panel hide and thread switch (`desktopTabLifetime`,
   `ElectronBrowserHost`). The audible indicator and Mute are part 5; the profile list is part 4.
2. **Chrome row.** Back, Forward, Refresh/Stop (tooltip "Loading…" while loading; pressing it reloads, as the
   reference's `PreviewManager.refresh`), the URL field ("Search or enter URL", `normalizePreviewUrl`; selected on
   focus, Enter goes, Escape restores the URL and leaves the field), "Open in system browser" (shown on hover), and the
   More menu ("More", aria-label "Preview menu") with Hard reload working and the rest disabled with their part
   (`PreviewChromeRow.tsx:121-320`, `PreviewMoreMenu.tsx:111-228`). Annotate, Capture and Float preview are visible,
   disabled and marked "part 3".
4. **Navigation states, the minimum.** `Idle`, `Loading`, `Success`, `LoadFailed(code, description)`
   (`preview.ts:146-163`, the desktop's `computeNavStatus` and `failed`), the loading bar
   (`.preview-loading-progress`), the empty state's "No preview yet", and the load-failed page's heading, words, error
   name and Reload (`PreviewUnreachable.tsx`). Recently used, Local servers and the Details checklist are part 2.
11. **Page inspection.** The tab's `WKWebView` is `isInspectable` in development builds only, through
    `T3WebInspection.mark` (#326); the reference's "Open DevTools" item is absent, a declared difference (as View ›
    Toggle Developer Tools, X2).
14. **Security posture.** Permission requests: the camera and the microphone denied through `WKUIDelegate`; clipboard,
    notifications and geolocation as WebKit offers them (declared); pop-ups through `createWebViewWith`
    (`previewWindowOpenAction`); the page keeps WebKit's native user agent; each environment's profile has its own
    persistent `WKWebsiteDataStore`, apart from the app's other web views.

Moved to parts 2–5 (their records hold the text, the acceptance rows and the tests to port): scope items 3 (the More
menu's device toolbar, appearance and zoom: part 2; its separate window: part 3; its Profile group: part 4), 4's rest
(part 2), 5 and 6 (part 2), 7, 8 and 9 (part 3), 10 (part 4; the Settings rows go with the part of their feature), 12
and 13 (part 5), and 1's Mute (part 5).

Excluded, as before: Chromium, CDP and per-page Chromium DevTools (path A, refused by #100); other platforms, headless
servers (the no-host error stays: `previewAutomation.ts:725-727`), T3 Connect / Clerk sign-in, telemetry, the update
feed, WSL.

**Planned split (done at `prepare`, 2026-10-09).** This ticket was a whole product; it is split into five PRs from the
updated integration branch (no stacks), this name for the first: (1) engine view, tabs and chrome; (2)
[navigation](20261005-browser-surface-navigation.md); (3) [capture](20261005-browser-surface-capture.md); (4)
[profiles](20261005-browser-surface-profiles.md); (5) [automation](20261005-browser-surface-automation.md). Each later
part starts after this one merges into `feat(example)/t3-code`.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/desktop/src/preview/{Manager,BrowserSession,FaviconCapture}.ts`,
`apps/web/src/browser/{ElectronBrowserHost,HostedBrowserWebview,desktopTabLifetime,webviewCrashRecovery,previewRuntimeTabId}.ts(x)`,
`apps/web/src/components/preview/{PreviewView,PreviewChromeRow,PreviewMoreMenu,PreviewEmptyState,PreviewUnreachable,addBrowserSurface,openPreviewSession,closePreviewSession,usePreviewSession,usePreviewBridge}.ts(x)`,
`apps/web/src/previewStateStore.ts`, `apps/web/src/rightPanelStore.ts`, `apps/web/src/components/RightPanelTabs.tsx`,
`packages/shared/src/preview.ts`, `packages/contracts/src/preview.ts`, `apps/server/src/preview/Manager.ts`.

How part 1 is built (the seams the later parts extend):
- **Data module.** `browser-url.ts` (`normalizePreviewUrl`, the load-failed words), `browser-state.ts`
  (`previewStateStore` as one store per client: sessions per thread, the closing tabs a stale list cannot bring back,
  the server's epoch and revision, the module's page state per tab; `previewRuntimeTabId`), `browser-surface.ts` (opening
  and closing tabs through `preview.open`/`.close`, `preview.list` once per thread and connection, the panel's browser
  surfaces and their reconcile, the chrome row's ops `surface-browser-*`, the page state mirrored into the store and
  reported with `preview.reportStatus` as `usePreviewBridge` dedupes it, the live set sent to the module, and the panel's
  projection). `r4-surfaces-panel.ts` routes `browser` and `b`, and its tabs and close hook.
- **Views.** `browser-surface.contract` (the body, the chrome row, the More menu, the empty state, the load-failed page,
  the tab's favicon, the "+" menu's Browser row and profile submenu), `browser-shapes.contract`; `r4-surfaces.contract`
  draws them; `shell-panels.contract`'s `SurfacePanel` holds the URL field's focus, so its Escape is the field's.
  `app.contract` gained no line (the key gate's `b`/`B` are on its existing line): 1,230 lines.
- **Module.** `T3BrowserSessions.swift` (one page per live session of every thread, made from `browserSync`, closed when
  the list drops it; the profile stores; `presentation.browserTabs`; `t3.browser:` log lines), `T3BrowserSession.swift`
  (the `WKWebView`, its states, commands, failures, crash recovery, pop-ups, permissions), `T3BrowserFavicon.swift`
  (icon candidates from a user script in the app's own content world), `T3BrowserView.swift` (the `t3-browser` view:
  borrows the page while mounted, gives it back when the panel hides or the tab changes), `T3Module+Browser.swift`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | [X1 embedded browser engine](../issues/closed/20261005-x01-chromium-cdp-browser-surface.md) | [#100](https://github.com/ccheever/exact2/issues/100), closed not planned 2026-10-08 | Path B, a `WKWebView` in the clone's module (user decision, 2026-10-08) | decided |
| merged task PR | [20261008-app-contract-root-rewrite](20261008-app-contract-root-rewrite.md) | [#332](https://github.com/ccheever/exact2/pull/332) | Merged: the surface adds root resources and state, and the root was at its line cap | merged into `feat(example)/t3-code` as `f633671b7`; this branch starts there |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | #99 | Merged | not a blocker for part 1 (the branch builds on the feature branch's adopted main) |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | — | Merged into the T3 branch | merged |
| framework issue | [X66](../issues/20261008-x66-popover-from-action-and-toggle.md) | #319 | nonblocking: the "+" menu's profile submenu opens from its chevron, not on hover | declared in `EXACT2-GAPS.md` |
| not a prerequisite | [20261005-desktop-oracle-and-trace](closed/20261005-desktop-oracle-and-trace.md) | none | Not built (user decision, 2026-10-06): oracle and trace rows are recorded "not run — user decision 2026-10-06" | — |

## Acceptance and reproduction (part 1)

Rows from the original table that part 1 owns (the others moved with their scope to parts 2–5). Lane: agent mode at
1280×840, light; `T3_LOCAL_HOME=<lane>/t3-home` (seeded with two projects by the runtime's `t3 project add`),
`T3_LOCAL_PORT=16650`, `T3_LOCAL_RUNTIME_DIR` (the staged runtime, unpacked), isolated `CODEX_HOME`, `CLAUDE_CONFIG_DIR`,
`XDG_*`, `T3CODE_TELEMETRY_ENABLED=false`, `PATH=<lane bin>:<bun>:/usr/bin:/bin:/usr/sbin:/sbin`; a lane fixture server
on 127.0.0.1:16651 (`/`, `/a`, `/b`, `/slow?ms=`, a favicon); a closed port 16699 for the failure; `example.com` for a
public page.

| Criterion | Action | Expected | Result |
| --- | --- | --- | --- |
| Ported tests | `bun test examples/t3-code` | The ported tests pass | pass: see "Tests" |
| Open and tabs (part 1's half) | "+" menu › Browser (B); open two tabs; switch; close one; hide and show the panel; switch threads | Tab titles and favicons follow the pages; tabs keep their pages across hide and thread switch; closing ends the session | Live (drive 2): Browser row available, a tab opens as `tab_1` with "Browser" and the globe, `t3.browser: open [env, thread, epoch, tab_1]`. Titles, favicons, hide/show, thread switch and close: AppKit (`testATabOutlivesItsViewAndClosesWithItsSession`, `testAnIdleTabLoadsAPageAndReportsItsStates`) and Bun (`desktopTabLifetime (through the panel)`, `RightPanelTabs: a browser tab`); live: not reached (see Attempts) |
| Chrome row | Back, Forward, Refresh, Stop during a slow load, URL entry (bare host, `localhost:5173`, full URL, invalid), Open in system browser | States, tooltips and normalization as the reference; invalid input does nothing (`handleSubmitUrl` catches); the system browser gets the URL | Logic: Bun (`normalizePreviewUrl` 9 + `handleSubmitUrl`, `Back, Forward, …`); engine: AppKit (`testBackForwardRefreshAndHardReload`, `testRefreshWhileLoadingAsksForThePendingPageAgain`); live: the chrome row draws (image 03), the field takes text; navigation from the field was not reached: the drive found the bug fixed after it (Attempts) |
| Empty, loading, failed | No tabs; then a slow load, a closed port | "No preview yet"; the loading bar; LoadFailed with code and description | Empty: live (image 02); loading and failed: AppKit (`testALoadFailureIsReportedUntilTheNextLoad`: `ERR_CONNECTION_REFUSED`, -1004; `testAMissingPageIsAPageNotAFailure`) and Bun (`the chrome row and the native host`) |
| More menu (part 1's half) | Open the menu | Hard reload works; the rest is listed, disabled, with its part | Tree read back in drive 2: Hard reload enabled once the page exists, the six other rows disabled; Hard reload: AppKit |
| Page inspection | A development build, then a release build | Development: inspectable; release: not | `isInspectable=true (development build)` read back from the app (`t3.inspection: browser web view …`, drive 2) and AppKit; release: the gate is #326's, tested there; Safari's Develop menu attaching: real-input batch |
| Permissions and popups | A page asking for the camera; a scripted pop-up; a `target=_blank` link | Camera denied; the pop-up opens a window that keeps its opener and refuses its own; the link loads in the tab | pass: AppKit (`testTheCameraAndMicrophoneAreDenied`, `testScriptedPopupsOpenAWindowAndBlankLinksStayInTheTab`, the three `previewWindowOpenAction` tests) |
| Crash and recovery | Kill the tab's WebKit content process | Recovery as `webviewCrashRecovery` | pass: AppKit (`testACrashedPageRecoversAtItsURL`: SIGKILL on the content process, reload at the URL; the two ported plan tests) |
| User agent | Read `navigator.userAgent` | WebKit's own, unchanged | pass: AppKit (`testThePageKeepsWebKitsNativeUserAgent…`) |
| Real input `(attended session)` | Typing in the field (Korean 2-Set), Escape in it, the hover reveal, clicks in the page, a real OAuth pop-up | As the reference | deferred to the real-input batch ("Real-input batch steps") |
| Standard gates | Clone checks, `bun scripts/caps.mjs`, the five repository checks | Green | see "Checks" |

Tests (`bun:test`; reference names unless marked "clone"): `browser-url.test.ts` 17 (preview.test.ts's
`isLoopbackHost` 9, its `it.each` rows, and `normalizePreviewUrl` 6; clone: the load-failed page's words 2),
`browser-state.test.ts` 32 (`previewStateStore.test.ts` 26, `previewRuntimeTabId` 3, `shouldShowPreviewEmptyState` 2,
clone `readSnapshot` 1), `browser-surface.test.ts` 18 (`openPreviewSession` 3 of 4, `addBrowserSurface` 2,
`closePreviewSession` 2; clone rows for reconcile, tab title and favicon, `projectDesktopState`, `buildReportInput`,
profiles, the chrome row's ops, the live set, and `desktopTabLifetime` through `panelView`). AppKit
`macos/tests/browser` 20 (`webviewCrashRecovery` 2 and `previewWindowOpenAction` 3 ported; 15 against a loopback
fixture). Substitutions are in each file's header. Not ported here (owned by parts 2–5): the rest of the original
list.

## Progress

2026-10-08: #100 closed upstream as not planned; the user chose path B (a `WKWebView` in the clone's module).
2026-10-09: `prepare` split the ticket into five parts (four planned records written) and part 1 was built on
`f633671b7` (the root rewrite), merged with `3c7b35984` (#333). Two live sessions (the limit); the second found that a
navigation from the URL field never reached the page, fixed after the drive (Attempts); a third session to drive the
navigation rows is asked of the coordinator.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Drive 1 (agent mode) | branch sources before the Contract fix | Stopped at op 5: the app showed the first-run wizard; the lane home was empty (no `T3_LOCAL_RUNTIME_DIR`, the embedded runtime was still unpacking) and had no project | image `00-start` (not uploaded) | lane setup, fixed for drive 2 |
| Drive 2 (agent mode, the retry) | same build | Ops 1–53 of 92: the launcher's Browser row available; `tap surface-browser` opened `tab_1` (module `t3.browser: open`, `t3.inspection: browser web view isInspectable=true (development build)`); the chrome row, the More menu rows (tree) and "No preview yet" drawn. Typing an address and Enter sent nothing to the page (no request reached the fixture, `Refresh` stayed disabled); the drive stopped at op 54 (`tap browser-failed-reload`: no failed page) | drive record; images 01–03 | **Found and fixed after the drive:** the field's focus and blur sent `surface-browser-url-focus` through the window's `chatLocal` mutation, so the blur after Enter let go of the navigation just sent through it (a newer send forgets the reply, and `letGoAware` rejects that answer's native calls). The focus is now view state in `SurfacePanel` (no send), and text that arrives before the focus is kept as the draft. Not re-driven: session limit |

## Real-input batch steps

Deferred to the real-input batch — screen locked (user away) at the time of writing, and the live session limit. Build:
`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle` (after
`terminal-host/build.mjs` and `stage-runtime.mjs`); copy the bundle as "T3 Code (Lane Browser)" with its own bundle id;
launch with the lane environment above (`--env PATH=<lane bin>:/usr/bin:/bin:/usr/sbin:/sbin`), the fixture server on
16651, a seeded home.
1. **Navigation rows (also needed in agent mode: one more session).** Right panel › Browser; type `127.0.0.1:16651`,
   Return: "Lane fixture", title in the tab, the red favicon. `127.0.0.1:16651/a`, `/b`; Back → Page A (Forward
   enabled); Forward → Page B. `127.0.0.1:16651/slow?ms=9000`: the loading bar, the button reads Stop and "Loading…"; press
   it: the load restarts (fixture log: a second `GET /slow`). `example.com`: "Example Domain", the public favicon. Hover
   the address: Open in system browser; press it: the default browser opens `https://example.com/` (agent mode: the URL
   lands in `T3_REMOTE_OPEN_LOG`). `127.0.0.1:16699`: "This site can't be reached", "127.0.0.1:16699: Connection
   refused.", `ERR_CONNECTION_REFUSED`; Reload: the same. More: Hard reload (fixture: `GET /a` after `/a`). "+" › Browser
   (a second tab); hide and show the panel; select the first tab: Page A without a new request; close it: `t3.browser:
   close …`, the server's `preview.list` no longer names it. "+" › chevron › Default: a third tab. The drive script is
   `target/browser-surface/drive.sh` (copy: evidence branch `browser-surface/drive.sh.txt`).
2. **Keyboard in the field.** Click the field (all of it selected), type with the Korean 2-Set source switched to ABC,
   Return; then type, Escape: the URL comes back and the panel stays open; Tab through the chrome buttons (focus ring
   on each).
3. **Safari Web Inspector.** Safari › Develop › the lane app › the tab's page: the inspector attaches (development
   build); a release bundle does not list it.
4. **A real pop-up.** A page's "Sign in" `window.open(url, name, "width=500,height=600")` from a click: a window opens
   and posts back to its opener; a `target=_blank` link loads in the tab.

## Next action

The coordinator: one more live session for the navigation rows (step 1 above, agent mode is enough), then the real-input
batch steps 2–4. Parts 2–5 start after this PR merges.
