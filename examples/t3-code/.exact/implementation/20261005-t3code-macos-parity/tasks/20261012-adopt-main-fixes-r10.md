---
name: 20261012-adopt-main-fixes-r10
plan: 20261005-t3code-macos-parity
implementation: done
verification: pending
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r10'
pr_url: https://github.com/ccheever/exact2/pull/472
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

## Adopted main

**`f084cc1c9`** (main's tip at the final merge, 79 commits past round 9's `ccc9afd52`):

- `ccb5364be`: this record's first version.
- `ff42bf623`: `git merge --no-ff origin/main` at `8e7d9cbb0` (#447, #451, #454, #455, #457, #459, #461, LLP
  1115/1116's waves, `ccd3d3be7`). No conflict.
- `958299f7c`: the retirements (RT-1 to RT-4, RT-6); `920afc609`: `EXACT2-GAPS.md`, `STATUS.md`, `issues/README.md`.
- `0af2a208d`: RT-6's two dialog roles the reference does not have (found in the first after drive, below).
- `6bb7e084d`: main `aec9a93a5` (two LLP commits, merged before the bundle); `fb6081a2a`: main `f084cc1c9` (10 commits:
  the Android reader and Canvas host, `apps/caltrain/android`, iOS navigation #449, LLP 1115.000/1115.001 accepted). The
  one conflict was `Cargo.toml`'s `members` line (main added `apps/caltrain/android`, the T3 branch carries
  `examples/t3-code/macos`): main's line plus the clone's member. `Cargo.lock` merged clean.

On the final head after a fresh fetch: `git merge-base --is-ancestor origin/main HEAD` yes, `git rev-list --count
HEAD..origin/feat(example)/t3-code` 0, and `git diff origin/main HEAD -- ':!examples/t3-code' ':!Cargo.lock'
':!Cargo.toml'` empty (the `Cargo.toml` / `Cargo.lock` difference is the `examples/t3-code/macos` member and the
`t3-code-macos` package only). The PR must land as a merge (or a fast-forward of the feature branch to its head), never a
squash: a squash drops main from the branch's ancestry.

## Each item of main's new commits

| Item | Main commit | Outcome | Clone change |
| --- | --- | --- | --- |
| Contract refusals and renames | all | `contract build` exit 0 on the merged tree before any clone change (6,161 slots, 110,198 nodes, 48 resources) and after it (110,194 nodes) | nothing forced |
| New `contract build` warnings: `analyze-read-after-assign` (LLP 1116.003 D6) | `42e0da95d` | 19 warnings at 9 sites, all reads the runtime already did (writes land when the action ends; the warning is new, the behaviour is not). Read each: `confirmUi` (`app.contract:1385`), `coreThemeDialog` (`:906`), `showSnapshotSetup` (`app-window.contract:467`, `:472`) and the rename `key` (`:541`) read the value from before the action on purpose; `paletteCursor` (`app.contract:313`/`:321`) is assigned and read in exclusive `if` arms (`ArrowUp`, `Back`); `sidebarDragMove`'s `dragDx`/`dragDy` (`app-window.contract:579`/`:591`) are 0 at a drag's start anyway (reset at its end, `:601`). One is a real bug: a row-action sweep (`sidebar.contract:254`-`262`) that starts after an earlier one keeps the last sweep's `sweepDx`/`sweepDy` to its release and places its first move from the last `sweepY` (the reset's writes land only when the action ends). Reproduced in a one-file app in review round 1 and listed in `STATUS.md` "Found, not in scope" (below, "Review round 1"); not changed here (behaviour as before) | none |
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
- **RT-4 (X73).** `closeSub` calls `blur()` when the pointer left the submenu's row (Base UI leaves the focus on BODY) and
  the `pr-filters-rest` box is gone. With nothing focused, the next Escape reaches main #446's `MenuHost.key` path and
  closes Filters. One thing the rest box gave and `blur()` does not: from the rest box the closing popover returned the
  focus to the Filters button, as the reference's FloatingFocusManager does from BODY; now the focus stays nowhere
  (below, "Decision needed"). While option (a) stands this is a declared difference: `EXACT2-GAPS.md` X73 (and X66's
  blocked list) and `STATUS.md` "Known differences" say so (review round 1).
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
| RT-4 | pass for the retirement: after Escape in "Search authors" (pointer left the row) nothing is focused (before: the rest box, node 937; reference: BODY), and the window's own Escape (`type t3-code key Escape`, nothing focused) closes Filters. Differs from the reference after the close: the focus stays nowhere, the reference returns it to Filters ("Decision needed"; declared in `EXACT2-GAPS.md` X73 and `STATUS.md` "Known differences" while option (a) stands). Before, the focus also returned to Filters (node 929), but an agent-mode window is never key, so its column draws no ring. Real keys: batch step 1 | [first Escape](https://raw.githubusercontent.com/ccheever/exact2/ad3986bcf662c4e6854c098e21af7977ec8d0368/adopt-main-fixes-r10/rt4-filters-escape.png), [second Escape](https://raw.githubusercontent.com/ccheever/exact2/822c4572c2f93c49548b5b35a60a47234233e006/adopt-main-fixes-r10/rt4-filters-closed-v2.png) (each column's focus written on it), [focus text](https://raw.githubusercontent.com/ccheever/exact2/d19c906677a5b1a5b2e9269a5cd580e159a851c9/adopt-main-fixes-r10/rt4-filters-focus.txt) |
| RT-5 | agent pass for X76 (an agent right-click on unselected "Text generation": the shell menu's Copy disabled before, enabled after); records updated; real input open (batch steps 2 and 3) | [menu log](https://raw.githubusercontent.com/ccheever/exact2/23b4b5e5f7881ab290402094ca554919e343425b/adopt-main-fixes-r10/rt5-textmenu.txt) |
| RT-6 | pass: the toast is `AXApplicationDialog` (before: not exposed), the provider banner `AXApplicationAlert` (before: plain text), "No settings found" `AXApplicationStatus`; the Settings page is no dialog (the first after drive showed `AXApplicationDialog "Settings"` before `0af2a208d`); `adopt-main-fixes-r10.test.ts` | [AX text](https://raw.githubusercontent.com/ccheever/exact2/50a0f72ac4edbff2c1ef6de71ffc77fb672bca4d/adopt-main-fixes-r10/rt6-roles-ax.txt) |
| RT-7 | none to retire (read; Cause and fix) | this record |

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
   window shows the ring on Filters; this branch is expected to show none (Decision needed).
2. **RT-5 (X76).** Settings › General: right-click a word of a section title that is not selected ("Text generation"):
   the word is selected and the shell's menu shows Copy enabled; choose Copy and paste into the composer: the word.
3. **RT-5 (X78).** A thread whose reply has a Markdown table with a web link in a cell (or any reply link): with
   Accessibility Inspector (or `osascript`/AXUIElement from another process) read the link: `AXLink` with `AXURL` equal to
   the link's address (before #455 the run answered `kAXErrorInvalidUIElement`). Optional: right-click a provider docs
   link in Settings › Providers: the shell's menu offers Copy Link.
4. **RT-1 under real input (optional).** Click into the composer and into Settings › Appearance's prompt preview: no
   focus ring on either.

## Tests

- `settings-prompt-preview.test.ts`: the composer's and the prompt preview's textareas say `outline="none"` (RT-1).
- `headings.test.ts`: Legacy features' heading is the trigger's first child, `role="heading" aria-level=2`,
  `font-weight=400`, and no `SettingsSrHeading` is left in `settings-rows.contract` (RT-2); the D7 weight list counts the
  two section-title lines.
- `pages-usage.test.ts`: `formatUsd` is `Intl.NumberFormat` (source), the fixed ICU answers and the 16,000-value sweep
  (RT-3).
- `menu-keys.test.ts`: `closeSub` calls `blur()`, no `pr-filters-rest` (RT-4).
- `adopt-main-fixes-r10.test.ts` (new, 3): explicit `aria-live` only on the browser profile removal error and the toast
  region; the provider setup's status lines keep `role="status"`, the Codex description has none; the Settings surface
  and the pull request link preview write no role (RT-6). `dialog-trap.test.ts` drops the two from its dialog inventory.

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

## Not done / not verified

- Real input (steps 1–3 above): reserved for the coordinator's `realinput-1011c`; this task used no real input.
- The live drive ran on `0af2a208d`'s sources over main `aec9a93a5`; the final head adds main `f084cc1c9` (Android, iOS
  navigation, LLP docs; nothing on the Mac path) and this record. Not re-driven (the brief's one retry was used).

## Decision needed

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

- `analyze-read-after-assign` at `sidebar.contract:254`–`262`: a row-action sweep after an earlier one keeps that
  sweep's offset to its release ("Each item"). A real bug, pre-existing; listed in `STATUS.md` "Found, not in scope"
  with steps and a one-file reproduction (review round 1), not fixed here.
- The reference lane's Usage page reads Claude Code $0.08 on Oct 9 where the clone's reads $0.10: the clone lane carries
  the pages lane's `acme-coder-1` price override and the reference lane does not (the same before and after).
- The sidebar's project scope popup writes `role="dialog"` where the reference's is a Combobox popup; not changed (not
  checked which role Base UI's Combobox popup takes).

## Next action

The coordinator reviews the draft PR, answers "Decision needed", runs steps 1–3 in `realinput-1011c`, and lands the PR
as a merge or a fast-forward (never a squash).
