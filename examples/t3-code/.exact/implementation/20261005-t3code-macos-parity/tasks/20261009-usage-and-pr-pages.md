---
name: 20261009-usage-and-pr-pages
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
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

## Next action

Prepare a branch from `feat(example)/t3-code` after app-color-scheme merges. Build and unit-test. Then do one batched live
drive at the end for every row's before/after pair. Close every row in this PR, or record the blocker of a row that
cannot pass.
