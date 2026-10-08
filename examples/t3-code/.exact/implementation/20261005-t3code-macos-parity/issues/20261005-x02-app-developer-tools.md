---
name: 20261005-x02-app-developer-tools
plan: 20261005-t3code-macos-parity
status: published
kind: framework-policy
blocks: [20261005-app-developer-tools, 20261005-desktop-shell-details, 20261005-terminal-surface]
upstream_url: https://github.com/ccheever/exact2/issues/101
reproduced_on: 4c893fef6
---

# X2: A developer-tools inspector for the app's own UI (View › Toggle Developer Tools)

**Status (reclassified 2026-10-08):** Bucket 1, done on main: main #309 (`f2f0e7092`) makes development `iframe` web views inspectable; round 7 adopts it and removes nothing. The View item is a permanent declared difference; #101 stays open upstream.

## Summary

The T3 Code desktop app has a View › Toggle Developer Tools menu item that opens Chromium DevTools for the whole app window. exact2 draws the
clone natively, so no DOM exists for an inspector, and its rules refuse a devtools UI. The clone has no such item. A decision from Charlie is needed:
change the DEFERRED rule so that a development inspector can exist, or close the item by decision.

## Why this issue arose

### The T3 Code behavior
- The application menu's View submenu is: Reload, Force Reload, **Toggle Developer Tools**, a separator, Actual Size (⌘0), Zoom In (⌘=, and a hidden ⌘Plus twin),
  Zoom Out (⌘-), a separator, Toggle Full Screen (`apps/desktop/src/window/DesktopApplicationMenu.ts:229-251`). The item is Electron's `toggleDevTools` role,
  so its label and accelerator come from Electron (the accelerator is to confirm on the oracle at `issue-open`).
- Trigger and result. Choosing the item opens Chromium DevTools for the app window's page (Elements, Console, Network, Sources); choosing it again closes DevTools.
  There is no timing, no setting and no error state in the app's code; the menu is built for every user, packaged builds included.
- Development builds also open DevTools detached at window creation (`apps/desktop/src/window/DesktopWindow.ts:833-835`: `if (environment.isDevelopment) { window.webContents.openDevTools({ mode: "detach" }); }`).
  That auto-open belongs to the reference's dev loop and is not carried by the clone.
- Browser tabs have their own per-page DevTools ("Open DevTools" in the preview More menu, `apps/desktop/src/preview/Manager.ts:2576-2590`). That is part of the
  Browser surface (X1, ticket `20261005-browser-surface`), not this issue.
- Reference tests: `DesktopApplicationMenu.test.ts` has a View test for the zoom items (`:218`, "routes View menu zoom to the main window instead of zoom roles"); I found no test for the DevTools role.

### What exact2 does today
- `EXACT2-GAPS.md` X2 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): "`rules/DEFERRED.md:615`: 'no devtools UI'. The agent
  API has `tree`, `state`, `layout`, `logs`, `perf` (DEFERRED Agent API)." And: "The clone UI is drawn natively. There is no DOM for an inspector. DEFERRED refuses a devtools UI."
- Same file, support options: "(1) Keep it absent and document it. (2) Show the item disabled. (3) Mark the app's own WKWebViews (terminal, HTML preview) `isInspectable` in
  development builds so Safari can inspect them. A real equivalent needs a DEFERRED change."
- Same file, current state: "The menu item is absent. The View menu has Reload, Force Reload and zoom (`R8KeysMenus.swift`, `T3Menus.swift:78-89`)."
- Library (`20261005-platforms-v3`, testing-and-debugging.md:37-52): the agent reads `tree`, `state`, `layout <testId>`, `logs` and screenshots, through `bun exact.mjs agent …`.
  This is a test driver, not an interactive inspector inside the app. An inspector as a feature is not covered: unknown in this library.
- Observed in the clone (mc-orch tree, 2026-10-05): `grep` of the Swift modules finds no `isInspectable` use (the terminal web view does not exist yet; the
  existing web views are `R6MediaPreview.swift` and `T3TimelineMermaid.swift`).

### Where the clone hits it
- `modules/apple/T3Menus.swift:78-89` adds Actual Size and Zoom In/Out to the host's View menu. No Toggle Developer Tools item exists, enabled or disabled.
- Workaround: none for a person. For an agent or a developer the clone uses the exact2 agent driver (`tree`, `state`, `layout`, `logs`, `perf`).
- Difference a user sees: the View menu has one fewer item. A developer cannot inspect the live view tree, styles or console of the clone's window; they cannot
  inspect the clone's WKWebViews from Safari unless the app sets them inspectable (not done today).

## Why it must be resolved

The goal is a complete clone of the desktop app, and the View menu is part of what a user sees. The item matters most to developers of T3 Code and of the clone:
a tool that shows why a layout or a state is wrong. Waiting ticket: `20261005-app-developer-tools` (blocked). `20261005-desktop-shell-details` lists the item under "Excluded"
and links this issue, so that ticket's menu comparison carries the missing item as a difference. The workaround costs nothing in code, but it is a permanent, visible difference, and "declared deviation"
is not an end state for this plan.

Policy involved: `rules/DEFERRED.md:615` ("no devtools UI", as quoted by `EXACT2-GAPS.md` X2) and the Agent API list in the same file. Decision needed from Charlie:
1. Allow a development inspector for the app UI (which option below), and say whether it ships in release builds or only development builds; or
2. Keep the refusal; then the plan closes this issue and `20261005-app-developer-tools` by user decision, with the item documented as absent.

## Requested support

The web way: a person opens the inspector from the menu or a shortcut and sees the element tree, computed style, console and network of the page.
macOS first; other hosts noted at `issue-open`.

| Option | What it is | Cost and limits |
| --- | --- | --- |
| A. Host inspector for the Contract tree | A "Toggle Developer Tools" host command that opens a window showing the same data as the agent API: `tree`, `state`, `layout`, `logs`, `perf`, updating live | A DEFERRED change; framework work; no DOM, so it is an Exact tree view, not Chromium DevTools |
| B. Inspectable native web views | Mark the app's own WKWebViews `isInspectable` in development builds (EXACT2-GAPS X2 option 3), so Safari's Web Inspector can attach | Covers only the web views (terminal, HTML preview, Mermaid), not the Contract UI; app module only; WebKit availability by OS version is to confirm |
| C. Disabled item | Show "Toggle Developer Tools" disabled (EXACT2-GAPS X2 option 2) | Does not resolve the issue; it is a declared difference |
| D. Close by decision | Keep it absent and document it | The issue closes by user decision |

Preferred for parity: A. B is a useful addition for the terminal web view whichever option is chosen. The host command could be a framework menu verb (see X26, host menu control).

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Build and launch the clone (`bun host/apple/build.mjs --run` in the example, or the lane build with `T3_LOCAL_HOME` and `T3_LOCAL_PORT` set).
2. Open the View menu. Expected (reference): Reload, Force Reload, Toggle Developer Tools, Actual Size, Zoom In, Zoom Out, Toggle Full Screen. Actual (clone): Toggle Developer Tools is absent.
3. Reference check on the oracle: choose Toggle Developer Tools; DevTools opens for the app window; choose it again; DevTools closes.

## Acceptance for the fix

- The chosen option is recorded with its scope (development builds only or all builds).
- Option A: the item exists in the View menu; choosing it opens the inspector; choosing it again closes it; the inspector shows the Contract tree with `testId`s, a node's resolved
  style, the state slots, and the log; an agent run confirms the menu verb exists (`logs` shows the host command) and an AppKit test covers the toggle.
- Option B: a development build sets the app's WKWebViews inspectable; Safari lists them (attended check); release builds do not.
- `rules/DEFERRED.md` is updated by its owner to match the decision.

## App adoption after resolution

Unblock `20261005-app-developer-tools` (`prepare`, then implement): add the View item in `T3Menus.swift` next to Reload/Force Reload (the order in the reference), wire it to the
framework verb, match the label and accelerator seen on the oracle, and add the menu-matrix row. If B is chosen, mark the terminal and preview web views inspectable in
development builds only. `issue-close` checks that the item toggles the inspector and that the View menu equals the oracle's order.

## Status and next action

Published 2026-10-06 as [#101](https://github.com/ccheever/exact2/issues/101) (reproduced on exact2 `4c893fef6` before filing). Decided upstream on 2026-10-08: see the last section.

## Decided upstream (2026-10-08): narrowed

[Charlie on #101](https://github.com/ccheever/exact2/issues/101#issuecomment-6055584890): "Allow Safari inspection of development WKWebViews; keep Exact's own inspector deferred. … Use
public isInspectable only in development, with release gating verified. … no new inspector window or inspect
command."
- Narrowed to option (3) under "Requested support": a development-only `isInspectable` on the app's own web
  views. An inspector for the app's UI is refused.
- **Declared difference (permanent):** View › Toggle Developer Tools stays absent; there is no inspector for it to
  open.
- The user's decision (2026-10-08): `app-developer-tools` is narrowed to a development-only `isInspectable` on the
  clone's web views (terminal, rendered HTML, Mermaid). That is app code, built in
  [#326](https://github.com/ccheever/exact2/pull/326) (`T3WebInspection.swift`; draft): a release build (the packaged
  `distribution.json`, a production-trust bake, a distributed bundle) is never inspectable. Nothing waits on main for the clone.
- Main [#309](https://github.com/ccheever/exact2/pull/309) (merged to main on 2026-10-08 as `f2f0e7092`; not in the T3 branch until a main-adoption round) is the main-side half: Exact's `iframe` web views,
  inspectable from the development web arm only (left out for production trust, `exact release` and IPA archives). It
  does not cover the clone's own web views; #326 follows its release line, and adopting #309 needs no clone change.
- Verified 2026-10-08 with real input (user-approved Safari setting, off again afterwards): Safari's Develop menu lists the
  development copy's terminal page and not the release copy (`app-developer-tools` record, evidence 09–12).
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): "existing ready PR #309", now merged on main (`f2f0e7092`): development-only `isInspectable` on Exact's own iframe web views, release-gated; the issue stays open. The clone's own module web views still need the clone-side gate (`app-developer-tools`).
