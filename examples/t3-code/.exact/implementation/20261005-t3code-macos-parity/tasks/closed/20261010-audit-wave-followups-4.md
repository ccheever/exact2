---
name: 20261010-audit-wave-followups-4
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-audit-wave-followups-4
pr_url: https://github.com/ccheever/exact2/pull/383
verified_commit: 954f6bbff15fe766362e7aac19cab8298a8f2b0f
---

# Differences the audit fix agents found outside their tasks (fourth set)

## Outcome

PRs #378 and #379 reported two more differences from the reference, outside their findings. This task fixes them as the
reference does. Earlier sets: [1](20261010-audit-wave-followups.md), [2](20261010-audit-wave-followups-2.md),
[3](20261010-audit-wave-followups-3.md).

## Findings

| Id | Reference (T3 Code `1e2ecbd975`) | Clone | Found by | Steps |
| --- | --- | --- | --- | --- |
| FX-1 | Restore defaults lists the scope's settings (`useScopedSettings`): in a project scope the list names what that scope would reset. | `restoreLabels` lists the focused environment's settings, so in a project scope only the list differs (the reset itself matches since #379). | restore-defaults-agent-browser-access (#379) | Settings › scope a project with an override › Restore defaults; compare the dialog's list. |
| FX-2 | A menu keeps one highlight: moving with the keys from a row the pointer rests on moves the one highlight (Base UI's `highlightedIndex` serves both). | The clone's painted menus shade two rows (hover and focus) until the pointer moves. All painted menus share this hover model. | audit-wave-followups-3 (#378) | Pull Requests › Filters; rest the pointer on Author; ArrowDown; compare. Then another painted menu (a sidebar row menu, the scope menu). |

## Scope and exclusions

Included: the two rows. FX-2 is a shared model change: do it once in the shared menu pieces (`menu-keys.contract`,
`KeyMenu`, the painted menu rows), not per menu, and keep the native NSMenus untouched. Excluded: framework changes.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FX-1 | Bun test of the scoped label list | text |
| FX-2 | agent drive on two painted menus (hover a row, ArrowDown: one highlight); Bun/Contract test of the shared rule | before / after / reference image |

## Cause and fix

- **FX-1.** `useSettingsRestore` reads `useScopedSettings`: the scope's representative target (SettingsScopeContext's
  `target`, the member on the scope's environment, else the first), whose settings are its environment's with the
  project overrides and, once read, its t3.json tier on top (`resolveProjectSettings`). The clone listed the focused
  environment's settings. `settingsCore` (`settings-core-view.ts`) now hands `restoreLabels` the target's settings
  (`scopeTarget`, `settings-core.ts`), null with no connected target (DEFAULT_SERVER_SETTINGS), and reads the members'
  t3.json in a project scope on every settings route (useMemberProjectFiles is the scope provider's, not a page's).
  `restoreLabels` now walks the reference's own list in its order (`RESTORE_ROWS`): device and environment values
  interleave as there, a font's label counts its family and its size ("Code font", not "Monospace font" and "Code font
  size"), "Visible threads" is listed, and "Theme mix" follows "Follow system" when a theme sits on one appearance only
  (`themeHalves !== null`: here `themeLight` or `themeDark` apart from `theme`, which a one-palette theme's Use sets and a
  whole theme clears). The base listed no row for that state, so Restore defaults stayed disabled with a mix alone (review
  round 1). Checked on the reference over CDP with the same settings: the environment
  scope lists "Snooze limited threads."; the work project scope lists "Auto-settle merged threads, Snooze limited
  threads, Response streaming, New thread mode." The last is the reference's own resolution: a read t3.json fills the
  unset `defaultThreadEnvMode` with its built-in "local", which is not the default's null, so every project scope lists
  it (seen on the reference with nothing changed: "This will reset: New thread mode."). The reset itself is unchanged
  (since #379 a project scope writes no environment key).
- **FX-2.** Base UI keeps one `highlightedIndex` for the pointer and the keys: an item the pointer moves over takes the
  focus (`focusItemOnHover`), the keys move on from it, and the pointer leaving an item hands the focus back to the popup
  (checked over CDP on Sort, the scope menu and Filters). The clone shaded a row by its hover and by its focus, and the
  popup's `KeyMenu` moved the keys from the last row its own keys reached. Now the highlight is the focus, in the
  shared pieces: every painted menu row paints from its own focus alone (37 row components and the menus whose owner
  draws the rows: the hero project menu, project scripts, the settings scope, select and traits menus, Files' "Open in",
  SnapShots' sound menu, the two link rows, the approval menu). A row the pointer enters focuses its door,
  `${id}-km`, a 1-pt invisible box `KeyMenu` draws for each item (`KmDoor`, `menu-keys.contract`); the door makes the row
  `current` and hands it the focus, so ↓ goes on from the row the pointer rests on and the row it leaves loses its
  shade. A row the pointer leaves while it holds the focus gives the focus to the popup (`keysId`), so ↓ starts again at
  the first row, as on the reference; a row that lost the focus to another control (Filters › Author's search field
  after a press) keeps nothing and takes nothing. Why doors: the popup cannot hear which row took the focus (no
  focusin, #283, X54), and it hears no pointer itself: a `hover` on it would trade the hover with its rows on every move
  on macOS (#322, X62), and a `pointermove` would make it hold the presses inside it, so a press on a row would no
  longer reach the window's light dismiss (popover-escape-parity's count; tried, and forwarding the presses through an
  injected window action grew the plan by 6.5 MB). The Pull Requests rows `pr-more-open-host` and `linked-pr-menu-open`
  (links), the project script rows, Files' editor rows and the SnapShots sound rows had no highlight at all; they have
  the focus highlight now. The settings scope and select menus keep the chosen row's wash beside the highlight
  (MenuRadioItem `data-checked`, SelectItem `data-selected`: `bg-foreground/8`), as the reference shows.
  The title menu keeps ContextMenu's DOM fallback model (a row highlights on hover or focus): it is not a Base UI menu.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| FX-1 | pass: Settings in the work project scope, Restore defaults lists "Auto-settle merged threads, Snooze limited threads, Response streaming, New thread mode." as the reference (before: "Snooze limited threads."); the environment scope stays "Snooze limited threads."; a one-palette theme on its half lists "Theme mix" and enables Restore defaults (before: nothing listed, disabled); the Bun tests of the scoped list and of the mix; the live drive's dialog | [FX-1-theme-mix.txt](https://raw.githubusercontent.com/ccheever/exact2/4ef1132986178c2c3eae3ab7be0704034ca903cd/audit-wave-followups-4/FX-1-theme-mix.txt), [FX-1-restore-list.txt](https://raw.githubusercontent.com/ccheever/exact2/fa219b729896c3a61badb2b9fc7bd2cf3ce9e072/audit-wave-followups-4/FX-1-restore-list.txt), [fx1-restore.png](https://raw.githubusercontent.com/ccheever/exact2/ccc2f5b0f0f356dcc4707c6295054c68cf9373d6/audit-wave-followups-4/fx1-restore.png) |
| FX-2 | pass (agent pointer and keys; real input open): Sort, the pointer on Blocked on me, ↓: one highlight on Recently updated, as the reference (before: Blocked on me and Merge readiness shaded, ↓ had gone from the popup to the first row); the settings scope menu, the pointer on Verification fixture, ↓: work highlighted, All projects keeps its checked wash, as the reference (before: All projects and Verification fixture shaded); the pointer then on Oldest shown moves the focus there (state: focus node 2165 → 2173; before: unchanged). `menu-one-highlight.test.contract` on the bundle, 5 of 5: hover a row, ↓, Enter presses the next row (Sort: Recently updated after Blocked on me, Largest shown after the pointer moved to Oldest shown; scope: Verification fixture after All projects), and the pointer leaving hands the keys back to the popup; the same steps on the before build pressed the first row (Merge readiness, All projects). The shared rule's Bun tests, each `keysId` checked against its popup | [FX-2-menu-one-highlight.txt](https://raw.githubusercontent.com/ccheever/exact2/bd2a6b0a89c853c8ec7518a3eee4cfd2301b338f/audit-wave-followups-4/FX-2-menu-one-highlight.txt), [fx2-sort.png](https://raw.githubusercontent.com/ccheever/exact2/a705b303aa34fa0e78e83a44738bb4011cf33898/audit-wave-followups-4/fx2-sort.png), [fx2-scope.png](https://raw.githubusercontent.com/ccheever/exact2/6e9698ce6223fe1868b62d6c06a4407388a56ddc/audit-wave-followups-4/fx2-scope.png) |

## Tests

- `settings-core.test.ts` "Restore defaults lists the scope target's values, in the reference's order": the environment
  scope, the checkout p1 (its overrides, the environment's value, New thread mode from the t3.json tier), the same on
  Source Control and in the project scope, a member without overrides, device values among the environment's, the font
  label, Visible threads, no connected target. The earlier restore tests take the new signature. "Restore defaults lists
  "Theme mix" for a theme on one appearance": a one-palette theme's half alone enables Restore defaults with "Theme mix",
  after Follow system; a whole theme clears it; a half over a whole theme is both; Restore clears it.
- `menu-keys.test.ts` "one highlight": `KeyMenu`/`KeyMenuWatched` make a door's row current and focus it and hear no
  hover or pointer; `KmDoor`'s box; each of the 37 row components paints from its focus, focuses its door on entering
  and hands the focus to `keysId` on leaving, and every use passes `keysId`; the owner-drawn menus; no keyboard-reachable
  menu row paints from its hover. The snooze, Act on and Filters expectations follow the new rows. "every keysId names
  the KeyMenu popup the row sits in": each of the 146 row uses' `keysId` equals the `menuId` of the `KeyMenu` around it
  (or a wrapper's, `SkPopup`/`CnMenu`, with `-keys`), following a component that hands its `keysId` on to its uses; only
  the device rail's rows sit beside their KeyMenu (Found, not changed).
- `menu-one-highlight.test.contract` (behaviour, the agent's pointer and keys on the macOS bundle, review round 1): Sort
  (a `KeyMenu` of `PrMenuItem` rows) and the settings scope menu (`ScopeMenu`, rows its owner draws). Each case hovers
  rows, sends ↓ and Enter to the popup's own box (a node that takes no focus, so the keys reach whatever holds it) and
  reads the pressed row from the state (`prList.sort`, `settingsCore.projectLabel`): the pointer on a row, ↓ goes on
  from it; the pointer moving to another row takes the current row with it; the pointer leaving hands the keys back to
  the popup. Run it with `EXACT_ROOT=<worktree> target/t3-audit/clone-drive.sh <lane> <port> --test
  examples/t3-code/menu-one-highlight.test.contract`. 5 of 5 pass on this branch; on the before build the first, second
  and fourth press the first row (the leave cases match the base, which never moved the keys with the pointer).

## Real-input batch steps

Run on this branch's bundle (lane `audit-wave-followups-4`, base port 16500: `T3_LOCAL_HOME=<lane>/clone-t3-home
T3_LOCAL_PORT=16502`), app launched normally and active, window 1280×840:

1. Sort: click Pull Requests in the sidebar, click Sort, rest the pointer on Blocked on me (it is shaded), press ↓:
   only Recently updated is shaded (Merge readiness keeps its lighter checked tint). Move the pointer onto Oldest shown:
   only Oldest shown is shaded; press ↓: only Largest shown. Move the pointer out of the menu to the page: no row is
   shaded; press ↓: Merge readiness is shaded. Press Escape.
2. Settings scope: Settings › General, click "All projects", rest the pointer on Verification fixture, press ↓: only
   work is highlighted (All projects keeps its wash). Press Escape.
3. Sidebar snooze menu: hover the Timeline verification row, click its clock: the menu opens and the row's actions stay
   shown. Rest the pointer on the second preset, press ↓: only the third is shaded; move the pointer across the presets:
   one row shaded at a time, no flicker; press Escape.
4. Filters › Author: click Filters, click Author and leave the pointer on it: "Search authors" keeps the caret; move the
   pointer into the submenu onto Anyone: Anyone is shaded (the caret leaves, as Base UI's hover takes the focus); press
   Escape twice: both menus close.

## Found, not changed

- The settings scope menu centres its row labels ("All projects", "work"); the reference starts them at the left
  (`ScopeMenu`'s label has no `text-align="left"`, X57). Visual only.
- Restore defaults resets Font smoothing (and the other client settings the reference's patch leaves alone: the legacy
  sidebar, typography's Advanced, the legacy thread sort); the reference neither lists nor resets them. The list keeps
  "Font smoothing" so it names what this app resets.
- The device rail's menus (`r6-device.contract`) draw their rows beside their `KeyMenu`, not inside it, so a key at a
  focused row reaches no menu; their rows follow the pointer now.
- Moving the pointer within the row the keys left does not take the highlight back (Base UI's `onMouseMove` does): only
  entering a row does. Hearing a move needs a `pointermove` on the popup or the rows, which holds the presses inside them
  (see Cause, FX-2). Asked under "Decision needed" in the PR.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975` Electron over CDP, lane `audit-wave-followups-4` (backend 16500, CDP 16501) | Complete | the third column of each image; the restore lists in FX-1-restore-list.txt |
| Before drive | `t3-code-evidence-base` at `950e8e2e5` (its menu rows and restore list are the tip's in these steps) | Complete on the third run (the first two stopped at navigation steps of the drive script) | the first column |
| After drive (the live drive) | this branch's bundle `d7622dffe` | Complete, one run | the second column |
| Review round 1 | this branch's bundle with the fixes below | The review asked for a behavioural test of the shared rule on a KeyMenu and an owner-drawn menu, a check of each `keysId` value, and "Theme mix" in the restore list: added; `menu-one-highlight.test.contract` 5/5 on the bundle (one run); the before build's same steps press the first row | [FX-2-menu-one-highlight.txt](https://raw.githubusercontent.com/ccheever/exact2/bd2a6b0a89c853c8ec7518a3eee4cfd2301b338f/audit-wave-followups-4/FX-2-menu-one-highlight.txt), [FX-1-theme-mix.txt](https://raw.githubusercontent.com/ccheever/exact2/4ef1132986178c2c3eae3ab7be0704034ca903cd/audit-wave-followups-4/FX-1-theme-mix.txt) |
| Design | — | A popup `pointermove` worked but took the presses inside menus from the window's light-dismiss count (`usage-pooled.test.ts`); forwarding them through injected window actions grew the plan from 27.1 to 33.7 MB; replaced by the doors | — |

## Next action

The coordinator reviews the draft PR [#383](https://github.com/ccheever/exact2/pull/383), answers its "Decision needed", runs the real-input steps in the next batch,
and merges it.

## Delivery

Merged on 2026-10-10 as `954f6bbff` (#383, squash) after an independent review and its repair round. The builder's question (re-highlight on any pointer move) was settled by the coordinator under the user's rule for local framework drafts: the enter-only rule stays, declared in `EXACT2-GAPS.md` with the local draft X71.
