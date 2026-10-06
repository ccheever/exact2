**A row of many inline runs is the longest fill unit** (2026-09-19): 7 ms at the
99th percentile against 2.6 at the median, about 0.5 ms in the runner and kernel
and 0.35 ms in the presenter per created view, and every inline run is a
`NodeView` that is never mounted.

*Filed under “Next, in order (2026-08-29)”.*
