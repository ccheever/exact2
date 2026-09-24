# line-clamp overrides display and overflow on the web

**Status:** Closed
**Resolution:** Legacy clamping is skipped for flex/grid/hidden display or scrolling overflow; it no longer replaces authored layout, visibility or scrolling. CSS regression covers each case and clearing the clamp; ordinary block text keeps legacy clamping. Limitation documented in LLP 1007.
**Systems:** Web host, Kernel style
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1007

`css_text` walks style rows in bit order. `display` is bit 31 and `overflow-x` / `overflow-y` are 33 and 34. `line_clamp` is bit 76, and when it is greater than zero it appends `display:-webkit-box` and the `overflow:hidden` shorthand (`host/web/src/css.rs`). In one `style` attribute the later declaration wins, so the browser drops the kernel's display and overflow.

A `column` with `line-clamp` greater than 0, or a scroller with `line-clamp`, becomes a `-webkit-box` that does not scroll. Native keeps the kernel box. `-webkit-line-clamp` needs that display on old engines, but it is not the box the kernel laid out.

Emit the clamp without replacing an authored `display` and `overflow`, or refuse the combination. Done when a flex column with `line-clamp` stays `display:flex` on the web and still clamps, and a scroll container with `line-clamp` still scrolls.
