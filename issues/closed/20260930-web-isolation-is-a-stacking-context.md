# The web's `isolation: isolate` traps a descendant's z-index and blending where `position: relative` did not

**Status:** Closed
**Resolution:** Documented existing sibling-scoped paint groups in LLP 1001 and verified nested z-index plus supported SVG blend backdrops on web and macOS with the shared parity fixture.
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

## Verification, 2026-09-30

LLP 1001 §5 already orders z-index among siblings; its tree-order promotion is a group. It now states the descendant and backdrop consequences explicitly. `host/web/parity.mjs --paint` compiles one fixture and samples the same pixels on web and macOS: the nested z-index case is blue, the SVG multiply case is black over an internal blue SVG rectangle and red over an external blue box. The ticket's literal box blend case is refused by Contract under LLP 1055.000 stage 10c; no unsupported box blending policy was added.

The fixture ran through the JS web target and the integrated macOS app. Web samples were (0,0,255), (0,0,0), (255,0,0); macOS samples were (0,0,245), (0,0,34), (234,51,35). The harness uses the declared macOS display-colour-space tolerance, with bands that still distinguish all three possible outcomes. Screenshots and the fixture's boxes were inspected; no runtime change was necessary.
