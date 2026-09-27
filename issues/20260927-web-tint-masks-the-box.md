# On the web, a tinted raster's mask also masks the image's own background, border and shadow

**Status:** Open
**Systems:** Web host (`host/web/src/element.rs` `host_css`, `host/web/src/document.rs`, `host/web/glue.js`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1011 §3, LLP 1001 (declared deviation), `issues/20260927-undeclared-deviations.md`

`tint-color` on a raster is `background-color: var(--exact-tint)` masked by `mask-image: url(<src>)` on the `<img>` itself (`host/web/src/element.rs:61`). A mask covers the whole element, so a tinted image's background, border and shadow are masked to the picture. Apple and Linux tint the picture alone.

**Rejected fix (not merged, `fix/review-deviations` 00f1888d):** it made *every* web image a `span` wrapping an `<img>`, and re-created the replaced element's intrinsic sizing and flex/grid stretch in JavaScript (`refreshImages`, `--exact-image-width`). That makes the web's image no longer a real replaced element, which is the "kernel disagrees with a bare element" bug class, for a P3. The branch's document tests also regressed.

**Do instead:** keep a bare `<img>` for every untinted image. Only when `tint-color` is set on a raster that has a box paint (background, border or shadow), paint the tint on a child layer (or declare this case as it is now). Held by `scripts/fixtures/tint.web.png` / `tint.web-dark.png`, which already show the intended pixels.
