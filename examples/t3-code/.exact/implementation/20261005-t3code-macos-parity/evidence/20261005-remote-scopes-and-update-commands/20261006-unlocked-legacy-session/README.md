# Unlocked normal-launch legacy-session verification — 2026-10-06

The existing-session reconnect, message round trip, and restart checks passed on the
native macOS app built from `60a28292df26bda45397934b6598e2f55d5a3053`.
This supersedes the lock blocker for those checks in the earlier attempts. Overall
verification remains open for keyboard focus and the reference desktop oracle/trace.

## Fixture and method

- Used the current native app bundle with a separate bundle ID,
  `com.exact.t3code.scope-review`, separate preferences, and a fixture-only Keychain item.
  Launched its executable normally, without `EXACT_AGENT` or clipboard injection.
- Ran the reference server at source `1e2ecbd975` on loopback port 16140, with an isolated
  data directory. Created a bearer using the old exchange scope request:
  `orchestration:read orchestration:operate review:write`. Seeded the saved environment
  and Keychain using the existing storage format. The old app binary was not run.
- Used a local Codex app-server protocol mock derived from the reference server's
  `codexCollabMockPeer.mjs` / `codexMultiAgentWire.json` fixtures. It implements the
  initialization/model/thread/turn operations and emits `LEGACY_SCOPE_OK` for the submitted
  turn. No external model call was made. The production app and reference server handled
  the actual authentication, WebSocket connection, dispatch, persistence, and rendering.
- Drove the native composer through accessibility controls, then read both the actual
  native UI and the mock's received `turn/start`. Some Orca actions returned a focus error
  after the accessibility action had already taken effect; success is based on the
  resulting UI and server/provider state, not the tool's action status.

## Observations

| Check | Evidence | Result |
| --- | --- | --- |
| Old scope request authenticates | `legacy-session-before.json` | Exactly the three old scopes |
| Native message dispatch | `provider-turn.json`, `legacy-message-tree.txt`, `legacy-message.png` | Prompt received; response rendered |
| Restart uses the stored credential | `legacy-clients-after-relaunch.json` | Session `6a8f4620-4f2c-477e-84d0-d445d53c7c12` connected after restart |
| Restart preserves scopes | Same server client record | Still three scopes; no `terminal:operate` or `relay:read` |
| Restart retains conversation | `legacy-relaunch-tree.txt`, `legacy-relaunch.png` | Prompt and `LEGACY_SCOPE_OK` remain visible |

The current behavior does not upgrade saved sessions. These checks establish existing
chat continuity, not access to newly scoped terminal/relay operations. U12 remains an
unresolved product decision; this evidence does not approve the current policy.

## Remaining checks and incidental observations

Orca reports Accessibility and Screen Recording permissions granted, but cannot establish
the normal window as its focused keyboard recipient. Tab produced no observed focus
change; `--restore-window` then returned `window_not_focused` (`tab-restore.json`).
Activating the app through `NSRunningApplication.activate` made it the frontmost PID but
did not resolve Orca's focused-window check. Manual focus was requested. Space/Enter and
the focus ring are not claimed as passing.

A later Connections screen showed a slow-request notification. Expanding it identified
`vcs.refreshStatus` requests (`slow-requests-tree.txt`), not message dispatch. The fixture
project is nested inside this large worktree; that may contribute, but the cause was not
established. This observation is not treated as a scope regression or a passing VCS test.

The reference desktop oracle and trace prerequisite remain unavailable on this checkout.
No full parity verdict, pixel parity, or external-model behavior is claimed.

## Reproduction outline

1. With the Mac unlocked, start an isolated reference server using its desktop-bootstrap
   token and create/exchange a pairing token requesting only the three old scopes.
2. Store that bearer in the fixture app's Keychain account (`origin + "\n" + environmentId`)
   and seed the fixture preferences' `t3.saved.environments` row.
3. Point only the isolated server's `providers.codex.binaryPath` at the local protocol mock.
4. Launch the normal native bundle, select the mock model, submit the prompt shown in
   `provider-turn.json`, and inspect the response and provider input.
5. Quit and relaunch that bundle. Query `/api/auth/clients` with the fixture owner's token;
   compare session ID, scopes, connected state, and the native conversation.

Temporary recipes remain under ignored `target/rsu-unlocked`; portable evidence here
contains no bearer, pairing credential, or bootstrap secret. These records are local
and have not been published to the PR.
