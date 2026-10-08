**Three findings; the first blocks landing.**

1. **Blocker — Zero-floor handling omits other nonnegative properties.** [kernel/src/style.rs:497](/private/tmp/bsky4-rv/wt-clamp/kernel/src/style.rs:497)  
   `width="min(-8px, -2px)"` folds to `Points(-8)`. Both static web paths serialize `width:-8px`, which CSS discards, producing `auto` instead of zero. A JS binding retains the math function and computes zero. Likewise, `padding-top="min(-4px, env(safe-area-inset-top))"` reaches Apple/Linux layout as negative padding while browsers clamp it. [CSS’s range rules](https://www.w3.org/TR/css-values-4/#calc-range) require clamping math results for these properties too.  
   **Fix:** Apply property-specific range handling to all admitted nonnegative dimensions, preserving negative margins/insets. Add native and browser tests for folded negative widths and environment-dependent negative padding; the conformance fixture currently exercises positive values.

2. **Should-fix — Unitless zero is incorrectly accepted as a length inside math functions.** [kernel/src/style/compare.rs:536](/private/tmp/bsky4-rv/wt-clamp/kernel/src/style/compare.rs:536)  
   `width=(flag ? "max(0, 10px)" : "20px")` passes compilation. Native and wasm normalize the first branch to `10px`; the JS target writes it verbatim and the browser rejects it. Inside math functions, zero remains a number and cannot be combined with a length. [CSS Values §10.9](https://www.w3.org/TR/css-values-4/#calc-type-checking) explicitly disallows this conversion.  
   **Fix:** Require `0px` inside length comparisons. Update the diagnostic, LLP/guide wording, and [the test currently asserting acceptance](/private/tmp/bsky4-rv/wt-clamp/kernel/tests/it/compare.rs:178). Add a dynamic-binding conformance case.

3. **Should-fix — The wire decoder enforces the total term limit after constructing the entire tree.** [kernel/src/style/compare.rs:208](/private/tmp/bsky4-rv/wt-clamp/kernel/src/style/compare.rs:208)  
   The recursive check limits each argument count, while the overall 64-term check runs afterward. A kind-24 tree containing 64 `min` children, each with 64 point terms, constructs 4,161 nodes before rejection. More nesting permits much larger allocations despite the advertised bound.  
   **Fix:** Pass a shared remaining-term budget through decoding and reject before constructing excess nodes. Test early refusal or consumed bytes; merely asserting an eventual error would also pass today.

I reviewed `65ee5d1a6` first. Its specific radius fixes preserve negative margins/insets, and the boundary check excludes `minmax(` and ASCII identifier prefixes. The radius floor agrees with CSS: bare negative radii are rejected, whereas negative math results are clamped.

Twelve focused tests passed using existing binaries from the matching branch, including the round-3 cases. A fresh Cargo run stopped on unavailable offline dependencies; no live browser or Apple-host verification was performed. The documented SVG/Apple canvas resolution gaps remain. No files were changed.

**Landing is blocked by finding 1.**