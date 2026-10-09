---
name: 20261009-usage-and-pr-pages
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-usage-and-pr-pages
pr_url: null
verified_commit: null
---

# Usage and Pull Requests pages: rounding, hover popover, tooltips, the environment menu, author search and Escape

## Outcome

- Usage amounts round as the reference's currency format.
- The unpriced popover opens on hover.
- Toggles and period options carry their shortcut tooltips.
- The environment menu shows full names on an opaque background.
- Pull Requests › Filters › Author has its search field.
- Escape leaves the Pull Requests page.

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
  $0.82 again; `EXACT2-GAPS.md` X71, a local draft). `formatUsd` (`pages-usage.ts`) now rounds the shortest digits half
  away from zero itself and keeps ICU's sign for a negative or negative-zero amount. Every amount on the page goes
  through it (the rows, the Day table, the model dialog, Totals, the chart's ticks).
- **PG-3.** The (i) was a `popovertarget` button (click only), and its popup a dark bubble wrapped in a 12pt containing
  block. It is now a hover trigger on the Usage page's popover machinery, as a pooled segment's is (fix-hover-cards):
  `enterSeg("unpriced")` (Base UI's 300 ms open delay on the window's hover clock), `pin` on a press, light dismiss,
  and Escape closes it and focuses the (i). The page draws `UnpricedPopup` above the (i)'s frame in its scroll content
  (side top, centred, 4pt offset, the tooltipStyle popup: popover colour, 1pt border, 12/16 text, one line).
- **PG-4.** `usageKeys` gives each shortcut UsagePage's `shortcutTitle` (`Cost (C)`, `Past 24h (⇧⌘1)`; the effective
  binding first), and `Segment` and `UsageOption` carry it as `title` through `usageTitle` (the label alone when the
  command has no binding; Breakdown's toggles have none, as in the reference). Exact's `title` is the platform tooltip.
- **PG-5.** The menu's width came from measured texts, exactly as wide as its widest row, and the label's `line-clamp`
  cut "Daehyeon's MacBook Pro" at a word when the layout came out a fraction short. The measured width is now the
  menu's least width (`min-width`), and the row label keeps its width (`flex-grow=1 flex-shrink=0 white-space=nowrap`), so
  the menu grows to its rows. The glass is drawn opaque (`light-dark(#ffffff, #111111)`), as the Code tab's menus are:
  the macOS presenter draws no backdrop blur. Same for the Model prices dialog's environment menu, the narrow selects
  and the Pull Requests list's menus (`PrMenuBox`), which a toast read through in the audit's PG-6 shot.
- **PG-6.** `PrFiltersMenu` keeps an `authorQuery`, cleared as the Filters menu opens. `PrFilterSub` draws the
  reference's compact InputGroup ("Search authors", autofocused, focused by `open`/`subKey` as the submenu opens; its
  keys stop at the field except ↓ and Escape). `prVisibleAuthors` lists the chosen author first, then every other whose
  login or name holds the trimmed lower-cased search, at most 10, or "No authors found". `pages-prs.ts` now sends every
  author with a row in the state (it cut the list to 10 before the search could see the rest) and their names.
- **PG-7.** `PrList` has the Usage page's hidden Back (`aria-keyshortcuts="Escape"`): `blur()` then `back()` (`openPage("")`,
  as Usage's). Exact hears a shortcut button before any `key` handler, so the page's owners of Escape are `aria-modal`
  while they own it: the row checks popover, the review composer, the freshness popover, the reaction and people pickers,
  and an open pull request editor (title, description or remark, reply, code comment draft). The Usage page's narrow
  selects are modal too. Declared in `EXACT2-GAPS.md` ("Usage and Pull Requests pages").
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
| PG-4 | Pass for the titles: the served keybindings give Cost (C), Tokens (T), Limits (L), Past 24h (⇧⌘1) … 90 days (⇧⌘4) (live state; before: none), the toggles and options carry them (`usage-pr-pages.test.ts`). The tooltip itself shows only under a real pointer in an active app: real-input step 2 | [pg4 text](https://raw.githubusercontent.com/ccheever/exact2/dd459d80885fe8a7b6309ddea7c55f0a7cb5f5d8/usage-and-pr-pages/pg4-toggle-tooltip.png), [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |
| PG-5 | Pass: "Daehyeon's MacBook Pro  Ready" in full, the menu as wide as the reference's, on an opaque popup (before: "Daehyeon's MacBook…", the headline through it) | [pg5 triple](https://raw.githubusercontent.com/ccheever/exact2/3960497b82649ccf4e4ba23d395da9e3ffde3bfb/usage-and-pr-pages/pg5-environment-menu.png) |
| PG-6 | Pass: the Author submenu starts with "Search authors", which has the focus (the live state's focus is the field's editor); typing "zz" reads "No authors found"; ↓ moves to Anyone; Escape closes the menus and the page stays (before: no field). Filtering with authors is unit-tested (the fixture has no pull requests on either side) | [pg6 triple](https://raw.githubusercontent.com/ccheever/exact2/8c7bb707f4beebe45e60d415240916999d526e48/usage-and-pr-pages/pg6-author-search.png), [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |
| PG-7 | Pass: thread → Pull Requests → Escape → the thread (before: Escape does nothing); with the Filters menu open, the first Escape closes it and the page stays, as the reference | [pg7 triple](https://raw.githubusercontent.com/ccheever/exact2/d782c8d5c44d8e35b0a0e1196078a47bd9e3f3d9/usage-and-pr-pages/pg7-escape-back.png), [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |

## Tests

- `pages-usage.test.ts`: PG-2's half-away-from-zero cases and an ICU oracle sweep (Bun's `Intl.NumberFormat`, as
  Chrome's) over 16,000 amounts, -0, 1e21, the largest double, NaN and the infinities; PG-4's titles from the default
  keybindings, a command bound twice titled by its effective binding, `shortcutGlyphs`.
- New `usage-pr-pages.test.ts` (reads the Contract sources): PG-3's trigger and popup wiring and placement; PG-4's titles
  on every toggle and option, none on Breakdown's; PG-5's `min-width`, the label that keeps its width, no translucent
  popups left; PG-6's `prVisibleAuthors` evaluated over a list (chosen first, login or name, trimmed, at most 10, none),
  the field's focus and keys, `presentList` sending all authors with names; PG-7's Back and the Escape owners'
  `aria-modal`.

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
Escape sequence again. `EXACT2-GAPS.md`: X71 (local draft, not filed) and the section "Usage and Pull Requests pages".

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Before drive | evidence-base `950e8e2e5` | Complete (a first run stopped at a second toast's dismiss button, which only the update toast's stack had; update checks off, then complete) | [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |
| After drive (the live drive) | this branch `a529aa727` (bundle) | Complete; every row as above but the search field's text sat at the top of its field | the links above |
| Retry drive (the one retry) | this branch `08679e5e8` (bundle) | Complete: the field's text centred, focus in the field, ↓ to Anyone, Escape twice | [pg6 triple](https://raw.githubusercontent.com/ccheever/exact2/8c7bb707f4beebe45e60d415240916999d526e48/usage-and-pr-pages/pg6-author-search.png), [text](https://raw.githubusercontent.com/ccheever/exact2/9ae47afb399b9560068888f460aa20fe18d54a58/usage-and-pr-pages/text-before-after.txt) |

## Next action

The coordinator reviews the draft PR, runs real-input steps 1 and 2 in the next batch, and merges it. Seen on both
builds, not this task's finding: the Usage page draws the Codex dot and chart series in their dark-scheme colour
(#f5f5f5) in a light window in agent mode (PG-1's area, [app-color-scheme](closed/20261009-app-color-scheme.md)).
`pages-usage*.contract` and `pages-prs.contract` are shared with other tasks; the second to merge keeps both sides.
