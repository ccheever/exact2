# On the web, a tinted raster's mask also masks the image's own background, border and shadow

**Status:** Partly fixed: retained and explained the declared box-paint deviation; Chrome reproduced both fixture mismatches and tests protect bare untinted images in live batches and documents. Remaining: a tinted raster's own background, border and shadow are still masked on the web.
**Systems:** Web host (`host/web/src/element.rs` `host_css`, `host/web/src/document.rs`, `host/web/glue.js`)
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1011 §3, LLP 1001 (declared deviation), `issues/20260927-undeclared-deviations.md`

`tint-color` on a raster is `background-color: var(--exact-tint)` masked by `mask-image: url(<src>)` on the `<img>` itself (`host/web/src/element.rs:61`). A mask covers the whole element, so a tinted image's background, border and shadow are masked to the picture. Apple and Linux tint the picture alone.

**Rejected fix (not merged, `fix/review-deviations` 00f1888d):** it made *every* web image a `span` wrapping an `<img>`, and re-created the replaced element's intrinsic sizing and flex/grid stretch in JavaScript (`refreshImages`, `--exact-image-width`). That makes the web's image no longer a real replaced element, which is the "kernel disagrees with a bare element" bug class, for a P3. The branch's document tests also regressed.

**Do instead:** keep a bare `<img>` for every untinted image. Only when `tint-color` is set on a raster that has a box paint (background, border or shadow), paint the tint on a child layer (or declare this case as it is now). Held by `scripts/fixtures/tint.web.png` / `tint.web-dark.png`, which already show the intended pixels.

**Disposition (Astra, 2026-09-27):** take the explicitly allowed declared-deviation
fallback. LLP 1001 §1 already declares this case; LLP 1011 §3 now explains why
it stays. A loaded replaced `<img>` has no generated `::before`/`::after`
boxes ([CSS Pseudo-Elements §4.1](https://drafts.csswg.org/css-pseudo-4/#generated-content)).
A mask on the image affects the whole element. A wrapper can isolate paint,
but becomes the outer sizing and flex/grid item; the hidden image plus a
percentage-sized layer does not by itself preserve replaced-element auto-axis
sizing. No clean CSS-only general solution was found in this review. There is
no renderer behavior change, host wrapper or new JavaScript layout. Authors
can put box paint on an explicit surrounding `view` with the tinted image
inside it. Removing the deviation still needs an implementation that preserves
intrinsic sizing, object-fit, live tint changes and document/live parity; no
new ruling from Charlie is required for this fallback.

**Reproduction:** compiled `scripts/fixtures/tint.contract`, rendered it through
`caltrain-render --viewport 390x460`, and loaded that document and the real PNG
in headless Chrome at 1×. In both light and dark, pixel (16,16) was opaque white
instead of the fixtures' green `(0,170,0)` border, and (20,20) was opaque white
instead of cream `(255,238,221)` padding. A fixed, magenta `img::after` probe
also painted nothing. The two reference PNGs are unchanged targets; this
review does **not** claim image-paint parity with them.

**Regression coverage:** `host/web/tests/it/tint.rs` now checks that untinted
images with background, border/padding or shadow remain bare images in both
the live batch and document, including natural size, one auto axis, fixed
dimensions, source and alt text. This guards against the rejected wrapper
approach; it is not a test claiming that the remaining paint defect is fixed.

**Verification:** tint tests: `2 passed; 0 failed`; document tests:
`12 passed; 0 failed`. Full `exact-web` lib/bin/integration suite:
`196 passed; 0 failed; 5 ignored`; web-host Clippy with warnings denied passed.
Required root checks all passed:

- `cargo build --all-targets --keep-going`: finished dev profile, 19.05s.
- `cargo test --lib --bins --tests --no-fail-fast`: 71 suites,
  `1750 passed; 0 failed; 8 ignored` (sum of Cargo's result lines).
- `cargo clippy --all-targets --keep-going -- -D warnings`: finished dev profile,
  17.21s; `cargo fmt --all -- --check`: exit 0.
- `git add -A && bun scripts/caps.mjs`: `All budgets within cap. 6 categories inspected.`
- `bun scripts/boot.mjs`: `modules reachable before first pixel: 2`;
  `Allowed import paths only.`
