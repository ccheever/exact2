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
remain pending until a Pro account is available, superseding the earlier closed disposition; the Cursor row is
now closed because the user has no Cursor Pro account (records sync, 2026-10-08).
Source: [original task and attended evidence](closed/20261005-provider-sign-in-and-install.md#attempts-and-evidence),
reconciled 2026-10-08 at `29dbc5dbf`. No implementer is assigned yet.

## Disposition and resumption

| PR item | Status / owner | Resume condition and completion evidence |
| --- | --- | --- |
| Subscription trace (`trace-diff.mjs`) | Not run; deferred by the 2026-10-06 decision not to build oracle/trace tooling | Resume only if that decision changes and tooling is available. Compare reference and clone subscriptions, unsubscriptions and refresh calls on the same scenario; record revisions and explain differences. Fixture RPC logs are not a reference trace comparison. |
| Oracle pixel pairs (`electron-oracle.mjs`) | Not run; same tooling decision | Resume under the same condition. Capture reference/clone pairs for the original acceptance matrix's sizes and appearances; link results and remaining gaps. Source inspection is not pixel verification. |
| Cursor real sign-in | **Closed (2026-10-08): the user has no Cursor Pro account.** Not verified with a real account | The recorded free-account `403 plan_required` stays the only live result; it is not successful verification. Reopen only if a Pro account becomes available. |
| Antigravity Google sign-in | Not verified; this task | A user elects to sign in to an isolated lane account. Drive the real browser flow, verify the app's resulting account state and retain sanitized evidence. The real runtime install already passed; it does not prove sign-in. |
| Real ACP sign-in (Gemini CLI) | Not verified; this task | A user elects to sign in with an available ACP agent. Verify discovery, the advertised flow and authenticated state in the app. Record provider/runtime versions; fixture success is insufficient. |
| Real-account Sign out / Change account | Not verified; this task; fixture coverage exists | Use a disposable account on a provider exposing the in-app Account row, with the user's agreement to sign out/change it. Verify Cancel leaves authentication unchanged, Confirm signs out once, and Change account completes with the new account state. Preserve the retained Codex/Claude lane logins. 2026-10-08: taken by [managed-codex-chatgpt](closed/20261005-managed-codex-chatgpt.md) ([PR #256](https://github.com/ccheever/exact2/pull/256)) on its own lane's managed ChatGPT account (Disconnect has no confirmation there, so Cancel is n/a); deferred to the real-input batch — screen locked (user away), steps in that task's "Real-input batch steps" item 6. Closes when that batch passes. |
| URL-auth action (`ProviderSettingsPanel.environment.test.tsx:584`) | Owned by [provider-settings-upkeep](closed/20261005-provider-settings-upkeep.md), existing URL auth acceptance row | Port the reference case and verify Continue authentication, `acceptAcpRegistryUrlAuth`, and an expired request. Record test and UI evidence there. |
| Accessible progress value | [X49](../issues/20261007-x49-progress-value-accessibility.md), filed as [#279](https://github.com/ccheever/exact2/issues/279); **waits for main fix of #279** (Charlie, 2026-10-08: "Add ARIA range values first; then determinate progress"); nonblocking workaround is status text plus `aria-description` | After the fix merges to `main` and an adoption round brings it in, swap `aria-description` for `aria-valuenow`/`min`/`max`/`valuetext` and verify the accessibility representation. |
| Tab reaches zero-size shortcut buttons after a dialog's last button | Owned by [dialog shortcut focus](closed/20261008-dialog-shortcut-focus.md) | Reproduce on the current base, fix the focus path, and record real-keyboard evidence without breaking shortcuts. |

## Related implementation tasks

Codex CLI authentication was verified in the attended session; the managed in-app ChatGPT
flow belongs to [managed-codex-chatgpt](closed/20261005-managed-codex-chatgpt.md).
Terminal sign-in implementation belongs to [sign-in-terminals](closed/20261005-sign-in-terminals.md).
Neither task's implementation status proves the real-account rows above.

## Acceptance and next action

- [x] Cursor real sign-in: closed, no Cursor Pro account (2026-10-08).
- [ ] When the user elects to provide Google/ACP sign-in, perform the two real-account rows.
- [ ] With a disposable supported account, perform real Sign out / Change account.
- [ ] If oracle/trace tooling is authorized later, complete both deferred comparison rows.
- [ ] For each completed row, record the commit, app/server/provider versions, steps, sanitized
  result and evidence link. Otherwise retain its explicit reason and resumption condition.

Keep this task open for the remaining verification after PR #238 merges. Linked implementation
and framework work stays in its existing owner. 2026-10-08 (records sync): the Cursor row is closed (no Cursor
Pro account); the X49 row waits for main fix of #279.

2026-10-08 (real-input batch, records PR): Antigravity Google sign-in and real Sign out (Cancel/Confirm) pass; Gemini CLI refused by Google for this account; Codex Change account passes (#256). Results and proof: "Real-input batch (2026-10-08)" below.

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| Antigravity Google sign-in (personal account) | PASS: "Authenticated · Google account", "Signed in."; token only in the lane T3 home | [ag-08-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/01-ag-08-crop.png), [ag-10-signed](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/02-ag-10-signed.png) |
| Real-account Sign out (Cancel, Confirm) and sign back in | PASS: Cancel keeps it, Confirm → one logout, then signed in again the same way | [ag-11-signout-dialog](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/03-ag-11-signout-dialog.png), [ag-signout](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/04-ag-signout.png), [ag-16-signed-again-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/05-ag-16-signed-again-crop.png) |
| Gemini CLI (ACP) "Log in with Google" | Blocked by Google: the browser sign-in succeeded but the agent refused — "This client is no longer supported for Gemini Code Assist for individuals…". Also FAIL (clone bug): the Add provider dialog's Sign-in method select does not open; no in-app consent step appeared before the agent opened the browser (reference behaviour not verified) | [gm-14-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/06-gm-14-crop.png), [gm-16-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/07-gm-16-crop.png), [gm-chrome-auth-success](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/08-gm-chrome-auth-success.png), [gm-17-signed](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/09-gm-17-signed.png) |
| Real-account Change account (Codex managed ChatGPT, #256) | PASS (see #256) | — |

Full record: [provider-sign-in-followup.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-sign-in-verification-followup/provider-sign-in-followup.txt).
