Both findings are fixed. No additional blocker, should-fix, or nit found.

Reservations remain held until readers exit and release on completion, error, panic, or spawn failure. No deadlock or timeout/abort settlement regression found. The four-reader cap intentionally refuses further uploads while saturated, even if existing reads are healthy.

Ten in-memory JS probes passed with zero unhandled rejections. Native behavior was source-traced, not executed.

Nothing blocks.