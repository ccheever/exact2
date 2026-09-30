# The web's `isolation: isolate` traps a descendant's z-index and blending where `position: relative` did not

**Status:** Open
**Systems:** web host, web JS target
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 D2, LLP 1001 §5, llp/reviews/code-2026-09-30-taffy-css.{astra,grok}.md (astra 8, grok 3), host/web/src/layers.rs

Since d8a4a0ebf the web hosts make a static box that follows something positioned in tree order `isolation: isolate`, where they used to make it `position: relative` (LLP 1074 D2). Both paint the box itself in tree order with the positioned boxes (CSS 2 Appendix E step 8), which the `elementFromPoint` probe confirmed. They differ for the box's *descendants*: `isolation: isolate` is a stacking context and `position: relative; z-index: auto` is not.

Two cases the reviewers gave, both CSS-derived, neither run:
- Sibling A `position: absolute; z-index: 1`; sibling B static, isolated only because it follows A; inside B an absolute box with `z-index: 2`. Before, the inner box took part in the parent's stacking context and painted above A. Now it is trapped in B's context and A paints above the whole group.
- A positioned blue box, then a transparent static wrapper overlapping it holding a red child with `mix-blend-mode: multiply`. Before, the child blended with the blue backdrop; now the isolated wrapper excludes that backdrop.

Native hosts scope `z-index` to siblings and paint in tree order, so an isolated wrapper is arguably *closer* to what native does than the old `relative` was. That is a design call, not an accident, and it is not written down. Decide which is wanted, then either isolate only where the box's own paint must form a group, or declare the stacking-context semantics in LLP 1001 §5, and either way add a fixture with a nested `z-index` and one with a blend mode, compared on the web and on macOS.
