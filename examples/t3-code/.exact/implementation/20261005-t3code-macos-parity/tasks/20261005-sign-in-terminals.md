---
name: 20261005-sign-in-terminals
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: open
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-terminal-drawer
pr_url: https://github.com/ccheever/exact2/pull/175
verified_commit: null
---

# Provider sign-in terminals and the onboarding install terminal

## Outcome

Two places show an inline terminal like the reference does. (D2) In Settings › Providers, a
provider sign-in that uses a terminal shows a 256 pt terminal in the Account row. The user types
into the provider's login program and the app sends the input to the server. (TN1) In the
welcome wizard, "Install" and "Sign in" on an agent card open an inline setup terminal. It types
the install or login command without pressing Enter. The user reviews the command and runs it.

## Scope and exclusions

Included:

1. **D2 terminal branch** of the Account row (`interaction.type === "terminal"`): the terminal component,
   the delta-write of the transcript, the input queue in slices of 4,096 chars, the resize message, link
   handling, and every message and state below. The row, `provider.auth.*` calls and the other
   interactions (browser, device code, credentials) come from `20261005-provider-sign-in-and-install`.
2. **TN1 onboarding terminal**: the "Install" / "Sign in" button on the agent card (terminal icon), the inline
   terminal under the cards, the command resolvers, cleanup. This includes the Codex "Use existing CLI" branch
   (the generic card with the terminal button, `WelcomeWizard.tsx:830-915`); the managed ChatGPT branch is
   `20261005-managed-codex-chatgpt`.
3. **Port of** `resolveOnboardingProviderLoginCommand` and `resolveOnboardingProviderInstallCommand`
   with their tests.

Excluded: sign-in start/cancel/logout, browser and device-code flows (`20261005-provider-sign-in-and-install`);
the server's PTY and the ACP agent (server side, not re-implemented); the ACP wizard steps.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior (T3 Code `1e2ecbd975`):
`apps/web/src/components/settings/ProviderAuthTerminal.tsx:1-80`; `ProviderAuthenticationSection.tsx:78-86,323-390`;
`packages/contracts/src/providerSetup.ts:36-95` (terminal interaction `output` ≤ 16,384 chars with `outputOffset`; response
`data` ≤ 4,096, `size` cols 1–500, rows 1–200); `apps/server/src/provider/acp/AcpRegistryAuth.ts:67,120,159-221` (80×24 PTY,
64-chunk sliding queue; the method is offered only when a PTY exists); `apps/web/src/components/onboarding/WelcomeWizard.tsx:104,634-650,735-790,915-1130`;
`apps/web/src/onboarding/providerReadiness.logic.ts:60-131`. Facts to keep. The D2 terminal does not use `terminal.*` RPCs.
Input goes through `provider.auth.respond`, one request at a time, in order (`ProviderAuthenticationSection.tsx:355-387`). A failed request
clears the queue and shows an error. Font size is 13, not a setting. `beforeKey` returns false for Tab, so Tab does not type.
Only `http(s)://` links open, in the system browser; text selection menus do not exist here. TN1 uses `terminal.open/write/attach/close`
with thread `onboarding-agent-setup` and terminal id `onboarding-<driver>-<uuid>`, cwd `serverConfig.cwd`, and the provider instance id.
Cleanup runs in one serial queue with a generation counter, so a cancelled open never closes or types into its replacement.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (queue, late replies), accessibility (labels, focus),
design (complete states), testing-and-debugging. Native views and web views are unknown in the library; the vendored surface from
`20261005-terminal-surface` is the basis. Consumer framework revision: the `main` pin of `20261005-clone-on-exact2-main`.
Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their
`target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`, `trace-diff.mjs`, `lane-backend.sh`. Clone state: the welcome card shows only a badge today (`pages-welcome.contract:327-346`,
`pages-welcome.ts:47-81`); there is no `provider.auth.*` caller.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-terminal-drawer](20261005-terminal-drawer.md) | pending | Merged (surface, bridge, terminal RPC client) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (needed for D2 only; TN1 does not need it) | pending |

## Issue assessment at preparation

Checked sources and time: plan issue drafts in [issues](../issues/README.md), 2026-10-05; not reproduced, not searched upstream. No prior attempt.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X8](../issues/20261005-x08-agent-pointer-native-views.md) | Pointer input for native views | `EXACT2-GAPS.md` X8 | nonblocking (workaround: `(attended session)`) | Link click is attended |
| [X25](../issues/20261005-x25-keyboard-keyup-code-capture.md) | Key facts for the web view | `EXACT2-GAPS.md` X25 | nonblocking | Reuse the key path from `20261005-terminal-surface` |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Two-way WebSocket | LLP 1016.000 | nonblocking (workaround: Swift transport) | Reuse the drawer's session client for TN1 |
| PTY in the embedded runtime | `AcpRegistryAuth.ts:67,120` offers a terminal method only with a PTY | Unknown for the embedded runtime | unknown | Check after `20261005-embedded-server-runtime`; the fixture runtime has a PTY |
| Fixture ACP agent with a `terminal` method | A fake agent for the lane | Not yet built | unknown | Confirm at prepare; else D2 rows use a fake transport and the live row stays unverified |

## Implementation notes

- D2 component: `provider-auth-terminal.ts` plus the hook box. Create the view only when the interaction type is
  `terminal`; key it by `flowId:interaction.id` so a new interaction gets a new view. Box: 256 pt high, `rounded-md`, 1 pt
  `border-border`, `aria-label="Provider sign-in terminal"`.
- Transcript algorithm (`ProviderAuthTerminal.tsx:22-30`): `delta = outputOffset - written`; if `0 < delta ≤ output.length`
  write `output.slice(-delta)`; else if `delta ≠ 0` reset and write all; then `written = outputOffset`. Make it a pure function
  `terminalTranscriptUpdate`. The reference has no test for it: add tests named after its three cases.
- Input: split `data` into slices of 4,096 chars (at least one slice, so a resize with `data:""` still sends). Keep one queue; send in order.
  With `readOnly` or no flow id, send nothing. Resize sends `{type:"terminal", data:"", size:{cols,rows}}`.
- Messages: Account description "Complete sign-in in the terminal below."; errors "The provider sign-in terminal is no longer available.",
  "Could not send input to the provider sign-in terminal.", "Could not open the provider link.", "Could not load the sign-in terminal.
  Cancel and retry sign-in."; fallback "Loading sign-in terminal…" while the view loads.
- TN1 button: `xs` ghost, terminal icon, label "Install" or "Sign in", disabled while that driver's terminal is open or the server
  config is missing (`WelcomeWizard.tsx:960-972`). Inline panel: header strip with the status text, optional "Retry" (open failed), "Close";
  body 256 pt. Status texts: "Preparing command...", "Review the command, then press Enter to run it.", "Run `<command>` in this terminal."
  (write failed), "Could not open the setup terminal.". The shell exit closes the panel. Closing closes the session with `deleteHistory:true`.
- Commands (mac server): Claude install `curl -fsSL https://claude.ai/install.sh | bash`, Codex install
  `curl -fsSL https://chatgpt.com/codex/install.sh | sh` (the Windows variants stay in the port); login `<binary> auth login` (Claude),
  `<binary> login` (Codex), with the quoting cases of `providerReadiness.logic.test.ts` (`:148`, `:320`).
- States. Loading: "Preparing command..." / "Loading sign-in terminal…". Empty: none. Error: strings above, `role="alert"` for the D2 error.
  Disabled: TN1 button states above; D2 read-only sends nothing. Hover and keyboard focus: buttons use the clone's styles; the terminal takes
  focus when it opens (`autoFocus`). Permission: `terminal:operate` for TN1 (TN2); a missing scope shows the open-failed state. Icon buttons have an `aria-label`.
  Escape and Tab: the D2 terminal sends Escape to the program and does not encode Tab (focus moves on); this ticket adds no dialog or menu (the
  sign-out confirm and the method select belong to `20261005-provider-sign-in-and-install`). Motion: none beyond the cursor blink (steady under reduced motion).

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test examples/t3-code` | The login/install command tests pass with original names; transcript tests pass | macOS | log |
| D2 flow | Lane backend with a fake ACP agent that offers a `terminal` method (or fake transport) | Settings › Providers › agent › Sign in | Terminal shows the program's prompt; trace shows `provider.auth.start`, then `respond` with `size`, then input in order | macOS 1280×840 | `--json`, trace diff |
| Input slices | Same | Agent `type` 10,000 chars | Three `respond` calls of 4,096, 4,096 and 1,808 chars, in order; no reorder | macOS | trace |
| Failure paths | Fake transport fails the second slice | Type | Queue cleared; message "The provider sign-in terminal is no longer available." | macOS | unit test, shot |
| Read-only and Tab | Read-only environment | Type; press Tab | Nothing is sent; Tab moves focus and sends no byte | macOS | trace |
| D2 pair | Same | Screenshot the Account row with the terminal | Pair with the oracle at 1280×840 and 840×620, light and dark | macOS | pairs |
| Link `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`, normal launch; program prints an `https://` URL | Click it | System browser opens that URL; a non-http link does nothing | macOS | notes |
| TN1 install and sign in | Welcome wizard; fixture providers: one missing, one unauthenticated | Click "Install", then "Sign in" | The command text appears in the shell without running; trace `terminal.write` has the command without `\r`; no process starts | macOS both sizes | trace, `state` |
| TN1 run `(attended session)` | Same lane build, normal launch, harmless command via settings override | Press Enter; type `exit` | Command runs; exit closes the panel | macOS | notes |
| TN1 cleanup | Same | Close; switch card; fail open (bad cwd) | `terminal.close` with `deleteHistory:true`; no write into a replaced session; "Retry" works | macOS | trace, unit test |
| TN1 pair | Same | Screenshot the wizard with the panel | Pair with the oracle, light and dark, both sizes | macOS | pairs |
| Codex existing CLI | Codex provider, "Use existing CLI" chosen | Open the step | Generic card with the terminal button | macOS | shot |
| Standard gates | `git add -A` | Clone checks, `bun scripts/caps.mjs`, five repository checks | Green; every moved matrix cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `provider-auth-terminal.ts`, `onboarding-terminal.ts`, `pages-welcome.ts`, `pages-welcome.contract`,
the Providers Account row files from `20261005-provider-sign-in-and-install`, `modules/apple/T3Terminal*.swift`, `macos/tests/terminal/**`, tests.
Required environment: lane backend with a PTY, oracle build, Xcode 27.0, Bun 1.4.2; lane builds set `T3_LOCAL_HOME=<lane>/t3-home` and
`T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773).

## Progress

Implemented in the terminal parity branch alongside the terminal surface and drawer. The provider-auth prerequisite has no remote PR as of 2026-10-06; this change supplies its terminal Account branch (subscribe/start/respond/cancel), with the other authentication interaction types still outside this task.

`onboarding-terminal.ts` owns command resolution and serialized generation-safe setup/cleanup; `pages-welcome.ts` owns one controller per environment and closes sessions when leaving Agents. `provider-auth-terminal.ts` owns transcript deltas and live Account state. `T3TerminalAuth.swift` serializes input through the native transport in 4,096-UTF-16-unit slices, preserving surrogate pairs and fencing identity/read-only changes. Native auth mode is supplied by `T3TerminalView.swift`; it skips terminal session RPCs and opens only HTTP(S) links.

Development checks: 48 focused tests pass across onboarding terminal, provider auth terminal, welcome wizard, and provider settings, including original command resolver cases and the integrated welcome open/leave cleanup path. Strict TypeScript compilation of `app.ts` passes with ES2022. Real harmless PTY fixture passes through the production Swift transport and pinned backend: terminal-auth subscribe/start/respond/cancel, ordered 4,096/4,096/1,808-byte input, 93×31 resize, read-only suppression, second-slice real server refusal clearing the queue, and onboarding pretype with no shell effect until explicit Enter. Evidence: `target/terminal-parity/auth/run-1.log` and `rpc-trace.json` (13 assertions). UI acceptance, screenshots, and the integrated native build remain unverified. Selected guidance remains state-and-data, accessibility, design, and testing-and-debugging at `20261005-platforms-v3`.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

Review the Account and onboarding runtime evidence below, retaining the matrix’s untested subcases. No real account login or installer execution was required for these terminal checks.

## Integrated verification, 2026-10-07

The actual macOS Account terminal passed separate OS key events for `abc`, exact 10,000-character native Cmd+V, Escape forwarded as byte `1b` with Account still open, Cancel unmounting the terminal, and successful retry. Physical Tab moved to Registry agent ID; Shift+Tab moved to Cancel, with zero PTY input bytes. No real authentication service or credentials were used. The final Tab repair also passed an actual WKWebView regression for stale key-view links and no double advancement.

The earlier JavaScript queue lost superseded requests during real continuous input. It was replaced by the native serial queue; the failed evidence remains distinct from the repaired results. Native auth tests cover ordering, failure clearing/recovery, identity fencing and read-only suppression. Evidence: [auth runtime proof](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/auth/sanitized-runtime-proof.json) and [Tab report](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/auth/gui-tab/RESULTS.md). Both actual onboarding Install buttons are now driven: Codex and Claude prefill the exact command without CR/LF, wait for Enter, then Close removes the terminal and its shell. No Enter was sent. Destination PTY byte proof and screenshots are in the [onboarding report](https://github.com/ccheever/exact2/blob/t3-code-evidence/evidence/terminal-drawer/20261007-expanded-parity/onboarding/RESULTS.md).
