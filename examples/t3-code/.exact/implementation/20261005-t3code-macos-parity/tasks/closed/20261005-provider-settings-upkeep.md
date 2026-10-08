---
name: 20261005-provider-settings-upkeep
plan: 20261005-t3code-macos-parity
implementation: done
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-provider-settings-upkeep
pr_url: https://github.com/ccheever/exact2/pull/251
verified_commit: d5e052389
---

# Provider settings upkeep: updates, ACP management, custom model options, icons

## Outcome

In Settings › Providers, a provider's update icon opens the same details popover from list rows and from the editor, with "Install vX" or "Update now", a progress state, and a copyable manual command. An **Update all** button updates every outdated provider on every connected environment and reports one result. Update toasts follow the server's `updateState` (running, failed, unchanged, succeeded). An ACP agent's card gets Native sessions (list, import, delete), Log out, Agent providers (protocol, base URL, write-only headers) and the "Continue authentication" link. A custom model gets the options editor. ACP agents show their registry icon everywhere an instance icon appears.

## Scope and exclusions

Included (missing or partial; sources in "Context"):

1. **A9 update details popover.** One node for list and editor (`versionAdvisoryNode`); read-only hides the action; running state from `updateState` or a local pending set. Today list rows draw a plain icon (`providers.contract:184-189`) and the editor shows an inline card (`providers.contract:262-292`).
2. **D12 Update all** in the Providers header and the **update toast states**. Replace the partial port in `shell-notify.ts:137-193` and `shell-commands.ts:33-51` (which treat an RPC return as success and ignore `updateState`) with faithful ports of the reference functions.
3. **D6 ACP management** (`server.listAcpRegistrySessions`, `importAcpRegistrySession`, `deleteAcpRegistrySession`, `listAcpRegistryProviders`, `setAcpRegistryProvider`, `disableAcpRegistryProvider`, `logoutAcpRegistry`, `acceptAcpRegistryUrlAuth`); the clone has only search, prepare and uninstall (`providers.ts:231, 342, 356`).
4. **D13 custom model options editor**; it replaces the inline rename (`providers.contract:487-510`, `providers.ts:418-438`), which becomes the editor's "Display name" field.
5. **ACP icons:** drop `registryIconUrl` and the agent-id URL into every instance icon (9 `DriverMark` sites in 5 Contract files; `registryIconUrl` is saved at `providers.ts:459` but never drawn).

Excluded: sign-in and install (`20261005-provider-sign-in-and-install`), Codex managed flow (`20261005-managed-codex-chatgpt`), the WSL per-environment update rows (`ProviderUpdateEnvironmentRows`, `ProviderUpdateLaunchNotification.environments.ts`), the sidebar update pill (done: `sidebar-provider-pill.ts`; its reference tests are mapped by `20261005-reference-logic-tests-done-areas`), email redaction (`20261005-provider-sign-in-and-install`), the ChatGPT account instance id and the Add ChatGPT account dialog (`20261005-managed-codex-chatgpt`).

PR #238 handoff (2026-10-08): the reference
`ProviderSettingsPanel.environment.test.tsx:584` URL-auth action case was excluded from
provider-sign-in-and-install. It remains owned here by the URL auth acceptance row below;
see [verification follow-up](../20261008-provider-sign-in-verification-followup.md).

## Context and guidance

Parent specification: [spec](../../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `W/settings/ProviderInstanceCard.tsx:772-870` (popover), `:994-1011` (URL auth), `:1111-1119`; `W/ProviderUpdatesAction.tsx:24-112`; `W/ProviderUpdateLaunchNotification.logic.ts` (`collectProviderUpdateCandidates` `:156`, `canOneClickUpdateProviderCandidate` `:200`, `getProviderUpdateProgressToastView` `:285`, `getProviderUpdateRunToastView` `:345`, `collectUpdatedProviderSnapshots`, `firstFailedProviderUpdateMessage`); `W/ProviderUpdatePrimaryNotification.tsx:100-330`; `W/settings/AcpSessionManagementSection.tsx:41-569`; `W/settings/ProviderSettingsPanel.tsx:639-664`; `W/settings/CustomModelEditor.tsx:46-389`; `W/settings/customModelEditor.logic.ts:41-274`; `W/settings/ProviderModelsSection.tsx:564-587`; `W/settings/AcpRegistryIcon.tsx:73-170`; `W/chat/ProviderInstanceIcon.tsx:56-100`; `packages/contracts/src/acpRegistry.ts:17-44`; `packages/contracts/src/rpc.ts:446-456`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (keep editor drafts separate from saved values; mutation then refresh), layout-and-interaction (a nested press inside a row button; native selects), design (all states), accessibility (icon buttons, expanded state), motion (popover open/close; reduced motion), testing-and-debugging. Unknown in the library: app-local Swift modules, remote image loading policy, popover alignment. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed constraints: the reference list row is one full-width button with the update icon laid over it (`pointer-events-auto`); the clone's row is one `button` (`providers.contract:173`), so the icon must become a sibling pressable. Other environments are already live: `EnvironmentFleet` keeps each switched-on environment's `config` (with `providers` and their `updateState`) through `subscribeServerConfig` (`settings-b-fleet.ts:12-18, 138-146`), and `EnvironmentFleet.native(native, key)` addresses its transport (`connections.ts:248`; `connections.ts:422-435` requests each target). Update all reads those snapshots and sends `server.updateProvider` on each environment's transport. `AcpRegistryIcon.tsx` fetches with credentials omitted, no referrer, redirects refused, a 512 KB cap and a persistent cache; the clone's `image <url>` policy is unknown.
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (popover/tooltip Contract). With `20261005-hot-file-split` merged, ops go into area files, not `client.ts`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](../20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (editor mounting, fixture, open-URL op) | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X17](../../issues/20261005-x17-popover-position-try.md), [X9](../../issues/20261005-x09-root-component-across-files.md), [X21](../../issues/20261005-x21-two-way-websocket.md), [X10](../../issues/20261005-x10-text-rendering-parity.md), [X13](../../issues/closed/20261005-x13-hover-keys-during-pan.md), [X35](../../issues/closed/20261005-x35-secure-text-entry.md), [X44](../../issues/20261005-x44-remote-image-policy.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X17 | Popover `align="end"` / side areas | Reference popover is `side="bottom" align="end"`; main places popovers above or below the invoker | nonblocking (workaround: anchor below, align as available; declare the difference) | Measure at `prepare` |
| X9 | Root cap | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines | nonblocking until the cap | Extend `providerPage`; editor drafts in child components; no new resource |
| X21 | RPC from a data module | Existing Swift transport | nonblocking | none |
| X35 | Secure (password) text entry in Contract | The Agent providers headers field is a write-only `type="password"` input in the reference (`AcpSessionManagementSection.tsx:551-559`) | nonblocking (workaround: the existing `type="password"` input, `providers-wizard.contract:333`) | Check at `prepare` 2026-10-07: #134 closed by main #167: a Contract password field's value is masked in `tree`, `layout` and the `type` reply, and `autocomplete` sets its content type; the app's state and data module stay outside that (adopt-main-fixes-input). |
| X10, X13 | Text truncation; hover during a pan | Row text, popover trigger tooltip | nonblocking | Cite in the pixel matrix |
| [X44](../../issues/20261005-x44-remote-image-policy.md) | Remote `image` loading policy (no credentials, no redirects, size cap, persistent cache, load state, remote SVG) | Reference `AcpRegistryIcon.tsx:7-70,128-136`, `acpRegistry.ts:13-44` | nonblocking (same icon shows in the happy path; policy differences declared) | Check what `image` does at `prepare`; apply the X44 adoption steps when it lands Update 2026-10-07 (record only; task on hold): main #177 adds `load`/`error` on `image` and documents the fetch (no cookie, no Referer, redirects followed, 64 MiB cap, disk cache); an SVG icon is still an `error` on Apple, so AcpRegistryIcon's SVG icons need the `error` fallback when this task resumes. |

## Implementation notes

- Port, with provenance headers and listed changes: `provider-updates.ts` (all functions named above, minus the WSL grouping), `acp-sessions.ts` (state machine of `AcpSessionManagementSection`: single in-flight, project lock, cursor paging, header JSON validation message "Headers must be a JSON object with string values."), `custom-model-editor.ts` (`customModelEditor.logic.ts` whole), `acp-icons.ts` (`resolveOfficialAcpRegistryIconUrl`, `officialAcpRegistryIconUrlForAgentId`).
- Replace `updateCandidates` / `updateKey` / `updateToastView` in `shell-notify.ts` with the ports; keep `providerUpdateDismissals` storage (`shell-prefs.ts`). The update toast changes kind in place: `updateToast` (`toast.ts:39`) patches only title, description, details, expandLabels, action. Extend it additively with kind and timeout (3 s for success).
- Contract: `providers-upkeep.contract` for the popover, ACP section and the editor; the model row pencil is `Edit <slug>` with tooltip "Edit name and options". Escape cancels the editor.
- The editor writes `config.customModels[].capabilities.optionDescriptors` through the existing atomic upsert (`providers.ts:242`); ACP stores plain slugs (`providers.ts:435`) and keeps that rule.
- States: loading ("Loading", "Updating"), empty ("No custom options. The composer uses the provider's default options."), error toasts (titles in `AcpSessionManagementSection.tsx` `reportFailure` calls), disabled (read-only, pending, `required` provider), hover and keyboard focus, destructive confirms. `aria-label`: "`<title>` — view details", "Copy update command", "Project for ACP sessions", "`<providerId>` protocol", "`<providerId>` base URL", "`<providerId>` write-only headers JSON", "Copy options from a built-in model", "Option id", "Option label", "Option type", "Choice value", "Choice label", "Default choice", "Remove choice", "Edit `<slug>`". Motion: popover fade/scale in the reference; use the app's popover timing and none under reduced motion.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Popover from both places | Lane backend + fixture config overlay (outdated provider with `canUpdate`, `updateCommand`; a `compatibilityAdvisory` warning case; read-only session) | `bun scripts/agent.mjs macos --json tree "tap provider-advisory-list-codex" tree "tap provider-update-now" "clock settle" state logs` | Same popover from list and editor; title, detail, "Install vX" vs "Update now", "or, update manually using" and the command; read-only has no action; running shows "Updating" disabled | macOS, 1280×840 and 840×620 | transcript, screenshots |
| Copy command | Same | Tap Copy update command | `copyText` op with the command; toast "`<Provider>` update command copied" with "Run it in a terminal when you are ready to update." | macOS | logs |
| Update all | Two environments connected; outdated providers on both; one with disagreeing update commands; one failing | Tap Update all | Hidden with no one-click candidate; tooltip lists "machine: providers"; "Updating…"; one toast: success, or "N of M provider updates failed" with one line each | macOS | transcript, trace |
| Toast progress | Fixture `updateState` running → failed / unchanged / succeeded | Start update from the launch toast | Loading → error / warning ("Provider still needs an update") / success (closes after 3 s) in place | macOS | transcript |
| ACP sessions and providers | Fixture ACP instance with sessions and providers | List, Load more, Import (then "Imported"), Delete (confirm), Log out, List providers, Save, Disable (confirm) | RPC payloads as the reference; toasts "ACP session imported", "ACP session deleted", "Logged out of ACP agent", "ACP provider configured", "ACP provider disabled"; project picker locks while pending | macOS | trace, transcript |
| URL auth | Fixture `auth.action` | Tap Continue authentication; then an expired request | Link opens (recorded); `acceptAcpRegistryUrlAuth`; expired gives "Authentication request expired" warning | macOS | trace |
| Custom options editor | Real fixture server, a Codex instance with one custom model | Edit; add Reasoning preset; Copy from a built-in; save; reopen; break validation | Round trip equals the draft; messages "Option 1 needs an id." etc.; `server.getSettings` shows the descriptors; the composer offers them | macOS | settings dump, screenshots |
| ACP icons | Instance with `registryIconUrl` | Open list, picker, wizard | Registry icon where the reference shows it; generic glyph if the URL is not allowed | macOS | screenshots |
| Trace and pixels | Oracle and clone on one lane backend | `target/t3-ui-parity/trace-diff.mjs`; pairs at both sizes, light and dark | Same calls; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | diff, images |
| Real hover and real update `(attended session)` | Disposable CLI on this Mac; real ACP agent with sessions; lane build with `T3_LOCAL_HOME=<lane>/t3-home`, `T3_LOCAL_PORT=<lane port 16xxx>` | Hover the update icon (tooltip), run a real update, import a real session | Tooltip text and delay as the reference; update runs and the row updates | macOS, real input | session notes |
| Ported tests | — | `bun test` | `ProviderUpdateLaunchNotification.logic.test.ts` "provider update launch notification logic" (all 16) and "multi-backend update outcomes" and `isTerminalProviderUpdatePhase` (the WSL grouping describe is n/a); `customModelEditor.logic.test.ts` (7); `ProviderInstanceCard.test.ts` ":20", ":50"; `AcpSessionManagementSection.test.tsx` (5); `AcpRegistryIcon.test.ts` ":105 accepts only credential-free HTTPS URLs on the official CDN" (the cache and blob cases are n/a-ui); `ProviderSettingsPanel.environment.test.tsx` ":285 routes refresh and provider update commands to the selected environment" | macOS host machine | test log |
| Keyboard focus, Escape, reduced motion | Fixture config: outdated provider with `canUpdate`, an ACP agent with sessions, a Codex instance with a custom model | Tab to the update icon in a list row and in the editor; Return; Tab to "Update now" and "Copy update command"; Escape. Then Tab through the ACP section, the options editor and each confirm (Delete session, Disable provider); Escape in each; `prefer prefers-reduced-motion reduce` and reopen the popover | The popover opens from the keyboard in both places, focus moves inside and returns to the icon on Escape; Escape in the options editor cancels without saving; Escape in a confirm keeps the item and sends nothing; a visible focus ring on every control; popover open and close are instant under reduced motion | macOS | transcript |
| Gates | `git add -A` | Clone checks, `bun scripts/caps.mjs`, the five repository checks | Green; every moved cell is fixed, or declared in `EXACT2-GAPS.md` with an issue link | macOS | logs |

Task-owned source paths: `C/provider-updates.ts`, `C/acp-sessions.ts`, `C/custom-model-editor.ts`, `C/acp-icons.ts`, `C/providers-upkeep.contract`, `C/providers.ts`, `C/providers.contract`, `C/provider-icons.contract`, `C/shell-notify.ts`, `C/shell-commands.ts`, `C/toast.ts`, `C/providers-meta.ts`, their `*.test.ts`.
Required environment: Xcode 27.0, pinned Bun and Hermes, oracle build, isolated lane backend; attended row: a disposable CLI and an ACP agent (names only). Never port 3773, `~/.t3` or the `t3code` scheme. Attended and normal-launch rows run a lane build with `T3_LOCAL_HOME=<lane>/t3-home` and `T3_LOCAL_PORT=<lane port 16xxx>`; dev and lane builds refuse the real `~/.t3` and port 3773 by default (`20261005-embedded-server-runtime`).

## Progress

2026-10-06: on hold (user decision: tasks that need a sign-in waited). 2026-10-07: the user lifted the hold. Rows that need a real account are signed in by the user in person on the lane build; every other sign-in row uses lane fixtures.

2026-10-08: implemented in [PR #251](https://github.com/ccheever/exact2/pull/251) (draft), all five scope items, from `d82fb6a47`.
- **A9 popover** (`providers-upkeep.ts` `advisoryView`, `providers-upkeep.contract` `ProviderVersionAdvisory`):
  one node for the list row (now a select button under the row's content, the icon its own
  pressable) and the editor header; `getProviderVersionAdvisoryPresentation` ported; "Install vX"
  only with `canInstallVersion` and a message, "Update now" only for
  `isProviderSettingsUpdateCandidate`; read-only hides the action; "Updating" (spinner, still
  under reduced motion) from a local pending set or `updateState` queued/running; "or, update
  manually using" and the command with Copy ("Copy command" tooltip). The editor's inline card
  is gone.
- **D12 Update all and toasts** (`provider-updates.ts`, `provider-update-notify.ts`): the
  reference logic ported whole (minus the WSL grouping); "Update all" in the Providers header over
  the focused environment and every connected background environment, tooltip
  "machine: providers", "Updating…", one toast (`getProviderUpdateRunToastView`). The launch prompt
  is ProviderUpdatePrimaryNotification: Update closes the prompt and runs the one-click providers
  on the primary, then one outcome toast from `updateState` (failed / "Provider still needs an
  update" / "Provider updated", 3 s). The old `shell:provider-update` and the partial
  `updateCandidates`/`updateKey`/`updateToastView` are removed. Updates run on their own queue
  mutation (`providerUpdateRun`, app.contract) so Settings stays usable, with a 15-minute request
  deadline (`T3Transport.swift` accepts up to 900 s when asked; a deadline sends Interrupt, which
  would stop the update on the server).
- **D6 ACP management** (`acp-sessions.ts`, `AcpManagement`): Native sessions (Log out, project
  picker locked while a project request is pending, List/Refresh, Import → "Imported", Delete
  with the dialogs.confirm copy, Load more by cursor) and Agent providers (protocol, base URL,
  write-only headers with "Headers must be a JSON object with string values.", Save, Disable with
  confirm); the "Continue authentication" link opens the URL (`remoteEditorsOpen`) and sends
  `server.acceptAcpRegistryUrlAuth`; `accepted:false` warns "Authentication request expired".
- **D13 custom model options** (`custom-model-editor.ts`, `CustomModelEditor`): the logic whole,
  `readCustomModelEntries`/`toCustomModelSetting`/`deriveProviderModelsForDisplay`; the pencil
  opens the editor under its row (replacing the inline rename); Save writes through the instance
  upsert; ACP stores plain slugs. Escape: while the editor is open, Settings' Back gives up Escape
  (`providerPage.escapeOwned`), so the editor's Cancel takes it.
- **ACP icons** (`acp-icons.ts`, `AcpRegistryAgentIcon`): `registryIconUrl` or the agent id's URL,
  allow-listed, at every `DriverMark` (9 sites) and in the wizard's results; the ACP glyph while
  loading and after an error. On Apple an SVG is a load error (X44 residual, [#121](https://github.com/ccheever/exact2/issues/121)),
  so registry icons (all `.svg`) show the glyph there.
- Found on the live drives and fixed: Escape in a Settings popover also pressed Settings' Back
  (the lower node id wins among `aria-keyshortcuts`): the popover and the select popups are
  `aria-modal` with their own Escape, which returns focus to the trigger (re-driven: Settings
  stays, focus on the icon); the popup's 84% glass let text show through: 95% plus
  `backdrop-filter`. The end alignment by margins did not apply on the approved session's first
  drive (the Apple host passes a popover's margins to its placement only when it names a
  `position-area`), so the popover says `position-area="bottom span-right"` (D2's own placement)
  and the margins now put its right edge at the trigger's (`{code_sha[:9]}`, re-driven at 1280×840,
  840×620 and dark; the independent review measured the right edges equal within a point on the
  screenshots: the drive's frame reads printed no coordinates). The hover tooltip closes when its
  trigger is pressed (Base UI's closeOnClick).
- Known after review (not changed: no live session left to verify a change): nothing pulls the
  list-row popover back from the left window edge (the Apple host clamps the margin box, which is the
  trigger's width), so with the Settings nav collapsed below about 1,240 points its left part is cut
  off (declared, X17); a candidate fix for the list mode only is `position-area="bottom"` with
  `margin-right="18.75rem"` (a 620-point margin box centred on the trigger keeps the right edges
  together and lets the clamp work), to check on both hosts. The press also clears the trigger's own
  hover highlight until the pointer leaves (one `over` state drives both); a separate tooltip-closed
  state would match Base UI. The web target was not driven for this popover.

Decided 2026-10-08 (user: match the original; [provisional-decisions-parity](20261008-provisional-decisions-parity.md)): (1) the launch prompt and the sidebar pill
read the primary's providers only; with no primary (the Local environment off, a remote-only or refused
launch) there is no prompt, as the reference mounts it only for an authenticated primary
(`__root.tsx:247`, `ProviderUpdatePrimaryNotification.tsx:101-102`); (2) no "Updating" toast while an
update runs (`shouldShowPrimaryProviderUpdateToast`, verified: the clone matched); (3) a custom ACP model
is stored with `toCustomModelSetting` like every driver (`ProviderInstanceCard.tsx:673-681`): an added
model is its slug; a name or options make an entry, which the server's ACP schema (a string list) refuses
when it next loads the instance, as for the reference.

2026-10-08 (real-input batch, records PR): popover, keys and the tapped real update pass; the update-icon tooltips are clipped (clone bug). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `321dc8189` on `d82fb6a47` | `bun test examples/t3-code` 2575 pass / 0 fail (base 2512); strict `tsc` clean; contract build 2691 slots, 46 resources (`app.contract` 1,500 lines); `cargo test -p t3-code-macos --lib` 11 pass; caps within; five checks: build, test (3383 pass / 0 fail / 33 ignored, 94 binaries), clippy, fmt, boot green | [evidence](https://github.com/ccheever/exact2/tree/t3-code-evidence/provider-settings-upkeep): 7 images, drive records, RPC log | rows below |
| live drive, try 1 | branch bundle, lane fixture | Pairing failed ("cannot decode raw data"): the fixture proxy forwarded `content-encoding: gzip` on a body Bun had decoded. Screenshots deleted (they showed a pairing token); record kept | `after-drive-attempt1-record.txt` | fixture fixed |
| live drive, try 2 (the retry) | branch bundle (before the Escape/alignment fixes) | Paired Mac Studio and Laptop; the launch prompt; providers list; list popover (title, detail, Update now, divider, command); URL auth (accept, then "Authentication request expired"); ACP List, Load more, Import ("Imported", toast), Delete confirm. Then Escape in the popover closed Settings (bug, fixed) and Escape aimed at the inert Delete button left the confirm open, so the later scenarios found nothing; the Update all press landed on the toasts covering the header (no RPC) | `after-drive-record.txt`, `rpc-log-live.txt`, images | one more session |
| approved session (2026-10-08), branch drive | `3a47f48f5` bundle with the Escape fixes; agent mode, 1280×840, fixture `target/upkeep-lane` (`drive2.mjs`) | Every row on the list: launch prompt → Update (codex succeeded, claude unchanged; the next Claude prompt comes to the front); Update all tooltip and press on both machines → one toast "2 of 3 provider updates failed" with a line each; list and editor popovers; Escape keeps Settings and returns focus; Copy (clipboard read back); Update now; ACP URL auth, sessions (List, Load more, Import, Delete: Escape keeps, Confirm deletes; Log out), providers (invalid headers, Save, Disable confirm); custom model editor (validation, Copy from, Save, reopen, Escape cancels) and the composer's options; 840×620; dark; wizard icons. Found: the popover was not end-aligned (fixed, below); Return is `Enter` to the driver | [record](https://raw.githubusercontent.com/ccheever/exact2/401c1dd7ded2d23c471fca8f6c42c8743aad69e4/provider-settings-upkeep/final/after-drive-record.txt), [RPC log](https://raw.githubusercontent.com/ccheever/exact2/401c1dd7ded2d23c471fca8f6c42c8743aad69e4/provider-settings-upkeep/final/rpc-log-session.txt) | — |
| same session, base drive | evidence base `da4f4512f` (no build) | Same steps on the base for the before images | [record](https://raw.githubusercontent.com/ccheever/exact2/401c1dd7ded2d23c471fca8f6c42c8743aad69e4/provider-settings-upkeep/final/before-drive-record.txt) | — |
| same session, part 2 | `d5e052389` bundle (`drive3.mjs`) | Popovers end-aligned (list, editor, 840×620, dark); tooltip closed on press; keyboard, read from `state`: Enter on the focused icon opened the popover, Tab moved the focus to Update now, Escape returned it to the icon (the screenshot after Tab is byte-identical to the click's: no ring drawn). The step that pairs server c for a tapped real update stopped on the driver: `clock: the clock cannot go backwards (59100.0 → 59099.99999999999)` (framework, below); not retried (no retry approved) | [record](https://raw.githubusercontent.com/ccheever/exact2/401c1dd7ded2d23c471fca8f6c42c8743aad69e4/provider-settings-upkeep/final/after-drive-part2-record.txt) | real-input batch |
| same session, real update | server c (`t3` 0.0.46-nightly, isolated HOME/CODEX_HOME/XDG/T3CODE_HOME on 16012), Codex 0.160.1 in `target/upkeep-lane/servers/c/npm` | The RPC the app's Update now sends (`server.updateProvider {instanceId, provider}`, per the RPC log) on the real server: it ran `npm install -g --prefix <lane prefix> --allow-scripts=@openai/codex @openai/codex@latest`, `updateState` succeeded "Provider updated." in 3 s, Codex 0.161.0, advisory cleared. `/opt/homebrew/bin/codex` stays 0.151.0 | [record](https://raw.githubusercontent.com/ccheever/exact2/401c1dd7ded2d23c471fca8f6c42c8743aad69e4/provider-settings-upkeep/final/real-update-rpc.txt) | — |
| checks and review of the fix | `d5e052389` (a Contract-only change) | Recipe runner (source fingerprint of the task-owned files, matched on the commit): `bun test examples/t3-code` 2576 pass / 0 fail, `bun scripts/caps.mjs` within, the macOS bundle builds, `cargo test -p t3-code-macos --lib` 11 pass; overall `blocked` only by the deferred real-input row. Independent review: no blocking finding; non-blocking ones recorded in Progress (left-edge clipping, the hover highlight, the web not driven) and in the wording of STATUS and this record | local runner reports (not published) | real-input batch |


## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| 5. Real hover tooltips (list icon, editor icon, Update all) | Update all PASS; list and editor icons FAIL (clone bug): the tooltip opens above the icon and is clipped by the card | [psu-01-hover-list](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/01-psu-01-hover-list.png), [psu-01b-zoom](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/02-psu-01b-zoom.png), [psu-02-zoom](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/03-psu-02-zoom.png), [psu-03-hover-updateall-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/04-psu-03-hover-updateall-crop.png) |
| 6. Click → popover end-aligned; Escape; Return/Tab/Escape with the ring | PASS | [psu-04-popover-list](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/05-psu-04-popover-list.png), [psu-05-escape-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/06-psu-05-escape-crop.png), [psu-keys-strip](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/07-psu-keys-strip.png) |
| 7. Tapped real update (lane npm Codex 0.160.1) | PASS: "Updating" then v0.161.0, icon gone; one `server.updateProvider` (5.1 s) | [psu-09-editor-popover](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/08-psu-09-editor-popover.png), [psu-update-pair](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/09-psu-update-pair.png) |

Full record: [provider-settings-upkeep.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/provider-settings-upkeep/provider-settings-upkeep.txt).

## Next action

Acceptance rows (2026-10-08, after the approved session):
- **Pass, live (agent mode, macOS) and tests:** Popover from both places (list and editor, 1280×840,
  840×620, dark; read-only and the "Updating" state by tests: the driver's tap returns after the
  update's answer, so a running state is not visible to it), Copy command, Update all (two machines,
  a failure line each), Toast progress (the reference flow: prompt → Update → outcome from
  `updateState`), ACP sessions and providers, URL auth, Custom options editor (and the composer
  offering the options), ACP icons (declared SVG fallback), Keyboard/Escape/reduced motion by the
  driver's keys read from `state` (Enter, Tab, Escape; the spinner stills under reduced motion), Ported tests, Gates.
- **Real update:** pass on the server path: the real `server.updateProvider` on a lane-local Codex
  install (above). The tap that sends it is live on the fixture; the two in one drive are in the
  batch below (the drive step stopped on a driver error and was not retried).
- **Deferred to the real-input batch — screen locked (user away):** real hover (tooltip text and
  delay on the list icon, the editor icon and Update all), real keys and a visible focus ring
  (the driver's screenshots show no ring on the popover's buttons; check with real Tab), and the
  tapped real update on server c.
- **Blocked:** a real ACP session import (the user chose not to sign in to Gemini CLI or Antigravity).
- **Not run:** Trace and pixels (user decision 2026-10-06: no oracle or trace tools).

Framework problem (filed 2026-10-08 as [#285](https://github.com/ccheever/exact2/issues/285), reproduced on main `0365ad1a4`): `scripts/agent.mjs` `clock "+N real"` (around line 1240) sent a
`to` a hair below the macOS host's clock and the host refused it (`host/apple/Sources/ExactKit/Agent.swift:521`:
`the clock cannot go backwards (59100.0 → 59099.99999999999)`). Seen once, 19:19:18Z in
`after-drive-part2-record.txt`, in a `settle` (`clock "+600 real"` then `clock settle`) after a
refused tap; floating-point drift between `s.now + N` and the host's clock.

Next: the real-input batch below, then review and merge the PR. The feature branch is merged up
to `220adb16f` (#247 through #256, managed Codex included; conflicts kept both sides, listed in
the PR); `app.contract` has no same-file `use` lines left to merge for a later cut.

### Real-input batch steps

Local apparatus (uncommitted) in this worktree's `target/upkeep-lane`: `fixture.mjs`, `probe-c.mjs`,
`update-c.mjs`, the lane app `app/T3 Code (Lane PSU).app` (bundle id
`com.exact.t3code.macos.lanepsu`, ad-hoc signed copy of the PR head's bundle, refreshed after the 2026-10-08 merges; rebuild and copy again if the head moves). Steps:

1. `export PATH="$HOME/.bun-1.4.2/bin:$PATH"; cd target/upkeep-lane; bun fixture.mjs serve` in the
   background (record its PIDs; servers 16010–16012, proxies 16020 "Mac Studio" and 16021), then
   `bun fixture.mjs seed` and `curl -X POST http://127.0.0.1:16020/__fixture/reset`.
2. For a tapped real update, put server c's Codex back one version first:
   `PATH=$PWD/servers/c/bin:/usr/bin:/bin HOME=$PWD/servers/c/home npm install -g --prefix $PWD/servers/c/npm @openai/codex@0.160.1`,
   then `bun probe-c.mjs refresh` must show `behind_latest`.
3. Take the lock `…/t3-code/target/t3-ui-parity/lanes/.realinput-lock` (owner note
   "provider-settings-upkeep: real input"). Launch the lane app by path with
   `HOME` and `CFFIXED_USER_HOME` set to `$PWD/apphome-real`; record its PID.
4. Pair: `bun fixture.mjs pair a` writes `pair-a`; put its URL in the welcome's pairing field with
   `orca computer set-value` (the input source is Korean 2-Set: never type it), press Pair, continue
   through the welcome, close the toasts. Open Settings › Providers.
5. Real hover: move the pointer onto the Codex row's update icon (`orca computer` move/scroll at
   its center), wait 1 s, screenshot: tooltip "Update available"; the same on the editor's icon and
   on "Update all" ("Mac Studio: Codex, Claude").
6. Click the icon: the popover opens end-aligned under it and the tooltip closes. `press-key
   Escape`: the popover closes, Settings stays, the icon shows a focus ring. `press-key Tab` until the
   icon has the ring, `press-key Return`: the popover opens; `press-key Tab`: the ring is on "Update
   now"; `press-key Escape`: back on the icon. Read each back by screenshot.
7. Tapped real update: Settings › Connections › Add, `bun fixture.mjs pair c`, set-value the
   `pair-c` URL, Connect; choose server c in "Applying settings … on"; Codex's editor icon →
   Update now: "Updating" with the spinner, then the new version and no icon. Read back with
   `bun probe-c.mjs` (version, `updateState.status: succeeded`).
8. Release the lock (only with your owner note). Kill the recorded PIDs (the app, the fixture).
   Delete the lane app's Keychain item (service `com.exact.t3code.macos.access-token`, account
   naming `127.0.0.1:1602x` or `:16012`) if one was written, and its preferences under `apphome-real`.
