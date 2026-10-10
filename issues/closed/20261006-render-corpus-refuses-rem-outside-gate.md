# Kernel-free render corpus is red on rem and em, and ungated

**Status:** Closed
**Resolution:** Fixed by 7784a7743: the corpus names relative-length plans as the documented kernel projection exception; the corpus regression passes.
**Systems:** host/render, host/web-js
**Severity:** P2
**Author:** Grok, for Charlie Cheever
**Date:** 2026-10-06
**Related:** QUEUE: rem/em the JS target still bakes; host/web-js/conformance/rem.contract

`host/render`'s `every_corpus_and_conformance_plan_is_the_same_without_a_kernel` failed on current main: 102 of 105 plans compared. The assertion allows a skip only when the kernel itself cannot render, or when a slot initializer reads a resource (`budget.contract`). `host/web-js/conformance/rem.contract` is a third skip: `not written without a kernel: a relative length (rem, em)`.

That plan is the #136 conformance case. The wasm page bakes the kernel's pixels; the JS target leaves `rem`/`em` in the stylesheet for the browser. The kernel-free renderer in `host/render` refuses them instead of writing either form, and the crate is not in the root gate (`exact-render` is not a default member), so the red test is invisible to the five checks.

The product gap for the JS target is already a QUEUE line (`rem`/`em` the JS target still bakes, or the browser resolves elsewhere). This issue is the red corpus assertion and the missing gate. Either teach the direct projection to emit the same CSS units the JS target does, and drop the skip, or name `rem.contract` as an allowed exception next to `budget.contract` until that QUEUE line lands. A silent red test in an ungated crate will drift.
