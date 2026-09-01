# CSS `transition` parser rejects a leading easing (`ease 1s`)

**Status:** Closed
**Resolution:** Transition parsing now follows CSS shorthand ordering and canonicalizes easing and linear forms correctly.
**Systems:** Motion, Kernel
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1003, rules/RULES.md (the web is the standard)

CSS `transition` shorthand admits an omitted property (`all`) and any order of the other parts except duration-before-delay. `Transitions::parse` (`motion/src/parse.rs`) treats a leading time as `all` but a leading easing as `UnknownProperty`. `ease 1s` and `linear 200ms` are valid CSS and valid kernel rows if built structurally; the dynamic/text path — `StyleValueError::BadTransition` via this parser — refuses them. The parser's own test documents the duration-first form and never the easing-first form.

Fix: if `parts[0]` is a known easing/spring, default the property to `all` (same as a leading time). Pin `ease 1s` and `linear 200ms` next to the existing `"0.3s"` fixture.

The review found that this is broader than a leading-easing special case.
Chrome accepts `1s opacity`, but the parser treats `opacity` as an easing after
consuming the first time. Conversely, Chrome rejects `opacity 1s ease linear`,
while the parser silently overwrites the first easing and accepts the second.
Its `linear()` stop handling also assigns omitted positions by argument index
instead of distributing them between surrounding explicit stops, and ignores
a third field where CSS permits two positions for one stop.

Fix by parsing the transition shorthand as unordered components with duplicate
checks and CSS's duration-before-delay rule, then implement the CSS `linear()`
stop fix-up algorithm. Use browser `CSS.supports` and computed timing fixtures
for property/time order, duplicate easing, omitted stop positions, and
two-position stops.
