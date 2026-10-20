# Review round 3, LLP 1121 stage 12 B1 (package components' params), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — [contract/cli/src/sources.rs:631](/tmp/rv-s12b1-r3-astra/contract/cli/src/sources.rs:631):** The dependency scan still folds only Contract params. A newly legal package param such as `glyph: icon = icon(svg "pin.svg")` makes compilation read `assets/pin.svg`, but omits it from `source_graph` and therefore incremental-build dependencies. Include package `design_tokens` in this scan, including when the app has no design file. Test existing and initially missing SVGs.

Rounds 1 and 2’s reported fixes look correct by inspection. Changed sources meet the 1499-line cap; whitespace checks pass. Cargo tests were blocked by sandbox denial of `.cargo-build-lock`; release-byte parity remains unverified.
