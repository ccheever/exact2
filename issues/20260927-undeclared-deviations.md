# New non-CSS rows and host carve-outs from PR #47 are declared only in their own RFCs, not in LLP 1001

**Status:** Fixed, except the web half of raster tint: LLP 1001 declares every deviation (with Charlie's 2026-09-27 rulings), and both Linux painters apply raster tint (pixel regressions and Chrome parity fixtures, worst mean difference 0.79/255). Astra's web fix made every image a `span` around an `<img>` with JavaScript re-creating replaced-element sizing; it was not merged, and the web tint moved to `issues/20260927-web-tint-masks-the-box.md`.
**Systems:** LLP 1001, LLP 1011, 1059, 1061, 1063, 1064, 1066; web (`host/web/src/element.rs`), Linux (`host/linux/src/paint.rs`), iOS (`SegmentsIOS.swift`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** CLAUDE.md (a deviation is declared in LLP 1001 with its reason)

CLAUDE.md requires each deviation from the web to be declared in `llp/1001-kernel-v1.spec.md`. PR #47's change to 1001 touches only `clip-path`, and 1001 still says raster tint is unsupported (`:108`). These are undeclared there:
- `press-scale`, `layout-transition` and `exit-animation` (schema bits 145–147) are not CSS.
- `box-shadow` is a subset: no spread, no `inset`, no list, and a missing colour is refused instead of meaning `currentcolor` (1064 D1).
- Gradient end colours are extended under the border instead of repeating the image (1066 D5).
- **Raster tint.** On the web it masks the whole `<img>`, including its background and border (`host/web/src/element.rs:61`). Linux ignores `tint-color` entirely (no handling under `host/linux/src`). A remote tinted image needs CORS or paints nothing (1011 §3).
- Native `text-transform` uses Unicode's root mapping, and native Markdown ignores it (1064 D5).
- Linux refuses `exit-animation` (1063 D8; `host/linux/src/presence.rs:34`).
- The projected iOS tab bar can grow outside its kernel box (`SegmentsIOS.swift:173-179`), which contradicts 1059's own "layout stays authored".

**Fix:** add one deviation block to 1001 that lists each item with its RFC and reason. Where the reason is only implementation convenience (Linux tint, web tint masking the box), fix the host instead.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max (design), Astra max (code and design). Verification: confirmed by reading.

## Resolution (Astra, 2026-09-27)

LLP 1001's raster-tint support statement and LLP 1011's implementation notes
now match the hosts. Web live creation, server documents and adoption share the
inner-image projection; background, padding, border and shadow stay on its box.
Linux passes optional tint through ordinary and retained paint, using source-in
in both tiny-skia and Vello without changing the decoded image. The painter test
module was moved to a sibling file to keep every source below 1,500 lines.

Reproduced first: Linux painted blue source pixels instead of red/translucent
tint in both painters; the web projection put `mask-image` on the image's box.
Both regressions now pass. Chrome tests exercise alpha, background/border,
light/dark, tint transition and clearing, all five fits, and natural/percentage/
min/max-width sizing in block, flex and grid. Nine saved Chrome cases pass on
both Linux backends in both schemes; worst mean difference 0.79/255, worst
share beyond the 48-channel band 0.74% (the existing limits are 2 and 2%).

Verification: root build, tests (1,714 passed, 8 ignored), clippy, fmt, caps and
boot passed. Web Rust tests: 195 passed, 5 ignored. Linux: 555 passed, 1 ignored;
the extracted projection tests also pass. The focused Chrome test passes.
Broader checks remain failing: `smoke.mjs web` reports Chrome keychain and
first-paint diagnostics; `document.test.mjs` reports canvas style differences,
two early-press replay timeouts, and missing native Hermes for Weatherlight.
These failures are recorded in QUEUE.md; no check was weakened.

For Charlie: rule on `press-scale`, `layout-transition`, `exit-animation`, the
`box-shadow` subset and the tab-bar projection/minimum height. Their semantics
are unchanged; documenting them here does not ratify them. CORS masks, native
gradient border extension, root case mapping/Markdown coverage and Linux exit
refusal remain the declared limitations listed in LLP 1001.
