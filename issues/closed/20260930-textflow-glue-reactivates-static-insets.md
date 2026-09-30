# The web's text-flow renderer makes a flowed static paragraph `relative`, which activates its insets

**Status:** Closed
**Resolution:** Flow fragments use the existing layout containment without changing static paragraphs to relative; 30 text-flow tests pass, including real Chrome point and percentage inset and fragment-position checks.
**Systems:** web host (textflow-glue.js), web JS target
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1043.000, LLP 1074 T1, llp/reviews/code-2026-09-30-taffy-css.astra.md (finding 12), host/web/textflow-glue.js:519, kernel/src/flow.rs `in_flow`

`kernel/src/flow.rs` now admits a static paragraph to auto-height flow whatever its insets, because a static box's insets do nothing. `host/web/textflow-glue.js:519` sets `position: relative` on a flowed paragraph whose computed position is `static`, to contain its line fragments, and does not neutralise the paragraph's authored insets. A static paragraph with `top: 50%` is therefore laid out at y = 0 by the kernel and admitted, and moved down half the context's height by the browser once fragments are installed.

Fix: contain the fragments without changing the paragraph's position (a wrapper, or `contain: layout`, or an inner positioned element), or zero `top`/`left`/`right`/`bottom` on the paragraph while it is relative and restore them with the rest of the saved style. Add a case with a static paragraph carrying a point inset and one with a percentage inset to `host/web/textflow.test.mjs`.
