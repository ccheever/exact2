---
name: 20261011-adopt-main-fixes-r9
plan: 20261005-t3code-macos-parity
implementation: done
verification: pending
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-adopt-main-fixes-r9'
pr_url:
verified_commit:
---

# Main adoption, round 9: merge current main into the T3 branch

## Outcome

`feat(example)/t3-code` takes current `main`. Round 8 ([adopt-main-fixes-r8](closed/20261010-adopt-main-fixes-r8.md),
#402) adopted main `d413487a8` and landed by fast-forward, so main `d413487a8` is an ancestor of the T3 branch and this
round is a plain `git merge origin/main` (no `-s ours` step). On 2026-10-11 main had 128 commits the T3 branch
(`6db5cadea`) lacks (`d413487a8..dd589856e`), among them LLP 1116's bake-off rounds (D1–D8: the web's designed default
look, iOS layout and navigation, `state … persist`, the warning channel in `contract build`, headings in the emphasized
weight, `aria-value*` on any node (`43f6d298c`) and a determinate `progress value=… max=…` (`ab9110c3a`), exact:content,
patterns), resource answers kept as bytes (#441) or forgotten when too big (#432), `fetch`'s `redirect` (#415), CSS
`text-box` (#442), a percentage height bound on columns (#435), a press's screen laid out ahead of its release, idle
workers that cost nothing, and the issue files of the clone's X74–X83 (#405 … #430). The user decided on 2026-10-11 that
this round is built now (no further decision needed).

| Id | Clone | What this round does |
| --- | --- | --- |
| AD-1 | The branch lacks main's last 128 commits (`d413487a8..dd589856e` at writing; take main's tip at work time) | A real merge of `origin/main` (never squash or rebase, so the PR lands as a merge that keeps main's ancestry); conflicts resolved keeping main's framework and the clone's `examples/t3-code`; the clone adapted to whatever main renamed or now refuses (Contract, main's examples sweep, the compiler stack depth X67); a clone workaround retired only when a merged main fix makes it unnecessary and the change is trivial, every other retirement listed for the coordinator. The clone's progress bars are not switched to main's determinate `progress` (a stacked follow-up task does that) |

## Steps

1. `git merge origin/main` at main's tip. Keep the T3 side for `examples/t3-code/` and main's side for framework files.
   After the merge, `git diff origin/main -- ':!examples/t3-code' ':!Cargo.lock' ':!Cargo.toml'` must be empty.
2. Check what the clone relies on in main's new commits and record each with its outcome: Contract refusals and
   renames (`contract build`, main's examples sweep `cargo test -p contract --test it`), the new `contract build`
   warnings (LLP 1116 D6), headings' emphasized weight (D7), `aria-value*` and determinate `progress` (D8, #279: list,
   do not switch), the agent and build scripts the clone's tools call (`--help` and unknown-flag refusals, D4's
   stale-build rebuilds), resource answers (#441, #432), `fetch` `redirect` (#415), a press laid out ahead of its
   release, pictures and idle workers, `rules/` and the docs.
3. #327 (bucket 2): check whether it merged; if it did, its retirements (STATUS "Next up" 4) are listed or done.
4. Main's `issues/` holds the clone's framework gaps; check which of the clone's X records main closed.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| AD-1: the branch contains main | `git merge-base --is-ancestor origin/main HEAD` at the final merge | text |
| AD-1: the framework tree is main's | the `git diff` of step 1 is empty | text |
| AD-1: the five checks pass on the merged tree | CLAUDE.md's five checks once on the final head | text |
| AD-1: the clone works | `bun test examples/t3-code`, strict tsc, `contract build`, `cargo test -p t3-code-macos --lib`, the AppKit binaries (README recipe), the macOS bundle; ONE agent-mode drive (launch, open a thread, Settings › Providers, Usage) on the before bundle (evidence-base) and on this branch's | before / after screenshots side by side |
| AD-1: each item of step 2 has an outcome | the record's table lists each with its commit, or "nothing to change" and why | this record |

## Reference

- Method: [adopt-main-fixes-r8](closed/20261010-adopt-main-fixes-r8.md) (Steps, "Adopted main", checks, Delivery) and
  [adopt-main-fixes-r7](closed/20261010-adopt-main-fixes-r7.md) (history; X67 and the flatten,
  [view-depth-under-test-stack](closed/20261009-view-depth-under-test-stack.md)).
- STATUS.md "Next up" item 4 (the #327 retirements list) and `issues/README.md`'s mapping table.
- The reference T3 Code source for D7's weights: `apps/web/src/components/settings/settingsLayout.tsx` (SettingsSection
  title `<h2 className="… text-sm font-normal text-foreground/70">`), `ThemeSettings.tsx` ("Color scheme" and "Themes",
  `<h3 className="… text-sm font-normal text-foreground/70">`). No other clone feature changes; the drive compares the
  clone before and after the merge.

## Adopted main

**`ccc9afd52`** (main's tip at the final merge, 129 commits past round 8's `d413487a8`):

- `cbeac7803`: this record's first version.
- `01ea0020e`: `git merge --no-ff origin/main` at `dd589856e` (the 128 commits above). No conflict: main has no
  `examples/t3-code`, and `Cargo.toml` / `Cargo.lock` merged clean (their difference from main is the
  `examples/t3-code/macos` member and the `t3-code-macos` package entry only).
- `86d52d68b`: the clone's one source change (headings' weights, below).
- `3c1d3b9c1`: main moved one commit while the round ran, `ccc9afd52` (#446, X73: Escape closes an auto popover after
  `blur()`), merged before the drive and the final checks; no conflict.
- `40486ea5d`: `EXACT2-GAPS.md` X49, X55 and X73 rows (fixed on main, adopted here).

On the final head, after a fresh fetch: `git merge-base --is-ancestor origin/main HEAD` yes (origin/main `ccc9afd52`),
`git rev-list --count HEAD..origin/feat(example)/t3-code` 0, and `git diff origin/main HEAD -- ':!examples/t3-code'
':!Cargo.lock' ':!Cargo.toml'` empty. The PR must land as a merge (or a fast-forward of the feature branch to its head,
as #402 did), never a squash: a squash drops main from the branch's ancestry.

## Each item of step 2

| Item | Main commit | Outcome | Clone change | Commit |
| --- | --- | --- | --- | --- |
| Contract refusals and renames | all | `contract build examples/t3-code/app.contract` exit 0 on the merged tree (6,157 slots, 110,195 nodes, 48 resources); main's examples sweep (`contract` `it` `every_app_corpus_and_example_compiles…`, part of `cargo test`) passes with the clone (the five checks); `view-depth.test.ts` (X67's measure on main's `contract-lower`) passes in the Bun suite | nothing | — |
| `contract build` warnings (LLP 1116 D6) | `730528b08`, `543d81cf0`, `9f0bd1ee2` | 231 warnings, no error. 230 `lower-colour-one-scheme`: the clone writes the reference's literal tokens (white on a primary button, the media overlays' blacks, `shapes.contract`'s 229 icon colours); it is a parity copy of a web app, so the author's colours stay (author > platform, as r8 recorded for `no-tells`). 1 `type-performance-now-persisted` at `app.contract:917` (`coreThemeLive`'s `n`): the theme colour picker's ordering stamp, kept only in the open draft's in-memory `pickerSeq` (`settings-appearance-editor.ts` `themeLocal`), never stored, so a relaunch cannot meet an old stamp | nothing | — |
| Headings take the emphasized weight (LLP 1116 D7) | `a6a49a015` | A `text role="heading"` with no written `font-weight` is now bold (700) for levels 1–3, where it was the title style's regular (400). Seven clone headings relied on that regular weight; the reference writes `font-normal` on each: Settings › General's section titles (`settings-rows.contract`), Appearance's "Color scheme" and "Themes", Archive's group titles, the provider editor's title, Scheduled tasks and Third-party notices. Each now writes `font-weight=400`. The others write their weight on the line or in their `style` class; the hidden ones (`opacity=0`) draw nothing | 7 headings `font-weight=400`; `headings.test.ts` holds every drawn text heading to a written weight (fails on the old `settings-rows.contract`) | `86d52d68b` |
| `aria-value*` on any node, determinate `progress` (LLP 1116 D8) | `43f6d298c`, `ab9110c3a` (#279 closed; #278's `progressbar` part) | Fixes X49 and X55's `progressbar` part. **Not switched here** (the user's split, 2026-10-11): the clone keeps `ProgressValue` (`progress-value.contract`: an invisible indeterminate `progress` named with the percentage beside each `aria-hidden` drawn bar) on the context window meter (`composer-controls.contract:693`) and the provider runtime download (`providers-setup.contract:361`); the follow-up gives the drawn bars `role="progressbar"` and `aria-valuenow`/`-valuemin`/`-valuemax` (or `progress value max`) and removes `ProgressValue` | `EXACT2-GAPS.md` X49, X55 rows | `40486ea5d` |
| Escape closes an auto popover after `blur()` (X73) | `ccc9afd52` (#446) | Fixes X73. The clone's workaround, the Pull Requests Filters popover's rest box (`pages-prs.contract` PrFiltersMenu: `closeSub` focuses `pr-filters-rest` instead of `blur()`), still works and is **not retired here**: its proof (realinput-1010c RC-3) was a real second Escape, so the retirement belongs with a real-input check. Listed below | `EXACT2-GAPS.md` X73 row | `40486ea5d` |
| Scripts: `--help` and unknown-flag refusals; D4's builds before a drive | `3f7f289b1`, `8fbd6c5b1` | The clone's commands ran unchanged: `host/apple/build.mjs t3-code-macos --bundle`, `terminal-host/build.mjs`, `stage-runtime.mjs --offline`, `scripts/exact.mjs contract build`, and `clone-drive.sh`'s `agent.mjs macos --json --size 1280x840 --storage …`. `agent.mjs` still refuses a stale build and never builds (only an `exact new` app's `exact.mjs` builds first, `agent-build.mjs`); the drive ran on a fresh bundle | nothing | — |
| A resource's answer kept as bytes, forgotten when too big (#441, #432) | `833aac7cf`, `644df5141` | The after drive's log says once each that `settingsCore`, `shell`, `data`, `shortcuts`, `markdown` and `usage` answered more than 4 KiB, so the next launch cannot show them before they answer again. They were over the budget before as well (dropped without a word); now an older kept entry is forgotten too. The clone's first window is the connecting state (#117, a declared difference), so no launch screen changes | nothing (a smaller first-screen resource would be a feature, listed below) | — |
| `fetch` honours `redirect` (#415) | `c001a862b` | The clone's own fetches (`r5-composer-paging.ts`, the terminal page's Ghostty wasm) pass no `redirect`; the default `follow` is unchanged | nothing | — |
| A press's screen laid out ahead of its release (runner `foresee`) | `d87ee71ee` | Every press goes through it. The drive's six taps answered on the same node, at the same point, with `delivery: platform`, as before; the screens after them are pixel-identical (max channel difference 7 of 255, the header art's animation) | nothing | — |
| A long conversation opens at its newest message | `06b6636f5` | `Collection.swift` keeps a port's start across a batch whose insets changed; AppKit insets no list's port (`portInsets` nil), so nothing moves on the Mac. The drive's `layout transcript`: the same scroll (`sy` 0, its content shorter than the port), frame and size before and after | nothing | — |
| Pictures and idle workers | `e20d365be`, `2ef81a679`, `f71ed611d` | The drive's pictures (the header art, provider marks, avatars) draw identically | nothing | — |
| CSS `text-box` (#442), multi-column height (#435), `-exact-fit`, `state … persist`, exact:content, iOS (D2, `46d0fe241`, `787e8151d`, `c95c19cb3`, `598ece478`), the web's look (D1) | various | New vocabulary the clone does not use (`text_box_edge` reads `initial auto` in the layout reply), or other hosts | nothing | — |
| The agent's accessibility tree | `43f6d298c` | Drawn scroll bars now read `scrollbar` (the range role) where they read `AXScrollBar`; the same elements, frames and order. Headings and their levels are unchanged in Settings › General and Providers | nothing | — |
| Docs and rules | `d4d5db7a4`, `d11053c0c`, `188889db7`, … | `rules/RULES.md` unchanged; `rules/DEFERRED.md` gains only grouped swipe actions on iOS (LLP 1116.001 D1). `AGENTS.md` adds `docs/recipes/` and `--help`. No rule changes the clone | nothing | — |
| #327 (bucket 2) | open (checked 2026-10-12) | its retirements (STATUS "Next up" 4) wait for it | nothing | — |
| Issues | `2002d5a31` … `2d04f681a`, `ccc9afd52` | X74–X83 live in main's `issues/`; X73's file moved to `issues/closed/` by #446; the aria-range file (X49) was removed by `ab9110c3a`; #278's file records the `progressbar` part. Nothing to copy | `EXACT2-GAPS.md` | `40486ea5d` |

## Retirements for the coordinator

Each is a separate task (none is done here, beyond the headings, which main's D7 forced):

1. **X49 / X55, the progress bars** (the stacked follow-up the user named): replace `ProgressValue`
   (`progress-value.contract`) on the context window meter (`composer-controls.contract:693–696`) and the provider runtime
   download bar (`providers-setup.contract:361–364`) with the drawn bar's own `role="progressbar"` and
   `aria-valuenow`/`aria-valuemin`/`aria-valuemax` (and `aria-valuetext`, the reference's percentage), or a determinate
   `progress value=… max=…`; drop `progress-value.test.ts`'s workaround rows.
2. **X73, the Filters rest box**: in `pages-prs.contract` PrFiltersMenu, `closeSub` can `blur()` instead of
   `focus("pr-filters-rest")` and the `pr-filters-rest` box goes; `menu-keys.test.ts:289–296` change with it. Proof:
   real-input RC-3 again (Pull Requests › Filters › Author, move the pointer into the submenu, Escape, Escape: Filters
   closes), or the agent's window key (`type t3-code key Escape`, nothing focused) after `closeSub`.
3. **#327** (open): the list in STATUS "Next up" 4 (X63, X64, X62's rationale, X52, X51, X50, #282, #292) once it merges.
4. Optional, not a retirement: the six resources over the kept-answer budget (#432's log) could leave a smaller
   first-screen resource, if the clone ever shows a kept screen at launch (today #117's connecting window comes first).

## Acceptance results

Evidence: [drive.sh](https://raw.githubusercontent.com/ccheever/exact2/333fdfb367784305ff59d95f9dc71fb47f10b03e/adopt-main-fixes-r9/drive.sh.txt).

| Row | Result | Proof |
| --- | --- | --- |
| AD-1: the branch contains main | pass: `git merge-base --is-ancestor origin/main HEAD` on `40486ea5d` with origin/main `ccc9afd52` (`dd589856e` at the first merge) | Adopted main |
| AD-1: the framework tree is main's | pass: the step 1 diff is empty on `01ea0020e`, `3c1d3b9c1` and `40486ea5d` | Adopted main |
| AD-1: the five checks pass on the merged tree | pending (final head) | Tests |
| AD-1: the clone works | pass: Bun 4,654 pass / 1 skip / 0 fail; strict tsc; contract build; the macOS bundle; AppKit 38 binaries (37 pass, `mermaid` against a scratch server; `snapshot` see below) and `timeline-keyboard`; the drive: every screenshot before and after pixel-identical within 7/255 (`3-settings-general`, `4-settings-providers` exactly), trees identical apart from lane ids, AX trees apart from the scroll bars' role name, no error or refusal in the logs | [shell](https://raw.githubusercontent.com/ccheever/exact2/675d37eeb2cad795276cfb0b5f90623c722e3e52/adopt-main-fixes-r9/1-shell.png), [thread](https://raw.githubusercontent.com/ccheever/exact2/49272cb3c33daa98088b19454c210a0877c0f5fa/adopt-main-fixes-r9/2-thread.png), [Settings › General](https://raw.githubusercontent.com/ccheever/exact2/0bbf3d38944f9b7561bcc386e4ee1d02dee3a7e1/adopt-main-fixes-r9/3-settings-general.png), [Settings › Providers](https://raw.githubusercontent.com/ccheever/exact2/b13f509db6b49a05d4dfe8c62292ece7a2d1e032/adopt-main-fixes-r9/4-settings-providers.png), [Usage](https://raw.githubusercontent.com/ccheever/exact2/747d62664d2eb536270002ebe25d58e64792cbfe/adopt-main-fixes-r9/5-usage.png) |
| AD-1: each item of step 2 has an outcome | pass: the table above | this record |
| Every drawn text heading writes its weight (main's D7) | pass: `headings.test.ts` fails on the old `settings-rows.contract` (`settings-rows.contract:174`) and passes on `86d52d68b`; Settings › General's and Providers' titles draw regular, pixel-identical to the before build | Settings images |

## Tests

- `headings.test.ts`: a fourth test, every drawn `text role="heading"` writes `font-weight` (on its line or in its
  `style` class; a text whose `ChatRuns` write their weight counts; `opacity=0` ones draw nothing) and the seven that
  relied on the old regular weight say `font-weight=400`; the settings-headings test's needle follows the new line.

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | the evidence-base worktree `6db5cadea` (main `d413487a8`'s host and agent), lane `adopt-main-fixes-r9-before`, base port 16700 | one try, completed | first column of each image |
| After drive (the live drive) | the bundle of `3c1d3b9c1`'s sources (main `ccc9afd52`), lane `adopt-main-fixes-r9`, base port 16700 | one try, completed | second column of each image |
| AppKit `snapshot` | this branch and the base's sources | exit 133 at `main.swift:82` ("actual current layout translates printable letter") on both: the test reads the current keyboard layout, and this Mac's current input source is Korean 2-Set (`com.apple.keylayout.2SetHangul`); r8 passed it under a Latin layout. The module and test sources are unchanged by the merge (`git diff 6db5cadea HEAD -- examples/t3-code/modules host/apple/modules` empty). The input source was not switched (the user's) | `target/t3-tests/snapshot/run.log`, `target/t3-tests-base/snapshot/run.log` |
| AppKit `mermaid` | this branch | failed without `T3_SERVER` (9, "Mermaid failed to load") as in earlier rounds; passed against a scratch `t3 --mode web` on the lane's spare port 16703 (stopped after) | `target/t3-tests/mermaid/run-server.log` |

The drive's `layout transcript` reported `safe-area-inset-top` 52 before and 32 after (the titlebar under
`viewport-fit=cover`); main changed nothing on that path (`AgentMac.swift`, `PresenterMac.swift`, `ExactViewMac.swift`
and the clone's window chrome are unchanged), the clone reads no `env()`, and the window screenshots are identical, so it
is the window's state at that moment, not the merge.

No row needs real input.

## Next action

The coordinator reviews the draft PR, syncs `STATUS.md` and `plan.md` (round 9 done, main `ccc9afd52` adopted; X49, X55's
`progressbar` part and X73 fixed on main), lands it as a merge or a fast-forward (never a squash), and schedules the
retirements above (1 is the stacked progress-bar task).
