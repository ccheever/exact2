# Code review: a large table laid out again unchanged publishes nothing (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree.
- **Method:** a quick review and its confirmation; the author (Claude, fb1) is not a reviewer.
- **Transcription:** each final message, unedited.

## First review, at `79b4420247`

- **MAJOR — [kernel/src/layout/publication.rs:422](/Users/admin/x2scratch/fb1-rv-reds/kernel/src/layout/publication.rs:422):** The ID ranges overlap at 500 rows. `CreateView` silently reuses an existing same-type node, so rows, cells and inner boxes overwrite and reparent one another. For example, row 200 reuses the first cell; node 999 eventually becomes a `td`. The resulting fixture has 1,303 nodes instead of 2,504, with 200 empty rows. Both the regression test and measurement therefore exercise the wrong workload; the measurement moves a cell rather than the intended box inside one. Allocate disjoint IDs and assert the row/cell structure and flat-row registration.

- **MAJOR — [kernel/src/layout/publication.rs:486](/Users/admin/x2scratch/fb1-rv-reds/kernel/src/layout/publication.rs:486):** These calls do reach publication and reset its visit counter, but identical inputs hit Taffy’s layout cache before grid placement. Consequently, they do not exercise repeated `set_grid_origin` calls and cannot establish that patch 69 suppresses unchanged-origin reports. An unconditional report in that setter could escape this test. Add a case that re-executes grid placement with unchanged origins, then checks publication visits and receipts.

Static review only; tests were not run.

VERDICT: CHANGES

**Response:** both findings addressed in the landed commit: disjoint ids with node-count and flat-part assertions, and a restyle that places the grid again with no origin changed.

## Confirmation, at `efb7c971b6`

No BLOCKER, MAJOR, or MINOR findings.

- [publication.rs:467](/Users/admin/x2scratch/fb1-rv-reds/kernel/src/layout/publication.rs:467): The builder produces 2,504 distinct nodes and checks that the tbody and endpoint rows are flattened table parts.
- [publication.rs:501](/Users/admin/x2scratch/fb1-rv-reds/kernel/src/layout/publication.rs:501): Unchanged layouts reach publication, which resets the visit counter. The subsequent `left: auto` → `0px` restyle invalidates the grid while preserving geometry, exercising the unchanged-origin comparison. The visit cap and fresh-layout comparison support the claim.
- [publication.rs:525](/Users/admin/x2scratch/fb1-rv-reds/kernel/src/layout/publication.rs:525): Timing excludes fixture setup, uses increasing transaction epochs and changing offsets, and averages 200 iterations. It measures complete relayout and style-construction/apply/layout costs, rather than isolating publication.

Static review only; tests, timings, and the reported mutation experiment were not rerun.

VERDICT: APPROVE
