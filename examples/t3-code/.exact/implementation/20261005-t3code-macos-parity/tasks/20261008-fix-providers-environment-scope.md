---
name: 20261008-fix-providers-environment-scope
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

# Settings › Providers shows the providers of the environment the scope menu chose

## Outcome

In Settings › Providers, choosing another environment in the scope menu ("Applying settings for All projects on
<name> ▾") shows that environment's providers, read from that environment's server, as the reference does. Today the menu ticks the chosen environment (for example a LAN
one), but the page keeps showing this Mac's providers, and the second server receives no provider queries.

Reference (T3 Code `1e2ecbd975`), `apps/web/src/routes/settings.providers.tsx:7-30`: "Providers are machine state, so
the page shows one environment at a time: the chosen one from the settings scope selector." The route renders
`ProviderSettingsPanel environmentId={environment.environmentId} … scoped`; with no connected environment it says
"Reconnect <label> to set up its providers." (environment scope) or "Connect an environment to set up its providers."

Found by [fix-provider-auth-state](https://github.com/ccheever/exact2/pull/312) (#312) and left out of its scope; the
coordinator made this follow-up task (2026-10-08). #312 merged on 2026-10-08, so it can start.

## Reproduction (from #312, "Found, not in scope")

Seen 2026-10-08 08:39–08:41Z on #312's build 9 (branch `feat(example)/t3-code-fix-provider-auth-state`, head
`914d45daa`), normal launch of a lane copy; an agent-mode drive should show it too.
1. Lane primary on 127.0.0.1:16260 (`T3_LOCAL_HOME`, `T3_LOCAL_PORT`) with its Codex signed in, so its page reads
   "Codex · Authenticated · ChatGPT".
2. A second T3 server with its own T3 home (`t3 serve --base-dir <home> --port 16261 --host <LAN IP>`, Codex not signed
   in); a pairing link from `t3 pair --base-dir <home> --ttl 15m`.
3. Settings › Connections › Add environment › Remote link: paste the link, Add environment. The row is listed and
   connected.
4. Settings › Providers: "Applying settings for All projects on <name> ▾" → choose the second environment's row
   (`… · http://<LAN IP>:16261`). The menu shows its check on that row.
5. Expected: the second environment's providers (Codex not authenticated). Seen: the primary's stay ("Checked 4m ago",
   Codex "Authenticated · ChatGPT"), also after closing and reopening Settings; the second server's trace shows one
   `getConfig` and `reportClientActivity` only, no provider queries.

## Scope and exclusions

Included:
1. The Providers page reads the provider list, status, config and update state of the environment the scope selector
   chose, and every action on the page (sign in and out, install, update, enable, add and edit instances) goes to that
   environment's server.
2. Closing and reopening Settings shows what the reference shows after the same steps. The reference keeps the scope
   in the settings route's search (`components/settings/SettingsScopeContext.tsx`); `prepare` confirms the rule.
3. The empty states for a disconnected or missing environment, with the reference's words (above).
4. A unit test that fails on the base: choosing a second environment changes the environment the page's reads name.

Excluded: other Settings pages' scope handling (not reported broken); framework changes.

## Context and guidance

- Parent: [spec](../spec.md), [plan](../plan.md).
- Prior work: [provider-sign-in-and-install](closed/20261005-provider-sign-in-and-install.md),
  [provider-settings-upkeep](closed/20261005-provider-settings-upkeep.md),
  [settings-scoped-controls-and-theme-editor](closed/20261005-settings-scoped-controls-and-theme-editor.md) (the
  multi-environment settings scope) and #312's provider state work.
- Lane rules from the common brief: isolated homes, ports 16000–16999, `--env PATH=<lane bin>:/usr/bin:/bin:/usr/sbin:/sbin`
  on `open`, never port 3773 or `~/.t3`.

## Dependencies

| Kind | Item | State | Effect |
| --- | --- | --- | --- |
| merged task PR | #312 `fix-provider-auth-state` | merged 2026-10-08 (`421047c46`) | met: the task can start |
| framework issue | none | — | no framework wait |

## Acceptance and reproduction

| Criterion | Check |
| --- | --- |
| Fails before, passes after | the new unit test fails on the base and passes on the branch |
| Live | the five reproduction steps on a lane copy (agent mode where it reaches): step 5 shows the second environment's providers; the second server's trace shows the provider reads; switching back shows the primary's again; reopening Settings follows the reference's scope rule |
| Clone checks | `bun test examples/t3-code`, strict `tsc`, `contract build`, `cargo test -p t3-code-macos --lib` |
| Repository | `git add -A && bun scripts/caps.mjs` and the five checks from `CLAUDE.md` |

## Progress

Planned (2026-10-08, records sync). Not started. No branch or PR yet.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | none |

## Next action

#312 has merged into `feat(example)/t3-code`: `prepare` (find where the Providers page takes its environment), then
implement and verify against the reproduction above.
