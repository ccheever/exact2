---
name: 20261007-settings-model-picker
plan: 20261005-t3code-macos-parity
implementation: done
verification: unverified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-settings-model-picker
pr_url: https://github.com/ccheever/exact2/pull/246
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

- [Composer fidelity](20261005-composer-fidelity.md) addresses the composer and its overflow;
  it does not cover model selection from Settings.
- [Provider settings upkeep](20261005-provider-settings-upkeep.md) owns the custom model options
  editor, provider updates and ACP management. Selecting an existing default model is separate.
- [Minor UI fixes](20261007-fix-minor-ui-issues.md) includes Settings Traits picker presentation,
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

## Progress

2026-10-08, draft PR #246 (`e9fbc8ef4`). Both General model rows now open the composer's
picker, with no fork. The shared parts are `pickerCatalog` with a `PickerTarget`, and `ModelPicker` with an `anchor`.
`settings-model-picker.ts` adapts the catalog to the settings scope. `settings-core.ts`
`scopedModelReason` ports `useScopedModelDisabledReason`: it disables rows, refuses picks and supplies the toast text.
Picks write through the existing `settings-core` command. Mixed rows show a neutral
trigger. In Settings, the picker's provider and jump keys act on the settings catalog. Escape closes
the picker, not Settings. Unit tests cover every acceptance row (`settings-model-picker.test.ts`, 9 tests).
No live row has run yet: two sessions failed in the drive's setup, and the approved third session ran with the screen locked (below). The rows are deferred to the real-input batch.

The brief's provider-sign-in lane logins no longer exist (the `t3-code-provider-sign-in-and-install`
worktree is gone). The lane catalog uses fixture credentials instead, with no real account:
- Codex: an API-key auth file with a placeholder.
- Claude: a placeholder `ANTHROPIC_API_KEY` in the instance environment.

Both probe `ready` / `authenticated`: Codex 0.151.0 with 5 models, all legacy, and Claude 2.1.293 with 12.

2026-10-08 (real-input batch, records PR): before/after pairs made (base 85cb6f4a1); every live row driven by real input; two clone bugs (no "No models found", no tooltip on an unavailable row). Results and proof: "Real-input batch (2026-10-08)" below.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-08) | `e9fbc8ef4` | `bun test examples/t3-code` 2520 pass / 1 skip / 0 fail (205 files); strict `tsc` clean; contract build 2628 slots, 46 resources; `cargo test -p t3-code-macos --lib` 11 pass; caps within (`app.contract` 1,499 lines); five checks: build 0, test 3383 pass / 0 fail / 33 ignored (94 binaries), clippy 0, fmt 0, boot 0 | [checks.txt](https://github.com/ccheever/exact2/blob/db93d8b98ef678b3be35ff249371538ef183b85b/settings-model-picker/checks.txt) | — |
| Live session 1 (18:20Z) | dev build of `e9fbc8ef4` sources, lane port 16810 | Failed in setup: the development build has no bundled `t3-runtime`, so "This machine" did not start (`T3_LOCAL_RUNTIME_DIR` unset). No flow ran | [live-attempts.md](https://github.com/ccheever/exact2/blob/56cd563cc93a3785951b6e72693d12ebb9e3941e/settings-model-picker/live-attempts.md) | — |
| Live session 2, retry (18:22Z) | same | Server up in 2 s, providers ready in 3 s. The drive treated the still-inert main window as decided, and every tap was refused. No flow ran | same | — |
| Approved session (2026-10-08 01:07-01:10Z, screen locked) | base `da4f4512f` (evidence-base, build lock held), then branch `e9fbc8ef4`; ports 16812 / 16810; `T3_LOCAL_RUNTIME_DIR` set | Base: every tap refused for 20 s and every capture blank. Branch: Settings opened at 0.56 s and closed, providers ready at 1.1 s, then every tap refused for 40 s and every capture blank. No row flow ran. Every process exited and the ports are free | [live-session-3.md](https://github.com/ccheever/exact2/blob/31dcb418267527791e7add96b034034f2ddaf3ef/settings-model-picker/live-session-3.md) | deferred to the real-input batch — screen locked (user away) |

## Real-input batch steps

Every live row is deferred to the real-input batch — screen locked (user away). Steps, with the screen unlocked:

1. **Builds.**
   - Branch: `t3-code-settings-model-picker` worktree at `e9fbc8ef4` (dev build already made; rebuild with
     `export PATH=$HOME/.bun-1.4.2/bin:$PATH; EXACT_APP_DIR=$PWD/examples/t3-code bun host/apple/build.mjs t3-code-macos`).
   - Base: `t3-code-evidence-base` at `da4f4512f`, already built. Take `.build-lock` while running it.
2. **Lane.** `target/smp/lane` in the task worktree:
   - Homes: `t3-a` for the branch, `t3-b` for the base, each `userdata/settings.json` with the Codex / Claude / Antigravity instances.
   - Runtime: `runtime/` (the staged t3 0.0.46-nightly, unpacked).
   - `bin/` symlinks: `codex`, `node`, `claude`.
   - Fixture keys: placeholder API keys in `codex/auth.json` and in the Claude instance environment. No account; no prompt is sent.
   - Ports: 16812 for the base, 16810 for the branch.
3. **Drive.** Both commands run from the task worktree.
   - Base: `bun target/smp/drive.mjs before target/smp/drive-before /Users/daehyeonmun/orca/workspaces/exact2/t3-code-evidence-base`.
   - Branch: `bun target/smp/drive.mjs after target/smp/drive-after /Users/daehyeonmun/orca/workspaces/exact2/t3-code-settings-model-picker`.
   - Copy: [drive.mjs.txt](https://github.com/ccheever/exact2/blob/012fdceb1d43ce42a7482ac1d5b6d7715e185c8f/settings-model-picker/drive.mjs.txt).
   - Each run writes `steps.ndjson` and PNGs. Check that `"uniform":false` holds on every `shot` line.
4. **Rows, which the branch drive performs in this order.** After each step, read back `catalog`, the General row, the lane `settings.json` and the composer model.
   - Before/after pairs: `02-default-model-open` and `03-text-generation-open` (General, picker open, untouched settings).
   - Provider browsing: tap `provider-claudeAgent`, then `provider-codex` (`04`, `05`). Read back: rail Favorites / Codex / Claude / Antigravity (disabled) and the selected rail.
   - Search: type `opus`, `codex`, `zzzz`, then clear it in `model-search` (`06`-`08`). Read back: cross-provider results, `model-empty`, and that the rail returns.
   - Favorites: star `favorite-claude-opus-5-5` (`09`), open `provider-favorites` (`10`), press Escape, close Settings. Then open the composer's `model-picker` (`11`, opens on Favorites), press Escape, reopen the Settings picker and unstar (`12`). Read back: `settings.json` and the composer model unchanged.
   - Legacy: `provider-codex`, `model-legacy` twice (`13`-`15`), then pick `model-gpt-5.5` (`16`). Read back: the row label and `defaultModelSelection`.
   - Scoped selection: choose project `model-demo` (add it first with `add-project` + the path `target/smp/lane/repos/model-demo`) and pick Claude Opus (`18`, `projectSettingsOverrides`). Choose the environment row and pick `gpt-5.2` (`19`). Choose All environments and pick Claude Sonnet.
   - Text generation: open the picker; the rail has no Antigravity. Pick Claude Opus (`20`, `textGenerationModelSelection`).
   - Keyboard: ArrowDown ×2 (`21`), ArrowLeft to the rail, ArrowDown, ArrowRight back to search, type `opus 5.5`, ArrowDown, Enter (`22`, the setting changes). Reopen, press Escape: the picker closes, Settings stays open, focus is on the trigger.
5. **Disabled-row tooltip (real hover).** Pair a second lane server through Settings › Connections › Add environment:
   - Command: `target/smp/lane/serve.sh 16811` with `claudeAgent` disabled in its settings.
   - At All environments, open the default model picker and hover a Claude row. Expect the tooltip "This model is unavailable on <second>. Select that environment to choose its model separately." The row is dimmed and its star is disabled.
   - The row is pressed with `orca computer` under the real-input lock.
6. **Evidence.** Compose base | branch pairs with PIL; upload them to `t3-code-evidence/settings-model-picker/`.

## Real-input batch (2026-10-08)

Run by the coordinator's real-input batch on an unlocked Mac (2026-10-08, 02:58-05:15 UTC), under the shared real-input lock (owner "real-input batch"), on the merged feature branch (`b7761f556`, rebuilt once at `07dcef1ab` for #263): one lane copy "T3 Code (Lane RIB)" launched normally (not agent mode) with isolated homes and lane ports 16450-16499. Real input: cliclick / CGEvent real mouse and wheel events, real HID key chords (posted only after a check that the lane app is frontmost), orca computer clicks and pastes. Records PR: draft "T3 Code clone: real-input batch for the tasks merged on 2026-10-08".

| Row | Result | Proof |
| --- | --- | --- |
| Before/after pairs (base `85cb6f4a1` vs `b7761f556`) | Done: default model, text generation, Escape | [01-default-model-open-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/p01-01-default-model-open-before-after.png), [02-text-generation-open-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/p02-02-text-generation-open-before-after.png), [03-escape-before-after](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/p03-03-escape-before-after.png) |
| Agent-mode drive (drive.mjs) | Not usable: blank captures and refused taps again (as live sessions 1-3); every row below was driven by real input instead | — |
| Provider browsing | PASS | [smpA-browse-search](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/01-smpA-browse-search.png) |
| Search (opus / codex / zzzz / clear) | PASS except: FAIL (clone bug) the no-match state shows no "No models found" | [smpA-browse-search](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/01-smpA-browse-search.png), [smpA-search-zoom](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/02-smpA-search-zoom.png) |
| Favorites (star, Favorites rail, composer, unstar; no setting written) | PASS (settings.json byte-identical) | [smpA-fav](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/03-smpA-fav.png), [smpA-24-composer-picker](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/04-smpA-24-composer-picker.png) |
| Legacy expand/collapse, pick GPT-5.5 | PASS (`defaultModelSelection {codex, gpt-5.5}`) | [smpA-legacy](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/05-smpA-legacy.png), [smpA-31-legacy-picked](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/06-smpA-31-legacy-picked.png) |
| Scoped selection (project / environment / All → Mixed) | PASS | [smpA-35-project-picked-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/08-smpA-35-project-picked-crop.png), [smpA-42-env-picked-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/09-smpA-42-env-picked-crop.png), [smpA-52-general-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/10-smpA-52-general-crop.png) |
| Text generation (no Antigravity; pick Claude Opus) | PASS | [smpA-13-textgen-picked-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/07-smpA-13-textgen-picked-crop.png) |
| Keyboard (↓/↑, ←/→ rail, type, Return, Escape focus return) | PASS; composer model unchanged | [smpA-kb](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/13-smpA-kb.png), [smpA-kb3-small](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/14-smpA-kb3-small.png), [smpA-46-composer-crop](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/15-smpA-46-composer-crop.png) |
| 5. Disabled-row tooltip (second server with Claude disabled) | Rows dimmed, stars greyed, click refused — PASS; hover tooltip FAIL (clone bug): no "This model is unavailable on …" tooltip | [smpA-56-click-haiku](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/11-smpA-56-click-haiku.png), [smpA-57](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/12-smpA-57.png) |

Full record: [settings-model-picker.txt](https://raw.githubusercontent.com/ccheever/exact2/ec2aeba0830cebdf6f0af0275e53a46bc330b90a/real-input-batch/settings-model-picker/settings-model-picker.txt).

## Next action

Run "Real-input batch steps" in the coordinator's real-input batch (screen unlocked). Then attach the pairs and
after-only images to PR #246 and change `verification`. Everything except the live rows is done.
