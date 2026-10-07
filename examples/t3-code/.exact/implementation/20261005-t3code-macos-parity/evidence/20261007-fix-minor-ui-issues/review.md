# Independent review, 2026-10-07

Reviewer: a separate agent given the diff (base `0a7ca50ad`), the ticket, the reference sources
(T3 Code `1e2ecbd975`) and the before/after images; not told an expected verdict. It read the code
and did not run anything.

Verdict: no blocking findings; every change matches the reference behaviour it cites
(title menu rules, Diff header/body/surface gating, MenuPopup `side="bottom"`, Contract syntax).

| # | Finding (non-blocking) | Resolution |
| --- | --- | --- |
| 1 | A failed source-control discovery was asked again on every palette read | Fixed: after a failure the next ask waits for `client.revision` to move (a time backoff is refused by the bake: no `Date.now()` in data sources); `palette.test.ts` covers both reads |
| 2 | Composer menu heights are estimates (a wrapped note or description runs past the estimate); a stale "side top" comment | Comment fixed; the estimate stays (follow-up below) |
| 3 | Settings traits: booleans (`fastMode`) not shown, label joins only the effort, bolt not gated to Codex, a tier alone drew a bolt | Bolt now Codex-only and only beside an effort label; booleans and the joined label are a follow-up |
| 4 | With a mixed selection across environments, a pick drops the saved options | Follow-up |
| 5 | Settings menu rows 32pt against the reference's 28 | Fixed (28pt, 4pt padding) |
| 6 | Diff: a stale not-git error kept a later repository blank; leftover files kept the header controls | Fixed: the stale error no longer blocks the empty state; nothing is listed outside git |
| 7 | Compaction row used a different glyph from Minimize2; the live row keeps the 14pt shimmer row | Glyph fixed (`minimize`); the live size is a follow-up |
| 8 | The settled badge's recede ignores `:focus-within` | Follow-up (the row's age text has the same rule today) |

Runner: attempt 1 passed; attempt 2 failed (the bake refused `Date.now()` in `palette-add.ts`);
attempt 3 passed with `source_unchanged: true` (`attempt3-report.json`).
