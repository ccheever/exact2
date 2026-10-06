**The JS target lacks `at()`** — `match at(xs, i)` compiles but the web build fails
with `No matching export in rt.js for import "x_at"` (found writing
`apps/shared-elements`, 2026-10-04; it uses `first(filter(…))` instead).

*Filed under “Next, in order (2026-08-29)”.*
