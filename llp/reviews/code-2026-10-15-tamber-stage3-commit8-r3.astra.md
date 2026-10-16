# Code review: LLP 1121 stage 3, commit 8, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `40ff248a075da7f8472bfacb5cb94fccbe28b1c1c3889c81ebaa7fe217b99c26`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: six medium; Grok: five medium, two low.
- **Disposition:** applied without re-review; two themes with the same empty name in one group are not refused

---

No high-severity findings. I found six medium-severity issues:

1. **Medium — Whole-token `$ref` classification still loses aliases.** [tamber/src/import/dtcg.rs:263](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:263)  
   With `a → b → c` expressed as whole-token `$ref`s and `c` a colour token, `b` is classified as a group when processing `a`. `extended()` then produces an empty group, so `a` disappears from both the proposal and source count. The same classifier lets `deep()` merge a whole-token alias into an inherited group, retaining that group’s `$type`. **Fix:** resolve reference chains sufficiently to distinguish tokens from groups, with cycle detection; replace token definitions atomically.

2. **Medium — Tokens Studio themes selecting no sets disappear.** [tamber/src/import/dtcg.rs:960](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:960)  
   Axes are reconstructed from set layers. With a common base theme, `brand=calm` selecting no sets, and `brand=loud` selecting an override, only `loud` reaches the axis. Its override becomes the default, and `calm` cannot be selected. Set precedence can also change which value becomes the default. **Fix:** retain the complete, ordered theme dimensions separately from set activation and precedence.

3. **Medium — Sibling overrides acquire the referenced document’s base.** [tamber/src/import/dtcg.rs:595](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:595)  
   In `r.json`, an entry such as `{"$ref":"sub/sets.json#/sets/base","sources":[{"$ref":"colors.json"}]}` resolves the overridden source against `sub/`, although it was authored in `r.json`. It either fails or silently imports `sub/colors.json`. **Fix:** preserve the originating document for each merged subtree, including local sibling overrides. This follows the [Resolver’s document-relative reference and override rules](https://www.designtokens.org/tr/2025.10/resolver/#reference-objects).

4. **Medium — Reached-set tracking confuses documents and escaped names.** [tamber/src/import/dtcg.rs:628](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:628)  
   Following `#/sets/inner` inside an external document marks the root document’s unrelated `sets.inner` as reached. Its unused tokens are consequently skipped entirely—neither counted nor reported. Conversely, `#/sets/a~1b` does not match the actual key `a/b`, so a used set is reported again as unused. **Fix:** track resolved document identity plus decoded pointer, including references followed inside `deref()`.

5. **Medium — Parent traversal above the source root selects the wrong file.** [tamber/src/import/dtcg.rs:553](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:553)  
   From root-level `r.json`, `../colors.json` normalizes to `colors.json`: popping an empty vector silently discards `..`. If that sibling exists, its colours are imported despite being a different target. **Fix:** reject traversal above the supplied source root, or preserve unresolved parent segments so lookup fails correctly.

6. **Medium — Layer merging retains descendants of a replaced group.** [tamber/src/import/dtcg.rs:1080](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:1080)  
   An earlier source defining `g.a` followed by a source defining token `g` leaves both winners, because merging operates only on flattened exact paths. The proposal imports the obsolete descendant. Such a result cannot represent a valid [DTCG token/group structure](https://www.designtokens.org/tr/2025.10/format/#group-structure). **Fix:** merge structured trees before flattening, applying token/group replacement, or explicitly reject the collision.

The worktree remains unchanged. Cargo tests were not run because the sandbox is read-only; the findings come from source review and in-memory control-flow traces.
