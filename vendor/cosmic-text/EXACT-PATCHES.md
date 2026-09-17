# cosmic-text 0.19.0 — one font-admission patch

Complete crates.io archive, including upstream MIT/Apache licenses and
.cargo_vcs_info.json. No source downloader, feature change or dependency upgrade.

- Upstream: https://github.com/pop-os/cosmic-text
- Exact release archive: https://static.crates.io/crates/cosmic-text/cosmic-text-0.19.0.crate
- Archive SHA256: be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73
- Upstream VCS revision: c24886c2471e5606587c46090cd25dbbf209186b
- Implemented: 2026-09-17, Leibniz, authorized giant-Markdown campaign.

The only upstream source delta is three lines in `src/font/mod.rs`:
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

Root `[patch.crates-io]` selects this copy while host dependency versions stay
unchanged. Remove the patch when a pinned upstream release supplies equivalent
admission protection and these regressions pass. This is not general support for
bitmap-only font metrics. All other archive files are byte-for-byte upstream.
