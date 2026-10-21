# Code review: An import is checked where it lands (LLP 1121 D14, H's follow-up), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** HEAD~1..HEAD at the time (`225aa2718`, `837bbbc46`), the landing check and its first round.
- **Method:** one brief (sha256 `d0444e526777d3fae31e8f9b0d57a1088736b86414c3c58bc9292354599e830c`), shared by both reviewers, blind to the other review, with the first round's records beside the worktree. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (P0s: an axis value that breaks the design left proposed, colour names past the edit grammar; P1s: kinds when the design does not fold, kept whens charging their owner, a when's default, two causes, quoted names).
- **Disposition:** the last round, applied without re-review in the second-round commit: every item, each with a test in `tamber/tests/it/import_kinds.rs` (`the_landing_check_keeps_what_holds_and_refuses_axes_that_do_not`).

---

Read-only source review; no files changed or tests run. R1’s vocabulary wiring, owned-`when` removal, direct alias propagation, transitive reads, long-chain termination and dial example have fixes. Repeated landing passes are consolidated. The remaining defects are:

1. **P0 — Refusing tokens leaves a failing axis edit proposed.** [landing.rs:320](/tmp/rv-land-r2/tamber/src/import/landing.rs:320), [shadcn.rs:370](/tmp/rv-land-r2/tamber/src/import/shadcn.rs:370)

   Design:
   ```text
   tamber app
   axes
     theme = default | blue
   theme
     size: length = 1rem
       when theme=default: 4px
       when theme=blue: 8px
     derived: length = calc(size + 1px)
   ```
   Import:
   ```css
   :root { --ink: #fff; }
   .theme-red { --ink: #000; }
   ```
   The original design folds. Adding `red` exposes `size = 1rem`, breaking `derived`. Landing reports both declarations and removes `shadcn.ink`, but reconciliation still proposes `AddAxisValue(theme, red)`. Acceptance therefore rejects the supposedly settled proposal. DTCG retains axes the same way.

   **Fix:** include axis operations in refusal handling, or refuse the whole import when its remaining axis edits cannot fold. Add a regression that applies the returned operations to this destination and checks it; checking token names alone misses this.

2. **P0 — Colour candidates bypass the edit grammar.** [landing.rs:96](/tmp/rv-land-r2/tamber/src/import/landing.rs:96), [kinds.rs:164](/tmp/rv-land-r2/tamber/src/import/kinds.rs:164)

   Import `:root { --café: #ffffff; }` through shadcn into `tamber app`.

   The importer proposes `AddToken("shadcn.café", "#ffffff")`, counted as mapped. `laid` constructs the token directly, and `check_design` does not validate token-name syntax. Actual acceptance calls `edit::valid`, which rejects the non-ASCII identifier. Only candidates with an explicit kind currently receive that validation.

   **Fix:** validate the actual reconciled operations for colours too, before considering them mapped. Test this valid CSS custom-property name through proposal and acceptance.

3. **P1 — Any pre-existing diagnostic disables protection for inferred kinds.** [landing.rs:148](/tmp/rv-land-r2/tamber/src/import/landing.rs:148)

   Design:
   ```text
   tamber app
   dials
     unused: color = #ffffff
   theme
     tw.brand = 8px
   ```
   Import `@theme { --color-brand: #ff0000; }` using `import_with` and the real vocabulary.

   `before` fails because the dial is inert. The fallback reads only the written kind, finds none for `tw.brand`, and permits changing the inferred length into a colour. The unchanged dial diagnostic is then filtered out, so the colour is mapped without a kind-conflict report.

   **Fix:** retain inferred-kind information even when another part of the design fails, or explicitly refuse planning against an unchecked baseline. Add this regression alongside the valid role-bearing baseline test.

4. **P1 — A retained `when` charges its owner instead of the changed dependency.** [landing.rs:85](/tmp/rv-land-r2/tamber/src/import/landing.rs:85), [landing.rs:238](/tmp/rv-land-r2/tamber/src/import/landing.rs:238)

   Design:
   ```text
   tamber app
   axes
     mode = normal | compact
   theme
     tw.radius.a: length = 4px
     tw.radius.b: length = 8px
       when mode=compact: calc(tw.radius.a + 1px)
   ```
   Import:
   ```css
   @theme { --radius-a: 1rem; --radius-b: 12px; }
   ```
   The retained `when` is stamped with `b`’s synthetic line. Its failure immediately refuses `b`. The next pass then refuses `a`, leaving both declarations reported. Refusing only `a` would allow the valid `b = 12px` update.

   **Fix:** preserve provenance for retained expressions and attribute their dependency failures through the changed dependencies. Treat ownership as a candidate, not automatic proof of responsibility. Assert that the `b` update survives.

5. **P1 — A design token’s dark-axis diagnostic loses its default dependency.** [landing.rs:259](/tmp/rv-land-r2/tamber/src/import/landing.rs:259)

   Design:
   ```text
   tamber app
   axes
     dark-style = normal | dim under scheme=dark
   theme
     tw.brand = #ffffff
     derived = tw.brand
       when dark-style=dim: light-dark(#ffffff, #222222)
   ```
   Import:
   ```css
   @theme { --color-brand: #000000; --color-safe: #ff0000; }
   ```
   `tamber-under-light` points at the literal `when`. Attribution examines only that branch, although the mismatch was caused by changing its default through `tw.brand`. It finds no candidate and refuses **both** imports, including independent `safe`.

   **Fix:** for cross-value diagnostics such as `tamber-under-light`, include the selected base/default expression’s dependencies. Test that only `brand` is reported and `safe` remains proposed.

6. **P1 — Multiple failing dependencies still cause innocent candidates to be refused.** [landing.rs:270](/tmp/rv-land-r2/tamber/src/import/landing.rs:270)

   Design:
   ```text
   tamber app
   theme
     tw.radius.a: length = 1px
     tw.radius.b: length = 1px
     tw.radius.c: length = 1px
     total: length = calc(tw.radius.a + tw.radius.b + tw.radius.c)
   ```
   Import:
   ```css
   @theme {
     --radius-a: 1rem;
     --radius-b: 1rem;
     --radius-c: 2px;
   }
   ```
   Removing either `a` or `b` alone leaves the other failure. Every counterfactual therefore retains a diagnostic with the same ID and line, and the empty-culprit fallback refuses all three—including valid `c`. This leaves R1’s “every mentioned token charged” problem partially unresolved.

   **Fix:** identify multiple responsible changes using evaluator provenance or progressively isolate the failing subset; do not interpret unsuccessful single-removal probes as evidence against every candidate. Assert that `c` survives.

7. **P1 — Refusal propagation treats quoted font names as token references.** [landing.rs:109](/tmp/rv-land-r2/tamber/src/import/landing.rs:109), [landing.rs:219](/tmp/rv-land-r2/tamber/src/import/landing.rs:219)

   With `tw.brand: length = 8px`, import:
   ```css
   @theme {
     --color-brand: #ff0000;
     --font-sans: "tw.brand";
   }
   ```
   Refusing the colour also refuses `tw.family.sans` with “it names `tw.brand`.” The quoted text is a literal font-family name and folds independently; the new word scanner incorrectly creates a dependency.

   **Fix:** extract semantic references while excluding strings, function names and literals. Add a regression preserving the family token while reporting only the colour.

WRONG
