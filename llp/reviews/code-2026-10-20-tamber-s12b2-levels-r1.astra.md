# Review round 1, LLP 1121 stage 12 B2-levels, Astra (gpt-6-astra, max)

RIGHT WITH CHANGES

1. **BLOCKING — [tamber_levels.rs:96](/tmp/rv-s12b2-r1-astra/contract/cli/src/tamber_levels.rs:96): Role overrides resolve incorrectly.** With `secondary-label = #123456` and a default naming `-exact-secondary-label`, the compiler uses the override but the walker reports `role`. Normalize both spellings using the checker’s role rules. Test both directions and mixed case.

2. **BLOCKING — [tamber_levels.rs:218](/tmp/rv-s12b2-r1-astra/contract/cli/src/tamber_levels.rs:218): Tuple 0 cannot represent the common base.** A chain reaching `space.md` in one mode and `space.lg` in another reports `levels.base: "space.md"`, also contaminating `reaches`. Return a base only when every tuple reaches that same base; otherwise null. Add that regression and update the docs.

3. **BLOCKING — [tamber_base.rs:369](/tmp/rv-s12b2-r1-astra/contract/cli/src/tamber_base.rs:369): Icon params miss the additions entirely.** `resolve` returns at line 277 and `explain` at line 404 before annotation. Move param annotation onto both return paths, and teach the walker to follow app icon tokens. Test an icon reference, a literal package icon param, and its app override.

4. **SHOULD-FIX — [tamber_levels.rs:203](/tmp/rv-s12b2-r1-astra/contract/cli/src/tamber_levels.rs:203): Param-reference metadata bypasses the walker.** A default such as `edge = param.pin` gets `levels.token: null` although it names a real param token; tuple resolution follows it correctly. Have the bounded traversal supply first-reference provenance for `levels`/`reaches`, preserving public param names. Cover both Contract and `.tamber` aliases.

5. **SHOULD-FIX — [tamber_levels.rs:199](/tmp/rv-s12b2-r1-astra/contract/cli/src/tamber_levels.rs:199), [tamber_base.rs:383](/tmp/rv-s12b2-r1-astra/contract/cli/src/tamber_base.rs:383): Non-reference defaults violate the field contract.** `bare()` accepts literals such as `transparent` and `-1`, yielding a levels object; expressions omit `levels` instead of returning the approved null. Distinguish actual references from literals, emit explicit null, and correct the tests currently asserting absence.

The sixth `default` level is appropriate. All changed source files satisfy the 1,499-line cap. Tests could not run: the read-only sandbox denied Cargo’s build lock.
