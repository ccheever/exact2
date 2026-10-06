---
name: 20261005-app-developer-tools
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

# View › Toggle Developer Tools: the app's own inspector

## Outcome

The clone's View menu has a Toggle Developer Tools item in the reference's position. Choosing it opens an inspector for the app window; choosing it again closes the inspector. A developer of T3 Code or of the clone can see the live view tree,
the resolved style of a node, the state and the log, as the reference's DevTools shows the page's elements, styles and console. The View menu then equals the reference's in items, order and labels.

This ticket is blocked. It starts only when issue X2 is decided: Charlie changes the DEFERRED "no devtools UI" rule (or allows a development-only inspector), the option is chosen (A host inspector, B inspectable web views, C disabled item), and the support is merged upstream;
or the user closes this ticket by decision (the item stays absent and is documented).

## Scope and exclusions

Included:

1. **The menu item.** "Toggle Developer Tools" in View, after Force Reload and before the first separator, enabled and keyboard reachable. Label and accelerator exactly as the reference shows them; read both from the oracle's menu at `prepare`
   (the reference uses Electron's `toggleDevTools` role, so they come from Electron; `DesktopApplicationMenu.ts:229-251`).
2. **Toggle behavior.** First choice opens the inspector; second closes it; closing the inspector with its own close control leaves the next menu choice opening it again. One inspector per app window.
3. **Inspector content (option A).** The same data the agent API reads: `tree` (nodes with `testId`), `state` (slots, derives, resources), `layout <testId>` (identity, resolved style source, coordinate spaces), `logs` (refused operations, data and host diagnostics) and `perf`. Content updates live.
4. **Inspectable web views (option B, development builds only).** The terminal, rendered-HTML and Mermaid web views are marked inspectable so Safari's Web Inspector can attach. This is the minimum even if option A is not built.
5. **Build scope.** The decision on whether release builds carry the item. The reference carries it in every build (the menu is built for all users).
6. **Ported tests.** `DesktopApplicationMenu.test.ts` View test: "routes View menu zoom to the main window instead of zoom roles" (`:218`), applied to the clone's View items. The reference has no test for the DevTools role; add one for the item's position, label and enabled state.

Excluded: the reference's habit of opening detached DevTools at window creation in development builds (`DesktopWindow.ts:833-835`); per-page DevTools of Browser tabs (`20261005-browser-surface`); remote debugging ports; any inspector for the embedded server.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/desktop/src/window/DesktopApplicationMenu.ts:229-251` (View: Reload, Force Reload, Toggle Developer Tools, separator, Actual Size ⌘0, Zoom In ⌘=, hidden Zoom In ⌘Plus, Zoom Out ⌘-, separator, Toggle Full Screen);
`DesktopWindow.ts:833-835`; tests `DesktopApplicationMenu.test.ts:127-250`.
Facts about exact2 come from `EXACT2-GAPS.md` X2 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): `rules/DEFERRED.md:615` says "no devtools UI"; the agent API has `tree`, `state`, `layout`, `logs`, `perf`; the clone UI is drawn natively, so no DOM exists for an inspector;
the options are (1) keep it absent, (2) show it disabled, (3) mark the app's own WKWebViews `isInspectable` in development builds; "A real equivalent needs a DEFERRED change." Everything else about exact2 for this ticket is to confirm at `issue-open` or `prepare`.
Clone state (mc-orch tree, 2026-10-05): the item is absent; `modules/apple/T3Menus.swift:78-89` adds Actual Size and Zoom In/Out to the host's View menu; `R8KeysMenus.swift` also edits menus; no module calls `isInspectable`.
Library revision: `20261005-platforms-v3`. Selected topics: testing-and-debugging (agent observations), accessibility (keyboard access to menu items), platforms (macOS menus). An in-app inspector and host menu verbs are unknown in the library.
Consumer framework revision: a `main` pin that contains the X2 support.
Line numbers are from the reference at `1e2ecbd975` and the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves clone code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `drive.mjs`, `lane-backend.sh`.
Rules for lane runs: attended rows build the lane app with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; lane ports are 16000 to 16999; never use port 3773, `~/.t3` or the `t3code` URL scheme.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| resolved framework issue | [X2 developer tools for the app UI](../issues/20261005-x02-app-developer-tools.md) | none yet (local draft) | Charlie's decision on the DEFERRED rule and the chosen option; only option A also needs support merged upstream (options B and C are app-side) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (the oracle's menu template and screenshots) | pending |
| scheduling preference | After `20261005-desktop-shell-details` (menu work) | none | Not a prerequisite | — |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X2](../issues/20261005-x02-app-developer-tools.md) | Inspector for the app UI; DEFERRED rule | `EXACT2-GAPS.md` X2 | blocking | Wait for the decision; then fill the **P** rows |
| [X26](../issues/20261005-x26-app-menu-control.md) | App menu control (host items, order) | `EXACT2-GAPS.md` X26: the host menu bar is fixed | nonblocking (workaround: `T3Menus.swift` inserts items) | Insert the item in the host's View menu in the reference order |

## Implementation notes

- Menu item: in `T3Menus.swift` next to the zoom items, using the same insertion pattern (`view.insertItem`) and the host's own Reload / Force Reload as the anchor; target and action come from the chosen option.
- Option A: the host verb opens the framework's inspector window; the item's checked state follows the window. Option B is an addition, not a substitute for the item: the web views' `isInspectable` is set in their creation path (`R6MediaPreview.swift`, `T3TimelineMermaid.swift`, the terminal view once built) in development builds.
  Option C is the disabled item and does not close X2.
- States. Disabled: only if the decision says so; then the reason is shown in the issue and the item is greyed. Keyboard: reachable with the menu bar keys and the accelerator. Escape closes the open menu only. Reduced motion: no animation in the item.
  Error: if the inspector cannot open, `logs` shows the host diagnostic and the menu stays usable.
- Accessibility: the menu item has its visible title as accessible name; the inspector window (option A) is the framework's.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` and `cargo test -p t3-code-macos --lib` | The View menu test and the new item test pass | macOS | log |
| Menu order | Oracle build and clone build; menu template read by `electron-oracle.mjs` and the AppKit menu test | Compare the View submenu | Items, order, labels, separators and accelerators equal the oracle's (Reload, Force Reload, Toggle Developer Tools, separator, Actual Size, Zoom In, Zoom Out, separator, Toggle Full Screen) | macOS | menu dump, AppKit test |
| Toggle | Lane build, thread open | The agent's menu operation if the framework offers one (to confirm at `prepare`), otherwise an `(attended session)` click: View › Toggle Developer Tools; again | Inspector opens, then closes; the next choice opens it again after the inspector's own close control (option A); `logs` shows the host command | macOS | `--json` logs, shots |
| Inspector content **P** | Same, with a known `testId` on screen | Open the inspector; select that node; change state; trigger a refused operation | The tree shows the node; style source and layout match `layout <testId>`; state and log update live | macOS | shots, JSON |
| Web views inspectable (option B) **P** | Development build with the Mermaid or HTML preview open | Attach Safari's Web Inspector `(attended session)` | The web view is listed and inspectable; a release build does not list it | macOS | notes, shot |
| Build scope **P** | Release and development builds | Open View | Item present as the decision says | macOS | menu dump |
| Keyboard and accelerator `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch | Menu bar keys to the item; press its accelerator; Escape with the menu open; Korean 2-Set input source | Same result as the oracle; Escape closes only the menu | macOS | notes |
| States | Menu open | Keyboard focus ring in the menu; reduced motion on | Standard menu focus; no animation | macOS | shots |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, affected AppKit binaries), `bun scripts/caps.mjs`, the five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `modules/apple/T3Menus.swift`, `modules/apple/R8KeysMenus.swift`, `apple/tests/menus/**`, the web view modules for option B, `AGENT-HANDOFF.md`, `EXACT2-GAPS.md` (remove the X2 row when it lands).
Required environment: oracle build, Xcode 27.0, Bun 1.4.2, the X2 support at the pinned `main`; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`.

## Progress

Blocked on X2 (Charlie's decision and the chosen option). No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |

## Next action

Blocked until X2 (`../issues/20261005-x02-app-developer-tools.md`) is resolved or decided; then `prepare`, or close this ticket if the decision is to close.
