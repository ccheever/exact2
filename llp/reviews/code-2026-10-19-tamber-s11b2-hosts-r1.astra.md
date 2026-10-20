# Review round 1, LLP 1121 stage 11 B2 (the hosts' columns), Astra (gpt-6-astra, max)

**RIGHT WITH CHANGES**

1. **BLOCKING** — [measures.rs:38](/tmp/rv-s11b2-r1-astra/host/web-js/src/measures.rs:38): An unset-base alias becomes `var(--exact-space-md)` even in an SVG attribute map. For a bound `pattern` width, `svgGeometry` clears the attribute and writes ineffective CSS instead. Resolve aliases to the column’s literal length when `attribute=true`, as direct base names already do. Test release and dev output.

2. **BLOCKING** — [base_scale.rs:40](/tmp/rv-s11b2-r1-astra/host/web-js/src/base_scale.rs:40): Changing `card.pad: length = space.md` to `space.lg` live references `--exact-space-lg`, which the original sheet never declared. The design remains compatible, so even the verifying build keeps that sheet. Track the required base-name set in JS build compatibility and rebuild when it changes, or update its declarations with the batch.

3. **BLOCKING** — [base_scale.rs:23](/tmp/rv-s11b2-r1-astra/host/web-js/src/base_scale.rs:23): Exact string-pool matching misses computed names such as `padding=("space." + size)`: neither the variable nor mapping is emitted. Conversely, plain `text "space.md"` emits an unused variable, violating unchanged release output for nonusers. Use length-read retention, conservatively covering computed strings, rather than every interned string.

4. **SHOULD-FIX** — [base_scale_tests.rs:138](/tmp/rv-s11b2-r1-astra/host/web-js/src/base_scale_tests.rs:138): The “live edit” test compares serialized tokens; it applies no batch and contains no alias. Extend it to update an existing page from 40px to 48px and assert that both a direct row and an alias change without a shadowing base variable.

Reviewed all three named commits (`HEAD~3..HEAD`). Host wiring appears correct; caps pass, including `emit.rs` at 1,499 lines. Three existing JS batch tests pass. Read-only runtime probes confirmed findings 1–2; Cargo tests were blocked by sandbox denial of the target-directory lock.
