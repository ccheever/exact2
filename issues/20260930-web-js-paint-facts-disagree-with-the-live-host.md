# The JS target decides isolation ahead of time and disagrees with the live web host in three places

**Status:** Open
**Systems:** web JS target, web host
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 D2, LLP 1071, llp/reviews/code-2026-09-30-taffy-css.{astra,grok}.md (astra 5, grok 4), host/web-js/src/style.rs, host/web/src/layers.rs, host/web-js/flow.js

The JS target's `isolation: isolate` is decided at build from the template (`host/web-js/src/style.rs`, `layers::isolates`) and the live host decides per commit (`Host::relayer`). Three places where the two can differ, from the review:

1. **A bound `position`.** The template says "may be positioned" (`Dynamic.paint.positioned = true` for any dynamic position binding), so the box is never isolated; the live host reads the current value, so with `position=(flag ? "relative" : "static")` and `flag` false it isolates the box when it follows something positioned. Chrome then paints the earlier absolute box over it on the JS page and under it on the live page.
2. **A repeated row.** `isolates` isolates a row's *first* copy too when it or a descendant is layered (`layers.rs:137`, `d.repeated && layered(...)`); the live host isolates a sibling only once an earlier sibling is layered. Layout agrees; paint does not.
3. **`flow.js`'s containing rule** has no root exception: `contains = position !== 'static'`, while `kernel/src/flow.rs` admits an exclusion whose parent is the root. A `wrap-flow: both` box that is a direct child of a static root is flowed by the kernel and refused as `context` by the JS mirror. (The web host gives the root element `position: relative` through `host_css`, so on a served page the root *is* positioned; check whether the JS target's built CSS carries that declaration for a root that authored no position — grok says it does not, spark's does because its root clips.)

Fix by emitting the same facts from both targets: a conditional isolation rule for a bound position (or refuse binding `position` where it changes the paint order), the same first-copy rule, and a root exception in `flow.js`. Add a conformance step that compares the two targets' `isolation` and the root's `position` per node.
