---
name: 20261008-fix-providers-environment-scope
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-fix-providers-environment-scope'
pr_url: https://github.com/ccheever/exact2/pull/329
verified_commit: 52378bbef
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

## Cause and fix

Cause: the `providerPage` resource took no scope. `app.ts` built the page, the Add provider wizard and the ACP search
from the focused `T3Client`, and every page action (`providerChange`, `providerAdd`, `setup:`, `upkeep:`) went to the
focused connection, so the scope menu changed only the sentence.

Fix (`92f678e26`):
- `providers-scope.ts` (new area module) resolves the route's environment from the settings scope (`settingsScopeOf`,
  shared with the scope sentence; SettingsScopeContext's `environment`: the connected target, the primary first) and uses
  its provider host: the focused client, or a background environment's `FleetSetupHost` (the host the welcome already
  uses). The page answer names it (`providerPage.environment`); the wizard, the ACP search and every page action carry
  it. Update all and the launch prompt stay on every environment (ProviderUpdatesAction). While Settings shows
  Providers but the scope names no connected environment, the page names `-` and every write is refused.
- `FleetSetupHost` gains the window's client as owner (model favorites and order are client settings; toasts), `ids`,
  `environmentId`, `shell` and the managed Codex command target; env-variable drafts are kept per environment.
- Default: `selectSingleEnvironmentScope` (primary, else connected, else first). The Providers route's boundary reads the
  chosen environment's connection ("Reconnect <label> to change its settings.").
- `app.contract` 1,488 → 1,468 lines (after merging 96c4c38f2, #311): resources and sends take the environment; a scope change starts the page over on
  its first row (routes/settings.tsx keys the page on its search); Settings links from outside Settings start with no
  scope (`retainSettingsScope`) through `openSettings()` (the Settings button, a setup link, the toast's Settings, the
  Connections and project links); a setup link names its environment (the chat's: its thread's, `ChatView.openProviderSetup`;
  Settings' own: the scope's).
- Empty states: as the reference renders them. A disconnected chosen environment shows the boundary's "Reconnect <label>
  to change its settings."; a removed one "This environment is no longer available."; none at all "Connect an environment
  to set up its providers.". The route's own "Reconnect <label> to set up its providers." never shows in the reference,
  because `SettingsScopeBoundary` answers first for the same condition.
- Scope rule on reopening (scope item 2, `prepare`'s rule): the reference keeps the scope in the route's search; a link
  from outside Settings carries no scope (or only its own explicit target), so reopening Settings starts on the primary.

## Acceptance results

| Criterion | Result | Proof |
| --- | --- | --- |
| Fails before, passes after | pass | `providers-scope.test.ts` (5 tests through `answer()`): base `44e939f1e` 0 / 5 ([run](https://raw.githubusercontent.com/ccheever/exact2/1360884715864b2d35f81c6b1e42d1a6d8fbfd54/fix-providers-environment-scope/regression-test-base.txt)), branch 5 / 0 ([run](https://raw.githubusercontent.com/ccheever/exact2/49960f609c8d8f0db62c359d4d8860b8e5389d6c/fix-providers-environment-scope/regression-test-branch.txt)) |
| Live: step 5 shows the second environment's providers | pass | agent drive, [image 1](https://raw.githubusercontent.com/ccheever/exact2/00511e5c11abf857b323a59b31f1f0023c248392/fix-providers-environment-scope/01-second-environment-chosen.png) |
| Live: the second server's trace shows the provider reads | pass: the page's provider writes and reads (`getSettings`, `updateSettings`, `getConfig` from the Codex switch); Refresh is covered by the regression test `providers-scope.test.ts` (Refresh goes to the chosen server) | [drive record](https://raw.githubusercontent.com/ccheever/exact2/39a5c500c6d60fbf00b8f0b0bed9431df930c27a/fix-providers-environment-scope/live-drive.txt), [image 3](https://raw.githubusercontent.com/ccheever/exact2/1adea10af4d1ff7c313490459d2aa4c49e63168b/fix-providers-environment-scope/03-codex-switch.png) |
| Live: switching back shows the primary's again | pass | [image 4](https://raw.githubusercontent.com/ccheever/exact2/696627f223eda937c0f6d6a3bd8c4ea74d34bd25/fix-providers-environment-scope/04-back-to-this-mac.png) |
| Live: reopening Settings follows the reference's scope rule | pass | [image 5](https://raw.githubusercontent.com/ccheever/exact2/d987c0161bc7dd0d86060bae307ce5647c150109/fix-providers-environment-scope/05-settings-reopened.png) |
| Empty state, disconnected environment | pass | [image 6](https://raw.githubusercontent.com/ccheever/exact2/3240238f8210b988d383cf2218059529f3ea1c7c/fix-providers-environment-scope/06-second-environment-stopped.png); removed and none: unit test |
| Clone checks | pass | on the merged tree `52378bbef`: `bun test examples/t3-code` 3,468 pass / 1 skip / 0 fail; strict `tsc` clean; `contract build` 5,512 slots, 46 resources; `cargo test -p t3-code-macos --lib` 13 pass |
| Repository | pass | on `52378bbef`: caps within; build exit 0; test 3,521 passed / 0 failed / 34 ignored (94 binaries); clippy and fmt clean; boot allowed paths only |

Refresh on the second environment: covered by the regression test `providers-scope.test.ts` (Refresh goes to the
chosen server: `server.refreshProviders` carries the second environment's fleet key). The drives pressed it too, but each
lane server writes its trace in batches and was stopped 6 s later, so the span is not in the traces.

### Review notes, closed (coordinator follow-up, 2026-10-08)

The independent review's round-2 notes, each closed in this PR (unit tests; no new live session):
1. **Setup link from Settings' own model picker, with a project scope.** Fixed. The reference opens Providers on the
   scope's representative environment: `ProjectDefaultsSettings.tsx:60-62, 168-173` (`target`, navigate only
   `if (representative)`) and `SettingsPanels.tsx:3260-3266` (`useSettingsScope().environment`, no link without one).
   `settingsCore` now returns that environment (`scopeEnvironment`, from `scopeRepresentative`: the scope's connected
   environment, the primary first, which Providers uses too); `openProviderSettings` uses it inside Settings and opens
   nothing without one; outside Settings it keeps the chat's environment (`ChatView.tsx:5217-5223`). Test: "a setup link
   inside Settings opens Providers on the scope's representative environment" (through `answer()`: a project only on the
   second environment names it; a project there that dropped names none).
2. **Wizard commands when its environment drops.** The reference shows no message: the Add provider dialog is a child of
   `EnvironmentProviderSettings` (`ProviderSettingsPanel.tsx:1377-1385`), which `AccessGatedProviderSettings`
   (`:551-586`, `classifyProviderEnvironmentAccess` in `ProviderSettingsPanel.logic.ts:150-171`) replaces with a
   placeholder when the environment is not connected, and the scope boundary replaces the scoped route; the dialog
   unmounts. Matched: `providerTick` closes the page's dialog when `providerPage.environment` is `-` (within a second).
   Tested: the page answers `-` and refuses every write when its environment drops ("a disconnected or removed
   environment leaves the words to the scope boundary"). The close itself is one Contract line, checked by the contract
   build and review round 3, not by a unit test: the Contract test form (`agent.mjs --test`) launches the app, and this
   task makes no further live session (one drive per build; the coordinator asked for unit tests only).
3. **Background writes not marked "uncertain".** Matches the reference: it marks no settings or provider write as
   uncertain on any environment; its only uncertain handling is the multi-thread submissions of `ChatView.tsx`
   (`uncertainSubmissions`, :778, :1947, :9092-9096). In the clone the flag only sets `ClientError.uncertain`, which no
   provider op reads (`runProviderOp`, `settingsFailure`). No change, no new task.
4. **Refresh trace.** Recorded above as covered by the regression test.

The new test fails on the branch head before the follow-up (`b2c512068`: 5 pass / 1 fail, `scopeEnvironment`
undefined) and passes after (6 / 0). Review round 3: the code has no findings; its one should-fix was two assertions on
the text of `app.contract`, which were removed (CLAUDE.md: verify by running, never by grepping).

## Real-input batch steps

None: the bug is about which server the page reads and writes; every row ran in agent mode.

## Progress

2026-10-08: implemented, verified and delivered as draft [PR #329](https://github.com/ccheever/exact2/pull/329) against
`feat(example)/t3-code` (merged `44e939f1e` first). Verify runner attempt 2 passed on the final source
(`source_unchanged: true`); the committed tree `92f678e26` matches its report. Independent review: round 1 no blocking
findings, two should-fix items taken; round 2 none.

2026-10-08 (coordinator follow-up): the review notes closed (see "Review notes, closed"): `506c4f9fe` (runner attempt 4
passed; review round 3: no code findings, its test should-fix taken) and the records. #311 merged: base `96c4c38f2` merged
as `52378bbef` (STATUS.md conflict, both sides kept); runner attempt 5 on the merged tree passed (11 / 11,
`source_unchanged: true`). `app.contract` 1,468 lines.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| unit test, base | `f45eab04a`, then `44e939f1e` sources | 0 pass / 4 fail, then 0 / 5 (the page with the second environment lists this Mac's "Authenticated · ChatGPT"; Refresh goes to the focused server) | [base run](https://raw.githubusercontent.com/ccheever/exact2/1360884715864b2d35f81c6b1e42d1a6d8fbfd54/fix-providers-environment-scope/regression-test-base.txt) | none |
| full suite, first run | working tree | 3,293 pass / 4 fail: the test's `answer()` calls left the highlighter sliced for later files; fixed with `resetHighlightSlicing()` in `afterEach`, then 3,297 / 0 | local `target/fpes/evidence/bun-test-1.txt`, `-2.txt` | none |
| live drive, base, attempt 1 | `f45eab04a` dev build | all taps landed; the scene screenshots painted white (popovers only); the trace-keyed watcher missed the batched Refresh span | local only | none (retried) |
| live drive, base, attempt 2 | `f45eab04a` dev build | window screenshots; with …:16551 chosen the page shows this Mac's Codex; the switch writes to this Mac's server | images 1-6 (before) | none |
| live drive, branch, attempt 1 | pre-review source | the second environment's providers; the switch writes to it | local only | none |
| verify runner attempt 1 + review round 1 | pre-review source | 11 / 11 checks passed; review: two should-fix (empty-scope writes fell back to the focused server; env drafts shared), notes 5 and 6 | local `target/fpes/verify/attempt-1` | taken |
| live drive, branch, final | final source (`92f678e26`) | as attempt 1; boundary text with the second server stopped | images 1-6 (after), [record](https://raw.githubusercontent.com/ccheever/exact2/39a5c500c6d60fbf00b8f0b0bed9431df930c27a/fix-providers-environment-scope/live-drive.txt) | none |
| verify runner attempt 2 + review round 2 | `92f678e26` (fingerprint `d887a061…`) | 11 / 11 passed, `source_unchanged: true`; committed tree matches; review: no blocking or should-fix findings | local `target/fpes/verify/attempt-2` | none |
| follow-up: runner attempts 3-4 + review round 3 | `506c4f9fe` | 11 / 11 passed each; review: no code findings; two assertions on the text of `app.contract` removed | local `target/fpes/verify/attempt-3`, `-4` | none |
| merged tree: runner attempt 5 | `52378bbef` (96c4c38f2 merged) | 11 / 11 passed, `source_unchanged: true`; `bun test examples/t3-code` 3,468 / 1 / 0; contract 5,512 slots | local `target/fpes/verify/attempt-5` | none |

## Next action

Coordinator: review and merge draft PR #329. Merge order: #311 may land first; this branch changed `app.contract` in place
(resources, sends, `coreScope`, the Settings openers), so a later merge with #311 may touch nearby lines.
