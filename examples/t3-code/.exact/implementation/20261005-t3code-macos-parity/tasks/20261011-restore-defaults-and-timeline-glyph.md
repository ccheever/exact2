---
name: 20261011-restore-defaults-and-timeline-glyph
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-restore-defaults-and-timeline-glyph
pr_url:
verified_commit:
---

# Restore defaults resets what the reference resets; the Timeline's lifecycle glyph

## Outcome

The user decided on 2026-10-11 that these two differences, recorded as found but not changed, are built now, matching
the reference T3 Code (`1e2ecbd975`).

| Id | Clone | Reference (check it first) | Recorded in |
| --- | --- | --- | --- |
| RD-1 | Settings › Restore defaults resets every device value this app keeps, so it also resets Font smoothing (and lists it), the legacy sidebar setting (Sidebar (legacy)), typography's Advanced switch, the legacy thread sort and Plan mode (legacy); with only Font smoothing changed, Restore defaults is enabled | `useSettingsRestore` lists and resets exactly its own keys: none of these five. With only Font smoothing changed, Restore defaults is disabled; a restore keeps it | [audit-wave-followups-4](closed/20261010-audit-wave-followups-4.md) "Found, not changed" |
| TG-1 | A pull request Timeline's "Pull request opened" row (and merged, closed) wears the actor's avatar on the rail | `LifecycleEvent` always draws an `IconMarker` with the pull request glyph (`PullRequestGlyph.pullRequest`, `.merged`, `.closed`); the actor's name stays beside the label | [search-key-and-timeline-commit-title](closed/20261011-search-key-and-timeline-commit-title.md) "Seen while building" |

## Steps

1. Reference first: `SettingsPanels.tsx` `useSettingsRestore` (its labels and its one `updateSettings` patch, the theme
   keys through `useTheme`); `PullRequestTimelineTab.tsx` `LifecycleEvent`, `IconMarker`, `ActorTimelineMarker`.
2. RD-1: `restoreDeviceDefaults` (`settings-core.ts`) resets the reference's device keys only, and `RESTORE_ROWS` drops
   "Font smoothing", so the list, the button's enabled state and the reset agree with the reference.
3. TG-1: the lifecycle rows draw the reference's `IconMarker` (the glyph on the rail, whatever the actor), in
   `pages-pr-timeline.contract`.
4. Bun tests; `contract build`; one agent drive (before / after) with the reference over CDP.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RD-1 | reference over CDP (Settings › Appearance, Font smoothing off: the button; with Chat width Wide too: the dialog's list and what a restore keeps); Bun test of the reset's keys and the list; agent drive | before / after / reference image |
| TG-1 | reference (PR #168's Timeline, the lane server's web client in headless Chrome); Bun test of the lifecycle marker; agent drive (`screenshot … window`) | before / after / reference image |

## Reference

- RD-1: `apps/web/src/components/settings/SettingsPanels.tsx` `useSettingsRestore` (`changedSettingLabels`,
  `restoreDefaults`' `updateSettings` patch, `setTheme("system")`, `clearThemeHalves`, `setFollowSystem(true)`);
  `SettingsPanels.logic.ts` `getChangedTypographySettingLabels`; `packages/contracts/src/settings.ts` ClientSettings
  (`fontSmoothing`, `legacySidebarEnabled`, `sidebarThreadSortOrder`, `planModeEnabled`); typography's Advanced is
  `useLocalStorage(TYPOGRAPHY_ADVANCED_STORAGE_KEY)` in `TypographySection`, outside the settings.
- TG-1: `apps/web/src/components/pullRequest/PullRequestTimelineTab.tsx` `LifecycleEvent` (`IconMarker` with
  `PullRequestGlyph.pullRequest` / `.merged` / `.closed`, size-3.5), `IconMarker`, `TimelineMarker`;
  `pullRequestIcons.tsx` `PullRequestGlyph` (`GitPullRequestArrowIcon`, `GitMergeIcon`, `GitPullRequestClosedIcon`).
- Fixture: the GitHub sandbox `daehyeonmun2021/playground` PR #168 (`examples/t3-code/tools/github-lane`).

## Next action

Build RD-1 and TG-1, prove them, open the draft PR.
