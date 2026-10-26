# Code review round 3: LLP 1121 stage 12 B3 (a08a04bf92), Astra

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — JS shadow reconstruction still mistakes platform colours for lengths.** [host/web-js/src/base_scale.rs:635](/tmp/rv-s12b3-astra/host/web-js/src/base_scale.rs:635)  
   Repro: `card.shadow: box-shadow = 0 1px 4px -exact-platform-color(ios named:space.md, #000)`, used by a development JS build. `length()` counts the colour as a fourth length because `named_in()` finds `space.md`. When reconstruction supplies the omitted spread, line 599 inserts `-exact-platform-color(ios named:var(--exact-space-md), #000)` into that slot, producing invalid shadow CSS. A text shadow with two lengths similarly corrupts its omitted blur. Apply the kernel’s length-function restriction here too. Round 2’s test checks compilation and kernel folding, so it misses this downstream failure.

2. **NIT — QUEUE still describes the superseded resolve shape.** [QUEUE.md:28](/tmp/rv-s12b3-astra/QUEUE.md:28)  
   Repro: resolving `card.shadow: box-shadow = 0 space.md 4px black` returns `lengths: [0,null,4]` and `length_texts: ["0px","space.md","4px"]`; this entry says residual text appears in `lengths`. Update it to match theme2’s ruling and the correctly updated 1.15 clause.

Round 1 accounting: A1–A5 and G1/G2/G4 are addressed in their original paths. G5’s header change is explicitly retained, with the required baseline test covering unchanged imports and exactly one edited value. **G3 remains deliberately conservative:** stale dependency bits are documented rather than removed; subsequent base edits can redundantly reset a shadow without changing its pixels.

Round 2 accounting: underscore-prefixed names and unkinded shadows of functions are fixed. The platform-colour refusal is fixed in `fold_lengths`, whose function matching is case-insensitive, but finding 1 leaves the JS case incomplete.

The widened length-function path appears sound for set/unset bases, aliases, nested functions and `min`/`max`/`clamp`; B2’s refusals remain. Folded negative px blurs reach the check-time rejection, and residual blurs receive per-column checks. `also_read` and `looks_read` preserve newly discovered dependencies. Resolve’s numeric/null arrays and conditional `length_texts` match the ruling.

Read-only source review; no builds or tests run. Reported red checks and the 43-app comparison remain unverified here. Schema/format declaration inputs are unchanged; changed Rust files are at most 1,499 lines.
