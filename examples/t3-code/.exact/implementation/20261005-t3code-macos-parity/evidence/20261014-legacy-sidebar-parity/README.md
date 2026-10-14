# Legacy sidebar repair — 2026-10-14

The seven application changes are implemented. Physical input qualification remains
open: the user directed continued work without screen input after the Mac locked.
Agent-dispatched native operations below are a separate, bounded check.

The prepared source was `4b062d4356acac5b4220a09bb9d65d5c86440ca7`, after the
dialog helper merge. `source-fingerprint.json` identifies the 23 changed runtime and
test files and records the aggregate algorithm. The [published packet](https://github.com/ccheever/exact2/blob/741e9780b1bbcbfff1cf3d0905d75eea4246c4f1/legacy-sidebar-parity/20261014/README.md)
is pinned to its evidence commit. `source-review.md` contains the
independent source approval, including the two drop races found and repaired in review.

## Behavior and scope

| Original row | Implemented behavior and evidence |
| --- | --- |
| LS-4 | The mounted rename field uses the merged field-focus/select helper and its own ID. Command completion refuses composer focus while rename is active. Contract-action tests cover UTF-16 title length, helper lifetime and the late-focus guard. The clean preserved-baseline agent double-click mounted rename with focus still in the composer and no app error (`native-before.json`). The fresh native replay focuses the field, a native agent key replaces the entire title, and Escape restores it (`native-after.json`). Different-row double-click also opens and focuses its rename field. Physical double-click remains open. |
| LS-6 | Every project-settings producer returns its destination in the command result, so the root does not read navigation from a separately refreshed snapshot. The existing raw-member-to-logical-project mapping remains. Dialog labels name the local environment and Group into… uses U+2026. Merged dialog trap/final-focus behavior is retained. Agent-mode context menus dismiss automatically; original native menu qualification remains open. |
| LS-8 | Shift unions the anchor range with the current selection. Missing anchors add the target; Command-toggle keeps the anchor. Modifier selection happens before any environment switch. Tests exercise both sidebar modes, directions, gaps, duplicates, project changes and plain-click clearing. |
| LSF-3 | All native row queue keys include environment and thread. Consumption waits for readiness, captures its owner before awaiting, releases stale staged files, and tracks an abandoned destination through later revisits. Image fallback chips keep the draft captured before editor insertion. Existing client/native fixtures cover readiness transitions, homonymous IDs, order, exactly-once consumption, failed presses and both asynchronous handoff races. Finder drop 4 remains open. |
| LSF-4b | The mounted inline Confirm uses its actual ID and the merged initial-focus helper. Contract-action tests retain within-row lifetime and check reopen focus. Physical focus and keyboard activation remain open. |
| LS-7 | Mark unread silently ignores ordinary server failures, retaining cancellation behavior and completed-run/fallback paths. Tests cover local and background responses and unrelated errors. Original native menu qualification remains open. |
| LEG-TIP | Visible and delayed legacy tips require an owner still rendered in the current sidebar. Invalid owners clear pending state; valid reorders and unrelated tooltip delays remain intact. Tests cover archive/removal, collapse, mode changes and the global open/close delays. Pointer-still OS qualification remains open. |

Returning the project ID in the command result removes a verified timing dependency;
this does not establish that dependency as the cause of the original native menu failure.
No framework, native Swift, provider, GitHub or shared final-focus implementation changed.

## Verification

`checks.json` records 5,073 passing Bun tests, 396 independent focused tests, strict
TypeScript and Contract compilation, all 29 app Rust tests, the fresh native bundle,
and the coordinator’s passing default-member, workspace, formatting, caps and Contract
source-sweep checks (94 roots, 195 imports). `native-build.json` records the fresh build
receipt and artifact hashes. The bounded native agent after-check passes; physical
qualification remains open.

### Native comparison

The clean before/after records use the same raw agent tap and short clock advances,
without `clock settle`. Before: snapshot 5 has composer focus; snapshot 7 mounts rename
but leaves native focus in the composer. After: snapshot 4 has composer focus; snapshot
6 focuses the mounted rename field and native FieldEditor. Both reads have empty app
error strings. After snapshot 7, the agent key `x` yields the complete field value `x`;
Escape restores `Branch work` in snapshot 9. A later double-click opens `Dev server`
and focuses its rename field in snapshot 11; Escape restores its title in snapshot 14.
All six after assertions pass. The fixture’s provider-unauthenticated banner is expected;
no provider request was made.

[Native rename capture](https://raw.githubusercontent.com/ccheever/exact2/741e9780b1bbcbfff1cf3d0905d75eea4246c4f1/legacy-sidebar-parity/20261014/rename.png) — screenshot operation after snapshot 11 produced
snapshot 12: `Dev server` rename with its full title selected, macOS 26.6.2, agent timing.
The image supports the focus/selection observation; typed replacement is established by
the key/value readback. Physical input remains unverified. The PNG is published in the
`t3-code-evidence` branch of `ccheever/exact2`, not included in application source.

`process-cleanup.json` confirms all recorded app/controller PIDs and the owned backend
are stopped, with no listeners on ports 16566/16567. Fixture files are preserved.

## Reproduction and retained failures

Run the task's commands from the repository root with Bun 1.4.2. The existing
`legacy-sidebar-full.test.ts` fixture exercises real client commands with native/server
replies; the compiled-plan tests reuse `composer_accessibility_tests.rs`'s fixture.
No additional test interpreter, debug endpoint or fixture-server implementation was introduced.

Local raw logs remain under `target/legacy-sidebar/`. The packet contains selected
results, not credentials or complete client snapshots. Retained failed attempts include
the initial seven-row regressions, the project-navigation stale-snapshot regression,
the deferred-clear/revisit regression, one Contract stale-read diagnostic fixed by
ordering the existing hover actions, and obsolete hover source assertions corrected
after the behavior changed. The first two compiled-plan tests also failed because
the test setup had not landed queued mutation callbacks. Adding the runner’s documented
`land_then()` step made the unchanged expectations pass; those were test-setup failures,
not product failures. The earlier native attempt with an admission error is retained in
`native-before-inconclusive.json`; the primary before record is the clean replay. These
were one implementation repair round.

## Fixture and qualification limits

The native fixture uses task-owned copies of the existing audit projects and homes,
ports 16566/16567, and the pinned `t3-0.0.46-nightly.20261005.2667-darwin-arm64` runtime.
No provider run or GitHub mutation was used. The baseline app was built before source
edits; its standalone files and receipt were preserved and hashed before the final bake.
The supported `EXACT_MAC_BIN` override labels its driver freshness unchecked; the
packet identifies that explicit old binary rather than treating it as a current build.

An earlier live setup established that a named Contract store and the two attempted
preference environment flags did not isolate the app's saved environments. Only this
task's exact catalog entry and Keychain account were removed. The coordinator verified
the restored typed preference data against the prelaunch snapshot and the account's
absence. `AGENT-HANDOFF.md` records the corrected recipe. Subsequent checks use
`EXACT_AGENT=1`, whose module keeps credentials and saved environments in memory.

The host is macOS 26.6.2 (25G83), Apple Silicon; global Keyboard Navigation remains off.
No macOS 14, web, iOS or Linux runtime parity is claimed. Physical double-click,
menu selection/traversal, Command/Shift pointer selection, Finder background drop,
Confirm keyboard activation, Mark unread menus and pointer-still tooltip removal remain
unverified. Publication or merge does not close those acceptance items.
