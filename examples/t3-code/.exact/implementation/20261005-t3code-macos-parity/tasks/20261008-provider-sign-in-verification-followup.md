---
name: 20261008-provider-sign-in-verification-followup
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

# Provider sign-in: remaining verification after PR #238

## Outcome and provenance

Keep every item in [PR #238](https://github.com/ccheever/exact2/pull/238)'s
"Not done / not verified" section traceable after the implementation merges.
This task records remaining verification, existing ownership and explicit user decisions;
it does not claim fresh live testing. The user clarified on 2026-10-08 that Cursor must
remain pending until a Pro account is available, superseding the earlier closed disposition.
Source: [original task and attended evidence](closed/20261005-provider-sign-in-and-install.md#attempts-and-evidence),
reconciled 2026-10-08 at `29dbc5dbf`. No implementer is assigned yet.

## Disposition and resumption

| PR item | Status / owner | Resume condition and completion evidence |
| --- | --- | --- |
| Subscription trace (`trace-diff.mjs`) | Not run; deferred by the 2026-10-06 decision not to build oracle/trace tooling | Resume only if that decision changes and tooling is available. Compare reference and clone subscriptions, unsubscriptions and refresh calls on the same scenario; record revisions and explain differences. Fixture RPC logs are not a reference trace comparison. |
| Oracle pixel pairs (`electron-oracle.mjs`) | Not run; same tooling decision | Resume under the same condition. Capture reference/clone pairs for the original acceptance matrix's sizes and appearances; link results and remaining gaps. Source inspection is not pixel verification. |
| Cursor real sign-in | Not verified; this task; waiting for a Cursor Pro account | When the user provides a Pro account, complete in-app browser sign-in on an isolated lane and verify authenticated state in the app. Record sanitized evidence and app/server versions. The recorded free-account `403 plan_required` is not successful verification. |
| Antigravity Google sign-in | Not verified; this task | A user elects to sign in to an isolated lane account. Drive the real browser flow, verify the app's resulting account state and retain sanitized evidence. The real runtime install already passed; it does not prove sign-in. |
| Real ACP sign-in (Gemini CLI) | Not verified; this task | A user elects to sign in with an available ACP agent. Verify discovery, the advertised flow and authenticated state in the app. Record provider/runtime versions; fixture success is insufficient. |
| Real-account Sign out / Change account | Not verified; this task; fixture coverage exists | Use a disposable account on a provider exposing the in-app Account row, with the user's agreement to sign out/change it. Verify Cancel leaves authentication unchanged, Confirm signs out once, and Change account completes with the new account state. Preserve the retained Codex/Claude lane logins. |
| URL-auth action (`ProviderSettingsPanel.environment.test.tsx:584`) | Owned by [provider-settings-upkeep](20261005-provider-settings-upkeep.md), existing URL auth acceptance row | Port the reference case and verify Continue authentication, `acceptAcpRegistryUrlAuth`, and an expired request. Record test and UI evidence there. |
| Accessible progress value | Existing local [X49 issue](../issues/20261007-x49-progress-value-accessibility.md); nonblocking workaround is status text plus `aria-description` | Recheck framework support, follow X49's publication decision, then adopt the supported value and verify its accessibility representation. This task does not authorize upstream publication. |
| Tab reaches zero-size shortcut buttons after a dialog's last button | Owned by [dialog shortcut focus](20261008-dialog-shortcut-focus.md) | Reproduce on the current base, fix the focus path, and record real-keyboard evidence without breaking shortcuts. |

## Related implementation tasks

Codex CLI authentication was verified in the attended session; the managed in-app ChatGPT
flow belongs to [managed-codex-chatgpt](20261005-managed-codex-chatgpt.md).
Terminal sign-in implementation belongs to [sign-in-terminals](closed/20261005-sign-in-terminals.md).
Neither task's implementation status proves the real-account rows above.

## Acceptance and next action

- [ ] When a Cursor Pro account is available, complete the real in-app Cursor sign-in row.
- [ ] When the user elects to provide Google/ACP sign-in, perform the two real-account rows.
- [ ] With a disposable supported account, perform real Sign out / Change account.
- [ ] If oracle/trace tooling is authorized later, complete both deferred comparison rows.
- [ ] For each completed row, record the commit, app/server/provider versions, steps, sanitized
  result and evidence link. Otherwise retain its explicit reason and resumption condition.

Keep this task open for the remaining verification after PR #238 merges. Linked implementation
and framework work stays in its existing owner; Cursor remains pending until Pro-account
verification succeeds.
