---
name: 20261008-visual-parity-followup
plan: 20261005-t3code-macos-parity
implementation: done
verification: passed
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-visual-parity-followup
pr_url: https://github.com/ccheever/exact2/pull/294
verified_commit: 654903eaac0764233c336369b446e5f9f2018cf0
---

# Desktop visual parity follow-up: shared selectors, hosts row menu, dark/Advanced/collapsed comparisons

## Outcome

Close the rows [desktop visual parity](20261007-desktop-visual-parity.md) (#250, merged) left
open: the other places that share its fixed selector, the hosts row menu, and the reference
comparisons in dark, with Advanced on and with collapsed tables. "After" means "now matches the
reference" for the selector rows.

## Acceptance

1. **Shared selectors.** Storage's cleanup select, the Source Control selects (merge method,
   writing style), the Project page's Workspace and Model selects and a keybinding row's
   Reset/Remove menu, re-verified on `07dcef1ab` (the #250 captures were of an older build), with
   before/after pairs against the reference; anything still misaligned is fixed.
2. **Hosts row menu** (Settings › Integrations › Device hosts), with a saved device host in the lane.
3. **Reference comparisons** in dark, with Advanced on and with collapsed tables, by #250's method
   (the pinned release's own web client served by the same lane server, in Chrome); if that method
   needed the oracle tool, the row would be "not run — user decision 2026-10-06". It does not.

## Findings on the base (`07dcef1ab`) and fixes

Measured with the agent's `layout` (points) in both apps; reference values from the web client's DOM.

| Control | Base | Reference | Fix |
| --- | --- | --- | --- |
| Every `SkPopup` (settings selects and row menus) | top-left at the invoker's bottom-left, no gap: its `margin-top` and the end-align `margin-left` were ignored | 4 pt below (`sideOffset` 4); row menus `align="end"` | `position-area="bottom span-right"` on `SkPopup`, so the painted layer places it by its margin box (LLP 1021 "Placement"; the pattern `providers-upkeep.contract` already uses) |
| Keybinding row Actions menu | 1041.5,221 160×66: starts at the 28 pt trigger and runs 132 pt past it, over the card's edge | 909,264 160×66, right edge on the trigger's | the `SkPopup` fix (its `endAlign=28` now applies): 909.5,225 160×66 |
| Device host row options menu | 1014,632 128×66, start-aligned | 882,756 160×66, end-aligned | width 160 (MenuPopup `min-w-[min(10rem,…)]`) and the `SkPopup` fix: 882,636 160×66 |
| Select items (`SkMenuItem` with `check`) | a check column on the selected row, no wash | `SelectItem` has no indicator; the selected row keeps `bg-foreground/8%` | selected wash `light-dark(#27272a14, #f5f5f514)`, no check; this also stops the merge-method popup growing 8.7 pt past its 160 pt trigger |
| Storage › Automatic worktree cleanup | trigger 104 pt | `SelectTrigger` `w-full` in the 10rem control column: 160 pt | width 160: trigger 1015.5,190 160×28, popup 1015.5,222 160×94 = the reference's 1015,190 / 1015,222 160×94 |
| Project › Model | a flat list of every model (`GhostSelect`, clipped at the window's edge) and an empty traits trigger | General's `ProviderModelPicker` + `TraitsPicker` (`ProjectDefaultsSettings` modelRow) | the Project page renders General's `default-model` row in the project scope (`settingsCore.projectModel`, `CoreRowView` in the `ProjectsPanel` slot); its picker, traits menu, inheritance and reset write the project's override (driven: pick, read back from the lane settings file, reset) |
| Traits trigger label (General and Project model rows) | the effort only | `buildTraitsTriggerDisplay` over every trait | `traitsDisplay` moved to `r3-composer-controls-model.ts` and shared with `settings-core.ts`. Shown by the unit test only (a context-window option gives "Medium · 1M"): in the lane, Claude Fable 5.1's catalog offered only Reasoning in the app's sessions, so both apps' later captures read "Medium"; one earlier reference capture read "Medium · 1M" (the server's catalog for that model varied) |
| Dialog selects holding a draft (New task Project/Workspace, keybinding Command) | no option marked (the options' `selected` is never set for a dialog draft) | the draft's value is washed | `SettingsSelect` `value` prop: the caller's draft when it has one, `""` reads each option's `selected` |
| Markdown table cells | expanded: wrapped at 22.5 rem; collapsed: cut at 24 rem | Chrome ignores `max-width` on a `td`: expanded wraps across the column, collapsed cuts with an ellipsis at the column's edge | shown cell content asks for no width (`width=0 min-width=100%`), so only the column's sizer sizes it and the text fills the column |
| Typography (dark; Advanced on in light and dark) | chips, diff colours and emphasis as the reference | — | none needed |

Source Control's writing-style select and the Project Workspace select already matched the
reference's geometry; they take the shared gap and wash.

Side effects, not captured: the scheduled-task row menu (`endAlign=28`) is now end-aligned, as the
reference's (`ScheduledTasksSettings.tsx` MenuPopup `align="end"`); Runs on, the New task Model
`GhostSelect` and the r7 device select take the 4 pt gap and the wash. `projects-view.ts`'s Actions
row no longer copies the Model row's inheritance fields (it spread `page.model`).

## Progress

- 2026-10-08: Lane on 16520/16521; reference captures by #250's method; the before session on the
  evidence worktree at `07dcef1ab`; fixes above; after attempt 1, then the one retry on the final tree.
  New test `settings-core.test.ts` "the Project page's Model row is General's, in the project scope"
  (fails on base: no `projectModel`; the "Medium · 1M" label fails on base `settings-core.ts`).

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining |
| --- | --- | --- | --- | --- |
| reference | pinned runtime `t3 v0.0.46-nightly.20261005.2667`, web client in headless Chrome (lane profile) | Storage, Source Control, Project, keybinding row and Command menus, hosts row menu, New task Workspace, Typography light/dark × Advanced off/on, tables light/dark × expanded/collapsed | `t3-code-evidence/visual-parity-followup/` reference panels; `live-session.md` | — |
| before | base `07dcef1ab` (evidence worktree, under its build lock) | the flow; findings above | `ops-before.txt` | — |
| after 1 | this branch before the traits label and `value` edits | every flow; the Project model picker wrote and reset a project override | `ops-after-attempt1.txt` | traits label, dialog wash |
| after (retry) | final tree | every flow; the after panels | pairs 01–16, `ops-after.txt` | — |

Checks on the final tree: `bun test examples/t3-code` 3037 pass / 1 skip / 0 fail (base 3036 + 1);
strict tsc clean; `contract build` 3859 slots, 46 resources, 4159 actions; `cargo test -p
t3-code-macos --lib` 11 passed; caps within budget; the five checks exit 0 (cargo tests 3383 passed,
0 failed, 33 ignored). Runner recipe (`target/vpf/verify/recipe.json`, local): `status: passed`,
`source_unchanged: true`, source digest `78ecf0d7338b…` over the 16 task-owned code files.

Independent review (separate agent, the staged diff at `07dcef1ab`): no blocking findings. Should-fix,
addressed in this record: the traits-label claim (row above), the uncaptured side effects (above), evidence 16's cut caption (recomposed). Notes kept for follow-up: a selected
select row keeps its wash on hover (the reference turns to `bg-accent`); the traits label and menu
resolve an unset trait differently (stored, `currentValue`, default vs stored, default, first) and an
empty label hides the traits trigger; settings selects stay start-aligned (`endAlign=0`), the same as
the reference's `align="end"` while the popup is no wider than its trigger; the Project card's Model
writes through settings-core to every target of the scope while Workspace still writes through REST
`scoped`; `CoreRowView` takes `settingsCore.palette` while the other Project rows use fixed colours;
an expanded header wider than 24rem is cut where the reference's `th` shows it whole.

## Not done / not verified

- Oracle and trace rows of #250: not run, user decision 2026-10-06.

## Found while capturing (not this task's rows; for a follow-up task)

- Dialog selects (New task Runs on, Project, Workspace): the reference's `SelectPopup` defaults to
  `alignItemWithTrigger` for a mouse open, overlaying the trigger with the selected row on its value
  (popup 287 × 94 at 643,277 over a 257 pt trigger at 646,282); the clone opens below. Needs the
  selected index in the popup's margin and a live session.
- The keybinding Command select's popup is `max-h-72` (18rem) in the reference; the clone's 22.5rem.
- The General/Project traits menu is clamped against the window's right edge (General's existing menu).
- From #250, still open: inline code naming a workspace path is a file chip in the reference; square
  intraline emphasis (inline spans paint background only).
- [X59](../../issues/closed/20261008-x59-line-clamp-first-layout-ellipsis.md): a collapsed cell's first layout
  after the toggle shows its wrapped first line without the ellipsis (base too); after a restyle (the
  dark capture) the ellipsis is there.

## Next action

Review of draft PR [#294](https://github.com/ccheever/exact2/pull/294). The implementation commit
`654903eaa` was verified (runner digest `78ecf0d7338b…`; the committed tree and the merged head match
it). After merging the feature branch (#265, #288, #289): `bun test examples/t3-code` 3049 pass /
1 skip / 0 fail, contract build 3860 slots, five checks and `cargo test -p t3-code-macos --lib` pass.
The ticket stays in `tasks/` (coordinator rule: records sync moves it).
