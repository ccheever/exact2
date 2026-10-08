---
name: 20261005-app-developer-tools
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-app-developer-tools
pr_url: null
verified_commit: null
---

# Safari's Web Inspector on the clone's web views in development builds; View › Toggle Developer Tools stays absent

## Outcome

Rewritten 2026-10-08 for the user's decision, which narrowed this task to Charlie's ruling on
[#101](https://github.com/ccheever/exact2/issues/101) (X2): "Allow Safari inspection of development WKWebViews; keep Exact's
own inspector deferred. [...] Use public isInspectable only in development, with release gating verified. Keep
tree/layout/state/logs/perf as the native inspection path; no new inspector window or inspect command."

In a development build, every WKWebView the clone's own module creates is `isInspectable`, so Safari's Develop menu lists
it and Web Inspector can attach: the terminal (`T3TerminalView`), the rendered-HTML preview (`R6MediaPreview`) and the
offscreen Mermaid renderer (`T3TimelineMermaid`). The packaged (release) build's web views are never inspectable. The line
between the two is the clone's existing development/release distinction, the flavor marker the embedded server already
reads: a bundle whose `Contents/Resources/distribution.json` is `{"flavor":"packaged"}` is the packaged build
(`T3LocalPolicy.packaged`); every other build is a development build. No environment variable changes the answer and there
is no cargo feature. (The reference's own line is `isDevelopment` = a Vite dev server URL; the clone has no dev server, and
the packaged marker is its equivalent of Electron's packaged app.)

The reference's View › Toggle Developer Tools item stays absent for good, a declared difference (#101): an inspector for the
app's own UI is not built in Exact, and the agent API (`tree`, `layout`, `state`, `logs`, `perf`) stays the native
inspection path. The View menu is unchanged by this task.

Superseded by the decision (the record before 2026-10-08): the menu item, its toggle, option A (a host inspector for the
Contract tree), option C (a disabled item) and the menu-order rows.

## Scope and exclusions

Included:

1. **The gate** (`modules/apple/T3WebInspection.swift`): `enabled`, read once from the bundle; `permits(resources:)`, the
   negation of `T3LocalPolicy.packaged(resources:)`; `mark(web, kind)`, which sets `isInspectable` and writes
   `t3.inspection: <kind> web view isInspectable=<flag read back from the view> (<development|packaged> build)` to stderr
   (the agent's `logs` host lines).
2. **Every creation path** calls `mark` once, before its first load: `T3TerminalView.init` (this replaces the old gate,
   `EXACT_ASSETS` or `T3_TERMINAL_INSPECTABLE=1`, so a development copy opened with `open` is inspectable too),
   `R6MediaPreview.webView(name:)`, and `T3TimelineMermaid.renderer()` (a new factory that `load(base:)` uses). Both popup
   delegates (`createWebViewWith`) return nil, so the module creates no other web view.
3. **Tests**: `macos/tests/r6-media/inspection.swift` (`WebInspectionTests`, 3 tests, run by the r6-media binary).
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
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | #99 (in progress, to main at the end) | The clone builds on exact2 main | the feature branch builds on main |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | merged | Merged into the feature branch | merged |

`20261005-desktop-oracle-and-trace` is no longer a dependency: no menu is compared (and the oracle is not built, user decision 2026-10-06).

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
| The packaged (release) build is never inspectable | The probe binary inside `Packaged.app` whose `Contents/Resources/distribution.json` is `{"flavor":"packaged"}`; scratch Resources folders | the probe; `testThePackagedFlavorIsTheReleaseLine`, `testMarkSetsAndClearsTheFlag` | `T3WebInspection.enabled == false`; all three views `isInspectable == false`; the flavor line equals `T3LocalPolicy.packaged` | macOS | **pass** (01, 02). Live: the packaged copy of the app bundle took the packaged policy, which reads the same marker (its server on `T3CODE_PORT` 16402, the runtime unpacked into `T3CODE_HOME`): [03-live-drive.txt](https://github.com/ccheever/exact2/blob/013674668e63b15c2640f866dbc58ffb6bd6d9c3/app-developer-tools/03-live-drive.txt), [04-live-drive-flavors.png](https://raw.githubusercontent.com/ccheever/exact2/cad5f46982a5a529545e7acb202ab800db274a3e/app-developer-tools/04-live-drive-flavors.png) |
| No environment override | Probe in both builds | `EXACT_ASSETS` and `T3_TERMINAL_INSPECTABLE=1` set | the flag follows the build only | macOS | **pass** (01: development true with or without them; packaged false with them) |
| Existing behavior of the three views | AppKit binaries | terminal, r6-media, mermaid (against a lane T3 server, pinned release, `127.0.0.1:16404`) | unchanged | macOS | **pass**: terminal 38 (1 skipped by design: `T3_TERMINAL_SCALE`), r6-media 8 (5 existing + 3 new), mermaid 10 checks (02) |
| In-app readback | Lane build, agent drive, the terminal drawer of a draft | `logs` after opening the terminal | `app: t3.inspection: terminal web view isInspectable=true (development build)`; `=false (packaged build)` for the packaged copy | macOS | **not verified**: both flavors reached the app on their lane servers, but the agent could not open the terminal (the header toggle was inert to it, `view 122 is hidden or inert`; the panel toggle exists only with a right panel open), so no web view was created; the one-session rule (one drive and its retry) stopped there. Moved to the real-input batch (steps below). 03, 04 |
| Safari lists the development build's web view | Lane copy of the development build, a terminal open; Safari's Develop menu | Develop › this Mac › the lane app | the lane app and its terminal page (`t3-terminal://`) are listed; choosing the page opens Web Inspector on it | macOS | **deferred to the real-input batch — screen locked (user away)**. Attempt without Safari's UI: webinspectord redacts listings in its log and refuses a debugger client without Apple's private entitlement, so the listing cannot be read from outside Safari ([05-webinspectord-attempt.txt](https://github.com/ccheever/exact2/blob/d52269b1e8153a75c00c575531688aa0f2c22ed4/app-developer-tools/05-webinspectord-attempt.txt)) |
| Safari does not list the packaged build | The packaged copy (marker added), a terminal open | Develop menu | the packaged copy is not listed | macOS | **deferred to the real-input batch — screen locked (user away)**; the flag is off on every creation path (01) |
| The View menu item stays absent | — | — | no Toggle Developer Tools item; `T3Menus.swift` and `R8KeysMenus.swift` unchanged | macOS | **pass by decision** (#101, user 2026-10-08): declared difference in `EXACT2-GAPS.md` X2 |
| Standard gates | `git add -A` | `bun test examples/t3-code`, strict `tsc`, `contract build`, `cargo test -p t3-code-macos --lib`, the affected AppKit binaries, `bun scripts/caps.mjs`, the five repository checks | green | macOS | green: see "Attempts and evidence" (06-checks.txt not uploaded yet) |

Former rows dropped with the scope: menu order against the oracle, toggle, inspector content (option A), build scope of the item, keyboard
and accelerator, menu states.

## Progress

- 2026-10-08: decision applied. Gate and three call sites written; AppKit tests added; before/after probe; the r6-media, terminal and
  mermaid binaries pass. One live drive and its retry: both flavors ran on lane servers; the in-app readback was not reached (terminal
  toggle inert to the agent). Feature branch merged at `84a52dde0`. Records rewritten (this file, X2 issue and README rows,
  `EXACT2-GAPS.md` X2, `STATUS.md`).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (implementation) | `58576a847`, merged `a37578082` (feature branch `84a52dde0`) | AppKit: r6-media 8/0 (3 new), terminal 38/0 (1 skipped by design), mermaid 10 ok against a lane server; probe before/after (base `732f0e3f3`); `bun test examples/t3-code` 3142 pass / 1 skip / 0 fail; strict `tsc` clean; `contract build` 3938 slots, 46 resources; `cargo test -p t3-code-macos --lib` 13/0; five checks green on `efeaf53a3` (build, test 3521 pass / 0 fail / 34 ignored, clippy, fmt, caps, boot) | 01, 02, 06 | none |
| 2 (webinspectord) | — | Read Safari's listing without Safari: refused (no debugger entitlement; listings `<private>` in the log) | 05 | needs Safari's UI: real-input batch |
| 3 (live drive + retry) | the bundle at `a37578082` | Development build (`T3_LOCAL_HOME`, 16401) and its packaged copy (`distribution.json`, `T3CODE_HOME`, 16402) both reached beta / New thread; the packaged copy took the packaged policy (port, runtime path); the terminal toggle was inert to the agent (attempt 1: chat header toggle `hidden or inert`; retry: `panel-toggle-terminal` absent without a right panel), so no web view and no `t3.inspection` line. An earlier start stopped before launching (`rustc did not report its host target` under `env -i`) | 03, 04 | one-session rule; the readback joins the real-input batch |

## Next action

Resume from here (2026-10-08, stopped at the coordinator's request, usage limit): the code, tests, README and handoff
notes are committed and pushed on `feat(example)/t3-code-app-developer-tools`; evidence 01–05 is uploaded to
`t3-code-evidence/app-developer-tools/`; every check is green (above). Not done yet: (1) the record edits outside this file —
`EXACT2-GAPS.md` X2 (summary row, the X1/X2 note, the X2 section: decided, built, item absent as a declared difference), the X2
issue file (status decided, `upstream_url` #101, status section) and its two rows in `issues/README.md`, `STATUS.md` (policy line,
open-tasks row, "X1, X2 and X48 block", a real-input batch row); (2) upload `06-checks.txt`; (3) fetch and merge
`origin/feat(example)/t3-code` again, re-run caps, push; (4) open the draft PR against `feat(example)/t3-code` (acceptance table
from this file, evidence links above), then set `pr_url`, `delivery: draft`, `verified_commit`. No lane server or app is running;
the real-input lock was never taken. Lane files are under `target/devtools/` (seed.mjs, drive.sh, apptest.sh, probes).


Draft PR open against `feat(example)/t3-code`. Left for the coordinator's real-input batch (screen locked): the three rows below. If
Safari's Develop menu is off on this Mac, turning it on is the user's Safari setting: ask first.

## Real-input batch steps

Deferred — screen locked (user away). One session, holding the real-input lock
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
5. Safari (`open -a Safari`): the Develop menu, then this Mac's submenu. Read back: "T3 Code (Lane Devtools)" with its terminal page
   (`t3-terminal://…`) is listed. `screencapture -x` with the submenu open → `app-developer-tools/06-safari-develop-dev.png`. Choose the
   page: Web Inspector opens on the terminal page (screenshot `07-web-inspector-terminal.png`, then close the inspector). If there is no
   Develop menu, stop and ask the user before turning on Safari › Settings › Advanced › "Show features for web developers".
6. Quit the development copy (⌘Q). Packaged copy: the same copy steps to `T3 Code (Lane Devtools Packaged).app` (bundle id
   `com.exact.t3code.macos.lanedevtools`), plus `printf '{"flavor":"packaged"}' > "<app>/Contents/Resources/distribution.json"` before
   `codesign`. Launch as in step 3 but with `--env T3CODE_HOME=<lane>/t3-home-packaged --env T3CODE_PORT=16402` and no `T3_LOCAL_*`.
   Step 4 again; read back `t3.inspection: terminal web view isInspectable=false (packaged build)` in its stderr file.
7. Safari's Develop menu again. Read back: "T3 Code (Lane Devtools Packaged)" is not listed (screenshot `08-safari-develop-packaged.png`).
8. Quit it; `defaults delete` both lane bundle ids if a plist was written; release the lock. Upload the screenshots and a short text
   record (pids, times, what was read back) to `t3-code-evidence/app-developer-tools/`.
