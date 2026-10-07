---
name: 20261005-remote-scopes-and-update-commands
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: passed
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-remote-scopes-and-update-commands
pr_url: https://github.com/ccheever/exact2/pull/142
verified_commit: 759779342dd34fa516bec36924923e6beeb4c23a
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

Parent specification: [spec](../../spec.md). Reference at `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): the trace diff is `target/t3-ui-parity/trace-diff.mjs`, the oracle `target/t3-ui-parity/electron-oracle.mjs`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (persistence awaited on macOS), accessibility (icon button names, keyboard activation), design (states),
testing-and-debugging. Pasteboard writes and the Swift transport are **unknown in the library**; the clone's runtime evidence on the pinned main is the basis.
Every attended or normal-launch row runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773, see `20261005-embedded-server-runtime`).
Port changes for file headers: `Effect` services become plain functions; `useCopyToClipboard` becomes the clone's `copyText` op (`T3Module.swift`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | [#147](https://github.com/ccheever/exact2/pull/147) | Merged into integration branch | Confirmed merged; included in local merge `759779342` |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](../20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| recorded decision | U12 (sessions with three scopes) | User answer 2026-10-06 | Existing sessions stay unchanged; new scopes require re-pairing | Confirmed by user after failure review |
| conditional merged task PR | [20261005-environment-routes](20261005-environment-routes.md) | [#148](https://github.com/ccheever/exact2/pull/148) | Notice prerequisite not applicable to confirmed U12 | Merged independently; route scope propagation integrated and tested |
| scheduling preference | Either order with `20261005-environment-routes` | #148 | Both edit pairing paths | Integrated; Add route and SSH route keep the standard scope request |

`20261005-terminal-drawer` and `20261005-server-update-banner` depend on this ticket.

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X21](../../issues/20261005-x21-two-way-websocket.md) | Native transport carries the exchange | existing | nonblocking | none |

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

2026-10-06: implemented on `feat(example)/t3-code-remote-scopes-and-update-commands` (base `d78ac86ff`),
in parallel with `20261005-hot-file-split` (user-authorized parallel wave; the merged-PR prerequisites
remain pending, not satisfied). Reference `1e2ecbd975`.

- Scopes: `remote-scopes.ts` holds `AUTH_STANDARD_CLIENT_SCOPES` (the five, reference order) and
  `encodeOAuthScope`; `withStandardScope` adds `scope` to every request that carries a credential
  (Add environment connected and not connected, `client.ts` connect, SSH pairing, outdated-host
  pairing). `modules/apple/T3RemoteAuth.swift` builds the token-exchange form from the `scope` it is
  given (omitted when absent, the embedded primary's case) and is used by `T3Transport.swift` (pairing
  and connect exchange) and `T3Fleet.swift`. Swift names no scope.
- Narrow link: the server answers HTTP 400 `{_tag: EnvironmentRequestInvalidError, reason:
  scope_not_granted}` with no `message`; the clone now shows `mapRemoteEnvironmentError`'s text
  "The environment rejected the authentication request." (was "The server returned HTTP 400.").
  No retry with fewer scopes. The link is spent by the refusal (server behavior, confirmed live).
  The text is read from the reference source, not from an oracle redemption (no oracle on this branch).
- U12: confirmed by the user on 2026-10-06 after failure review: keep existing sessions
  unchanged, with no notice; extra permissions arrive when the user pairs again.
- Install-aware command: `server-installation.ts` ports `ServerInstallation`
  (`ForwardCompatibleOptional`: unknown kind or npm-global without prefix decodes as absent),
  `manualServerUpdateCommand`, the labels ("Copy update command" / "Copy relaunch command"), the
  toasts, the failure toast ("Could not copy update command", ClipboardWriteError's text) and
  `serverUpdateAriaLabel`. `connections.ts` uses them in the row and in `environment-update`; a
  desktop-managed server without `desktopAppUpdate` shows "Update the desktop app on that machine
  to update this server." in place of the button (`updateNote`, `connections.contract`).
  `settings-b-outdated.ts` needed only the scope (its update paths are self-update only).
- Exports for `20261005-server-update-banner`: `manualUpdateCopy`, `manualServerUpdateCommand`,
  `serverUpdateActionLabel`, `serverUpdateAriaLabel`, `desktopManagedOnly`, `configInstallation`.
- Shared-file edits (own commit `bebd6d4d4`): `client.ts` lines 57 (import) and 644 (connect spreads
  `withStandardScope(target)`); `T3Transport.swift` line 47 (`exchangeScope`), 307 (set from the
  request), 337 and 352 (pairing failure text and form), 400 (connect exchange form), 489–491 (HTTP
  failure text).

Live drive (macOS agent driver, drive lock held; isolated reference servers on 16140 "Lane box",
web mode, and 16141 "Desk box", desktop mode; lane tool `target/t3-ui-parity/narrow-link.mjs`
mints links through `POST /api/auth/pairing-token` with the server's desktop-bootstrap owner seed):

```
read-only link id: 3569dac3-…  (scopes ["orchestration:read"])   standard link id: a75cd7d6-… (five scopes)
Text#1587 [connection-error] "The environment rejected the authentication request."
Text#1568 [toast-title-2] "Could not add backend"
links after: neither 3569dac3 nor a75cd7d6 is listed (both spent)
clients: {"label":"standard-link","scopes":["orchestration:read","orchestration:operate","terminal:operate","review:write","relay:read"]}
state: "update": "Copy relaunch command"  (Lane box: no serverSelfUpdate, no serverInstallation)
state: "updateNote": "Update the desktop app on that machine to update this server."  (Desk box)
Pressable#1742 [environment-update-8b7b…] label="Copy relaunch command for Lane box server"
Text#1958 [environment-update-tip-…] "Copy relaunch command"
Text#1968 [toast-title-6] "Relaunch command copied"
Text#1969 [toast-description-6] "Stop t3 on Lane box server, then relaunch with `npx t3@0.0.46-nightly.20261004.1` using the same subcommand and options. This does not update an installed t3 command."
pasteboard: npx t3@0.0.46-nightly.20261004.1   (the previous text pasteboard was restored)
```

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-06) | `bd18baeab` on base `d78ac86ff` | `bun test examples/t3-code` 1216 pass / 0 fail (base 1200; the four named reference tests pass, `ServerUpdateAction` table has 6 kinds); strict tsc clean; `contract build` 2158 slots, 42 resources, 48015 nodes; `cargo test -p t3-code-macos --lib` 10/0; AppKit `transport` 40 tests 0 failures (2 live skips), `fleet` 8/0, `ssh` 4 (1 skip); `git add -A && bun scripts/caps.mjs` pass; five checks pass (build, test 2927/0, clippy, fmt, caps, boot); `build.mjs t3-code-macos` bundle built | Live drive above; screenshots on `t3-code-evidence/remote-scopes-and-update-commands/` (01 refused Read only link, 02 relaunch tooltip and desktop sentence, 03 relaunch toast) | Not run: trace-diff T0 and oracle redemption (no oracle/trace tools on this branch, `20261005-desktop-oracle-and-trace` pending); light/dark and 840×620 pairs (no-pixel-loop rule); pasteboard failure forced live (unit test only); Tab/Space/Enter focus ring (attended); three-scope session launch from a pre-change fixture store (unit test of the projection only); npm-global / pnpm-dlx / bunx live (the reference server built from source reports no installation; unit tests cover all kinds) |

## Verification close-out (2026-10-06)

The user authorized repair and the remaining verification, and explicitly confirmed U12:
retain existing three-scope sessions without a notice. Implementation is locally complete and
this ticket's acceptance is verified on `759779342dd34fa516bec36924923e6beeb4c23a`.

[Repair and full trace report](../../evidence/20261005-remote-scopes-and-update-commands/20261006-repair-and-trace/README.md):

- Custom AppKit pressables expose their focus mask bounds and use an exterior ring. A screen-region
  capture and an existing later window capture confirm the visible ring. The immediate post-input
  image was insufficient evidence; the earlier screenshot-based failure conclusion was unreliable. Previous failed evidence is preserved, not overwritten.
- Merged environment routes preserve all five scopes, including new Add route and SSH route paths.
- Actual native and Electron T0 captures contain matching ordered scope requests/grants, successful
  messages and approvals, and the assistant result. The full normalized comparison was executed.
  It reports 176 differences with no blanket allow list; whole-stream equality is not claimed.
  Differences include CORS, client identity, subscriptions/polling and trailing prompt whitespace.
  The ticket's scope-exchange criterion passes; unrelated full-app parity remains separate.
- TypeScript 1,829 pass/1 skip; root Rust 2,928 pass; app Rust 10 pass; final native button tests
  4 pass; merged transport 47 tests/2 live skips/0 failures. Build, typecheck, lint, formatting,
  caps and boot pass. Previously passing installation matrices and legacy-session evidence are reused.

The two historical dependency records above still have no separate merged PR recorded. Direct
runtime and trace evidence was produced for this user-authorized parallel implementation; this does
not mark the broader clone-on-main or reusable-oracle tickets delivered. #147, #148 and #155 are
confirmed merged into the integration base, now included locally.

Fixture processes, Keychain accounts and preference domains are cleaned up. Repair commits and
verification evidence are local; no remote push, PR update or merge was performed in this repair.
