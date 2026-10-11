---
name: 20261012-adopt-main-fixes-r10
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: pending
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r10'
pr_url:
verified_commit:
---

# Main adoption, round 10, and the workarounds its fixes end

## Outcome

`feat(example)/t3-code` takes current `main`. Round 9 ([adopt-main-fixes-r9](20261011-adopt-main-fixes-r9.md), #458)
adopted main `ccc9afd52` (X73's #446) and the T3 branch landed it, then progress-native-bars (#462); the T3 tip is
`fe0e39c91`. On 2026-10-12 main landed the clone's framework fixes #447 (X72), #451 (X74), #454 (X76), #455 (X78),
#457 (X55: `status`, `alert`, `dialog`, `alertdialog` roles; X49's file closed), #459 (X61: `outline: none` on
`input`/`textarea`) and #461 (X71 part 2), up to `ba4fa5326`, and Charlie's LLP 1115/1116 waves (`ccd3d3be7` fixes
the `apps/color-gallery` `button_migration` red the T3 branch inherited in round 9). The user decided on 2026-10-11
that every item below is built now, matching the reference T3 Code (no further decision needed unless the reference
itself is ambiguous).

| Id | Clone (the T3 tip `fe0e39c91`) | What this round does (reference first) |
| --- | --- | --- |
| AD-1 | The branch lacks main's commits after `ccc9afd52` (`ccc9afd52..ba4fa5326` at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge or fast-forward that keeps main's ancestry); main's framework, the clone's `examples/t3-code`; the clone adapted to whatever main renamed or now refuses |
| RT-1 | X61: the composer and Settings › Appearance's prompt preview draw Exact's focus ring when focused; Contract had no `outline` | `outline="none"` on both textareas, as the reference's `focus:outline-none` (`ComposerPromptEditorTiptap.tsx`, the preview is the same editor) |
| RT-2 | X74: Settings › General › Legacy features: an sr-only level-2 heading (`SettingsSrHeading`) sits before the button, because a Mac button was an accessibility leaf | The level-2 heading goes inside its button, as the reference's `CollapsibleTrigger` holds `<h2>`: button › heading [level=2]; `SettingsSrHeading` stays only where the reference's heading is itself `sr-only` |
| RT-3 | X72: `formatUsd` (`pages-usage.ts`) rounds half away from zero itself because Apple's `Intl` rounded half to even | `Intl.NumberFormat` as the reference's `usageFormat.ts` `CURRENCY`, if the test oracle still passes and the running macOS app shows USD 0.825 as $0.83; else kept, with why |
| RT-4 | X73: the Pull Requests Filters popover's rest box (`pr-filters-rest`): `closeSub` focuses it instead of `blur()` | `closeSub` calls `blur()` (the reference leaves the focus on BODY); the rest box goes; `menu-keys.test.ts` follows; an agent drive closes Filters by the window's own key path with nothing focused |
| RT-5 | X76, X78: the shell menu's Copy is disabled over unselected page text; a `link` node has no `AXURL` and a Markdown table cell's link runs are invalid elements outside the app | No clone change expected: `EXACT2-GAPS.md` rows and "Text context menu" S1/S3 say fixed on main and adopted, real-input check pending; real-input steps below |
| RT-6 | X55: a `status`/`alert` node was plain text (or nothing) to AppKit, and announced only with an explicit `aria-live` | Check the clone's status and alert nodes (toasts, inline errors, live regions) reach AX with the reference's roles (`AXApplicationStatus`/`AXApplicationAlert`/`AXApplicationDialog`); retire an explicit `aria-live` that only stood in for a role; the `aria-modal` subtree projection still waits on main (X55 stays partial) |
| RT-7 | X71 part 2: on macOS a press in a popover reached no ancestor's `pointerdown` | Retire a clone workaround for it, if any; else say none |

## Steps

1. Before: build this branch's bundle at the T3 tip `fe0e39c91` before merging main and drive it once (lane
   `adopt-main-fixes-r10-before`, base port 16740: the clone's server on 16742).
2. `git merge --no-ff origin/main` at main's tip; main's side for the framework, the T3 side for `examples/t3-code/`.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
3. Check what the clone relies on in main's new commits (Contract refusals and renames, `contract build` warnings,
   the scripts the clone's tools call, rules and docs) and record each with its outcome ("Adopted main").
4. RT-1 to RT-7 against the reference source; Bun tests for each source change.
5. `EXACT2-GAPS.md` (each row: fixed on main by #N, adopted in round 10, workaround retired or kept), `STATUS.md`'s
   rows these touch, `issues/README.md`'s mapping.
6. After: this branch's bundle, the same drive (lane `adopt-main-fixes-r10`); the reference over CDP where a row
   needs it; before | after | reference images.
7. Merge the latest `origin/feat(example)/t3-code`, then the checks once on the final head.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 2 is empty | text |
| AD-1: the five checks pass on the merged tree | CLAUDE.md's five checks once on the final head | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, the macOS bundle; ONE agent-mode drive on the before bundle and on this branch's | before / after screenshots |
| RT-1 | the composer and the prompt preview focused, `screenshot … window` | before / after / reference images |
| RT-2 | `tree --ax` of Settings › General › Legacy features | before / after text (button › heading [level=2]) |
| RT-3 | `pages-usage.test.ts`; the running app's Usage page, 30 days, an amount of USD 0.825 | before / after text |
| RT-4 | `menu-keys.test.ts`; agent drive: Filters › Author, the pointer into the submenu, Escape in the search (`closeSub`), then the window's key path with nothing focused | before / after text (focus and popover state) |
| RT-5 | `EXACT2-GAPS.md`; real-input steps | real input (open) |
| RT-6 | `tree --ax` of the clone's status, alert and toast nodes | before / after text |
| RT-7 | reading the clone's popovers and its outside-press plumbing | text |

## Reference

- Method: [adopt-main-fixes-r9](20261011-adopt-main-fixes-r9.md) (Steps, "Adopted main", checks, Delivery).
- RT-1: `apps/web/src/components/ComposerPromptEditorTiptap.tsx` (`editorAttributes` class `… focus:outline-none`),
  `apps/web/src/components/settings/SettingsFontPreviews.tsx` `PromptFontPreview` (the same `ComposerPromptEditor`).
- RT-2: `apps/web/src/components/settings/SettingsPanels.tsx` `LegacyFeaturesSection` (`<CollapsibleTrigger>` holding
  `<h2 className="text-sm font-normal …">Legacy features</h2>` and the chevron).
- RT-3: `packages/shared/src/usageFormat.ts` (`CURRENCY = new Intl.NumberFormat("en-US", { style: "currency",
  currency: "USD", minimumFractionDigits: 2, maximumFractionDigits: 2 })`, `formatUsd = CURRENCY.format`).
- RT-4: `apps/web/src/components/pullRequest/PullRequestListFilters.tsx` (Base UI Menu submenus; the focus left on
  BODY when the pointer left the submenu's trigger), closed record
  [realinput-1010c-fixes](closed/20261010-realinput-1010c-fixes.md) RC-3.
- RT-6: `apps/web/src/components/settings/ProviderSetupSection.tsx` (`<p role="status">` installation status, the
  `sr-only` `role="status"` pending label), `ProviderAuthenticationSection.tsx` (`<span role="status">` account
  description), `CodexSetupSection.tsx` (the account description is plain `SettingsRow`/`CodexWelcomeCard` text),
  `IntegrationsSettings.tsx` (the profile removal error `<p aria-live="polite">`), `ui/toast.tsx` over Base UI
  `Toast.Viewport` (`role="region"`, `aria-live="polite"`) and `Toast.Root` (`role="dialog"`).
- Main: `issues/closed/` files of X72, X74, X76, X78; `issues/20261009-macos-role-modal-accessibility.md` (X55, open:
  the `aria-modal` subtree), `issues/20261010-pointer-events-reach-ancestors.md` (X71, part 1 open).
