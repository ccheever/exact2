---
name: 20261005-wsl-environments
plan: 20261005-t3code-macos-parity
implementation: blocked
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: daehyeon/t3-code
branch: null
pr_url: null
verified_commit: null
---

# WSL environments (Windows only)

## Outcome

On macOS, the clone shows no WSL row and exposes no WSL state, exactly as the reference does, and a check proves it. If the user decides in issue X41 to keep WSL for a Windows build of this example, this ticket is the starting point for that work: a second server in a WSL distro, or only that server, with the reference's states, texts and recovery paths.
This ticket starts only after the user decides. The expected decision is to close it as not applicable to macOS.

## Scope and exclusions

Included on macOS (runs now, after the decision to keep the ticket open for the proof only):

1. **Absence proof.** The state the reference reports off Windows is `{ enabled: false, distro: null, available: false, wslOnly: false, distros: [], preflightError: null }` (`packages/contracts/src/ipc.ts:553-572`; `available` is false unless the platform is `win32` and `wsl.exe` exists, `apps/desktop/src/wsl/DesktopWslEnvironment.ts:1154-1163`).
   The row shows only when WSL is available, enabled or in WSL-only mode (`apps/web/src/components/settings/ConnectionsSettings.logic.ts:14-20`), and the settings-search entry is Windows-only (`settingsSearch.ts:810-821`). The clone keeps the catalog entry `wsl-backend` hidden (`settings-catalog.ts:104`, flags `desktop,windows,localBackend,wsl`;
   `settings-search.ts:46-56` never sets `windows` or `wsl`) and draws no WSL row in "This machine" (`20261005-this-machine-network-access`).

Included only if the user keeps WSL for a Windows build (behavior at `1e2ecbd975`; evidence in [X41](../issues/20261005-x41-wsl-environments.md)); this plan cannot verify it, because it has no Windows host:

2. **Settings and bridge.** The "WSL backend" row and the bridge methods `getWslState`, `setWslBackendEnabled`, `setWslDistro`, `setWslOnly` (`ipc.ts:1197-1200`); distro choice with "track the WSL default" as null; inline `preflightError`; persisted settings `wslBackendEnabled`, `wslDistro`, `wslOnly`.
3. **Dual mode.** The Windows server stays primary. A second server is registered in the backend pool as `wsl:default` or `wsl:<distro>` on its own loopback-only port; changing the distro unregisters the old instance and registers a new one; errors are logged and never fatal;
   `reconcile` is idempotent and runs once after the primary starts and after each setting change (`DesktopWslBackend.ts:1-120`). Preflight failures (no Node, wrong version, missing build tools) show inline (`DesktopWslBackend.ts:58,112,167`).
4. **WSL-only mode.** Only the WSL server runs, as primary; turning it on needs an app restart. During a cold boot the app shows a "Connecting to WSL" splash instead of the main window (`DesktopApp.ts:237-245`, `DesktopWindow.ts:881-924`); a failed preflight shows a dialog and falls back to Windows.
5. **Runtime in the distro.** The packaged app carries a runtime archive that is installed into the distro's own filesystem; the first launch after an app update can take longer (`DesktopBackendConfiguration.ts:650-660`, `docs/user/install.md:79-85`). WSL resource telemetry is reported unavailable (`DesktopBackendConfiguration.ts:644-648`).
6. **Paths and `wsl.exe`.** UNC paths such as `\\wsl.localhost\<distro>\…` map to Linux paths for folder picking; distro names are validated; `wsl.exe -l` output is parsed as UTF-16 when needed (`wslPathParsing.ts`); `wsl.exe` calls have timeouts and fixed messages, for example
   "WSL backend preflight could not start wsl.exe to probe for <subject>. Check that WSL is installed and the distro is accessible." (`DesktopWslEnvironment.ts:176-180`). With the local environment off, WSL backends stay off (`docs/user/remote-access.md:229-232`).

Excluded: macOS and Linux behavior beyond the absence proof, Windows terminal helpers, T3 Connect, telemetry, the update feed, the Browser surface.

## Context and guidance

Parent specification: [spec](../spec.md). Source behavior: `apps/desktop/src/wsl/*` (`DesktopWslBackend.ts` 269 lines, `DesktopWslEnvironment.ts` 1,321, `DesktopWslServerTree.ts` 254, `wslPathParsing.ts` 126), `packages/contracts/src/ipc.ts`, `apps/web/src/components/settings/ConnectionsSettings*.ts*`, `docs/user/install.md`.
Library revision: `20261005-platforms-v3`. Selected topics: not applicable on macOS; Windows hosts and WSL are **unknown in the library**. The project instructions list web, macos, ios and linux hosts and no Windows host.
Consumer framework revision and toolchain: the pin from `20261005-clone-on-exact2-main`.
Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): `electron-oracle.mjs`. The oracle for the absence proof is the reference on this Mac; the Windows rows need a Windows oracle that this plan does not have.
Every launch runs a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 (see `20261005-embedded-server-runtime`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| recorded decision | [X41](../issues/20261005-x41-wsl-environments.md) | pending | The user closes it as not applicable (then this ticket closes), or keeps it for a Windows build (then a Windows host and oracle are named) | pending |
| merged task PR | [20261005-hot-file-split](closed/20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| scheduling preference | After `20261005-local-primary-environment` | none | The `desktop` and `localBackend` catalog flags are on, so the hidden state is the real one | pending |

## Issue assessment at preparation

Checked sources and time: planning pass 2026-10-05 against the local drafts in `../issues/` (unpublished, not reproduced); upstream not searched.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X41](../issues/20261005-x41-wsl-environments.md) | Decision: close as not applicable, or keep for Windows | scope decision; not in the library | blocking | `issue-open`, then the user decides |

## Implementation notes

- On macOS nothing is built. The proof is a pair of checks (below) and a test that the catalog flags keep the entry hidden after `20261005-local-primary-environment` turns on `desktop` and `localBackend`.
- For a Windows build, start from the ported pure code (`wslPathParsing.ts` and its tests, the state schema, `reconcile` rules) and write the process parts against a fake `wsl.exe`. Real `wsl.exe` runs need a Windows host.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Absence on macOS | — | Owned by `20261005-local-primary-environment` (row "No WSL on macOS"), so it survives if this ticket closes | — | — | — |
| Hidden stays hidden | `bun test` on the catalog and search filter | Run the search with `desktop` and `localBackend` on | `wsl-backend` is never listed | macOS | test log |
| Ported tests (only if kept for Windows) | Fake `wsl.exe` | `bun test`: `wslPathParsing.test.ts`; AppKit or Windows tests for `DesktopWslBackend`, `DesktopWslEnvironment`, `DesktopWslServerTree` cases | Original test names pass | Windows host with WSL2 (not available in this plan) | logs |
| Dual mode (only if kept) | Windows host, a distro with and without Node | Enable WSL backend; change distro; disable; turn the local environment off | `wsl:<distro>` appears and disappears; preflight errors inline; WSL stays off when the local environment is off | Windows | recording, state |
| WSL-only mode (only if kept) | Windows host | Turn on, restart | "Connecting to WSL" splash, then the main window; failed preflight shows the dialog and falls back to Windows | Windows | recording |

Task-owned source paths: `settings-catalog.ts` (the entry), `settings-search.ts`, tests under `bun test`; for a Windows build the files named in the plan that the user then creates.
Required environment: macOS 14+ with Xcode 27.0 and pinned Bun for the absence proof; for a Windows build, a Windows machine with WSL2 and at least one distro (names only).

## Progress

Blocked. Not started.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | X41 decision |

## Next action

Blocked until X41 is resolved or decided; then `prepare`, or close this ticket if the decision is to close.
