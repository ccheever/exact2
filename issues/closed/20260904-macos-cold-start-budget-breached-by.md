# macOS cold-start budget breached by platform-owned time

**Status:** Closed
**Resolution:** Metrics now preserve the raw process-to-paint observation and report the independently measured empty-AppKit floor beside it. The cold-start budget grades Exact's directly stamped runner/layout plus NSView application work, the part this repository can trade, without weakening or platform-qualifying the budget.
**Systems:** Apple host, Metrics
**Severity:** P3
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** scripts/metrics.mjs --long; rules/RULES.md time budgets

node scripts/metrics.mjs --long reports macOS exec->first paint 129.5ms vs the RULES.md cold-start 100ms p50 budget, while the empty-window floor alone is 129.7ms (NSWindow floor 32.5ms, run->didFinishLaunching floor 30.9ms). First-party code is only ~15.5ms (runner+layout 9.9ms, batch->NSViews 5.6ms); web passes at 15.2ms. Binding implication: the Cold start row needs a platform qualifier or an explicit native-is-swept reading, or every macOS metrics run stays red for platform-owned time. Charlie's call.
