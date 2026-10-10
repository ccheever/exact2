---
name: 20261005-browser-surface
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface
pr_url: https://github.com/ccheever/exact2/pull/337
verified_commit: d5ba74653bada10a334aa4d79e09eb1584559f6b
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
- [part 3, capture](../20261005-browser-surface-capture.md): Annotate, screenshots, recording, downloads, the separate
  window and the floating player;
- [part 4, profiles](20261005-browser-surface-profiles.md): Incognito and named profiles, cookie import, Clear cookies
  and Clear cache;
- [part 5, automation](20261005-browser-surface-automation.md): the `previewAutomation.*` host, preview events, links
  from chat and terminal, and Mute.

Engine (user decision, 2026-10-08, after [#100](https://github.com/ccheever/exact2/issues/100) was closed upstream as not planned): **path B of
[X1](../../issues/closed/20261005-x01-chromium-cdp-browser-surface.md)**, a `WKWebView` in the clone's module.
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
[navigation](20261005-browser-surface-navigation.md); (3) [capture](../20261005-browser-surface-capture.md); (4)
[profiles](20261005-browser-surface-profiles.md); (5) [automation](20261005-browser-surface-automation.md). Each later
part starts after this one merges into `feat(example)/t3-code`.

## Context and guidance

Parent specification: [spec](../../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/desktop/src/preview/{Manager,BrowserSession,FaviconCapture}.ts`,
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
| recorded decision | [X1 embedded browser engine](../../issues/closed/20261005-x01-chromium-cdp-browser-surface.md) | [#100](https://github.com/ccheever/exact2/issues/100), closed not planned 2026-10-08 | Path B, a `WKWebView` in the clone's module (user decision, 2026-10-08) | decided |
| merged task PR | [20261008-app-contract-root-rewrite](20261008-app-contract-root-rewrite.md) | [#332](https://github.com/ccheever/exact2/pull/332) | Merged: the surface adds root resources and state, and the root was at its line cap | merged into `feat(example)/t3-code` as `f633671b7`; this branch starts there |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | #99 | Merged | not a blocker for part 1 (the branch builds on the feature branch's adopted main) |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | — | Merged into the T3 branch | merged |
| framework issue | [X66](../../issues/closed/20261008-x66-popover-from-action-and-toggle.md) | #319 | nonblocking: the "+" menu's profile submenu opens from its chevron, not on hover | declared in `EXACT2-GAPS.md` |
| not a prerequisite | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | none | Not built (user decision, 2026-10-06): oracle and trace rows are recorded "not run — user decision 2026-10-06" | — |

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
| Open and tabs (part 1's half) | "+" menu › Browser (B); open two tabs; switch; close one; hide and show the panel; switch threads | Tab titles and favicons follow the pages; tabs keep their pages across hide and thread switch; closing ends the session | Live (drive 2): Browser row available, a tab opens as `tab_1` with "Browser" and the globe, `t3.browser: open [env, thread, epoch, tab_1]`. Live ([drive 3](https://raw.githubusercontent.com/ccheever/exact2/b3f0a2582919aa7cbf99fd6bcfd6230acbef9fab/browser-surface/drive3-record.txt)): the tab's title follows the page ("Page A", "Page B", "Example Domain") and the lane page's favicon is captured (`browser-tab-favicon`). Hide/show, thread switch and close: AppKit (`testATabOutlivesItsViewAndClosesWithItsSession`) and Bun (`desktopTabLifetime (through the panel)`); pass by real input ([session 1](https://raw.githubusercontent.com/ccheever/exact2/977b726e5e447108b59e4711fa3d1cff3e5ddb46/browser-surface/real1-record.txt)): a second tab from "+" › the chevron › Default ([r1-06](https://raw.githubusercontent.com/ccheever/exact2/96be0413057c3b6a19db9511e298bc75635c7ee4/browser-surface/r1-06-profile-submenu.png)); switching tabs and hiding and showing the panel ask for nothing again and open no session ([r1-09](https://raw.githubusercontent.com/ccheever/exact2/665d4def69fa7d21ed42785f242a340f2baf7c34/browser-surface/r1-09-hide-show.png)); the chip's × closes a tab (`t3.browser: close … tab_1`, [r1-10](https://raw.githubusercontent.com/ccheever/exact2/cdcf68a4ca1750c226ab6c9315f13956e4cd6002/browser-surface/r1-10-switch-close.png)). A live thread switch was not driven (the lane's only thread is a draft, with no other thread to come back from); Bun and AppKit cover it |
| Chrome row | Back, Forward, Refresh, Stop during a slow load, URL entry (bare host, `localhost:5173`, full URL, invalid), Open in system browser | States, tooltips and normalization as the reference; invalid input does nothing (`handleSubmitUrl` catches); the system browser gets the URL | pass live ([drive 3](https://raw.githubusercontent.com/ccheever/exact2/b3f0a2582919aa7cbf99fd6bcfd6230acbef9fab/browser-surface/drive3-record.txt)): `127.0.0.1:16651` → `http://127.0.0.1:16651/` (`t3.browser: navigate`, fixture `GET /`), `/a`, `/b`; Back → Page A with Back and Forward enabled, Forward → Page B (Forward disabled); during `/slow` the button reads "Stop" and pressing it asks for the page again (a second `GET /slow`); `example.com` → `https://example.com/`, "Example Domain"; Open in system browser recorded `https://example.com/`. Real input: a pasted address and a real Return load the page ([r1-01](https://raw.githubusercontent.com/ccheever/exact2/b126c802bbfc464a8f6d85dac8c12c2780f27383/browser-surface/r1-01-paste-return.png)); a real click on Open in system browser opened the Mac's default browser at the page (fixture `GET /` at 01:23:32Z; the browser tab it opened was closed again). Also Bun and AppKit (`testBackForwardRefreshAndHardReload`, `testRefreshWhileLoadingAsksForThePendingPageAgain`) |
| Empty, loading, failed | No tabs; then a slow load, a closed port | "No preview yet"; the loading bar; LoadFailed with code and description | pass live: empty (drive 2, image 02); loading (Stop and the loading bar, [drive 3](https://raw.githubusercontent.com/ccheever/exact2/b3f0a2582919aa7cbf99fd6bcfd6230acbef9fab/browser-surface/drive3-record.txt)); failed ([drive 3](https://raw.githubusercontent.com/ccheever/exact2/b3f0a2582919aa7cbf99fd6bcfd6230acbef9fab/browser-surface/drive3-record.txt)): "This site can’t be reached", "127.0.0.1:16699: Connection refused.", `ERR_CONNECTION_REFUSED`, Reload pressed. Also AppKit and Bun |
| More menu (part 1's half) | Open the menu | Hard reload works; the rest is listed, disabled, with its part | Tree read back in drive 2: Hard reload enabled once the page exists, the six other rows disabled; drive 3 opened the menu; Hard reload by a real click: `t3.browser: hard-reload`, fixture `GET /b` ([r1-05](https://raw.githubusercontent.com/ccheever/exact2/d32372e917979b0eaf1dd2b3c1b453c7f95fb4a0/browser-surface/r1-05-hard-reload.png)); AppKit `testBackForwardRefreshAndHardReload`. Its longest row ran under its part note: fixed ([r2-02](https://raw.githubusercontent.com/ccheever/exact2/78b67b590314cf78f90e3fba21cebbfb0ba36c1b/browser-surface/r2-02-more-menu.png)) |
| Page inspection | A development build, then a release build | Development: inspectable; release: not | `isInspectable=true (development build)` read back from the app (`t3.inspection: browser web view …`, drive 2) and AppKit; release: the gate is #326's, tested there. Safari attaching, by real input: Develop › this Mac lists "T3 Code (Lane Browser)" › "127.0.0.1 — b"; its Web Inspector opens on the page and its console runs there (`document.title` is "Page B"; the page's body repainted in the app) ([r1-11](https://raw.githubusercontent.com/ccheever/exact2/cdd16ff9e68e76836af490b14b3bd689577380b4/browser-surface/r1-11-safari-inspector.png)); Safari's setting was on from 10:36:49 to 10:38:13 KST, off again (read back) and Safari quit; the user turned it on again afterwards for their own use ([r1-12](https://raw.githubusercontent.com/ccheever/exact2/07d827317a97f7b1b13653840886ee642e162553/browser-surface/r1-12-safari-setting.png)) |
| Permissions and popups | A page asking for the camera; a scripted pop-up; a `target=_blank` link | Camera denied; the pop-up opens a window that keeps its opener and refuses its own; the link loads in the tab | pass: AppKit (`testTheCameraAndMicrophoneAreDenied`, `testScriptedPopupsOpenAWindowAndBlankLinksStayInTheTab`, the three `previewWindowOpenAction` tests); real input: a click's `window.open('/auth', 'auth', 'width=440,height=360')` opens a 440×360 window, Approve posts back to the opener and the window closes ("Signed in: approved"); a `target=_blank` link loads in the tab, no window ([r1-07](https://raw.githubusercontent.com/ccheever/exact2/9cb7b3010c8308b66c30d4134dffc9470d376385/browser-surface/r1-07-popup.png), [r1-08](https://raw.githubusercontent.com/ccheever/exact2/eb8a005c6374eba5d9b8d3bf186d2338382a87bc/browser-surface/r1-08-blank-link.png)) |
| Crash and recovery | Kill the tab's WebKit content process | Recovery as `webviewCrashRecovery` | pass: AppKit (`testACrashedPageRecoversAtItsURL`: SIGKILL on the content process, reload at the URL; the two ported plan tests) |
| User agent | Read `navigator.userAgent` | WebKit's own, unchanged | pass: AppKit (`testThePageKeepsWebKitsNativeUserAgent…`) |
| Real input `(attended session)` | Typing in the field (Korean 2-Set), Escape in it, the hover reveal, clicks in the page, a real OAuth pop-up | As the reference | pass ([session 1](https://raw.githubusercontent.com/ccheever/exact2/977b726e5e447108b59e4711fa3d1cff3e5ddb46/browser-surface/real1-record.txt), 2026-10-09 10:22–10:39 KST; the Mac stayed on Korean 2-Set, text went in by paste): a pasted address and Return navigate ([r1-01](https://raw.githubusercontent.com/ccheever/exact2/b126c802bbfc464a8f6d85dac8c12c2780f27383/browser-surface/r1-01-paste-return.png)); Escape puts the URL back, the field lets go, the panel stays ([r1-02](https://raw.githubusercontent.com/ccheever/exact2/de954efe9ea3ad798e3a8fef853bb8497fcbaa82/browser-surface/r1-02-escape.png)); a real pointer reveals Open in system browser over the address and hides it off it ([r1-03](https://raw.githubusercontent.com/ccheever/exact2/cf1876330f687f53eef9b8c76d2301c7ca91856e/browser-surface/r1-03-hover-reveal.png)); Tab and Shift-Tab reach Back, Refresh, the field (all selected), Open in system browser and More, each with AppKit's ring, and skip the disabled ones ([r1-04](https://raw.githubusercontent.com/ccheever/exact2/f1ec6140237ede90d4771f5b7d60382e847090bf/browser-surface/r1-04-focus-rings.png)); clicks in the page follow links and open the pop-up. A click into a filled field puts the caret under the pointer rather than selecting all: the reference selects on focus (`queueMicrotask(select)`), which a Chromium click undoes the same way (a single click inside a selection collapses it at mouse-up); kept, and not compared against the reference app. Three findings, fixed and re-checked by real input ([fix check](https://raw.githubusercontent.com/ccheever/exact2/3e8f28d3789dc7633a61454597fda6940e1d1e8e/browser-surface/real2-record.txt)): Open in system browser stayed invisible while it had the keyboard focus ([r2-01](https://raw.githubusercontent.com/ccheever/exact2/cad5141787422022cfd7a095320cbb00689c6865/browser-surface/r2-01-external-focus.png)); the More menu's longest row ran under its note ([r2-02](https://raw.githubusercontent.com/ccheever/exact2/78b67b590314cf78f90e3fba21cebbfb0ba36c1b/browser-surface/r2-02-more-menu.png)); a long page title lost its start in the tab chip (X57, [r2-03](https://raw.githubusercontent.com/ccheever/exact2/024a0aa4e1c0fc3557559c288b2ca4745ccdabb8/browser-surface/r2-03-tab-title.png)) |
| Standard gates | Clone checks, `bun scripts/caps.mjs`, the five repository checks | Green | see "Checks" |

Tests (`bun:test`; reference names unless marked "clone"): `browser-url.test.ts` 17 (preview.test.ts's
`isLoopbackHost` 9, its `it.each` rows, and `normalizePreviewUrl` 6; clone: the load-failed page's words 2),
`browser-state.test.ts` 32 (`previewStateStore.test.ts` 26, `previewRuntimeTabId` 3, `shouldShowPreviewEmptyState` 2,
clone `readSnapshot` 1), `browser-surface.test.ts` 22 (`openPreviewSession` 3 of 4, `addBrowserSurface` 2,
`closePreviewSession` 2; clone rows for reconcile, tab title and favicon, `projectDesktopState`, `buildReportInput`,
profiles, the chrome row's ops, the live set, `desktopTabLifetime` through `panelView`, and the two let-go answers of
review round 1; two Contract-shape rows for the real-input fixes). AppKit `macos/tests/browser` 23 (`webviewCrashRecovery` 2 and `previewWindowOpenAction` 3 ported; 18
against a loopback fixture, three of them from review round 1). Substitutions are in each file's header. Not ported here (owned by parts 2–5): the rest of the original
list.

## Progress

2026-10-08: #100 closed upstream as not planned; the user chose path B (a `WKWebView` in the clone's module).
2026-10-09: `prepare` split the ticket into five parts (four planned records written) and part 1 was built on
`f633671b7` (the root rewrite), merged with `3c7b35984` (#333). Two live sessions (the limit); the second found that a
navigation from the URL field never reached the page, fixed after the drive (Attempts). Review round 1 fixed nine issues.
The coordinator allowed a third agent-mode session on the fixed build (screen locked): it passed the navigation rows and
stopped before Hard reload, hide/show and close, on the agent driver's `clock +N real` bug (#285, fixed on main by #304,
not yet in this branch).
2026-10-09 (later): the user asked for the remaining work and the ready flip. With the screen unlocked, the real-input
batch ran on a normal launch of a lane copy: every remaining row passed (Hard reload, hide/show and close among them),
Safari's setting was turned on for the inspector check and off again, and three findings were fixed and re-checked by
real input on the rebuilt bundle.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Drive 1 (agent mode) | branch sources before the Contract fix | Stopped at op 5: the app showed the first-run wizard; the lane home was empty (no `T3_LOCAL_RUNTIME_DIR`, the embedded runtime was still unpacking) and had no project | image `00-start` (not uploaded) | lane setup, fixed for drive 2 |
| Drive 2 (agent mode, the retry) | same build | Ops 1–53 of 92: the launcher's Browser row available; `tap surface-browser` opened `tab_1` (module `t3.browser: open`, `t3.inspection: browser web view isInspectable=true (development build)`); the chrome row, the More menu rows (tree) and "No preview yet" drawn. Typing an address and Enter sent nothing to the page (no request reached the fixture, `Refresh` stayed disabled); the drive stopped at op 54 (`tap browser-failed-reload`: no failed page) | drive record; images 01–03 | **Found and fixed after the drive:** the field's focus and blur sent `surface-browser-url-focus` through the window's `chatLocal` mutation, so the blur after Enter let go of the navigation just sent through it (a newer send forgets the reply, and `letGoAware` rejects that answer's native calls). The focus is now view state in `SurfacePanel` (no send), and text that arrives before the focus is kept as the draft. Not re-driven: session limit |

### Independent review, round 1 (2026-10-09)

A read-only review of the branch found nine issues; all are fixed in the follow-up commit, each with a test where one
can run without the app:
1. The close hook kept the native of the first answer that installed it; once that answer was let go, every later
   close threw before `preview.close` (and "Close all" stopped part way). Now installed again by every answer, as the
   terminal's (test: "closing a tab calls through the closing answer …").
2. A tab closed while its favicon downloaded could ask an invalidated `URLSession` for the next candidate (an
   exception). Now cancelled for good (AppKit `testAFaviconFetchCancelledWithItsTabDoesNotRunOn`).
3. A failed load was reported to the server on every projection (the reference reports per desktop state change). Now
   once per failure: the module counts failures (`failures`), `buildReportInput` dedupes on it.
4. The page could stay Loading: a fragment jump (no provisional navigation) and a refused download behind a redirect
   kept the pending URL. Now a fragment jump sets none, the pending URL follows a redirect, and a load that ends
   without a navigation in flight ends it (AppKit `testAFragmentJumpAndARedirectedRefusedDownloadDoNotStayLoading`).
5. A `preview.list` started by another answer was shared; if that answer was let go, the waiting one (opening a tab)
   failed silently. Now it lists again for itself (test: "a listing another answer started and lost …").
6. One process-wide page registry: two sessions in one process (the sample host's) would close each other's pages.
   Now one per module (`T3Module.browserSessions`, `T3BrowserSessionOwner`) (AppKit `testTwoSessionsKeepTheirOwnPages`).
7. Server sessions could add Browser tabs before the saved panel was restored, so `restoreRightPanel` skipped it and the
   saved Files/Diff tabs were lost. The reconcile now waits for `client.ready` (restore runs first in `panelView`).
8. A pop-up shares the tab's configuration, so the favicon script ran in it and could cancel the tab's icon. Messages
   from any page but the tab's own are ignored.
9. The URL field's focus could outlive the field (a tab switch without a blur), keeping Escape from the panel toggle;
   and one tab's draft carried over to another. The focus is now kept by tab, and the chrome row is mounted per tab.

| Drive 3 (agent mode, coordinator go-ahead, screen locked) | `cf53202c4` (bundle rebuilt) | Ops 1–54 of 71: the lane page, `/a`, `/b`, Back and Forward, Stop during `/slow` (a second `GET /slow`), `example.com`, Open in system browser (recorded), the failed page for 16699 and its Reload, `/a` again, the More menu opened. Op 55 (`clock +1000 real`) refused by the agent driver: "the clock cannot go backwards (73800.0 → 73799.99999999999)" | [drive 3](https://raw.githubusercontent.com/ccheever/exact2/b3f0a2582919aa7cbf99fd6bcfd6230acbef9fab/browser-surface/drive3-record.txt) (trees, host logs, fixture log; agent captures are blank while the screen is locked) | Hard reload, hide/show and close not reached: exact2 [#285](https://github.com/ccheever/exact2/issues/285), fixed on main by #304, adopted with round 7 (blocked by #320); AppKit and Bun cover them |
| Real-input session 1 (normal launch, the user's go-ahead) | `cf53202c4` bundle, copied as "T3 Code (Lane Browser)" (own bundle id), launched by path in the lane environment ([lane-app.sh](https://raw.githubusercontent.com/ccheever/exact2/a1beeefc46c91a000532d1d4c3d11b87f74cb52f/browser-surface/lane-app.sh.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/1ff038d45e90cd7bfb24679d1bb547f52cf7d640/browser-surface/fixture-r.mjs.txt)) | Every remaining row passed: paste and Return, Escape, the hover reveal, Tab and its rings, Hard reload, "+" › chevron › Default, hide/show, switch and close, a real pop-up and a `target=_blank` link, Safari's Web Inspector. Open in system browser, clicked for real, opened the Mac's default browser (its tab closed again at once). Three findings: fixes 1–3 below | [record](https://raw.githubusercontent.com/ccheever/exact2/977b726e5e447108b59e4711fa3d1cff3e5ddb46/browser-surface/real1-record.txt); r1-01 to r1-12 | fixes 1–3 |
| Real-input fix check | the branch with fixes 1–3 (`d5ba74653`), bundle rebuilt | Fix 1: Open in system browser shows with its ring under keyboard focus, pointer away, and hides when the focus leaves; fix 2: the More menu holds its longest row, end-aligned, Escape closes it with the ring on More; fix 3: "Pop-up test: sign…" keeps its start | [record](https://raw.githubusercontent.com/ccheever/exact2/3e8f28d3789dc7633a61454597fda6940e1d1e8e/browser-surface/real2-record.txt); r2-01 to r2-03 (before and after) | none |

## Real-input batch steps

Done on 2026-10-09 (Attempts: real-input session 1 and the fix check). For a rerun: build with
`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle` (after `terminal-host/build.mjs`
and `stage-runtime.mjs`); copy the bundle as "T3 Code (Lane Browser)" with its own bundle id and launch its executable
with the lane environment ([lane-app.sh](https://raw.githubusercontent.com/ccheever/exact2/a1beeefc46c91a000532d1d4c3d11b87f74cb52f/browser-surface/lane-app.sh.txt): the seeded home, `T3_LOCAL_PORT=16650`,
`T3_LOCAL_RUNTIME_DIR`, `HOME` and `CFFIXED_USER_HOME` in the lane dir, the lane PATH), the fixture on 16651
([fixture](https://raw.githubusercontent.com/ccheever/exact2/1ff038d45e90cd7bfb24679d1bb547f52cf7d640/browser-surface/fixture-r.mjs.txt): `/`, `/a`, `/b`, `/slow`, `/popup`, `/auth`, `/long`). Take the real-input lock.
Click by window coordinates (`orca computer click`), put text in with `paste-text` (never switch the Korean 2-Set
source; a click in a filled field leaves a caret, so select all first or Tab into it), move the pointer with
`cliclick m:x,y` (screen coordinates), and capture a focus ring about 0.9 s after the key (AppKit fades it in).
Read effects back from the fixture log, the host's `t3.browser:` lines and the AX focus. On a normal launch Open in
system browser really opens the Mac's default browser: close the tab it opens. Safari: open it, Settings › Advanced ›
"Show features for web developers" on (approved for #326 and here), Develop › this Mac › "T3 Code (Lane Browser)" ›
the page, then the setting off (read back) and quit Safari if it was not running. After: quit the copy (an Apple
Event quit stops its server), stop the fixture, `defaults delete com.exact.t3code.macos.lanebrowser`, release the lock.

## Next action

2026-10-09 (records sync, `t3-code-records-337`): merged into `feat(example)/t3-code` as #337 (`dce6d78df`); the record moved to `tasks/closed/`. Every real-input row passed before the merge; parts 2–5 start from here.

None for part 1: review and merge #337. Parts 2–5 start after it merges.
