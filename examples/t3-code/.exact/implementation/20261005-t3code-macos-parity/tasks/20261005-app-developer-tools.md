---
name: 20261005-app-developer-tools
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-app-developer-tools
pr_url: https://github.com/ccheever/exact2/pull/326
verified_commit: caf50cab9
---

# Safari's Web Inspector on the clone's web views in development builds; View › Toggle Developer Tools stays absent

## Outcome

Rewritten 2026-10-08 for the user's decision, which narrowed this task to Charlie's ruling on
[#101](https://github.com/ccheever/exact2/issues/101) (X2): "Allow Safari inspection of development WKWebViews; keep Exact's
own inspector deferred. [...] Use public isInspectable only in development, with release gating verified. Keep
tree/layout/state/logs/perf as the native inspection path; no new inspector window or inspect command."

In a development build, every WKWebView the clone's own module creates is `isInspectable`, so Safari's Develop menu lists
it and Web Inspector can attach: the terminal (`T3TerminalView`), the rendered-HTML preview (`R6MediaPreview`) and the
offscreen Mermaid renderer (`T3TimelineMermaid`). A release build's web views are never inspectable. A release build is any
of: the clone's packaged build, the flavor marker the embedded server already reads (`Contents/Resources/distribution.json`
`{"flavor":"packaged"}`, `T3LocalPolicy.packaged`); a production-trust bake (the bundle receipt's `build.trust`); a
distributed bundle (`--distribution`, what `exact release` signs, or an IPA), whose receipt is the shipped one (its binary
reduced to a digest). Every other build is a development build. No environment variable changes the answer and there is no
cargo feature. (The reference's own line is `isDevelopment` = a Vite dev server URL; the clone has no dev server.)

**Relation to main [#309](https://github.com/ccheever/exact2/pull/309)** ("Apple: allow Safari inspection of development
iframe web views", merged to main on 2026-10-08 as `f2f0e7092`; not in the T3 branch until a main-adoption round; the main-side half of #101). #309 opts the host's `iframe` web arm into `isInspectable` when it
compiles the development arm, and leaves the opt-in out for production trust, `exact release` and IPA archives. It covers
Exact's `iframe` web views only: the clone's own WKWebViews are created by its module and need this clone-side gate, and the
clone adopts #309 in a main-adoption round with no change here. The two lines agree: the receipt checks are #309's
production-trust and distribution exclusions read at run time (the module artifact is compiled the same way in both, so the
module cannot take #309's compile-time define). The packaged marker is the clone's own addition: the packaged build is the
clone's release and uses the real `~/.t3`, so it is never inspectable even if it were assembled from a development bake.

| Build | #309, Exact's iframe arm | This gate, the clone's web views |
| --- | --- | --- |
| Development (`build.mjs`, `--run`, the agent) | inspectable | inspectable |
| Production trust (`EXACT_UPDATE_TRUST=production`) | not | not (receipt `build.trust`) |
| `--distribution` (what `exact release` signs) | not | not (shipped receipt) |
| IPA archive | not | not (shipped receipt; the clone has no iOS build) |
| The clone's packaged build (`distribution.json`) | follows the bake it came from | not |

The reference's View › Toggle Developer Tools item stays absent for good, a declared difference (#101): an inspector for the
app's own UI is not built in Exact, and the agent API (`tree`, `layout`, `state`, `logs`, `perf`) stays the native
inspection path. The View menu is unchanged by this task.

Superseded by the decision (the record before 2026-10-08): the menu item, its toggle, option A (a host inspector for the
Contract tree), option C (a disabled item) and the menu-order rows.

## Scope and exclusions

Included:

1. **The gate** (`modules/apple/T3WebInspection.swift`): `enabled`, read once from the bundle; `permits(resources:)`: false
   for the packaged marker (`T3LocalPolicy.packaged(resources:)`), a receipt with `build.trust` `production`, or a shipped
   receipt (`build.binary` holding `sha256` alone, as `shippedReceipt` in `host/apple/build.mjs` writes it); `mark(web, kind)`,
   which sets `isInspectable` and writes `t3.inspection: <kind> web view isInspectable=<flag read back from the view>
   (<development|release> build)` to stderr (the agent's `logs` host lines). Reading a development bundle's 4.1 MB receipt
   costs 11–14 ms, once per process, at the first web view; a shipped receipt is small.
2. **Every creation path** calls `mark` once, before its first load: `T3TerminalView.init` (this replaces the old gate,
   `EXACT_ASSETS` or `T3_TERMINAL_INSPECTABLE=1`, so a development copy opened with `open` is inspectable too),
   `R6MediaPreview.webView(name:)`, and `T3TimelineMermaid.renderer()` (a new factory that `load(base:)` uses). Both popup
   delegates (`createWebViewWith`) return nil, so the module creates no other web view.
3. **Tests**: `macos/tests/r6-media/inspection.swift` (`WebInspectionTests`, 4 tests, run by the r6-media binary).
4. **Docs**: README (the local-backend section), `AGENT-HANDOFF.md` (terminal row S9, the in-app differences list),
   `EXACT2-GAPS.md` X2, the X2 issue record and its rows in `issues/README.md`, `STATUS.md`.

Excluded by the decision: the View menu item in any form (enabled, disabled or development-only); an inspector for the Exact
tree (former option A); a menu verb or an inspect command; the reference's detached DevTools at window creation in
development builds (`DesktopWindow.ts:833-835`); per-page DevTools of Browser tabs (`20261005-browser-surface`, X1); remote
debugging ports; any inspector for the embedded server.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`): `apps/desktop/src/window/DesktopApplicationMenu.ts:229-251`
(View: Reload, Force Reload, Toggle Developer Tools, separator, Actual Size ⌘0, Zoom In ⌘=, hidden Zoom In ⌘Plus, Zoom Out ⌘-, separator,
Toggle Full Screen); `DesktopWindow.ts:833-835`; `app/DesktopEarlyElectronStartup.ts:49-50` (`isDevelopment` is `VITE_DEV_SERVER_URL` set).
The reference has no test for the DevTools role; its View test (`DesktopApplicationMenu.test.ts:218`) covers zoom, which this task does not change.
exact2: `rules/DEFERRED.md` "Tooling — no Design Mode, no Guide system, no devtools UI"; the decision on #101 keeps that rule and allows the
platform inspector on the app's own web views in development. The module's web views are plain `WKWebView`s; `isInspectable` is public API
since macOS 13.3 and the app's minimum is 14.0, so no availability check is needed.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| framework policy decision | [X2 developer tools for the app UI](../issues/20261005-x02-app-developer-tools.md) | [#101](https://github.com/ccheever/exact2/issues/101) | Charlie's decision | decided 2026-10-08 (Charlie, [comment](https://github.com/ccheever/exact2/issues/101#issuecomment-6055584890)): Safari inspection of development WKWebViews; Exact's own inspector stays deferred; #101 stays open with that bounded scope |
| user decision | Scope of this task | none | The user's narrowing | 2026-10-08: development-only `isInspectable` on the clone's own WKWebViews, gated on the existing development/release line, no cargo feature; the menu item stays absent as a declared difference |
| task in progress | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | #99 (to main at the end) | The clone builds on exact2 main | the feature branch builds on main |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | merged | Merged into the feature branch | merged |

`20261005-desktop-oracle-and-trace` is no longer a dependency (dropped 2026-10-08, so no link to it stays here: records-sync PR #323 moves it to `tasks/closed/`): no menu is compared, and the oracle is not built (user decision 2026-10-06).

Related, not a prerequisite: main [#309](https://github.com/ccheever/exact2/pull/309) (merged to main on 2026-10-08 as `f2f0e7092`; not in the T3 branch until a main-adoption round) gives Exact's own `iframe` web views the same
development-only opt-in; the clone takes it with main in a later adoption round, with no change here ("Outcome").

## Issue assessment at preparation

Checked 2026-10-08 against the feature branch at `732f0e3f3` (merged to `84a52dde0` before the final checks).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X2](../issues/20261005-x02-app-developer-tools.md), #101 | Inspector for the app UI; DEFERRED rule | decided 2026-10-08 | resolved for this task: only the app-side web view flag is allowed, and that needs no framework change | none; #101 stays open upstream for its bounded scope |
| [X26](../issues/20261005-x26-app-menu-control.md) | App menu control | `EXACT2-GAPS.md` X26 | none: no menu item is added | none |

## Implementation notes

- The gate reads the bundle once (`static let`), as `T3LocalPolicy` does for the server; a test passes `enabled:` to `mark` to check the
  release path without a second process, and the probe runs the same code inside a bundle that carries the marker.
- The base made only the terminal inspectable, and only when `EXACT_ASSETS` (set by the host's `--run` and the agent) or
  `T3_TERMINAL_INSPECTABLE=1` was in the environment: a development copy opened with `open` was not inspectable, the HTML preview and
  the Mermaid renderer never were, and the flag followed the environment rather than the build. Now the build decides for all three.
- The stderr line is one per web view creation. It is how a drive reads the flag in a running app (the agent's `state` does not carry
  native status); `T3LocalBackend` and `T3AppControl` log the same way (`t3.local:`, `t3.activation:`).
- Safari lists a web view only once it is inspectable and Safari's Develop menu is enabled (Safari › Settings › Advanced › "Show
  features for web developers"); the offscreen Mermaid renderer is listed while it exists, under the app.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Platform | Result and proof |
| --- | --- | --- | --- | --- | --- |
| Every web view inspectable in a development build | AppKit r6-media binary (README recipe), no `distribution.json` beside it, `EXACT_ASSETS` and `T3_TERMINAL_INSPECTABLE` unset | `WebInspectionTests.testEveryWebViewTheModuleCreatesFollowsTheBuild`; the probe (`01`) on the base and the branch | terminal, html-preview and mermaid `isInspectable == true` | macOS | **pass**. Base: terminal false (true only with `EXACT_ASSETS`), html-preview false, mermaid never set; branch: all three true. [01-probe-before-after.txt](https://github.com/ccheever/exact2/blob/7b77d2c2f3bbedad7b58d0f2d59d40592a83dbbd/app-developer-tools/01-probe-before-after.txt), [02-appkit-tests.txt](https://github.com/ccheever/exact2/blob/def4fed450489d326048e137ffe992856ec6f62c/app-developer-tools/02-appkit-tests.txt) |
| Release builds are never inspectable | The probe inside bundles carrying the packaged marker, a production-trust receipt, and the shipped receipt `shippedReceipt()` (`host/apple/build.mjs`) makes of the real development receipt; scratch Resources folders | the probe; `testThePackagedFlavorIsTheReleaseLine`, `testTheReceiptMarksProductionAndDistributedBundles`, `testMarkSetsAndClearsTheFlag` | `T3WebInspection.enabled == false`; all three views `isInspectable == false`; the real development receipt stays true | macOS | **pass** (01, [01b-probe-release-lines.txt](https://github.com/ccheever/exact2/blob/67537475c671705d466f5fe83e03772ef2318be3/app-developer-tools/01b-probe-release-lines.txt), [02b-appkit-tests.txt](https://github.com/ccheever/exact2/blob/80692bb6afd895ab8d196701d32dcc9965f6c919/app-developer-tools/02b-appkit-tests.txt)). Live: the packaged copy of the app bundle took the packaged policy, which reads the same marker (its server on `T3CODE_PORT` 16402, the runtime unpacked into `T3CODE_HOME`): [03-live-drive.txt](https://github.com/ccheever/exact2/blob/013674668e63b15c2640f866dbc58ffb6bd6d9c3/app-developer-tools/03-live-drive.txt), [04-live-drive-flavors.png](https://raw.githubusercontent.com/ccheever/exact2/cad5f46982a5a529545e7acb202ab800db274a3e/app-developer-tools/04-live-drive-flavors.png) |
| No environment override | Probe in every kind of build | `EXACT_ASSETS` and `T3_TERMINAL_INSPECTABLE=1` set | the flag follows the build only | macOS | **pass** (01: development true with or without them; packaged false with them) |
| Existing behavior of the three views | AppKit binaries | terminal, r6-media, mermaid (against a lane T3 server, pinned release, `127.0.0.1:16404`) | unchanged | macOS | **pass**: terminal 38 (1 skipped by design: `T3_TERMINAL_SCALE`), r6-media 9 (5 existing + 4 new), mermaid 10 checks (02, 02b) |
| In-app readback | Lane build, agent drive, the terminal drawer of a draft | `logs` after opening the terminal | `app: t3.inspection: terminal web view isInspectable=true (development build)`; `=false (release build)` for the packaged copy | macOS | **pass** (real-input session 2, 2026-10-08 20:28–20:30 KST; the agent drive could not open the terminal, 03): launched with `open` and the lane PATH, a real click on the terminal toggle; `dev.err`: `t3.inspection: terminal web view isInspectable=true (development build)`; the release copy's `packaged.err`: `t3.inspection: terminal web view isInspectable=false (release build)`. [10-terminals-dev-and-release.png](https://raw.githubusercontent.com/ccheever/exact2/15a6ff874f12cddfb887ab978c312c9f2b98deb0/app-developer-tools/10-terminals-dev-and-release.png), [12-safari-session-2.txt](https://github.com/ccheever/exact2/blob/adc322dd8995a05eb17dc0b5804c9ae3d000edd0/app-developer-tools/12-safari-session-2.txt) |
| Safari lists the development build's web view | Lane copy of the development build, a terminal open; Safari's Develop menu | Develop › this Mac › the lane app | the lane app and its terminal page (`t3-terminal://`) are listed; choosing the page opens Web Inspector on it | macOS | **pass** (20:30 KST, user-approved setting on): Develop › "Daehyeon's MacBook Pro / macOS 26.6.2" lists "T3 Code (Lane Devtools)" with its page "host — terminal-host.html" (read by System Events and on screen). [09-develop-menu.png](https://raw.githubusercontent.com/ccheever/exact2/1548904b0d042fa427b558100db8698b3a471b4b/app-developer-tools/09-develop-menu.png), [12-safari-session-2.txt](https://github.com/ccheever/exact2/blob/adc322dd8995a05eb17dc0b5804c9ae3d000edd0/app-developer-tools/12-safari-session-2.txt). Before the approval: 07, 08 (menu off). Without Safari: 05 |
| Safari does not list the packaged build | The packaged copy (marker added), a terminal open | Develop menu | the packaged copy is not listed | macOS | **pass**: the release copy ran with its terminal open at the same time and is not in that submenu. The setting was turned off again at 20:31 (read back: unchecked, no Develop menu). [09-develop-menu.png](https://raw.githubusercontent.com/ccheever/exact2/1548904b0d042fa427b558100db8698b3a471b4b/app-developer-tools/09-develop-menu.png), [11-safari-setting-on-off.png](https://raw.githubusercontent.com/ccheever/exact2/fbc17785498a0799228d5359a5c90f8217da645f/app-developer-tools/11-safari-setting-on-off.png), [12-safari-session-2.txt](https://github.com/ccheever/exact2/blob/adc322dd8995a05eb17dc0b5804c9ae3d000edd0/app-developer-tools/12-safari-session-2.txt) |
| The View menu item stays absent | — | — | no Toggle Developer Tools item; `T3Menus.swift` and `R8KeysMenus.swift` unchanged | macOS | **pass by decision** (#101, user 2026-10-08): declared difference in `EXACT2-GAPS.md` X2 |
| Standard gates | `git add -A` | `bun test examples/t3-code`, strict `tsc`, `contract build`, `cargo test -p t3-code-macos --lib`, the affected AppKit binaries, `bun scripts/caps.mjs`, the five repository checks | green | macOS | **pass**: [06-checks.txt](https://github.com/ccheever/exact2/blob/897fb925c58285c9911973c6fd6783d6448f00e6/app-developer-tools/06-checks.txt) |

Former rows dropped with the scope: menu order against the oracle, toggle, inspector content (option A), build scope of the item, keyboard
and accelerator, menu states.

## Progress

- 2026-10-08: decision applied. Gate and three call sites written; AppKit tests added; before/after probe; the r6-media, terminal and
  mermaid binaries pass. One live drive and its retry: both flavors ran on lane servers; the in-app readback was not reached (terminal
  toggle inert to the agent). Feature branch merged at `84a52dde0`.
- 2026-10-08 (resumed): main #309 merged (`f2f0e7092`); the base merged at `0e2901aec` (#312, #308). Safari row: Safari's Develop
  menu is off on this Mac; not turned on (user approval needed).
- 2026-10-08 (session 2): the user approved the setting for the check. Safari lists the development copy's terminal page and not the
  release copy; both `t3.inspection` lines read back from running copies; the setting is off again. Every row passes.
  After the base merge (`0e2901aec`): `bun test examples/t3-code` 3232 pass / 1 skip / 0 fail, strict `tsc` clean, `contract build`
  4478 slots, AppKit r6-media 9/0, `cargo test -p t3-code-macos --lib` 13/0, caps green.
- 2026-10-08 (resumed): the release line aligned with main #309 (production trust and distributed bundles, read from the receipt;
  `caf50cab9`, a fourth test); records updated (X2 in `EXACT2-GAPS.md`, the X2 issue record and its two README rows, `STATUS.md`); the
  stale link to `desktop-oracle-and-trace` dropped with the dependency (#323 has not merged); draft PR #326.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (implementation) | `58576a847`, merged `a37578082` (feature branch `84a52dde0`) | AppKit: r6-media 8/0 (3 new), terminal 38/0 (1 skipped by design), mermaid 10 ok against a lane server; probe before/after (base `732f0e3f3`); `bun test examples/t3-code` 3142 pass / 1 skip / 0 fail; strict `tsc` clean; `contract build` 3938 slots, 46 resources; `cargo test -p t3-code-macos --lib` 13/0; five checks green on `efeaf53a3` (build, test 3521 pass / 0 fail / 34 ignored, clippy, fmt, caps, boot) | 01, 02, 06 | none |
| 2 (webinspectord) | — | Read Safari's listing without Safari: refused (no debugger entitlement; listings `<private>` in the log) | 05 | needs Safari's UI: real-input batch |
| 4 (#309 alignment) | `caf50cab9` | Release line extended to production trust and distributed bundles (receipt); probe across the four bundles; AppKit r6-media 9/0, terminal 38/0 (1 skipped); `bun test examples/t3-code` 3142 / 1 skip / 0 fail; strict `tsc` clean; `contract build` 3938 slots; five checks green (cargo test 3521 passed, 0 failed, 34 ignored) | 01b, 02b, 06 | none |
| 5 (Safari real input) | lane copies built from `caf50cab9` (not launched) | Lock taken 20:23 KST after #307; Safari opened; its menu bar has no Develop menu (web developer features off); Safari quit; lock released 20:23:42; Safari's setting unchanged | 07, 08 | the user's approval to turn on Safari's web developer features |
| 6 (Safari real input, approved) | lane copies built from `caf50cab9` | Lock 20:27:16–20:31:34 KST. The setting was already on when the session began (Safari started 20:25:45 by someone else). Dev copy pid 44391 and release copy pid 45167 with terminals open; the Develop menu lists only the dev copy; the setting turned off and read back; Safari quit; both copies quit; a Documents prompt from the lane dev copy denied | 09, 10, 11, 12 | none |
| 3 (live drive + retry) | the bundle at `a37578082` | Development build (`T3_LOCAL_HOME`, 16401) and its packaged copy (`distribution.json`, `T3CODE_HOME`, 16402) both reached beta / New thread; the packaged copy took the packaged policy (port, runtime path); the terminal toggle was inert to the agent (attempt 1: chat header toggle `hidden or inert`; retry: `panel-toggle-terminal` absent without a right panel), so no web view and no `t3.inspection` line. An earlier start stopped before launching (`rustc did not report its host target` under `env -i`) | 03, 04 | one-session rule; the readback joins the real-input batch |

## Next action

Draft PR [#326](https://github.com/ccheever/exact2/pull/326) against `feat(example)/t3-code`; every acceptance row passes. Review, then the
coordinator flips it to ready. The steps below stay as the record of how the Safari rows were run (session 2, 2026-10-08).

## Real-input batch steps

Run on 2026-10-08 (session 2, results in "Attempts and evidence"). One session, holding the real-input lock
(`/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-ui-parity/lanes/.realinput-lock`, owner "app-developer-tools: Safari
Develop menu"). Lane ports 16400–16449; never `~/.t3`, 3773 or the `t3code` scheme.

1. Build in this worktree (`/Users/daehyeonmun/orca/workspaces/exact2/t3-code-app-developer-tools`):
   `export PATH="$HOME/.bun-1.4.2/bin:$PATH" EXACT_APP_DIR=$PWD/examples/t3-code; bun examples/t3-code/terminal-host/build.mjs;
   bun examples/t3-code/stage-runtime.mjs; bun host/apple/build.mjs t3-code-macos --bundle`.
2. Lane: `bun target/devtools/seed.mjs t3-home-seed 16403` makes `target/devtools/lane` (isolated home, Codex, Claude, XDG, tmp, the
   pinned runtime) and a T3 home with two projects (alpha, beta). Copy it to `t3-home-dev` and `t3-home-packaged`.
3. Development copy: copy the built `target/clients/*/com.exact.t3code.macos/macos/T3 Code (Exact).app` to
   `target/devtools/lane/apps/T3 Code (Lane Devtools).app`; Info.plist `CFBundleIdentifier` `com.exact.t3code.macos.lanedevtools.dev`,
   `CFBundleName` "T3 Code (Lane Devtools)"; `codesign --force --deep -s -`. Launch with
   `open -n -a "<that app>" --stderr <lane>/dev.err --env HOME=<lane>/home --env PATH=<lane>/bin:/usr/bin:/bin:/usr/sbin:/sbin
   --env SHELL=/bin/sh --env TMPDIR=<lane>/tmp/ --env CODEX_HOME=<lane>/codex --env CLAUDE_CONFIG_DIR=<lane>/claude
   --env XDG_CONFIG_HOME=<lane>/xdg/config (and DATA, CACHE, STATE) --env T3CODE_TELEMETRY_ENABLED=false
   --env T3_LOCAL_HOME=<lane>/t3-home-dev --env T3_LOCAL_PORT=16401 --env T3_LOCAL_RUNTIME_DIR=<lane>/runtime/0.0.46-nightly.20261005.2667`.
   Record the pid (`pgrep -n -f "Lane Devtools.app"`).
4. In the window: the sidebar's New thread (compose icon), then the terminal toggle (top right, the panel-bottom icon) or ⌘J. Read
   back: the drawer shows a shell prompt, and `<lane>/dev.err` has `t3.inspection: terminal web view isInspectable=true (development build)`.
5. Safari (`open -a Safari`; first confirm the user approved its web developer features, off on 2026-10-08): the Develop menu, then this Mac's submenu. Read back: "T3 Code (Lane Devtools)" with its terminal page
   (`t3-terminal://…`) is listed. `screencapture -x` with the submenu open → `app-developer-tools/06-safari-develop-dev.png`. Choose the
   page: Web Inspector opens on the terminal page (screenshot `07-web-inspector-terminal.png`, then close the inspector). If there is no
   Develop menu, stop and ask the user before turning on Safari › Settings › Advanced › "Show features for web developers".
6. Quit the development copy (⌘Q). Packaged copy: the same copy steps to `T3 Code (Lane Devtools Packaged).app` (bundle id
   `com.exact.t3code.macos.lanedevtools`), plus `printf '{"flavor":"packaged"}' > "<app>/Contents/Resources/distribution.json"` before
   `codesign`. Launch as in step 3 but with `--env T3CODE_HOME=<lane>/t3-home-packaged --env T3CODE_PORT=16402` and no `T3_LOCAL_*`.
   Step 4 again; read back `t3.inspection: terminal web view isInspectable=false (release build)` in its stderr file.
7. Safari's Develop menu again. Read back: "T3 Code (Lane Devtools Packaged)" is not listed (screenshot `08-safari-develop-packaged.png`).
8. Quit it; `defaults delete` both lane bundle ids if a plist was written; release the lock. Upload the screenshots and a short text
   record (pids, times, what was read back) to `t3-code-evidence/app-developer-tools/`.
