---
name: 20261009-model-picker-parity
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-model-picker-parity
pr_url: https://github.com/ccheever/exact2/pull/374
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
audit lanes (see [composer-provider-state-and-details](closed/20261009-composer-provider-state-and-details.md), "Lane setup").

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
- The composer's provider state and model resolution: [composer-provider-state-and-details](closed/20261009-composer-provider-state-and-details.md).
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
- `keyboard-dispatch.ts` with [shell-sidebar-palette-keys](closed/20261009-shell-sidebar-palette-keys.md).
- `r3-composer-controls-fanout.ts` with [composer-provider-state-and-details](closed/20261009-composer-provider-state-and-details.md).
- `settings-scheduled.contract` and `settings-source-control.contract` with [settings-rows-and-labels](closed/20261009-settings-rows-and-labels.md).

Start after those merge, or rebase.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| CO-3 | Type "opus": the first row is highlighted. Enter picks it and closes the picker; the trigger changes. | `co3-search-enter.png` | agent |
| CO-7 | Shift+click adds a second model and the picker stays open; Shift+click again removes it. | `co7-shift-click.png` | agent, then real (the native press recorder) |
| CO-4 | From Favorites, ⇧⌘↓ selects Claude and skips the disabled Codex. Unit test on the rail order. | `co4-next-provider.png` | agent |
| CO-1 | The first nine selectable rows show ⌘1…⌘9, in search results and Favorites too. | `co1-jump-badges.png` | agent |
| CO-2 | The Codex rail button's name and hover tooltip carry the kind and the provider message; ready buttons show the name. | `co2-rail-tooltip.png` | agent (hover) |
| CO-11 | The picker's accessible name is the model names; the Plan toggle reports pressed (tree). | text: tree before/after | agent |
| S2-4 | New task › Model and the writer model open the provider picker (rail, search, favorites, reasons, "Choose model" when none). | `s2-4-new-task-model.png`, `s2-4-writer-model.png` | agent |

## Cause and fix

- CO-3: the highlight stayed -1 until an arrow key. `model-picker.contract` now follows Combobox `autoHighlight`
  ("input-change"): while a search is on, the first row a person can choose (`catalog.firstEnabled`) is highlighted until
  the arrows move it. Return in the search field chooses the highlighted row (Shift+Return adds it) or opens the legacy
  section; the field's `submit` is gone, the key handler owns Return. Return's target is any highlighted row a person
  can choose, an open Legacy section's rows too (the reference's Enter parses the highlighted model key; review fix).
- CO-7: `selectModel` always closed the picker. A row's press now passes its `shiftKey` (and Return its own); on a draft
  that can start several models (`catalog.multiple`, ProviderModelPicker `onToggleModel`) Shift keeps the picker open and
  sends the toggle as `command("model", …, 1)`. `changeModel` takes that flag; the native gesture read stays as the
  fallback. A settings picker has no `onToggleModel`: Shift picks as a click does. Review fix: every pick takes the
  native press first (T3ComposerIntent hands it out once). Before that, n 1 skipped the read and the press stayed: the
  one model a removal left toggled back in, so removing the draft's original model did nothing, and a ⌘N jump within 2 s
  of a Shift pick added its model. The model a toggle leaves is now a plain pick.
- CO-4: the dispatch built the rail from every instance. `adjacentPickerProvider` (`model-catalog.ts`) ports
  `adjacentModelPickerProvider`: Favorites and the selectable instances only, wrapping, and from a choice outside that list
  down to Favorites and up to the last.
- CO-1: `withJumpLabels` gives the first nine choosable rows (an open Legacy section's rows follow the current ones, as
  `visibleModels`) the jump command's shortcut label and chord from the server's keybindings in the open picker's
  context (`effectiveShortcut`); `ModelListRow`'s Kbd draws it before the star, and the row's `aria-keyshortcuts` is that
  chord.
- CO-2: `railLabel` ports `describeUnavailableInstance` and the locked-thread tooltip; it is each rail button's
  `aria-label` and its tooltip (the window's hover layer, side left), on a wrapper that keeps the hover of a disabled
  button. The rail buttons report `aria-pressed`, as `Toolbar.Button` does.
- CO-11: the trigger's name is the model names (`allModelNames`, or the label it shows), and the Plan toggle has
  `aria-pressed`.
- S2-4: `SettingsPickerTrigger` (`model-picker.contract`) opens the window's picker under Scheduled Tasks' Model and the
  writer model through the root's `pickModel(row, context)`; `settings-model-picker.ts` builds their catalogs
  (`task-model:|<environment>|<model>`: the editing environment's instances, the draft's model or the first instance, no
  setup and no reasons; `source-control-writer-model:|<environment>:<project>`: the text generation instances, the writer
  selection, `useScopedModelDisabledReason` and the setup footer). A task pick is the root's `taskModelPick`, which the
  editor reads until it closes; a writer pick is the page's `rest:scoped` write. The trigger shows the model name, the
  draft's unknown slug, or "Choose model" (`taskModelMarks`). The old listbox and `ScheduledPage.models` are gone. The live
  drive found the task picker drawn under the editor (z-index orders siblings): a Settings row's picker now stacks at 49/50.

## Lane setup

Lane `model-picker-parity` (base port 16380), before lane `model-picker-parity-before`. The reference home and both clone
homes got `providerInstances` with Claude at `~/.local/bin/claude` and Codex at `~/.local/bin/codex` (lane `CODEX_HOME`:
not authenticated), `enableProviderUpdateChecks: false` and `sourceControlWriterModelSelection` claudeAgent /
claude-sonnet-5-5, so all three sides list the same Claude models and a disabled Codex. The after drive ran with a fresh
client store (`CLONE_STORAGE=audit-model-picker-parity-retry`) on its retry. The before build (`t3-code-evidence-base`,
`950e8e2e5`) keeps the picker open on Return and closes it on Shift+click, so its drive was split into sessions where the
two builds diverge; the steps are the same.

## Acceptance results

Before = `t3-code-evidence-base` (`950e8e2e5`); after = this branch's bundle; reference = T3 Code `1e2ecbd975` Electron.
Every row ran in agent mode. CO-7 also needs real input: agent taps never fill the native press recorder that a real
Shift+click fills, and the review found the bug there. See "Real-input batch steps".

| Id | Result | Evidence |
| --- | --- | --- |
| CO-3 | pass: "opus" highlights Claude Opus 5 (before: nothing); Return picks it, the picker closes and the trigger reads "Claude Opus 5", as the reference. | [co3-search-enter.png](https://raw.githubusercontent.com/ccheever/exact2/1b8cc8fd6d1d311457793352032f44a8361809c6/model-picker-parity/co3-search-enter.png) |
| CO-3 (review: legacy rows) | pass: on the Claude list, Return on the Legacy header opens it, ↓ highlights Claude Fable 5, and Return picks it: the picker closes and the trigger is named "Claude Fable 5", as the reference (before: the picker stayed open and nothing changed). | [co3-legacy-return.png](https://raw.githubusercontent.com/ccheever/exact2/162702f3bf6841603fa8d755f75c24c926079ea4/model-picker-parity/co3-legacy-return.png) |
| CO-7 (agent) | pass: Shift+click Claude Sonnet 5.5 checks both rows, the trigger reads "Claude Opus 5, Claude Sonnet 5.5" and the picker stays open (before: it closed); Shift+click again leaves "Claude Opus 5", picker open. | [co7-shift-click.png](https://raw.githubusercontent.com/ccheever/exact2/a103ad2964158408060d150b8a39e500fd9d8069/model-picker-parity/co7-shift-click.png) |
| CO-7 (real input) | open | Needs real input: a real Shift+click fills the native press recorder. Unit tests cover the recorder path (the press with n 1 removes the original model; a ⌘N jump right after a Shift pick switches). See "Real-input batch steps". |
| CO-4 | pass: from Favorites, ⇧⌘↓ selects Claude (before: the disabled Codex and "No models found"). Unit tests on the rail order. | [co4-next-provider.png](https://raw.githubusercontent.com/ccheever/exact2/e1b86854a4e6700891afb7513f75674c16506ab3/model-picker-parity/co4-next-provider.png) |
| CO-1 | pass: ⌘1–⌘5 before the stars on the Claude list, ⌘1–⌘6 on the "opus" results, ⌘1 on Favorites, as the reference. | [co1-jump-badges.png](https://raw.githubusercontent.com/ccheever/exact2/878c05c64468236edd6a38d5fbc45064f730b4ac/model-picker-parity/co1-jump-badges.png) |
| CO-2 | pass: the Codex button's name and its hover tooltip read "Codex — Unavailable. Codex CLI is not authenticated. Run `codex login` and try again." (before: "Codex — Not ready.", no tooltip); Claude's is "Claude". | [co2-rail-tooltip.png](https://raw.githubusercontent.com/ccheever/exact2/b65ba1d5cbff61d73cef8936c4aa196f277c7f15/model-picker-parity/co2-rail-tooltip.png) |
| CO-11 | pass (tree): the trigger is named "Claude Fable 5.1", "Claude Opus 5, Claude Sonnet 5.5" with two models (before: always "Choose provider and model"); the Plan toggle reports pressed true/false (before: none). Reference: `button "Claude Opus 5"`, `button "Plan mode — click to return to normal build mode" [pressed]`. | text in the PR |
| S2-4 | pass: New task › Model opens the provider picker (rail with the disabled Codex and its reason, search, Favorites, ⌘1) and a Claude › Claude Sonnet 5 pick shows on the trigger; the writer model opens it above its trigger and a pick writes Claude Opus 5.5 (before: plain listboxes). | [s2-4-new-task-model.png](https://raw.githubusercontent.com/ccheever/exact2/5b0c6437f2960c70b89f231c9048af75da60f95d/model-picker-parity/s2-4-new-task-model.png), [s2-4-writer-model.png](https://raw.githubusercontent.com/ccheever/exact2/9b82d2153b80c04896d261d9f51610596cc23889/model-picker-parity/s2-4-writer-model.png) |

Tests: `model-picker-parity.test.ts`, 21 tests:
- CO-4: the adjacent provider order, and the dispatch from Favorites.
- CO-1: jump labels on lists, search results, Favorites, an open Legacy section, and a rebound chord.
- CO-2: rail labels per state.
- CO-3: `firstEnabled`. From the Contract source: the `base` autoHighlight derive, searchKey's Return (chooses, Shift adds,
  opens the legacy section, no `submit` on the field), and Return's target with legacy rows.
- CO-7: Shift's toggle without the native press and on a started thread. With the native press and n 1: the draft's
  original model is removed, a ⌘N jump right after a Shift pick switches, and a press the picker could not see still adds.
  From the Contract source: the row's `e.shiftKey`, the root's n 1, and the picker kept open on the search field.
- CO-11: the trigger's names (`modelLabel`, `fanoutAria`). From the Contract source: the trigger's aria-label and the Plan
  toggle's `aria-pressed`.
- S2-4: the task and writer catalogs, their setup and selections, the trigger marks and the writer row, and the picker's
  stacking over the editor.

The review's three new behavior tests (the native press with n 1, twice, and Return's legacy target) fail on `4df469158`'s
sources and pass after.

## Review fixes (2026-10-10)

An independent review of PR #374 found these problems; each is fixed in `db50e5631`:
- Blocking (CO-7): a Shift pick (n 1) left the native press unread, so removing the draft's original model did nothing
  and a ⌘N jump within 2 s of a Shift pick added its model. Fixed in `changeModel` (see "Cause and fix"); unit tests.
- Should-fix: the record said no row needs real input. CO-7 now has a real-input row and batch steps.
- Should-fix: CO-11 and the Contract halves of CO-3/CO-7 had no tests. They are now read from the Contract sources.
- Should-fix: Return on an open Legacy section's row did nothing. Return's target now takes those rows; the live drive
  and the reference agree (CO-3 row above).

## Real-input batch steps

CO-7 with a real pointer and real keys, in one session (screen unlocked; lane `model-picker-parity`, a build of this
branch). `A` = `/Users/daehyeonmun/orca/workspaces/exact2/t3-code/target/t3-audit`, `L` = `$A/lanes/model-picker-parity`.
1. From `/Users/daehyeonmun/orca/workspaces/exact2/t3-code-model-picker-parity`, with the lane's isolation as
   `$A/clone-drive.sh` sets it, run: `PATH=$HOME/.bun-1.4.2/bin:$PATH EXACT_APP_DIR=$PWD/examples/t3-code
   T3_LOCAL_HOME=$L/clone-t3-home T3_LOCAL_PORT=16382 T3CODE_TELEMETRY_ENABLED=false
   T3_LOCAL_RUNTIME_DIR=$A/runtime/t3-0.0.46-nightly.20261005.2667-darwin-arm64 CODEX_HOME=$L/codex
   CLAUDE_CONFIG_DIR=$L/claude XDG_CONFIG_HOME=$L/xdg/config XDG_DATA_HOME=$L/xdg/data XDG_STATE_HOME=$L/xdg/state
   XDG_CACHE_HOME=$L/xdg/cache bun host/apple/build.mjs t3-code-macos --bundle --run`.
2. Dismiss the mobile-app notice. On the "work" new-thread draft, click the model trigger, then the Claude rail button.
   Note the trigger's model, the original model (for example "Claude Fable 5.1"). Do not send anything.
3. Shift+click "Claude Sonnet 5.5". It passes when both rows show a check, the trigger reads "<original>, Claude Sonnet
   5.5" and the picker stays open.
4. Shift+click the original model's row. It passes when its check goes away, the trigger reads "Claude Sonnet 5.5", and
   the picker stays open. (Before the review fix: both checks stayed.)
5. Shift+click another unchecked row that has no ⌘1 badge (for example "Claude Opus 5"); the trigger names two models.
   Within 2 s, press ⌘1. It passes when the picker closes and the trigger names only the ⌘1 row's model; reopened, the
   picker shows no checks. (Before the review fix: the ⌘1 model was added to the several.)

## Not done / not verified

- CO-7 with real input: open until the coordinator's real-input batch runs the steps above (the screen is locked for
  task agents).

## Next action

The coordinator runs the real-input batch steps, then reviews and merges the PR.
