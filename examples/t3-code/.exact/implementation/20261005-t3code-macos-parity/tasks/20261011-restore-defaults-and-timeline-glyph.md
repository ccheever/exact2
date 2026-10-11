---
name: 20261011-restore-defaults-and-timeline-glyph
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-restore-defaults-and-timeline-glyph
pr_url: https://github.com/ccheever/exact2/pull/470
verified_commit: ee0bd2f44
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
| RD-1 | pass (agent drive, reference over CDP, Bun): Font smoothing off with Advanced on, Restore device defaults stays disabled (before: enabled); with Project grouping off the list reads "This will reset: Project Grouping." (before: "Project Grouping, Font smoothing."), as the reference; after Confirm, Project grouping is back on and Advanced and Font smoothing stay as set (before: Advanced off, so the Font smoothing row is gone, and Font smoothing on). The legacy sidebar, the legacy sort and Plan mode: the Bun test (before 4 fail, after 5 pass) | [button](https://raw.githubusercontent.com/ccheever/exact2/837713ef231561cbcbd421de986c02a71c65db51/restore-defaults-and-timeline-glyph/rd1-button.png), [dialog](https://raw.githubusercontent.com/ccheever/exact2/dbc765fadd20a94bb57be6a23cf8d66c43b8e7c8/restore-defaults-and-timeline-glyph/rd1-dialog.png), [kept](https://raw.githubusercontent.com/ccheever/exact2/58716d8ded04abf46e7f7b96224b38082bf82152/restore-defaults-and-timeline-glyph/rd1-kept.png), [record](https://raw.githubusercontent.com/ccheever/exact2/b75b58dfd0ef587516815fae28f7b7a618fcfb35/restore-defaults-and-timeline-glyph/drive-record.txt) |
| TG-1 | pass (agent drive, `screenshot … window`; reference in headless Chrome): PR #168's Timeline, the "Pull request opened" row's rail shows the git-pull-request-arrow glyph (before: daehyeonmun2021's avatar image), the name and label beside it, as the reference (`lucide-git-pull-request-arrow`, no image) | [timeline](https://raw.githubusercontent.com/ccheever/exact2/be5d02be86d82b0839561e827b2482a5227d4c2a/restore-defaults-and-timeline-glyph/tg1-timeline.png), [record](https://raw.githubusercontent.com/ccheever/exact2/b75b58dfd0ef587516815fae28f7b7a618fcfb35/restore-defaults-and-timeline-glyph/drive-record.txt) |

Drives (agent mode, 1280×840, `target/rdtg/drive.sh` in the worktree, the same ops on both sides): before on
`t3-code-evidence-base` at `6db5cadea`; after on this branch's bundle (`b1acb571f`'s app code), one run. RD-1's
reference: T3 Code `1e2ecbd975` Electron over CDP on lane `restore-defaults-and-timeline-glyph` (backend 16580, CDP 16581);
TG-1's: the GitHub lane's primary server (16583, `daehyeonmun2021/playground`) and its own web client in headless Chrome
at 1280×840, paired through Settings › Connections in the clone. No real input needed: both rows are state and drawing
that the agent's taps and `screenshot … window` reach.

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

On `ee0bd2f44` (after a fetch: `origin/feat(example)/t3-code` had no commit past this branch's base `fe0e39c91`, so no
merge was needed), once:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4666 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,398 lines |
| `cargo test -p t3-code-macos --lib`, AppKit binaries | not run | no Rust and no Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | every file within its cap (`settings-core.ts` 754, `pages-pr-timeline.contract` 387 lines) |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 101 | 3,744 passed, 1 failed: `contract` `button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` on `apps/color-gallery/app.contract:269:17` ("styled default button needs appearance=\"none\""), main's color-gallery, inherited from the feature tip (adopt-main-fixes-r9's record, #440); this branch changes nothing outside `examples/t3-code` |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

`host/apple/build.mjs t3-code-macos --bundle`: 0, once, before the live drive (`b1acb571f`'s app code, the same as the
checked head's).

## Not done / not verified

None. No real-input rows: both rows are state and drawing the agent's taps and `screenshot … window` reach.

## Delivery

Draft PR [#470](https://github.com/ccheever/exact2/pull/470) into `feat(example)/t3-code`.

## Next action

The coordinator reviews the draft PR and merges it.
