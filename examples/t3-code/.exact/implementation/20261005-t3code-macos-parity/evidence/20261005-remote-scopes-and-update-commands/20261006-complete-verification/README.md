# macOS verification: failed acceptance

Reviewed implementation: `60a28292df26bda45397934b6598e2f55d5a3053`.
Reference: `1e2ecbd975`. Product source was unchanged throughout this verification.

The update button works with the keyboard, but its focus indicator is not visible.
This fails the ticket's explicit focus-ring requirement. Independent review confirmed
the finding. The task remains active; no implementation was committed or published.

## Results

| Acceptance | Result | Evidence |
| --- | --- | --- |
| Standard five scopes | Pass: native and Electron request the same five scopes in the same order; standard links succeed | `scope-comparison.json`, `scope-trace.ndjson` |
| Read-only link | Pass: both return HTTP 400, show “The environment rejected the authentication request.”, and consume the one-time link | `*-readonly*` |
| Legacy three-scope session | Pass for connection, chat and restart; the same session retains three scopes | [previous unlocked-session capture](../20261006-unlocked-legacy-session/README.md) |
| Installation variants | Pass: seven variants × two sizes × two themes on each app; clipboard and text assertions match | `native-matrix/`, `oracle-matrix-settled/` |
| Desktop-managed server | Pass: explanatory sentence replaces the button | matrix `desktop-*` |
| Pasteboard failure and retry | Pass: forced native write failure shows failure toast; retry copies successfully | [failure capture](../20261006-install-kinds-and-lock-diagnosis/README.md) |
| Keyboard and accessible name | Pass: Tab reaches the control; Space and Return copy the expected command and show the toast | `keyboard-results.json`, `keyboard-Space.png`, `keyboard-Return.png` |
| Visible keyboard focus | **Fail:** AX focus identifies the update control, but the screenshot has no visible focus ring | `keyboard-focus.json`, `keyboard-focus.png` |
| Tooltip | Native tooltip captured in earlier runtime proof; all 24 manual-command oracle cases match their labels | earlier capture and oracle matrix results |
| Message and approval | Pass for the tested flow: same final prompt, approval card, `accept` receipt, and `T0_READY_FOR_APPROVAL` response | `t0-comparison.json`, `provider-*`, `*-t0-*` |
| Full normalized T0 protocol trace | Not established: scoped exchanges and semantic actions match, ancillary request sequences differ; complete payload/result comparison is absent | `scope-trace.ndjson`, independent review |
| Automated gates | Pass after correcting the inherited Bun version | logs below |

The matrix includes `npm-global` with `/opt/scope fixture's node`, `npx`, `pnpm-dlx`,
`bunx`, absent installation, unknown future kind, and desktop-managed installation.
Window sizes are 1280×840 and 840×620, in light and dark themes: 28 captures per app.
The descriptor proxy changes only installation/version/capability fixture fields.
Both applications otherwise use the same live reference server. The earlier attempt
also launched real npm/npx/pnpm/bunx installation layouts.

Comparison normalizes only the fixture's server label and displayed server version.
Commands, accessible labels, toast text and desktop-managed wording match. Whole-window
pixel equality is not claimed: primary panels, saved names and toast history differ.
The settled oracle capture replaces an earlier, retained local capture taken during
toast animation. No remote update RPC appears in the captured command-copy trace.

## Focus failure reproduction

1. Launch the normal native app with the isolated saved remote environment.
2. Open Settings → Connections with a server that requires a manual update.
3. Focus the settings search field, then press Tab through the settings controls.
4. Stop when AX focus names “Copy relaunch command for … server”.
5. Inspect the button: no focus outline appears. Press Space or Return: copying works.

Expected: a visible focus indicator, as required by the ticket. Observed: keyboard
focus and activation work without that visual indication. The cause is unproven;
the shared contract styling and native host focus rendering are both relevant to
investigation. This report does not assign the defect to either layer.

## Runtime identity and fixture limitations

macOS 26.6.2 arm64; Xcode 27.0 (27A266a); Bun 1.4.2; Cargo 1.97.0.
The native app is a normal launch without `EXACT_AGENT`, using an isolated bundle ID
and preferences. The binary came from the current source build. Electron 44.4.2 runs
the actual reference desktop/web code with isolated home/app-data paths and mock
keychain storage. Ports: server 16140, descriptor proxy 16158, oracle 16155/16156.

The oracle workspace build first failed because `typescript-legacy` was unavailable.
The web build succeeded; the desktop build then succeeded separately. The run uses
the previously source-built server distribution at the same reference pin. Preserve
`reference-build.log` and `desktop-build.log`; this is not a successful full reference
workspace build claim.

A local deterministic Codex protocol fixture emits the response and approval request.
No external LLM or shell command runs. Two initial fixture approval requests failed
schema decoding. Adding the required timestamp and restarting the backend refreshed
the cached fixture process. The final oracle approval and two native approvals return
`accept`; the first native prompt was accidentally duplicated because an input tool
reported failure after applying input. The last native run uses the exact oracle
prompt and is the comparison run. Failed attempts remain in the provider receipt log.

The proxy omits credentials and records RPC method names, command types, decisions
and payload keys, not complete payloads. Native and oracle differ in background
subscriptions, probes, VCS traffic and `thread.visit`. This proves the scoped pairing
criterion and tested message/approval behavior, not full protocol equivalence.

The existing-session fixture uses the pre-change scope request and saved-store format;
it was not minted by running an old executable. U12 remains an unanswered product
decision: current behavior leaves existing sessions unchanged, without a notice.
Merged-task prerequisites recorded in the ticket also remain unresolved.

## Automated checks

Run with the pinned Bun directory first on `PATH` (Cargo tests spawn Bun):

```sh
bun test examples/t3-code
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2020 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2020,DOM examples/t3-code/app.ts
cargo build --all-targets --keep-going
cargo test --lib --bins --tests --no-fail-fast
cargo clippy --all-targets --keep-going -- -D warnings
cargo fmt --all -- --check
bun scripts/caps.mjs
bun scripts/boot.mjs
cargo run -q -p contract -- build examples/t3-code/app.contract -o target/rsu-complete/t3-code.plan
EXACT_APP_DIR="$PWD/examples/t3-code" cargo test -p t3-code-macos --lib
T3_APP_DIR="$PWD/examples/t3-code" target/t3-tests/transport/transport-tests
```

Results: TS 1216 pass; root Rust 2928 pass, 18 ignored; app Rust 10 pass;
AppKit transport 40 tests, zero failures, two live skips. Build, strict typecheck,
clippy, formatting, caps, boot and Contract build pass. Initial root/app Rust attempts
inherited Bun 1.3.14 and failed; corrected `*-pinned-bun.log` runs pass. Original
failed logs and `gates-results.json` are preserved, not overwritten as successful.

The final runner checks the recorded scope/matrix assertions, source identity and
acceptance observations. It does not rerun the expensive gates above. Its failure
records the observed focus defect; unavailable prerequisites remain explicit.

Test app/server/proxy processes were stopped; fixture Keychain entries and isolated
preferences were removed (`cleanup.json`). The real `~/.t3` directory mtime matches
its pre-oracle value; this is a directory-level check, not a recursive file audit.

Portable logs trim trailing whitespace only; raw command output remains in the ignored local attempt directory.
