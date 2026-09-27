# New non-CSS rows and host carve-outs from PR #47 are declared only in their own RFCs, not in LLP 1001

**Status:** Open
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
