---
name: 20261009-settings-pages-subscribed-config
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

## Next action

Build after the audit's Diagnostics task merges, or in parallel with it on separate pages first.
