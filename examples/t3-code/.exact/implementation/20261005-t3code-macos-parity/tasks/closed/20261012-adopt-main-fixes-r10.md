---
name: 20261012-adopt-main-fixes-r10
plan: 20261005-t3code-macos-parity
implementation: done
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r10'
pr_url: https://github.com/ccheever/exact2/pull/472
verified_commit: 1f813789b
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
| RT-4 | X73: the Pull Requests Filters popover's rest box (`pr-filters-rest`): `closeSub` focuses it instead of `blur()` | `closeSub` calls `blur()` (the reference leaves the focus on BODY); the rest box goes; `menu-keys.test.ts` follows; an agent drive closes Filters by the window's own key path with nothing focused. **Decided (b) in review round 2: kept**, as at `fe0e39c91`: the rest box also returns the focus to Filters after the second Escape, as the reference does, until a popover close event (X66, #319) |
| RT-5 | X76, X78: the shell menu's Copy is disabled over unselected page text; a `link` node has no `AXURL` and a Markdown table cell's link runs are invalid elements outside the app | No clone change expected: `EXACT2-GAPS.md` rows and "Text context menu" S1/S3 say fixed on main and adopted, real-input check pending; real-input steps below |
| RT-6 | X55: a `status`/`alert` node was plain text (or nothing) to AppKit, and announced only with an explicit `aria-live` | Check the clone's status and alert nodes (toasts, inline errors, live regions) reach AX with the reference's roles (`AXApplicationStatus`/`AXApplicationAlert`/`AXApplicationDialog`); retire an explicit `aria-live` that only stood in for a role; the `aria-modal` subtree projection still waits on main (X55 stays partial) |
| RT-7 | X71 part 2: on macOS a press in a popover reached no ancestor's `pointerdown` | Retire a clone workaround for it, if any; else say none |
| SW-1 | Found in this round (main's `analyze-read-after-assign` at `sidebar.contract:254`–`262`): a sidebar row-action sweep reads the slots its start branch resets, so one that starts after an earlier sweep keeps that sweep's offset to its release, places its first move from the old origin and arms at once with the old verb, top and rows (or stays cancelled after an Escape), and every move arms the rows of the move before | Added in review round 2 (the coordinator): the sweep starts fresh each time, as the reference's `startActionSweep` makes a new closure per press and resolves the rows at the pointer's current position on each move; the start's values and the move's offset are `let` bindings and the slots are written from them |

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
| RT-4 | `menu-keys.test.ts`; agent drive: Filters › Author, the pointer into the submenu, Escape in the search (`closeSub`), then the window's key path with nothing focused (decided (b): FW-3's rest box check; the base build's drive is the after, the code being `fe0e39c91`'s) | before / after text (focus and popover state) |
| RT-5 | `EXACT2-GAPS.md`; real-input steps | real input (open) |
| RT-6 | `tree --ax` of the clone's status, alert and toast nodes | before / after text |
| RT-7 | reading the clone's popovers and its outside-press plumbing | text |
| SW-1 | `sidebar-sweep.test.ts` runs ThreadSection's own `sweep` and `cancelSweep` from `sidebar.contract` with Contract's action semantics: fails on the base body, passes after | before / after text (the test's output) |

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
  [realinput-1010c-fixes](20261010-realinput-1010c-fixes.md) RC-3.
- SW-1: `apps/web/src/components/Sidebar.tsx` `startActionSweep` (a new `targetKey`/`sweptKeys` closure per press; `onStart: sweepTo(originKey)`, `onMove: sweepTo(sidebarThreadKeyAtY(list, y))`), `SIDEBAR_DRAG_DISTANCE = 6`; `Sidebar.logic.ts` `resolveSidebarSweepKeys`.
- RT-6: `apps/web/src/components/settings/ProviderSetupSection.tsx` (`<p role="status">` installation status, the
  `sr-only` `role="status"` pending label), `ProviderAuthenticationSection.tsx` (`<span role="status">` account
  description), `CodexSetupSection.tsx` (the account description is plain `SettingsRow`/`CodexWelcomeCard` text),
  `IntegrationsSettings.tsx` (the profile removal error `<p aria-live="polite">`), `ui/toast.tsx` over Base UI
  `Toast.Viewport` (`role="region"`, `aria-live="polite"`) and `Toast.Root` (`role="dialog"`).
- Main: `issues/closed/` files of X72, X74, X76, X78; `issues/20261009-macos-role-modal-accessibility.md` (X55, open:
  the `aria-modal` subtree), `issues/20261010-pointer-events-reach-ancestors.md` (X71, part 1 open).

## Adopted main

**`f084cc1c9`** (main's tip at the final merge, 79 commits past round 9's `ccc9afd52`):

- `ccb5364be`: this record's first version.
- `ff42bf623`: `git merge --no-ff origin/main` at `8e7d9cbb0` (#447, #451, #454, #455, #457, #459, #461, LLP
  1115/1116's waves, `ccd3d3be7`). No conflict.
- `958299f7c`: the retirements (RT-1 to RT-4, RT-6; RT-4's undone in review round 2, `5c8fbb6e1`); `920afc609`: `EXACT2-GAPS.md`, `STATUS.md`, `issues/README.md`.
- `0af2a208d`: RT-6's two dialog roles the reference does not have (found in the first after drive, below).
- `6bb7e084d`: main `aec9a93a5` (two LLP commits, merged before the bundle); `fb6081a2a`: main `f084cc1c9` (10 commits:
  the Android reader and Canvas host, `apps/caltrain/android`, iOS navigation #449, LLP 1115.000/1115.001 accepted). The
  one conflict was `Cargo.toml`'s `members` line (main added `apps/caltrain/android`, the T3 branch carries
  `examples/t3-code/macos`): main's line plus the clone's member. `Cargo.lock` merged clean.
- Review round 2 (`5c8fbb6e1`, `ade46167f`, then records): after a fresh fetch `origin/feat(example)/t3-code` is still
  `fe0e39c91`, already in this branch, so `git merge 'origin/feat(example)/t3-code'` is "Already up to date" and makes no
  merge commit. Main is now `4c72dc622`, 62 commits past `f084cc1c9` (Android, LLP 1116.004, `efe10e0c1` and `ad3d5695d`'s
  style refusals, #422, #424); not adopted here, the next round takes them.

On the final head after a fresh fetch: `git merge-base --is-ancestor f084cc1c9 HEAD` yes (main's tip at the last merge;
`origin/main` has moved since, above), `git rev-list --count
HEAD..origin/feat(example)/t3-code` 0, and `git diff f084cc1c9 HEAD -- ':!examples/t3-code' ':!Cargo.lock'
':!Cargo.toml'` empty (the `Cargo.toml` / `Cargo.lock` difference is the `examples/t3-code/macos` member and the
`t3-code-macos` package only). The PR must land as a merge (or a fast-forward of the feature branch to its head), never a
squash: a squash drops main from the branch's ancestry.

## Each item of main's new commits

| Item | Main commit | Outcome | Clone change |
| --- | --- | --- | --- |
| Contract refusals and renames | all | `contract build` exit 0 on the merged tree before any clone change (6,161 slots, 110,198 nodes, 48 resources) and after it (110,194 nodes) | nothing forced |
| New `contract build` warnings: `analyze-read-after-assign` (LLP 1116.003 D6) | `42e0da95d` | 19 warnings at 9 sites, all reads the runtime already did (writes land when the action ends; the warning is new, the behaviour is not). Read each: `confirmUi` (`app.contract:1385`), `coreThemeDialog` (`:906`), `showSnapshotSetup` (`app-window.contract:467`, `:472`) and the rename `key` (`:541`) read the value from before the action on purpose; `paletteCursor` (`app.contract:313`/`:321`) is assigned and read in exclusive `if` arms (`ArrowUp`, `Back`); `sidebarDragMove`'s `dragDx`/`dragDy` (`app-window.contract:579`/`:591`) are 0 at a drag's start anyway (reset at its end, `:601`). One is a real bug: a row-action sweep (`sidebar.contract:254`-`262`) that starts after an earlier one keeps the last sweep's `sweepDx`/`sweepDy` to its release and places its first move from the last `sweepY` (the reset's writes land only when the action ends). Reproduced in a one-file app in review round 1; fixed in review round 2 (SW-1). **Review round 2 found this list capped**: `contract build` prints at most 20 warnings before the colours (`contract_lower::MAX_WARNINGS`, `contract/lower/src/warnings.rs`), so the 19 shown and `type-performance-now-persisted` were the first 20. A compiler build with the cap lifted (the constant changed in the worktree for one run and restored at once; nothing committed) lists 24 `analyze-read-after-assign` at 14 sites before SW-1 and 21 after it. The five not read in round 1: `browser-surface.contract:117` (`focusField`: `touched` is unchanged when `on`), `pages-prs.contract:451` (`open`: the old `sub` says the submenu is opening), `requests.contract:103` and `shell-details.contract:115` (`toggleMenu`, `toggleScripts`: `not menuOpen`, `not scriptsOpen` is the opening, commented so) read the old value on purpose; `composer-controls.contract:922` is the sweep's twin, a real bug: the queued-run grip `drag` resets `dragDy` in its start branch and then adds to the old one, so a grip drag after an earlier one (after its drop bumps `queueDropSeq`, or on another run) starts at that drag's offset, and its insertion line and drop index are shifted by it. Not fixed here (outside the coordinator's two items): listed in `STATUS.md` "Found, not in scope" with the same fix shape | SW-1 |
| `lower-colour-one-scheme`, `type-performance-now-persisted` | — | 230 and 1, as round 9 recorded (unchanged) | none |
| X61 `outline: none` | `998fe3031` (#459) | fixes X61 | RT-1 |
| X74 a heading inside a button | `845cd6af3` (#451) | fixes X74 | RT-2 |
| X72 Apple `Intl` rounding | `704459286` (#447) | fixes X72 | RT-3 |
| X73 Escape after `blur()` | `ccc9afd52` (#446, round 9) | fixed in round 9 | RT-4 |
| X76 right-click selects the word, X78 link URL to accessibility | `a961b8268` (#454), `a4613a574` (#455) | fix X76 and X78; the clone's module picks both up unchanged (RT-5) | none |
| X55 `status`, `alert`, `dialog`, `alertdialog` | `ba4fa5326` (#457) | fixes X55's role part; the `aria-modal` subtree stays open on main | RT-6 |
| X71 part 2 | `77ae46f03` (#461) | fixes part 2; part 1 open | none (RT-7) |
| A double click selects the text system's word (MAC-16), a placeholder's fallback font (MAC-40) | `5525d9991`, `18ecbf8fa` | the clone writes every field's font; no clone workaround on either | none |
| `ccd3d3be7`: an `accent-color` keeps a default button native | `ccd3d3be7` | fixes main's own `apps/color-gallery` `button_migration` red that round 9 inherited; the clone writes `accent-color` on no button | none |
| Scripts: `agent check`, `agent-build`, settled screenshots | `feb872802`, `febb8a14e`, … | the drive's commands ran unchanged (`clone-drive.sh`, `host/apple/build.mjs --bundle`, `stage-runtime.mjs`, `terminal-host/build.mjs`) | none |
| iOS, web, Android, LLP 1115/1116 docs, `rules/` | various | `rules/` unchanged; nothing on the Mac path the clone uses (`Accessibility.swift`'s change is the iOS autofocus path) | none |

## Cause and fix

- **RT-1 (X61).** The reference's editor class is `focus:outline-none` (`ComposerPromptEditorTiptap.tsx`) in the composer
  and in Settings › Appearance's `PromptFontPreview`, so neither shows a ring; on the reference over CDP the focused
  editor's `outline-style` is `none` in both. Both clone textareas now write `outline="none"` (main #459), which takes
  Exact's `exact.fieldFocus` ring and AppKit's off a focused field; the native editor (`T3ComposerEditor`) works on the
  same `NSTextView`, so the hatch keeps it. The preview keeps `appearance="none"` (the bare box).
- **RT-2 (X74).** `LegacyFeaturesSection`'s `CollapsibleTrigger` holds `<h2 className="text-sm font-normal …">`; the
  reference's ARIA snapshot is `button "Legacy features" [expanded]` › `heading "Legacy features" [level=2]`. The trigger
  now holds its `text … role="heading" aria-level=2 font-weight=400`, and the `SettingsSrHeading` before it is gone (the
  component stays for the reference's own `sr-only` headings: Appearance, Project, Scheduled Tasks).
- **RT-3 (X72).** `formatUsd` is `Intl.NumberFormat('en-US', USD, 2 fraction digits)` as `usageFormat.ts` `CURRENCY`
  (made at the first amount, not at module load); the clone's digit rounding (`plainDigits`, `incremented`) is gone.
  `formatCount` keeps its own grouping (not a workaround; the reference's `INTEGER` formatter gives the same text for an
  integer). The Bun oracle still passes (its fixed list is ICU's answers), and the running macOS app shows USD 0.825 as
  $0.83 through Hermes with main's prelude.
- **RT-4 (X73). Kept, decided (b).** Since main #446 (round 9) the window's Escape closes an auto popover with nothing
  focused, so `closeSub` could call `blur()` (Base UI leaves the focus on BODY) without the rest box; this round's first
  build did, and its drive showed nothing focused after the Escape in "Search authors" and Filters closing on the window's
  Escape. But the rest box gives one thing `blur()` cannot: when that Escape closes Filters, the closing popover returns
  the focus that was inside it to the Filters button, as the reference's FloatingFocusManager (`returnFocus`, from BODY)
  does, ring included (drive: the base build, logical focus on Filters, node 929; `blur()`, none; the reference,
  `document.activeElement` BUTTON "Filters"). The clone cannot answer the close with `focus("pull-requests-filters")`: a
  popover has no close event (X66, #319). The coordinator chose (b) by the reference parity rule: after the second
  Escape the reference returns the focus to Filters, and the base build did too, so losing it is worse than an invisible
  focus holder after the first Escape. `PrFiltersMenu` (`pages-prs.contract`: the `pr-filters-rest` box and `closeSub`'s
  `focus("pr-filters-rest")`) and `menu-keys.test.ts` FW-3 are `fe0e39c91`'s again (their comments now say why the box
  stays); `EXACT2-GAPS.md` X73 has its workaround text back with the note, and the declared difference review round 1
  added (X73, `STATUS.md` "Known differences") is gone.
- **RT-5 (X76, X78).** No clone change. The shell menu module (`T3TextContextMenu.replacement(for:)`) takes ExactKit's
  text menu wherever ExactKit shows one, so once #454 selects the word on a secondary click, the shell's menu over
  unselected text has Copy enabled; `link(at:)` already falls back to a link node's own `accessibilityURL()`, which #455
  now sets.
- **RT-6 (X55).** #457 makes a `status` node `AXApplicationStatus` (polite), `alert` `AXApplicationAlert` (assertive),
  `dialog` `AXApplicationDialog`. Clone side: (1) the explicit `aria-live="polite"` on three provider setup status nodes
  (runtime status, the sr-only pending label, the account description) only stood in for the role's own live region:
  removed, `role="status"` kept as the reference's `<p role="status">` / `<span role="status">`; (2) the Codex account
  description (Settings row and welcome card) wrote `role="status" aria-live="polite"` where the reference's is plain
  description text: both removed; (3) the toast region gains Base UI `Toast.Viewport`'s `aria-live="polite"` (it had
  `role="region"` and the name only); (4) two `role="dialog"` boxes the reference does not have, which #457 newly exposed:
  the Settings surface (a route in the reference) and the pull request link preview (Base UI's PreviewCard popup has no
  role). The one explicit `aria-live` left is the reference's own (the browser profile removal error). The 84 `status`
  and 74 `alert` nodes, the toasts' `dialog` and the provider banner's `alert` now reach AX by their roles. The
  `aria-modal` subtree (the page behind a modal) still waits on main (X55 partial).
- **RT-7 (X71 part 2).** None to retire. The clone's forwarded presses (the theme colour picker's trigger, plane and hue,
  the diff and file gutters) are nodes that hear the pointer themselves, X71's part 1, which stays open; no clone code
  stood in for a press inside a popover reaching the root's `pointerdown`. With #461 a press on a popover's plain part
  (the colour picker's header, a painted menu's padding) now counts as the window's outside press, as Base UI's does.
- **SW-1 (the sweep's carry-over).** `ThreadSection.sweep` (`sidebar.contract`) reset its slots in a start branch and
  then read them, and an action's reads see the values from before it (its writes land when it ends). On the base body
  (`sidebar-sweep.test.ts`, run on it first): a sweep on row c after one of three 10-pt moves on row a gives offset 33
  (not 3), point 44 (not 77) and arms `|a|b|` at once on a 3-pt move; one on the same row after the release keeps the
  last sweep's offset (30, not 10) and arms with its verb (`unsettle`: nothing); one after an Escape stays cancelled on
  its first move; and every move arms the rows of the move before (the 6-pt distance and `sweepAt` read the last
  move's), so the first move past 6 pt armed nothing. The reference's `startActionSweep` makes a new closure per press
  (`targetKey = null`, `sweptKeys = []`, the pressed row and action) and resolves the rows at the pointer's current
  position on each move. Fix: `let fresh`, the start's values (`mode`, `cancelled`, `armed`, `originY`, `originTop`, `lo`,
  `hi`: the slot when not fresh) and this move's offset and point (`movedX`, `movedY`, `at`) are `let` bindings; the
  start branch writes the origin slots from them, `sweepDx`/`sweepDy`/`sweepAt` take the locals, and `sweepKeys` is
  computed from them (emptied on a fresh sweep still under the distance). `cancelSweep` and the release (`sweepEpoch`)
  are unchanged.

## Acceptance results

Evidence: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/b112e8c4fb3c22eb689e951de125b5612bc77697/adopt-main-fixes-r10/drive.sh.txt).

| Row | Result | Proof |
| --- | --- | --- |
| AD-1: the branch contains main | pass (final head, origin/main `f084cc1c9`) | Adopted main |
| AD-1: the framework tree is main's | pass: the step 2 diff is empty on `ff42bf623`, `0af2a208d` and the final head | Adopted main |
| AD-1: the five checks pass on the merged tree | see Checks | Checks |
| AD-1: the clone works | pass: the same drive before (this branch at `fe0e39c91`) and after; every window screenshot is pixel-identical (max difference 0) except the two focus-ring shots (RT-1); no op failed; no app error, refusal or failure line | [pixels](https://raw.githubusercontent.com/ccheever/exact2/7d9eb04ec81815042d6385fd2b1e7610c6c05103/adopt-main-fixes-r10/ad1-pixels.txt), [shell](https://raw.githubusercontent.com/ccheever/exact2/242b4e8b9c012e72c9310e7125960a66d99bdac1/adopt-main-fixes-r10/ad1-1-shell.png), [thread](https://raw.githubusercontent.com/ccheever/exact2/ca0f358effb3132c6db44f8580a444436dd7a07d/adopt-main-fixes-r10/ad1-2-thread.png), [General](https://raw.githubusercontent.com/ccheever/exact2/4c967414f6dd2dfcce9efc3d22b0710a9698822f/adopt-main-fixes-r10/ad1-6-settings-general.png), [Legacy open](https://raw.githubusercontent.com/ccheever/exact2/5d52f9f52d56364d5a80e7e3cfe7635ab36d22d9/adopt-main-fixes-r10/ad1-7-legacy-open.png), [Providers](https://raw.githubusercontent.com/ccheever/exact2/7bf6285251dc1966228b4c50ee10a68c989813ce/adopt-main-fixes-r10/ad1-9-settings-providers.png), [Usage](https://raw.githubusercontent.com/ccheever/exact2/7e0e6eb60f13cc5b530ef96c023f903862fc1888/adopt-main-fixes-r10/ad1-10-usage.png) |
| RT-1 | pass: focused composer and prompt preview, before the 2 pt ring, after none, the reference none (`outline-style: none`) | [composer](https://raw.githubusercontent.com/ccheever/exact2/3f8ab69802190a8b46a383dead052c080d80f4bc/adopt-main-fixes-r10/rt1-composer-focus.png), [prompt preview](https://raw.githubusercontent.com/ccheever/exact2/7226af957c2955e572a47e5074390c49d93fbeb0/adopt-main-fixes-r10/rt1-prompt-preview-focus.png) |
| RT-2 | pass: before the 1×1 heading is the button's sibling; after `button "Legacy features"` › `heading [level=2]` (its parent is the button), as the reference's snapshot | [AX text](https://raw.githubusercontent.com/ccheever/exact2/be71f2eef41de8effffd11f7245b09cebc07e032/adopt-main-fixes-r10/rt2-legacy-ax.txt) |
| RT-3 | pass: Day table, Oct 9 Codex (USD 0.825) `$0.83` before (own rounding) and after (`Intl.NumberFormat`), the reference `$0.83`; `pages-usage.test.ts` passes | [image](https://raw.githubusercontent.com/ccheever/exact2/b74860fcabfde6acc43ee0f183ef4d63127084dc/adopt-main-fixes-r10/rt3-usage-day.png), [text](https://raw.githubusercontent.com/ccheever/exact2/5659b8143a96374eba51b35012022615c1344d40/adopt-main-fixes-r10/rt3-usage.txt) |
| RT-4 | kept, decided (b) (review round 2): `closeSub` focuses the rest box, as at `fe0e39c91`; `menu-keys.test.ts` FW-3 passes. The `PrFiltersMenu` code equals `fe0e39c91`'s, so the base build's drive is the after (not re-driven): after Escape in "Search authors" the focus is on the hidden rest box (node 937; the reference: BODY; no ring either way), the window's Escape closes Filters and the focus returns to the Filters button (node 929; the reference: BUTTON "Filters", ring). The `blur()` variant (option (a), built in this round's first commits and not landed) left nothing focused after the close: the middle column of each image. The focus ring itself does not draw under the agent (its window is never key). Real keys: batch step 1 | [first Escape](https://raw.githubusercontent.com/ccheever/exact2/ad3986bcf662c4e6854c098e21af7977ec8d0368/adopt-main-fixes-r10/rt4-filters-escape.png), [second Escape](https://raw.githubusercontent.com/ccheever/exact2/822c4572c2f93c49548b5b35a60a47234233e006/adopt-main-fixes-r10/rt4-filters-closed-v2.png) (the before column is this branch's code; each column's focus written on it), [focus text](https://raw.githubusercontent.com/ccheever/exact2/d19c906677a5b1a5b2e9269a5cd580e159a851c9/adopt-main-fixes-r10/rt4-filters-focus.txt) |
| RT-5 | agent pass for X76 (an agent right-click on unselected "Text generation": the shell menu's Copy disabled before, enabled after); records updated; real input open (batch steps 2 and 3) | [menu log](https://raw.githubusercontent.com/ccheever/exact2/23b4b5e5f7881ab290402094ca554919e343425b/adopt-main-fixes-r10/rt5-textmenu.txt) |
| RT-6 | pass: the toast is `AXApplicationDialog` (before: not exposed), the provider banner `AXApplicationAlert` (before: plain text), "No settings found" `AXApplicationStatus`; the Settings page is no dialog (the first after drive showed `AXApplicationDialog "Settings"` before `0af2a208d`); `adopt-main-fixes-r10.test.ts` | [AX text](https://raw.githubusercontent.com/ccheever/exact2/50a0f72ac4edbff2c1ef6de71ffc77fb672bca4d/adopt-main-fixes-r10/rt6-roles-ax.txt) |
| RT-7 | none to retire (read; Cause and fix) | this record |
| SW-1 | pass: `sidebar-sweep.test.ts` against `fe0e39c91`'s `sidebar.contract` 3 fail (a 2-pt first move puts `sweepAt` at 0, not 16; a sweep on row c after one on row a gives `[33, 74, 44, "|a|b|"]` for `[3, 74, 77, ""]`; the same row after a release keeps offset 30 for 10 and arms with the last verb), on this branch 3 pass. Not driven in the app (a real drag; batch step 5) | [test output before / after](https://raw.githubusercontent.com/ccheever/exact2/3a023d8c0fd5924cf8068b10a03624c8feee84b2/adopt-main-fixes-r10/sweep-test.txt) |

Drives (agent mode, 1280×840, `--epoch 2026-10-12T04:00:00+09:00 --time-zone Asia/Seoul`, base port 16740, the clone's
server on 16742; both lanes reset each time with the same seed, the provider CLIs pinned to an absent path, the pages
lane's synthetic transcripts and rates): before on this worktree at `fe0e39c91`'s bundle (lane
`adopt-main-fixes-r10-before`), after on this branch's bundle (lane `adopt-main-fixes-r10`). The reference is the
Electron build on lane `adopt-main-fixes-r10` (16740/16741), driven over CDP at 1280×840.

## Real-input batch steps

For `realinput-1011c` (the coordinator's session). Launch this branch's bundle as a lane copy (its own bundle id, a lane
home outside any git checkout), window 1280×840, a thread open (Verification fixture). Never send a message.

1. **RT-4 (X73, RC-3's steps).** Pull Requests (sidebar) › Filters › click Author; move the pointer into the submenu and
   click in "Search authors"; press Escape: Author closes, Filters stays open. Press Escape again: Filters closes.
   Note where the focus is after the second Escape (a ring on Filters or none): the reference T3 Code (Alpha) lane
   window shows the ring on Filters, and so should this branch (the rest box, decided (b)).
2. **RT-5 (X76).** Settings › General: right-click a word of a section title that is not selected ("Text generation"):
   the word is selected and the shell's menu shows Copy enabled; choose Copy and paste into the composer: the word.
3. **RT-5 (X78).** A thread whose reply has a Markdown table with a web link in a cell (or any reply link): with
   Accessibility Inspector (or `osascript`/AXUIElement from another process) read the link: `AXLink` with `AXURL` equal to
   the link's address (before #455 the run answered `kAXErrorInvalidUIElement`). Optional: right-click a provider docs
   link in Settings › Providers: the shell's menu offers Copy Link.
4. **RT-1 under real input (optional).** Click into the composer and into Settings › Appearance's prompt preview: no
   focus ring on either.
5. **SW-1 (optional; it settles lane threads, Undo after).** A section with four or more settleable threads: press a
   row's Settle and drag down over two rows, release (they settle; Undo). Press another row's Settle and drag down by one
   row: the armed rows start at the pressed row and follow the pointer (before the fix they were offset by the first
   sweep's drag and armed on the first move).

## Tests

- `settings-prompt-preview.test.ts`: the composer's and the prompt preview's textareas say `outline="none"` (RT-1).
- `headings.test.ts`: Legacy features' heading is the trigger's first child, `role="heading" aria-level=2`,
  `font-weight=400`, and no `SettingsSrHeading` is left in `settings-rows.contract` (RT-2); the D7 weight list counts the
  two section-title lines.
- `pages-usage.test.ts`: `formatUsd` is `Intl.NumberFormat` (source), the fixed ICU answers and the 16,000-value sweep
  (RT-3).
- `menu-keys.test.ts`: FW-3's check as at `fe0e39c91` (`closeSub` focuses `pr-filters-rest`, no `blur()`, the rest box is the
  popover's first child) (RT-4, decided (b)); the round's first commits had made it check `blur()`.
- `adopt-main-fixes-r10.test.ts` (new, 3): explicit `aria-live` only on the browser profile removal error and the toast
  region; the provider setup's status lines keep `role="status"`, the Codex description has none; the Settings surface
  and the pull request link preview write no role (RT-6). `dialog-trap.test.ts` drops the two from its dialog inventory.
- `sidebar-sweep.test.ts` (new, 3; SW-1): reads ThreadSection's `sweep` and `cancelSweep` from `sidebar.contract` and runs them
  with Contract's action semantics (a read sees the value from before the action, a `let` is visible after its line, the
  last write wins) over four rows' frames: each move arms the rows up to the pointer as it is; a sweep on another row after
  a release, on the same row after a release, and after an Escape start from the press. 3 fail on `fe0e39c91`'s body.

## Checks

Once, on `fb6081a2a` (the final merge; later commits change this record only), `target/r10/checks.sh`:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,665 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | TypeScript 7.0.2 |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6,161 slots, 110,194 nodes, 48 resources; 250 warnings, no error (230 `lower-colour-one-scheme`, 19 `analyze-read-after-assign`, 1 `type-performance-now-persisted`; "Each item"). `app.contract` 1,398 lines |
| `cargo test -p t3-code-macos --lib` | 0 | 21 pass (no clone Rust or Swift changed; run because main changed the host) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (95 suites): main's `ccd3d3be7` ended round 9's `color-gallery` red; the examples sweep compiles the clone |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | before (`fe0e39c91`), after (`6bb7e084d`) and the retry's (`0af2a208d`) |

Review round 1 (`9ada21cc2`: records only, the sources are `fb6081a2a`'s, so the Bun, tsc, `contract build` and
`t3-code-macos` rows above stand), `target/r10/checks-rr1.sh`: `git add -A && bun scripts/caps.mjs` 0 (all budgets
within cap); `cargo build --all-targets --keep-going` 0; `cargo test --lib --bins --tests --no-fail-fast` 0 (3,756
passed, 0 failed, 36 ignored, 95 suites); `cargo clippy --all-targets --keep-going -- -D warnings` 0; `cargo fmt --all
-- --check` 0; `bun scripts/boot.mjs` 0. The one Bun test that names `EXACT2-GAPS.md` (`dialog-trap.test.ts`, a comment)
passes (161).

Review round 2, once on `d66474556` (`5c8fbb6e1` RT-4 (b), `ade46167f` SW-1, `STATUS.md` and this record; later commits
change this record only), `target/r10/checks-rr2.sh`:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4,668 pass, 1 skip, 0 fail (313 files: `sidebar-sweep.test.ts` added) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 6,161 slots, 110,196 nodes, 48 resources; 250 warnings shown, no error (230 `lower-colour-one-scheme`, 19 `analyze-read-after-assign` of 21 behind the 20-warning cap, 1 `type-performance-now-persisted`; none at `sidebar.contract`; "Each item"). `app.contract` 1,398 lines; `sidebar.contract` 306, `pages-prs.contract` 817, `sidebar-sweep.test.ts` 188 |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (95 suites) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

`cargo test -p t3-code-macos --lib` and the bundle were not run again: review round 2 changed no Rust or Swift, and nothing
was driven.

The AppKit binaries were not run: no module Swift changed, and main's Apple changes since round 9 are the AX roles, the
link URL, the right-click word, the heading child, the field ring, the popover press walk and an iOS autofocus guard,
each driven above where the clone meets it.

After the checks main moved 11 commits (`5f50e9877`, among them `efe10e0c1` "refuse string arms on number-only style
rows", `ad3d5695d` lowering's `check_style`, `eff14ad1d` "preserve Exact nodes inside hatches", #422 calc sizing for
measured leaves); not adopted here (the next round takes them; the first two may refuse clone lines).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive 1 | this worktree at `fe0e39c91` (bundle built before the merge) | the composer and preview taps did not focus the native editors under the agent (its window is never key), the Usage window followed the agent's default epoch (Dec–Jan, empty), and the Filters row's leave was not driven: fixed in the drive (a `type … key ArrowRight` focuses, `--epoch`, hover steps as realinput-1010c-fixes drove RC-3) | `target/r10/drive/before` (overwritten) |
| Before drive 2 | the same | the row's leave still missing (a hover on the search before its press was needed) | overwritten |
| Before drive 3 | the same | completed | first column of each image |
| After drive 1 (the live drive) | bundle of `6bb7e084d` (main `aec9a93a5`, the sources of `958299f7c`) | completed; found the Settings surface exposed as `AXApplicationDialog "Settings"` (and, by reading, the link preview), fixed in `0af2a208d` | `target/r10/drive/after` |
| After drive 2 (the one retry) | bundle of `0af2a208d` | completed, plus a status node (settings search "zzqx": "No settings found") | second column of each image |
| Reference | Electron 1e2ecbd975 on the lane, CDP | composer and preview focused, Legacy features' ARIA, Usage Day table, the Filters Escapes, the toast region's roles | third column |

## Review round 1

An independent review of the draft PR found two should-fix problems; both are records, no source changed.

| Finding | What was done |
| --- | --- |
| RT-4: retiring the rest box loses the focus return after the second Escape, which the base build had; the X73 row said only "workaround retired"; image 5's before column draws no ring though the before build focused Filters | `EXACT2-GAPS.md` X73 declares the lost return while option (a) stands (cause, drive numbers, X66 #319 as the blocker, what (b) undoes); X66's blocked list names it; `STATUS.md` "Known differences" has the row; "Decision needed" keeps the question for the coordinator. Image 5 is recomposed with each column's focus written on it (before: Filters, node 929, no ring because an agent-mode window is never key; after: none; reference: BUTTON "Filters", ring), [rt4-filters-closed-v2.png](https://raw.githubusercontent.com/ccheever/exact2/822c4572c2f93c49548b5b35a60a47234233e006/adopt-main-fixes-r10/rt4-filters-closed-v2.png); the PR caption says so |
| The row-action sweep's carry-over (`sidebar.contract:254`-`262`) was recorded only here | Reproduced: a one-file app with the sweep's start branch and move, run headless by this worktree's `exact-terminal`, gives row b's first move an offset of 33 after three moves of 10 on row a (3 expected) and its second 36 (6 expected), so the old offset lasts the whole sweep ([repro](https://raw.githubusercontent.com/ccheever/exact2/8bdbf4b0b2f1a7637d7eb37897deb902e51d40c9/adopt-main-fixes-r10/sweep-carryover.txt)). Listed in `STATUS.md` "Found, not in scope" with steps, cause and the fix shape (`let` bindings), as the user's 2026-10-11 rule lists build observations rather than opening tasks; not driven in the app and not fixed here (it predates this PR) |

## Review round 2

The coordinator decided the two open points of review round 1.

| Decision | What was done |
| --- | --- |
| RT-4 takes option (b) by the reference parity rule: after the second Escape the reference returns the focus to Filters, and the base build did too, so losing it is worse than an invisible focus holder after the first Escape | `5c8fbb6e1`: `pages-prs.contract` `PrFiltersMenu` (the hidden `pr-filters-rest` box, `closeSub`'s `focus("pr-filters-rest")`) and `menu-keys.test.ts` FW-3's check are `fe0e39c91`'s again (comments only differ: why the box stays after #446); `EXACT2-GAPS.md` X73 has its workaround text back with the note that main fixed X73 (#446) and the box stays until X66 (#319, a popover close event) because it also returns the focus to Filters as the reference does; X66's blocked list says the same; `issues/README.md`'s X73 row likewise. The declared difference of review round 1 is gone from X73 and from `STATUS.md` "Known differences". Not re-driven: the code equals `fe0e39c91`'s, so the base build's RT-4 readouts are the after (Acceptance results) |
| Fix the sidebar sweep's carry-over, reset per sweep as the reference's sweep does, with a test that fails on the base and passes after; move it from `STATUS.md` "Found, not in scope" to this record's built rows | `ade46167f`: SW-1 (Outcome, Cause and fix, Acceptance results). The headless one-file repro needs a built `exact-terminal`, which no clone Bun test spawns, so the test is a Bun test over the contract's own action instead (`sidebar-sweep.test.ts`: it reads ThreadSection's `sweep` and `cancelSweep` from `sidebar.contract` and runs them with Contract's action semantics); 3 fail on `fe0e39c91`'s body, 3 pass after. `STATUS.md` "Found, not in scope" no longer lists it |

Found while doing it: `contract build` prints at most 20 warnings before the colours, so round 1's warning list was the first
19 `analyze-read-after-assign`; the full list ("Each item") has one more real bug of the same class, the queued
messages' grip drag (`composer-controls.contract:916`–`922`), listed in `STATUS.md` "Found, not in scope", not fixed here.

## Not done / not verified

- Real input (steps 1–3, and the optional 4 and 5): ran in `realinput-1011c` ("Real-input results" below).
- The live drive ran on `0af2a208d`'s sources over main `aec9a93a5`; the final head adds main `f084cc1c9` (Android, iOS
  navigation, LLP docs; nothing on the Mac path), review round 2's RT-4 restoration (the code of `fe0e39c91`, whose
  before drive is its evidence) and SW-1 (a drag; its proof is `sidebar-sweep.test.ts`, its real drag step 5). Not
  re-driven (the brief's one retry was used).
- The queued messages' grip drag carries the last drag's offset (`composer-controls.contract:916`–`922`): found in review
  round 2, outside the coordinator's two items; listed in `STATUS.md` "Found, not in scope".

## Decision needed

**Decided (b)** by the coordinator in review round 2 (the reference parity rule; "Review round 2"). The question as
it was asked:

RT-4: retiring the rest box (as specified) leaves no focus after Filters closes from the window's Escape, where the
reference (Base UI's FloatingFocusManager `returnFocus`, from BODY) gives it back to the Filters button, ring and all.
The rest box gave the same return incidentally (the closing popover returns the focus that was inside it). Contract has
no popover close event the clone could answer with `focus("pull-requests-filters")` (X66, #319). Options: (a) keep the
retirement (this PR): the first Escape matches the reference (nothing focused), the second leaves nothing focused; (b)
restore the rest box: both Escapes close as the reference, the focus returns to Filters, the hidden box stays. Recommended:
(b) until X66 gives a close event, if the focus return is judged worth the hidden box; else (a).

Review round 1 noted that (a) matches the reference less than the base build after the second Escape: the base build
gave the focus back to Filters (node 929), as the reference does, and (a) leaves it nowhere. After the first Escape the
two options differ only in which node holds an invisible focus (the hidden rest box under (b), nothing under (a)); a
person sees no ring either way. While (a) stands, the lost return is declared in `EXACT2-GAPS.md` X73 (and X66's
blocked list) and in `STATUS.md` "Known differences"; choosing (b) restores the rest box, `menu-keys.test.ts` FW-3's
old check and the X73 row's workaround text, and drops those sentences.

## Seen while building, not a finding row

- `analyze-read-after-assign` at `sidebar.contract:254`–`262`: a row-action sweep after an earlier one kept that
  sweep's offset to its release ("Each item"). Built in review round 2 as SW-1.
- The reference lane's Usage page reads Claude Code $0.08 on Oct 9 where the clone's reads $0.10: the clone lane carries
  the pages lane's `acme-coder-1` price override and the reference lane does not (the same before and after).
- The sidebar's project scope popup writes `role="dialog"` where the reference's is a Combobox popup; not changed (not
  checked which role Base UI's Combobox popup takes).

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, RT-4 (X73, RC-3's steps) | R10-1 (B12) | Fail (clone): the first Escape passes; after the second Escape the focus goes to the window and no ring shows on Filters (reference: the focus and its ring on Filters) | [R10-1-focus-pair.png](https://raw.githubusercontent.com/ccheever/exact2/3a72de6a243dde7070d3aef5f3a1220fcd701e2a/realinput-1011c/b12-github-lane/pairs/R10-1-focus-pair.png) |
| 2, RT-5 X76 | R10-2 (B13) | Pass | [R10-2-menu-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/R10-2-menu-pair.png) |
| 3, RT-5 X78 | R10-3 (B13) | Pass: `AXLink` with its `AXURL` from another process; the optional Copy Link step not run | [R10-3-axurl-clone.txt](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/R10-3-axurl-clone.txt) |
| 4, RT-1 (optional) | R10-4 (B13, clone only) | Pass | [R10-4-composer-clone.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/R10-4-composer-clone.png) |
| 5, SW-1 (optional) | SW-1 (B5) | Pass in the clone; not verified against the reference, whose real-input sweep armed the rows and settled nothing on release | [SW-1-mid-drag-pair.png](https://raw.githubusercontent.com/ccheever/exact2/a5982ddaf650de6fb430525d69f3975e8cfcf000/realinput-1011c/b5-sidebar-device/pairs/SW-1-mid-drag-pair.png) |

Not passed: R10-1 (the rest box kept by decision (b) gave the focus back to Filters in the agent drive; under real keys it went to the window).

## Delivery

Merged on 2026-10-12 by fast-forward (#472; the branch tip `e91bdc8af`), never a squash.

## Next action

None. Closed after `realinput-1011c`; R10-1 (the focus after Filters' second Escape) is in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
