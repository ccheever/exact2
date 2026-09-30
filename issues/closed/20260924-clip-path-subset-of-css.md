# clip-path accepts only absolute M/L/Q/C/Z, so a circle is hand-approximated

**Status:** Closed
**Resolution:** Superseded by the full SVG path parser: the requested arc and relative-command examples normalize and round-trip; other basic shapes remain the explicit unsupported subset in LLP 1001.
**Systems:** kernel, contract compiler, Apple host, Linux host
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1035.005 D5 (app artwork stays clip-path); LLP 1035.004 D1 (symbol roles); kernel/src/clip.rs

Seth's Crew port (report of 2026-09-24, D11) drew two tab icons as `clip-path` paths and had to approximate circles with cubic Béziers. The symbol roles it lacked landed as rows (`repeat`, `activity`; LLP 1035.004 table). The narrower gap is CSS parity.

`ClipPath::parse` (`kernel/src/clip.rs`) accepts `none` or `path('…')` with explicit absolute `M L Q C Z` only. The Contract compiler refuses anything else on every surface (`contract/lower/src/values.rs`, `BadClipPath`: "expected none or path() with explicit absolute M/L/Q/C/Z commands …"), so the surfaces agree, but on a subset of CSS: no arcs (`A`/`a`), no relative commands, no `H`/`V`/`S`/`T`, no implicit repeated coordinates, and no basic shapes (`circle()`, `ellipse()`, `inset()`, `polygon()`). A browser accepts all of them, so an author working from web CSS or an SVG export hits a compile error, and the design's answer for app artwork (LLP 1035.005 D5: keep it `clip-path` until an SVG-path asset row is proposed) is weaker than it looks.

Fix in the kernel so hosts do not change: normalise in `ClipPath::parse` to the absolute `M L Q C Z` the batch already carries. Relative commands, `H`/`V`, `S`/`T` and repeated coordinates are bookkeeping; arcs convert to cubics by the SVG implementation notes' endpoint-to-centre parameterisation, at most 90° per segment. Basic shapes reference the box and need layout size: either resolve at layout (a new `RowValue` form) or admit only absolute lengths first and refuse percentages by name.

Done when `path('M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20Z')` and `path('m2 2h20v20h-20z')` compile and clip the same pixels on the web, macOS and iOS (a parity case beside the existing clip tests), and whatever is still refused names the command it refused.
