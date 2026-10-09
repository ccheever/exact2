---
name: 20261005-browser-surface-navigation
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-browser-surface-navigation
pr_url: https://github.com/ccheever/exact2/pull/348
verified_commit: 4466f27ced9f089b5919bda4259198146922a500
---

# Browser surface part 2: navigation, history, zoom and the device toolbar

## Outcome

The Browser tab built in [part 1](closed/20261005-browser-surface.md) gains the reference's navigation aids: the empty state's
"Recently used" and "Local servers", the full unreachable page, the history store, target resolution, page zoom, the
appearance preference sent to the page, the device toolbar (fill, freeform, the 17 presets, rotate, resize handles) and
the preview keys. The More menu's Show/Hide device toolbar, Appearance and Zoom rows, disabled and marked "Part 2" by
part 1, work. Settings › Integrations › Browser's default viewport, zoom and appearance rows (item 10) are not built here
(below, "Not built"); new tabs open at the reference's defaults (fill, 100%, System).

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
    (`browserDefaultOpenViewport`, `browserDefaultTabState`). **Not built** (2026-10-09): the coordinator's part-2 scope
    list leaves it out; the group's other rows are parts 3–5's, and it stays as part 1 left it (inert, "Only available in
    the desktop app."). Proposed: build the whole Integrations › Browser group in one follow-up once parts 3–5 land.

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
  `browserHistory` and read back through `mergeBrowserHistoryState`; the thread registers its project's logical key,
  `r6-polish-groups.ts logicalKey`), `browser-targets.ts` (browserTargetResolver over the client's connected origin;
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
| framework issue | [X21](../issues/20261005-x21-two-way-websocket.md) | #126 | nonblocking: `subscribeDiscoveredLocalServers` is a stream; the Swift transport carries it until #126 | — |
| framework issue | [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | #140 | nonblocking: the preview keys are native monitors while the page has focus | — |

## Acceptance and reproduction

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
| Recently used | Visit pages; a second tab; remove one; open one | At most 8, newest first, title or address with the visit's age; × removes; a row opens | pass live (ops 95–109): four rows (Page B, 127.0.0.1:16749, Page A, Lane fixture, "just now"), Remove 127.0.0.1:16749 leaves three, Page A opens (`navigate http://localhost:16701/a`). Bun: 35 history tests, `PreviewEmptyState` 4 |
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
`browser-navigation.test.ts` 19 (`PreviewEmptyState` 4; clone 15), AppKit `macos/tests/browser` 27 (part 1's 23 with two
of its title waits made race-free, and 4 for part 2). Not ported, with their owner: `browser/browserDefaults` (4: three
are profile resolution, part 4; one is the settings read, with item 10), `browserSurfaceStore` (9: the Electron slot's
positioning; the page is borrowed by `T3BrowserView`, part 1, and fit-to-source is part 3's mini player),
`previewClickFocus` (8: agent clicks, part 5), `apps/desktop` `PreviewKeyboard` (12: it builds the automation's key
events, `previewAutomation.press`, part 5; the preview chords here are the module's monitor, AppKit).

Provisional decision (user decision pending): **Recently used's project registration.** At `1e2ecbd975` nothing calls
`registerThreadProject` (#2829, `de34391427`, dropped ChatView's effect), so the reference's list stays empty for a
thread registered after that change. The clone registers the active thread's project as `72d673a855` did (the
feature's design and this record's scope), by the sidebar's logical project key; it does not borrow a sibling row's
identity (`buildPhysicalToLogicalProjectKeyMap`).

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.
2026-10-09 (later): built on `dce6d78df` (part 1 merged), fast-forwarded to `7ce613206` (#344) and `d564a5c02` (#345).
WebKit mechanisms measured first (a probe): a host whose bounds scale the web view keeps the page's CSS viewport and dpr
(presentation only, as the reference's CSS transform); `pageZoom` behaves as Chromium's zoom (the viewport narrows, dpr
follows); the view's `appearance` is the page's `prefers-color-scheme`. One live drive and its retry (agent mode; the
screen locked mid-way, so no window images); an independent review; one repair round (below); verification attempt 2.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| Drive 1 (agent mode) | build6 of the branch | Stopped at op 9: the drive's own test id (`browser-server-127.0.0.1:16701`); the scanner names loopback servers `localhost:<port>`. Its op 8 tree already listed the servers | drive record | none (retry) |
| Drive 2 (agent mode, the retry) | same build | 111 ops, agent exit 0; every row passed but Rotate and Lock, whose taps the lane's provider-updates toast covered | [drive record](https://raw.githubusercontent.com/ccheever/exact2/10258e0535b2ae3fd56edcf6b131f782eac36336/browser-surface-navigation/drive-record.txt), [drive script](https://raw.githubusercontent.com/ccheever/exact2/f7ebb5cc55fe5942c9b725d4ed68bb4a47addf30/browser-surface-navigation/drive.sh.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/9611ab6410041b5cfb8e1bbd2181ac9c1ea3d4b6/browser-surface-navigation/fixture.mjs.txt) | Rotate/Lock live and window images: real-input batch (screen locked) |
| Verification attempt 1 | the staged change on `d564a5c02` | 11 required checks passed, source unchanged (runner `run_checks.py`): Bun 3,661 run / 0 fail, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, AppKit 27/27, caps, `cargo build --all-targets --keep-going`, `cargo test --lib --bins --tests --no-fail-fast` (94 test binaries ok), clippy `-D warnings`, fmt, boot | runner report | review findings (below) |
| Verification attempt 2 | after review round 1 | Stopped after 4 passing checks: review round 2's fixes changed the source | runner report (partial) | none |
| Verification attempt 3 | the staged change on `d564a5c02` after both review rounds; source fingerprint `ffe6a1cd44c2…` (sha256 over the 25 task-owned files), unchanged by the run | 11 required checks passed (`run_checks.py`): Bun 3,662 pass / 1 skip / 0 fail (3,663 tests, 261 files); strict tsc; contract build (5,882 slots); `cargo test -p t3-code-macos --lib` 13 passed; AppKit `macos/tests/browser` 27/27; caps; `cargo build --all-targets --keep-going`; `cargo test --lib --bins --tests --no-fail-fast` (94 binaries ok: 3,521 passed, 0 failed, 34 ignored); `cargo clippy --all-targets --keep-going -- -D warnings`; `cargo fmt --all -- --check`; boot. Optional, recorded as unavailable: the live drive (session limit used) and real input (screen locked) | runner report | real-input batch |
| Part-1 test flake found | — | Two part-1 AppKit assertions read a page's title the moment its load ends; WebKit's title KVO can trail it (base `7ce613206`: 1 of 6 runs failed; the branch before the fix 2–4 of 9). Their waits now include the title: 10 of 10 clean | AppKit runs | none |

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

Review and merge the draft PR. Then the real-input batch (steps above). Parts 3 (capture) and 4 (profiles) start after
this merges; the Integrations › Browser group (item 10 with parts 3–5's rows) is a proposed follow-up.
