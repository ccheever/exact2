# Review round 2, LLP 1121 stage 9 H: an empty symbol: name is no symbol, Astra (gpt-6-astra, max)

Reviewed: 963e889b00.

No **BLOCKING** or **SHOULD-FIX** findings. Round-1 fixes check out; I found no remaining behavioral mismatch between the two spellings.

- [grouped.rs:213](/tmp/rv-rl-empty2/kernel/src/grouped.rs:213) preserves `Some("")`, producing the same standard cell. The [regression test:1152](/tmp/rv-rl-empty2/contract/cli/tests/it/vocab.rs:1152) checks both parity and the expected cell shape.
- [snapshot.rs:299](/tmp/rv-rl-empty2/contract/cli/src/snapshot.rs:299) admits both as local; tests exercise both.
- Lowering accepts the empty name without skipping dead-arm checks. Runner resolution, measurement, rendering, and agent reasons agree. Terminal behavior remains shared and unchanged.
- The repro test meaningfully boots and asserts exactly one image, `symbol:close`; prerender and Linux tests cover empty paths and raster cleanup.

**NIT — coverage:** [document.rs:39](/tmp/rv-rl-empty2/host/web/tests/it/document.rs:39) tests prerendering, but the added tests don’t directly protect host log silence or agent `reason: "empty"`. Existing host tests could cover both spellings after replacing a visible symbol.

I ran in-memory checks against both web implementations: identical empty masks, no logs, and `reason: "empty"` passed across those transitions; an unknown-role control still logged. Rust/Swift suites weren’t rerun because this read-only checkout has no build artifacts. No files changed.

After round 2: the coverage NIT (no host test asserts the silent log or `reason: "empty"`) is noted, not taken: no existing host test asserts that log for any source, so it would need new browser scaffolding.
