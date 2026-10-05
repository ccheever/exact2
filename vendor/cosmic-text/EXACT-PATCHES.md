# cosmic-text 0.19.0 — six local patches

Complete crates.io archive, including upstream MIT/Apache licenses and
.cargo_vcs_info.json. No source downloader, feature change or dependency upgrade.

- Upstream: https://github.com/pop-os/cosmic-text
- Exact release archive: https://static.crates.io/crates/cosmic-text/cosmic-text-0.19.0.crate
- Archive SHA256: be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73
- Upstream VCS revision: c24886c2471e5606587c46090cd25dbbf209186b
- Implemented: 2026-09-17, Leibniz, authorized giant-Markdown campaign.

The font-admission delta is three lines in `src/font/mod.rs`:
`Font::new` returns `None` if parsed `metrics.units_per_em == 0`.
Cosmic normalizes shaped glyph positions by that value. Some bitmap-only faces
have `bhed` but no `head`; the pinned skrifa parser reports zero units and cosmic
previously admitted them, producing infinite glyph advances. Failed admission
uses existing fallback/cache behavior. There is no name blacklist, metric clamp,
font replacement, or change to valid-font glyph/raster behavior.

Regression tests live in host/linux/src/text/transfer_tests/font_admission.rs:
missing/zero units, fallback continuation, ordinary finite-offer code, intrinsic
widths, stock/treatment finite-font glyph/RGBA comparison, and worker parity.
System fallback selection depends on installed fonts; the damaged-font admission
fixtures are portable and derived from the existing bundled DejaVu fixture.

The span-storage delta removes one line in `src/shape.rs`: `ShapeLine::build`
no longer reserves one `ShapeSpan` per UTF-8 byte before finding actual bidi-level
runs. Existing pushes grow the vector with actual span count. No prescan, shrink,
shaping change or new cache policy is introduced. Fresh single-run paragraphs
avoid byte-proportional spare header capacity; reused ShapeLine and FontSystem
scratch still retain their high-water allocations. Many alternating bidi runs
can incur additional vector growth; this is not a universal latency improvement.

Implemented 2026-09-17 by Carson in the authorized giant-Markdown campaign.
Regressions in host/linux/src/text/span_capacity_tests.rs cover public capacity,
ordinary/empty/multibyte/mixed-RTL semantics and warm reuse. Optional immutable
pre-change numeric/layout/pixel captures compare exact before/after output without
making external evidence a dependency of the portable tests. Existing host text
tests cover adopted-A ownership, sharing, glyph batches, pixels and font admission.
This change does not establish an address-space/RSS bound or 4 MiB reflow fit.

Root `[patch.crates-io]` selects this copy while host dependency versions stay
unchanged. Remove the patch when a pinned upstream release supplies equivalent
admission and span-storage behavior and these regressions pass. This is not general
support for bitmap-only font metrics. All other archive files are byte-for-byte upstream.

The base-direction delta (LLP 1053, 2026-09-25, Claude Opus 5.5) adds
`ShapeLine::new_with_base` and `ShapeLine::build_with_base` in `src/shape.rs`,
which pass a given paragraph level to `unicode_bidi::BidiInfo::new` instead of
`None`; `new` and `build` call them with `None`, so their behavior is unchanged.
CSS `direction` sets a paragraph's base direction, while the Unicode
first-strong heuristic is `unicode-bidi: plaintext`. With the level given,
`abc אבג` in an `rtl` paragraph orders its Latin word at the right. The host
passes `Some(true)` for every line of an `rtl` paragraph and `None` otherwise
(`host/linux/src/text/shaping.rs`; LLP 1001 §1 declares the `ltr` case); the
regression is in `host/linux/src/text/sharing_tests.rs`.

The book-typography deltas (the reader diary, 2026-10-04, Claude Opus 5.5) are
in `src/shape.rs`. A soft hyphen (U+00AD) carries the face's own `-` glyph and
advance (`ShapeGlyph::hyphen`, shaped with the same face, or its charmap's in
the basic path); a visual line that ends at one, not the paragraph's last,
shows that glyph with that advance and counts it in its width for alignment
and justification, and a word ending at one fits only with its hyphen
(`ShapeWord::hyphen_width`), as in Chrome. CSS `text-indent` is
`ShapeLine::layout_to_buffer_indented`: the first visual line starts that far
from its start edge and the indent is part of its width;
`layout_to_buffer` calls it with zero, so its behavior is unchanged. The host
passes the indent for a paragraph's first line (`host/linux/src/text/
shaping.rs`); the regressions are in `host/linux/src/text/css_tests.rs`.

The weight-axis delta (2026-10-04, Claude Opus 5.5, Android cold start) is in
`src/font/system.rs`. Font matching asks, for every face of another weight,
whether its `wght` axis covers the requested weight; it opened and mapped the
face's file each time to read that (about 300 files on a phone, per attributes
matched: most of a first layout). `FontSystem` now keeps each face's range once
read (`face_weight_axis`, public), and `set_weight_axes` takes ranges known
ahead: the host's font-directory cache (`host/linux/src/text/font_cache.rs`)
stores each face's range beside it, so a cached launch opens no file to match.
The comparison is unchanged (`min <= weight <= max`); no match result changes.
