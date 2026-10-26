# Code review round 1: B2-levels per side (e33c12527e), Astra

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — Mixed-axis selection lacks regression coverage.** [contract/cli/tests/it/tamber_levels.rs:588](/tmp/rv-sides-astra/contract/cli/tests/it/tamber_levels.rs:588). The scheme tests use only an under-dark axis; the overlapping-`when` test at line 528 uses only normal axes. Neither exercises their interaction.

   Repro: declare normal axis `mode = a | b` and `dark-style = black | dim under scheme=dark`, with:
   ```text
   theme
     ink = light-dark(#111111, #222222)
   params
     Map.pin = reading.chain
       when mode=b: ink
       when mode=b dark-style=dim: #000000
   ```
   For `(b, dim)`, expect light `app-token/ink`, value `#111111ff`; dark `app-param/Map.pin` (full name), value `#000000ff`. For `(b, black)`, both levels name `ink`, with values `#111111ff` and `#222222ff`. This catches treating mixed conditions as scheme-independent or ignoring normal branches on dark. For example, changing `.any(under)` to `.all(under)` would escape the existing fixtures. Exercise this selection on package tokens too, and assert folded values alongside provenance.

2. **SHOULD-FIX — The documented light fallback is inaccurate.** [docs/tamber-protocol.md:570](/tmp/rv-sides-astra/docs/tamber-protocol.md:570), repeated in [llp/1121-tamber.rfc.md:5573](/tmp/rv-sides-astra/llp/1121-tamber.rfc.md:5573) and [contract/cli/src/tamber_levels.rs:169](/tmp/rv-sides-astra/contract/cli/src/tamber_levels.rs:169). “The light side taking the line’s own value” omits applicable normal-axis branches. In the repro above, light follows `when mode=b: ink`, not `reading.chain`. Say that light selects the most-specific holding `when` containing no under-dark axis, falling back to the token’s own value only when none holds. The implementation already does this correctly.

Read-only inspection found no blocking implementation defect: selection matches `select`/`under_value`; the side propagates through param, app-token and package-token references; expressions including `light-dark()` stop at their owning line; the eight-hop cap remains intact; and `levels.base` checks both sides. The 1.15 clause records the shape correction. The new counterexample would fail against the old behavior, although it currently asserts provenance without the corresponding folded values. No builds or tests were run.
