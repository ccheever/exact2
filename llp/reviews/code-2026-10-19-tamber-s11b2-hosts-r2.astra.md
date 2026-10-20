# Review round 2, LLP 1121 stage 11 B2 (the hosts' columns), Astra (gpt-6-astra, max)

First pass (per-name design):

**RIGHT WITH CHANGES**

1. **BLOCKING** — [measures.rs:403](/tmp/rv-s11b2-r2-astra/host/web-js/src/measures.rs:403): Maps now differ by `reads_length`, but `map_name` still gives keyword and length rows the same `$tl` key. With a retained length token, a parent’s bound `color-scheme` can cache a map without bases; a child’s bound padding then reuses it and cannot resolve `space.md`. Distinguish these maps in the cache key; test keyword-first ordering.

2. **BLOCKING** — [base_scale.rs:147](/tmp/rv-s11b2-r2-astra/host/web-js/src/base_scale.rs:147): `padding=("space." + size + " ")` declares no bases: `no_base` treats the trailing space as disqualifying, although runtime lookup trims it. With `size="md"`, valid spacing disappears. Make concatenation pruning account for trimming; test leading and trailing whitespace.

3. **BLOCKING** — [measures.rs:387](/tmp/rv-s11b2-r2-astra/host/web-js/src/measures.rs:387): The SVG fix freezes bound aliases into literal maps. For `pattern width=size`, where `size="card.pad"`, switching the alias from `space.md` to `space.lg` leaves width at 16px: the string binding already declares all nine bases, structure stays unchanged, and the batch updates only CSS properties. Mark potentially affected attribute tokens for rebuild, or update their maps and bindings live.

4. **BLOCKING** — [dev.mjs:782](/tmp/rv-s11b2-r2-astra/host/web-js/dev.mjs:782): Round 1’s alias-switch fix misses scrubbing. `scrubShow` ignores `structure`, and `design-serve` patches retain the previous structure. A read-only probe accepted `space.md → space.lg` and emitted an undeclared `var(--exact-space-lg)`. Check required bases in both scrub paths before batching; require rebuilding when they change.

5. **SHOULD-FIX** — [base_scale.rs:239](/tmp/rv-s11b2-r2-astra/host/web-js/src/base_scale.rs:239): Region typing loses constructed values: `Some` preserves the inner type, while `List` falls through as unknown. Numeric `match some(size)` and `for n in [12,16]` bindings therefore declare all nine bases, changing releases that use none. Preserve option/list element types and add numeric and string region tests.

Caps pass (`emit.rs`: 1,499 lines). Six focused tests pass using existing B2 binaries; saved comparisons confirm 14/14 apps match. Fresh Cargo testing was blocked by sandbox denial of the build lock.

Re-review on the simplified build (theme2's all-nine-or-none):

**RIGHT WITH CHANGES**
