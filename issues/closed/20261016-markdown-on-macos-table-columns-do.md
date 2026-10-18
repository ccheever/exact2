# Markdown on macOS: table columns do not line up and the header row overflows its column

**Status:** Closed
**Resolution:** Per-row widths did not reproduce on 0d2e94e3b: macOS and the web, like Chrome, gave every row of the 11-column table equal columns (`flex: 1`, `min-width: 0`), and words spilled into the next cell; if per-row widths come back with that tree, the host or kernel is wrong. apps/markdown now draws a pipe table as one block and one CSS grid (`repeat(n, auto)`) inside a sideways scroller, as a GFM table on GitHub; apps/markdown/data/tests/table.rs holds it.
**Systems:** apps/markdown, kernel flex layout, host/apple macOS text measurement
**Author:** Charlie Cheever
**Date:** 2026-10-16

Reported by Charlie from a screenshot of the macOS Markdown app (`apps/markdown`) showing
`/Users/ccheever/projects/tui-comparison-20261010/REPORT.md`, whose tables are ordinary GFM pipe tables
(e.g. line 46: an 11-column header `| Framework | App | Typing ms | … | Observed limits |`, a `| --- |` row,
then rows such as `| ink | harness | 4.68 | 97.94 | 1,779,030.00 | … |`).

What the window shows:

- **Columns do not line up between rows.** Each body row places its cells at different x positions:
  the `bubbletea` rows start their second cell about 30 points further right than the `ink` rows, and
  numbers do not sit under their headers (`Typing ms` is over `harness`/`4.68`, `Find ms` is over a
  value that belongs to `Scroll ms`).
- **The header row is wider than the table.** Its last columns (`Sampled peak RAM MiB`, `Observed
  limits`) run past the end of the rows' bottom rules, outside the 740-point reading column.
- Cell widths look content-sized per row.

Why this looks like a layout bug rather than the app's design: `apps/markdown/app.contract` (the
`table-row` branch, around line 212) draws each table row as its own `row width="100%"`, with every
cell a `text flex=1 min-width=0 padding=8`. In CSS `flex: 1` is `1 1 0%` and `min-width: 0` lets a
cell shrink below its content, so every cell of an n-cell row gets an equal share of the same
width, and rows with the same number of cells line up whatever their text. The screenshot instead
shows content-based bases (as `flex-basis: auto`, or a `min-width: auto` floor from the text's
min-content width) and an overflowing header.

To check first: render the same document on the web target (`bun host/web/dev.mjs --app markdown`,
or the agent's web host) and compare `layout` of the header and two body rows with macOS
(`bun scripts/agent.mjs macos layout …`). If the web lines the columns up, the macOS host or the
kernel's measurement of a `text` flex item with `min-width: 0` disagrees with Chrome; reduce it to a
two-row fixture (`row` of `text flex=1 min-width=0` cells with long and short contents) and fix it
there. If the web shows the same, the app's table needs a column model (one grid, or one width per
column shared by all rows), which is an app change.
