---
name: 20261005-upstream-ui-sync
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-upstream-ui-sync
pr_url: null
verified_commit: null
---

# Sidebar, device setup, brand mark and panel layering match T3 Code 1e2ecbd975

## Outcome

Five small reference changes from `f870c419fc..1e2ecbd975` show in the clone as they do in the reference desktop app:

- **A5.** The Working shelf lists threads by the last message the person wrote, newest first. A run that ends or wakes does not move a row.
- **A12.** The pinned row's Unpin button shows a pin at rest and a pin-off icon on hover and on keyboard focus.
- **A11.** Simulator diagnostics show before the device hub is ready: in the "Set up devices" wizard (step 0) and in Settings › Integrations.
- **A13.** The Azure DevOps mark is the new full-colour mark everywhere it appears.
- **A16.** The thread details popover sits below dialogs and below the floating device player.

## Scope and exclusions

Clone paths are `examples/t3-code/<file>`; the table writes `<file>`. Line numbers come from the mc-orch tree, 2026-10-05. Reference paths are repo-relative at T3 Code `1e2ecbd975` (`W/` is `apps/web/src/components/`). IDs are from [research](../research.md).

| ID | Reference behavior and states | Reference evidence | Clone state and work |
| --- | --- | --- | --- |
| A5 (`1302ccacbd`) | Working shelf order = max(`createdAt`, last authored message). `latestUserAuthoredMessageAt` absent (older server): use `latestRun.requestedAt`. `null` (agent-launched thread): `createdAt` only. Tie: thread id, then environment id. Applies to the Working shelf only; the active shelf keeps the return-time order. Shelf shows only when the "Working" setting is on. | `packages/client-runtime/src/state/threadInbox.ts:74-101` (`sortWorkingThreadsBySend`), `W/Sidebar.tsx:2799`. Wire field `packages/contracts/src/orchestrationV2.ts:1758`; `client-runtime/src/state/models.ts` keeps the field absent when the server omits it. Test `threadInbox.test.ts:61-90` "orders by the last message the user sent, not by later runs". | **Missing.** `sidebar-view.ts:105` sorts Working with `sortByReturn` (`sidebar-model.ts:173`: run request and completion times, plus the return tracker). Setting: `clientSettings.sidebarWorkingShelfEnabled` (`sidebar-view.ts:98`). Shells are untyped `Obj`, so the field is readable. Keep `sortByReturn` for the active shelf (`sidebar-view.ts:104`). |
| A12 (`6e0abd5a10`) | The button (aria-label "Unpin thread", tooltip "Unpin thread") draws `pin` at rest. On hover and on keyboard focus it draws `pin-off`. Colour change keeps its existing transition; the icon swap is instant. | `W/Sidebar.tsx:1714-1728`. No tests. | **Missing, glyph ready.** `SidebarPinButton` in `sidebar-row.contract:246-256` always draws `pin`; `pin-off` exists in `sidebar-icons.contract:11`; hover is `hoverId == t.hoverKey#unpin`. Add a local focus state (`focus=`/`blur=`, as `sidebar-row.contract:356`). The disabled "Pinned" case (`not t.canPin`) keeps the plain pin. |
| A11 (`5318d054a5`) | `localPlatformsUnavailable` = some host with `kind == "local"` has no available platform. Wizard step 0 also shows the "Check simulator support" section (with a 16 pt top margin) when the hub is enabled and `localPlatformsUnavailable`. Integrations: the "Simulator support" row (iOS and Android status, compact; "Ready" or the message; button "Refresh", "Checking…" while it runs, disabled when no environment, hub off, busy or pending) reveals when `(hostStatus == ready or localPlatformsUnavailable)` and the hub is not being switched on. It stays revealed while the hub is on. With several connected environments the row says "Status for {label}. Select an environment to inspect its simulator support." The reveal animates the height. | `W/device/DeviceSetup.tsx:63-70,123-124,290-310` (`PlatformStatus`). `W/settings/IntegrationsSettings.tsx:646-665,788-820`. RPC `device.list` (`packages/contracts/src/rpc.ts:425`), `device.configure` (:424). Host kind `packages/contracts/src/device.ts:92` (`"local" \| "ssh"`). No tests. | **Missing in both places.** Wizard: `r4-surfaces-device.ts:94-128` (`deviceView`, `platformSetupStatus` at :69) and `r4-surfaces.contract:436-447` show the section only at `device.step == 1`. Integrations: the search catalog lists "Simulator support" (`settings-catalog.ts:79`) but no row draws it; the Hub and Agent rows exist (`settings-source-control.contract:402`, `source-control-view.ts:124,246,264`). `device.list` is already called by `r4-surfaces-device.ts:154`. Reuse it. |
| A13 (`1c6269326a`) | New mark: viewBox 512, nine gradients and nine paths, from selfhst/icons (CC BY 4.0). | `W/Icons.tsx:161-325` (the source comment carries the attribution URL). | **Missing.** `BrandAzureDevOps` in `settings-brands.contract:41-62` is the old 96-box mark. Users: `palette.contract:144`, `r4-git-publish.contract:27`, `settings-source-control.contract:230`. Gradient ids are static in the clone (`azure-a`…), which is fine while every instance draws the same defs. |
| A16 (`e19a48d220`) | The details card in popover form (`variant="panel"`) has z `--z-sheet` 46. Order: docked sheets (46), floating preview player (47–49), dialogs and other popovers (50 and up). A dialog opened over the open card covers it; the floating player covers it. Other popovers stay at 130. | `W/ui/popover.tsx:73-79`, `apps/web/src/index.css:112-115` (comment), `W/chat/ThreadDetailsCard.tsx:151`. No tests. | **Likely wrong; needs a drive.** `ThreadDetailsPanel` (`shell-details.contract:36`) is a full-window layer at z 50 with a click-away backdrop, above the floating player (z 40, `r6-device.contract:267`) and the snooze dialog (z 40, `sidebar-overlays.contract:128`). Palette is z 60 (`palette.contract:391,482`). Other dialogs: `connections.contract`, `providers-wizard.contract`, `r6-pr.contract`, `settings-*` dialogs, `shell-panels.contract`, `timeline.contract` (nine `role="alertdialog"` columns). The inline card (docked from 984 pt) is z 20 and is not affected. |

Not in this ticket (no work): `d3bec62ec1` (macOS has a hovering primary pointer); `fe93a5dbdd` (server only); the `AnimatedHeight` reveal of A11 is a layout interpolation, which the library lists as unsupported: show the row at once, or use a measured layout transition if the oracle pair needs it, and record the choice.

States: A5 has none beyond an empty shelf (unchanged). A12: rest, hover, keyboard focus, disabled "Pinned". A11: hub off (no section), hub on with platforms unavailable (section shown, "Refresh" enabled), checking (button reads "Checking…" and is disabled), no environment (button disabled), several environments (description line). Wizard step 0 keeps its switch row and hub status line above the section. `aria-label` on every icon button is unchanged. A16: popover over a dialog, dialog over a popover, player over a popover. Keyboard and Escape: the details popover closes on Escape (its backdrop button already carries `aria-keyshortcuts="Escape"`, `shell-details.contract:37`) and focus returns to the control that opened it; a dialog opened over it takes Escape first and leaves the popover open; Tab stays inside the topmost dialog. The wizard closes on Escape. Motion: the reference animates the A11 row's height (`AnimatedHeight`); the clone shows the row at once, with or without reduced motion, because layout interpolation is unsupported (capabilities topic); the A12 icon swap and the popover z change have no motion.

## Context and guidance

Parent specification: [spec](../spec.md). Research: [research](../research.md). Library revision: `20261005-platforms-v3`. Selected topics:

- layout-and-interaction: boxes stack by `z-index`; a layout box is not proof that its centre takes a press, so assert with coordinate taps.
- design: complete states and long labels.
- accessibility: focus-visible behaviour and labels.
- testing-and-debugging: `--json`, one session per observation, `clock`.
- capabilities: general layout interpolation is unsupported.

Unknown in the library: the stacking rules of the macOS host for overlapping absolute layers and native views (the floating player is a native view); the clone's runtime evidence on the pinned main is the basis. Consumer framework revision and toolchain: the pin chosen by `20261005-clone-on-exact2-main`.

Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23): the oracle, trace and build tools come from `20261005-desktop-oracle-and-trace` (`electron-oracle.mjs`, `ref-build.sh`), and the fixture device proxy is the lane tool `target/t3-ui-parity/lanes/r7-integrate-dev/tools/devproxy.mjs`, which `20261005-clone-on-exact2-main` copies into the worktree.

Logic reuse (user rule): port `sortWorkingThreadsBySend` with its name and its test (`bun:test`, original title) into the sidebar model; do not re-derive the order. A11's `localPlatformsUnavailable` and A13's SVG come from the reference unchanged. Record each exact2 change in the file header. A13 adds a CC BY 4.0 notice: keep the source URL in the Contract comment and add the notice wherever the clone lists third-party notices (check the Licenses page at `prepare`).

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged into `daehyeon/t3-code` (common prerequisite: room and per-area seams in the shared files) | pending |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged (oracle shots, runtime at the new pin) | pending |

Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption`, which touches the same sidebar, popover and tooltip Contract files. Coordinate with `20261005-floating-device-player`: it must keep the player between the sheets and the dialogs (A16).

## Issue assessment at preparation

Checked sources and time: local issue drafts in [issues](../issues/README.md), `EXACT2-GAPS.md`, the library; no upstream search (planning). Re-check at `prepare`.

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| [X17](../issues/20261005-x17-popover-position-try.md) | Popover side areas and flips | A16 reads the popover's placement near the window edge at 840 pt | nonblocking | Record the oracle pair; no change expected |
| [X9](../issues/20261005-x09-root-component-across-files.md) | Line caps | `settings-source-control.contract` and `r4-surfaces.contract` grow with the new Integrations row | nonblocking until the cap | Put the row in a new `.contract` file |
| none found | A5, A12, A13, A11 | — | none | — |

## Implementation notes

- A5: add `sortWorkingThreadsBySend` to `sidebar-model.ts` (or a new file) with the reference's input type. Thread shells from other environments (fleet threads) carry the same field.
- A11 wizard: add `localPlatformsUnavailable` to `deviceView`; the section's markup at step 1 becomes a shared component used at step 0 and step 1. Fixture: the lane's device proxy reports a local host whose platforms are all unavailable (a fixture variant of `target/t3-ui-parity/lanes/r7-integrate-dev/tools/devproxy.mjs`; adding it is apparatus, decision U2).
- A11 Integrations: new row component reading `platformSetupStatus`, with the reveal state kept in the row (reset when the hub turns off).
- A13: paste the reference SVG as `defs` and `path` nodes, as the old mark does. Keep ids unique within the mark.
- A16: set one documented z scale in a comment at the top of `shell-details.contract`: sheets, details popover, player, dialogs. Move the popover below the player and every dialog; check each dialog layer listed above. The click-away backdrop must not block the player or a dialog.

## Acceptance and reproduction

All rows: macOS 26.6.2, 1280×840 and 840×620, light and dark, isolated fixture backends (ports 16000–16999; isolated HOME, CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_*, T3CODE_HOME). Pixel pairs use the reference desktop oracle.

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| A5 order | Working setting on; three threads whose runs are running; the third started last | `state` / `tree` of the sidebar; let a run end; send in thread 1 | Order follows the last send; ending a run does not move a row; the new send moves its thread first | macOS | ported test, agent transcript, sidebar crop beside the oracle |
| A5 older server | Shell without the field | Unit input | Falls back to `latestRun.requestedAt`; `null` uses `createdAt` | — | ported test cases |
| A12 icons | A pinned thread | Agent hover over the button; keyboard focus (Tab) | Pin at rest; pin-off on hover and on focus; disabled "Pinned" stays a pin; press still unpins | macOS | `tree`, shots of rest, hover and focus beside the oracle |
| A12 real pointer `(attended session)` | Lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>` (dev and lane builds refuse the real `~/.t3` and port 3773 by default, see `20261005-embedded-server-runtime`); a pinned thread | Move the real pointer onto the button and away; Tab to it | Pin-off while hovered or focused; pin otherwise; nothing stays painted after the pointer leaves | macOS | `(attended session)` notes, shots |
| A11 wizard | Hub enabled, local platforms unavailable | Open "Set up devices" | Step 0 shows "Check simulator support" with the same content as step 1 (both platforms, the explanation line, the "Check again" button) | macOS | shots beside the oracle, `state` |
| A11 Integrations | Same fixture; one and two connected environments; hub off | Open Settings › Integrations | Row hidden when hub off; revealed when on and unavailable; "Refresh" sends `device.list`; "Checking…" disables it; two environments show the "Status for …" line | macOS | shots, trace shows `device.list` |
| A13 mark | — | Open the Source Control settings, the publish wizard, the palette | New mark in all three; same pixels as the oracle at the crop | macOS | pixel pairs, notice present |
| A16 keyboard and Escape | 840 pt window; details card open from the header button | Press Escape; reopen, open the add-environment dialog, press Escape; Tab through the dialog | First Escape closes the card and focus returns to its button; the second sequence closes only the dialog and the card stays; Tab stays in the dialog | macOS | `tree --ax`, `state`, transcript |
| A11 wizard Escape and reduced motion | Hub enabled, platforms unavailable; `prefer prefers-reduced-motion reduce` | Open "Set up devices", press Escape; reopen Settings › Integrations and toggle the hub | Escape closes only the wizard; the Integrations row appears at once with and without the preference | macOS | transcript, shots |
| A16 layering | 840 pt window (popover form); details card open | Open the add-environment dialog, the snooze dialog and the palette; float a device player | Each dialog and the player draws over the card and takes presses (coordinate taps); the card keeps its place | macOS | `layout` z facts, taps, shots beside the oracle |
| Checks | `git add -A` | clone checks (bun test, strict tsc, contract build, `cargo test -p t3-code-macos --lib`, AppKit binaries), `bun scripts/caps.mjs`, the five checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `sidebar-model.ts`, `sidebar-view.ts`, `sidebar-row.contract`, `r4-surfaces-device.ts`, `r4-surfaces.contract`, `settings-source-control.contract`, `source-control-view.ts`, `settings-brands.contract`, `shell-details.contract`, `sidebar-overlays.contract`, `palette.contract` (z only), tests, `AGENT-HANDOFF.md`.
Required environment: the reference runtime at the new pin; the fixture device proxy variant; no real device hub needed.

## Progress

Implemented 2026-10-06 on `feat(example)/t3-code-upstream-ui-sync`; verification unverified.

- A5: `sortWorkingThreadsBySend` in `sidebar-model.ts` (raw V2 shells through `latestRun`; tie by thread id, then environment id); `sidebar-view.ts` orders the Working shelf with it and keeps `sortByReturn` for the active shelf. Ported test in `sidebar-working.test.ts` (original title) plus the older-server fallback, the tie and the shelf split.
- A12: `SidebarPinButton` draws `pin-off` while hovered or focused (local `focused` state from `focus=`/`blur=`; any focus, not only focus-visible), `pin` at rest; the disabled "Pinned" case keeps `pin`. Colour and aria-label unchanged.
- A11: `localPlatformsUnavailable` in `r4-surfaces-device.ts`, on `DeviceView`/`R4Device`. The wizard's "Check simulator support" section is the shared `SimulatorSupportSection` (`device-support.contract`) at step 1 and at step 0 when the hub is on and the local host has no available platform (4 pt margin over the panel's 12 pt gap = the reference's 16). Integrations › Devices: `SimulatorSupportSettingsRow` (compact iOS / Android status, "Status for {label}…" with several connected environments, Refresh → `device.list {}`, "Checking…" while its own refresh is pending). The reveal flag lives in `device-support.ts` per client and environment and resets when the hub is off. The reveal shows at once (AnimatedHeight is a layout interpolation; unsupported), with or without reduced motion.
- A13: `BrandAzureDevOps` is the reference's 512-box mark (nine gradients, nine paths; static ids `azdo-a`…`azdo-i`); the selfhst/icons CC BY 4.0 notice is in the Contract comment and in `LICENSE-T3` (the clone's notice file; the Licenses page lists the server's manifest only).
- A16: the details popover layer moves from z 50 to 35, under the floating player (40) and every dialog (40 and up), over the right-panel sheets (10/11) and the click-away menus (29–31). The scale is documented at the top of `shell-details.contract`. Remaining difference: the reference puts other popovers (130) over the card; here click-away menus stay under it.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (implement) | `883aed5b0` on `d78ac86ff` | `bun test examples/t3-code` 1209 pass / 0 fail (base 1200); strict tsc clean; `contract build` 2179 slots, 42 resources, 48348 nodes; `cargo test -p t3-code-macos --lib` 10 pass; no Swift touched (no AppKit binary affected); macOS bundle build pass; five checks: build pass, test 2928 pass / 0 fail, clippy and fmt clean, caps pass, boot pass | unit tests `sidebar-working.test.ts`, `device-support.test.ts` | — |
| 2 (live drive, macOS) | same build, reference server 1e2ecbd975 on 127.0.0.1:16180, isolated HOME/CODEX_HOME/CLAUDE_CONFIG_DIR/XDG_*/T3CODE_HOME | drive 1 stopped at op 1: `no view matches welcome-pairing-link` (the welcome wizard had not loaded at the first op); drive 2 stopped at op 1: `tap connection-settings: view 92 is hidden or inert` (the welcome modal had loaded). Both failures are drive-script timing; no app code was involved. Two drives used (ADDENDUM 3): live acceptance recorded unverified | text records only, no screenshots | live rows unverified |

## Next action

`verify`: one live drive that waits for the welcome wizard (`clock +3000 real` first), pairs through it, then covers A13 (Source control), A11 (Integrations hub off/on), A16 (840 pt popover, ⌘K palette over it, Escape order). Not run: every live row; A5 and A12 live rows also need a server thread (the reference server's `--auto-bootstrap-project-from-cwd` created no project here) and a pinned thread (pin is a context-menu or ⇧⌘P action); A12 real pointer (attended); oracle pixel pairs (no desktop oracle tool in this worktree); A11 wizard and Integrations with all platforms unavailable need the fixture device proxy variant, which is not in this worktree; A16 player-over-popover needs a floating device player.
