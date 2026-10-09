---
name: 20261005-browser-surface-navigation
plan: 20261005-t3code-macos-parity
implementation: verified
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-navigation-v2
pr_url: https://github.com/ccheever/exact2/pull/352
verified_commit: ab220bfdb3cf5c81636591ff89f0f3ca1c37b250
---

# Browser surface part 2: navigation, history, zoom and the device toolbar

## Outcome

The Browser tab built in [part 1](closed/20261005-browser-surface.md) gains the reference's navigation aids: the empty state's
"Recently used" and "Local servers", the full unreachable page, the history store, target resolution, page zoom, the
appearance preference sent to the page, the device toolbar (fill, freeform, the 17 presets, rotate, resize handles) and
the preview keys. The More menu's Show/Hide device toolbar, Appearance and Zoom rows, disabled and marked "Part 2" by
part 1, work. Settings › Integrations › Browser's default viewport, zoom and appearance rows (item 10) moved to
[browser-surface-profiles](20261005-browser-surface-profiles.md) (part 4); until then new tabs open at the reference's
defaults (fill, 100%, System).

Split from [20261005-browser-surface](closed/20261005-browser-surface.md) at its `prepare` (planned split, part 2). It starts
after part 1 merges into `feat(example)/t3-code`; the engine (`T3BrowserSession.swift`), the state store
(`browser-state.ts`) and the chrome row (`browser-surface.contract`) are part 1's.

## Scope and exclusions

From the parent's scope (its numbering):

3. **More menu, part 2 rows.** Show or Hide device toolbar, Appearance (System, Light, Dark; the radio group), and the
   Zoom out / in / reset row (buttons keep the menu open) (`PreviewMoreMenu.tsx:111-228`). Hard reload is part 1's; Open
   or Close separate preview window is part 3's; the Profile group is part 4's.
4. **Navigation states, the rest.** The unreachable page's Details toggle and checklist ("Checking your connection",
   "Confirming the dev server is running", "Checking the proxy and the firewall", `PreviewUnreachable.tsx:38-40`), and
   the empty state's "Recently used" (at most 8, each removable) and "Local servers" ("Select a live local server to open
   it in this browser tab.") (`PreviewEmptyState.tsx:25-110`, `PreviewRecentUrlCard.tsx`, `PreviewLocalServerCard.tsx`).
   Part 1 has Idle, Loading, Success, LoadFailed, the loading bar, the empty state's "No preview yet" and the
   load-failed page's heading, words, error name and Reload.
5. **History and discovery.** `browserHistoryStore` (50 entries per project, 20 projects, URL up to 2,048 characters,
   title up to 512; `browserHistoryStore.ts:11-16`), `recordVisitForThread` on submit, the server's discovered local
   servers (`subscribeDiscoveredLocalServers`, `useDiscoveredLocalServers`, configured `previewUrl`s from project
   scripts: `getConfiguredPreviewUrls`), target resolution for loopback and environment ports (`browserTargetResolver.ts`).
6. **Viewport and zoom.** Fill, freeform or one of the 17 device presets (`packages/shared/src/previewViewport.ts:20-143`),
   240 to 3,840 px, area at most 3,840×2,160, resize handles, rotate, the device toolbar (`BrowserDeviceToolbar.tsx`,
   `BrowserViewportResizeHandles.tsx`, `browserViewportLayout.ts`, `preview.resize`), the zoom ladder 25% … 500%
   (`preview.ts:127-134`; WebKit `pageZoom`), the zoom indicator, and the appearance preference (`prefers-color-scheme`
   emulation; WebKit: the web view's `appearance` or an injected override, confirmed at `prepare`). Keys: ⇧⌘J toggles the
   surface, and while the preview has focus ⌘R refresh, ⌘L focus the URL field, ⌘= / ⌘+ zoom in, ⌘- zoom out, ⌘0 reset
   (`packages/shared/src/keybindings.ts:33-39`, context `previewFocus`; the page swallows keys, so the module forwards
   them as `PreviewKeyboard.ts` does).
10. **Settings rows, part 2's.** Default browser viewport, Default browser zoom, Default browser appearance
    (`settings-source-control.contract` BrowserDefaults, `settings.ts:314-357`); new tabs open with them
    (`browserDefaultOpenViewport`, `browserDefaultTabState`). **Moved to browser-surface-profiles (part 4)**
    (coordinator, 2026-10-09); the group stays as part 1 left it until then (inert, "Only available in the desktop app.").

Excluded: annotate, capture, recording, picture in picture (part 3); profiles, cookie import, clearing (part 4); the
`previewAutomation.*` host, links, Mute (part 5).

## Context and guidance

Parent specification: [spec](../spec.md); engine decisions and declared differences: the parent record and
`EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)". Reference: T3 Code `1e2ecbd975`
(`apps/web/src/browser/*`, `apps/web/src/components/preview/*`, `apps/web/src/browserHistoryStore.ts`,
`apps/server/src/preview/PortScanner.ts`). Part 1's seams: `browser-surface.ts` (`browserView`, `browserLocal`),
`browser-surface.contract` (`BrowserMore`, `BrowserEmpty`, `BrowserLoadFailed`), `T3BrowserSession.swift` (commands).
The More menu rows marked "Part 2" lose the mark when they work.

How part 2 is built (2026-10-09):
- **Data module.** `browser-history.ts` (browserHistoryStore as one store per client, saved with `client.local` as
  `browserHistory` and read back through `mergeBrowserHistoryState`; nothing registers a thread's project, as at
  `1e2ecbd975`), `browser-targets.ts` (browserTargetResolver over the client's connected origin;
  `mergeServers`, `boundConfiguredLocalServerUrls`, `getConfiguredPreviewUrls`; the `subscribeDiscoveredLocalServers`
  stream on the focused connection while a Browser tab shows its empty state, one dispatch line in `client.ts`),
  `browser-viewport.ts` (the presets, the layout, the commit queue, readiness and rollback for part 5, the zoom ladder,
  the toggle's responsive size), `browser-navigation.ts` (the ops `surface-browser-<op>` of part 2, the projection's part-2
  fields, ⇧⌘J's dispatch row, the `previewFocus` chords). A data source reads no clock or timer: visits are timed by the
  projection's wall time, and the commit queue's 15 s deadline is the caller's.
- **Views.** `browser-stage.contract` (the page area: fill, or the device toolbar over the fitted viewport with its five
  rails, the live drag size and the zoom pill), `browser-surface.contract` (the More menu's rows, the empty state's lists,
  the Details checklist; the page stays mounted under the load-failed page), `browser-shapes.contract`; the URL field's
  focus reaches the page through `shell-panels.contract` and `r4-surfaces.contract`; ⇧⌘J's close is
  `settings-shortcuts.contract`'s `panel-close`. `app.contract` gained no line (1,230).
- **Module.** `T3BrowserSession+Navigation.swift` (`pageZoom` on the ladder, stepped from the page's own zoom; the web
  view's `appearance`; the `browserSet` op), `T3BrowserView.swift` (`fit-scale`: the box's bounds scale the web view into the
  fitted footprint; the key monitor for the preview's chords), one line each in `T3BrowserSessions.swift` (the report) and
  `T3Module+Browser.swift` (the route). `T3BrowserSession.swift` is untouched.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](closed/20261005-browser-surface.md) (part 1) | [#337](https://github.com/ccheever/exact2/pull/337) | Merged into `feat(example)/t3-code` | merged as `dce6d78df` (2026-10-09) |
| merge order (coordinator) | [20261005-browser-surface-automation](20261005-browser-surface-automation.md) (part 5) | [#346](https://github.com/ccheever/exact2/pull/346) | Merges first; this PR then wires its `_resize` hook and the agent default | merged as `5f0ae7dca` (2026-10-09), merged in |
| framework issue | [X21](../issues/20261005-x21-two-way-websocket.md) | #126 | nonblocking: `subscribeDiscoveredLocalServers` is a stream; the Swift transport carries it until #126 | — |
| framework issue | [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | #140 | nonblocking: the preview keys are native monitors while the page has focus | — |

## Acceptance and reproduction

#348's acceptance as recorded before the revert (history). The re-land's, on #352, is in "Re-land acceptance" below.

Rows from the parent's table: "Empty, loading, failed, unreachable" (the Recently used, Local servers and Details
parts), "More menu" (the device toolbar, appearance and zoom rows), "Device toolbar", "Keys", and "States" for these
surfaces; lane rules and fixtures as the parent's. Tests to port (`bun:test`, original names; counts from the parent):
`browserHistoryStore.test.ts` (33), `browser/browserDefaults` (4), `browserTargetResolver` (19), `browserViewportActions`
(4), `browserViewportLayout` (12), `BrowserDeviceToolbar` (2), `browserSurfaceStore` (9), `previewEmptyStateLogic`
(`getConfiguredPreviewUrls`, 1), `useDiscoveredLocalServers` (11), `previewViewportReadiness` (3),
`previewViewportRollback` (2), `previewClickFocus` (8), `packages/shared` `previewViewport.test.ts`, `apps/desktop`
`PreviewKeyboard` (12). Standard gates as the parent's.

Lane (2026-10-09): agent mode at 1280×840, light; `T3_LOCAL_HOME` seeded by the runtime's `t3 project add` (`work`,
`site`), `T3_LOCAL_PORT=16700`, `T3_LOCAL_RUNTIME_DIR` (t3 v0.0.46-nightly.20261005.2667, staged and unpacked), isolated
`HOME`, `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_*`, `T3CODE_TELEMETRY_ENABLED=false`, the lane PATH; the fixture on
127.0.0.1:16701 (pages that print their CSS viewport, dpr and colour scheme); 16749 closed. The screen locked at about
04:04Z: agent window captures are blank, so the live proof is the agent's trees, the host's `t3.browser:` lines and the
command log ([drive record](https://raw.githubusercontent.com/ccheever/exact2/10258e0535b2ae3fd56edcf6b131f782eac36336/browser-surface-navigation/drive-record.txt)).

| Criterion | Action | Expected | Result |
| --- | --- | --- | --- |
| Ported tests | `bun test examples/t3-code` | The ported tests pass | pass (see "Tests") |
| Local servers | A new Browser tab | The live loopback servers, configured ones first; the hint; a row opens its server in the tab | pass live (drive ops 8–12): the fixture `localhost:16701` "bun" and the lane server "t3" listed with the hint; tapping 16701 navigates the tab (`t3.browser: navigate http://localhost:16701/`). Bun: `mergeServers` 13, the stream's lifecycle |
| Recently used | Visit pages; a second tab; remove one; open one | At most 8, newest first, title or address with the visit's age; × removes; a row opens | The list, its rows, removal and opening passed live (ops 95–109: four rows, Remove 127.0.0.1:16749 leaves three, Page A opens `http://localhost:16701/a`) on the build that still registered the thread's project. After the user's decision (match the reference) a newly opened thread's visits wait unregistered and its list stays empty, as at `1e2ecbd975`; the list shows for a thread with a saved project mapping. Bun: 35 history tests, `PreviewEmptyState` 4, the unregistered and saved-mapping rows |
| Unreachable Details | A closed port, then Details | The checklist and "Hide details" | pass live (op 23: `browser-failed-details`). Bun projection |
| History store and targets | — | 50 per project, 20 projects, URL ≤ 2,048, title ≤ 512; loopback and environment ports resolve as the reference | pass: Bun 35 + 21 (`browserTargetResolver` 19, `boundConfiguredLocalServerUrls` 2) |
| More menu | Open it; Zoom in twice; Appearance › Dark; Show device toolbar | Rows enabled; zoom row keeps the menu open; Dark checked | pass live (ops 27–51): 125% in the row and the pill, Dark checked from the module's report, the toolbar opens |
| Zoom | ⌘= ⌘0 in the page; the ladder | `pageZoom`, 25–500% | pass live (ops 79–83: 110%, then 100%); AppKit (`testZoomIsThePagesZoom…`: 600×400 → 300×200 CSS px at 200%, kept across a navigation, the ladder's ends); [image](https://raw.githubusercontent.com/ccheever/exact2/aacaccc709cc055658695b89f29e83336d859c34/browser-surface-navigation/page-zoom.png) |
| Appearance | Dark, Light, System | `prefers-color-scheme` follows | pass: AppKit (`testAppearanceIsWhatThePagePrefers`), [image](https://raw.githubusercontent.com/ccheever/exact2/035d46277da551356661101a27659ec39f997343/browser-surface-navigation/page-appearance.png); live Dark checked (op 45) |
| Device toolbar | Show; preset; rotate; lock; drag a rail; type a width; close | Sizes and clamps as the reference; the page keeps its CSS viewport when fitted | pass live for show (519 × 706), iPhone 12 Pro (390 × 844, fit 0.836), the east rail (+80 pt → 581 × 844), a typed width (500 × 844) and close (fill). Rotate and Lock were covered by the lane's "Updates Available" toast in the drive (its taps at x 1056 and 1030 landed on the toast region, x 888–1248): Bun (`chooses a preset, types a size, rotates, locks the ratio and closes`, `keeps the locked ratio on a drag`) and the real-input batch. AppKit (`testAFixedViewportScaledToFitKeepsItsCSSSize`), [image](https://raw.githubusercontent.com/ccheever/exact2/d20bb9c15cebb0627cfed2cecf477f3d35ef0fe8/browser-surface-navigation/page-fit.png); Bun layout 12, clamps |
| Keys | ⌘= ⌘0 ⌘L in the page; ⇧⌘J twice | As the reference's `previewFocus` chords and `preview.toggle` | pass live (ops 79–94: zoom, reset, the URL field focused and selected; ⇧⌘J hid and showed the panel on the same tab); AppKit (`testThePreviewsChordsAreAnswered…`: the monitor, Korean 2-Set, Dvorak, Shift-= as ⌘+, ⌘R/⌃R/⌃⌘R, a stale field focus claims nothing). Real keys: the real-input batch |
| Real input | Pointer drags, hover reveal, real keys, window images | As the reference | deferred to the real-input batch — screen locked (user away); steps below |
| Standard gates | Clone checks, caps, the five checks | Green | pass (see "Attempts and evidence") |

Tests: `browser-history.test.ts` 35 (`browserHistoryStore.test.ts` 33; clone 2), `browser-targets.test.ts` 38
(`browserTargetResolver` 19, `useDiscoveredLocalServers` `mergeServers` 13, `getConfiguredPreviewUrls` 1,
`portDiscoveryState` `boundConfiguredLocalServerUrls` 2; clone 3: the stream), `browser-viewport.test.ts` 30
(`previewViewport` 3, `browserViewportLayout` 12, `browserViewportActions` 4, `BrowserDeviceToolbar` 2,
`previewViewportReadiness` 3, `previewViewportRollback` 2; clone 4: the ladder, the toggle, the bounds),
`browser-navigation.test.ts` 20 (`PreviewEmptyState` 4; clone 16), AppKit `macos/tests/browser` 27 (part 1's 23 with two
of its title waits made race-free, and 4 for part 2). Not ported, with their owner: `browser/browserDefaults` (4: three
are profile resolution and one the settings read, all with part 4, which now holds item 10), `browserSurfaceStore` (9: the Electron slot's
positioning; the page is borrowed by `T3BrowserView`, part 1, and fit-to-source is part 3's mini player),
`previewClickFocus` (8: agent clicks, part 5), `apps/desktop` `PreviewKeyboard` (12: it builds the automation's key
events, `previewAutomation.press`, part 5; the preview chords here are the module's monitor, AppKit).

User decision 2026-10-09: match the reference (de34391427). **Recently used's project registration.** At
`1e2ecbd975` nothing calls `registerThreadProject` (#2829, `de34391427`, dropped ChatView's effect), so a thread's
visits wait in the store's pending queue and Recently used lists only the history of a thread whose project mapping was
saved before that change; newly opened threads do not fill it. The clone does the same: the registration it first
made (as `72d673a855` did) was removed; the list's UI, its limits, removal, the visits recorded on submit and on opening
a server, and the titles stay as the reference has them (Bun: `records a typed address, which waits unregistered as at
1e2ecbd975`).

## Re-land acceptance (#352, head 26f5c9066)

#348 was squash-merged as `46436acc3` a minute before Charlie's sweep asked to keep it open, reverted by #351 (`c603c22d6`), and
re-landed as draft [#352](https://github.com/ccheever/exact2/pull/352) (`91b6f9a83`, the same tree as `46436acc3`). Charlie's
checks ([comment](https://github.com/ccheever/exact2/pull/348#issuecomment-6076127296)) were run fresh on the re-land; the rows
in "Acceptance and reproduction" above are #348's and stay as history. The drives found defects, fixed on #352 in two commits
(below); checks 1-3, the first-layout probe and the eleven checks are on the code head `26f5c9066`. Later commits change only records.
Check 4 ran by real input on `878e605b8` and again on the code head `26f5c9066` (2026-10-09 13:39-13:41Z); it passed on both.

Lane (2026-10-09, run5 09:38-09:41Z): agent mode at 1280×840, light; the app's embedded T3 server on 16700 (`T3_LOCAL_HOME` a
fresh copy of a home seeded by the runtime's `t3 project add`, the staged t3 v0.0.46-nightly.20261005.2667, isolated `HOME`,
`CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_*`, telemetry off); the fixture on 16701 (pages print their CSS viewport and record the
size their first script ran at); the server's ACP Registry instance "Lane browser agent" runs a lane-only fake ACP agent that calls
the server's `preview_*` MCP tools for a prompt `bsn <scenario>` (part 5's approach; [agent](https://raw.githubusercontent.com/ccheever/exact2/46c7e6bc248d58987d203390acdca8fe6fe43fd3/browser-surface-navigation-v2/h26f5c9066-fake-acp.mjs.txt),
[drive](https://raw.githubusercontent.com/ccheever/exact2/eaa5314cbc74014bebbda0048c4dc678fadc8c49/browser-surface-navigation-v2/h26f5c9066-drive.sh.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/50f676e6aecf0ba50fbab523474f2b7b61704e00/browser-surface-navigation-v2/h26f5c9066-fixture.mjs.txt)). Every answer below is the MCP result the
agent received ([drive record](https://raw.githubusercontent.com/ccheever/exact2/cfcd0378e379bde2132f133f8529af643d4403b7/browser-surface-navigation-v2/h26f5c9066-drive-record.txt): tool results, the app's tree after each step, the server
trace, the fixture and host logs; [results](https://raw.githubusercontent.com/ccheever/exact2/6cc1f064d3ea0e19d47df46d009f8101200a2e4a/browser-surface-navigation-v2/results-run5.jsonl.txt)).

| Charlie's check | Result | Evidence |
| --- | --- | --- |
| 1. Live `preview_resize` through the server's MCP tools, and the 1280 × 800 default for agent-opened tabs | **pass (live, `26f5c9066`).** `preview_open` made tab_1 at `freeform 1280 × 800` and answered viewport 1280 × 800; the page measured 1280 × 800 at dpr 2, and its first layout was 1280 × 800; the device toolbar read 1280 × 800. `preview_resize` freeform 800 × 600, iPhone 12 Pro (390 × 844), iPhone 12 Pro landscape (844 × 390) and freeform 1024 × 768: each answer's setting and viewport, and the page's own measure, matched; the toolbar ended at 1024 × 768 | [image](https://raw.githubusercontent.com/ccheever/exact2/8fd9786790c255600accc4f5c1bc4323d26c2c28/browser-surface-navigation-v2/h26f5c9066-c1-open-and-resize.png), drive record |
| 2. An agent-requested resize while the panel is hidden | **pass (live, `26f5c9066`), two cases.** (a) The person hid the panel with a real ⇧⌘J key event (`preview_status` visible false); `preview_resize` 900 × 650 answered 900 × 650 and the page measured it. The resize showed the panel again, the clone's stand-in for the reference's floating preview (`shouldAutoShowPreviewForAutomationUse`, part 5), with the toolbar at 900 × 650. (b) With the panel hidden, the agent opened tab_2 with `open: false`: its first layout was 1280 × 800 in no stage, then freeform 1000 × 700 and iPad Mini (768 × 1024) were each answered and measured; `preview_status` visible false, and the panel stayed hidden (tree). When the person showed the panel (⇧⌘J shows the thread's latest tab, tab_2), its toolbar read iPad Mini 768 × 1024; tab_1 kept 900 × 650 | [image (a)](https://raw.githubusercontent.com/ccheever/exact2/b59ac84c8b0abeaaa4f22eb6dd6e113143ee196b/browser-surface-navigation-v2/h26f5c9066-c2a-hidden-panel.png), [image (b)](https://raw.githubusercontent.com/ccheever/exact2/841e1f9418a27b457dd3815d688c07946d4de9a6/browser-surface-navigation-v2/h26f5c9066-c2b-suppressed.png), drive record |
| 3. A rejected or timed-out resize restores the intended viewport | **pass (live, `26f5c9066`).** Rejected: freeform 3840 × 3840, 120 × 600, and fill with a size were refused by the server's tool schema (-32602, the reference's messages); tab_1 stayed 900 × 650 (status, page, toolbar). Timed out: freeform 500 × 900 with `timeoutMs` 25. The broker answered "Preview automation resize timed out after 25ms." and dropped the host's connection, as the reference's broker does. The host had set 500 × 900 (`preview.resize` at 09:40:14.209Z), its own 25 ms render wait ended, and it went back to 900 × 650 (`preview.resize` at .272, host `PreviewAutomationTimeoutError`); it reconnected at .260. Four seconds on, the setting, the page and the toolbar were 900 × 650. AppKit `testAResizeNotRenderedInTimeGoesBackToThePreviousSize`. A `preview.resize` the server refuses after the host sends it cannot be produced through the tools (the schema refuses first; the manager fails only for a missing session); the host then answers an execution error and has changed nothing | [image](https://raw.githubusercontent.com/ccheever/exact2/574de1f5932553af7ba24ab07533488e265c9bf2/browser-surface-navigation-v2/h26f5c9066-c3-reject-timeout.png), drive record (server trace) |
| 4. Rotate/Lock by real input after dismissing the updates toast | **pass (real input, `26f5c9066`).** 2026-10-09 13:39:08-13:41:31Z under the shared real-input lock: a normal launch of a lane copy of the `26f5c9066` build (`T3 Code (Lane Nav 1009b)`, embedded server on 16960, fixture on 16961, isolated homes, telemetry off), real clicks at window coordinates (`orca computer click`), read back from the accessibility tree. Dismissed the "Update Available: Codex v0.162.0" toast and the "Nightly needs the beta mobile app" toast behind it with their ×; a Browser tab on the fixture (539 × 748 at fill); More › Show device toolbar (Responsive 519 × 706, the page 519 × 706); iPhone 12 Pro (390 × 844, the page 390 × 844); **Rotate** → 844 × 390, still "iPhone 12 Pro", the page 844 × 390; **Lock** → "Unlock viewport aspect ratio", Value 1 (pressed), the size still 844 × 390. The same steps passed on `878e605b8` (08:38:51-08:41:13Z) | [image](https://raw.githubusercontent.com/ccheever/exact2/da9155a758604606a7d68b0bce3b7dc90ad49d57/browser-surface-navigation-v2/c4-rotate-lock-26f5c9066.png), [record](https://raw.githubusercontent.com/ccheever/exact2/05a75796fa6dbf344fa13622d2ff11e2ae7c3890/browser-surface-navigation-v2/realinput-26f5c9066.txt); `878e605b8`: [image](https://raw.githubusercontent.com/ccheever/exact2/5ee8d0f620b96077cb844ab9db511f753a905459/browser-surface-navigation-v2/c4-rotate-lock-878e605b8.png), [record](https://raw.githubusercontent.com/ccheever/exact2/c5c858aeb5a8e0fa9a72cc020e1f334c346d03c8/browser-surface-navigation-v2/realinput-878e605b8.txt) |
| Settings defaults; app-only cleanup | unchanged: the Settings default rows stay with part 4 (browser-surface-profiles), the app-only cleanup with #99 | — |

Fixed on #352 (each case failed before its fix; read-only review at each step, no blocking findings, its should-fix items done):
- In `878e605b8`, found by the drive on `91b6f9a83`:
  1. **`preview_open` answered the wrong viewport** for a new agent tab: 1578 × 1183 in three runs of four (the page measured
     1280 × 800 on the next call). A shown tab with a fixed size now waits, at most 2 s and within the request's deadline, until the
     page renders it. AppKit `testAnOpenedTabAnswersTheDefaultSizeItsStageRenders`, `testAReusedTabAnswersItsFixedSizeOnceItsStageRenders`
     ([before](https://raw.githubusercontent.com/ccheever/exact2/fc45e5f06b82550d166df732cfb2425a8fd78d19/browser-surface-navigation-v2/appkit-browser-automation-before-fix.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/a1d55aef1fc56f820ed68e56fe0570d4eebba217/browser-surface-navigation-v2/appkit-browser-automation-before-fix3.txt)).
  2. **A tab opened with `open: false` was shown** by the agent's next operation: the module's `opened` note carried `present`
     but not `suppress` (part 5). AppKit `testATabOpenedWithoutPresentationStaysSuppressed` ([before](https://raw.githubusercontent.com/ccheever/exact2/6658d3468358c486f5ebf94c30fff35d250454a0/browser-surface-navigation-v2/appkit-browser-automation-before-fix2.txt)); Bun.
- In `26f5c9066`, asked after part 4's drive (#354) saw a tab at a fixed default viewport lay its first page out at 512 × 384:
  3. **A page was not made at its tab's fixed size** (the STATUS "Found, not in scope" row of `e08f0b525`, now fixed).
     `browserSync` made every page at the web view's first 640 × 480 and loaded it at once, and the stage's box
     (`T3BrowserView.Host`) applied its fit scale before the layout sized it. Now `liveSessions` carries a fixed tab's width,
     height and zoom and the page is made at that size before its first load; the box holds no page and scales nothing until
     its first layout; an agent's new tab's page is made by the host at 1280 × 800 right after `preview.resize`. AppKit
     `testAPageSyncedAtAFixedViewportLaysOutAtItFirst` (100% and 125%), `testAShownPageIsMadeAtItsStagesLaidOutSize` (they saw
     640 × 480 and 1280 × 960, [before](https://raw.githubusercontent.com/ccheever/exact2/a3651a3240cfeb4faf09698358021b92a749141e/browser-surface-navigation-v2/appkit-browser-before-fix4.txt)), `testTheHostMakesANewTabsPageAtTheDefaultBeforeItsFirstLoad`
     (no page before, [before](https://raw.githubusercontent.com/ccheever/exact2/ceb47e6f49bab993d7d236fd6fa56cc0b131955b/browser-surface-navigation-v2/appkit-browser-automation-before-fix5.txt)); Bun, the sync payload. Live: both agent pages'
     first layout 1280 × 800 (the fixture's `/first` log).
  4. **The open answered before the data module adopted the tab** once the host made the page itself, so the next request showed
     the `open: false` tab (run4, on a first version of this commit that was never pushed). `browserSync` now names the notes the
     data module adopted, and the open answers once its own is among them (an Idle tab's open waits at most 3 s). AppKit
     `testAnOpenAnswersOnceTheDataModuleHasAdoptedTheTab` (answered at once, then on a sync that only listed the tab:
     [before](https://raw.githubusercontent.com/ccheever/exact2/3f961338ad22c21efb03f330c7cb36f9934b7b8b/browser-surface-navigation-v2/appkit-browser-automation-before-fix6.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/c2f9a92711458113c27c259a4097ac8127fb7c42/browser-surface-navigation-v2/appkit-browser-automation-before-fix7.txt)); Bun.
- After: AppKit [browser 29/29](https://raw.githubusercontent.com/ccheever/exact2/371a66bc86401a0b4f4c760ffa13c0afbc8a5cdf/browser-surface-navigation-v2/appkit-browser-after-fix7.txt), [browser-automation 25/25](https://raw.githubusercontent.com/ccheever/exact2/343610c3729399a0767a8a1d56ac8f50bda0d671/browser-surface-navigation-v2/appkit-browser-automation-after-fix7.txt).
- Handed to part 4 (#354): a new page gets the zoom `browserSync` carries, which is the module's last report (1 for a page not made
  yet); part 4's default zoom must go into that field for its 125% default to be the first layout.

Also corrected: the "Keys" row's ⇧⌘J evidence in #348's drive (ops 88-94) was a tap on the hidden 1 × 1 `shortcut-preview.toggle`
button, which presses nothing in an agent drive, so that drive never hid the panel. On `878e605b8` and `26f5c9066` a real ⇧⌘J key event
(`type composer key Meta+Shift+j`) hid the panel, showed it again and hid it.

Checks on `26f5c9066` ([summary](https://raw.githubusercontent.com/ccheever/exact2/78afd1b3dc2f42867d64b821ea6a69d289774e27/browser-surface-navigation-v2/checks-26f5c9066.txt); `run_checks.py`, source digest `7760fb76…` over the 30 task-owned code files, unchanged by the run): AppKit `browser` 29/29 and `browser-automation` 25/25; Bun 3,773 pass, 1 skip, 0 fail (3,774 tests, 264 files); strict tsc; contract build (5,900 slots; `app.contract` unchanged at 1,230 lines); `cargo test -p t3-code-macos --lib` 13 passed; caps; `cargo build --all-targets --keep-going`; `cargo test --lib --bins --tests --no-fail-fast` (94 binaries: 3,521 passed, 0 failed, 34 ignored); `cargo clippy --all-targets --keep-going -- -D warnings`; `cargo fmt --all -- --check`; `bun scripts/boot.mjs`. (On `878e605b8` the same eleven passed: Bun 3,771, AppKit 27/23.)

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.
2026-10-09 (later): built on `dce6d78df` (part 1 merged), fast-forwarded to `7ce613206` (#344) and `d564a5c02` (#345).
WebKit mechanisms measured first (a probe): a host whose bounds scale the web view keeps the page's CSS viewport and dpr
(presentation only, as the reference's CSS transform); `pageZoom` behaves as Chromium's zoom (the viewport narrows, dpr
follows); the view's `appearance` is the page's `prefers-color-scheme`. One live drive and its retry (agent mode; the
screen locked mid-way, so no window images); an independent review; one repair round (below); verification attempt 2.
2026-10-09 (user decision, after the PR): Recently used matches the reference (`de34391427`): the clone's registration of
the thread's project was removed; item 10 moved to browser-surface-profiles (part 4) (coordinator).
2026-10-09 (after #346, part 5, merged as `5f0ae7dca`; coordinator): merged the base keeping both parts, and wired part
5's `previewAutomation` host to the viewport. `preview_resize` resolves a preset with `resolvePreviewViewport` (part 5
sent a preset without its size) and renders freeform and preset sizes: a page the panel shows is sized by the stage from
the tab's snapshot, a page no stage holds by the host (`T3BrowserViewport.hold`: `width × zoom` by `height × zoom`, also
on every later operation, so a zoom change keeps it). A tab an agent opens on Fill is resized to 1280×800 before the data
module adopts it (`previewAutomationDefaultViewport`). A size not rendered in time goes back to the previous one
(`shouldRollbackPreviewViewport` without the latest-setting and epoch checks, declared). The two parts each set the
page's appearance; now one mechanism (`setColorScheme`), and the host's status reads the page's own, so the More menu's
Appearance shows an agent's choice and part 5's overlay no longer reports System over it.

2026-10-09 (re-land, #352): #348 had been squash-merged as `46436acc3` a minute before Charlie's sweep asked to keep it open; the user had it reverted (#351, `c603c22d6`) and re-landed as draft #352 (`91b6f9a83`, the same tree). Charlie's checks were run fresh ("Re-land acceptance"): a lane with a fake ACP agent calling the server's `preview_*` MCP tools (part 5's approach), agent mode. The first runs on `91b6f9a83` found two host defects, fixed in `878e605b8`; checks 1-3 passed live there, the eleven checks passed, and check 4 passed by real input in the one unlocked window (08:38-08:41Z). Part 4's drive then found pages made before their fixed size; the coordinator asked for the fix here: `26f5c9066` (its first version, never pushed, let an `open: false` tab be shown; fixed before the push). On `26f5c9066`: checks 1-3 and the first-layout probe passed live, and the eleven checks passed. Check 4 was not re-run there: the screen locked again.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Drive 1 (agent mode) | build6 of the branch | Stopped at op 9: the drive's own test id (`browser-server-127.0.0.1:16701`); the scanner names loopback servers `localhost:<port>`. Its op 8 tree already listed the servers | drive record | none (retry) |
| Drive 2 (agent mode, the retry) | same build | 111 ops, agent exit 0; every row passed but Rotate and Lock, whose taps the lane's provider-updates toast covered | [drive record](https://raw.githubusercontent.com/ccheever/exact2/10258e0535b2ae3fd56edcf6b131f782eac36336/browser-surface-navigation/drive-record.txt), [drive script](https://raw.githubusercontent.com/ccheever/exact2/f7ebb5cc55fe5942c9b725d4ed68bb4a47addf30/browser-surface-navigation/drive.sh.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/9611ab6410041b5cfb8e1bbd2181ac9c1ea3d4b6/browser-surface-navigation/fixture.mjs.txt) | Rotate/Lock live and window images: real-input batch (screen locked) |
| Verification attempt 1 | the staged change on `d564a5c02` | 11 required checks passed, source unchanged (runner `run_checks.py`): Bun 3,661 run / 0 fail, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, AppKit 27/27, caps, `cargo build --all-targets --keep-going`, `cargo test --lib --bins --tests --no-fail-fast` (94 test binaries ok), clippy `-D warnings`, fmt, boot | runner report | review findings (below) |
| Verification attempt 2 | after review round 1 | Stopped after 4 passing checks: review round 2's fixes changed the source | runner report (partial) | none |
| Verification attempt 3 | the staged change on `d564a5c02` after both review rounds; source fingerprint `ffe6a1cd44c2…` (sha256 over the 25 task-owned files), unchanged by the run | 11 required checks passed (`run_checks.py`): Bun 3,662 pass / 1 skip / 0 fail (3,663 tests, 261 files); strict tsc; contract build (5,882 slots); `cargo test -p t3-code-macos --lib` 13 passed; AppKit `macos/tests/browser` 27/27; caps; `cargo build --all-targets --keep-going`; `cargo test --lib --bins --tests --no-fail-fast` (94 binaries ok: 3,521 passed, 0 failed, 34 ignored); `cargo clippy --all-targets --keep-going -- -D warnings`; `cargo fmt --all -- --check`; boot. Optional, recorded as unavailable: the live drive (session limit used) and real input (screen locked) | runner report | real-input batch |
| User decision applied (Recently used matches the reference; item 10 moved to part 4) | after the PR's `2cb4c271a` (base `0fe34a3b0`) | The project registration removed (`browser-navigation.ts`), its tests updated (an unregistered thread's visit waits and the list stays empty; a saved mapping lists and titles). Bun 3,664 pass / 1 skip / 0 fail (3,665 tests), strict tsc, contract build (5,882 slots), caps. No Swift, Rust or Contract change, so the cargo and AppKit checks were not run again; no new live session (coordinator) | Bun and checks above | real-input batch |
| Part 5 merged and wired (`_resize`, the agent default, the rollback, one appearance mechanism) | `f5d232b0f` (base `5f0ae7dca`, #346, merged in as `777c3a4d5`) | All on the committed tree, unchanged by the run: Bun 3,770 pass / 1 skip / 0 fail (3,771 tests, 264 files); strict tsc; contract build (5,900 slots); caps; `cargo test -p t3-code-macos --lib` 13 passed; AppKit `macos/tests/browser` 27/27 and `macos/tests/browser-automation` 20/20 (three new viewport cases); `cargo build --all-targets --keep-going`; `cargo test --lib --bins --tests --no-fail-fast` (94 binaries ok: 3,521 passed, 0 failed, 34 ignored); clippy `-D warnings`; fmt; boot. No live agent run of the wired resize (no new live session) | check logs (`target/bsn/checks-wire346`) | real-input batch |
| Part-1 test flake found | — | Two part-1 AppKit assertions read a page's title the moment its load ends; WebKit's title KVO can trail it (base `7ce613206`: 1 of 6 runs failed; the branch before the fix 2–4 of 9). Their waits now include the title: 10 of 10 clean | AppKit runs | none |
| Re-land drive run1 (agent mode) | `91b6f9a83` | Check 1's open and resizes passed; `preview_open` answered 1578 × 1183 for the 1280 × 800 tab. Stopped at op 17: `toggle-right-panel` is not this layout's | [drive record](https://raw.githubusercontent.com/ccheever/exact2/4232b477694317b269e57197465e35cc68da37ad/browser-surface-navigation-v2/drive-record.txt) (earlier runs) | the open's answer: fixed in `878e605b8` |
| Re-land drive run2, diag1, diag2 (agent mode) | `91b6f9a83` | All ops ran. The hidden 1 × 1 `shortcut-preview.toggle` tap presses nothing (the panel never hid; a real ⇧⌘J key event works); tab_2 opened with `open: false` was shown by the agent's next op; timeoutMs 1 ends at the host's readiness budget, before any resize. The open answered 1578 × 1183 twice more | drive record (earlier runs) | `suppress` in the note: fixed in `878e605b8`; the drive now uses ⇧⌘J keys and timeoutMs 25 |
| Fixes and review | `878e605b8` | AppKit `browser-automation`: three new cases, each failed before its fix ([1](https://raw.githubusercontent.com/ccheever/exact2/fc45e5f06b82550d166df732cfb2425a8fd78d19/browser-surface-navigation-v2/appkit-browser-automation-before-fix.txt), [2](https://raw.githubusercontent.com/ccheever/exact2/6658d3468358c486f5ebf94c30fff35d250454a0/browser-surface-navigation-v2/appkit-browser-automation-before-fix2.txt), [3](https://raw.githubusercontent.com/ccheever/exact2/a1d55aef1fc56f820ed68e56fe0570d4eebba217/browser-surface-navigation-v2/appkit-browser-automation-before-fix3.txt)), 23/23 after; Bun suppression test. Read-only review twice: no blocking findings | [after](https://raw.githubusercontent.com/ccheever/exact2/47d4517a283be6e1398d32bcfa9dd93502a59fe5/browser-surface-navigation-v2/appkit-browser-automation-after.txt) | — |
| Re-land drive run3 (agent mode) | `878e605b8` | Checks 1-3 passed live (all 67 ops, driver exit 0) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/4232b477694317b269e57197465e35cc68da37ad/browser-surface-navigation-v2/drive-record.txt), images in "Re-land acceptance" | check 4: real input (screen locked) |
| Checks | `878e605b8` (source digest `df69dbef…` over the 29 task-owned code files, unchanged by the run) | Passed through `run_checks.py`: AppKit `browser` 27/27 and `browser-automation` 23/23; Bun 3,771 pass, 1 skip, 0 fail (3,772 tests, 264 files); strict tsc; contract build (5,900 slots); `cargo test -p t3-code-macos --lib` 13 passed; caps; `cargo build --all-targets --keep-going`; `cargo test --lib --bins --tests --no-fail-fast` (94 binaries: 3,521 passed, 0 failed, 34 ignored); clippy `-D warnings`; fmt; boot | [checks](https://raw.githubusercontent.com/ccheever/exact2/8d2e82133d0148a3be52d8471452500b79f11671/browser-surface-navigation-v2/checks-878e605b8.txt) | — |
| First layout (part 4's question) | `878e605b8` | Both agent-opened pages laid out first at 1280 × 800 (shown, and `open: false` in no stage) | [record](https://raw.githubusercontent.com/ccheever/exact2/85ef8e55a2de43e3b3e0301167b102303a6ac306/browser-surface-navigation-v2/first-layout-record.txt) | — (a race the host wins; recorded) |
| Fixes on the first-layout question | f38d20cf6 (never pushed), then `26f5c9066` | AppKit: pages made at their fixed size (browser 29/29: two new, [before](https://raw.githubusercontent.com/ccheever/exact2/a3651a3240cfeb4faf09698358021b92a749141e/browser-surface-navigation-v2/appkit-browser-before-fix4.txt)); the host makes an agent tab's page ([before](https://raw.githubusercontent.com/ccheever/exact2/ceb47e6f49bab993d7d236fd6fa56cc0b131955b/browser-surface-navigation-v2/appkit-browser-automation-before-fix5.txt)); run4 on f38d20cf6 showed the `open: false` tab after the next request, so the open now answers once its note is adopted ([before](https://raw.githubusercontent.com/ccheever/exact2/3f961338ad22c21efb03f330c7cb36f9934b7b8b/browser-surface-navigation-v2/appkit-browser-automation-before-fix6.txt), [before](https://raw.githubusercontent.com/ccheever/exact2/c2f9a92711458113c27c259a4097ac8127fb7c42/browser-surface-navigation-v2/appkit-browser-automation-before-fix7.txt)); browser-automation 25/25. Read-only review twice: no blocking findings | [after](https://raw.githubusercontent.com/ccheever/exact2/343610c3729399a0767a8a1d56ac8f50bda0d671/browser-surface-navigation-v2/appkit-browser-automation-after-fix7.txt) | — |
| Real input, check 4 | `878e605b8` | Rotate and Lock passed by real clicks (08:38:51-08:41:13Z, lock taken and released) | [record](https://raw.githubusercontent.com/ccheever/exact2/c5c858aeb5a8e0fa9a72cc020e1f334c346d03c8/browser-surface-navigation-v2/realinput-878e605b8.txt), [image](https://raw.githubusercontent.com/ccheever/exact2/5ee8d0f620b96077cb844ab9db511f753a905459/browser-surface-navigation-v2/c4-rotate-lock-878e605b8.png) | — (re-run on `26f5c9066`: next row) |
| Real input, check 4 | `26f5c9066` | Rotate and Lock passed by real clicks (2026-10-09 13:39:08-13:41:31Z, lock taken and released; a start at 13:24Z stopped when the screen locked, before any read-back) | [record](https://raw.githubusercontent.com/ccheever/exact2/05a75796fa6dbf344fa13622d2ff11e2ae7c3890/browser-surface-navigation-v2/realinput-26f5c9066.txt), [image](https://raw.githubusercontent.com/ccheever/exact2/da9155a758604606a7d68b0bce3b7dc90ad49d57/browser-surface-navigation-v2/c4-rotate-lock-26f5c9066.png) | — |
| Re-land drive run5 (agent mode) | `26f5c9066` | Checks 1-3 passed live, all 67 ops, driver exit 0; first layouts 1280 × 800 | [drive record](https://raw.githubusercontent.com/ccheever/exact2/cfcd0378e379bde2132f133f8529af643d4403b7/browser-surface-navigation-v2/h26f5c9066-drive-record.txt) | — |
| Checks | `26f5c9066` (source digest `7760fb76…`, unchanged by the run) | Passed through `run_checks.py`: AppKit `browser` 29/29, `browser-automation` 25/25; Bun 3,773 pass, 1 skip, 0 fail; strict tsc; contract build (5,900 slots); `cargo test -p t3-code-macos --lib` 13; caps; the five checks (cargo test: 94 binaries, 3,521 passed, 0 failed, 34 ignored) | [checks](https://raw.githubusercontent.com/ccheever/exact2/78afd1b3dc2f42867d64b821ea6a69d289774e27/browser-surface-navigation-v2/checks-26f5c9066.txt) | — |

### Independent review, round 1 (2026-10-09)

A read-only review of the staged change found no blocking defect; its should-fix items were fixed in one repair round,
each with a test:
1. A URL field that left without a blur (the panel hidden, the tab switched) kept `chrome-focus`, so the module could take
   ⌘0, ⌘=, ⌘-, ⌘L and ⌘R from the whole window. The field now counts only while the window's first responder is inside
   the field tagged `browser-url` (AppKit: a stale focus claims nothing).
2. A transport error on the discovery stream resubscribed at once, past the transport's backoff. It now keeps the last
   list and waits as `live-streams.ts` does; a scheduled retry subscribes again (Bun).
3. A drag or key that asks for no change left the view holding its drawn size. Every such op now settles the serial (Bun).
4. Chords were named by ANSI key codes only (Dvorak, AZERTY). The layout's character comes first; key codes remain for a
   non-Latin source (AppKit: Dvorak, Korean 2-Set).
5. Zoom steps were taken from the last reported zoom. The module now steps the ladder from the page's own zoom (AppKit).
6–8. Recorded here: the registration decision, item 10, the evidence gaps (images and real input deferred). Also fixed:
   ⌃⌘R refreshes (Manager.ts reads Meta or Control); ⇧⌘J shows the thread's active session even without a surface;
   the locked ratio's other field is clamped; a typed size stays drawn until the commit's answer lands; the failed-resize
   test asserts its toast; the unused default helpers were removed. Kept as declared differences in EXACT2-GAPS:
   arrow-key steps are not debounced (no timer).

Round 2 (the same reviewer, on the repairs): fixes 1–5 confirmed, nothing blocking. Three small items, fixed: a typed
size that asks for nothing (invalid, unchanged, or a toolbar older than a fill answer) now settles the serial too (Bun);
a blur after Enter keeps the drawn size until it lands; with the ratio locked the typed side is bounded as
`resizeAtAspectRatio` bounds it, so the ratio holds at 240 and 3,840. Left as recorded: the URL-field gate in the app
rests on the host tagging the field's view with its testId (`NodeViewMac.swift:919-924`), shown by AppKit with a field
tagged by hand; the in-app chord with the field focused is a real-input row.

## Real-input batch steps

Deferred: the screen locked at about 04:04Z on 2026-10-09 (agent window captures blank, no real input). One session,
in this order, under the shared real-input lock; it also takes the PR's window images (before: a base build of
`d564a5c02`; after: this branch; same lane, fixture and 1280×840 size).

Build and lane: `bun examples/t3-code/terminal-host/build.mjs`, `bun examples/t3-code/stage-runtime.mjs`,
`EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle`; copy the bundle as
"T3 Code (Lane Navigation)" with its own bundle id (as part 1's `lane-app.sh`: PlistBuddy, `codesign --force --deep -s -`)
and launch its executable with the lane environment: `T3_LOCAL_HOME` copied from a home seeded by the runtime's
`t3 project add` (projects `work`, `site`), `T3_LOCAL_PORT=16700`, `T3_LOCAL_RUNTIME_DIR` (the staged runtime, unpacked),
`HOME`/`CFFIXED_USER_HOME` in the lane dir, isolated `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `XDG_*`,
`T3CODE_TELEMETRY_ENABLED=false`, `PATH=<lane bin>:/usr/bin:/bin:/usr/sbin:/sbin`; the fixture on 16701 (the PR's
`fixture.mjs.txt`: `/`, `/a`, `/b` print the CSS viewport, dpr and colour scheme). Dismiss the "Updates Available"
toast first (it covers the device toolbar's middle). Put text in with `paste-text`; never switch the Korean 2-Set source.
Read effects back from the tree/AX, the host's `t3.browser:` lines, the fixture log and the page's printed probe.

1. Images: the empty state (Local servers; after a few visits a second tab with Recently used), the load-failed page
   with Details open, the More menu (device toolbar, Appearance, Zoom rows), the zoom pill, a Dark page, the device
   toolbar at fill → Show device toolbar, the preset list, iPhone 12 Pro fitted, rotated, a rail mid-drag (the live size
   label). Before: the same screens on the base build (No preview yet; the failed page without Details; the More menu
   with its "Part 2" rows).
2. Device toolbar by real pointer: Rotate (844 × 390, a preset stays a preset), Lock aspect ratio (the button reads
   "Unlock …", aria-pressed true), then drag the east rail and the south-east corner: the live "W × H" label follows the
   pointer and the size keeps the ratio; release commits (`preview.resize`, tree values). Tab to a rail, ← → ↑ ↓ (10 pt,
   50 with Shift).
3. Recently used by real pointer: hovering a row reveals its ×, a click removes the row; clicking a row opens it.
4. Keys with real key events (Korean 2-Set on): in the page (click into it first) ⌘R (the fixture logs a second `GET`),
   ⌘= and ⌘+ (Shift-=) zoom in, ⌘- zooms out, ⌘0 resets (the pill and the probe's width), ⌘L puts the focus in the URL
   field with its text selected; the same chords with the focus in the URL field; ⇧⌘J hides the Browser panel and shows
   it again on the same tab (and opens a Browser tab when the thread has none).
5. Appearance submenu by real pointer: More › Appearance opens beside the menu, Dark/Light/System re-check and the page
   repaints; the zoom −/+ buttons keep the menu open.
After: quit the copy (an Apple Event quit stops its server), stop the fixture, `defaults delete` the copy's bundle id,
release the lock.

## Next action

#352 stays a draft (Charlie, 2026-10-09). Check 4 (Rotate and Lock by real input) passed on `26f5c9066` (2026-10-09 13:39-13:41Z, [record](https://raw.githubusercontent.com/ccheever/exact2/05a75796fa6dbf344fa13622d2ff11e2ae7c3890/browser-surface-navigation-v2/realinput-26f5c9066.txt)). Left: the rest of the real-input batch (steps above). Then the coordinator's conflict check and Charlie's review; no merge here. Part 4 (#354) holds item 10's Settings rows, and its default zoom goes into the `zoom` that `browserSync` carries.

## Delivery

Merged on 2026-10-10 as `ab220bfdb` (#352, squash). Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
