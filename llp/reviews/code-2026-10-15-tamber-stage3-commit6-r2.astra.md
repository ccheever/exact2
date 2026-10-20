# Code review: LLP 1121 stage 3, commit 6, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `17b5bf6126a446f26db2cbd9b8b48bf81e7471b769bcdee4dd07bdb47229e4db`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, nine medium, one low; Grok: one high, five medium, four low.
- **Disposition:** fixed in round 2; an unclosed rule's later rules stay nested in it, as CSS nesting reads them

---

Found **1 high, 9 medium, and 1 low** issue.

1. **High — Re-import deletes conditions the importer does not own.** [tamber/src/import/mod.rs:147](/tmp/rv-s3c6/tamber/src/import/mod.rs:147)  
   Every existing `when` absent from the import is removed. An unchanged source therefore deletes an authored `when density=compact` on `shadcn.background`, even though shadcn supplies no density axis. Restrict removal to conditions owned by the importer, retaining that ownership when source themes disappear.

2. **Medium — Colour classification runs before supersession and across unrelated targets.** [tamber/src/import/shadcn.rs:243](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:243)  
   With `:root { --x:#fff } .dark { --x:1rem; --y:var(--x) }`, `--y` is incorrectly mapped as a colour because `--x` is a colour elsewhere. Also, `--x:#fff; --x:1rem` still imports `#fff`: the non-colour declaration is skipped before replacing the winner. A superseded literal can even seed a subsequently cyclic alias graph. Select winning raw declarations first, including non-colours, then resolve colour eligibility and cycles per target.

3. **Medium — Nested selectors lose their enclosing theme.** [tamber/src/import/shadcn.rs:186](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:186)  
   Only the innermost selector is interpreted. `.theme-blue { &.dark { --background:#111 } }` becomes a default-theme dark declaration, losing `blue` and potentially changing every theme. Resolve nested selectors against their enclosing selectors, or report unsupported nesting rather than mapping it incorrectly.

4. **Medium — Nested declarations are processed in the wrong order.** [tamber/src/import/mod.rs:327](/tmp/rv-s3c6/tamber/src/import/mod.rs:327)  
   Children are emitted immediately, but all parent declarations are emitted afterward. In `.dark { --x:#111; @media (prefers-color-scheme:dark) { --x:#222; } }`, the earlier `#111` wins and the later `#222` is reported as superseded. Emit declaration runs in source order, flushing the parent’s preceding declarations before descending.

5. **Medium — Unbalanced braces silently truncate the counted source.** [tamber/src/import/mod.rs:323](/tmp/rv-s3c6/tamber/src/import/mod.rs:323)  
   `:root { --background:#fff;` produces no blocks and succeeds with zero source values. A stray `}` also stops scanning subsequent rules. Return a syntax diagnostic or recover explicitly; an incomplete scan must not pass the completeness check.

6. **Medium — Braced custom-property values disappear entirely.** [tamber/src/import/mod.rs:306](/tmp/rv-s3c6/tamber/src/import/mod.rs:306)  
   `:root { --config:{a:b}; --background:#fff; }` treats `--config:` as a nested-rule prelude. `--config` is neither counted nor reported. Recognize a custom-property declaration before interpreting its braces, consuming balanced value blocks through the declaration terminator.

7. **Medium — Themes containing only reported values produce no axis.** [tamber/src/import/shadcn.rs:297](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:297)  
   Theme discovery occurs only after successful colour mapping. `.theme-blue { --radius:1rem; }` consequently proposes no `theme=blue` axis. With no other operations, `importTheme` also skips holding its review. Collect themes from recognized selector targets independently of whether their declarations map.

8. **Medium — Accepted role offers retain overriding `when` lines.** [tamber/src/import/shadcn.rs:436](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:436)  
   An existing role with conditional values receives only a default `SetToken`. Its conditions continue overriding the offered mapping, and subsequent library imports keep offering the same default assignment because `whens` remains nonempty. Include the necessary conditional removals in the separately accepted offer, or explicitly describe a default-only offer and suppress repeated no-ops.

9. **Medium — “Mapped” literals can make the entire proposal invalid.** [tamber/src/import/shadcn.rs:114](/tmp/rv-s3c6/tamber/src/import/shadcn.rs:114)  
   A multiline `oklch()` is accepted and returned verbatim, but edit operations reject newline-containing values. `currentColor` is also accepted here, although Tamber’s evaluator explicitly rejects it as a token value. Normalize supported literals to one line and report context-dependent colours such as `currentColor`.

10. **Medium — Malformed review metadata throws after deleting the proposal.** [scripts/tamber-server.mjs:420](/tmp/rv-s3c6/scripts/tamber-server.mjs:420)  
    `review: { offers: {} }` passes the object-only validation. Partial acceptance then throws on `.map()` after `proposals.delete(id)`; retry returns 404. This reproduced against the actual server module with an in-memory backend. Validate the consumed metadata structure before holding it, and restore the proposal on recomputation exceptions.

11. **Low — Partial acceptance retains a stale token count.** [scripts/tamber-server.mjs:418](/tmp/rv-s3c6/scripts/tamber-server.mjs:418)  
    Starting with two token operations and one offer, keeping `[0,2]` correctly renumbers the offer to `[1]`, but leaves `review.tokens = 2`. The review now claims two token operations although only one remains. Recompute the token count from retained original indices. This also reproduced in memory.

No additional finding in supported-colour selector-list counting, theme-only default/`when` selection, quoted-string escapes, ordinary URL semicolons, the header-comment fix, or cargo spawn-error handling.

**Validation:** caps passed all eight categories; `git diff --check` passed. The existing tests miss the counterexamples above. Rust and filesystem integration suites were not run because this read-only session has no build artifacts. No files changed.
