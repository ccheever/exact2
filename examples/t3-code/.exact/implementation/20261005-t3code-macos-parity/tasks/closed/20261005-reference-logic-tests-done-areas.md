---
name: 20261005-reference-logic-tests-done-areas
plan: 20261005-t3code-macos-parity
implementation: dropped
verification: blocked
delivery: closed-unmerged
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: feat(example)/t3-code-reference-logic-tests-done-areas
pr_url: https://github.com/ccheever/exact2/pull/151
verified_commit: null
---

# Every T3 Code test file is mapped to the clone

## Outcome

A reviewed map, `examples/t3-code/REFERENCE-TESTS.md`, lists every test file of T3 Code `1e2ecbd975` in the four packages the clone draws logic from. Each row has exactly one class and names the clone module and test it belongs to. The finished areas (rounds 1–12) get their `done-equivalent` rows proven by a title comparison. Every other row says what is left: a port, a Swift test, a reason it does not apply, or the ticket that owns it.

This ticket ports no test and fixes no divergence. The map is the input for the port tickets (see Next action).

## Scope and exclusions

Clone paths are `examples/t3-code/<file>`. Line numbers come from the mc-orch tree, 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Reference paths are repo-relative at T3 Code `1e2ecbd975`.

**Test files at the pin** (`find <dir> -name '*.test.ts' -o -name '*.test.tsx'`; files, not cases; about 6,800 `it(`/`test(` lines in the four packages by grep estimate):

| Package | Files | Notes |
| --- | --- | --- |
| `apps/web/src` | 431 | 62 in `components/chat`, 50 in `components/settings`, 43 in `lib`, 21 in `components/pullRequest`, 16 in `components/files`, 23 in `components/preview` and 17 in `browser` (Browser surface, excluded), 5 in `terminal/ghostty`, 5 in `components/clerk` and 8 under `cloud` (excluded) |
| `packages/client-runtime/src` | 121 | 50 `state`, 19 `device`, 9 `connection`, 5 `work-log`, 4 `relay` (excluded), 4 `errors`, 4 `operations` |
| `packages/shared/src` | 76 | composer, usage, git, preview, relay and CLI helpers |
| `apps/desktop/src` | 106 | Electron main: `app`, `backend`, `window`, `snapShot`, `preview`, `wsl`, `updates`, `ssh`, `settings`, `ipc`, `telemetry` |

Total 734 files. `packages/contracts` (36 files) and `apps/server` (468) are `n/a-server` as a group; list them as two rows.

**Classes.** Every file gets exactly one:

- `done-equivalent`: the clone already runs the same cases. The row names the clone test and the title comparison shows every reference title present.
- `port`: pure logic that a port ticket will move into a `bun:test` file with the reference titles. The row records the conversion it needs (below).
- `swift`: the behavior lives in an app Swift module. The row names the AppKit binary that gets the cases, or `n/a-electron` with the reason.
- `n/a-ui`: DOM or React rendering with no pure logic. The feature tickets prove it by agent drives and oracle pixel pairs.
- `n/a-server`: server, contracts, relay, CLI, database. The embedded server is the original's own.
- `n/a-excluded`: Browser (`browser/`, `preview/`), Clerk, T3 Connect (`cloud/`, `relay`), telemetry, the update feed, WSL, Linux and Windows files.
- `later-ticket: <full ticket name>`: an area another ticket builds. Owners are the existing tickets: the pull request tickets (`20261005-fake-github-fixture`, `20261005-pr-conversation-and-refresh`, `20261005-pr-header-actions-and-stacks`, `20261005-pr-handoffs-and-quick-actions`, `20261005-pr-writing-and-metadata`, `20261005-pr-code-tab`, `20261005-pr-links-previews-and-routing`, `20261005-diff-review-engine`); the provider and settings tickets (`20261005-provider-sign-in-and-install`, `20261005-managed-codex-chatgpt`, `20261005-provider-settings-upkeep`, `20261005-usage-reset-and-feedback`, `20261005-usage-pooled-view`, `20261005-live-automations-and-clones`, `20261005-settings-scoped-controls-and-theme-editor`); the terminal tickets (`20261005-terminal-surface`, `20261005-terminal-drawer`, `20261005-terminal-layout`, `20261005-terminal-integrations`, `20261005-sign-in-terminals`); the connection and desktop tickets (`20261005-environment-routes`, `20261005-remote-scopes-and-update-commands`, `20261005-server-update-banner`, `20261005-auto-balance`, `20261005-embedded-server-runtime`, `20261005-local-primary-environment`, `20261005-this-machine-network-access`, `20261005-app-activation`, `20261005-desktop-shell-details`, `20261005-portable-app-download`); and `20261005-composer-fidelity`, `20261005-thread-commands-and-keys`, `20261005-media-actions`, `20261005-legacy-sidebar`, `20261005-floating-device-player`, `20261005-interface-font-size`, `20261005-client-activity-reporting`, `20261005-upstream-timeline-and-markdown`, `20261005-upstream-ui-sync`, `20261005-shiki-residuals`. If a file's work belongs to no ticket, say so in the Notes column and tell the user: it is a plan gap.

Where to start (examples; the map is the deliverable, not this list):

| Area | Reference tests (examples) | Likely clone counterpart |
| --- | --- | --- |
| Sidebar | `components/Sidebar.logic\|drag\|motion\|pointer\|snooze.test.ts`, `threadSidebarWidth`, `ThreadStatusIndicators.test.ts`, `lib/threadSort`, `environmentGrouping`; `state/threadSort\|threadSnoozed\|customSnooze\|threadSearch\|projectGrouping\|threadShell` | `sidebar-model.ts`, `sidebar-view.ts`, `sidebar-state.ts`; tests `sidebar*.test.ts`, `r3-sidebar.test.ts`, `r12-sidebar.test.ts` |
| Conversation | `chat/MessagesTimeline.logic.test.ts`, `session-logic.test.ts`, `work-log/*` (5), `markdown-*`, `proposedPlan`, `chat/timelineMinimapItems\|timelineScrollTarget`; shared `orchestrationV2Timeline`, `toolActivity`, `toolOutput`, `assistantCitations`; `userMessage`, `markdownImages`, `codexFileCitations` | `timeline-*.ts`, `chat.ts`; tests `timeline*.test.ts` |
| Composer | `composer-logic`, `composer-list-continuation`, shared `composerTrigger\|composerInlineTokens\|composerContext*`, `chat/composerSubmission\|queuedMessageEdit\|composerPromptHistory\|modelPickerSearch\|modelPickerKeys\|TraitsPicker`, `modelSelection`, `modelOrdering`, `ContextWindowMeter.logic` | `composer-*.ts`, `r3-composer-controls-*.ts`; tests `composer*.test.ts` |
| Version control | `BranchToolbar.logic`, `GitActionsControl.logic`, `lib/baseRefChoices`, `state/vcs\|vcsAction\|sourceControl`; shared `git`, `gitPatchPath` | `r4-git-*.ts`; `r4-git.test.ts` |
| Right panel, Files, Device | `rightPanelStore`, `diffPanelStore`, `components/files/*` (16), `lib/turnDiffTree\|diffCollapse\|diffFileContents`, shared `filePreview`, client-runtime `device/*` (19) | `r4-surfaces-*.ts`, `diff.ts`, `r6-media-device.ts`, `r7-device-tools.ts`, `r9-device-*.ts`, Swift `R7Device*` |
| Pages | `components/usage/*` (9), shared `usageFormat\|usageLimits\|usageMerge`, `state/usage\|serverUsage`, `onboarding/*`, `CommandPalette.logic`, `timestampFormat` | `pages-*.ts`, `palette*.ts` |
| Settings | `components/settings/*` (50), `appearanceFonts`, `themePalette`, `vscodeThemeImport`, `keybindings`, `KeybindingsSettings.logic`, shared `serverSettings\|projectSettings` | `settings-*.ts`, `keybinding-*.ts` |
| Connection | client-runtime `connection/*` (9), `rpc/*`, `errors/*`, `platform/storageDocument`; web `connection/`, `hostedPairing`, `versionSkew` | `connections.ts`, `r3-protocol-*.ts`, `r8-pointer-reconnect.ts`; Swift `T3Transport`, `T3Fleet`, `T3Credentials` with `macos/tests/transport`, `fleet` |
| Desktop shell | `window/QuitHold\|DesktopApplicationMenu\|DesktopWindow`, `electron/ElectronMenu\|ElectronWindow`, `ipc/window\|notificationBadge\|sshEnvironment`, `settings/DesktopSavedEnvironments`, `snapShot/MacSnapShot\|MacModifierPairShortcutProcess\|DesktopSnapShot\|captureConfigEdit` | Swift `T3Menus`, `R8KeysMenus`, `T3Notifications`, `T3SnapShot*`, `T3Ssh`; `macos/tests/menus`, `snapshot`, `notifications`, `ssh` |

Excluded from this ticket: porting any test; fixing any divergence; new features; changing a module that another ticket changes (`itemDetail`, `markdownLinks`, `threadInbox` working sort, `providerSkills` display names belong to `20261005-upstream-timeline-and-markdown` and `20261005-upstream-ui-sync`).

## Context and guidance

Parent specification: [spec](../../spec.md) (goal and logic reuse row). Research: [research](../../research.md). Library revision: `20261005-platforms-v3`. Selected topics: testing-and-debugging (record what each test proves; static evidence is not runtime evidence; keep the source identity), state-and-data (time and persistence enter as arguments), foundations (build commands). The library does not cover app-local Swift modules; the clone's AppKit recipe in `README.md` is the basis for `swift` rows.

Consumer framework revision and toolchain: the pin chosen by `20261005-clone-on-exact2-main`; pinned Bun 1.4.2.

Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23). This ticket adds one verification script, `target/t3-ui-parity/test-map.mjs` (apparatus, decision U2): it lists the reference test files with `git -C <reference> ls-files`, extracts `describe`/`it`/`test` titles, and compares them with a named clone test. It reads the read-only reference checkout. The app build never does.

Conversions the port tickets will need; record the ones that apply in each `port` row's Notes:

- `vite-plus/test` → `bun:test` (same `describe`, `it`, `expect`).
- Fake timers (`vi.useFakeTimers`, in about 110 files) → a `now` argument.
- `vi.mock` → the clone's fake native harness (`composer-controls-fixture.ts`).
- `@effect/vitest` (about 150 files), branded ids (`ThreadId.make("x")`), `effect/DateTime` → plain strings and ISO text. A module that is Effect-based end to end has no pure part in the clone: class `n/a-ui` or `swift`, with the reason. Whether Effect runs in the data module is unknown; do not assume it.
- Locale or `Intl` use (`timestampFormat`, `dateTime`, `usageFormat`): mark the row. See X36.
- The clone re-implemented some reference modules under other names (for example `sortByReturn` for `sortInboxThreadsByReturn`). Note the reference name so the port ticket adds an export with it.

File headers in the clone name their source (for example "T3 Code, MIT, see LICENSE-T3: Sidebar.tsx, Sidebar.logic.ts, …" in `sidebar-model.ts`). Use them to find the clone module of a reference test.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged first: group 1 changes shared files that this ticket also edits (review finding: sequence it first) | pending |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| recorded decision | U2: apparatus: `target/t3-ui-parity/test-map.mjs` | none | User approves (the default is a one-off command recorded in Attempts) | pending |

## Issue assessment at preparation

Checked sources and time: local issue drafts in [issues](../../issues/README.md), `EXACT2-GAPS.md`; no upstream search (planning). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X36](../../issues/closed/20261005-x36-data-runtime-intl-locale.md) | Locale-aware `Intl` in the data runtime | Date, time and number tests use `Intl`; fixed by main #204 (#118: `Intl.Locale`, `getWeekInfo()` as Chrome), adopted in [adopt-main-fixes-r4](20261007-adopt-main-fixes-r4.md) | none | Port the rows; the `ja-JP` long-date space and a `Locale` object passed to a formatter are main's documented differences |
| [X19](../../issues/closed/20261005-x19-data-source-timers.md) | Timers in data sources | Ported tests use a `now` argument | nonblocking | None |
| none found | Mapping and classification | — | none | — |

## Implementation notes

- First commit: the file list alone, one row per file with its package and cases count (`describe`, `it`, `test` lines). Then add classes and clone modules area by area.
- For `done-equivalent`: run `test-map.mjs` with the reference file and the clone test. All reference titles must appear. If some do not, the row is `port` with the missing titles in Notes.
- A file that has some cases covered and some not is `port`, not `done-equivalent`.
- Keep the map under 1,500 lines (one row per file, plus the two group rows and a header with counts per class).
- Do not edit code. The script and the map are the only additions.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Map is complete | Reference checkout at `1e2ecbd975` (verification only) | List test files with `git -C <reference> ls-files '*.test.ts' '*.test.tsx'` for the four packages; compare with the map rows | Zero unmapped files; zero files with two classes; 734 file rows plus the two group rows; counts per class recorded | macOS | command output, `REFERENCE-TESTS.md` |
| `done-equivalent` rows are proven | Same | `test-map.mjs` for each row | Every reference title appears in the named clone test; clone-only titles are listed | macOS | script output attached per row or as one log |
| Other classes carry a reason | — | Read the map | Every `port` and `swift` row has a conversion note; every `n/a-*` row has a reason; every `later-ticket` row names an existing ticket file in `tasks/` | macOS | a script check of the ticket names |
| Plan gaps reported | — | List rows with no owner | Each is reported to the user as a plan gap | — | list in Attempts |
| Independent spot check | — | A second agent classifies 30 random files without seeing the map | Disagreements are resolved and recorded | — | note in Attempts |
| Repository gates | `git add -A` | Clone checks (unchanged code: bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, AppKit binaries), `bun scripts/caps.mjs`, the five checks | Green; no matrix run needed because no code changed | macOS | logs |

Integrated acceptance (plan): at the end of the plan, `REFERENCE-TESTS.md` has no `later-ticket` row. Each such row was moved by its owner ticket or by a port ticket, or carries a waiver the user signed. This ticket only provides the list; the integrated acceptance runs the check.

Task-owned source paths: `REFERENCE-TESTS.md`, `target/t3-ui-parity/test-map.mjs`, a pointer line in `AGENT-HANDOFF.md`.
Required environment: the read-only reference checkout for the map and title checks; pinned Bun 1.4.2. No fixture backend and no Xcode run beyond the unchanged-code checks.

## Progress

2026-10-08 (records sync): moved to `tasks/closed/` (`implementation: dropped`): dropped by the user's decision of 2026-10-06 (#151, the map, closed as unnecessary).

2026-10-06: dropped (user decision). The user closed #151, the map, as unnecessary.

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

None unless the user reverses the 2026-10-06 decision.
