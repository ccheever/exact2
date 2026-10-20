# Code review: a folded :disabled or :checked row read as its base (LLP 1122 queue item (2)), the second narrow verification round, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `cc1e2b922b`.
- **Method:** one brief (sha256 `2f85d7629846492f39a831f6e281d610426e2bda5f953ca847059c54f3137f44`) quoting the first narrow round's two reviews verbatim, asking of each open finding "closed? broke anything?"; commit cc1e2b922b. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER (new: the case-fold reached custom tokens); A1 and B1 closed; 3 NIT.
- **Disposition:** taken in the commit named in the sign-off request:
  - (1) The broad case-fold is reverted to the match the first narrow round found closed: a name matches exactly, or a role's `-exact-` spelling in any case against its canonical lowercase name; a custom token's case is its own (tested: `brand.fill` with `Brand.Fill` reported keeps `brand.fill`). Grok's first-narrow-round NIT 3 (a bare system colour in another case) is therefore declined: matching it needs knowing the name is a role, and losing it costs only the token beside the base's own edit.
  - (3) Two parts of one state are told apart by file, line and column.
  - (5) docs/reference.md says a Contract source carries its `home` (a design or `exact.json` none).
  - (2) and (4) as the review says (closed; the decline holds; the first narrow round's record overstated the cost of telling a full literal apart, as noted here).

**HEAD still has one blocker:** the broader role match can offer the wrong custom token.

1. **BLOCKER — New wrong-token regression; B3’s specific case is closed.** [tamber-inspect.mjs:503](/tmp/rv-dt-q2m/scripts/tamber-inspect.mjs:503) case-folds every token name, without establishing that it is a role. Custom tokens remain case-sensitive ([check.rs:600](/tmp/rv-dt-q2m/tamber/src/check.rs:600)).

   In an in-memory join with base `brand.fill`, disabled part `Brand.Fill`, and that disabled branch showing, **HEAD offers `Brand.Fill` as the base’s token; HEAD’s parent offers no token**. With both names reported and `Brand.Fill` first, HEAD also chooses it over the exact match. The inspector exposes this as an edit action. Restrict case-insensitive matching to verified roles and preserve exact matching for custom tokens.

   B3’s `canvastext` → `CanvasText` example now works, but its fix introduces this blocker.

2. **BLOCKER — A1 closed; B1 closed too.** [tamber-inspect.mjs:481](/tmp/rv-dt-q2m/scripts/tamber-inspect.mjs:481) recovers the source home, and [:498](/tmp/rv-dt-q2m/scripts/tamber-inspect.mjs:498) matches exactly `<home>#<component>.<name>`. Probes confirmed the correct app/package param with two same-named components, and refusal when only another file’s param is cached.

   The component above the base’s source row is the correct owner for slot fills: param rewriting walks the caller’s component view, including call children, before expansion ([rewrite.rs:357](/tmp/rv-dt-q2m/contract/lower/src/params/rewrite.rs:357), [:533](/tmp/rv-dt-q2m/contract/lower/src/params/rewrite.rs:533)). The slot-fill probe selected the caller’s param.

3. **NIT — A7 partially closed.** Full paths at [inspector.ts:111](/tmp/rv-dt-q2m/apps/design-tool/inspector.ts:111) distinguish `styles/card.contract:6` from `ui/card.contract:6`.

   Columns remain omitted. A valid state line containing `border-color="#111111" border-top-color="#999999"` produces two top-side parts at the same file and line. `selectionView` gives them identical names, hence identical keys/test IDs, despite distinct edit addresses at columns 9 and 32. Include the column. This is a remaining ambiguity, not a new HEAD regression.

4. **NIT — B2 remains; the decline holds as a source-parts policy.** [states.rs:520](/tmp/rv-dt-q2m/contract/lower/src/states.rs:520) still retains the earlier part beneath a later full literal. Each edit addresses its own source attribute, so retaining authored parts is defensible.

   The [disposition’s explanation](/tmp/rv-dt-q2m/llp/reviews/code-2026-10-20-dtool-q2-n.grok.md:9) overstates the need for expression evaluation: a full literal is already distinguishable in [over](/tmp/rv-dt-q2m/contract/lower/src/states.rs:211). That does not make the deliberate provenance policy blocking.

5. **NIT — `sources[].home` is not universal, despite the documentation.** Ordinary Contract sources get a home **even when they declare no components**, because assignment occurs outside the component loop ([sources.rs:742](/tmp/rv-dt-q2m/contract/cli/src/sources.rs:742)). However, app designs, package designs, and `exact.json` bypass that assignment ([sources.rs:414](/tmp/rv-dt-q2m/contract/cli/src/sources.rs:414), [packages.rs:96](/tmp/rv-dt-q2m/contract/cli/src/sources/packages.rs:96), [sources.rs:821](/tmp/rv-dt-q2m/contract/cli/src/sources.rs:821)). The serializer therefore omits their home.

   This contradicts [reference.md:202](/tmp/rv-dt-q2m/docs/reference.md:202), but does not break the folded Contract-param lookup. Source indexes remain aligned. Relocation changes paths while retaining the compiler’s original homes ([sources.rs:63](/tmp/rv-dt-q2m/contract/cli/src/sources.rs:63)); that correctly preserves compiled param identities.

No files changed. Only source inspection and in-memory probes ran; no builds or test suites. Plan-byte equality was not remeasured.
