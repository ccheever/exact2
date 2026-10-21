# Review round 3, LLP 1121 stage 9 E (held rows name their tokens), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — [host/web/input-glue.js:407](/tmp/rv-s9e-r3/host/web/input-glue.js:407): A known role masks an unresolved token reference.**  
   After round 2, `token.names` includes role overrides, while `refs` counts only `--x-*` references. A held value combining `var(--exact-canvas-text)` and `var(--x-missing)` therefore has both counts equal to one and incorrectly omits `provenance: "incomplete"`. Reproduced using the real `modes.token` resolver. Resolve each token reference independently, flag any unresolved reference, and add a regression combining a known role with an unknown token.

The other round 1/2 fixes check out. Seven focused Bun tests, caps, and formatting passed; in-memory checks verified owner identity, instance paths, source freshness, authored versus direct D7 names, and token passthrough. Rust/native integration and T4 were not rerun under the read-only sandbox.
