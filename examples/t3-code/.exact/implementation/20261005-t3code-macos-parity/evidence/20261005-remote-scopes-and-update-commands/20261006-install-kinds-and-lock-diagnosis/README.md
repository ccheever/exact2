# Remaining macOS checks, 2026-10-06

This attempt adds live evidence to the earlier macOS runtime review. Application source remains at `60a28292df26bda45397934b6598e2f55d5a3053`. It does not complete the ticket.

## Results

| Check | Observation |
|---|---|
| npm-global | Native update control copies npm install with the exact advertised prefix. A prefix containing a space and apostrophe is quoted correctly; shell tokenization reproduces the original prefix. Label and success toast match. |
| pnpm-dlx | Native control copies `pnpm dlx t3@0.0.46-nightly.20261004.1`; relaunch label/toast match. |
| bunx | Native control copies `bunx t3@0.0.46-nightly.20261004.1`; relaunch label/toast match. |
| npx | Native control copies `npx t3@0.0.46-nightly.20261004.1`; relaunch label/toast match. |
| Pasteboard failure | A process-local test library makes the first npm update command write return NO from NSPasteboard.setString. Actual native button press produces `Could not copy update command` / `Failed to copy update command to the clipboard.` |
| Retry after failure | Second native button press succeeds, the pasteboard contains the expected command, success toast appears, and inline errors clear. |
| Small dark window | 840×620 native capture inspected. No pixel-parity claim. |
| Legacy saved session | Still blocked. Sampling shows T3Credentials.read waiting inside SecItemCopyMatching. OS reports IOConsoleLocked=Yes and CGSSessionScreenIsLocked=Yes. User cannot unlock at this time. |
| Normal keyboard / accessibility | Blocked by the locked interactive desktop. Orca returns permission_denied for AX window inspection even though permissions report granted. |

The first install drive passed npm-global but the next Add environment action did not expose its input while a toast/hover remained. Its failure log is preserved. The second drive dismissed toasts and moved the pointer away before opening the next dialog; all four cases passed. No app source was changed to obtain these passes.

## Fixtures and reproduction

Use the already-built source from the preceding attempt with `EXACT_APP_DIR` pointing at examples/t3-code and Bun 1.4.2. The supported native driver keeps its build-receipt check enabled.

Reference server pin: 1e2ecbd975. Four copies of its built runtime were placed in recognized package layouts under ignored target/rsu-followup, with reference node_modules linked read-only. The npm copy had a matching package.json and bin/t3 symlink; other copies lived under pnpm/dlx, bunx-fixture and _npx directories. Server installation descriptors were asserted before driving the client. This tests actual advertised capabilities, not a stubbed client config. Package-manager install/update commands were never executed.

Ports: 16142 npm-global, 16143 pnpm-dlx, 16144 bunx, 16145 npx. Every server used a separate data directory and one-time owner secret. New standard pairing links were redeemed through native onboarding / Settings > Connections. For each row the drive hovered the control, checked the label, pressed it, read pbpaste, asserted the toast, and captured the native window. Clipboard contents were restored after each drive.

The failure test used a temporary Objective-C dylib loaded only into the agent test process with DYLD_INSERT_LIBRARIES. It swizzled NSPasteboard.setString:forType: and returned NO once for text beginning with `npm install --global --prefix ` and containing ` t3@`; subsequent calls invoked the original implementation. The refusal marker is present in clipboard-fault-logs.json. Production Swift, TypeScript and built app files were unchanged. The failure screenshot also shows the same error twice inline; reference comparison is still required before claiming exact failure-state fidelity.

The normal-launch legacy fixture used the old three-scope token exchange and an isolated app preference domain. A fresh local-only test token with no trusted-app prompt still blocked while the Mac was locked, correcting the earlier assumption that token ACL configuration alone explained the wait. No attempt was made to unlock the Mac or alter user security settings. The copied app, fixture Keychain item, preferences and all six test servers were cleaned up by recorded identity/PID. No PR metadata or source code was changed.

Local fixture tooling remains under ignored target/rsu-review and target/rsu-followup. Screenshot/report artifacts here contain no access tokens or pairing credentials.

## Resume when the desktop is unlocked

1. Confirm IOConsoleLocked=No and CGSSessionScreenIsLocked=No before starting normal-window checks.
2. Restart the isolated reference fixture, mint a fresh three-scope token, and seed only the isolated review app's saved environment and Keychain account. Never use the user's saved credentials.
3. Launch the current app normally. Require the reference server to report that exact legacy session connected, still with three scopes. Capture its Connections row and session record.
4. Send a deterministic test message through a fixture provider, verify a persisted response, quit and relaunch, and confirm the saved session reconnects. A server-issued token by itself is not this proof.
5. Tab to the update control, activate with Space and Enter, inspect focus and resulting clipboard/toasts using the visible window. Restore clipboard and remove fixture credentials afterward.
6. Once the desktop-oracle-and-trace prerequisite exists, redeem a separate equivalent read-only link on the reference desktop app, compare rejection text and T0 exchange traces. Each one-time link must be newly minted; do not reuse a spent link.
7. Record U12 explicitly. Current code leaves old scopes unchanged and adds no notice; this attempt does not approve that choice.

Existing-session behavior, keyboard fidelity and reference comparison remain unverified. Overall task stays unverified and must not be represented as fully passed.
