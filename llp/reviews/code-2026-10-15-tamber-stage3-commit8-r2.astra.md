# Code review: LLP 1121 stage 3, commit 8, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `da314db09820144a403716ee5d6e06c3c05e6dbcc957797bcb410ba54ab15d43`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: thirteen medium, one low; Grok: three high, three medium.
- **Disposition:** fixed in round 2

---

No high-severity findings. These issues remain:

1. **Medium — Tokens Studio precedence is only enforced within each theme.** [dtcg.rs:404](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:404)  
   With `tokenSetOrder = [early, late]`, an earlier theme group selecting `late` loses to a later group selecting `early`. Group order overrides the global set order; a later group’s `source` set can also override an earlier group’s `enabled` set. Assemble each tuple’s selected sets first, then apply global status/set precedence. [Tokens Studio ordering](https://docs.tokens.studio/manage-tokens/token-sets/)

2. **Medium — Sanitisation silently merges distinct axes and values.** [dtcg.rs:410](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:410), [dtcg.rs:769](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:769)  
   Groups `Brand Tone` and `Brand-Tone` become one axis. Themes `High Contrast` and `High-Contrast` become one value, activating both layers together. This loses combinations or selects the wrong definition without a collision diagnostic. Preserve raw identities and reject/report collisions before constructing scopes. The shared representation also affects Figma and resolver modes.

3. **Medium — `$extends` remains shallow and non-transitive.** [dtcg.rs:203](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:203)  
   If `base.g` contains `a` and `b`, extending it while overriding `g.a` removes inherited `g.b`. A second extension of a group containing only `$extends` inherits nothing. Resolve inheritance recursively, deep-merge groups, replace whole tokens at matching paths, and diagnose cycles/missing targets. Keep this separate from the Resolver Module’s correctly shallow sibling merge. [Format inheritance rules](https://www.designtokens.org/tr/2025.10/format/#inheritance-semantics)

4. **Medium — Whole-token `$ref` aliases disappear uncounted.** [dtcg.rs:157](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:157)  
   `alias: {"$ref":"#/base"}` is classified as a group; copying its target then discards `$value`. No alias or unmapped entry results. Resolve reference objects before deciding whether they denote tokens, preserving a reference where possible. The current test covers only `$ref` *inside* `$value`. [Format JSON Pointer support](https://www.designtokens.org/tr/2025.10/format/#json-pointer-support)

5. **Medium — Alias pointers do not decode escaped segments.** [dtcg.rs:663](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:663)  
   `#/x~1y/$value` looks up literal `x~1y`, rather than `x/y`. It either reports a valid alias as dangling or selects the wrong token if both names exist. Decode JSON Pointer escapes before constructing the lookup path. [Pointer resolution rules](https://www.designtokens.org/tr/2025.10/format/#resolution-algorithm-for-json-pointer)

6. **Medium — External references retain the original document’s base.** [dtcg.rs:469](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:469), [dtcg.rs:506](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:506)  
   After loading `sub/sets.json`, its `#/…` references still address the outer resolver, and `./colors.json` resolves beside the outer resolver. Valid imports fail—or read another existing file. Carry each referenced node’s owning document and base path through subsequent resolution.

7. **Medium — Nested set expansion stops after one level.** [dtcg.rs:507](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:507)  
   For `resolutionOrder → A.sources → B.sources → C.sources`, the inner set wrapper reaches `tokens()`, which ignores its `sources` array. Its tokens vanish, potentially yielding a successful zero-count import. Recursively expand referenced sets with cycle detection until reaching token structures.

8. **Medium — Unused resolver sets escape the count.** [dtcg.rs:513](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:513)  
   Only definitions reached through `resolutionOrder` enter `Read.defs`. An unused declared set contributes neither to `source` nor to `reported`, unlike the Tokens Studio fix. Inventory source definitions independently of layer selection and report those unused by every resolution.

9. **Medium — Existing `under scheme=dark` axes change imported semantics.** [dtcg.rs:785](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:785)  
   Matching the default value permits reuse of an existing dark-only axis. An imported `when brand=loud: #ff0000`, intended to apply in both schemes, then changes only the dark side. Reject incompatible `under_dark` axes or explicitly preserve the imported semantics.

10. **Medium — The tuple cap excludes retained values of existing axes.** [dtcg.rs:803](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:803)  
    An existing 64-value axis plus an import containing its default and one new value passes as two tuples. Reconciliation appends the new value, producing an invalid 65-tuple proposal. Calculate the checked product over the final union of existing and proposed values.

11. **Medium — Depth guards misclassify or silently discard valid input.** [dtcg.rs:686](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:686), [dtcg.rs:154](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:154)  
    An 18-hop untyped alias chain is reported as having no `$type`, despite ending at a typed colour. Tokens below traversal depth 32 disappear without entering the count. Use cycle-aware resolution; any deliberate resource limit must produce an explicit diagnostic rather than missing definitions.

12. **Medium — Name collisions prevent otherwise resolvable aliases from being inlined.** [dtcg.rs:870](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:870)  
    With distinct tokens `a b` and `a-b`, the second collides after sanitisation. An alias to `{a-b}` is then reported as dangling because that raw path was removed from the winner map. Keep colliding source definitions available for resolution, suppress their emitted names, and inline dependants using their resolved values.

13. **Medium — Alias resolution has cubic worst-case work per tuple.** [dtcg.rs:712](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:712)  
    Every token retraverses its alias chain, and every step linearly scans `visiting`. A chain of 1,000 explicitly typed tokens across 64 tuples entails roughly 10.7 billion membership comparisons. Memoize type/colour resolution per tuple and use constant-time visitation state. This is an algorithmic finding, not a measured runtime claim.

14. **Low — Uppercase JSON extensions are accepted but misclassified.** [dtcg.rs:246](/tmp/rv-s3c8/tamber/src/import/dtcg.rs:246), [tamber_import.rs:131](/tmp/rv-s3c8/contract/cli/src/tamber_import.rs:131)  
    `Theme.Light.JSON` and `Theme.Dark.JSON` are read, but become separate singleton collections with mode `JSON`; light/dark composition is lost. `$themes.JSON` similarly evades detection. Strip extensions case-insensitively throughout classification and set-name handling.

I found no separate defect in the new `when` specificity/tie handling, axis-value ordering, or preserved references between successfully resolved tokens. Counting a winning definition whose assembled token is dropped is now correct.

Verification: all 22 existing import tests passed using the prebuilt binary from the matching HEAD checkout; caps and `git diff --check` passed here. An in-memory mirror checked 55,872 tuple selections successfully. No files changed; no fresh Cargo build was performed.
