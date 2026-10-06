---
name: 20261005-provider-settings-upkeep
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

## Context and guidance

Parent specification: [spec](../spec.md). Paths: `C/` = `examples/t3-code/`; `W/` = `apps/web/src/components/` at T3 Code `1e2ecbd975`. Line numbers are from the mc-orch tree on 2026-10-05; `20261005-hot-file-split` moves code, so find it by symbol. Tools are named by their `target/t3-ui-parity/…` path (committed under `examples/t3-code/tools/` with the same relative paths, decision U23).
Source behavior: `W/settings/ProviderInstanceCard.tsx:772-870` (popover), `:994-1011` (URL auth), `:1111-1119`; `W/ProviderUpdatesAction.tsx:24-112`; `W/ProviderUpdateLaunchNotification.logic.ts` (`collectProviderUpdateCandidates` `:156`, `canOneClickUpdateProviderCandidate` `:200`, `getProviderUpdateProgressToastView` `:285`, `getProviderUpdateRunToastView` `:345`, `collectUpdatedProviderSnapshots`, `firstFailedProviderUpdateMessage`); `W/ProviderUpdatePrimaryNotification.tsx:100-330`; `W/settings/AcpSessionManagementSection.tsx:41-569`; `W/settings/ProviderSettingsPanel.tsx:639-664`; `W/settings/CustomModelEditor.tsx:46-389`; `W/settings/customModelEditor.logic.ts:41-274`; `W/settings/ProviderModelsSection.tsx:564-587`; `W/settings/AcpRegistryIcon.tsx:73-170`; `W/chat/ProviderInstanceIcon.tsx:56-100`; `packages/contracts/src/acpRegistry.ts:17-44`; `packages/contracts/src/rpc.ts:446-456`.
Library revision: `20261005-platforms-v3`. Selected topics: state-and-data (keep editor drafts separate from saved values; mutation then refresh), layout-and-interaction (a nested press inside a row button; native selects), design (all states), accessibility (icon buttons, expanded state), motion (popover open/close; reduced motion), testing-and-debugging. Unknown in the library: app-local Swift modules, remote image loading policy, popover alignment. The clone's runtime evidence on the pinned main is the basis.
Consumer framework revision and toolchain: main pin from `20261005-clone-on-exact2-main`; Xcode 27.0; pinned Bun 1.4.2.
Observed constraints: the reference list row is one full-width button with the update icon laid over it (`pointer-events-auto`); the clone's row is one `button` (`providers.contract:173`), so the icon must become a sibling pressable. Other environments are already live: `EnvironmentFleet` keeps each switched-on environment's `config` (with `providers` and their `updateState`) through `subscribeServerConfig` (`settings-b-fleet.ts:12-18, 138-146`), and `EnvironmentFleet.native(native, key)` addresses its transport (`connections.ts:248`; `connections.ts:422-435` requests each target). Update all reads those snapshots and sends `server.updateProvider` on each environment's transport. `AcpRegistryIcon.tsx` fetches with credentials omitted, no referrer, redirects refused, a 512 KB cap and a persistent cache; the clone's `image <url>` policy is unknown.
Scheduling preference (not a prerequisite): after `20261005-main-fix-adoption` (popover/tooltip Contract). With `20261005-hot-file-split` merged, ops go into area files, not `client.ts`.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| merged task PR | [20261005-clone-on-exact2-main](20261005-clone-on-exact2-main.md) | pending | Merged | pending |
| merged task PR | [20261005-desktop-oracle-and-trace](20261005-desktop-oracle-and-trace.md) | pending | Merged | pending |
| merged task PR | [20261005-hot-file-split](20261005-hot-file-split.md) | pending | Merged (area files and registration points exist) | pending |
| merged task PR | [20261005-provider-sign-in-and-install](20261005-provider-sign-in-and-install.md) | pending | Merged (editor mounting, fixture, open-URL op) | pending |

## Issue assessment at preparation

Checked sources and time: plan `issues/` drafts read 2026-10-05; no upstream search (no network). Records: [X17](../issues/20261005-x17-popover-position-try.md), [X9](../issues/20261005-x09-root-component-across-files.md), [X21](../issues/20261005-x21-two-way-websocket.md), [X10](../issues/20261005-x10-text-rendering-parity.md), [X13](../issues/20261005-x13-hover-keys-during-pan.md), [X35](../issues/20261005-x35-secure-text-entry.md), [X44](../issues/20261005-x44-remote-image-policy.md).

| Issue / reference | Capability and target | Evidence / affected revision | Impact | Next action |
| --- | --- | --- | --- | --- |
| X17 | Popover `align="end"` / side areas | Reference popover is `side="bottom" align="end"`; main places popovers above or below the invoker | nonblocking (workaround: anchor below, align as available; declare the difference) | Measure at `prepare` |
| X9 | Root cap | `app.contract` 1,327 and `client.ts` 1,455 of 1,500 lines | nonblocking until the cap | Extend `providerPage`; editor drafts in child components; no new resource |
| X21 | RPC from a data module | Existing Swift transport | nonblocking | none |
| X35 | Secure (password) text entry in Contract | The Agent providers headers field is a write-only `type="password"` input in the reference (`AcpSessionManagementSection.tsx:551-559`) | nonblocking (workaround: the existing `type="password"` input, `providers-wizard.contract:333`) | Check at `prepare` 2026-10-07: #134 closed by main #167: a Contract password field's value is masked in `tree`, `layout` and the `type` reply, and `autocomplete` sets its content type; the app's state and data module stay outside that (adopt-main-fixes-input). |
| X10, X13 | Text truncation; hover during a pan | Row text, popover trigger tooltip | nonblocking | Cite in the pixel matrix |
| [X44](../issues/20261005-x44-remote-image-policy.md) | Remote `image` loading policy (no credentials, no redirects, size cap, persistent cache, load state, remote SVG) | Reference `AcpRegistryIcon.tsx:7-70,128-136`, `acpRegistry.ts:13-44` | nonblocking (same icon shows in the happy path; policy differences declared) | Check what `image` does at `prepare`; apply the X44 adoption steps when it lands |

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

Planned. No branch.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Next action

`prepare` after `20261005-provider-sign-in-and-install` merges; confirm the per-environment config read and the image policy first.
