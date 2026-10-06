# macOS runtime review, 2026-10-06

Source: `60a28292df26bda45397934b6598e2f55d5a3053`. App implementation unchanged. macOS 26.6.2 (25G83), arm64; Bun 1.4.2.

The PR fixes new pairings. Existing-session handling is explicitly included as decision U12 in the task, not excluded. The implementation selected the leave-as-is default without recording the user's decision. This review does not approve that choice or add a notice.

## Observed results

| Check | Result | Evidence |
|---|---|---|
| Build current macOS bundle | Pass | build.log |
| Read-only link in native onboarding | Refused with `The environment rejected the authentication request.`; no environment saved | readonly-tree.json, assertions.json |
| Standard link in native onboarding | Connected; server reports exactly the standard five scopes, including terminal:operate and relay:read | clients-web.json, assertions.json |
| Manual update control | Real native press; pasteboard equals `npx t3@0.0.46-nightly.20261004.1`; relaunch toast rendered | clipboard.json, command-copied.png |
| Desktop-managed server | Connected row shows the desktop-app update sentence and has no update button | connections.json, command-copied.png |
| Related TS tests | 29 passed, 0 failed, 165 assertions | unit-tests.log |
| Existing three-scope session, normal launch | Blocked in fixture Keychain read; not passed | keychain-stack.txt, legacy-normal.png, legacy-clients-after-launch.json |
| Keyboard activation / focus | Blocked for normal window observation; AX tool returned permission_denied although both permissions reported granted | legacy-normal-ui-retry.json |

## Reproduction and boundaries

Build from repository root:

```sh
EXACT_APP_DIR="$PWD/examples/t3-code" ~/.bun-1.4.2/bin/bun host/apple/build.mjs t3-code-macos --bundle
~/.bun-1.4.2/bin/bun test examples/t3-code/server-installation.test.ts examples/t3-code/connections.test.ts examples/t3-code/settings-b-ssh.test.ts
```

Native drive used the supported `open`, `tap`, `type`, `clock`, `tree`, `state`, `layout` and `screenshot` methods from scripts/agent.mjs. A fresh agent session at 1280×840 began in onboarding. It redeemed a read-only link, then a fresh five-scope link, completed onboarding, opened Settings > Connections, paired the desktop server, hovered and pressed the manual-update control, and read pbpaste. The clipboard was restored afterward. The driver's build-receipt freshness check was active; EXACT_MAC_BIN was not overridden.

Servers were isolated loopback reference instances at ports 16140 (web mode) and 16141 (desktop mode), source pin 1e2ecbd975, runtime server version 0.0.45. Each used a task-local data directory and owner bootstrap secret. No installation descriptor was advertised, so this drive verifies the npx fallback, not npm-global/pnpm-dlx/bunx. Those other branches only have unit-test evidence here. No updater command was executed.

For the legacy case, the reference server minted a link with the old three scopes and exchanged it using the old three-scope request. The resulting session was seeded as a saved environment into an isolated, newly signed copy of the just-built app, bundle ID com.exact.t3code.scope-review. Production source was not changed. The app was launched normally, without EXACT_AGENT. Initial fixture setup used a trailing slash in the Keychain account; this was corrected to match T3Endpoint normalization. The corrected run remained inside T3Credentials.read → SecItemCopyMatching, as sampled from its recorded PID. The server reported no connected legacy session. This is an unresolved fixture/security interaction, not evidence that legacy app sessions work, and not a demonstrated PR regression.

The normal window's screenshot was captured with screencapture using the exact window ID returned by Orca. AX inspection was attempted twice and remained blocked. No terminal action, message-send check, oracle trace comparison, forced pasteboard failure, alternate install-kind live run, or pixel parity sweep passed in this attempt.

Test servers and the copied app were stopped by recorded PID. Both fixture Keychain entries and the copied app's preference domain were deleted. User app preferences and credentials were not changed. No source code, commit, PR comment or remote metadata was changed by this review. Evidence is local and unpublished.

Required next evidence: a saved three-scope session that can be read from Keychain in normal launch, successful reconnect and message send, plus the outstanding task matrix. U12 remains an open product decision. Overall verification remains blocked; the passing rows above do not close the task.
