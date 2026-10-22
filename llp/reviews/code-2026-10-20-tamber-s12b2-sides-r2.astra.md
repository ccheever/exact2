# Code review round 2: B2-levels per side (1d17d046d7), Astra

**RIGHT**

No BLOCKING, SHOULD-FIX, or NIT findings. Round 1 is fixed.

1. **Regression coverage added.** [contract/cli/tests/it/tamber_levels.rs:621](/tmp/rv-sides-astra/contract/cli/tests/it/tamber_levels.rs:621) covers a mixed-axis `when` on a params line, asserting both provenance and folded values. For `(mode=b, dark-style=dim)`, light reaches `app-token/ink` (`#111111ff`), while dark reaches `app-param/Map.pin` (`#000000ff`). With `black`, the normal branch supplies both sides of `ink`’s `light-dark()`. The package-token case also verifies that a two-axis normal branch outranks a dark-only branch.

2. **Documentation corrected.** [docs/tamber-protocol.md:570](/tmp/rv-sides-astra/docs/tamber-protocol.md:570), the walker comment, and [LLP 1121:5572](/tmp/rv-sides-astra/llp/1121-tamber.rfc.md:5572) now correctly describe light selecting the most-specific holding branch containing no under-dark axis, falling back to the owning value only when none holds. The 1.15 clause records the `{light, dark}` correction within the open window.

The implementation matches `select`/`under_value`: dark considers every holding branch; light excludes branches containing **any** under-dark condition. The side propagates through params, app tokens and package tokens within the existing eight-hop cap. References continue the walk; expressions, including `light-dark()`, retain their owning line. `levels.base` checks every tuple on both sides.

By inspection, the [counterexample at line 588](/tmp/rv-sides-astra/contract/cli/tests/it/tamber_levels.rs:588) fails under the old behavior: its pair arm stops at `Map.pin`, incorrectly reporting `app-param` for light instead of following `reading.chain` to `package/reading.pin`.

Read-only review; no builds or tests run.
