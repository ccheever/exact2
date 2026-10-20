# Review round 1, LLP 1121 stage 11 B1 (the base scale table), Astra (gpt-6-astra, max)

RIGHT WITH CHANGES

1. **BLOCKING** — [contract/lower/src/theme.rs:1199](/tmp/rv-s11b1-r1-astra/contract/lower/src/theme.rs:1199): Base names reach view validation but not Tamber’s evaluator. `card.pad: length = space.md` in `app.tamber` still produces `tamber-unknown-token`; `vocab()` never supplies the base names. Support base references in design values and `when` arms, preserving per-host resolution and precedence of app definitions.

2. **SHOULD-FIX** — [contract/lower/src/theme.rs:440](/tmp/rv-s11b1-r1-astra/contract/lower/src/theme.rs:440): `tamber-base-kind` checks only `folding.tokens`. An icon named `space.md` bypasses it, while padding silently reads the base length. Apply the reserved-name check to icon declarations too, with a refusal test.

3. **SHOULD-FIX** — [kernel/build/scale.rs:115](/tmp/rv-s11b1-r1-astra/kernel/build/scale.rs:115): Source validation checks only identifier spelling. A nonexistent measurement ID passes generation, contrary to the approved build refusal. Validate documentation entries during generation and add the document as a Cargo rebuild dependency.

4. **SHOULD-FIX** — [kernel/src/style/scale.rs:232](/tmp/rv-s11b1-r1-astra/kernel/src/style/scale.rs:232): The provenance test accepts any bold number in a shared entry. Changing Material `radius.md` from **8 to 4** passes both this predicate and monotonicity. Bind each documented value to its specific step, using distinct source IDs or explicit row/value mappings.

Verified the current 54 cells, caps, formatting, unchanged PROTOCOL, and digest move `0xad42_cc2d_c5b5_26f3 → 0x7dbe_77e0_84c0_e38c`. Rust tests were blocked by sandbox denial of target-directory creation; release-byte equivalence remains unverified.
