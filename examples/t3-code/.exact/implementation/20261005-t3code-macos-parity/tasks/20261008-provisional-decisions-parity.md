---
name: 20261008-provisional-decisions-parity
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-provisional-decisions-parity'
pr_url: https://github.com/ccheever/exact2/pull/296
verified_commit: null
---

# The provisional decisions now match the reference (U5–U10, provider upkeep)

## Outcome

The user decided on 2026-10-08: "원본 기능과 동일해야함" — every provisional decision must match the original T3
Code. Earlier task PRs took a recommended default for open decisions U5–U10 and three provider-upkeep questions and
marked it provisional. Each one was read against the reference (`~/Documents/work/3.open-source/t3code` at
`1e2ecbd975`: desktop app, web client, server), the clone was changed where it differed, and the provisional marks
are gone from the records. A difference stays only where a framework limit forces it: U5 (#117 / X31) and the
number field's character filter (X60, a local draft; the user files issues).

## Scope and exclusions

Included: U5, U6, U7 (`20261005-local-primary-environment`), U8, U9 (`20261005-this-machine-network-access`), U10
(`20261005-app-activation`), provider-settings-upkeep (a), (b), (c). Excluded: U13 (the real-`~/.t3` smoke; not in
this decision), the relaunch rows (#122 / X45), T3 Connect (X38, out of scope by user decision).

## Per item: reference, clone before, clone after, proof

| Item | Reference behavior (`1e2ecbd975`) | Clone before | Clone after | Proof |
| --- | --- | --- | --- | --- |
| U5 first window | No window until the backend is ready; with the Local environment off the window opens at once (`apps/desktop/src/window/DesktopWindow.ts:866-878`, `:984-988`) | Window at launch, connecting state until the primary connects | Unchanged, now decided: exact2 still cannot hold the first window. Main `f464bad43`: `scripts/app.schema.json` `host.macos.window` allows only `width`, `height`, `minWidth`, `minHeight` (`additionalProperties: false`); `host/apple/Sources/ExactMac/main.swift:558` orders the window front with no condition; #117 open | Framework limit **#117** (X31); adopt the hold when it lands |
| U6 saved duplicate of the primary | `installPlatformRegistration` (`packages/client-runtime/src/connection/registry.ts:614-693`): forget GitHub routing trust for the id first (a failure keeps everything), drop the saved registration and its credentials (`storageDocument.ts:174-199`), replace the entry in place (a socket on it moves to the primary); no message. Pairing the primary's own server: `register` returns early for a platform id (`registry.ts:559-561`) after the code was spent; the UI still says "Backend added" | Entry and credential removed silently; GitHub sharing trust kept; a window focused on the duplicate was disconnected and stayed so; pairing this machine saved the credential and entry, then the next sync removed them | Trust forgotten first, a failed write keeps the entry (`local-primary.ts` `forgetGitHubRouting`, `routingKeyEnvironment`); Swift `forgetEnvironment` reports `forgotFocus` and the primary takes the window (`r8-pointer-reconnect.ts` `primaryTakesFocus`); `pairEnvironment` with the primary's id spends the code and saves nothing (`T3Transport+Environments.swift`) | Bun: 3 tests in `local-primary.test.ts`, `connections.test.ts`; Swift: `testForgettingAFocusedDuplicateSaysSoSoThePrimaryCanTakeTheWindow`, `testPairingThisMachinesOwnServerSpendsTheCodeAndSavesNothing`; live: own pairing (`u6-own-pairing.png`) |
| U7 settings file | `<T3 home>/userdata/desktop-settings.json` (`DesktopStatePaths.ts:23-32`, `DesktopEnvironment.ts:209`); lenient JSON (comments, trailing commas, `schemaJson.ts:166-217`); one key of the wrong type makes the whole document default (`DesktopAppSettings.ts:396-416`); normalization `:215-251`; sparse write of non-default values in schema order, compact JSON plus a newline, through `<target>.<pid>.<uuid>.tmp` and a rename onto the symlink chain's target (`:253-298`, `:418-482`); read once before the backend starts | Top-level keys of the clone's own `t3-code.json` under Exact's data root; T3 Code's file never read | `T3DesktopSettings.swift` ports the schema, decoding, normalization, encoding and write; `T3LocalBackend` reads it when the first session attaches (before any TypeScript answer) and decides the switch and the bind host from it; TypeScript reads the four keys from `localBackendStatus.desktopSettings` and writes through `desktopSettingsSet` (the renderer's IPC setters). One-time carry-over: a key the old `t3-code.json` has and the desktop document lacks is adopted and written; `client.ts` saves once without the old keys. The default endpoint stays in `t3-code.json` (the reference keeps it in renderer localStorage, `uiStateStore.ts:6,29`) | Swift: 21 tests in `macos/tests/local-backend/settings.swift` (the reference's own test names); Bun: `client.test.ts`, `local-primary.test.ts`, `connections-network.test.ts`, `server-exposure.test.ts`, `local-backend.test.ts`; live: U7 A/B/C (`u7-launch-local-off.png`, `u7-network-from-file.png`, record) |
| U8 hosted pairing link | Hosted link only for an endpoint marked hosted-compatible with an `https:` URL (`ConnectionsSettings.tsx:550-561`, `pairingUrls.ts:10-20`); `https://app.t3.codes/pair?host=<encoded endpoint>#token=<code>` (`hostedPairing.ts:73-87`, `remote.ts:165-170`); hint "Opens the hosted app, no install needed", toast "Hosted app link copied"; copy buttons never disabled (`ConnectionsSettings.tsx:816-826`, `:942-953`, `:865-872`) | Same conditions, URL and strings; Copy code, Copy link, Copy code only and the reveal dialog's buttons were disabled while any command ran | The six buttons are never disabled | Live, base and branch identical: pasteboard `https://app.t3.codes/pair?host=https%3A%2F%2Flane-pdp.example.test%2F#token=[redacted]` (record); unit tests `pairing-urls.test.ts` unchanged |
| U9 Tailscale | `tailscale status --json` 1.5 s, `Self.DNSName` without the trailing dot, 100.64/10, endpoints and strings (`packages/tailscale/src/tailscale.ts`, `tailscaleEndpointProvider.ts`); the probe's 2.5 s is a deadline for the whole request (`tailscale.ts:365-381`); the page's snapshot is an SWR atom: revalidated on mount when older than 30 s and after a change, never polled while open (`state/desktopNetworkAccess.ts:12,82-89`); the port field is `type="number"` (ArrowUp/ArrowDown step) and valid when the trimmed text matches `/^\d+$/` and is 1–65535 (`ConnectionsSettings.tsx:2179-2185`, `:3772-3782`) | Same CLI calls, endpoints and strings; the probe timeout was URLSession's idle timeout; the snapshot was re-read every second while the page was open; "0443" refused; no stepping | Probe: a deadline (`T3Once`) for the whole request; snapshot: the SWR rule (`connections-network.ts` `NETWORK_STALE_TIME_MS`, revalidate on open and after a change); port: digits-only rule (`tsDigitsOnly`), `type="number" min=1 max=65535 step=1`, ArrowUp/ArrowDown step in the dialog (`tsStep`) because the macOS number field does not (X60) | Swift `testTheHttpsProbeIsADeadlineForTheWholeRequest`; Bun "keeps its snapshot while the page stays open and revalidates on open once 30 s old"; live pairs `u9-port-0443.png`, `u9-port-arrowup.png`. Live Tailscale needs a tailnet the user provides (verification gap only) |
| U10 CLI install action | None in the desktop app: no menu item, palette command, settings row, symlink or bundled `t3` (`DesktopApplicationMenu.ts:156-266`, `CommandPalette.tsx:1915-2283`, `scripts/build-desktop-artifact.ts:1039-1102`); users install with `scripts/install.sh:219-227` or npm (`docs/user/install.md:13-26`) | None built | Unchanged, now decided as the reference. The `t3code://` deep link is T3 Connect (`DesktopClerk.ts:127-193`): out of scope by user decision (X38), not a difference | Reference read (above) |
| (a) launch prompt source | Mounted only for an authenticated primary (`__root.tsx:247`, `:97`); providers from `primaryServerProvidersAtom` (`ProviderUpdatePrimaryNotification.tsx:101-102`, `state/server.ts:104-107`); the sidebar pill reads the same atom (`SidebarProviderUpdatePill.tsx:44`); dismissals kept without a limit (`providerUpdateDismissal.ts:65-93`) | With no primary, the focused environment stood in (prompt and update); the pill read the focused environment; dismissals capped at 100 | `primaryTarget` returns nothing without a primary; the pill reads `primaryTarget`'s providers; no cap | Bun: "the launch prompt follows the primary only…", "the pill reads the primary's providers only…", "dismissed launch prompts are all kept…"; live pair `a-prompt-local-off.png` |
| (b) no "Updating" toast | `shouldShowPrimaryProviderUpdateToast` drops the running view (`ProviderUpdateLaunchNotification.logic.ts:253-255`); the running update shows in the sidebar pill (`:456-476`) | Matched | Unchanged (verified) | Existing test "the launch notification…" (`toasts(client)` empty while running) |
| (c) custom ACP model | `updateCustomModels` writes `toCustomModelSetting` for every driver (`ProviderInstanceCard.tsx:673-681`; `packages/shared/src/model.ts:408-419`): an added model is its slug (`ProviderModelsSection.tsx:250`), a name or options make `{slug, name?, capabilities?}`; the server's ACP schema is a string list (`packages/contracts/src/settings.ts:908`), so such an instance fails to decode and becomes unavailable (`ProviderInstanceRegistryLive.ts:139-158`) | ACP kept only slugs: a saved name or options were dropped without a word | `storedCustomModels` is `toCustomModelSetting` for every driver | Bun `custom-model-editor.test.ts` (updated). The server's refusal is the server's own code (read, not driven) |

## What was built

1. **U7, desktop-settings.json.** `modules/apple/T3DesktopSettings.swift` (new): `T3DesktopSettings` (the 12 keys and
   their defaults; the default update channel from the version this client mirrors, `CLIENT_VERSION`, checked by a Bun
   test), `document` (lenient parse, the schema's per-key type checks, all or nothing), `normalize`, `encode` (sparse,
   schema order, compact) and `T3DesktopSettingsStore` (`load` once, `persist` serialized, the write through a temp file
   and a rename onto the resolved symlink target, the reference's `DesktopSettingsWriteError` message). A refused
   development build (no T3 home) keeps the settings in memory. `T3LocalBackend.attach` loads it and carries the old
   keys over; `begin` and `prepare` read the switch and the exposure from it; the op `desktopSettingsSet`
   (`T3Module+Local.swift`). TypeScript: `local-backend.ts` (`settings` in the status, `writeDesktopSettings`),
   `local-primary.ts`, `this-machine.ts` (persist first, write back on a failed start), `connections-network.ts`
   (the exposure store over the native file), `local-environment.ts` and `client.ts` (save once without the old keys).
2. **U6.** `local-primary.ts` `forgetGitHubRouting` and `dropPrimaryDuplicates` (`{origins, focusDropped}`),
   `connection-routes.ts` `routingKeyEnvironment`, `settings-b-fleet.ts` `focusDropped`, `r8-pointer-reconnect.ts`
   `primaryTakesFocus`, `client.ts`; Swift `forgetEnvironment` answers `forgotFocus` and `pairEnvironment` saves nothing
   for `primaryEnvironmentId`; `connections.ts` sends it and places no route for such a pairing.
3. **U8.** `connections-network.contract`, `connections-network-dialogs.contract`: the copy buttons are not disabled.
4. **U9.** `T3LocalNetwork.swift` (probe deadline, `T3Once`), `connections-network.ts` (SWR snapshot),
   `connections-network-dialogs.contract` (`tsDigitsOnly`, `tsStep`, `type="number"`); issue draft X60.
5. **(a), (c).** `provider-update-notify.ts`, `sidebar-provider-pill.ts`, `shell-prefs.ts`, `custom-model-editor.ts`.
6. Records: this file; the provisional marks removed from `20261005-local-primary-environment` (U13 stays
   provisional), `20261005-this-machine-network-access`, `20261005-app-activation`,
   `20261005-provider-settings-upkeep`; X31; X60 (new); `issues/README.md` (X60 row, X31 status, two leftover
   conflict-marker blocks resolved); `EXACT2-GAPS.md`; `STATUS.md`; `README.md`; code comments.

## Acceptance and results

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| U5 checked on main | Pass (kept, decided) | main `f464bad43` schema and `main.swift`; #117 open | #117 (X31) |
| U6 matches the registry | Pass | Bun and Swift tests above; live own pairing | — |
| U6 launch race (focused duplicate) live | Not run live: an agent relaunch starts with no saved environments (the module's data directory is per process in agent mode, X50 in `EXACT2-GAPS.md`), and a normal launch of a lane copy shares the user's clone defaults domain (same bundle id) | Swift `testForgettingAFocusedDuplicateSaysSoSoThePrimaryCanTakeTheWindow` + Bun "a window focused on a duplicate moves to the primary…" | X50 (agent relaunch keeps no module data) |
| U7 file, format, keys | Pass | Swift settings tests (21); live A/B/C | — |
| U7 shared with T3 Code | Pass for the file the clone reads and writes (format checked byte for byte: `{}` + newline, sparse keys, other keys kept); T3 Code itself not launched (never touch the user's T3 Code) | live C; Swift `testTheClonesOldKeysAreCarriedOverOnceAndTheDesktopFileWins` | — |
| U7 carry-over once | Pass (unit) | Swift `testTheClonesOldKeysAreCarriedOverOnceAndTheDesktopFileWins`; Bun `client.test.ts` "a t3-code.json from before decision U7 is saved once…" | live: an agent run's `t3-code.json` is a fresh per-process folder (X50), so no old file can be seeded |
| U8 conditions, URL, UI | Pass | live record (base = branch), unit tests | — |
| U9 implementation vs reference | Pass | tests above; live port pairs | live Tailscale: a tailnet the user provides |
| U10 | Pass (decided as the reference) | reference read | — |
| (a) | Pass | tests; live pair | — |
| (b) | Pass (already matched) | existing test | — |
| (c) | Pass | test | the server-side refusal was read, not driven |

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `106483621`…`6936a71ee`, merge `708419bec` | Clone: `bun test examples/t3-code` 3058 pass / 1 skip / 0 fail (base `07dcef1ab` 3036 pass / 1 skip); strict `tsc` clean; contract build 3845 slots, 46 resources; Swift module tests: local-backend 82, transport 59 and every other recipe directory 0 failures (mermaid, timeline-keyboard not run: own setups); `cargo test -p t3-code-macos --lib` 11 pass; caps within; five checks green: build, test 3383 passed / 0 failed / 33 ignored (94 binaries), clippy, fmt, boot | [evidence](https://github.com/ccheever/exact2/tree/t3-code-evidence/provisional-decisions-parity) | rows above |
| live drive | before `07dcef1ab`, after `592657b3a` / `6936a71ee`, agent mode, lane ports 16601/16602 | First screenshots were white: taken before the first paint; a 6 s real wait fixed it. Drive A's `tap welcome-continue` failed on a Local-off launch (no wizard): the script was fixed. The U9 drive first found no Tailscale switch: the reference reads Tailscale only with network access or Serve on, so the drive turns network access on first | [record](https://raw.githubusercontent.com/ccheever/exact2/82b35ee9e658228de317416e75525da72b7da1f9/provisional-decisions-parity/drive-record.txt) | — |

## Real-input batch steps

None needed: every row here was driven in agent mode or is a unit test. Optional, for the U6 launch race in a
persistent session: a lane build without agent mode, pair this machine's own server from another address before
the primary names itself (or restore a saved entry with the primary's environment id), relaunch, and read that the
window ends on the primary ("This machine") rather than "Disconnected.".

## Next action

Review of the draft PR. The user files X60 if wanted. Adopt #117 (U5) and #122 (relaunch) when exact2 has them.
