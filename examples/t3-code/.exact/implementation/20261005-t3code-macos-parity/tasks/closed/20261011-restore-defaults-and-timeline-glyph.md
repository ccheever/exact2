---
name: 20261011-restore-defaults-and-timeline-glyph
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-restore-defaults-and-timeline-glyph
pr_url: https://github.com/ccheever/exact2/pull/470
verified_commit: 1f813789b
---

# Restore defaults resets what the reference resets; the Timeline's lifecycle glyph

## Outcome

The user decided on 2026-10-11 that these two differences, recorded as found but not changed, are built now, matching
the reference T3 Code (`1e2ecbd975`).

| Id | Clone | Reference (check it first) | Recorded in |
| --- | --- | --- | --- |
| RD-1 | Settings › Restore defaults resets every device value this app keeps, so it also resets Font smoothing (and lists it), the legacy sidebar setting (Sidebar (legacy)), typography's Advanced switch, the legacy thread sort and Plan mode (legacy); with only Font smoothing changed, Restore defaults is enabled | `useSettingsRestore` lists and resets exactly its own keys: none of these five. With only Font smoothing changed, Restore defaults is disabled; a restore keeps it | [audit-wave-followups-4](20261010-audit-wave-followups-4.md) "Found, not changed" |
| TG-1 | A pull request Timeline's "Pull request opened" row (and merged, closed) wears the actor's avatar on the rail | `LifecycleEvent` always draws an `IconMarker` with the pull request glyph (`PullRequestGlyph.pullRequest`, `.merged`, `.closed`); the actor's name stays beside the label | [search-key-and-timeline-commit-title](20261011-search-key-and-timeline-commit-title.md) "Seen while building" |

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

## Cause and fix

**RD-1.** `restoreDeviceDefaults` (`settings-core.ts`) replaced the device settings with `DEVICE_DEFAULTS` and the client
settings with `decodeClientPrefs({})`, so it reset every value this app keeps; `RESTORE_ROWS` listed "Font smoothing" to
name that. The reference's `restoreDefaults` sends one `updateSettings` patch of named keys plus `useTheme`'s theme, mix
and follow-system resets; Font smoothing, `legacySidebarEnabled`, `sidebarThreadSortOrder` and `planModeEnabled` are
not in it, and typography's Advanced is `useLocalStorage` state outside the settings. The restore now keeps those five
(`KEPT_ON_RESTORE` and Plan mode in the device settings) and the list no longer names Font smoothing, so the list, the
button's enabled state and the reset are the reference's. Checked on the reference over CDP with the same steps: Font
smoothing off with Advanced on leaves Restore device defaults disabled; with Project grouping off the dialog reads "This
will reset: Project Grouping."; after Confirm both stay as set. Plan mode (legacy) is the fifth key the reference leaves
alone, beyond the four the specification named.

**TG-1.** The lifecycle rows (opened, merged, closed) drew `PrdMarker`, the clone's `ActorTimelineMarker`, which shows
the actor's face when there is one. The reference's `LifecycleEvent` draws `IconMarker` with the pull request glyph
whatever the actor. A new `PrdIconMarker` (`pages-pr-timeline.contract`) is that `IconMarker`: the 2rem rail box,
centred, holding the 14 pt glyph in the muted tone; the lifecycle rows use it and keep the actor's name beside the label.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RD-1 | pass (agent drive, reference over CDP, Bun): Font smoothing off with Advanced on, Restore device defaults stays disabled (before: enabled); with Project grouping off the list reads "This will reset: Project Grouping." (before: "Project Grouping, Font smoothing."), as the reference; after Confirm, Project grouping is back on and Advanced and Font smoothing stay as set (before: Advanced off, so the Font smoothing row is gone, and Font smoothing on). The legacy sidebar, the legacy sort and Plan mode: the Bun test (before 4 fail, after 5 pass) | [button](https://raw.githubusercontent.com/ccheever/exact2/530d205f11aaf7207b1ed9b7148d349e5eedaf3a/restore-defaults-and-timeline-glyph/rd1-button.png), [dialog](https://raw.githubusercontent.com/ccheever/exact2/dbc765fadd20a94bb57be6a23cf8d66c43b8e7c8/restore-defaults-and-timeline-glyph/rd1-dialog.png), [kept](https://raw.githubusercontent.com/ccheever/exact2/58716d8ded04abf46e7f7b96224b38082bf82152/restore-defaults-and-timeline-glyph/rd1-kept.png), [record](https://raw.githubusercontent.com/ccheever/exact2/b75b58dfd0ef587516815fae28f7b7a618fcfb35/restore-defaults-and-timeline-glyph/drive-record.txt) |
| TG-1 | pass (agent drive, `screenshot … window`; reference in headless Chrome): PR #168's Timeline, the "Pull request opened" row's rail shows the git-pull-request-arrow glyph (before: daehyeonmun2021's avatar image), the name and label beside it, as the reference (`lucide-git-pull-request-arrow`, no image) | [timeline](https://raw.githubusercontent.com/ccheever/exact2/be5d02be86d82b0839561e827b2482a5227d4c2a/restore-defaults-and-timeline-glyph/tg1-timeline.png), [record](https://raw.githubusercontent.com/ccheever/exact2/b75b58dfd0ef587516815fae28f7b7a618fcfb35/restore-defaults-and-timeline-glyph/drive-record.txt) |

Drives (agent mode, 1280×840, `target/rdtg/drive.sh` in the worktree, the same ops on both sides): before on
`t3-code-evidence-base` at `6db5cadea`; after on this branch's bundle (`b1acb571f`'s app code), one run. RD-1's
reference: T3 Code `1e2ecbd975` Electron over CDP on lane `restore-defaults-and-timeline-glyph` (backend 16580, CDP 16581);
TG-1's: the GitHub lane's primary server (16583, `daehyeonmun2021/playground`) and its own web client in headless Chrome
at 1280×840, paired through Settings › Connections in the clone. No real input needed: both rows are state and drawing
that the agent's taps and `screenshot … window` reach.

## Review round 1

An independent review found two should-fix items and no blocking one:

- `changedDeviceLabels`' doc comment (`settings-core.ts`) still said it covered every device-local General/Appearance
  value of "Restore device defaults". It now says what it does: it reports every device-local key that is off its
  default, the five a restore keeps (KEPT_ON_RESTORE, Plan mode) included, and `restoreLabels` lists only
  `RESTORE_ROWS`' keys among them. Comment only (the line count is unchanged), so no behaviour or UI changed.
- `rd1-button.png` showed the before/after difference as a dimmed label at full-window scale, under the "Update Available:
  Codex" toast (the lane's known limit: the clone's server finds the Mac's real `codex`). It is recomposed from the same
  drive's screenshots (`before/11-general-restore.png`, `after/11-general-restore.png`, the reference's
  `r2-general-restore-disabled.png`): the header row only (window points 1000,4 to 1280,52, above the toast, which starts
  near y 84), zoomed 3x, each tile labelled with the drive's state (before `disabled=False`, after `disabled=True`,
  reference `disabled=true`). No new drive: this round changed no app code since the one live drive.

## Tests

- `restore-defaults-and-timeline-glyph.test.ts` (5): RD-1, every key this app keeps is either in the reference's
  restore patch (transcribed, its client-side keys) or `useTheme`'s theme keys, or one the reference keeps, never both,
  and every client-side key of the patch is one this app keeps; with every client and device value changed, a restore
  resets the reference's and keeps Font smoothing, Sidebar (legacy), the legacy sort, Advanced and Plan mode, and the
  list has no "Font smoothing"; Font smoothing alone leaves `restoreCount` 0, with Chat width the list is "Chat width."
  and a restore keeps the five and writes nothing to the server. TG-1, the lifecycle branch draws `PrdIconMarker` and no
  `PrdMarker` or `PrAvatar`, names the actor, and the marker is the reference's box and glyph; the opened row keeps its
  actor for the name. Against the feature tip's sources: 1 pass, 4 fail.
- `settings-core.test.ts` unchanged and passing (its restore cases never changed one of the five).

## Checks

On `4f3f93def`, review round 1's head: the comment fix (`414d5acb6`) merged with `origin/feat(example)/t3-code` at
`8fa9446cc` (main adoption round 10 among it; no file of this task changed in the merge), once:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4710 pass, 1 skip, 0 fail (318 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,418 lines |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | every file within its cap (`settings-core.ts` 754, `pages-pr-timeline.contract` 387 lines) |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (95 binaries); the color-gallery `button_migration` failure of the first round's head (`ee0bd2f44`) is fixed by the merged main adoption |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

`host/apple/build.mjs t3-code-macos --bundle`: 0, once, before the live drive (`b1acb571f`'s app code; since then the app
code changed by one comment line and the merge, which touched none of this task's files). The first round's checks, on
`ee0bd2f44`: the same, except `cargo test` exit 101 (3,744 passed, 1 failed, the color-gallery `button_migration` test
inherited from the feature tip then) and `app.contract` 1,398 lines.

## Not done / not verified

None. No real-input rows: both rows are state and drawing the agent's taps and `screenshot … window` reach.

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

This task has no real-input steps. On the same bundle G2-04 (every quit mode as the reference, [G2-04-double-hint-pair.png](https://raw.githubusercontent.com/ccheever/exact2/ce58c483fa3c5994a24469f0da7a8444bc19b15a/realinput-1011c/b3-window-activity/pairs/G2-04-double-hint-pair.png)) passed, and G4-15 (the clone redone in the session's prep, agent mode) passed. TG-1 ends `STATUS.md`'s "Pull request opened" row.

## Delivery

Merged on 2026-10-12 as `84d8c3474` (#470, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`.
