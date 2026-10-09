---
name: 20261009-settings-pages-subscribed-config
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-pages-subscribed-config
pr_url: https://github.com/ccheever/exact2/pull/366
verified_commit: null
---

# The other Settings pages read the subscribed server config, not a server request per answer

## Outcome

Settings › Source control, Storage, Archive, Diagnostics, Scheduled tasks and Keybindings read `server.getConfig` and
`server.getSettings` from the server again on every `t3.status`/`t3.events` wake while they are open. The reference reads
both from the connection's subscribed config (`server.getConfig` once when the connection starts, then
`subscribeServerConfig`). After this task each of these pages reads the connection's config, as Settings › Integrations
does since #353, and sends a server request only for the data the reference requests itself.

Found by fix-settings-integrations-loop (#353, merged as `950e8e2e5`), its "Found, not changed" section: `client.drain`
bumps `client.revision` on every `data` answer, so every resource keyed on `data.revision` is asked again on any wake.
These pages do not loop (nothing they read publishes back), but each wake costs two server reads per open page.

## Findings

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| SC-1 | Source control, Storage, Archive, Diagnostics, Scheduled tasks and Keybindings read the server config and settings through `useScopedSettings()`/`useServerConfig()` (the subscribed config). They request nothing when a status or event arrives. | Each page's answer sends `server.getConfig` and `server.getSettings` on every `data.revision` change. | Open each page on a lane for 30 s while the thread list changes (or a provider status arrives); count the lane server's `ws.rpc.server.getConfig` and `ws.rpc.server.getSettings` spans in `server.trace.ndjson`. | #353's measurement method: its record "Live drive" and `settings-integrations-reads.test.ts` |

## Scope and exclusions

Included: how the answers of those six pages read the server config and settings (`client.config` and its `settings`,
as `integrationsPage` in `source-control-view.ts` does now). Excluded: the pages' rows and the Contract; data the
reference itself requests per page (for example Diagnostics' process and trace reads, Archive's archived-thread list):
keep those, but ask them only when the reference does. Coordinate with the audit task that fixes Diagnostics (S2-2,
S2-11): if both change Diagnostics' answer, the second to merge merges the base and keeps both.

## Context and guidance

- Reference (`1e2ecbd975`): `apps/web/src/hooks/useSettings.ts` (`useScopedSettings`), `rpc/session.ts`
  (`server.getConfig` at connect, `subscribeServerConfig`), the settings route components
  (`apps/web/src/components/settings/*`).
- Clone: `source-control-view.ts` (`integrationsPage`: the pattern to follow), the six pages' answers (search for
  `server.getConfig` / `server.getSettings` in `examples/t3-code/*.ts`), `client-ops-settings.ts`.
- Test model: `settings-integrations-reads.test.ts` (one ask on open, zero server reads; one ask per real event).

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Each of the six pages opens with no `server.getConfig`/`server.getSettings` request and answers again with none | A Bun test per page through the app's `answer()` with a runner of `data` and the page's resource (as `settings-integrations-reads.test.ts`); it fails on the base | text: request counts base vs branch |
| A settings change still updates each page | Bun test: a `settingsUpdated` event re-answers the page with the new value | text |
| Live: 30 s on each page reads nothing per wake | one agent-mode drive; count server spans per page, base vs branch | text table (as #353) and one screenshot pair per page showing the same rows |

## Cause and fix

Every `t3.status`/`t3.events` wake drains into `data`, whose `drain` bumps `client.revision`, and app.contract asks each
open settings page again on `data.revision` (and Archive, Diagnostics and Scheduled tasks on the minute tick,
`wallTime.epochAtZero + elapsed`). Measured on the base, per answer: Source control and Storage sent `server.getConfig`
and `server.getSettings`; Keybindings `server.getConfig`; Archive `orchestration.getArchivedShellSnapshot`; the
Scheduled tasks editor `vcs.listRefs` for every project; Diagnostics re-read `server.getResourceTelemetryHistory` once
the 5-s staleness had passed (in practice at each minute tick). Diagnostics and the Scheduled tasks list read no
server config per answer already (#357 and live-automations); the record's SC-1 row named them from #353's note.

The reference reads the server config and settings from the environment's subscribed config (`useScopedSettings`,
`serverConfig`: `server.getConfig` when the connection starts, then `subscribeServerConfig`), and its page queries run
once per mount and on Refresh or its own actions: `createEnvironmentQueryAtomFamily` wraps each query in `Atom.swr`,
whose revalidation runs only when the atom is built (mount), never on a re-render (client-runtime
`state/runtime.ts:492-572`, effect `Atom.ts` `swr`); `archivedShellSnapshot` is refreshed by
`refreshArchivedThreadsForEnvironment` after this client's unarchive/delete (`useThreadActions.ts`).

- `source-control-view.ts` `sourceControlPage`, `settings-data.ts` `storageSettings`, `keybinding-settings.ts`
  `keybindingSettings`: read `client.config` and its `settings` (kept by `subscribeServerConfig`: `settingsUpdated`,
  `keybindingsUpdated`), as `integrationsPage` does since #353. Discovery keeps its per-connection/rescan cache.
- `settings-data.ts` `archivedSettings`: one shared read per visit, keyed on the connection and `settingsRefresh`
  (app.contract bumps it after a rest command, so Unarchive and Delete read again, as the reference's refresh does);
  closing the page drops it. `app.ts` passes the resource's `settingsRefresh` (args[4]).
- `settings-a-telemetry.ts` `history`: one read per visit, window and Refresh (keyed on the connection too); no 5-s
  re-read while the page stays open.
- `scheduled-view.ts` `scheduledPage`: the editor's refs are read when it opens and kept until it closes (Refresh, a new
  connection or another project list reads again); the focused environment's read is shared. `app.ts` passes
  `settingsRefresh` (args[6]).
- Review round: Diagnostics sent `server.getResourceTelemetryHistory` twice when it opened (the reference once). Lane
  trace: the second read started 13.2 ms after the first reply ended. The server sends the telemetry stream's current
  sample at once (`subscribeBeforeSnapshot`); draining it moves `data.telemetry`, Exact asks Diagnostics again and
  forgets the opening answer (LLP 1016 D5), and a reply that reached the transport but not that answer is dropped
  (`host/apple/src/executor_core.rs` `forget`: a completed outcome is dropped undrained). T3Transport's shared read joins
  only a read still pending, so the next answer sent it again. `diagnostics-view.ts` `diagnosticsPage` and
  `settings-a-telemetry.ts` `telemetryPage` now let the page's three reads and the timeline land before they open the
  stream. A wake from elsewhere during the ~25 ms of those reads can still cost a read the same way; none did in the
  three live opens.

No Contract change; no framework limit involved.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Each of the six pages opens with no `server.getConfig`/`server.getSettings` request and answers again with none | pass: per page, opening sends only the page's own query (Source control: discovery; Archive: the archived snapshot; Diagnostics: its four reads), and a wake, a second wake, the minute tick send nothing; a reopen runs the page's own queries again. **Fails on the base** for Source control, Storage, Archive, Diagnostics (the minute tick) and Keybindings | `settings-pages-reads.test.ts` "each settings page reads the subscribed config…"; [base vs branch output](https://raw.githubusercontent.com/ccheever/exact2/0668d7502508d820dafc55b59f774b786046c8ad/settings-pages-subscribed-config/test-base-vs-branch.txt) |
| A settings change still updates each page | pass: `settingsUpdated` reaches Source control (Automatically pull) and Storage (log retention), `keybindingsUpdated` Keybindings (2 bindings), the scheduled-tasks stream and `settingsUpdated` the Scheduled tasks list and editor (new default model), all with no read; Unarchive (settingsRefresh) reads Archive once and Refresh reads Diagnostics once | same file, "a change still reaches each page, without a read" |
| Live: 30 s on each page reads nothing per wake | pass: with the project renamed every 7 s (4-5 wakes per page), after = 0 server reads on every page and in the New task editor; before = Source control 4 getConfig + 4 getSettings, Storage 5 + 5, Archive 6 snapshots, Keybindings 4 getConfig, the editor 13 `vcs.listRefs`; the reference = 0 on every page. On Diagnostics the live count covers `getConfig`, `getSettings` and `getResourceTelemetryHistory` only: the lane server does not trace `getProcessDiagnostics`, `getProcessResourceHistory` or `getTraceDiagnostics` as `ws.rpc` spans (on either app), so only the Bun tests check those three per open and per wake. Review round, on the fixed head: Diagnostics opened three times reads `getResourceTelemetryHistory` once and subscribes once per open (before: 2 and 1; the reference 1 and 1), and 0 reads in 30 s held with 4 renames | [diagnostics-open-reads.txt](https://raw.githubusercontent.com/ccheever/exact2/3c23ac18839ba777de7d88d913d2b5bdd3a2d26e/settings-pages-subscribed-config/diagnostics-open-reads.txt) ([drive](https://raw.githubusercontent.com/ccheever/exact2/1aa9ed61d526de638c672092dbb6b6674902be30/settings-pages-subscribed-config/drive-diag.sh.txt), [counter](https://raw.githubusercontent.com/ccheever/exact2/39a7f16e6e6ab3c3d085e1e20f9872987a7bfbf9/settings-pages-subscribed-config/count-diag.mjs.txt)); | [live-reads.txt](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/live-reads.txt), [drive](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/drive.sh.txt), [counter](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/count.mjs.txt), [renamer](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/renamer.sh.txt); one image per page below |

Images (before 950e8e2e5 | after | reference, each after 30 s open): [Source control](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/01-source-control.png),
[Storage](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/02-storage.png), [Archive](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/03-archived.png), [Diagnostics](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/04-diagnostics.png) (before is the
evidence-base build at 950e8e2e5, which predates #357: its page stays empty, so it does not show the base 6cb2828ba's
page; only the Bun test output covers the base's Diagnostics reads), [Scheduled tasks](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/05-scheduled-tasks.png),
[New task editor](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/06-scheduled-editor.png), [Keybindings](https://raw.githubusercontent.com/ccheever/exact2/e3a599c8f6cc342adf354ea8ebdaf969dd581b81/settings-pages-subscribed-config/07-keybindings.png). The rows read the same before
and after.

## Found, not changed

- Source control's discovery (`server.discoverSourceControl`) runs at the first visit per connection; the reference's
  lane showed none at its visit (its atom may have been warm). Not changed.

## Tests

- Added `settings-pages-reads.test.ts` (10 tests through the app's `answer()`; 8 fail without the fix, all pass with it).
  It imports its own instance of `app.ts` (`./app.ts?settings-pages-reads`): the app's client adopts only a newer
  connection generation, the runner visits files in directory order (this file ran before providers-scope,
  settings-diagnostics-scope and settings-integrations-reads, whose fixed generations then went unadopted: 9 failures in
  the first full run), so this file never touches the shared client.
- Updated `settings-a-telemetry.test.ts`: the history is read once per visit and window, a minute later included.
- Review round: added "Diagnostics: the stream's first sample, which lets the opening answer go, costs no second read"
  to `settings-pages-reads.test.ts` (every call the opening answer makes after its subscribe has its reply dropped, as
  the runtime drops one that reached the transport after the answer was forgotten). Without the fix it fails:
  `getResourceTelemetryHistory` sent twice and the second answer reads all four again. `settings-diagnostics-scope.test.ts`
  "a read the next answer joins…" replies three turns after a read, not two: the reads now land before the stream opens,
  so two turns let only one answer go and the test would no longer replace every answer before its reply.

## Attempts and evidence

Live drives (agent mode, lanes `settings-pages-subscribed-config[-before]`, base port 16200): before attempt 1 stopped at
`tap View diagnostics` (an unquoted multi-word target); attempt 2 at the New task button, which the lane's Codex update
toast (the lane finds the real Codex CLI) covered; attempt 3 likewise; attempt 4 (dismiss both toasts, the editor last)
is the before run. The after drive ran once. Reference: one CDP session plus one for the editor and Keybindings.
Review round: one after drive on the rebuilt bundle (Diagnostics opened three times, then held 30 s; renamer running),
first attempt.

## Next action

Coordinator review of the draft PR. No real-input rows: the change is about requests, which agent mode measures.
