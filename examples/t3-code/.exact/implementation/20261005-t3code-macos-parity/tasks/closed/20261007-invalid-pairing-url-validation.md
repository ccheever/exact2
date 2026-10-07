---
name: 20261007-invalid-pairing-url-validation
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-invalid-pairing-url
pr_url: null
verified_commit: null
---

# Welcome rejects an invalid pairing URL with its own inline error

## Outcome

Submitting malformed text in Welcome > Add a computer shows `Pairing URL is invalid.` in
the pairing form. It does not enter a transport connection attempt or put the form's error
in the background application's global banner. Correcting the field lets the user pair normally.

This task records a live discrepancy found on 2026-10-07. The original mismatch below
was observed in both apps; implementation and verification progress follow it.

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

Publish the verified task PR into `feat(example)/t3-code` with accessible
before/after screenshots, then record its URL and delivery status. The implementation
commit is recorded in the bookkeeping update after the commit exists.

## Scope and exclusions

Welcome's Add a computer URL validation, its form-owned errors and recovery are in
scope. Settings keeps its separate host and code mode. No framework, transport,
provider sign-in or delivery changes are required.

## Context and guidance

Parent [specification](../../spec.md), especially macOS-only scope and its latest
continuation, and [plan](../../plan.md), especially the 2026-10-06 PR workflow.
Guidance is `20261005-platforms-v3`: foundations, state and data, platforms,
accessibility, testing and debugging. The checked-out framework source remains
the implementation authority. Reference `packages/shared/src/remote.ts` and
`remote.test.ts` were inspected at `1e2ecbd9758830669684b494d4398f626b0576e0`.

## Dependencies

No unresolved task or framework prerequisite. The Orca worktree starts from
`origin/feat(example)/t3-code`, commit `f9027798945cf33d5ee900c38b269505c0b2b00f`.
The user authorized implementation, before/after evidence and task PR creation on
2026-10-08. The parent plan's later workflow keeps verification logs local.

## Implementation notes

- `remote.ts` ports the reference's strict pairing URL branch, preserving malformed
  link, missing token and hosted backend errors. Exact adapts its result to the
  existing native origin/credential pair and uses `ClientError`.
- Welcome dispatches `welcome-pair` in both connection states. It shares the existing
  pairing and cleanup path but leaves errors in the Welcome form. Settings still
  uses `environment-add` and its existing host/code parsing and toasts.
- Whitespace-only input is disabled, matching the reference's submit guard. A
  successful submission clears the field and prior error, then collapses the form.
- `remote.test.ts` ports URL cases and drives `T3Client.command` to check transport
  exclusion, connected/disconnected behavior, error ownership, recovery and Settings.

## Progress

2026-10-08: focused tests pass, 53 tests. The full application suite passes, 2,343
tests, one existing skip. Strict TypeScript, Contract compilation, 11 app Rust tests,
the rebuilt native bundle and this branch's staged caps pass. Native acceptance and
[independent review](../../reviews/20261008-invalid-pairing-url-validation.md) pass.
The shared unchanged-framework gates also pass: build, test, clippy with warnings
denied, formatting, caps and boot. The coordinator ran those six commands at the
same core base `f9027798945cf33d5ee900c38b269505c0b2b00f` in its isolated snooze worktree,
with individual results in `target/title-snooze/verification-01/report.json`, checks
001 through 006. That report also contains an unrelated failing snooze acceptance
check; only the six shared gate results are used here.

| Acceptance | Observed result |
| --- | --- |
| Malformed link, both connection states | Exact inline validation message, enabled retry and no global banner in native tree/state. Command tests observe zero connection, pairing or disconnect calls. |
| Validation distinctions | Missing token, unsupported protocol and malformed hosted target return the reference messages in native captures and ported tests. |
| Recovery | Enter pairs the first disposable official backend; Pair adds the second. Native state shows two connected computers, cleared field/error and collapsed form. Continue opens Agents. |
| Genuine transport failure | Unavailable port 16874 fails inline with retry and no global banner in both states. Existing native message is preserved; exact Electron transport-error wording parity is unverified. |
| Settings regression | Separate host/code succeeds through `T3Client.command` in both connection states and retains the Settings success toast. |
| Keyboard | Native Enter rejects malformed input and successfully retries a valid disposable link. Physical keyboard input is not claimed. |

Primary commands, run with pinned Bun 1.4.2 on `PATH`:

```sh
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2023 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2023,DOM examples/t3-code/app.ts
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/pairing-validation/verified.plan --json
EXACT_APP_DIR="$PWD/examples/t3-code" cargo test -p t3-code-macos --lib --no-fail-fast
EXACT_APP_DIR="$PWD/examples/t3-code" bun host/apple/build.mjs t3-code-macos --bundle
bun scripts/caps.mjs
```

The practical native sweep was macOS 26.6.2, Xcode 27.0, 1280×840, light appearance.
The parent plan's latest continuation supersedes repeated visual matrices. The
reference source formats network errors with an endpoint and cause; preserving
the existing native network message was the coordinator's scoped decision here.
No exact transport-error text parity or broader visual matrix is claimed.

## Attempts and evidence

Baseline source `f9027798945cf33d5ee900c38b269505c0b2b00f` was built and driven before
edits. `target/pairing-validation/before/invalid-disconnected.png` and its tree/state
show `Could not connect to the server.` inline and the global `error-banner` present.
The reset first submitted an unavailable fixture link at port 16874 so the old
malformed-input path would never contact the default port 3773. The baseline window
was closed after capture. All app runs use the native driver's disposable storage
and in-memory credentials. Fixture servers use their own data directories and
`T3CODE_TELEMETRY_ENABLED=false`.

After captures are under `target/pairing-validation/after-runtime-02/`. The first
after attempt stalled in an unbounded fixture readiness probe before opening the
app; its recorded server PID was stopped. Adding a one-second timeout to the local
capture helper made the second attempt pass. No production code changed between
those attempts. Both fixture processes and the app were stopped after the sweep.

Source digest: `d65e457275b116c34ee976de6f2bf428987551c2a1ea524d08684f1a7a6c674b`.
The 835-path report at `target/pairing-validation/verification-20261008/report.json`
passed with `source_unchanged: true`; comparison against
`target/pairing-validation/recipe.json` matches. Its recipe binds the exact local
capture driver and checks command behavior, native readback and Contract compilation.
All 40 artifact checksums in the plan-local
`evidence/20261007-invalid-pairing-url-validation/20261008-welcome-validation/complete-manifest/manifest.json`
validate. Logs, recipes and manifests stay local. The PR supplies accessible links
to the reviewed credential-free before/after and recovery screenshots.
