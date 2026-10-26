# Code review round 1: LLP 1121 stage 12 B3 (c351b29131), Astra

(Astra's first run stopped at model capacity after noting item 1; this is its full rerun on the same commit.)

**RIGHT WITH CHANGES**

1. **BLOCKING — Defining a base breaks an otherwise accepted shadow expression.** [tamber/src/shadow.rs:355](/tmp/rv-s12b3-astra/tamber/src/shadow.rs:355)  
   Confirmed the known gap: `card.shadow: box-shadow = 0 calc(space.md * 2) 4px black` is accepted while `space.md` is unset. Add `space.md = 10px` and `over_base` becomes false, sending `calc(...)` to the colour parser instead of producing a 20px offset. The same problem affects `min`/`max`/`clamp` and aliases whose final unset base becomes defined. Dispatch these length functions through length folding regardless of whether a residual remains.

2. **BLOCKING — Newly read bases are still lost for state looks.** [runner/src/runner/theme.rs:119](/tmp/rv-s12b3-astra/runner/src/runner/theme.rs:119), [runner/src/instance/looks.rs:70](/tmp/rv-s12b3-astra/runner/src/instance/looks.rs:70)  
   `also_read` updates ordinary rows only; `looks::op` discards the reads collected during re-resolution. Repro: adapt the new live-edit test so `card.shadow` is used by a `:hover` box-shadow, keeping `padding="space.md"` to retain that token. Start with `space.md=10px` and a literal shadow. Call `set_token("card.shadow", "0px space.md 4px #000000ff", &[])`, then set `space.md` to `20px`. The hover shadow first becomes 10px but remains there after the second edit, while padding updates. Preserve newly read dependencies for looks too.

3. **BLOCKING — Resolve contradicts the plan’s explicit round-one wire ruling.** [contract/cli/src/tamber_base.rs:348](/tmp/rv-s12b3-astra/contract/cli/src/tamber_base.rs:348)  
   Resolving `card.shadow: box-shadow = 0 space.md 4px black` returns mixed numeric/string `lengths`, approximately `[0,"space.md",4]`. The approved plan’s appended ruling requires `[0,null,4]` and parallel `length_texts: ["0px","space.md","4px"]` when residuals exist. Update the implementation, protocol clause, and test currently asserting the mixed array. This is separate from the deliberately queued per-column shadow text.

4. **SHOULD-FIX — Border diagnostics mistake decimal dimensions for token names.** [contract/lower/src/shorthands.rs:190](/tmp/rv-s12b3-astra/contract/lower/src/shorthands.rs:190)  
   With any nonempty design bound, compile `border="-1.5px solid black"`. The negative width fails width parsing, then the new `contains('.')` condition reports that `-1.5px` is “no token of this app’s design.” Invalid functional colours containing decimal numbers can receive the same misleading diagnosis. Restrict this branch to dotted identifiers; preserve the width/colour diagnostic for dimensions and functions.

5. **SHOULD-FIX — The approved export-header change lacks its required baseline regression test.** [tamber/src/export.rs:47](/tmp/rv-s12b3-astra/tamber/src/export.rs:47)  
   The plan explicitly requires testing an export pulled before the `CONVERSIONS` change. No such test accompanies HEAD. Repro to cover: retain an export with the previous header, import it under HEAD against the unchanged design, and assert no edits; then change one exported value and assert only that edit. `baseline_for` appears to select by design digest correctly, but this required compatibility check is missing.

Read-only review; no builds or tests run. The required live-base, column-naming negative-blur, and JS/wasm shadow tests are present. Digest inputs are unchanged, and changed source files meet the 1,499-line limit. The claimed 43-app byte comparison remains unverified here.
