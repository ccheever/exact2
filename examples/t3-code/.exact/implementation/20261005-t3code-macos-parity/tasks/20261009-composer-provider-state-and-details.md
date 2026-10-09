---
name: 20261009-composer-provider-state-and-details
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Composer provider state and the thread details card

## Outcome

- The composer decides "no provider" and "which model" by the reference's rules:
  - the placeholder says "Enable a provider in Settings to send a message" when no provider can run the turn;
  - a thread locked to an enabled but failing provider keeps its model picker, mode and access controls;
  - a thread whose stored model is not in the catalog shows and sends the provider's default model.
- The thread details card follows the composer:
  - multi-model's forced New worktree shows in the Workspace row;
  - "Add project script" opens the Add Action dialog in place;
  - an unstarted server thread offers the Workspace select.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

Lane setup: `clone-drive.sh` did not isolate HOME and PATH for the clone's embedded server, so the clone found a
signed-in Claude and Codex 0.160.1 while the reference found no Claude and Codex 0.151.0. For CO-9 and CO-10 the composer
auditor set Claude's binary path to `/nonexistent/claude` and left Codex not authenticated. Give the task's lane the same
provider state on both sides (isolate HOME and PATH, or set the binary paths) before comparing.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| CO-9 | With no selectable provider (Claude not found, Codex not authenticated or unsupported) the placeholder is "Enable a provider in Settings to send a message", with the "Open provider settings" control (`noProviderAvailable` from `resolveComposerProviderSelection`). | The control shows, but the placeholder stays "Ask for changes, send follow-ups, or attach images". `composer-presentation.ts:179` sets `providerUnavailable` only when no provider is enabled with a status other than "disabled"; an enabled provider in status "error" counts as available. `providerControl` uses a stricter rule, so the two disagree. | Composer lane. Claude binary `/nonexistent/claude`, Codex not authenticated. Open the new-thread draft for project work. Read the placeholder. | `target/t3-audit/evidence/composer/CO-9-ref.png`, `CO-9-clone.png`, `composer/dumps/clone-noprov-draft.tree.txt` |
| CO-10 | On the fixture thread (Codex enabled, not authenticated) the reference keeps the model picker ("gpt-6-astra"), the runtime mode and the Build toggle. `resolveComposerProviderSelection` accepts a candidate instance that is enabled and not "unavailable", whatever its status. The "Codex is unauthenticated" banner explains the problem. | The whole control row becomes "Open provider settings" (`providerControl` requires enabled, installed, not unauthenticated and status not "error" for every provider). The user cannot see or change the thread's model, mode or access. | Same lane. Open Verification fixture › Timeline verification. Compare the control row. | `target/t3-audit/evidence/composer/CO-10-ref.png`, `CO-10-clone.png`, `composer/dumps/ref-noprov-fixture.aria`, `composer/dumps/clone-noprov-fixture.tree.txt` |
| CO-6 | The fixture thread stores `codex/gpt-5.4`. Codex reports no models, so the reference resolves the selection to the provider default and shows and would send "gpt-6-astra" (`resolveAppModelSelection` → `getDefaultServerModel` → `DEFAULT_MODEL_BY_PROVIDER.codex`). | The trigger shows "gpt-5.4" (the stored slug) and the tooltip "gpt-5.4 · ⇧⌘M". | Open Verification fixture › Timeline verification (Claude ready, Codex not authenticated). Read the model trigger. | `target/t3-audit/evidence/composer/CO-6-ref.png`, `CO-6-clone.png`, `composer/dumps/clone-fixture.tree.txt` |
| CO-8 | With two models selected, the composer strip and the details card both switch to New worktree ("New worktree · Create", "From feature/audit"). | The strip shows "New worktree" and "From feature/audit", but the details Workspace row still shows "work" with Current checkout selected. | On the work draft, Shift+click a second model. Compare the strip and the details Workspace row. | `target/t3-audit/evidence/composer/CO-8-ref.png`, `CO-8-clone.png`, `composer/dumps/clone-multimodel.tree.txt` |
| CO-5 | "Add project script" in the details card opens the "Add Action" dialog in place (Name with icon, Keybinding, Command, Preview URL, three switches, Cancel / Save action). | The row runs `ui:project-settings` and navigates to Settings › Project; the dialog exists only in Settings (`settings-b-actions.contract`). | On the work draft, dismiss the toasts, click "Add project script". | `target/t3-audit/evidence/composer/CO-5-ref.png`, `CO-5-clone.png`, `composer/dumps/ref-add-script.aria` |
| PA-9 | A server thread with no messages and no runtime is not locked. Its Workspace row is a select with "Current checkout" and "New worktree". | Every server thread shows a static folder row. The select appears only on drafts (`shell-details.ts:139`: `envModeSelect: !thread`). | Open a server thread with no messages (work › Audit work thread). Click the Workspace row. | `target/t3-audit/evidence/panel/PA-9-ref.png`, `PA-9-clone.png`, `panel/dumps/r-aria-ws-combo.txt` |

## Scope and exclusions

Included: the six findings above.

Excluded:
- The model picker's own rows (CO-1 to CO-4, CO-7, CO-11): [model-picker-parity](20261009-model-picker-parity.md).
- The composer's focus ring (CO-12): X61, waits for main fix of #302.
- Queue, steer, attachments and the usage meter (the audit could not compare them).
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src`):
- CO-9, CO-10: `components/chat/ChatComposer.tsx:2080-2124, 5375-5389, 7369-7384`; `components/ChatView.logic.ts:567-620`
  (`resolveComposerProviderSelection`); `providerInstances.ts:101, 293-316`.
- CO-6: `modelSelection.ts:274-323`; `providerModels.ts:95-107`; `packages/contracts/src/model.ts:171-182`;
  `composerDraftStore.ts` (`useEffectiveComposerModelState`).
- CO-8: `components/BranchToolbar.tsx`; `components/chat/ThreadDetailsCard.tsx`.
- CO-5: `components/ProjectScriptsControl.tsx:428-469`.
- PA-9: `components/ChatView.tsx:2945` (`envLocked`); `components/BranchToolbarEnvModeSelector.tsx:174-266`.

Clone (`examples/t3-code`): `composer-presentation.ts:61-72, 150-167, 179`; `composer-controls-view.ts:280-289`;
`protocol.ts:115-118`; `shell-details.contract:180` and the `details-workspace` row; `shell-details.ts:139`;
`r3-composer-controls-fanout.ts`; `settings-b-actions.contract` (the Add Action dialog to reuse).

Notes:
- CO-9 and CO-10 share one rule. Port `resolveComposerProviderSelection` with its tests by name, and use it for the
  placeholder and for the control row.
- CO-6: port the model resolution with its tests; the trigger, the tooltip and the sent selection use the resolved model.
- Shared file: `r3-composer-controls-fanout.ts` is also changed by [model-picker-parity](20261009-model-picker-parity.md)
  (CO-7). Merge one before the other starts, or rebase.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| CO-9 | No selectable provider: the draft's placeholder is "Enable a provider in Settings to send a message" and the control shows. Ported tests pass. | `co9-no-provider-draft.png` | agent |
| CO-10 | Fixture thread with Codex enabled but not authenticated: the model picker, mode and Build toggle show, with the banner. | `co10-failing-provider-thread.png` | agent |
| CO-6 | The fixture thread's trigger shows "gpt-6-astra" and its tooltip "gpt-6-astra · ⇧⌘M". A unit test checks the sent selection. | `co6-default-model.png` | agent |
| CO-8 | After Shift+click of a second model, the details Workspace row shows New worktree. | `co8-details-workspace.png` | agent |
| CO-5 | "Add project script" opens the Add Action dialog over the thread. Cancel closes it; Save action saves the script. | `co5-add-action-dialog.png` | agent |
| PA-9 | An unstarted server thread's Workspace row is a select with both options; a started thread stays locked. | `pa9-workspace-select.png` | agent |

## Next action

Prepare a branch from `feat(example)/t3-code`. Set up the lane's provider state. Build and unit-test. Then do one batched
live drive at the end for every row's before/after pair. Close every row in this PR, or record the blocker of a row that
cannot pass.
