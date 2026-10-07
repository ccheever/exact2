---
name: 20261007-invalid-pairing-url-validation
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

# Welcome rejects an invalid pairing URL with its own inline error

## Outcome

Submitting malformed text in Welcome > Add a computer shows `Pairing URL is invalid.` in
the pairing form. It does not enter a transport connection attempt or put the form's error
in the background application's global banner. Correcting the field lets the user pair normally.

This task records a live discrepancy found on 2026-10-07. No fix is included.
`verification: unverified` describes the future fix. The mismatch below was observed in both apps.

## Observation and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.
Both source trees were unchanged during the comparison.

1. In a fresh Welcome flow, expand Add a computer.
2. Enter the exact text `invalid pairing URL` into Pairing link.
3. Press Pair and inspect the form and the background app before and after the operation settles.

| App | Observed result |
| --- | --- |
| Reference Electron | Inline alert `Pairing URL is invalid.`; Pair remains available after validation |
| Exact, first capture | Field disabled and button reads `Pairing...`; the background `error-banner` already says `Could not connect to the server.` |
| Exact, settled capture | Field enabled; both `welcome-pairing-error` and background `error-banner` say `Could not connect to the server.` |

The busy connection UI is observed. This audit did not capture a network trace proving which
transport requests were sent for the malformed value. The incorrect inline result and duplicate
global error are present in the retained native tree.

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-02-invalid-pair.{png,txt}`
- `target/desktop-audit/native/native-02-invalid-pair.{png,json}`
- `target/desktop-audit/native/native-03-invalid-pair-settled.{png,json}`
- Session driver log: `target/desktop-audit/native/driver.log`. These files are local audit evidence.

## Scope and implementation guidance

The reference separates a pairing URL from a backend host plus a separate code.
`packages/shared/src/remote.ts` defines `RemotePairingUrlInvalidError` and
`resolveRemotePairingTarget`; `remote.test.ts` covers malformed URLs, unsupported protocols,
hosted pairing links and missing tokens. Welcome's `PairingForm` in
`apps/web/src/components/onboarding/WelcomeWizard.tsx:518-552` calls `connectPairing` with
`reportFailure: false` and places the failure message only in its own form.

The clone's `app.contract:191-197` passes the Welcome field as the command value and chooses
`environment-add` when connected or `connect` otherwise. `client-ops-connection.ts:16` calls
`parsePairing(id || this.origin, value)`. `protocol.ts:94` accepts server addresses and
separate credentials as well as links, so its input contract is broader than Welcome's field.
Inspect both dispatch branches when adding pairing-URL validation; do not remove Settings >
Connections' supported host-plus-code entry mode.

`client.ts:642-649` distinguishes form-owned failures from global failures. Its form-command
classification includes environment operations but not plain `connect`. This explains a
plausible path to the duplicate background error and should be checked with a targeted test.
`pages-welcome.contract:258-268` already has the inline alert and busy controls.

Preserve the reference's separate invalid-URL, missing-token, unsupported-protocol and actual
connection-failure results. Validate before calling the transport. Never include credentials
in error messages or captured evidence. This is app behavior, with no framework issue identified.

## Deduplication

- `closed/20261005-environment-routes.md` covers routes and wrong-machine rejection, not malformed Welcome input.
- `20261005-local-primary-environment.md` covers first-run decisions and the local primary, not this validation path.
- `closed/20261005-round12-wrapup.md` covers removed credentials and onboarding timestamps.
- `closed/20261007-let-go-banner.md` suppresses abandoned-answer failures; the current error is a pairing-form failure.

No existing task explicitly tracks this mismatch.

## Acceptance and reproduction

| Criterion | Setup and action | Expected result | Proof |
| --- | --- | --- | --- |
| Malformed URL | Submit `invalid pairing URL` from Welcome while disconnected and while already connected to one environment | Inline `Pairing URL is invalid.`; no connect/pair transport call; no new global banner | Targeted command test and live captures |
| Validation distinctions | Submit a URL with no token, an unsupported protocol and a malformed hosted target | The reference's specific message in the form; no credentials or unrelated state changed | Ported reference cases |
| Recovery | Replace invalid text with a valid disposable pairing link and submit | Successful pair and normal next-step behavior; prior form error cleared | Isolated backend and UI readback |
| Genuine transport failure | Submit a structurally valid link to an unavailable fixture server | Reference-compatible failure in the form, with retry available and no duplicated global banner | Live failure capture |
| Settings regression | Add an environment using separate host and pairing code | Existing supported flow still succeeds | Focused connection test |
| Keyboard | Submit with Enter, correct the field, then retry | Same validation and recovery as Pair button | Bounded live drive |

## Next action

Implement the Welcome input boundary and form-owned error handling, then run targeted
connection/client tests and rebuild the macOS app for the two Welcome connection states.
