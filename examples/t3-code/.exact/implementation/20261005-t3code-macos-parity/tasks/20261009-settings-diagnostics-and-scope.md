---
name: 20261009-settings-diagnostics-and-scope
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-diagnostics-and-scope
pr_url: https://github.com/ccheever/exact2/pull/357
verified_commit: null
---

# Settings: the Diagnostics page fills again, and the Project page opens on every environment

## Outcome

- Settings › General › View diagnostics shows its sections again (Resource monitor through Top Span Names). Today it stays
  empty, because its request is replaced before it can answer. The 2026-10-07 audit saw this page filled (regression).
- ~~Diagnostics keeps the scope sentence "Applying settings for All projects across All environments".~~ Superseded
  (2026-10-09): the reference's only way into Diagnostics passes the General scope's environment
  (`SettingsPanels.tsx:3357`, `search={{ machine }}`), so it reads "All projects on Daehyeon's MacBook Pro", as the clone
  does. No change; see "Cause and fix", S2-11. (The title said "Diagnostics and the Project page keep the
  all-environments scope" until then.)
- The Project page opens scoped to all environments.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| S2-2 (high) | Diagnostics shows Resource monitor, Host & collection, Resource timeline, Live process tree, Instrumented application I/O, Live Processes, Resource History, Trace Diagnostics (Spans 1,787, Failures 207), Latest Failures, Most Common Failures, Slowest Spans, Span Logs, Top Span Names. | Only the scope sentence and an empty `#diagnostics-settings`, after 4 s and after 12 s. Logs repeat "changed (1 asked again)", then "forget request N (diagnostics)" / "request N+16 (diagnostics): POST exact-native:"; the request never answers. Seen in agent mode only; a normal launch was not checked. | Settings › General › View diagnostics; wait 12 s. | `target/t3-audit/evidence/settings-2/S2-2-ref.png`, `S2-2-clone.png`, `settings-2/clone/drive8.jsonl`, `settings-2/ref/diagnostics.aria` |
| S2-11 | The sentence stays "Applying settings for All projects across All environments"; the page inspects the scope's representative environment. | The sentence becomes "Applying settings for All projects on Daehyeon's MacBook Pro" (`settings-core.ts:433`, the `envScope` diagnostics row). | Settings › General › View diagnostics; read the sentence. | `target/t3-audit/evidence/settings-2/S2-11-ref.png`, `S2-11-clone.png` |
| PG-8 | `/projects/$projectKey` redirects to `/settings/projects?project=<key>` with no machine: "Applying settings for work across All environments". | "Applying settings for work on Daehyeon's MacBook Pro": `settingsEnvironmentId` is set to the current environment (`app.contract:336-349`), so edits target one environment. | On a thread of project work: ⌘K › "Project settings" › Enter. Read the scope line. | `target/t3-audit/evidence/pages/PG-8-ref.png`, `PG-8-clone.png` |

## Cause of S2-2 (from the auditor's reading)

`app.contract:444`: `resource diagnostics = diagnosticsSettings(…, wallTime.epochAtZero + elapsed, settingsRefresh,
data.revision)`. A new argument replaces the request in flight (LLP 1016 D5, by design), so a slow native read never
lands. The fix is in the clone's arguments: ask again only when the reference asks again (read
`DiagnosticsSettings.tsx` for its refresh controls and any periodic reads; data modules have no timers, #124, so a
periodic read is a gated task).

## Scope and exclusions

Included: the three findings above.

Excluded:
- How the other Settings pages read the server config:
  [settings-pages-subscribed-config](20261009-settings-pages-subscribed-config.md) (planned; it also touches Diagnostics'
  answer and says to coordinate with this task). If both change Diagnostics' answer, the second to merge merges the base
  and keeps both.
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`): `components/settings/DiagnosticsSettings.tsx:708-906`;
`routes/settings.tsx` (`singleEnvironment` only for `/settings/providers`); `routes/projects.$projectKey.tsx:11-15`;
`components/CommandPalette.tsx:2286-2290`.

Clone (`examples/t3-code`): `app.contract:444` (the `diagnostics` resource), `app.contract:336-349` (the Project page's
scope), `settings-core.ts:433`. Related, merged: [fix-settings-integrations-loop](closed/20261009-fix-settings-integrations-loop.md)
(#353) fixed the same pattern for Settings › Integrations (a resource keyed on `data.revision`).

Shared files: `app.contract` with [shell-sidebar-palette-keys](20261009-shell-sidebar-palette-keys.md) (SH-1 may touch
`app.contract:992-1015`); `settings-core.ts` with [settings-rows-and-labels](20261009-settings-rows-and-labels.md) (line 264).

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| S2-2 | Diagnostics fills within a few seconds and stays filled; the logs show no repeated "forget request … (diagnostics)". A Bun test checks that the resource's arguments do not change on every answer. Check one normal launch too. | `s2-2-diagnostics.png` | agent, plus one normal launch |
| S2-11 | ~~Diagnostics' sentence stays "All projects across All environments".~~ Superseded: the sentence matches the reference's after "View diagnostics" ("All projects on Daehyeon's MacBook Pro"; `SettingsPanels.tsx:3357` passes `machine`). | `s2-11-scope.png` | agent |
| PG-8 | "Project settings" from the palette, the sidebar and the thread menu opens with "work across All environments". | `pg8-project-scope.png` | agent |

## Cause and fix

**S2-2 (Diagnostics stays empty).** The `diagnostics` resource was asked on `data.revision`. The client's event drain
bumps that revision on every batch, and the page opens the `subscribeResourceTelemetry` stream, which sends a sample
every second. So every sample asked every `data.revision` reader again (about 35 resources), and Exact replaced the
Diagnostics answer in flight (LLP 1016 D5). The next answer sent its server reads again (`request`, not shared), so no
reply ever reached a live answer. Live, on the base: 35 "forget request … (diagnostics)" lines in the last 4,096 log lines
and no fulfil; the agent's 4-s and 8-s waits each took about 40 s (the driver's settle cap with the request always in
flight). Not a framework limit.

The reference re-renders only `ResourceTelemetryDiagnostics` on a sample, and reads Live Processes, Resource History and
Trace Diagnostics once per visit and on Refresh (`DiagnosticsSettings.tsx:708-760`; `client-runtime/src/state/server.ts`:
those queries have no `refreshIntervalMs`; `resourceTelemetryHistory` has `staleTimeMs: 5_000`). Fix:
- `client.ts` drain: a batch of resource telemetry samples alone no longer bumps `revision`. It bumps `telemetryRevision`,
  which the snapshot exposes as `data.telemetry` (`presentation.ts`, `shapes.contract`). Mixed batches bump both.
- `app.contract`: `diagnostics` also takes `data.telemetry`, so a sample asks only Diagnostics again.
- `diagnostics-view.ts` and `settings-a-telemetry.ts` (`history`): the four reads are shared reads (`restAccess().read`,
  T3Transport `share`) kept one by one. An answer asked again before a reply joins the read still pending and keeps what
  the earlier answer never received. No timer and no throttle.

**S2-11 (the scope sentence on Diagnostics).** No change. The live reference, with Settings › General › "View
diagnostics" clicked, opens `#/settings/diagnostics?machine=<representative environment>` and reads "Applying settings
for All projects on Daehyeon's MacBook Pro", as the clone does. The link is `SettingsPanels.tsx:3357`
(`search={{ machine: environmentId ?? undefined }}`, `environmentId` = the General scope's representative,
`SettingsPanels.tsx:2181`). The audit's image shows the route opened without that link (no `machine`). It is the only
way into Diagnostics in the reference. Decided by "match the original"; no decision needed.

**PG-8 (the Project page's scope).** Every Project settings entry of the clone (palette, sidebar, thread menu, details
card, the projects popover) names the project by its id (`settingsProjectId`). The Settings scope read a bare project id
as that project's checkout ("work on Daehyeon's MacBook Pro"). In the reference every entry navigates to
`/projects/$projectKey`, which redirects to `/settings/projects` with `{ project: key, machine: undefined }`
(`routes/projects.$projectKey.tsx:11-15`). Fix: a bare project id is its project group with no machine and no
checkout, in `settingsScopeOf` (`settings-core-view.ts`: the scope sentence, the General/Project rows, Providers),
`app.ts` (`projectsView`), `source-control-view.ts` (Integrations' device scope) and `scheduled-view.ts` (`taskScope`;
a project in no group keeps the old checkout reading). The result is the scope the scope menu gives when the project is
chosen there. Observed side effect: the Project page's Model row now shows the project scope's traits ("Medium · 1M"),
as choosing "work" in the scope menu already did; before it showed the checkout's ("Medium").

What this changes for a project with more than one checkout (two checkouts of one repository), opened
from the palette, the sidebar, the thread menu or the details card:
- The Project page lists every checkout, and its Danger row reads "Remove this project everywhere" / "Remove all
  entries"; its confirmation removes every checkout entry of the project (`removeTarget` empty, so the dialog confirms
  `remove-group`: a `project.delete` per member, `client-ops-sidebar.ts` `manageGroup`). Before, it was one checkout,
  "Remove checkout", and removed that one entry. This is the reference's project scope
  (`ProjectSettingsPanel.tsx:497-523`).
  "Remove checkout" stays on each checkout's own row and when a checkout is chosen in the scope menu.
- Scheduled Tasks opened from that scope lists the tasks of every checkout, and Integrations' device switches read and
  write Agent device access for every checkout (one `server.updateSettings` with each checkout's override). Before, both
  covered the one checkout.
A project with one checkout reads and writes the same entry as before.

## Acceptance results

| Id | Result | Evidence |
| --- | --- | --- |
| S2-2 | Pass (agent mode). Diagnostics shows all 13 sections, from Resource monitor to Top Span Names, at 4 s and at 12 s (the `diagnostics-settings` tree has 1,507 nodes; before: 1 node, empty). The 4-s and 8-s waits took 4.0 s and 8.0 s (before: 40.2 s and 40.6 s). Three "forget request … (diagnostics)" lines while the stream's first sample arrived, all in the first burst; then none: each 1-s sample asked `data` and Diagnostics only (12 and 11 asks in 12 s), with no native request from Diagnostics. Session log: 3,295 lines (before: 46,000). Bun: `settings-diagnostics-scope.test.ts`, two S2-2 tests, fail on the base sources and pass on the fix | [s2-2 pair](https://raw.githubusercontent.com/ccheever/exact2/ee2ee1042fd2882339d241bb59a624dc1563b803/settings-diagnostics-and-scope/s2-2-diagnostics.png), [live record](https://raw.githubusercontent.com/ccheever/exact2/792ab62d792b01fea88108865a17289dc01236b3/settings-diagnostics-and-scope/live-drive.txt), [test base vs fix](https://raw.githubusercontent.com/ccheever/exact2/a2e70ec46f101411fa3644a106cc79d094aeaa5e/settings-diagnostics-and-scope/test-base-vs-fix.txt), [drive steps](https://raw.githubusercontent.com/ccheever/exact2/1375b94cce24af8ffa3bf893b8d2605b819078fb/settings-diagnostics-and-scope/drive.sh.txt) |
| S2-2, one normal launch | Not verified: the Mac's screen is locked, and a normal launch cannot be read or driven without real input | Real-input batch step 2 |
| S2-11 | Pass, no change: before, after and the live reference all read "All projects on Daehyeon's MacBook Pro" after "View diagnostics"; the reference URL carries `machine` | [s2-11 triple](https://raw.githubusercontent.com/ccheever/exact2/79c492e18dd66fb029b41df03f2947a6d855f964/settings-diagnostics-and-scope/s2-11-scope.png), [reference record](https://raw.githubusercontent.com/ccheever/exact2/e344564cd2262f1370665898d2ea85c8d0a18b6f/settings-diagnostics-and-scope/reference.txt) |
| PG-8, palette | Pass: "work across All environments" (before: "work on Daehyeon's MacBook Pro"); the reference reads the same | [pg8 pair](https://raw.githubusercontent.com/ccheever/exact2/6e290627d84a013a54b8e9114663fba21f591b08/settings-diagnostics-and-scope/pg8-project-scope.png) (row 1), [live record](https://raw.githubusercontent.com/ccheever/exact2/792ab62d792b01fea88108865a17289dc01236b3/settings-diagnostics-and-scope/live-drive.txt) |
| PG-8, thread menu | Pass: "Verification fixture across All environments" (before: "… on Daehyeon's MacBook Pro") | [pg8 pair](https://raw.githubusercontent.com/ccheever/exact2/6e290627d84a013a54b8e9114663fba21f591b08/settings-diagnostics-and-scope/pg8-project-scope.png) (row 2) |
| PG-8, sidebar | Code path passes: the sidebar sets the same bare project id, and the Bun test checks the scope a bare id gives (fails on the base, passes on the fix). Live: not verified, the row's menu is a native NSMenu | Real-input batch step 1 |
| PG-8, every hunk tested (review 2026-10-10) | Pass: a two-checkout project from a bare id: the Project page lists both, its Danger row is "Remove all entries" with no single target, and confirming it deletes both entries; Scheduled Tasks lists both checkouts' tasks; Integrations' device scope is the project and Agent device access writes both overrides. A checkout chosen in the scope menu stays one in each. With each of `app.ts`, `scheduled-view.ts`, `source-control-view.ts` and `settings-core-view.ts` put back to the base alone, a test fails | [hunks base vs fix](https://raw.githubusercontent.com/ccheever/exact2/63430af2a84037ff6376b32f9477c52f05ad14a5/settings-diagnostics-and-scope/pg8-hunks-base-vs-fix.txt) |

## Tests

- New `settings-diagnostics-scope.test.ts` (6 tests, through the app's own `answer()`; the S2-2 ones with a small runner
  of `data` and `diagnostics`, where a replaced answer's native calls reject as Exact's do):
  - a telemetry sample asks only Diagnostics again: `data.revision` stays, `data.telemetry` moves, no read is sent;
  - a read the next answer joins lands although every answer is replaced before its reply (a sample every turn, replies
    two turns late), with one server request per read;
  - a bare project id gives the project scope across all environments, the same as the scope menu's project.
  These three fail on the base sources and pass on the fix ([output](https://raw.githubusercontent.com/ccheever/exact2/a2e70ec46f101411fa3644a106cc79d094aeaa5e/settings-diagnostics-and-scope/test-base-vs-fix.txt)).
  Added after the review of 2026-10-10, each on a project with two checkouts (its own environment, `env-work`, so the
  app client loads its shell and the next file's `env1` loads its own again):
  - `projectsView` from a bare id: both members, "Remove this project everywhere" / "Remove all entries", no single
    `removeTarget`, the confirmation for 3 threads in 2 entries, and confirming sends `project.delete` for both; the scope
    menu's project gives the same; its checkout gives "Remove checkout" with target `p1`. It runs last in the file;
  - `scheduledSettings` (`taskScope`) from a bare id lists both checkouts' tasks; a checkout lists its own;
  - `integrationsPage` from a bare id gives the device scope `|<key>|`, and Agent device access pressed with it writes
    both checkouts' overrides in one `server.updateSettings`.
  Each fails with its hunk put back to the base alone ([output](https://raw.githubusercontent.com/ccheever/exact2/63430af2a84037ff6376b32f9477c52f05ad14a5/settings-diagnostics-and-scope/pg8-hunks-base-vs-fix.txt)).
  Its servers use generations 50 to 55: the app tests share the app's client in file order, and the client adopts only
  a newer generation (`client.ts` `adoptStatus`), so they sit between `providers-scope.test.ts` (41, before) and
  `settings-integrations-reads.test.ts` (60, after).
- `settings-core.test.ts`: the bare-project-id expectation is now `project` (was `checkout`), with the reference route.
- `settings-a-telemetry.test.ts`: its fake client has `read` as well as `request`.

Checks: see the PR ("Checks").

## Real-input batch steps

Lane `settings-diagnostics-and-scope` (`target/t3-audit/lanes/settings-diagnostics-and-scope`, embedded server on 16802),
the bundle built from this branch's worktree `/Users/daehyeonmun/orca/workspaces/exact2/t3-code-settings-diagnostics-and-scope`.
Launch it normally (not agent mode) from that worktree:

```sh
A=/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit L=$A/lanes/settings-diagnostics-and-scope
T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16802 T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 \
  CODEX_HOME=$L/codex CLAUDE_CONFIG_DIR=$L/claude T3CODE_TELEMETRY_ENABLED=false \
  EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos --bundle --run
```

1. **PG-8, sidebar.** Right-click the "Timeline verification" thread row in the sidebar and choose "Project settings"
   in the native menu. Read the scope sentence: "Applying settings for Verification fixture across All environments".
2. **S2-2, normal launch.** Click the Settings gear, then General › "View diagnostics". Wait 12 s. The page shows Resource
   monitor, Host & collection, Resource timeline, Live process tree, Instrumented application I/O, Live Processes,
   Resource History and Trace Diagnostics with its tables; the Resource monitor's numbers change about once a second;
   the window answers clicks at once (for example General in the Settings sidebar).

## Progress

2026-10-09: reproduced S2-2 live on the base (evidence-base) and in a Bun model of the two resources; checked S2-11 and
PG-8 on the live reference (clicked links, read the URL); fixed, tested, built the bundle, and drove the branch once in
agent mode with the same steps as the base. Draft PR [#357](https://github.com/ccheever/exact2/pull/357).

2026-10-10: the first final check run had two failures in the full Bun suite, both in
`settings-integrations-reads.test.ts` (each file passed alone). This file's servers used generations 72 to 74, above that
later file's 60 and 61, so the shared app client ignored the later servers and never opened their device stream. The
test now uses 50 to 52, and the full suite passes. The final checks ran once on the head (results in the PR).

2026-10-10, review round: the review found three of the five PG-8 hunks untested (`app.ts`, `scheduled-view.ts`,
`source-control-view.ts`; the old `projectsView` check used a one-checkout project, which passes either way), the PR body
silent on the Danger row's wider scope, and this record still promising S2-11's "across All environments". Added three
tests on a two-checkout project (each fails with its hunk at the base), wrote the scope change into "Cause and fix"
and the PR body, and marked S2-11's planned outcome superseded. No app code changed, so no new screenshots and no live
drive.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive 1-2 | evidence-base `950e8e2e5` | Stopped at a target name (a quoted label; then "Back" names two views); the S2-2 part had already reproduced the bug | — |
| Before drive 3 | evidence-base `950e8e2e5` | Complete; its log file was overwritten by another lane in the shared scratch folder | — |
| Before drive 4 | evidence-base `950e8e2e5` | Complete, the record's numbers | [live record](https://raw.githubusercontent.com/ccheever/exact2/792ab62d792b01fea88108865a17289dc01236b3/settings-diagnostics-and-scope/live-drive.txt) |
| After drive (the one live drive) | this branch, bundle built after merging `6e2040c58` | Complete, every row passes | [live record](https://raw.githubusercontent.com/ccheever/exact2/792ab62d792b01fea88108865a17289dc01236b3/settings-diagnostics-and-scope/live-drive.txt) |

## Next action

The coordinator runs the two real-input batch steps, reviews the draft PR, and merges it. If
[settings-pages-subscribed-config](20261009-settings-pages-subscribed-config.md) merges first and also changes
Diagnostics' answer (`diagnostics-view.ts`), the second to merge keeps both: the shared, kept reads here and its
subscribed config there.
