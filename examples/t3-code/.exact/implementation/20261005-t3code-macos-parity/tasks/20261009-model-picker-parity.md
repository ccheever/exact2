---
name: 20261009-model-picker-parity
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

# Model picker: search, Shift+click, provider keys, jump badges, rail labels, names, and the two Settings pickers

## Outcome

The provider model picker behaves as the reference's in the composer:
- search highlights the first match and Enter picks it;
- Shift+click adds or removes a model and keeps the picker open;
- ⇧⌘↓/⇧⌘↑ skip providers that are not ready;
- rows show the ⌘1–⌘9 badges;
- an unavailable provider's rail button names its state and reason, with a tooltip;
- the trigger and the Plan toggle carry the reference's accessible name and pressed state.

Settings › Scheduled Tasks › New task › Model and Settings › Source Control's writer model use the same picker.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed. Provider state differs between the
audit lanes (see [composer-provider-state-and-details](20261009-composer-provider-state-and-details.md), "Lane setup").

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| CO-3 | Typing "opus" highlights the first result (Combobox `autoHighlight`). Enter selects it and closes the picker. | After "opus" no row is highlighted (`model-picker.contract:109-111`: highlight is -1 until ArrowDown). Enter leaves the picker open and the model unchanged. | Open the picker on the work draft, type "opus", press Enter. | `target/t3-audit/evidence/composer/CO-3-ref.png`, `CO-3-clone.png`, `composer/dumps/clone-search-enter.tree.txt` |
| CO-7 | Shift+click on a second model adds it (check marks on both rows, trigger "Claude Fable 5.1, Claude Sonnet 5.5"). The picker stays open; Shift+click again removes it. | Shift+click adds the model and the label matches, but the picker closes at once. | Open the picker on the work draft, Shift+click Claude Sonnet 5.5. | `target/t3-audit/evidence/composer/CO-7-ref.png`, `CO-7-clone.png`, `composer/dumps/ref-multi.aria`, `composer/dumps/clone-multimodel.tree.txt` |
| CO-4 | From Favorites, ⇧⌘↓ skips the disabled Codex button and selects Claude (`adjacentModelPickerProvider` keeps picker-ready or selectable-unavailable instances). | ⇧⌘↓ selects the disabled Codex rail button and the list says "No models found" (`keyboard-dispatch.ts:231-235` builds the rail from every provider). | Favorite one model, open the picker, select Favorites, press ⇧⌘↓. | `target/t3-audit/evidence/composer/CO-4-ref.png`, `CO-4-clone.png`, `composer/dumps/clone-nextprov.tree.txt` |
| CO-1 | The first nine selectable rows show a "⌘1"…"⌘9" badge before the star, also in search results and Favorites. | The rows carry `aria-keyshortcuts` Meta+N and the keys work, but no badge is drawn. | Open the picker on the work draft; compare the right side of each row. | `target/t3-audit/evidence/composer/CO-1-ref.png`, `CO-1-clone.png` |
| CO-2 | The disabled Codex button's hover and aria-label: "Codex — Unavailable. Codex CLI is not authenticated. Run `codex login` and try again." (the kind and the provider message). Ready buttons have the display name as tooltip. | aria-label "Codex — Not ready." with no kind or message. No rail button has a hover tooltip. | Open the picker, hover the Codex rail button. | `target/t3-audit/evidence/composer/CO-2-ref.png`, `CO-2-clone.png`, `composer/dumps/clone-model-picker.tree.txt` |
| CO-11 | The picker button's name is the model names ("Claude Fable 5.1"). The Plan toggle exposes `aria-pressed` ("Plan mode — click to return to normal build mode", pressed). | The picker's name is always "Choose provider and model" (`composer-controls.contract:194`). The interaction-mode button has the label but no pressed state. | Read the composer's accessibility tree on the work draft; turn Plan on. | `target/t3-audit/evidence/composer/CO-11-ref.png`, `CO-11-clone.png`, `composer/dumps/ref-plan-on.aria` |
| S2-4 | New task › Model and Source Control's writer model both use `ProviderModelPicker`: a "Choose model" trigger when nothing is available; a popup with the Providers rail (Favorites, Codex, Claude with "Unavailable…" reasons), "Search models...", "No models found", favorites and legacy grouping. | Both are a plain listbox of every model ("Claude Opus 5.5 · Claude", …) with no search, rail, favorites or reasons. The 2026-10-07 task fixed only General's two pickers. | Settings › Scheduled Tasks › New task › Model. Settings › Source Control › "Use a separate source control writer model" › open the picker. | `target/t3-audit/evidence/settings-2/S2-4-ref.png`, `S2-4-clone.png`, `S2-4b-ref.png`, `S2-4b-clone.png`, `settings-2/ref/st-model.aria` |

## Scope and exclusions

Included: the seven findings above.

Excluded:
- The composer's provider state and model resolution: [composer-provider-state-and-details](20261009-composer-provider-state-and-details.md).
- The search field's focus ring (CO-12): X61, waits for main fix of #302.
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975/apps/web/src/components`):
- `chat/ModelPickerContent.tsx:118-144` (`adjacentModelPickerProvider`), `:610-635, 890-896, 955` (additive picks),
  `:651-749, 1032` (jump labels), `:855-900` (search, `autoHighlight`).
- `chat/ModelListRow.tsx:42, 107` (`jumpLabel`); `chat/ModelPickerSidebar.tsx:15-31, 150-200` (rail labels and tooltips).
- `chat/ProviderModelPicker.tsx:208` (aria-label = all model names); `chat/ChatComposer.tsx:1266` (Plan `aria-pressed`).
- `settings/ScheduledTasksSettings.tsx:798-811`; `settings/SourceControlWritingSettings.tsx:302-310`.

Clone (`examples/t3-code`): `model-picker.contract:59-73, 107-152, 180-182, 190`; `keyboard-dispatch.ts:228-238`
(`modelPickerRows`); `composer-controls.contract:194`; `r3-composer-controls-fanout.ts`; `settings-scheduled.contract:397`
(`GhostSelect task-model`); the Source Control writer model listbox (`settings-source-control.contract`). The closed
[settings-model-picker](closed/20261007-settings-model-picker.md) put the provider picker in General: reuse its pattern.

Shared files:
- `keyboard-dispatch.ts` with [shell-sidebar-palette-keys](20261009-shell-sidebar-palette-keys.md).
- `r3-composer-controls-fanout.ts` with [composer-provider-state-and-details](20261009-composer-provider-state-and-details.md).
- `settings-scheduled.contract` and `settings-source-control.contract` with [settings-rows-and-labels](20261009-settings-rows-and-labels.md).

Start after those merge, or rebase.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| CO-3 | Type "opus": the first row is highlighted. Enter picks it and closes the picker; the trigger changes. | `co3-search-enter.png` | agent |
| CO-7 | Shift+click adds a second model and the picker stays open; Shift+click again removes it. | `co7-shift-click.png` | agent |
| CO-4 | From Favorites, ⇧⌘↓ selects Claude and skips the disabled Codex. Unit test on the rail order. | `co4-next-provider.png` | agent |
| CO-1 | The first nine selectable rows show ⌘1…⌘9, in search results and Favorites too. | `co1-jump-badges.png` | agent |
| CO-2 | The Codex rail button's name and hover tooltip carry the kind and the provider message; ready buttons show the name. | `co2-rail-tooltip.png` | agent (hover) |
| CO-11 | The picker's accessible name is the model names; the Plan toggle reports pressed (tree). | text: tree before/after | agent |
| S2-4 | New task › Model and the writer model open the provider picker (rail, search, favorites, reasons, "Choose model" when none). | `s2-4-new-task-model.png`, `s2-4-writer-model.png` | agent |

## Next action

Prepare a branch from `feat(example)/t3-code` after the shared-file tasks above. Build and unit-test. Then do one batched
live drive at the end for every row's before/after pair. Close every row in this PR, or record the blocker of a row that
cannot pass.
