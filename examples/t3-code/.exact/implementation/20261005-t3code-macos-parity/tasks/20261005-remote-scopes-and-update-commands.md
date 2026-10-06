---
name: 20261005-remote-scopes-and-update-commands
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Remote pairing asks for the standard scopes; update commands match the install

## Outcome

A remote environment paired after this ticket holds the reference's five standard scopes, so terminals and relay reads are
authorized. For a remote `t3` that cannot update itself, the update button copies the command that matches how that `t3` was installed
(`npm-global`, `npx`, `pnpm dlx`, `bunx`) and the toast says what the command does. A host whose server belongs to a desktop app that
cannot update says so in text instead of showing nothing.

## Scope and exclusions

Included (reference commit `3b0093d716` and the standard-scope rule; clone rows partial or missing):

1. **Scopes.** Add `AUTH_STANDARD_CLIENT_SCOPES` (port of `AuthStandardClientScopes`, `packages/contracts/src/auth.ts:103-109`: orchestration:read,
   orchestration:operate, terminal:operate, review:write, relay:read). TS passes it to Swift as the `scope` of every remote token exchange, space-separated
   (`encodeOAuthScope`, `packages/shared/src/oauthScope.ts`, used at `packages/client-runtime/src/authorization/remote.ts:107`): pairing and re-exchange in `T3Transport.swift` (the form body with
   `"scope": "orchestration:read orchestration:operate review:write"`, two places) and `T3Fleet.swift` (one place). The SSH path reaches the same exchange.
   The embedded primary omits `scope` and receives all eight (`20261005-embedded-server-runtime`). The scopes a session was granted (the token response `scope`)
   stay in `source.scopes` (`connections.ts`), which gates the existing "cannot change its settings" lock and the terminal error.
2. **Narrow links.** The server grants only what the link carries and answers `scope_not_granted` when a requested scope is missing
   (`apps/server/src/auth/EnvironmentAuth.ts:809-812`; code list `packages/contracts/src/environmentHttp.ts:77-78`). The reference client asks for the standard
   five whatever the link says (`apps/web/src/connection/platform.ts:230`), so a "Read only" link fails for it too. Match that; never retry with fewer scopes. The failed exchange also spends the one-time link: the server consumes the credential (`apps/server/src/auth/PairingGrantStore.ts:512-527`) before it checks scopes, so a new link is needed.
   The error text the reference shows is unknown: read it from the oracle by redeeming the same link there. The fixture creates the Read only link itself, through the lane server's own pairing HTTP API:
   a lane tool (a script under `target/t3-ui-parity/`, not app code) exchanges the lane's owner credential for a bearer that has `access:write` (`POST /oauth/token`), then calls
   `POST /api/auth/pairing-token` with `{"label": "read-only", "scopes": ["orchestration:read"]}` (`packages/contracts/src/environmentHttp.ts:466-470`, `AuthCreatePairingCredentialInput`); how the lane backend exposes its
   owner credential is read from `lane-backend.sh` at `prepare`. This ticket does not need the Network access and pairing-link screens of `20261005-this-machine-network-access`.
3. **Sessions paired before this change (decision U12).** They keep working with three scopes and lack `terminal:operate` and `relay:read`. Default (recommended):
   leave them; `20261005-terminal-drawer` shows the reference's authorization error for them. Option: a one-time notice per environment ("<label> was paired with fewer
   permissions. Pair it again to use terminals.", no reference wording, so it is an addition the user must approve) with an action that opens the "Add route" dialog of
   `20261005-environment-routes`, which re-pairs without a duplicate row.
4. **Install-aware command (A8).** Port `ServerInstallation` (`packages/contracts/src/environment.ts`, a union of `{kind: npx|pnpm-dlx|bunx}` and `{kind: npm-global, prefix}`; an unknown
   future kind decodes to absent, `ForwardCompatibleOptional`) and `manualServerUpdateCommand(targetVersion, installation)` (`apps/web/src/versionSkew.ts`): npm-global gives
   `npm install --global --prefix '<prefix>' t3@<ver>` with `'` written as `'\''`; `pnpm-dlx` gives `pnpm dlx t3@<ver>`; `bunx` gives `bunx t3@<ver>`; everything else `npx t3@<ver>`.
   The capability appears only when the server cannot update itself (`serverSelfUpdate` absent, `apps/server/src/environment/ServerEnvironment.ts:200`).
5. **Strings (`ServerUpdateAction.tsx`).** Button label and tooltip: "Copy update command" for `npm-global`, "Copy relaunch command" for anything else; the icon form's `aria-label`
   is `<label> for <server label>`. Success toast for `npm-global`: "Update command copied" / "Run `<command>` on <server label>, then restart t3 with your usual options."; otherwise
   "Relaunch command copied" / "Stop t3 on <server label>, then relaunch with `<command>` using the same subcommand and options. This does not update an installed t3 command."
   Failure toast "Could not copy update command" with the error message. A `desktop-managed` server without `desktopAppUpdate` shows the text "Update the desktop app on that machine
   to update this server." where the button would be (the clone hides the button, `connections.ts:174`). Replace the fixed `npx t3@${CLIENT_VERSION}` and its toast
   (`connections.ts:400-402`) and the update-all and outdated-host paths (`settings-b-outdated.ts`).
6. **Exports.** The command function and the string helpers are plain TS functions that `20261005-server-update-banner` calls from the composer banner and the
   details card (the reference changed `ChatView.tsx` and `useAutoBalanceUpdateBanner.tsx` the same way).

Excluded: the server update banner (`20261005-server-update-banner`), Auto balance (`20261005-auto-balance`), server-side `resolveServerInstallation` (the server's own code), routes (`20261005-environment-routes`).

## Context and guidance

Parent specification: [spec](../spec.md). Reference at `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): the trace diff is `target/t3-ui-parity/trace-diff.mjs`, the oracle `target/t3-ui-parity/electron-oracle.mjs`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (persistence awaited on macOS), accessibility (icon button names, keyboard activation), design (states),
testing-and-debugging. Pasteboard writes and the Swift transport are **unknown in the library**; the clone's runtime evidence on the pinned main is the basis.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`).
Port changes for file headers: `Effect` services become plain functions; `useCopyToClipboard` becomes the clone's `copyText` op (`T3Module.swift`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| recorded decision | U12 (sessions with three scopes) | none | Answered at `prepare` | pending |
| conditional merged task PR | [20261005-environment-routes](20261005-environment-routes.md) | pending | Only if U12 chooses the re-pair notice | pending |
| scheduling preference | Either order with `20261005-environment-routes` | none | Both edit `connections.ts` and the pairing code in `T3Transport.swift` | pending |

`20261005-terminal-drawer` and `20261005-server-update-banner` depend on this ticket.

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../issues/20261005-x21-two-way-websocket.md) | Native transport carries the exchange | existing | nonblocking | none |

## Implementation notes

- The constant lives once in TS; Swift never hard-codes scope names.
- Keep the old three-scope string only as a test fixture of a pre-change session.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Ported tests | — | `bun test`: "updates only the proven npm prefix and safely quotes its path", "keeps runner and unknown commands as relaunches", "copies an honest manual command for %j without invoking remote update" (three kinds), "decodes old, recognized and future manual installation descriptors" | Original names pass | macOS | log |
| Five scopes | Fixture backend on a lane port; scenario T0 on the oracle and the clone | `target/t3-ui-parity/trace-diff.mjs T0` | `POST /oauth/token` has the five scopes, in the oracle's order; no credential in the trace | macOS | ndjson diff |
| Terminal scope granted | Pair a fixture backend with a standard link | `state` of the environment | `source.scopes` has `terminal:operate` | macOS | `--json` state |
| Narrow link | A Read only link created by the lane tool through `POST /api/auth/pairing-token` with `scopes: ["orchestration:read"]` on the lane server; the same link redeemed on the oracle first to record its text | Pair with the link in the app | Refused with the oracle's text; nothing saved; the link is gone from `GET /api/auth/pairing-links` after the refusal (the server spends it first); the oracle's behavior is recorded first and matched | macOS | transcript, HTTP log |
| Three-scope session | A saved environment from a pre-change build (fixture store) | Launch; `state`; send a message in a thread of that environment | It connects and works with three scopes (`source.scopes` lacks `terminal:operate` and `relay:read`); U12 outcome: untouched, or the one-time notice with its action. The terminal authorization error for such a session is asserted in `20261005-terminal-drawer` | macOS | state, screenshot |
| Update command per kind | Config objects for npm-global, npx, pnpm-dlx, bunx, absent and unknown-future | Press the update control in the row | Label, pasteboard text and toast exactly as listed; no remote update call | macOS 1280×840, 840×620, light and dark | `pbpaste`, screenshot pairs vs oracle |
| Desktop-managed text | `serverSelfUpdate: "desktop-managed"` without `desktopAppUpdate` | Open Settings > Connections | The sentence replaces the button; no button | macOS | tree, png |
| Failure and states | Pasteboard write forced to fail; Tab to the control; hover for the tooltip | Press Space and Enter; hover | "Could not copy update command" toast; focus ring visible; tooltip text equals the label; `aria-label` as listed; no motion to reduce | macOS | tree `--ax`, screenshots |
| Standard gates | `git add -A` | Clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `transport` AppKit binary); `bun scripts/caps.mjs`; the repository's five checks | All pass; every moved matrix cell is fixed or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `server-installation.ts` (+ tests), `connections.ts`, `settings-b-outdated.ts`, `modules/apple/{T3Transport,T3Fleet}.swift`, `macos/tests/transport`.
Required environment: Xcode 27.0, pinned Bun, reference oracle runtime, lane ports 16000–16999.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the three merged task PRs; ask U12 first. Close with clone checks green (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, `transport` AppKit binary), `bun scripts/caps.mjs` after `git add -A`, the repository's five checks, and every moved matrix cell fixed or declared in `EXACT2-GAPS.md` with an issue link.
