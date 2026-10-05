---
name: 20261005-x35-secure-text-entry
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: [20261005-managed-codex-chatgpt, 20261005-provider-settings-upkeep, 20261005-provider-sign-in-and-install, 20261005-ssh-password-and-remote-open]
upstream_url: null
reproduced_on: null
---

# X35: Secure (password) text entry in Contract

## Summary

T3 Code asks for passwords and secrets in `<input type="password">` fields: the SSH password dialog, provider credentials, Bitbucket tokens, usage-hub keys and write-only headers. exact2's Contract accepts `type="password"` on an `input`, and the clone uses it in four places, but nothing in this plan's sources says what the macOS host does with it: masking, copy protection, secure keyboard input, accessibility value, and whether the agent's `tree`, `state`, `logs` and screenshots can reveal the value. The SSH password dialog needs a field that is certainly secure; the clone plans a native field in its module. What is needed is a defined, tested meaning of `type="password"` on macOS.

## Why this issue arose

### The T3 Code behavior

Reference fields with `type="password"` (all at `1e2ecbd975`):

- SSH password dialog: `apps/web/src/components/desktop/SshPasswordPromptDialog.tsx:150–230` (input at :196). Title "SSH Password Required"; the prompt text from the request; a countdown that becomes "Expired"; `autoComplete="current-password"`, `name="ssh-password"`; disabled while responding or expired; Enter submits the form; Cancel (or "Dismiss" when expired) and Continue; a hint "Use SSH keys to avoid repeated password prompts on new SSH sessions." that becomes the error text on failure. The field is focused and selected when the dialog opens (see `20261005-ssh-password-and-remote-open`).
- Provider and settings secrets: `settings/ProviderAuthenticationSection.tsx:406`, `CodexSetupSection.tsx:675`, `ProviderInstanceCard.tsx:275,414`, `ProviderSettingsForm.tsx:289`, `AcpSessionManagementSection.tsx:553` (write-only headers JSON), `BitbucketCredentialsSettings.tsx:61`, `AddUsageLimitSourceDialog.tsx:123`.
- In Chromium a password field shows bullets, does not allow copy or cut of its text, and (on macOS) turns on the system's secure keyboard input while focused. These are general web-platform behaviors named here for context; this plan did not observe them.
- The password goes to the local `ssh` process only (askpass, in memory) and "is not saved by T3 Code".

### What exact2 does today

Bundled library (`20261005-platforms-v3`): native form controls are "supported", but `type="password"` and secure entry are **not covered: unknown**. `EXACT2-GAPS.md` has no entry for it (this gap was found while writing the plan's tickets on 2026-10-05). Observed in the clone: the Contract compiles `input ... type="password"` (the contract build is OK with it, round 12: 1865 slots). What the host draws and exposes for it on macOS is **not checked**: whether it is an `NSSecureTextField`, whether its value appears in the accessibility tree, the agent's `state`/`tree`/`logs`, or screenshots.

Related facts in the clone: `AGENT-HANDOFF.md` lines 160 and 291 say the real-input state reader redacts "secure values and tokens"; `modules/apple/T3SnapshotAccessibility.swift:73` skips `AXSecureTextField` when capturing accessibility text. Neither says which clone fields are meant.

### Where the clone hits it

- Existing secret inputs in Contract: `providers-wizard.contract:333` (usage-hub "Management key"), `providers.contract:411` (provider environment fields), `settings-a-bitbucket.contract:44` and `:50` ("Access token", "API token"). The plan tickets for provider sign-in and settings reuse this pattern and record "whether that is true secure entry is unchecked".
- The SSH password dialog (`20261005-ssh-password-and-remote-open`) is planned as a native secure field in the app module, in the style of the `t3-key-recorder` view (`T3Module.swift` `views`), because the Contract field's behavior is unknown. The native field must clear its memory on dismiss, refuse copy, and carry the request's prompt as its accessible name. A native view inside a Contract dialog also means the app owns focus order, Enter/Escape handling and IME; the agent can reach it only through the two native input forms that exist (`ExactNativeInput` has `.text` and `.key`, per `EXACT2-GAPS.md` X8).

What would differ for a person if the Contract field is not secure: visible text, copyable text, secrets in agent output and in lane evidence screenshots. If it is secure, nothing differs and the native view is unneeded.

## Why it must be resolved

Parity goal: password entry must behave as in the reference (masked, not copyable, not saved, cleared on dismiss). It is also a safety matter for this plan: the verification rules forbid printing tokens, and agent evidence (`tree`, `state`, `logs`, screenshots) must not carry secrets. Three tickets wait for an answer (in `blocks`); the SSH ticket would otherwise carry a native view whose only reason is this gap. Cost of keeping the workaround: a native secure field plus its tests for the SSH dialog, and an unverified assumption behind every other secret field in the clone.

## Requested support

The web standard is `<input type="password">` (plus `autocomplete` tokens such as `current-password`, `new-password`, `off`). On the macOS host first:

- **A. Define `type="password"` (preferred).** The host draws it as a secure text field: masked entry, no copy or cut, secure keyboard input while focused, accessibility role secure text field with no value exposed, and `autocomplete` passed through so system autofill does not engage unless asked. The value never appears in `tree`, `state`, `logs` or agent screenshots (shown as bullets or redacted). `autofocus` and select-on-focus work on it.
- **B. Document the gap.** If the host cannot do this, document what `type="password"` does on macOS so apps know to use a module view, and keep the native field.

Other hosts: to confirm at `issue-open`.

## How to reproduce

To confirm on the pinned `main` at `issue-open`:

1. Minimal app with one `input type="password" testId="secret"` and a `text` that echoes the bound value.
2. Agent: `type secret hunter2`, then `tree --ax`, `state`, `logs`, `screenshot`. Expected (reference-like): bullets on screen; the value absent or redacted in `tree`, `state` and `logs`; the accessibility role is a secure text field. Actual: unknown.
3. Attended session (macOS): type the value, select all, press ⌘C and paste elsewhere. Expected: nothing is copied. Check with Korean 2-Set (IME) as well; whether macOS restricts input sources in secure fields is to confirm.
4. Clone: open Settings › Source Control (Bitbucket) and the usage-hub dialog and repeat steps 2–3 with a fake token.

## Acceptance for the fix

- Steps 2–3 pass in a minimal app: bullets, no value in agent output, secure text field role, no copy, `autocomplete` honored.
- An AppKit test creates the Contract field and asserts the view is an `NSSecureTextField` (or the documented equivalent).
- The agent's `layout`/`tree --ax` for the field shows the redaction, not the value.
- Conformance against Chrome: the observable web behaviors (masking, `value` readable by the page, `autocomplete`) match where they apply.

## App adoption after resolution

Under A: the SSH password dialog uses the Contract field and drops the native secure view; a test in the SSH ticket checks the dialog's accessible name, Enter/Escape and clearing on dismiss; the four existing fields get the acceptance checks and an attended copy check; lane evidence rules (never print secrets) become checkable with a redaction assertion. Under B: the native-field approach stays and the other fields are checked or replaced. `issue-close` verifies the AppKit test and the attended copy check.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (check the capability on the pin first; if it already exists, record it and close this issue; otherwise reproduce, search for duplicates and prepare the report for the user's approval; publication only after approval).
