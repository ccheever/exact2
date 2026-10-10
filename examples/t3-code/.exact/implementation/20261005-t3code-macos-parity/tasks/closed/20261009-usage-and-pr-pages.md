---
name: 20261009-usage-and-pr-pages
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-usage-and-pr-pages
pr_url: https://github.com/ccheever/exact2/pull/375
verified_commit: 7c3708cf72c34da11a249badee86e01372fd46f5
---

# Usage and Pull Requests pages: rounding, hover popover, tooltips, the environment menu, author search and Escape

## Outcome

- Usage amounts round as the reference's currency format.
- The unpriced popover opens on hover.
- Toggles and period options carry their shortcut tooltips.
- The environment menu shows full names on an opaque background.
- Pull Requests › Filters › Author has its search field.
- Escape leaves the Pull Requests page for the page before it, unless a menu, a popover or a focused editor field holds it.

Found by the 2026-10-09 desktop audit ([review](../reviews/20261009-desktop-audit.md)). Reference: T3 Code `1e2ecbd975`
as an Electron production build. Clone: `c603c22d6`, a development build.

## Findings

Evidence paths are under the repository root. They stay local and are not committed.

Fixture: the pages auditor wrote synthetic transcripts in the lane only (`target/t3-audit/lanes/pages/codex/sessions/2026/10/{01,08,09}`,
`lanes/pages/claude/projects/-audit-work`): Codex gpt-5.5 (one priority-tier turn) and gpt-5.4-mini, Claude
claude-sonnet-4-5 and claude-opus-4-1, and an unknown acme-coder-1. Drive the clone with `--epoch <now> --time-zone
Asia/Seoul` so its windows match.

| Id | Reference | Clone | Steps | Evidence |
| --- | --- | --- | --- | --- |
| PG-2 | `formatUsd` uses `Intl.NumberFormat` currency (half-expand): 0.825 → $0.83; gpt-5.5 Input $1.28; Past 24h Codex row "85.4% of tokens · $0.83". | `formatUsd` uses `Number.toFixed(2)` on the binary value: 0.825 → $0.82; Input $1.27; "· $0.82"; Day table Oct 9 Codex $0.82. | Usage › Tokens › Past 24h (Codex 0.46 + 0.365 = 0.825 USD); open gpt-5.5 in 30 days and read Cost by type › Input. | `target/t3-audit/evidence/pages/PG-2-ref.png`, `PG-2-clone.png`, `pages/ref-usage-tokens-24h.txt`, `target/t3-audit/lanes/pages/logs/clone2.jsonl` |
| PG-3 | `PopoverTrigger openOnHover`: hovering the (i) after "API estimate" shows "API estimate excludes 10.0% unpriced records.". | A `popovertarget` button: hover shows nothing; only a click opens the same text. | Usage › Cost › 30 days with an unpriced model; hover the (i), then click it. | `target/t3-audit/evidence/pages/PG-3-ref.png`, `PG-3-clone.png`, `PG-3-clone-after-click.png` |
| PG-4 | Each toggle and select option has `title` = label + shortcut: Cost (C), Tokens (T), Limits (L), Past 24h (⇧⌘1), 7 days (⇧⌘2), 30 days (⇧⌘3), 90 days (⇧⌘4). | Segment buttons and select options have no title or tooltip. | Usage; hover Cost or Past 24h and wait. | `target/t3-audit/evidence/pages/PG-4-ref.txt`, `PG-4-clone.txt` |
| PG-5 | The environment menu sizes to its content: "Daehyeon’s MacBook Pro  Ready" in full. | The menu has a computed width and reads "Daehyeon’s MacBook…  Ready"; its background is translucent enough that the headline behind it shows through. | Usage › Cost; click "All environments" in the breadcrumb. | `target/t3-audit/evidence/pages/PG-5-ref.png`, `PG-5-clone.png` |
| PG-6 | The Author submenu starts with an autofocused "Search authors" field that filters the author list (up to 10), then Anyone, the authors, or "No authors found". | Only "Anyone" (checked) and "No authors found"; no search field (`pages-prs.contract:518`). | Pull Requests › Filters › Author. | `target/t3-audit/evidence/pages/PG-6-ref.png`, `PG-6-clone.png` |
| PG-7 | `useEscapeToGoBack`: an Escape that no control consumed blurs the focus and goes back (draft → Pull Requests → Escape → draft). | Escape on the page does nothing. The Usage page has this handler (`pages-usage.contract:230-232`); the Pull Requests page has none. | From a thread open Pull Requests (sidebar). Make sure no menu or field holds Escape. Press Escape. | `target/t3-audit/evidence/pages/PG-7-ref-before.png`, `PG-7-ref.png`, `PG-7-clone.png` |

## Scope and exclusions

Included: the six findings above.

Excluded:
- Usage colours (PG-1): [app-color-scheme](20261009-app-color-scheme.md).
- Rows the audit could not compare (Cursor and ChatGPT rows, populated PR rows).
- Framework code.

## Context and guidance

Reference (`target/t3-ref/src-1e2ecbd975`):
- PG-2: `packages/shared/src/usageFormat.ts:20-22`.
- PG-3, PG-4, PG-5: `apps/web/src/components/usage/UsagePage.tsx:519-532`, `:116-123, 363, 381, 416, 442`, `:1151-1196`.
- PG-6: `apps/web/src/components/pullRequest/PullRequestListFilters.tsx:280-316`.
- PG-7: `apps/web/src/routes/_chat.pull-requests.tsx:346`; `apps/web/src/hooks/useNavigateBack.ts:19-40`.

Clone (`examples/t3-code`): `pages-usage.ts:24-28`; `pages-usage.contract:252-260, 348, 411-423, 614-618, 230-232`;
`pages-prs.contract:518`. Use the clone's hover layer (from fix-hover-cards) for PG-3 and the tooltips. `Intl` with
currency formatting is in the data runtime (X36, main #204, adopted).

Shared files: `pages-usage*.contract` with [app-color-scheme](20261009-app-color-scheme.md). Start after it merges, or rebase.

## Acceptance

Before/after evidence: one side-by-side image per scenario (base build | branch build, same state,
`screenshot <abs.png> window`).

| Id | How to verify | Before/after pair | Input |
| --- | --- | --- | --- |
| PG-2 | 0.825 USD shows as $0.83 in the Past 24h row, the Day table and the model dialog's Input $1.28. Ported `usageFormat` tests pass. | `pg2-rounding.png` | agent |
| PG-3 | Hover on the (i) opens the unpriced popover; leaving closes it. | `pg3-hover-popover.png` | agent (hover) |
| PG-4 | Hover shows "Cost (C)", "Past 24h (⇧⌘1)" and the others; the narrow layout's options carry the same titles. | `pg4-toggle-tooltip.png` | agent (hover) |
| PG-5 | The environment row reads its full name; the menu background is opaque. | `pg5-environment-menu.png` | agent |
| PG-6 | The Author submenu has "Search authors", autofocused, and filters the list. | `pg6-author-search.png` | agent |
| PG-7 | Escape on Pull Requests (nothing holding Escape) returns to the previous page. | `pg7-escape-back.png` | agent |

## Cause and fix

- **PG-2.** `formatUsd` used `toFixed(2)`, which rounds the binary value (0.825 is 0.82499…). The reference's
  `Intl.NumberFormat` (ICU) rounds the shortest decimal digits half away from zero. `Intl` in the app's runtime does not:
  Hermes's Apple `Intl` formats through an `NSNumberFormatter` whose rounding mode it never sets (half to even, so
  $0.82 again; `EXACT2-GAPS.md` X72, a local draft then, filed on main on 2026-10-10, plan record
  `../../issues/closed/20261010-x72-apple-intl-half-even.md`). `formatUsd` (`pages-usage.ts`) now rounds the shortest
  digits half away from zero itself and keeps ICU's sign for a negative or negative-zero amount. Every amount on the
  page goes through it (the rows, the Day table, the model dialog, Totals, the chart's ticks).
- **PG-3.** The (i) was a `popovertarget` button (click only), and its popup a dark bubble wrapped in a 12pt containing
  block. It is now a hover trigger on the Usage page's popover machinery, as a pooled segment's is (fix-hover-cards):
  `enterSeg("unpriced")` (Base UI's 300 ms open delay on the window's hover clock), `pin` on a press, light dismiss,
  and Escape closes it and focuses the (i). The page draws `UnpricedPopup` above the (i)'s frame in its scroll content
  (side top, centred, 4pt offset, the tooltipStyle popup: popover colour, 1pt border, 12/16 text, one line).
- **PG-4.** `usageKeys` gives each shortcut UsagePage's `shortcutTitle` (`Cost (C)`, `Past 24h (⇧⌘1)`; the effective
  binding first), and `Segment` and `UsageOption` carry it as `title` through `usageTitle` (the label alone when the
  command has no binding; Breakdown's toggles have none, as in the reference). Exact's `title` is the platform tooltip
  (`PresenterMac` sets `NSView.toolTip` from it). The agent's `tree --ax` gives no description for it (review round 1:
  the first version's note said it did); `tree <testId>` shows it in the node's props.
- **PG-5.** The menu's width came from measured texts, exactly as wide as its widest row, and the label's `line-clamp`
  cut "Daehyeon's MacBook Pro" at a word when the layout came out a fraction short. The measured width is now the
  menu's least width (`min-width`), and the row label keeps its width (`flex-grow=1 flex-shrink=0 white-space=nowrap`), so
  the menu grows to its rows. The glass is drawn opaque (`light-dark(#ffffff, #111111)`), as the Code tab's menus are:
  the macOS presenter draws no backdrop blur. Same for the Model prices dialog's environment menu, the narrow selects
  and the Pull Requests list's menus (`PrMenuBox`), which a toast read through in the audit's PG-6 shot.
- **PG-6.** `PrFiltersMenu` keeps an `authorQuery`, cleared as the Filters menu opens, whether by a press, Enter or
  Space (`openMenu`) or by ↓ or ↑ (`keyOpened`, review round 1): the reference's popup mounts afresh each time.
  `PrFilterSub` draws the reference's compact InputGroup ("Search authors"; its keys stop at the field except ↓ and
  Escape). Where the focus goes as the submenu opens is Base UI's, checked on the reference in review round 1: a press
  on Author focuses the field (its autoFocus), and →, Enter or Space focus the first row, Anyone (a `menuitemradio`).
  So a press focuses `pr-author-search` and marks the submenu `subPointer`, whose KeyMenu then sees no keyboard opening
  (`keyed` 0) and leaves the focus; a key opens it as the other submenus are, with the focus on its first row. The field
  is no longer `autofocus`: on the keyboard path it raced the KeyMenu's own focus. `prVisibleAuthors` lists the chosen
  author first, then every other whose login or name holds the trimmed lower-cased search, at most 10, or "No authors
  found". `pages-prs.ts` now sends every author with a row in the state (it cut the list to 10 before the search could
  see the rest) and their names.
- **PG-7.** The page has the Usage page's hidden Back (`PrPageBack`, `aria-keyshortcuts="Escape"`): `blur()` then
  `back()`. Back is `useNavigateBack`'s history.back(): the root keeps the trail of utility pages
  (`app.contract` `pageTrail`, `pages-hero.contract` `pageTrailNext`; a page opened from another keeps the one it left,
  one opened from a thread starts over) and `pageBack` returns to the last of them, else the thread, for the Usage page
  too (review round 1; before, both went straight to the thread). The list holds Back while no panel shows, and the
  panel (`PrPanel`) while it shows. Exact hears a shortcut button before any `key` handler, so Back gives Escape up to
  whatever owns it:
  - A pull request editor's field owns it while it has the focus, as the reference's onKeyDown hears it only from
    there (review round 1; the first version made the editors `aria-modal`, which silenced every other app shortcut
    while one was open). Each field reports its focus and blur through `local` (op `pr-escape-hold`), which `PrPanel`
    keeps as `escapeField` for the pull request it was on (`escapeHeld`; the thread's surface drops the op). The
    markdown editor counts its field and its Write and Preview toggles (`PrdModeSegment`), as its onKeyDown sits on its
    wrapper. A field the view removes sends no blur on macOS, so each editor lets go as it cancels or saves (a save's
    Escape is the page's in the reference: the editor ignores it while saving, the title's field is disabled), the
    markdown editor's ⌘↵ as its key comes up (`keyup`: the analyzer refuses the save's write and the report in one
    action, `analyze-send-twice`), and `PrPanel` lets go of the code comment draft once no draft is open and of a reply
    once its thread's reply serial moves on (`prdEscapeLive`). A new pull request, or closing the panel, starts over.
  - The page's popovers (the row checks popover, the review composer, the freshness popover, the reaction and people
    pickers) are `aria-modal` while they show, as its menus already were: a popover's showing has no event the page
    could hold Escape by (X66, #319). The Usage page's narrow selects are modal too.
  Declared in `EXACT2-GAPS.md` ("Usage and Pull Requests pages"), with what remains: the app's other shortcuts wait
  while a page popover shows (X66), and the markdown editor's Cancel and Save report no focus (X54, #283).
- Also: `probeKey` in `r5-composer-menus.ts` is a function declaration, so the import cycle through
  `r6-polish-measure.ts` no longer throws when `pages-usage.test.ts` runs alone.

## Acceptance results

Before is the evidence-base build (`950e8e2e5`, lane `usage-and-pr-pages-before`). After is this branch's bundle (lane
`usage-and-pr-pages`, embedded server on 16362). Both drives ran `target/usage-and-pr-pages/drive.sh` (the worktree's)
with the same steps, agent mode, 1280×840, `--epoch <now> --time-zone Asia/Seoul`, the pages lane's synthetic
transcripts and custom price copied into both lanes, provider update checks off (so only the Nightly toast shows, and
is dismissed). The reference is the Electron build on this lane (16360/16361), driven over CDP.

| Id | Result | Evidence |
| --- | --- | --- |
| PG-2 | Pass: Past 24h Codex "85.4% of tokens · $0.83", Day table Oct 9 Codex $0.83, gpt-5.5 Input $1.28 (before: $0.82, $0.82, $1.27); the reference reads the same. `pages-usage.test.ts` compares `formatUsd` with ICU's `Intl.NumberFormat` over 16,000 values | [pg2 triple](https://raw.githubusercontent.com/ccheever/exact2/82126c6ce55240af5dba7a1480c313f27781910d/usage-and-pr-pages/pg2-rounding.png) |
| PG-3 | Pass for opening: the pointer on the (i) for 600 ms shows "API estimate excludes 10.0% unpriced records." above it, one line, as the reference (before: nothing). Leaving: not shown live (the drive's "leave" step put the pointer on the popup itself, which keeps it open, as Base UI's does); real-input step 1 | [pg3 triple](https://raw.githubusercontent.com/ccheever/exact2/30fcfd12c0a0087095376fdd3cd946541c0c24a1/usage-and-pr-pages/pg3-hover-popover.png) |
| PG-4 | Pass for the titles: the served keybindings give Cost (C), Tokens (T), Limits (L), Past 24h (⇧⌘1) … 90 days (⇧⌘4) (live state; before: none), the toggles and options carry them (`usage-pr-pages.test.ts`). The tooltip itself shows only under a real pointer in an active app: real-input step 2. Review round 1: the rendered toggles carry them, `tree usage-metric-cost` reads `"title": "Cost (C)"` and `tree usage-period-1` `"title": "Past 24h (⇧⌘1)"` (before: none); `tree --ax` gives no description for a `title` (the first note said it did) | [pg4 text](https://raw.githubusercontent.com/ccheever/exact2/dd459d80885fe8a7b6309ddea7c55f0a7cb5f5d8/usage-and-pr-pages/pg4-toggle-tooltip.png), [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt), [round 1 text](https://raw.githubusercontent.com/ccheever/exact2/41bf67b8e8366a86d9036b1e3ebbe87f32460dda/usage-and-pr-pages/text-r1-before-after.txt) |
| PG-5 | Pass: "Daehyeon's MacBook Pro  Ready" in full, the menu as wide as the reference's, on an opaque popup (before: "Daehyeon's MacBook…", the headline through it) | [pg5 triple](https://raw.githubusercontent.com/ccheever/exact2/3960497b82649ccf4e4ba23d395da9e3ffde3bfb/usage-and-pr-pages/pg5-environment-menu.png) |
| PG-6 | Pass: a press on Author focuses "Search authors" (live: focus `pr-author-search`), typing "zz" reads "No authors found", ↓ moves to Anyone, Escape closes the menus and the page stays (before: no field). Review round 1: →, Enter or Space on Author focus its first row, Anyone, as the reference's Base UI submenu does (reference: `menuitemradio` "Anyone"; live: focus `pr-author-anyone`); Filters opened again by ↓ (not only by a press) starts with an empty search (live: value "" after "zz", as the reference). Filtering with authors is unit-tested (no pull requests in either fixture) | [pg6 triple](https://raw.githubusercontent.com/ccheever/exact2/8c7bb707f4beebe45e60d415240916999d526e48/usage-and-pr-pages/pg6-author-search.png), [round 1 triple](https://raw.githubusercontent.com/ccheever/exact2/10247bcaf459a53a83bc3a45a0a7604ddfcce574/usage-and-pr-pages/r1-pg6-author-focus.png), [round 1 text](https://raw.githubusercontent.com/ccheever/exact2/41bf67b8e8366a86d9036b1e3ebbe87f32460dda/usage-and-pr-pages/text-r1-before-after.txt) |
| PG-7 | Pass: thread → Pull Requests → Escape → the thread (before: Escape does nothing); with the Filters menu open, the first Escape closes it and the page stays, as the reference. Review round 1: Escape goes back to the page before, as history.back(): Usage → ⌘K "Open pull requests" → Escape → Usage → Escape → the thread (live `utilityPage` "usage" then ""; reference `#/usage` then the thread; before: the page stays). The editors hold Escape only while their field has the focus and are not modal: not driven (no pull request in either fixture), unit-tested and declared in `EXACT2-GAPS.md` | [pg7 triple](https://raw.githubusercontent.com/ccheever/exact2/d782c8d5c44d8e35b0a0e1196078a47bd9e3f3d9/usage-and-pr-pages/pg7-escape-back.png), [round 1 triple](https://raw.githubusercontent.com/ccheever/exact2/85f181925cc50180d448f287a057d372c6129c40/usage-and-pr-pages/r1-pg7-escape-history.png), [round 1 text](https://raw.githubusercontent.com/ccheever/exact2/41bf67b8e8366a86d9036b1e3ebbe87f32460dda/usage-and-pr-pages/text-r1-before-after.txt) |

## Tests

- `pages-usage.test.ts`: PG-2's half-away-from-zero cases and an ICU oracle sweep (Bun's `Intl.NumberFormat`, as
  Chrome's) over 16,000 amounts, -0, 1e21, the largest double, NaN and the infinities; PG-4's titles from the default
  keybindings, a command bound twice titled by its effective binding, `shortcutGlyphs`.
- New `usage-pr-pages.test.ts` (reads the Contract sources): PG-3's trigger and popup wiring and placement; PG-4's titles
  on every toggle and option, none on Breakdown's; PG-5's `min-width`, the label that keeps its width, no translucent
  popups left; PG-6's `prVisibleAuthors` evaluated over a list (chosen first, login or name, trimmed, at most 10, none),
  the field's focus and keys, `presentList` sending all authors with names; PG-7's Back and the Escape owners'
  `aria-modal`.
- Review round 1, in `usage-pr-pages.test.ts`: PG-6's reset on a ↓/↑ opening (`keyOpened`) and where each opening puts
  the focus (a press: the field; a key: the first row); PG-7's `pageTrailNext` evaluated page by page (Usage → Pull
  Requests → back → back, a longer trail, a thread between), every Back wired to `pageBack`, the list's and the
  panel's `PrPageBack`, no editor `aria-modal`, each editor's focus report and its letting go (title, markdown editor
  with Write/Preview and the ⌘↵ keyup, reply, draft), `PrPanel`'s hold and the surface's filter, `prdEscapeLive`
  evaluated (the draft open or not, a reply before and after its thread's serial moves on).

Checks: see the PR ("Checks").

## Real-input batch steps

Run on the branch's bundle (lane `usage-and-pr-pages`, its T3 home has the pages lane's transcripts and the
acme-coder-1 price), app launched normally and active, window 1280×840:

1. PG-3: Usage › Cost › 30 days. Move the pointer onto the (i) after "API estimate" and hold it still: within about
   0.3 s "API estimate excludes 10.0% unpriced records." shows above it. Move the pointer straight down onto the Codex
   row (not up into the popup): it closes. Move back onto the (i), then up into the popup: it stays. Click the (i): it
   stays while the pointer leaves; click on the chart: it closes. Click the (i) again, press Escape: it closes and the
   (i) has the focus ring.
2. PG-4: Usage with the window at 1280 wide. Rest the pointer on Cost, Tokens, Limits, Past 24h, 7 days, 30 days and 90
   days, about 1.5 s each: the macOS tooltip reads "Cost (C)", "Tokens (T)", "Limits (L)", "Past 24h (⇧⌘1)",
   "7 days (⇧⌘2)", "30 days (⇧⌘3)", "90 days (⇧⌘4)". Narrow the window below 1280, open the metric and period selects
   and rest on each option: the same titles.

## Progress

2026-10-10: built PG-2 to PG-7. Shot the reference on this lane (Tokens Past 24h, the Day table, the model dialog, the
unpriced hover, the titles, the environment menu, Filters › Author with "zz", and the Escape sequence). Drove the base
build and then this branch with the same steps; live drive 1 showed the author search field's text at the top of a
28pt field (fixed: the field is one line tall, centred by its row), and the one retry drive checked it, ↓ and the
Escape sequence again. `EXACT2-GAPS.md`: X72 (local draft, not filed) and the section "Usage and Pull Requests pages".

2026-10-10, review round 1 (independent review of PR #375, five should-fix rows): (1) the pull request editors were
`aria-modal` while open, which silenced every other app shortcut and kept an Escape from outside them on the page: they
now hold Escape only while their field has the focus, reported to `PrPanel`, and are not modal; the popovers stay modal
for want of a popover event (X66, #319), and the markdown editor's Cancel and Save report no focus (X54, #283), both
declared with their issue numbers. (2) Back went straight to the thread: it now goes back through the pages
(`pageTrail`), for the Usage page too. (3) Filters opened by ↓ or ↑ kept the last author search: it is cleared on every
opening. (4) Where the Author submenu puts the focus was checked on the reference: a key gives it to Anyone (Base UI's
first row), a press to the search; the clone did not match on the keyboard path (it focused the search), now does, and
the drive checked both. (5) The PG-4 note claimed `tree --ax` shows the title; the drive's `tree` props do, and the note
says so. Reference re-shot over CDP; the base and this branch driven once each with `target/usage-and-pr-pages/drive2.sh`.
Merged `origin/feat(example)/t3-code` three times (`ba0af7967`, `2dc9b0043`, `256c189d8`; conflicts only on the
T3Window call in `app.contract` and the PagesCover call in `app-window.contract`, both sides kept). The live drive ran
on `e74a541ad`; the final head adds only the `256c189d8` merge (#371, Markdown links and the Files preview).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | evidence-base `950e8e2e5` | Complete (a first run stopped at a second toast's dismiss button, which only the update toast's stack had; update checks off, then complete) | [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |
| After drive (the live drive) | this branch `a529aa727` (bundle) | Complete; every row as above but the search field's text sat at the top of its field | the links above |
| Retry drive (the one retry) | this branch `08679e5e8` (bundle) | Complete: the field's text centred, focus in the field, ↓ to Anyone, Escape twice | [pg6 triple](https://raw.githubusercontent.com/ccheever/exact2/8c7bb707f4beebe45e60d415240916999d526e48/usage-and-pr-pages/pg6-author-search.png), [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |
| Review round 1, before | evidence-base `950e8e2e5` | Complete on the second run (the first stopped at `tap open-pull-requests`: the base stays on Pull Requests after Escape, where the sidebar shows Back; the base side skips that tap) | [round 1 text](https://raw.githubusercontent.com/ccheever/exact2/41bf67b8e8366a86d9036b1e3ebbe87f32460dda/usage-and-pr-pages/text-r1-before-after.txt) |
| Review round 1, after (the live drive) | this branch `e74a541ad` (bundle) | Complete: titles in the props, Escape to Usage then the thread, → to Anyone, a press to the search, the search empty after a ↓ reopen | [pg7 round 1](https://raw.githubusercontent.com/ccheever/exact2/85f181925cc50180d448f287a057d372c6129c40/usage-and-pr-pages/r1-pg7-escape-history.png), [pg6 round 1](https://raw.githubusercontent.com/ccheever/exact2/10247bcaf459a53a83bc3a45a0a7604ddfcce574/usage-and-pr-pages/r1-pg6-author-focus.png), [text](https://raw.githubusercontent.com/ccheever/exact2/41bf67b8e8366a86d9036b1e3ebbe87f32460dda/usage-and-pr-pages/text-r1-before-after.txt) |

## Next action

The coordinator reviews the draft PR [#375](https://github.com/ccheever/exact2/pull/375), runs real-input steps 1 and 2 in the next batch, and merges it. Seen on both
builds, not this task's finding: the Usage page draws the Codex dot and chart series in their dark-scheme colour
(#f5f5f5) in a light window in agent mode (PG-1's area, [app-color-scheme](closed/20261009-app-color-scheme.md)).
Also seen on the reference in review round 1, not this task's findings and not fixed: Escape in a Filters submenu
closes only the submenu and gives the focus back to its row (the clone's closes the whole Filters menu, every
submenu, since fix-keyboard-focus); the reference draws no check on the Author submenu's Anyone while no author is
chosen (the clone ticks it). `pages-usage*.contract` and `pages-prs.contract` are shared with other tasks; the second
to merge keeps both sides.

## Delivery

Merged on 2026-10-10 as `7c3708cf7` (#375, squash) after an independent review and its repair round. Rows that need real input are in `examples/t3-code/STATUS.md` "Next real-input batch".
