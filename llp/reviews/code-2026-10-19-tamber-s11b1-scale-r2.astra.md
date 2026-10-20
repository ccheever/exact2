# Review round 2, LLP 1121 stage 11 B1 (the base scale table), Astra (gpt-6-astra, max)

RIGHT WITH CHANGES

1. **SHOULD-FIX** — [runner/src/runner/tamber.rs:272](/tmp/rv-s11b1-r2-astra/runner/src/runner/tamber.rs:272): A retained `card.pad: length = space.md` resolves through style rows, but `Tamber::length("card.pad", …)` returns `None`: `points()` cannot parse `"space.md"`. Resolve through `theme::measure` under the selected tuple’s table; test native binding reads across columns and edits.

2. **SHOULD-FIX** — [tamber/src/export_kinds.rs:56](/tmp/rv-s11b1-r2-astra/tamber/src/export_kinds.rs:56): The same alias exports as **1px**, because base names are absent from `Vocab.sizes` and this fallback invents a value. Resolve exports from the shared scale table under a documented column policy, or explicitly refuse them; add a regression test.

3. **SHOULD-FIX** — [contract/lower/src/scale.rs:50](/tmp/rv-s11b1-r2-astra/contract/lower/src/scale.rs:50): `tamber-base-kind` runs only during lowering. A design-only server transaction can set `space.md` to a colour and commit an unbuildable design; `write_checks` checks roles and params, not this invariant. Move the reserved-name validation, including icons, into the shared Tamber checker using `Vocab::bases`.

4. **SHOULD-FIX** — [kernel/build/scale.rs:35](/tmp/rv-s11b1-r2-astra/kernel/build/scale.rs:35): Every numeric `style` metric is accepted as a length. For example, Neutral `space.xs =grouped-button-title-grow` becomes **1px**, and both provenance and monotonic checks accept that dimensionless flex-grow value. Restrict references to metrics holding lengths and add this refusal case.

Round 1’s reported paths are fixed by inspection. All 45 cells match their recorded sources; caps and formatting pass. Digest verified: `0xad42_cc2d_c5b5_26f3 → 0x374f_477d_d305_5b7e`; PROTOCOL unchanged. Cargo tests were sandbox-blocked; live-runtime behavior and release-byte equivalence remain unverified.
