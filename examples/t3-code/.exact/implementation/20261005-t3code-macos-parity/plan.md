# 20261005-t3code-macos-parity

Specification: [spec](spec.md). Findings: [research](research.md). Framework issues (filed as issues only; no framework PRs):
[issues](issues/README.md).

Goal: re-implement the T3 Code desktop app (Electron) with exact2 as the example
`examples/t3-code`, using T3 Code's own data/client logic and its own server, with
every feature except the spec exclusions, and a downloadable build that runs on another Mac.

## SSH feature acceptance, 2026-10-06

[SSH password and remote Open](tasks/closed/20261005-ssh-password-and-remote-open.md#final-acceptance-2026-10-06),
PR #157: **verified / open for review**. Its durable evidence covers real SSH and
remote editor use, secure prompt behavior, oracle/IME, FIFO, Markdown/Files and the
last missing-route UI fixture. Implementation commit: `762f501cb`.
The PR remains unmerged. Feature acceptance does not close the parent's separate
generic oracle/trace infrastructure or full T0 matrix.

## Knowledge snapshot

Library revision: `20261005-platforms-v3` (platforms, performance, testing-and-debugging
reviewed at exact2 `e45434ff217f`; other topics at `fd1c879072aa`; runtime-tested revision:
none). Framework compatibility evidence: the clone ran on exact2 `c1522fdac` (round 11);
the new base is a pinned `origin/main` chosen in `20261005-clone-on-exact2-main` (main was
`9d442ecba` on 2026-10-05). Whether the library's reviewed revisions match that pin is
**unknown**; `prepare` checks it per ticket. Most native capabilities the clone needs are
unknown in the library (research "Knowledge-update handoff"; spec open decision 1).

Reference product pin: T3 Code `1e2ecbd975` (no fetch made). The embedded server pin is
chosen in `20261005-embedded-server-runtime`.

## PR workflow, 2026-10-06 (supersedes every branch rule below)

The user's decision (2026-10-06, model A): one feature branch, no stacked PRs.

- The clone lives at `examples/t3-code/` (main's `examples/<app>/` layout; the AppKit crate is
  `examples/t3-code/macos`, package `t3-code-macos`). The feature branch
  `feat(example)/t3-code` carries it; PR #99 takes it to `main` (it replaced #97).
- Every task is one PR into `feat(example)/t3-code`, from a branch
  `feat(example)/t3-code-<task>` created from the current feature branch in its own Orca
  worktree. Independent tasks run in parallel. A task that needs another task starts after
  that task's PR merges into the feature branch. No PR is based on another unmerged task branch.
- The feature branch merges `main` regularly. #99 goes to `main` at milestones or at the end.
- Framework gaps are GitHub issues (#100–#141, filed 2026-10-06). When a fix lands on `main`,
  the feature branch merges `main` and a `feat(example)/t3-code-adopt-<issue>` task removes the workaround.
- Where a task document says "integration branch" or `daehyeon/t3-code`, read `feat(example)/t3-code`.
- Verification evidence logs are not committed. The pre-cleanup history, including the
  `evidence/` trees, stays on `daehyeon/t3code-parallel-features`.

## Parallel implementation, 2026-10-06

The user requested parallel work on independent tasks without unresolved issue prerequisites.
For this local implementation wave, work starts from the preserved round-12 source snapshot
on `daehyeon/t3code-parallel-features`, with a task-named branch/worktree for each worker.
The original `daehyeon/t3-code` branch is preserved and is not reset. The main import,
hot-file-split and desktop-oracle PR gates remain pending for delivery and final acceptance;
no merged prerequisite is claimed. Implementation is separated from those procedural gates
for this wave; missing runtime/tooling evidence remains explicitly unverified.

Selected: client-activity-reporting, right-panel-tab-menu, upstream-timeline-and-markdown.
Their listed framework issues have existing app workarounds or are excluded features.
Workers own separate feature files; the coordinator alone edits shared root/client/native
registration files and plan indexes. Library revision: `20261005-platforms-v3`, with the
checkout's code as framework authority. Agents inherit the current host model and reasoning.
No scheduler, external publication or pixel-fidelity loop is part of this request.

The three implementation commits are integrated locally: activity `d8c6f1241`, tabs
`da446560d` plus Contract compatibility fixes, and timeline `0816bec75` plus concrete
activity-field defaults. Shared root mutations, native hook lifecycle, skill projection,
and monotonic source time are wired in the integration branch. Independent reviews
covered request/cache identity, async close races and native ownership.

Combined checks: 1,197 Bun tests, strict TypeScript, Contract compilation (2,146 slots,
42 resources), 10 app Rust tests, formatting, staged caps and boot pass. Root Cargo
build and clippy passed; root tests' pinned-Bun failures passed when the affected
packages were rerun with Bun 1.4.2. Full live backend acceptance remains unverified.

## Implementation order

Groups run in order; tickets inside a group may run in parallel (the user's execution
decision: one workflow per phase with at most 8 lanes + 1 integrator, so a large group runs
in waves). Every task PR targets `feat(example)/t3-code`; dependent work starts from it after its
prerequisites merge (no stacks, no integration branch).

Common prerequisites, not repeated per row: every feature ticket (group 2 and later) needs
`clone-on-exact2-main`, `hot-file-split` and `desktop-oracle-and-trace` merged.

| Order / parallel group | Task | Complete outcome | Repository | Dependencies | Why this boundary and order | Verification |
| --- | --- | --- | --- | --- | --- | --- |
| 0a | [20261005-round12-wrapup](tasks/20261005-round12-wrapup.md) | Round-12 items work or are recorded; tree green on `c1522fdac` | exact2 (mc-orch worktree, untracked; no PR) | — | User chose to finish round 12 before the move | clone checks, agent drives, AppKit |
| 0b | [20261005-clone-on-exact2-main](tasks/20261005-clone-on-exact2-main.md) | Clone at `examples/t3-code` on `main`; builds, runs, verifiable | exact2 `feat(example)/t3-code` → `main` | round12-wrapup; branch-reset confirmation | Every later PR needs the clone on main | clone checks, five checks, launch + pair + drive, matrix |
| 1 | [20261005-desktop-oracle-and-trace](tasks/20261005-desktop-oracle-and-trace.md) | Reference desktop oracle, trace proxy/diff, RPC tally | exact2 (tools in `target/` or `tools/`, U23) | clone-on-exact2-main; apparatus + installs approval | Desktop app is the fidelity oracle; RPC tally found the largest gap | oracle shot, trace diff finds known G1 gap |
| 1 | [20261005-hot-file-split](tasks/closed/20261005-hot-file-split.md) | Shared files split by area; room under the line cap | exact2 | clone-on-exact2-main | Parallel PRs would collide on `client.ts`/`app.contract` and hit the cap | no-change matrix, caps |
| 2 | [20261005-client-activity-reporting](tasks/closed/20261005-client-activity-reporting.md) | Server liveness: activity reports and slash-command refresh | exact2 | common | Highest-impact gap; small and independent | trace cadence + scopes, server effect |
| 2 | [20261005-composer-fidelity](tasks/closed/20261005-composer-fidelity.md) | Ultrathink, compact composer menu, queued-edit attachments, subagent tooltip, small items | exact2 | common | One surface (composer) | oracle pairs, ported layout tests |
| 2 | [20261005-desktop-shell-details](tasks/closed/20261005-desktop-shell-details.md) | Pickers, system locale, ⌘W repeat block, quit-hold conceal, full-screen inset | exact2 | common | Desktop bridge remainder that needs no embedded server | AppKit, attended |
| 2 | [20261005-diff-review-engine](tasks/closed/20261005-diff-review-engine.md) | Diff tree, large diffs, line comments, Cite | exact2 | common | No gh needed; the PR Code tab reuses it | ported tests, oracle pairs |
| 2 | [20261005-embedded-server-runtime](tasks/20261005-embedded-server-runtime.md) | Official server runtime fetched at build, bundled, launched and supervised | exact2 | common; U3 decided (CLI archive); issues X4, X6 | Highest technical risk; desktop work and the portable build depend on it | process/port/readiness/restart/quit checks |
| 2 | [20261005-environment-routes](tasks/closed/20261005-environment-routes.md) | Multi-route environments, learned routes, no duplicate saved rows | exact2 | common | Changes the saved-environment model; PR routing and server update build on it | ported routes tests, trace, relaunch |
| 2 | [20261005-fake-github-fixture](tasks/20261005-fake-github-fixture.md) | Fake gh supports every read/write the server uses, with permission variants | exact2 (tools in `target/` or `tools/`, U23) | common; apparatus approval | All PR tickets verify against it; one owner avoids `gh.mjs` conflicts | server-driven calls log + state diffs |
| 2 | [20261005-floating-device-player](tasks/closed/20261005-floating-device-player.md) | Floating player drag, resize, avoidance | exact2 | common | Independent surface | ported layout tests, attended drag |
| 2 | [20261005-legacy-sidebar](tasks/closed/20261005-legacy-sidebar.md) | "Sidebar (legacy)" switch works as the reference | exact2 | common | Large, separate surface | oracle pairs |
| 2 | [20261005-live-automations-and-clones](tasks/closed/20261005-live-automations-and-clones.md) | Live automations and tracked project clones | exact2 | common | Two streams; independent | trace, effect checks |
| 2 | [20261005-main-fix-adoption](tasks/closed/20261005-main-fix-adoption.md) | Workarounds for main-fixed limits removed; gap doc current | exact2 | common | Removes code that feature PRs would otherwise edit around | tap proofs, matrix |
| 5 (wave 4) | [20261007-adopt-main-fixes-input](tasks/closed/20261007-adopt-main-fixes-input.md) | main's input fixes (#110, #111, #132, #133, #134) adopted where they cover the clone; #120 recorded as not planned | exact2 | origin/main merged into the feature branch (`4cdb8aa63`) | Adoption task per PR workflow: workarounds removed only where main's fix covers them | ported checks, AppKit, one before/after drive |
| adopt (wave 4) | [20261007-adopt-main-fixes-shell](tasks/closed/20261007-adopt-main-fixes-shell.md) | main's #173 (ATS), #164 (frame restore), #177 (image load/error) and #174 (hover follows layout) adopted; #170 (X45) and #159 checked | exact2 | main merged into the feature branch (`dbae6c2e0`) | Removes workarounds for issues main closed (#106, #113, #121, #139) | drive of the assembled `.app`, AppKit, clone checks |
| fix (wave 4) | [20261007-let-go-banner](tasks/closed/20261007-let-go-banner.md) | A request Exact let go (main `a19523a57`, FetchError `Aborted`) shows no banner, toast or failure field; real failures still do | exact2 | main merged into the feature branch (`4cdb8aa63`) | Regression: every #182 drive showed the banner | clone tests, one before/after drive |
| fix (wave 4) | [20261007-fix-minor-ui-issues](tasks/closed/20261007-fix-minor-ui-issues.md) (#191) | Side-by-side pass against T3 Code (Nightly): settings traits picker, composer menu side, filter popup, discovery retry, compaction row, settled badge, surface chooser, Diff outside git, title menu rules | exact2 | feature branch tip `0a7ca50ad` | Small differences found by using both apps; missing features become their own tasks | before/after drives, unit tests |
| 2 | [20261007-context-menu-gaps](tasks/closed/20261007-context-menu-gaps.md) (#203) | Files tree, pull request link and chat file-link context menus as the reference | exact2 | common | Found by fix-minor-ui-issues; the other reference menus match | unit tests of the items, drive record |
| 2 | [20261005-provider-sign-in-and-install](tasks/20261005-provider-sign-in-and-install.md) | Provider sign-in and runtime install flows, redacted account text | exact2 | common | Base for managed Codex, settings upkeep, usage extras and sign-in terminals | fixture streams, attended real sign-in |
| 2 | [20261005-reference-logic-tests-done-areas](tasks/20261005-reference-logic-tests-done-areas.md) | Map of every reference test file to clone modules, with done-equivalent proof (port tickets follow) | exact2 | common | Proves "same logic as T3 Code" for work that has no feature ticket | ported tests pass; mapping table |
| 2 | [20261005-remote-scopes-and-update-commands](tasks/closed/20261005-remote-scopes-and-update-commands.md) | Standard remote scopes incl. `terminal:operate`, re-pair decision, install-aware update commands | exact2 | common | Small; terminal and server-update tickets need it | trace, ported tests |
| 2 | [20261005-right-panel-tab-menu](tasks/closed/20261005-right-panel-tab-menu.md) | Right-panel tab context menu (rename, copy path, close variants) | exact2 | common | General right-panel feature; must not wait for the terminal verdict | oracle pairs, attended right-click |
| 2 | [20261005-settings-scoped-controls-and-theme-editor](tasks/closed/20261005-settings-scoped-controls-and-theme-editor.md) | Mixed switch, multi-environment settings scope, app-wide theme editor | exact2 | common | Settings controls and the multi-environment settings scope; independent | oracle pairs |
| 2 | [20261005-shiki-residuals](tasks/closed/20261005-shiki-residuals.md) | More grammars, long texts, italics; grammar generator from pinned packages | exact2 | common | Own decision (U15), budget and code surface | corpus diff, perf budget |
| 2 | [20261005-ssh-password-and-remote-open](tasks/closed/20261005-ssh-password-and-remote-open.md) | SSH password prompt (askpass) and remote Open in editor | exact2 | common | Split from shell details; SSH-specific | SSH test double, attended |
| 2 | [20261005-terminal-surface](tasks/closed/20261005-terminal-surface.md) | Go/no-go: vendored Ghostty terminal in a WKWebView with keys, IME, clipboard | exact2 | common | Highest-risk terminal step first | spike report, AppKit, attended IME |
| 2 | [20261005-upstream-timeline-and-markdown](tasks/closed/20261005-upstream-timeline-and-markdown.md) | Tool rows, file links and skill chips match `1e2ecbd975` | exact2 | common | Upstream sync for one surface (timeline + Markdown) | trace (`getTurnItem`), oracle pairs |
| 2 | [20261005-upstream-ui-sync](tasks/closed/20261005-upstream-ui-sync.md) | Small upstream UI changes (Working order, unpin icon, Azure mark, device step 0, popover z-order) | exact2 | common | Small changes grouped as one upstream sync | oracle pairs, ported tests |
| 3 | [20261005-local-primary-environment](tasks/20261005-local-primary-environment.md) | Embedded server is the primary "This machine" environment | exact2 | embedded-server-runtime | Needs the running server | relaunch, trace, oracle pairs |
| 3 | [20261005-managed-codex-chatgpt](tasks/20261005-managed-codex-chatgpt.md) | Managed Codex with ChatGPT sign-in and plan notices | exact2 | provider-sign-in-and-install | Reuses the auth flow | loopback XCTest, attended real account |
| 3 | [20261005-media-actions](tasks/closed/20261005-media-actions.md) | Media context menus, copy/save, failure fallbacks, HTML preview assets | exact2 | main-fix-adoption; issue X7 for external hosts | One surface (media) | attended right-click, effect checks |
| 3 | [20261005-pr-conversation-and-refresh](tasks/20261005-pr-conversation-and-refresh.md) | PR activity, skeletons, live refresh | exact2 | fake-github-fixture | Base model for all PR tickets | fake gh calls, trace, oracle pairs |
| 3 | [20261005-provider-settings-upkeep](tasks/20261005-provider-settings-upkeep.md) | Provider settings remainder (ACP sessions, Update all, model editor, icons) | exact2 | provider-sign-in-and-install | Shares the provider card | trace, oracle pairs |
| 3 | [20261005-reference-logic-test-ports](tasks/20261005-reference-logic-test-ports.md) | Reference `port` tests pass against the done areas (split per area at prepare) | exact2 | reference-logic-tests-done-areas | Proves the same logic as T3 Code; needs the map | ported tests, map diff |
| 3 | [20261005-server-update-banner](tasks/closed/20261005-server-update-banner.md) | Server update banner, offline banner grace and "Disconnect server", version-differ card | exact2 | remote-scopes-and-update-commands | Uses install-aware update commands | stub-server trace, oracle pairs |
| 3 | [20261005-terminal-drawer](tasks/closed/20261005-terminal-drawer.md) | Terminal sessions and the single drawer | exact2 | remote-scopes-and-update-commands, terminal-surface | Needs the surface; terminal RPCs need the `terminal:operate` scope that remote-scopes-and-update-commands adds | ported tests, attended keys |
| 3 | [20261005-thread-commands-and-keys](tasks/closed/20261005-thread-commands-and-keys.md) | Delete the worktree too, the missing key commands | exact2 | composer-fidelity | ⌥↑ needs queued edit | ported tests, trace, effect (git worktree) |
| 3 | [20261005-usage-reset-and-feedback](tasks/20261005-usage-reset-and-feedback.md) | Reset credits (composer banner bars, redeem dialog) and `/feedback` | exact2 | provider-sign-in-and-install | Uses the provider-setup fixture and redaction from the sign-in ticket | trace, ported tests |
| 4 | [20261005-app-activation](tasks/20261005-app-activation.md) | `t3 app <dir>` opens a project and thread | exact2 | local-primary-environment; E4 decision | Needs the primary environment | socket test, effect |
| 4 | [20261005-auto-balance](tasks/closed/20261005-auto-balance.md) | Auto balance and the multi-machine update banner | exact2 | server-update-banner | Reuses the update store of the banner ticket | trace, oracle pairs |
| 4 | [20261005-portable-app-download](tasks/20261005-portable-app-download.md) | Downloadable `.app` archive that runs on a clean Mac account | exact2 | embedded-server-runtime, local-primary-environment, terminal-surface; U11 decided (ad-hoc, zip, macOS 14 VM) | Proves the self-contained requirement once the server connects and the large assets exist | clean-account run |
| 4 | [20261005-pr-header-actions-and-stacks](tasks/20261005-pr-header-actions-and-stacks.md) | PR primary control, More menu, dialogs, failure hints, freshness popover, stacks | exact2 | fake-github-fixture, pr-conversation-and-refresh | Needs the PR model | fake gh calls, oracle pairs |
| 4 | [20261005-pr-writing-and-metadata](tasks/20261005-pr-writing-and-metadata.md) | Comments, reviews, edits, reviewers, labels, reactions | exact2 | fake-github-fixture, pr-conversation-and-refresh | Needs the PR model | fake gh calls, oracle pairs |
| 4 | [20261005-sign-in-terminals](tasks/closed/20261005-sign-in-terminals.md) | Terminal sign-in for ACP agents and onboarding | exact2 | provider-sign-in-and-install, terminal-drawer | Joins both | attended |
| 4 | [20261005-terminal-layout](tasks/closed/20261005-terminal-layout.md) | Terminal tabs, splits, panel surface, keys, sidebar indicator, terminal close behavior | exact2 | right-panel-tab-menu, terminal-drawer | Builds on the drawer; split from integrations for size | ported tests, attended |
| 4 | [20261005-this-machine-network-access](tasks/20261005-this-machine-network-access.md) | Network access, Tailscale HTTPS, authorized clients, pairing links | exact2 | local-primary-environment; U4 decided (relaunch, X45); Tailscale decision U9 | Needs the primary environment | effect (LISTEN, pairing), attended firewall |
| 4 | [20261005-usage-pooled-view](tasks/20261005-usage-pooled-view.md) | Pooled Usage page across connected environments, account popover with redeem, Cursor keychain enable prompt | exact2 | usage-reset-and-feedback | Reuses the redeem machinery and bars from usage-reset-and-feedback | trace, oracle pairs |
| 5 | [20261005-pr-handoffs-and-quick-actions](tasks/20261005-pr-handoffs-and-quick-actions.md) | PR panel hand-offs (Ask, Explain, Fix findings, Check out), header fold, Shift quick actions, row menu and popovers | exact2 | fake-github-fixture, pr-conversation-and-refresh, pr-header-actions-and-stacks | Split from the header ticket for size; the Code tab and links reuse the hand-offs and row menus | fake gh calls, oracle pairs, attended Shift |
| 5 | [20261005-terminal-integrations](tasks/closed/20261005-terminal-integrations.md) | Selection actions, Add to chat, terminal menus, links, scripts, Run in terminal, Open terminal | exact2 | terminal-drawer, terminal-layout | Needs tabs and the panel surface | ported tests, attended |
| 6 | [20261005-pr-code-tab](tasks/20261005-pr-code-tab.md) | PR Code tab with review threads | exact2 | diff-review-engine, fake-github-fixture, pr-conversation-and-refresh, pr-handoffs-and-quick-actions, pr-writing-and-metadata | Reuses the diff engine, the writes, and the hand-off functions of pr-handoffs-and-quick-actions | fake gh, oracle pairs |
| 6 | [20261005-pr-links-previews-and-routing](tasks/20261005-pr-links-previews-and-routing.md) | Thread links, `#N` hover cards, cross-environment routing | exact2 | environment-routes, fake-github-fixture, pr-conversation-and-refresh, pr-handoffs-and-quick-actions, pr-header-actions-and-stacks | Routing trust depends on routes; row menus from pr-handoffs-and-quick-actions | fake gh, trace |
| 7 (last) | [20261005-interface-font-size](tasks/closed/20261005-interface-font-size.md) | Root font size foundation: `rem` check, size map, shared style classes (per-area conversion tickets follow) | exact2 | resolved framework issue X3 (if reproduced); preference: after all UI tickets | Converts every UI size; last to avoid churn | matrix at 16 px unchanged; 12/20 px pairs |
| 8 (last) | [20261005-interface-font-size-conversion](tasks/closed/20261005-interface-font-size-conversion.md) | Every surface scales with the interface font size (split per area at prepare) | exact2 | interface-font-size; resolved framework issue X3 (if reproduced); preference: after the area's last UI ticket | Needs the root size and the size map | matrix at 16 unchanged; 12/20 pairs |
| blocked (X2) | [20261005-app-developer-tools](tasks/20261005-app-developer-tools.md) | View › Toggle Developer Tools | exact2 | issue X2 resolved or decided | DEFERRED refuses a devtools UI; waits for X2 | blocked |
| blocked (X40) | [20261005-app-update-feed](tasks/20261005-app-update-feed.md) | The T3 desktop update feed and its UI | exact2 | issue X40 resolved or decided | Excluded scope; implemented or closed after the X40 decision | blocked |
| blocked (X1) | [20261005-browser-surface](tasks/20261005-browser-surface.md) | The reference Browser surface (tabs, navigation, Annotate, capture, PiP, device toolbar, profiles, automation) | exact2 | issue X1 resolved or decided | Needs framework support for Chromium and a DEFERRED waiver; written now so it can start when X1 is resolved | blocked |
| blocked (X38) | [20261005-t3-connect-sign-in](tasks/20261005-t3-connect-sign-in.md) | T3 Connect / Clerk sign-in, relay connections, `t3code://` handoff | exact2 | issue X38 resolved or decided; X5 | Excluded scope; implemented or closed after the X38 decision | blocked |
| blocked (X39) | [20261005-telemetry](tasks/20261005-telemetry.md) | Desktop and server telemetry | exact2 | issue X39 resolved or decided | Excluded scope; implemented or closed after the X39 decision | blocked |
| blocked (X41) | [20261005-wsl-environments](tasks/20261005-wsl-environments.md) | WSL environments (Windows only) | exact2 | issue X41 resolved or decided | Excluded scope; likely closed as not applicable on macOS after the X41 decision | blocked |

Task files own mutable status. Links must be updated when a task closes or reopens.

## Dependency review

- **Edges are acyclic.** The table is generated from each ticket's `merged task PR` rows;
  a group's number is its longest path from the start, so no ticket depends on its own or a
  later group (checked by script). Longest chain: round12-wrapup → clone-on-exact2-main →
  desktop-oracle-and-trace → fake-github-fixture → pr-conversation-and-refresh →
  pr-header-actions-and-stacks → pr-handoffs-and-quick-actions → pr-code-tab. Other chains:
  embedded-server-runtime → local-primary-environment → this-machine-network-access /
  app-activation / portable-app-download; terminal-surface + remote-scopes-and-update-commands
  → terminal-drawer → terminal-layout (also after right-panel-tab-menu) →
  terminal-integrations; provider-sign-in-and-install → managed-codex-chatgpt /
  provider-settings-upkeep / usage-reset-and-feedback → usage-pooled-view;
  remote-scopes-and-update-commands → server-update-banner → auto-balance;
  reference-logic-tests-done-areas → reference-logic-test-ports; interface-font-size →
  interface-font-size-conversion (last, blocked by X3 if reproduced).
- **Real-input rows (decision U14).** Rows marked `(attended session)` are run by the
  implementing or verifying agent with `orca computer` (accessibility clicks, right-click,
  drag, scroll, typing, key chords, screenshots) inside that ticket's own verification, so no
  separate human session gates the groups. Drivers take the real-input lock one at a time and
  target only their lane app copy. Limits to record when met: synthetic input may not exercise
  IME composition, trackpad pinch/rotate, or OS permission prompts.
- **Group 1.** hot-file-split moves code in `client.ts`, `app.contract` and the Swift
  registration points; main-fix-adoption and reference-logic-tests-done-areas wait for it.
  desktop-oracle-and-trace runs beside it because it changes only tools and docs (review
  round 2).
- **Actual merge prerequisites** are the rows' Dependencies plus the common three. Scheduling
  preferences only: main-fix-adoption before tickets that edit sidebar/popover/tooltip
  Contract; environment-routes before local-primary-environment and ssh-password-and-remote-open
  (same `environmentKey` call sites and `T3Ssh.swift`); interface-font-size and its
  conversion last.
- **Shared files.** After `hot-file-split`, features add code in area files. Remaining
  shared points: `app.contract` root resources/actions, `shapes.contract`, `app.json`,
  `T3Module.swift` op registration, `README.md`/`AGENT-HANDOFF.md`/`EXACT2-GAPS.md`. A PR that
  merges second rebases on `main` and re-runs its checks; the integrator
  serializes the docs updates.
- **Transport stream cap (16)** is touched by live-automations-and-clones,
  pr-conversation-and-refresh and terminal-drawer; terminal-drawer owns any cap change and
  uses a side stream for terminal output.
- **Framework issues** never block unrelated tickets. The blockers are listed in the
  completion rule under Integrated acceptance. Embedded-server-runtime depends on the outcome of X4/X6 checks.
- **Framework support arrives only through issues.** This plan writes no framework PR.
  `issue-open` publishes each issue (after the user approves); when its fix lands on `main`,
  the affected ticket reopens or unblocks, and
  `issue-close` verifies the adoption in the app.

## Integrated acceptance

| Workflow | Reset and fixture | Required platforms | Assertions | Evidence |
| --- | --- | --- | --- | --- |
| Fresh install from the download | The `.app` archive from `portable-app-download`; the clean environment chosen in U11 at the spec's minimum macOS, with no repo, Bun, Node or T3; one provider CLI installed through its standalone installer (no Node) | macOS 14 VM or second Mac (or the lowered minimum, U11), Apple Silicon | First launch unpacks and starts the embedded server; the server reports the pinned version and commit; onboarding; add a project; new thread; send; stream; approval; diff; terminal (the `terminal-drawer` "Embedded server" row); relaunch keeps state; `which node` still finds nothing | screenshots, server pid/port/version, `~/.t3` tree, logs |
| Desktop parity matrix | Lane fixture backend at the server pin; oracle at the same pin | macOS 1280×840 and 840×620, light and dark | Every surface pair is "match" or a declared exact2 deviation with an issue link | matrix in `AGENT-HANDOFF.md` |
| Protocol parity | Scenario set from `desktop-oracle-and-trace` | macOS | Trace diff clean or allow-listed with reasons; RPC tally: every method the reference desktop calls in a scenario is called by the clone, except excluded features | trace reports, RPC tally |
| Local server lifecycle | Embedded server running | macOS | Crash → restart with backoff; settings change → restart/reconnect per decision; ⌘Q → no orphan process | `ps`, `lsof`, logs |
| Remote environments | Two isolated lane servers; SSH host fixture | macOS | Pair, multiple routes, fallback, SSH (password prompt), relaunch; credentials only in Keychain | state, Keychain item presence by service/account |
| Pull requests (fake gh) | Fake gh state per scenario | macOS | Comment, review, edit, labels, reviewers, reactions, merge, stack, Code tab threads; failure and permission states | `calls.ndjson`, state diffs, oracle pairs |
| Providers | Fixture streams; attended real accounts | macOS | Sign-in, install, managed Codex, ACP sessions; failure/cancel paths | trace, attended notes |
| Terminal | Embedded server | macOS | Drawer, tabs, splits, scripts, links, Add to chat, IME, clipboard | AppKit, attended session |
| Failure states | Fault injection (server stop, slow reply, refused write) | macOS | Reference wording and recovery for lost connection, uncertain write, slow request, missing provider, server crash | agent transcripts |
| Attended real input | Final checklist (handoff "Manual verification checklist", updated) | macOS with real keyboard/trackpad | Every row passes or is recorded with evidence | session log, screenshots |
| Repository gates | `main` head | macOS | Five checks, `caps.mjs`, clone checks, all AppKit binaries | logs |
| Coverage | `research.md` ID tables; the reference test map from `reference-logic-tests-done-areas` | — | Every research ID marked missing or partial maps to a verified ticket and its evidence; the reference test map has no `later-ticket` rows left, and every `port` and `swift` row is `ported` with a passing test | coverage table in `AGENT-HANDOFF.md`, map |
| Issues closed | `issues/README.md` | — | Every issue is closed: resolved upstream, adopted in the app and verified by `issue-close` (workaround removed, its rows pass at parity), or closed by the user's decision with the reason recorded | issue records, `EXACT2-GAPS.md` |
| Interface font size | Settings at 12, 16 and 20 px | macOS 1280×840 and 840×620 | Every surface scales where the reference uses `rem` and keeps its `px` sizes; 16 px equals the pre-conversion matrix | shot pairs at each size |

Completion also needs: every ticket verified and merged into `main`, except the
six blocked tickets (browser-surface, app-developer-tools, t3-connect-sign-in, telemetry,
app-update-feed, wsl-environments), which are either done the same way after their issue is
resolved or decided, or closed by that decision; and every issue in `issues/` closed —
resolved upstream and adopted in the app (verified by `issue-close`), or closed by the
user's decision. Criteria blocked today: X3 (if reproduced), X7 (one `media-actions`
criterion), X4 (only if the archive workaround fails), X32–X34 (their rows, if the capability
is absent), X31 (if a connecting state is not accepted), X30 (the Inspect row, U18), and any
criterion whose ticket marks it blocked.

## Apparatus requiring approval

`CLAUDE.md`: agents add no apparatus without a human saying so. Approving the plan does not
approve these; approve them per item.

| Item | Ticket | Location |
| --- | --- | --- |
| Lane tool copies (decision #1; location per U23) | clone-on-exact2-main | `examples/t3-code/tools/` (U23) |
| `ref-build.sh`, `electron-oracle.mjs`, `trace-proxy.mjs`, `trace-diff.mjs` + `scenarios/`, RPC tally | desktop-oracle-and-trace | `examples/t3-code/tools/` (U23) |
| Network installs: pnpm (reference-pinned), `vp`, Electron (reference-pinned), playwright-core | desktop-oracle-and-trace | worktree `target/` |
| Fake gh write verbs, state model, stdin logging, error injection | fake-github-fixture | `examples/t3-code/tools/fakegh/` (U23) |
| Build-time fetch + verification of the official server runtime; its committed hash; an `app.json` command for staging | embedded-server-runtime | example directory |
| Fake server for the local-backend AppKit tests | embedded-server-runtime | `macos/tests/local-backend/` |
| Vendored Ghostty terminal files + `VENDOR.json` check | terminal-surface | `terminal-host/` in the example |
| Packaging script for the downloadable archive | portable-app-download | example directory |
| `audit-bundle.mjs`, `bundle-allowlist.json`, `sandbox-exec` profiles | portable-app-download | example directory |
| Provider-setup fixture (scripted `provider.auth.*`, `provider.install.*`, reset credit, feedback) | provider-sign-in-and-install, provider-settings-upkeep, usage-reset-and-feedback | `examples/t3-code/tools/` (U23) |
| Fake ACP agent for terminal sign-in | sign-in-terminals | `examples/t3-code/tools/` (U23) |
| Lifecycle-event injection through the trace proxy | local-primary-environment | `examples/t3-code/tools/` (U23) |
| Rendered-HTML fixture server | media-actions | `examples/t3-code/tools/` (U23) |
| SSH test double that checks the secret; SSH host fixture | ssh-password-and-remote-open, integrated acceptance | `examples/t3-code/tools/` (U23) |
| Tailscale stub endpoint provider | this-machine-network-access | `examples/t3-code/tools/` (U23) |
| Fault injection (server stop, slow reply, refused write) | integrated acceptance | `examples/t3-code/tools/` (U23) |
| Computed-style probe and `rem` conversion script | interface-font-size | `examples/t3-code/tools/` (U23) |
| Grammar generator moved into the example (input: pinned published packages with hashes) | shiki-residuals | example directory |
| Token-comparison harness `shiki-compare.mjs` and its `shiki@4.2.0` install | shiki-residuals | `examples/t3-code/tools/` (U23) |
| Device-proxy variant with no local platform | upstream-ui-sync | `examples/t3-code/tools/` (U23) |
| Stub T3 RPC server for version skew and `server.getHostResources` | server-update-banner, auto-balance | `examples/t3-code/tools/` (U23) |
| Optional: catalog-injecting proxy; terminal vendor `refresh.mjs`; reference test title-comparison script | composer-fidelity, terminal-surface, reference-logic-tests-done-areas | `examples/t3-code/tools/` (U23) |

## Decisions the tickets need

User decisions (product scope, approvals, environments). Plan-wide ones are also in spec
"Open decisions". Each ticket repeats its own in Dependencies as `recorded decision`.

| # | Decision | Ticket(s) | Options (recommended first) |
| --- | --- | --- | --- |
| U1 | Framework knowledge basis | all tickets | **Decided 2026-10-05:** implementation uses the exact2 repo itself (docs, LLPs and source of the checkout being built) as its framework reference; the bundled library stays the planning snapshot |
| U2 | Every item in "Apparatus requiring approval" | the tickets named there | approve per item |
| U3 | Embedded server artifact, pin, keep or drop `client/`, unpack location | embedded-server-runtime | **Decided 2026-10-05:** the official CLI archive (Node SEA) at the release that matches the reference pin, verified against `SHA256SUMS` and a hash committed in the example; unpacked under `<T3 home>/runtime/versions` (the product's own layout); `client/` kept or dropped per the ticket's measurement |
| U4 | After a Local environment, Network access or Tailscale change | local-primary-environment, this-machine-network-access | **Decided 2026-10-05: the same as T3 Code** — relaunch the whole app. If exact2 cannot relaunch an app (issue X45), the tickets ship a restart-in-place stopgap and keep the relaunch rows blocked until X45 is resolved and adopted |
| U5 | The window cannot wait for server readiness (issue X31) | local-primary-environment | accept a connecting state until X31 is resolved / block the row on X31 |
| U6 | A saved entry with the primary's environment id | local-primary-environment | remove it / keep a duplicate row |
| U7 | Client settings store | local-primary-environment | keep the clone's `t3-code.json` / read the original's `desktop-settings.json` |
| U8 | Hosted pairing link on `app.t3.codes` for HTTPS endpoints (decide together with issue X38, T3 Connect) | this-machine-network-access | keep (reference default) / drop |
| U9 | Tailscale verification | this-machine-network-access | a tailnet you provide / stub provider only |
| U10 | A `t3` command-line install action (E4 `t3code://` now belongs to `t3-connect-sign-in`, issue X38) | app-activation | decide the CLI action |
| U11 | Signing, clean-test environment, archive format | portable-app-download | **Decided 2026-10-05:** ad-hoc signature, a clean macOS 14 VM, a zip archive. The recipient opens the app once through Gatekeeper ("Open Anyway"); Developer ID signing (issue X37) is not needed for this plan |
| U12 | Sessions paired with 3 scopes before the scope fix | remote-scopes-and-update-commands | prompt a re-pair / leave them |
| U13 | A smoke test on the real `~/.t3` | local-primary-environment | only with your explicit go and your own backup |
| U14 | Real-input checks | most UI tickets | **Decided 2026-10-05:** agents run them with Orca's computer-use CLI (`orca computer …`), no person needed; one driver at a time (`target/t3-ui-parity/lanes/.realinput-lock`), only the lane app copy is targeted; a synthetic action counts only after its effect is read back |
| U15 | Languages beyond Shiki's 16 grammars and the long-text limit | shiki-residuals | you choose the list and the limit |
| U16 | Third-party marks: CC BY 4.0 notice for the Azure DevOps mark; terms for 20 editor brand icons | upstream-ui-sync, composer-fidelity | a licenses/notices row in Settings › Licenses / omit marks without clear terms |
| U17 | Ultrathink and Cursor Fast mode states the fixture cannot produce | composer-fidelity | unit tests + attended real account / catalog-injecting proxy |
| U18 | Theme editor "Inspect app colors" | settings-scoped-controls-and-theme-editor | keep the row blocked on issue X30 / waive the row |
| U19 | Paste over 65,536 characters into a terminal | terminal-drawer | match the reference (it fails) / chunk |
| U20 | Script `autoOpenPreview` (reference opens the in-app Browser) | terminal-integrations | system browser / skip |
| U21 | Fakes for other source-control CLIs (`glab`, `az`, `fj`/`tea`) | fake-github-fixture | capability-driven unit tests only / build fakes |
| U22 | Full "Sidebar (legacy)" (about 3,800 reference lines) | legacy-sidebar | build it in full (spec: every feature) / defer |
| U23 | Location of the verification tools | every ticket's acceptance | **Decided 2026-10-05:** commit them under `examples/t3-code/tools/` (same relative paths as the lane tools), so a fresh clone can run every acceptance command; no absolute user paths or credentials in the committed files |

Technical questions that `prepare` answers with a spike (no user decision unless the spike
fails): macOS mapping of `visible`/`focused` for activity reports; which layer owns the A4
file-link rule; diff parser basis; sticky headers in lists (`contract vocab --json
position`); idle input for live refresh; header fold on scroll; inline-link frame for hover
cards; a POST-JSON transport op for the PR diff; the native transcript selection for Cite;
terminal view-count and memory budget (NO-GO stops the terminal tickets); DOM-bound emulator
tests; `rem` in number props; cursor keywords for the player; the remote image policy for
ACP icons; a slow-clone fixture; whether the CLI archive ships `node-pty`; whether
Foundation `Process` can pass fd 3; Gatekeeper behavior for ad-hoc apps on macOS 15+.

## Plan review

Independent reviewer: a separate agent that wrote no part of the plan.

- **Round 1 (2026-10-05):** 17 findings (2 blocking, 7 major, 8 minor). Repair round 1:
  ticket splits, dev builds refuse the real `~/.t3`, docs restored in the import, issue
  drafts X31–X37, impossible rows fixed, apparatus list, names and IDs, focus/Escape rows.
- **Round 2 (re-review):** 9 of 17 resolved, 8 partly; 3 new major and 5 new minor findings.
  Repair round 2: follow-up tickets `reference-logic-test-ports` and
  `interface-font-size-conversion` added with integrated rows; the right-panel tab menu has its
  own ticket (`right-panel-tab-menu`, not behind the terminal verdict); the remote-scopes rows no
  longer need later tickets; stale names, apparatus rows, keychain restore, the oracle edge
  (now a preference) and the clone-on-main wording fixed; the SSH secure-field row takes its
  expected result from the oracle.
- **Round 3:** 3 major and 3 minor findings, all short scope edits (TN8 owned with a row by
  `right-panel-tab-menu`; `swift` rows added to `reference-logic-test-ports` and the Coverage
  row; native fixed metrics added to `interface-font-size-conversion`; a table edge, a stale
  name and the X30 Inspect impact fixed). Applied by the coordinator. Reviewer verdict: ready
  to present with its open decisions once these were corrected; they were not re-reviewed
  again.
- **Round 4 (new user decisions):** issues only, no framework PRs; all issues rewritten with
  detailed context (why it arose, why it must be resolved, requested support, reproduction,
  fix acceptance, app adoption); six blocked tickets with issues X1, X2, X38–X41. Review: 3
  major (lane servers could send telemetry; `blocks` lists missed citing tickets; the Browser
  ticket is a whole product) and minor findings. Fixed by the coordinator: every T3 server the
  plan starts sets `T3CODE_TELEMETRY_ENABLED=false` (with acceptance rows); `blocks` lists are
  generated from each ticket's issue rows; adoption owners for X10 and X18; planned splits
  for `browser-surface` and `t3-connect-sign-in`; unblock conditions per option for X1/X2; a
  conditional edge for the update feed on X39 part 3; new issue X45 (app relaunch, for U4);
  unchecked gaps marked "(unconfirmed)"; stale sentences fixed. Reviewer verdict: ready to
  present once the telemetry leak and the `blocks` lists were fixed; the fixes were not
  re-reviewed.
- **Open by design:** finding 1 (tool location) needs the user's decision U23.

## Publication and next action

These records are committed with the base PR `feat(example)/t3-code` → `main`
(`examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/`). Next stage:
`clone-on-exact2-main` makes the moved clone build and run on `main`.

Integrated native bundle build passed (app Rust bake and full Swift module). The
isolated macOS driver launched the app and read its disconnected tree. The bounded
interaction check did not pass: the first requested welcome target was absent; the
actual Open Connections target was outside the default viewport, then reported hidden
or inert at 1280×900. Stopped after three attempts without UI adjustment. App interaction
acceptance remains unverified. Local evidence: `/tmp/t3-parallel-final-native.log`,
`/tmp/t3-parallel-final-native-tests.log`, `/tmp/t3-parallel-final-bun.log`, and
`/tmp/t3-parallel-final-smoke.log`. No push or PR publication performed.

## Exact skill verification, 2026-10-06

Verification of implementation `8498fdc8a` found functional failures; records stay active.
Timeline: `failed` (actual host Tab skips output). Tab menu: `failed` (native action
remains pending beyond20s; correct relative clipboard value arrives later). Activity:
`blocked` (component/cadence and accepted live activity wire checks pass, remaining
integrated server effects/retry/lifecycle incomplete). The new reference runtime is
provisioned and getTurnItem works; the older-runtime limitation no longer applies to
this attempt. Main migration and oracle delivery tasks remain separate work.

Evidence: [live attempt](evidence/parallel/20261006-live-verification/attempt.md),
[independent review](reviews/20261006-parallel-verification.md). No code repairs,
framework edits, remote issue writes, task closure or publication occurred.
