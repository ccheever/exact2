---
name: 20261005-desktop-oracle-and-trace
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

# Reference desktop oracle and protocol trace comparison

## Outcome

Any lane can (1) launch the reference T3 Code **desktop** app, built at the pinned reference
commit, fully isolated from the user's real T3 installation, and capture it at the same
window sizes and appearances as the clone; (2) record the HTTP and WebSocket RPC traffic of
the clone and of the reference desktop app for the same scenario; (3) diff the two traces
with a reasoned allow list; and (4) print an RPC tally: for every reference `WS_METHODS`
entry, whether the reference and the clone call it. Every later feature ticket uses these
four outputs as acceptance evidence.

## Scope and exclusions

Included (each item is apparatus; see plan "Apparatus requiring approval"):

1. **Reference desktop build** (`ref-build`): build the reference at a pin from a copy
   (`target/t3-ref/src-<sha>` from `git -C <ref> archive <sha>`; never modify the reference
   checkout) with the reference's own toolchain (`npx pnpm@<pinned>`, the workspace `vp`).
   Output: `target/t3-ref/desktop-<sha>` and `target/t3-ref/runtime-<sha>`. Pin: at least
   `1e2ecbd975` — feature tickets need server features newer than the round-11 fixture
   `runtime-f870c41` (for example `orchestration.getTurnItem` from `5a96895a85`). Prefer the
   same version as the embedded server pin, so the oracle, the lane fixture backend and the
   app's own server agree. Switch `lane-backend.sh`'s default to this runtime.
2. **Electron oracle** (`electron-oracle.mjs`): run the built Electron executable directly
   (never `start:desktop`, which registers the bundle id and the `t3code` scheme) with
   `--user-data-dir`, a remote-debugging port, mock keychain if it works, isolated
   `HOME`/`CODEX_HOME`/`CLAUDE_CONFIG_DIR`/`XDG_*`, `T3CODE_HOME=<lane>/ref-home`,
   `T3CODE_TELEMETRY_ENABLED=false`, `T3CODE_DISABLE_AUTO_UPDATE=1`, `T3CODE_PORT=<lane port>`.
   **Keychain.** The reference stores saved-environment bearer tokens with Electron
   `safeStorage` (`apps/desktop/src/settings/DesktopSavedEnvironments.ts:230`), which uses the
   login Keychain under the app's display name (`DesktopAppIdentity.ts:94`). Use mock keychain
   if it works; otherwise give the oracle a lane-specific display name, or a disposable
   keychain (`security create-keychain` + `security list-keychains -d user -s …` restored after
   the run). Restore the keychain search list in a `trap` that also runs when the oracle
   fails partway, and verify it with `security list-keychains -d user` before and after.
   Never read or change the user's existing items.
   Capture with playwright-core `_electron` + `page.screenshot()` at 1280×840 and 840×620,
   light and dark. Native chrome (title bar, menus) only with `screencapture -l` when the user
   grants Screen Recording. Check `lsregister` before and after: no `com.t3tools.t3code`
   or `t3code` registration may appear.
3. **Trace proxy** (`trace-proxy.mjs`): a local proxy between client and server that records
   each HTTP request and WebSocket frame (tag, method, payload) with tokens redacted. The
   clone reaches it through an agent-only origin override; the reference desktop records the
   same format with Playwright `page.on('websocket')`.
4. **Trace diff** (`trace-diff.mjs` + `scenarios/<id>.allow.json`): normalize ids and
   timestamps, drop acks and pings, align steps with `/__trace/mark?step=`, compare writes in
   order and reads/subscriptions as multisets; every allowed difference states its reason.
5. **RPC tally**: from the reference contracts' `WS_METHODS` list, a table of
   reference-called / clone-called per scenario, saved per phase.
6. **Location.** Decision U23 (2026-10-05): commit these tools under
   `examples/macos/t3-code/tools/`; their outputs (reference builds, captures, traces) stay in
   `target/`.
7. **Docs**: an `AGENT-HANDOFF.md` section that documents the four tools, the isolation
   rules, and a new parity matrix layout: ID | reference source (file:line@pin) | clone files |
   ported tests | trace | pixel cells | effect evidence | real input | status.

Excluded: fixing any parity difference the tools find (that belongs to the feature tickets);
Browser surface capture.

## Context and guidance

Parent specification: [spec](../spec.md) (fidelity row: the oracle is the desktop app).
Source behavior: reference `apps/desktop/` (Electron main, preload, `DesktopBridge` in
`packages/contracts/src/ipc.ts`), `packages/contracts` `WS_METHODS`. Base for the proxy:
mc-orch `target/t3-ui-parity/lanes/r7-integrate-dev/tools/devproxy.mjs`.
Library revision: `20261005-platforms-v3`. Selected topics: testing-and-debugging (an exit code
of zero is not proof; review recipe and outputs; record source identity), platforms (label
capture mode; native chrome needs normal-launch capture).
Why: the 2026-10-05 audit found the largest missing behavior (`server.reportClientActivity`)
only by counting RPC use, and found that earlier rounds compared against the web client.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged into `daehyeon/t3-code` | pending |
| recorded decision | Apparatus approval for items 1–5 | none | User approves | pending |
| recorded decision | Network installs: pnpm (reference-pinned version), `vp`, the Electron version the reference pins, Playwright core | none | User approves | pending |

Scheduling preference (not a prerequisite): after `20261005-hot-file-split`, if this ticket
needs an agent-only origin override in app code.

## Issue assessment at preparation

Checked sources and time: {{at prepare}}. No framework blocker expected: the tools run outside
the exact2 app.

## Implementation notes

- Tools live in this worktree's `target/t3-ui-parity/` (user decision #1). If the user wants
  them portable for fresh clones, commit them under `examples/macos/t3-code/tools/` instead;
  this is part of the apparatus approval.
- The reference desktop app keeps its server inside the app; the oracle must not reach the
  real `~/.t3` even though the clone's production build shares it (spec). Lanes always isolate.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Isolated oracle launch | `ref-build` at the pin; lane port 16xxx | `electron-oracle.mjs <lane> shot home 1280x840 dark` | Screenshot of the reference desktop home; `lsof` shows only lane ports; `lsregister` unchanged; real `~/.t3` mtime unchanged | macOS 26.6.2 | png + before/after `lsregister` dumps |
| Keychain isolated | Before and after one oracle run | List items by service name only (`security find-generic-password -s "<display name> Safe Storage"`); never dump the keychain | No new item under the real T3 Code names; any lane item is removed after the run | macOS | before/after listings |
| Clone and oracle trace | Same fixture backend for both | Scenario T0: pair, open a thread, send one message, answer one approval | Two trace files with the same step marks | macOS | ndjson traces |
| Trace diff finds a known difference | T0 traces on the clone **before** `20261005-client-activity-reporting` | `trace-diff.mjs T0` | Reports the missing `server.reportClientActivity` calls (a known gap) and nothing unexplained | macOS | diff report |
| RPC tally | T0 traces | tally command | Table lists every `WS_METHODS` entry with reference/clone columns | — | md table |
| Secrets | — | grep traces for bearer tokens and pairing codes | None present | — | grep log |

Task-owned source paths: `target/t3-ui-parity/{electron-oracle,trace-proxy,trace-diff}.mjs`,
`target/t3-ui-parity/scenarios/`, `target/t3-ui-parity/ref-build.sh`, the clone's agent-only
origin override (if it needs app code), `AGENT-HANDOFF.md`.
Required environment: network access for the approved installs; optional Screen Recording
permission for native chrome captures.

## Progress

Planned.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after the clone-on-main PR merges and the apparatus is approved.
