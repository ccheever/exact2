---
name: 20261005-browser-surface-navigation
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Browser surface part 2: navigation, history, zoom and the device toolbar

## Outcome

The Browser tab built in [part 1](20261005-browser-surface.md) gains the reference's navigation aids: the empty state's
"Recently used" and "Local servers", the full unreachable page, the history store, target resolution, page zoom, the
appearance preference sent to the page, the device toolbar (fill, freeform, the 17 presets, rotate, resize handles) and
the preview keys. The More menu's Show/Hide device toolbar, Appearance and Zoom rows, disabled and marked "Part 2" by
part 1, work. Settings › Integrations › Browser gains its default viewport, zoom and appearance rows.

Split from [20261005-browser-surface](20261005-browser-surface.md) at its `prepare` (planned split, part 2). It starts
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
    (`browserDefaultOpenViewport`, `browserDefaultTabState`).

Excluded: annotate, capture, recording, picture in picture (part 3); profiles, cookie import, clearing (part 4); the
`previewAutomation.*` host, links, Mute (part 5).

## Context and guidance

Parent specification: [spec](../spec.md); engine decisions and declared differences: the parent record and
`EXACT2-GAPS.md` "Browser surface: declared differences (X1 path B)". Reference: T3 Code `1e2ecbd975`
(`apps/web/src/browser/*`, `apps/web/src/components/preview/*`, `apps/web/src/browserHistoryStore.ts`,
`apps/server/src/preview/PortScanner.ts`). Part 1's seams: `browser-surface.ts` (`browserView`, `browserLocal`),
`browser-surface.contract` (`BrowserMore`, `BrowserEmpty`, `BrowserLoadFailed`), `T3BrowserSession.swift` (commands).
The More menu rows marked "Part 2" lose the mark when they work.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-browser-surface](20261005-browser-surface.md) (part 1) | its draft PR | Merged into `feat(example)/t3-code` | pending |
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

## Progress

2026-10-09: written at part 1's `prepare` (planned split). Planned; starts after part 1 merges.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |

## Next action

After part 1 merges: `prepare` (WebKit mechanisms for zoom and appearance, the discovery stream on the Swift
transport), then implement.
