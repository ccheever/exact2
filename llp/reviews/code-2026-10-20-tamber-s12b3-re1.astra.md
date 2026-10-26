# Narrow re-check after round 3: LLP 1121 stage 12 B3 (dcea053f2b), Astra

**RIGHT.** No BLOCKING or NIT findings in this narrow re-check.

- [JS shadow reconstruction](/tmp/rv-s12b3-astra/host/web-js/src/base_scale.rs:586) uses `scale::is_length_item` for both authored lengths and canonical CSS slots. The predicate admits px, unitless `0`, bare bases and case-insensitive `calc/min/max/clamp`; other functions remain untouched. The added tests cover omitted box spread and text blur, comparing the result exactly with the kernel row’s CSS.
- [QUEUE](/tmp/rv-s12b3-astra/QUEUE.md:28) now matches resolve: residual entries are null in `lengths`, and `length_texts` supplies every length’s text when a residual exists.
- [Unkinded shadow detection](/tmp/rv-s12b3-astra/tamber/src/shadow.rs:156) recursively checks function arguments for dimensions or length names, including unset bases, tokens and dials. The numeric-only example reaches its earlier refusal; functions over bases still trigger “write its kind.”

No source-level regression found in `fold_lengths` or kernel `measure`: substitution, column selection, author overrides and dependency recording remain intact. App sources and schema/format declarations are unchanged from the stated comparison baseline.

Read-only source review; no builds or tests run. HEAD~1’s reported 43-app byte identity, matching `expose` failure and identical diagnostics remain reported evidence, not independently reproduced here.
