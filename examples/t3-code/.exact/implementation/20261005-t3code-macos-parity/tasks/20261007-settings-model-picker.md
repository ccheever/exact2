---
name: 20261007-settings-model-picker
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

# Search, browse and favorite models from General settings

## Outcome

Settings > General uses the reference model picker for New threads > Model and Text generation
model. Users can search models, choose a provider, browse favorites, favorite a model and expand
legacy models before selecting a value for the current settings scope.

This is a discovery record from the 2026-10-07 desktop comparison. No fix is included.
`verification: unverified` describes the future implementation, not the observed discrepancy.

## Observed behavior and reproduction

Reference source: `1e2ecbd9758830669684b494d4398f626b0576e0`.
Clone source: `fbce02624d2e33449ee2cde34497083d6fd47457`.

1. Connect the audit environment advertising Codex and Claude models.
2. Open Settings > General > New threads > Model in each running app.
3. Open Text generation model in the Exact app to inspect the second affected control.

The reference's New threads picker has a Providers toolbar with Favorites, Codex and Claude,
a `Search models...` combobox, per-model favorite buttons and a collapsed `Legacy models`
row. The Exact control opens one flat list of advertised models from both providers. It has
no search field, provider navigation, favorites controls or legacy expansion. Text generation
uses the same flat-list implementation. This is a missing selection workflow, beyond the
popup's appearance.

Local evidence, relative to the checkout root:

- `target/desktop-audit/evidence/ref-settings-general-model-picker.{png,txt}`
- `target/desktop-audit/native/native-settings-general-default-model-model.{png,json}`
- `target/desktop-audit/native/native-settings-general-text-generation-model-model.{png,json}`

The captures are local artifacts and are not committed. The reference Text generation row's
shared picker is confirmed by source; its complete interaction sequence was not run in this audit.

## Scope and guidance

Reference `apps/web/src/components/settings/SettingsPanels.tsx` uses `ProviderModelPicker`
for these rows. Follow `chat/ProviderModelPicker.tsx`, `ModelPickerContent.tsx`,
`ModelPickerSidebar.tsx` and `modelPickerSearch.ts` for selection, search, provider navigation,
favorite storage, legacy grouping, unavailable models and keyboard behavior.

Clone `settings-core.ts` `modelControl()` flattens `providers[].models` into `CoreRow.options`.
`settings-rows.contract` renders those options through `CoreMenu` for `row.kind == "model"`.
The composer already has richer behavior in `model-picker.contract` and `model-catalog.ts`;
adapt that behavior for a settings selection target instead of creating another independent
catalog or favorite store. Keep settings writes routed through their existing scoped command
path, including Mixed values and inheritance. Choosing a settings model must not change the
active thread's model.

Text generation must retain its capability restrictions and disabled reasons. Keep model
selection and the adjacent Traits picker independent; changing a provider or model must apply
the reference's compatible option handling. Use the same advertised catalog in both apps for
comparison. Account authentication is not needed to prove these controls with an isolated lane.

## Dependencies and deduplication

- [Composer fidelity](closed/20261005-composer-fidelity.md) addresses the composer and its overflow;
  it does not cover model selection from Settings.
- [Provider settings upkeep](20261005-provider-settings-upkeep.md) owns the custom model options
  editor, provider updates and ACP management. Selecting an existing default model is separate.
- [Minor UI fixes](closed/20261007-fix-minor-ui-issues.md) includes Settings Traits picker presentation,
  not the missing searchable model picker.
- No framework blocker was demonstrated. Missing controls are implemented in the example's
  composer already. Pure spacing, color and alignment differences remain in the audit's shared
  visual task.

## Acceptance and reproduction

| Criterion | Action | Expected result | Proof |
| --- | --- | --- | --- |
| Provider browsing | Open each General model picker; switch Codex and Claude | Matching provider rail, selected provider and model list | Paired captures |
| Search | Search by model name and provider; clear; enter a query with no matches | Reference-compatible search results, empty state and restored browse state | UI drive |
| Favorites | Add and remove a favorite; open Favorites; reopen the composer and Settings | Shared favorite state persists; favoriting alone does not select a model or write a setting | UI and preference readback |
| Legacy models | Expand and collapse Legacy models; select a legacy entry | Reference grouping, selected value and dismissal | UI and scoped setting readback |
| Scoped selection | Choose models under a project, one environment and All environments | Same intended setting targets, Mixed/inherited state and per-target failure handling | Existing scoped-write tests plus a lane drive |
| Text generation | Use a catalog with an unsupported provider/model | Unsupported choices follow the reference's visibility/disabled rules and cannot be saved | Fixture test and capture |
| Keyboard | Open, search, move through providers/results, choose and dismiss with Escape | Reference keyboard navigation and focus return; current thread selection stays unchanged | Bounded live drive |

## Next action

Prepare a settings adapter for the existing model catalog/picker, then implement and run the
affected app tests and a rebuilt macOS comparison before changing verification status.
