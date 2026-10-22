# Code review round 2: LLP 1121 stage 12 B3 (a16ed67f65), Astra

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — G4’s colour-function exclusion misses valid platform colours.** [kernel/src/style/scale.rs:162](/tmp/rv-s12b3-astra/kernel/src/style/scale.rs:162)  
   Repro: declare `card.shadow: box-shadow = 0 1px 4px -exact-platform-color(ios named:space.md, #000)` and use `box-shadow="card.shadow"`. `named:space.md` is a valid asset name, but `named_in` finds its `space.md`; because `-exact-platform-color` is absent from the exclusion, `fold_lengths` substitutes inside the colour and attempts to evaluate it as length arithmetic. Lowering consequently reports `tamber-base-expression`, “lengths don't fold on ios,” despite all lengths being literal px. Text shadows have the same problem. Preserve platform-colour functions whole and add this regression case. The exclusion also omits `contrast-color`.

2. **NIT — The dotted-name diagnostic excludes valid underscore-prefixed names.** [contract/lower/src/theme/scan.rs:420](/tmp/rv-s12b3-astra/contract/lower/src/theme/scan.rs:420)  
   With a nonempty design, compile `box-shadow="0 0 8px _eu.nope"` or `border="1px solid _eu.nope"`. These retain the generic colour/shorthand refusal instead of `tamber-unknown-token`. Tamber’s identifier grammar explicitly permits `_` first; `dotted_name` requires a letter. Match the actual identifier grammar while continuing to exclude dimensions and functions.

Round 1 accounting: **A1–A5 and G1/G2 are addressed by inspection.** The function path consistently uses B2 folding for set/unset bases, aliases, nested functions and `min`/`max`/`clamp`, retaining B2’s refusals. Its widened px-token acceptance appears sound. A folded negative px blur reaches the existing check-time rejection; residual blurs receive per-column validation. Rows and looks preserve newly recorded reads through `also_read` and `looks_read`.

Resolve now emits numeric/null `lengths` and conditional parallel `length_texts`, matching the 1.15 clause. The baseline test covers unchanged old-header imports and exactly one edited value, against both kept and fresh baselines. **G3’s conservative stale dependency bits remain**, now documented; they cause redundant updates without changing pixels. **G4’s original examples are fixed**, including case-insensitive matching, but finding 1 remains. **G5 is resolved by theme2’s ruling and the baseline test.** Literal shadow base names remain queued.

Read-only source review; no builds or tests run. I did not independently reproduce the reported red checks or 43-app comparison. Schema/format digest inputs are unchanged, and changed Rust files are at most 1,499 lines.
