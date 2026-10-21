# Review round 2, LLP 1121 stage 12 B2-levels, Astra (gpt-6-astra, max)

RIGHT WITH CHANGES

1. **SHOULD-FIX — [tamber_levels.rs:252](/tmp/rv-s12b2-r2-astra/contract/cli/src/tamber_levels.rs:252): `levels.token` exceeds the eight-hop cap.** For `p0 → p1 → … → p8 → reading.pin`, `walk(p0)` stops at `p8`, but `levels()` follows eight param references and then looks up `reading.pin`—a ninth hop. Consequently, `reaches.token` exceeds the cap too. Include the final token lookup in the shared traversal’s budget; test tokens reached at hops eight and nine.

2. **SHOULD-FIX — [tamber_levels.rs:211](/tmp/rv-s12b2-r2-astra/contract/cli/tests/it/tamber_levels.rs:211): Tuple coverage misses selection errors.** Tests use one axis and one `when`; none condition an app `params` override. Incorrect tuple ordering, first-match selection, or ignoring param `when`s could pass. Add two axes with overlapping general/specific branches on package tokens and app overrides, asserting every tuple’s value, level and name.

Round 1’s reported role, icon, alias, null-level and common-base cases are fixed by inspection. Wire additions remain under `tamber/1.15`; changed sources satisfy the 1,499-line cap.

Focused tests could not run: the read-only sandbox denied Cargo’s build lock in `/tmp/rv-target-astra`.
