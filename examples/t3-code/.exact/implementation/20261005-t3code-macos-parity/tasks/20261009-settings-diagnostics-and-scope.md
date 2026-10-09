---
name: 20261009-settings-diagnostics-and-scope
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

# Settings: the Diagnostics page fills again, and Diagnostics and the Project page keep the all-environments scope

## Outcome

- Settings › General › View diagnostics shows its sections again (Resource monitor through Top Span Names). Today it stays
  empty, because its request is replaced before it can answer. The 2026-10-07 audit saw this page filled (regression).
- Diagnostics keeps the scope sentence "Applying settings for All projects across All environments".
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
| S2-11 | Diagnostics' sentence stays "All projects across All environments". | `s2-11-scope.png` | agent |
| PG-8 | "Project settings" from the palette, the sidebar and the thread menu opens with "work across All environments". | `pg8-project-scope.png` | agent |

## Next action

Prepare a branch from `feat(example)/t3-code`. S2-2 is high severity: start it first. Build and unit-test. Then do one
batched live drive at the end for every row's before/after pair. Close every row in this PR, or record the blocker of a
row that cannot pass.
